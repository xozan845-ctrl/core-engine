//! Router HTTP y middlewares transversales.

pub mod componentes;
pub mod empresa;
pub mod envelope;
pub mod internal;
pub mod inventario;
pub mod items;
pub mod operaciones;
pub mod precios;
pub mod series;
pub mod tarifas;
pub mod ubicaciones;
pub mod validacion;

use crate::auth::{clave_interna_valida, UsuarioContexto};
use crate::config::Config;
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{FromRequest, MatchedPath, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/catalog-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · catalog-service</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
      window.ui = SwaggerUIBundle({ url: '/docs-json', dom_id: '#swagger-ui' });
    </script>
  </body>
</html>
"#;

pub fn build_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/docs", get(docs))
        .route("/docs-json", get(docs_json))
        .route(
            "/api/v2/catalog/items",
            get(items::listar).post(items::crear),
        )
        .route(
            "/api/v2/catalog/items/alquileres",
            post(items::crear_alquiler),
        )
        .route(
            "/api/v2/catalog/items/atributos/esquemas",
            get(items::esquemas).post(items::guardar_esquema),
        )
        .route(
            "/api/v2/catalog/items/{id}",
            get(items::detalle)
                .patch(items::actualizar)
                .delete(items::eliminar),
        )
        .route(
            "/api/v2/catalog/items/{id}/variantes",
            post(items::crear_variante),
        )
        .route(
            "/api/v2/catalog/items/{id}/variantes/{variante_id}",
            patch(items::actualizar_variante).delete(items::eliminar_variante),
        )
        .route(
            "/api/v2/catalog/items/{id}/imagenes",
            post(items::agregar_imagen),
        )
        .route(
            "/api/v2/catalog/items/{id}/imagenes/{imagen_id}",
            axum::routing::delete(items::eliminar_imagen),
        )
        // precios
        .route(
            "/api/v2/catalog/listas-precio",
            get(precios::listar_listas).post(precios::crear_lista),
        )
        .route(
            "/api/v2/catalog/listas-precio/{lista_id}",
            patch(precios::actualizar_lista),
        )
        .route(
            "/api/v2/catalog/listas-precio/{lista_id}/precios",
            get(precios::listar_precios).post(precios::crear_precio),
        )
        .route(
            "/api/v2/catalog/listas-precio/{lista_id}/precios/{precio_id}",
            patch(precios::actualizar_precio).delete(precios::eliminar_precio),
        )
        .route(
            "/api/v2/catalog/items/{id}/escalas",
            get(precios::listar_escalas).post(precios::crear_escala),
        )
        .route(
            "/api/v2/catalog/items/{id}/escalas/{escala_id}",
            patch(precios::actualizar_escala).delete(precios::eliminar_escala),
        )
        .route(
            "/api/v2/catalog/items/{id}/precio",
            get(precios::resolver_precio),
        )
        // tarifas
        .route(
            "/api/v2/catalog/items/{id}/tarifas",
            get(tarifas::listar).post(tarifas::crear),
        )
        .route(
            "/api/v2/catalog/items/{id}/tarifas/{tarifa_id}",
            patch(tarifas::actualizar).delete(tarifas::eliminar),
        )
        .route(
            "/api/v2/catalog/items/{id}/cotizacion",
            get(tarifas::cotizar),
        )
        // series
        .route(
            "/api/v2/catalog/items/{id}/series",
            get(series::listar).post(series::crear),
        )
        .route(
            "/api/v2/catalog/items/{id}/serializacion",
            patch(series::habilitar),
        )
        .route(
            "/api/v2/catalog/series/{id}",
            patch(series::actualizar).get(series::buscar),
        )
        // componentes
        .route(
            "/api/v2/catalog/items/{id}/componentes",
            get(componentes::listar).post(componentes::crear),
        )
        .route(
            "/api/v2/catalog/items/{id}/componentes/{componente_id}",
            patch(componentes::actualizar).delete(componentes::eliminar),
        )
        // operaciones
        .route(
            "/api/v2/catalog/configuracion-inventario",
            get(operaciones::configuracion).put(operaciones::guardar_configuracion),
        )
        .route(
            "/api/v2/catalog/items/importar",
            post(operaciones::importar),
        )
        .route("/api/v2/catalog/escanear", get(operaciones::escanear))
        .route(
            "/api/v2/catalog/combos",
            get(operaciones::listar_combos).post(operaciones::crear_combo),
        )
        .route(
            "/api/v2/catalog/combos/{id}",
            get(operaciones::detalle_combo)
                .patch(operaciones::actualizar_combo)
                .delete(operaciones::eliminar_combo),
        )
        .route(
            "/api/v2/catalog/promociones",
            get(operaciones::listar_promociones).post(operaciones::crear_promocion),
        )
        .route(
            "/api/v2/catalog/promociones/{id}",
            get(operaciones::detalle_promocion)
                .patch(operaciones::actualizar_promocion)
                .delete(operaciones::eliminar_promocion),
        )
        .route(
            "/api/v2/catalog/inventario/movimientos",
            get(operaciones::listar_movimientos).post(operaciones::crear_movimiento),
        )
        // superficie interna
        .route("/internal/items/lote", get(internal::lote_items))
        .route("/internal/items/precios", post(internal::precios_lote))
        .route("/internal/items/{id}", get(internal::item_por_id))
        .route("/internal/items/{id}/cotizacion", get(internal::cotizacion))
        .route("/internal/items/{id}/precio", get(internal::precio))
        .route(
            "/internal/inventario/resumen",
            get(internal::resumen_inventario),
        )
        .route(
            "/internal/inventario/lineas",
            get(internal::lineas_inventario),
        )
        .route("/api/v2/admin/inventario", get(inventario::listar))
        .route(
            "/api/v2/admin/inventario/proveedores",
            post(inventario::crear_proveedor),
        )
        .route(
            "/api/v2/admin/inventario/lotes",
            get(inventario::listar_lotes).post(inventario::crear_lote),
        )
        .route(
            "/api/v2/admin/inventario/lotes/consumir",
            post(inventario::consumir_lote),
        )
        .route(
            "/api/v2/admin/ubicaciones",
            get(ubicaciones::listar).post(ubicaciones::crear),
        )
        .route(
            "/api/v2/admin/ubicaciones/transferencias",
            post(ubicaciones::transferir),
        )
        .route(
            "/api/v2/admin/ubicaciones/items/{item_id}/stock",
            get(ubicaciones::stock_item),
        )
        .route(
            "/api/v2/admin/ubicaciones/{id}",
            patch(ubicaciones::actualizar),
        )
        .route(
            "/api/v2/admin/ubicaciones/{id}/stock",
            get(ubicaciones::stock_ubicacion),
        )
        .route(
            "/api/v2/admin/ubicaciones/{id}/stock/{item_id}",
            put(ubicaciones::fijar_stock),
        )
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

/// Capas en el mismo orden que los interceptores globales de Nest:
/// correlación (más externa) → métricas → envelope → handler.
pub(crate) fn aplicar_capas(router: Router, state: AppState) -> Router {
    router
        .layer(middleware::from_fn(envelope::envelope))
        .layer(middleware::from_fn_with_state(state.clone(), metricas))
        .layer(middleware::from_fn_with_state(state, correlacion))
}

/// Equivalente a la cadena global `RolesGuard` → `CapabilityGuard`.
pub(crate) async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
    roles: &[&str],
    permisos: &[&str],
    capacidades_extra: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    crate::auth::autorizar(
        &usuario,
        roles,
        permisos,
        capacidades_extra,
        state.capacidades.as_ref(),
    )
    .await?;
    Ok(usuario)
}

