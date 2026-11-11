//! Rutas de agenda (`/api/v2/scheduling/**`) y de la superficie interna.
//!
//! El orden replica los guards globales de Nest: contexto/rol → capacidad del
//! tenant → validación → handler (R-PR-6, R-DO-8).

use crate::auth::{UsuarioContexto, ROLES_AGENDA};
use crate::errors::ApiError;
use crate::http::Cuerpo;
use crate::scheduling::{
    validar_crear_bloqueo, validar_crear_cita, validar_crear_disponibilidad, validar_crear_recurso,
    validar_verificar_disponibilidad,
};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

pub const CAPACIDAD_AGENDA: &str = "agenda_recursos";

async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
    permiso: &str,
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    usuario.exigir_roles(&ROLES_AGENDA, &[permiso])?;

    if matches!(usuario.rol.as_str(), "super_admin" | "admin") {
        return Ok(usuario);
    }

    let tenant = usuario.tenant()?;
    let activas = state
        .capacidades
        .activas(tenant, &[CAPACIDAD_AGENDA])
        .await
        .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
    let activa = activas.iter().any(|capacidad| {
        capacidad.empresa_id == tenant
            && capacidad.capacidad_codigo == CAPACIDAD_AGENDA
            && capacidad.activo
    });
    if !activa {
        return Err(ApiError::nueva(
            "CAPACIDAD_NO_ACTIVA",
            format!("La empresa no tiene activas las capacidades requeridas: {CAPACIDAD_AGENDA}."),
            axum::http::StatusCode::FORBIDDEN,
        )
        .con_detalles(json!({
            "empresa_id": tenant,
            "capacidades": [CAPACIDAD_AGENDA],
        })));
    }
    Ok(usuario)
}

pub async fn listar_recursos(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.leer").await?;
    let tenant = usuario.tenant()?;
    Ok(Json(json!(state.agenda.listar_recursos(tenant).await?)))
}

pub async fn crear_recurso(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.escribir").await?;
    let tenant = usuario.tenant()?;
    let dto = validar_crear_recurso(&cuerpo)?;
    Ok((
        StatusCode::CREATED,
        Json(state.agenda.crear_recurso(tenant, &dto).await?),
    ))
}

pub async fn listar_disponibilidad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(recurso_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.leer").await?;
    let tenant = usuario.tenant()?;
    Ok(Json(json!(
        state
            .agenda
            .listar_disponibilidad(tenant, &recurso_id)
            .await?
    )))
}

pub async fn crear_disponibilidad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.escribir").await?;
    let tenant = usuario.tenant()?;
    let dto = validar_crear_disponibilidad(&cuerpo)?;
    Ok(Json(state.agenda.crear_disponibilidad(tenant, &dto).await?))
}

pub async fn crear_bloqueo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.escribir").await?;
    let tenant = usuario.tenant()?;
    let dto = validar_crear_bloqueo(&cuerpo)?;
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .agenda
                .crear_bloqueo(tenant, Some(&usuario.user_id), &dto)
                .await?,
        ),
    ))
}

#[derive(Debug, Deserialize)]
pub struct FiltroCitas {
    pub desde: Option<String>,
    pub hasta: Option<String>,
}

pub async fn listar_citas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroCitas>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.leer").await?;
    let tenant = usuario.tenant()?;
    Ok(Json(json!(
        state
            .agenda
            .listar_citas(tenant, filtro.desde.as_deref(), filtro.hasta.as_deref())
            .await?
    )))
}

pub async fn crear_cita(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.escribir").await?;
    let tenant = usuario.tenant()?;
    let dto = validar_crear_cita(&cuerpo)?;
    Ok((
        StatusCode::CREATED,
        Json(state.agenda.crear_cita(tenant, &dto).await?),
    ))
}

pub async fn cancelar_cita(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cita_id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.escribir").await?;
    let tenant = usuario.tenant()?;
    let motivo = cuerpo.get("motivo").and_then(Value::as_str);
    Ok(Json(
        state.agenda.cancelar_cita(tenant, &cita_id, motivo).await?,
    ))
}

pub async fn listar_recordatorios(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, "agenda.leer").await?;
    let tenant = usuario.tenant()?;
    Ok(Json(json!(
        state.agenda.listar_recordatorios(tenant).await?
    )))
}