/// `true` si el modelo permite el tipo de ítem; lista vacía = sin restricción.
pub(crate) fn tipo_permitido(config: &crate::auth::ConfiguracionOperativa, tipo: &str) -> bool {
    config.items.is_empty() || config.items.iter().any(|permitido| permitido == tipo)
}

/// Valida el tipo de ítem contra los defaults del pack (identity interno).
pub(crate) async fn validar_tipo_permitido(
    state: &AppState,
    empresa: &str,
    tipo: &str,
) -> Result<(), ApiError> {
    let config = state
        .capacidades
        .configuracion(empresa)
        .await
        .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
    if let Some(config) = config {
        if !tipo_permitido(&config, tipo) {
            return Err(ApiError::dominio(
                "TIPO_ITEM_NO_PERMITIDO",
                format!("El modelo operativo de la empresa no permite items de tipo \"{tipo}\"."),
            ));
        }
    }
    Ok(())
}

/// Id del ítem en la vista (`item_id` o `id`).
fn id_de_item(item: &serde_json::Value) -> Option<&str> {
    item.get("item_id")
        .or_else(|| item.get("id"))
        .and_then(serde_json::Value::as_str)
}

/// Fusiona los valores de extensión de un ítem bajo `extensiones[codigo]`.
fn fusionar_extensiones(item: &mut serde_json::Value, valores: &[crate::auth::ValorExtension]) {
    let Some(id) = id_de_item(item).map(str::to_string) else {
        return;
    };
    let mut mapa = serde_json::Map::new();
    for valor in valores.iter().filter(|valor| valor.recurso_id == id) {
        mapa.insert(valor.codigo.clone(), valor.valores.clone());
    }
    if !mapa.is_empty() {
        item["extensiones"] = serde_json::Value::Object(mapa);
    }
}

/// Adjunta extensiones a un ítem o a una página `{items:[...]}`. Es aditivo:
/// si identity no responde, la respuesta del catálogo no se bloquea.
pub(crate) async fn adjuntar_extensiones(
    state: &AppState,
    empresa: &str,
    respuesta: &mut serde_json::Value,
) {
    let ids: Vec<String> = respuesta
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(id_de_item)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| {
            id_de_item(respuesta)
                .map(str::to_string)
                .into_iter()
                .collect()
        });
    if ids.is_empty() {
        return;
    }
    let Some(valores) = state
        .capacidades
        .valores_extensiones(empresa, "item", &ids)
        .await
    else {
        return;
    };
    if let Some(items) = respuesta
        .get_mut("items")
        .and_then(serde_json::Value::as_array_mut)
    {
        for item in items {
            fusionar_extensiones(item, &valores);
        }
    } else {
        fusionar_extensiones(respuesta, &valores);
    }
}

async fn health(State(state): State<AppState>) -> Json<crate::metrics::MetricaSalud> {
    Json(state.metrics.salud())
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.metrics.texto(),
    )
}

async fn docs() -> Html<&'static str> {
    Html(DOCS_HTML)
}

async fn docs_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI_PUBLICO,
    )
}

async fn no_encontrado() -> Response {
    crate::errors::ApiError::nueva(
        "NO_ENCONTRADO",
        "Ruta no encontrada.",
        StatusCode::NOT_FOUND,
    )
    .into_response()
}

/// Cuerpo JSON con el shape de error de Nest cuando el payload es inválido.
/// Un cuerpo vacío se trata como `null` para que la validación DTO reporte los
/// campos requeridos (igual que el body-parser de Express).
pub struct Cuerpo(pub Value);

impl<S> FromRequest<S> for Cuerpo
where
    S: Send + Sync,
{
    type Rejection = crate::errors::ApiError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|_| {
                crate::errors::ApiError::nueva(
                    "HTTP_413",
                    "El cuerpo de la peticion excede el limite permitido.",
                    StatusCode::PAYLOAD_TOO_LARGE,
                )
            })?;
        if bytes.is_empty() {
            return Ok(Cuerpo(Value::Null));
        }
        serde_json::from_slice(&bytes).map(Cuerpo).map_err(|_| {
            crate::errors::ApiError::nueva(
                "SOLICITUD_INVALIDA",
                "Peticion invalida.",
                StatusCode::BAD_REQUEST,
            )
        })
    }
}

/// ID de correlación, logs de petición y contexto para RLS.
async fn correlacion(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let usuario = UsuarioContexto::desde_headers(request.headers());
    // Las rutas internas no llevan JWT de usuario, pero deben poder consultar
    // datos cross-tenant bajo RLS: se expresa como claim confiable, igual que
    // `TrazabilidadInterceptor`.
    let interna = clave_interna_valida(request.headers(), state.config.internal_api_key.as_deref());
    let usuario_id = usuario
        .as_ref()
        .map(|u| u.user_id.clone())
        .or_else(|| interna.then(|| "00000000-0000-0000-0000-000000000000".to_string()));
    let rol = usuario
        .as_ref()
        .map(|u| u.rol.clone())
        .or_else(|| interna.then(|| "super_admin".to_string()));
    let metodo = request.method().as_str().to_string();
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let contexto = ContextoCorrelacion {
        request_id: request_id.clone(),
        servicio: state.config.servicio.clone(),
        usuario_id: usuario_id.clone(),
        rol,
        tenant_id: usuario.as_ref().and_then(|u| u.tenant_id.clone()),
    };

    let mut response = ejecutar_con_contexto(contexto, async move {
        crate::logs::info(
            "http",
            crate::campos! {
                "msg": "peticion",
                "metodo": metodo,
                "ruta": ruta,
                "usuario": usuario_id,
            },
        );
        next.run(request).await
    })
    .await;
    if let Ok(valor) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", valor);
    }
    response
}

/// Latencia, totales y errores por ruta (mismos nombres que `MetricsService`).
async fn metricas(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let metodo = request.method().as_str().to_string();
    let plantilla = request
        .extensions()
        .get::<MatchedPath>()
        .map(|ruta| ruta.as_str().to_string());
    let inicio = Instant::now();
    let response = next.run(request).await;
    let ruta = plantilla.unwrap_or_else(|| "/_unmatched".to_string());
    state.metrics.registrar_peticion(
        &metodo,
        &ruta,
        response.status().as_u16(),
        inicio.elapsed().as_millis() as u64,
    );
    response
}