/// Superficie interna: `orders` valida una reserva antes de crearla.
pub async fn verificar_disponibilidad(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let clave = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok());
    if clave.is_none() || clave != state.config.internal_api_key.as_deref() {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    let dto = validar_verificar_disponibilidad(&cuerpo)?;
    state.agenda.verificar_disponibilidad(&dto).await?;
    Ok((StatusCode::CREATED, Json(json!({ "disponible": true }))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{CapacidadActiva, Capacidades};
    use crate::config::Config;
    use crate::http::{aplicar_capas, estado_de_prueba};
    use crate::metrics::Metrics;
    use crate::scheduling::{
        AgendaPort, CrearBloqueo, CrearCita, CrearDisponibilidad, CrearRecurso,
        VerificarDisponibilidad,
    };
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    const TENANT: &str = "11111111-1111-4111-8111-111111111111";

    struct AgendaFalsa;

    #[async_trait]
    impl AgendaPort for AgendaFalsa {
        async fn crear_recurso(&self, _: &str, dto: &CrearRecurso) -> Result<Value, ApiError> {
            Ok(json!({ "id": "r-1", "nombre": dto.nombre }))
        }
        async fn listar_recursos(&self, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![json!({ "id": "r-1" })])
        }
        async fn crear_disponibilidad(
            &self,
            _: &str,
            _: &CrearDisponibilidad,
        ) -> Result<Value, ApiError> {
            Ok(json!({ "id": "d-1" }))
        }
        async fn listar_disponibilidad(&self, _: &str, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn crear_bloqueo(
            &self,
            _: &str,
            _: Option<&str>,
            _: &CrearBloqueo,
        ) -> Result<Value, ApiError> {
            Ok(json!({ "id": "b-1" }))
        }
        async fn crear_cita(&self, _: &str, _: &CrearCita) -> Result<Value, ApiError> {
            Ok(json!({ "id": "c-1", "estado": "reservada" }))
        }
        async fn listar_citas(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<Vec<Value>, ApiError> {
            Ok(vec![json!({ "id": "c-1" })])
        }
        async fn cancelar_cita(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<Value, ApiError> {
            Ok(json!({ "id": "c-1", "estado": "cancelada" }))
        }
        async fn listar_recordatorios(&self, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn crear_desde_reserva(
            &self,
            _: &crate::scheduling::OrdenReserva,
        ) -> Result<Value, ApiError> {
            Ok(json!({ "id": "c-1" }))
        }
        async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
            Ok(0)
        }
        async fn verificar_disponibilidad(
            &self,
            _: &VerificarDisponibilidad,
        ) -> Result<(), ApiError> {
            Ok(())
        }
    }

    struct CapacidadesFijas(bool);

    #[async_trait]
    impl Capacidades for CapacidadesFijas {
        async fn activas(&self, empresa_id: &str, _: &[&str]) -> Result<Vec<CapacidadActiva>, ()> {
            Ok(vec![CapacidadActiva {
                empresa_id: empresa_id.to_string(),
                capacidad_codigo: CAPACIDAD_AGENDA.to_string(),
                activo: self.0,
            }])
        }
    }

    fn router(capacidad_activa: bool) -> Router {
        let config = Config::from_pairs([
            ("SERVICIO", "scheduling-service"),
            ("INTERNAL_API_KEY", "clave-interna"),
        ])
        .expect("config");
        let state = AppState::con_dependencias(
            config,
            Arc::new(Metrics::nueva("scheduling-service")),
            Arc::new(AgendaFalsa),
            Arc::new(CapacidadesFijas(capacidad_activa)),
        );
        aplicar_capas(
            Router::new()
                .route(
                    "/api/v2/scheduling/recursos",
                    get(listar_recursos).post(crear_recurso),
                )
                .route(
                    "/api/v2/scheduling/recursos/{recurso_id}/disponibilidad",
                    get(listar_disponibilidad),
                )
                .route(
                    "/api/v2/scheduling/disponibilidad",
                    post(crear_disponibilidad),
                )
                .route("/api/v2/scheduling/bloqueos", post(crear_bloqueo))
                .route(
                    "/internal/scheduling/disponibilidad",
                    post(verificar_disponibilidad),
                )
                .with_state(state.clone()),
            state,
        )
    }

    fn headers(rol: &str) -> Vec<(&'static str, String)> {
        vec![
            ("x-user-id", "u-1".to_string()),
            ("x-user-rol", rol.to_string()),
            ("x-tenant", TENANT.to_string()),
        ]
    }

    fn peticion(
        metodo: &str,
        ruta: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: Option<Value>,
    ) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder().method(metodo).uri(ruta);
        for (clave, valor) in cabeceras {
            builder = builder.header(*clave, valor);
        }
        match cuerpo {
            Some(valor) => builder
                .header("content-type", "application/json")
                .body(Body::from(valor.to_string()))
                .expect("request"),
            None => builder.body(Body::empty()).expect("request"),
        }
    }

    async fn json(response: axum::response::Response) -> Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("cuerpo")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("json")
    }

    #[tokio::test]
    async fn debe_rechazar_sin_contexto_autenticado() {
        let response = router(true)
            .oneshot(peticion("GET", "/api/v2/scheduling/recursos", &[], None))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(json(response).await["codigo"], "NO_AUTORIZADO");
    }

    #[tokio::test]
    async fn debe_rechazar_un_rol_fuera_de_la_agenda() {
        let response = router(true)
            .oneshot(peticion(
                "GET",
                "/api/v2/scheduling/recursos",
                &headers("comprador"),
                None,
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json(response).await["codigo"], "ACCESO_DENEGADO");
    }

    #[tokio::test]
    async fn debe_rechazar_si_el_tenant_no_tiene_la_capacidad() {
        let response = router(false)
            .oneshot(peticion(
                "GET",
                "/api/v2/scheduling/recursos",
                &headers("empresario"),
                None,
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let cuerpo = json(response).await;
        assert_eq!(cuerpo["codigo"], "CAPACIDAD_NO_ACTIVA");
        assert_eq!(cuerpo["detalles"]["empresa_id"], TENANT);
    }

    #[tokio::test]
    async fn debe_envolver_el_listado_en_el_envelope() {
        let response = router(true)
            .oneshot(peticion(
                "GET",
                "/api/v2/scheduling/recursos",
                &headers("empresario"),
                None,
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let cuerpo = json(response).await;
        assert_eq!(cuerpo["data"][0]["id"], "r-1");
        assert_eq!(cuerpo["meta"]["path"], "/api/v2/scheduling/recursos");
    }

    #[tokio::test]
    async fn debe_validar_el_cuerpo_despues_de_los_guards() {
        let response = router(true)
            .oneshot(peticion(
                "POST",
                "/api/v2/scheduling/recursos",
                &headers("empresario"),
                Some(json!({ "tipo": "nave" })),
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let cuerpo = json(response).await;
        assert_eq!(cuerpo["codigo"], "VALIDACION");
        assert_eq!(
            cuerpo["detalles"],
            json!([
                {"campo": "tipo", "restricciones": ["isIn"]},
                {"campo": "nombre", "restricciones": ["isString"]},
            ])
        );
    }

    #[tokio::test]
    async fn debe_responder_201_en_creaciones_y_reflejarlo_en_el_envelope() {
        let response = router(true)
            .oneshot(peticion(
                "POST",
                "/api/v2/scheduling/recursos",
                &headers("empresario"),
                Some(json!({ "tipo": "sala", "nombre": "Sala 1" })),
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let cuerpo = json(response).await;
        assert_eq!(cuerpo["meta"]["statusCode"], 201);
        assert_eq!(cuerpo["data"]["id"], "r-1");
    }

    #[tokio::test]
    async fn debe_exigir_la_clave_interna_sin_envelope() {
        let sin_clave = router(true)
            .oneshot(peticion(
                "POST",
                "/internal/scheduling/disponibilidad",
                &[],
                Some(json!({
                    "empresa_id": TENANT, "recurso_id": "22222222-2222-4222-8222-222222222222",
                    "inicia_en": "2030-04-15T14:00:00Z", "termina_en": "2030-04-15T15:00:00Z"
                })),
            ))
            .await
            .expect("response");
        assert_eq!(sin_clave.status(), StatusCode::FORBIDDEN);

        let response = router(true)
            .oneshot(peticion(
                "POST",
                "/internal/scheduling/disponibilidad",
                &[("x-internal-key", "clave-interna".to_string())],
                Some(json!({
                    "empresa_id": TENANT, "recurso_id": "22222222-2222-4222-8222-222222222222",
                    "inicia_en": "2030-04-15T14:00:00Z", "termina_en": "2030-04-15T15:00:00Z"
                })),
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(json(response).await, json!({ "disponible": true }));
    }

    #[tokio::test]
    async fn debe_responder_503_si_identity_no_esta_disponible() {
        let config = Config::from_pairs([
            ("SERVICIO", "scheduling-service"),
            ("INTERNAL_API_KEY", "clave-interna"),
        ])
        .expect("config");
        let state = AppState::con_dependencias(
            config,
            Arc::new(Metrics::nueva("scheduling-service")),
            Arc::new(AgendaFalsa),
            Arc::new(crate::auth::CapacidadesNoDisponible),
        );
        let app = aplicar_capas(
            Router::new()
                .route("/api/v2/scheduling/recursos", get(listar_recursos))
                .with_state(state.clone()),
            state,
        );
        let response = app
            .oneshot(peticion(
                "GET",
                "/api/v2/scheduling/recursos",
                &headers("empresario"),
                None,
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json(response).await["codigo"], "SERVICIO_NO_DISPONIBLE");
    }

    #[test]
    fn debe_mantener_el_estado_de_prueba_sin_dependencias() {
        let state = estado_de_prueba(
            Config::from_pairs([("SERVICIO", "scheduling-service")]).expect("config"),
        );
        assert!(Arc::strong_count(&state.metrics) >= 1);
    }
}