/// Construye el estado de pruebas sin depender de infraestructura.
pub fn estado_de_prueba(config: Config) -> AppState {
    AppState::nueva(config.clone(), Metrics::nueva(config.servicio.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "catalog-service")]).expect("config")
    }

    async fn cuerpo(response: Response) -> String {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("cuerpo")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("utf8")
    }

    #[tokio::test]
    async fn debe_responder_salud_sin_envelope() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let texto = cuerpo(response).await;
        let valor: serde_json::Value = serde_json::from_str(&texto).expect("json");
        assert_eq!(valor["servicio"], "catalog-service");
        assert!(valor["uptime_seg"].is_number());
        assert!(valor.get("data").is_none());
    }

    #[tokio::test]
    async fn debe_exponer_metricas_en_texto_prometheus() {
        let state = estado_de_prueba(config());
        let app = build_router(state.clone());
        let _ = app
            .clone()
            .oneshot(
                HttpRequest::get("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("health");
        let response = app
            .oneshot(
                HttpRequest::get("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics");
        assert_eq!(response.status(), StatusCode::OK);
        let texto = cuerpo(response).await;
        assert!(texto.contains("core_engine_http_requests_total"));
        assert!(texto.contains("ruta=\"/health\""));
    }

    #[tokio::test]
    async fn debe_responder_404_estructurado_en_rutas_desconocidas() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/desconocido")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_envolver_las_rutas_publicas_y_propagar_request_id() {
        let state = estado_de_prueba(config());
        let app = aplicar_capas(
            Router::new()
                .route(
                    "/api/v2/prueba",
                    get(|| async { Json(serde_json::json!({ "ok": true })) }),
                )
                .with_state(state.clone()),
            state,
        );
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/prueba")
                    .header("x-request-id", "req-42")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()["x-request-id"], "req-42");
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["ok"], true);
        assert_eq!(valor["meta"]["path"], "/api/v2/prueba");
        assert_eq!(valor["meta"]["statusCode"], 200);
    }

    #[test]
    fn debe_permitir_cualquier_tipo_sin_restriccion() {
        let config = crate::auth::ConfiguracionOperativa::default();
        assert!(tipo_permitido(&config, "producto"));
        assert!(tipo_permitido(&config, "servicio"));
        assert!(tipo_permitido(&config, "alquiler"));
    }

    #[test]
    fn debe_respetar_los_items_del_modelo() {
        let config = crate::auth::ConfiguracionOperativa {
            items: vec!["servicio".to_string()],
            ..Default::default()
        };
        assert!(tipo_permitido(&config, "servicio"));
        assert!(!tipo_permitido(&config, "producto"));
    }

    struct CapacidadesRestringidas;

    #[async_trait::async_trait]
    impl crate::auth::Capacidades for CapacidadesRestringidas {
        async fn activas(
            &self,
            _empresa_id: &str,
            _requeridas: &[&str],
        ) -> Result<Vec<crate::auth::CapacidadActiva>, ()> {
            Ok(Vec::new())
        }

        async fn configuracion(
            &self,
            _empresa_id: &str,
        ) -> Result<Option<crate::auth::ConfiguracionOperativa>, ()> {
            Ok(Some(crate::auth::ConfiguracionOperativa {
                items: vec!["servicio".to_string()],
                ..Default::default()
            }))
        }
    }

    #[tokio::test]
    async fn debe_rechazar_un_tipo_que_el_modelo_no_permite() {
        let mut state = estado_de_prueba(config());
        state.capacidades = std::sync::Arc::new(CapacidadesRestringidas);
        let error = validar_tipo_permitido(&state, "empresa", "producto")
            .await
            .expect_err("tipo no permitido");
        assert_eq!(error.codigo, "TIPO_ITEM_NO_PERMITIDO");
        validar_tipo_permitido(&state, "empresa", "servicio")
            .await
            .expect("tipo permitido");
    }

    #[test]
    fn debe_fusionar_extensiones_por_item() {
        let mut item = serde_json::json!({ "item_id": "i-1", "nombre": "Cafe" });
        let valores = vec![crate::auth::ValorExtension {
            recurso_id: "i-1".into(),
            codigo: "ficha".into(),
            version_schema: 1,
            valores: serde_json::json!({ "edad": 20 }),
        }];
        fusionar_extensiones(&mut item, &valores);
        assert_eq!(item["extensiones"]["ficha"]["edad"], 20);
    }

    #[test]
    fn debe_ignorar_extensiones_de_otro_item() {
        let mut item = serde_json::json!({ "item_id": "i-1" });
        let valores = vec![crate::auth::ValorExtension {
            recurso_id: "i-2".into(),
            codigo: "ficha".into(),
            version_schema: 1,
            valores: serde_json::json!({ "edad": 20 }),
        }];
        fusionar_extensiones(&mut item, &valores);
        assert!(item.get("extensiones").is_none());
    }

    #[tokio::test]
    async fn debe_servir_el_openapi_versionado() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/docs-json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-servicio"], "catalog-service");
    }
}
