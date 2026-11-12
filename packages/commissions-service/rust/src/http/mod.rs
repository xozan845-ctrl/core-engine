//! Router HTTP y middlewares transversales.
//!
//! Capas en el mismo orden que los interceptores globales de Nest:
//! correlación (más externa) → métricas → envelope → handler.

pub mod envelope;
pub mod rutas;

use crate::auth::UsuarioContexto;
use crate::campos;
use crate::config::Config;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../../docs/openapi/commissions-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · commissions-service</title>
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
            "/api/v2/vendedores/me/liquidaciones",
            get(rutas::mis_liquidaciones),
        )
        .route(
            "/api/v2/empresarios/me/liquidaciones",
            get(rutas::liquidaciones_empresa),
        )
        .route(
            "/api/v2/admin/liquidaciones/corte",
            post(rutas::corte_manual),
        )
        .route(
            "/api/v2/admin/liquidaciones/{id}/pagar",
            post(rutas::pagar_liquidacion),
        )
        .route(
            "/api/v2/vendedores/me/ventas",
            get(rutas::estado_cuenta_vendedor),
        )
        .route(
            "/api/v2/empresarios/me/ventas",
            get(rutas::estado_cuenta_empresa),
        )
        .route("/api/v2/admin/reportes", get(rutas::reportes))
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

pub(crate) fn aplicar_capas(router: Router, state: AppState) -> Router {
    router
        .layer(middleware::from_fn(envelope::envelope))
        .layer(middleware::from_fn_with_state(state.clone(), metricas))
        .layer(middleware::from_fn_with_state(state, correlacion))
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
    let usuario_id = usuario.as_ref().map(|u| u.user_id.clone());
    let metodo = request.method().as_str().to_string();
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let contexto = ContextoCorrelacion {
        request_id: request_id.clone(),
        servicio: state.config.servicio.clone(),
        usuario_id: usuario.as_ref().map(|u| u.user_id.clone()),
        rol: usuario.as_ref().map(|u| u.rol.clone()),
        tenant_id: usuario.as_ref().and_then(|u| u.tenant_id.clone()),
    };

    let mut response = ejecutar_con_contexto(contexto, async move {
        crate::logs::info(
            "http",
            campos! {
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
    use crate::auth::{CapacidadActiva, Capacidades};
    use crate::comisiones::{calcular_con_tasa, Calculo, Comision, ComisionesPort};
    use crate::errors::ApiError;
    use crate::liquidaciones::{Liquidacion, LiquidacionEmpresa, LiquidacionesPort, Periodo};
    use crate::reportes::{ReporteKpi, ReportesPort};
    use crate::ventas::{EstadoCuenta, EstadoCuentaEmpresa, VentasPort};
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use chrono::NaiveDate;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "commissions-service")]).expect("config")
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

    fn liquidacion() -> Liquidacion {
        Liquidacion {
            id: "l-1".into(),
            vendedor_id: "v-1".into(),
            periodo_inicio: "2026-09-01T06:00:00.000Z".into(),
            periodo_fin: "2026-09-15T06:00:00.000Z".into(),
            monto_cents: 18_000,
            estado: "aprobada".into(),
            creado_en: "2026-09-16T00:00:00.000Z".into(),
        }
    }

    struct CapsFijas {
        filas: Vec<CapacidadActiva>,
        falla: bool,
    }

    #[async_trait]
    impl Capacidades for CapsFijas {
        async fn activas(
            &self,
            _empresa_id: &str,
            _requeridas: &[&str],
        ) -> Result<Vec<CapacidadActiva>, ()> {
            if self.falla {
                Err(())
            } else {
                Ok(self.filas.clone())
            }
        }
    }

    fn caps(activa: bool) -> Arc<dyn Capacidades> {
        Arc::new(CapsFijas {
            filas: vec![CapacidadActiva {
                empresa_id: "e-1".into(),
                capacidad_codigo: "liquidaciones".into(),
                activo: activa,
            }],
            falla: false,
        })
    }

    struct LiquidacionesFake;

    #[async_trait]
    impl LiquidacionesPort for LiquidacionesFake {
        async fn cerrar_periodo(&self, _periodo: Periodo) -> Result<Vec<Liquidacion>, ApiError> {
            Ok(vec![liquidacion()])
        }

        async fn cerrar_periodo_anterior(
            &self,
            _fecha: NaiveDate,
        ) -> Result<Vec<Liquidacion>, ApiError> {
            Ok(vec![liquidacion()])
        }

        async fn de_vendedor(&self, _vendedor_id: &str) -> Result<Vec<Liquidacion>, ApiError> {
            Ok(vec![liquidacion()])
        }

        async fn de_empresa(&self, _empresa_id: &str) -> Result<Vec<LiquidacionEmpresa>, ApiError> {
            Ok(vec![])
        }

        async fn marcar_pagada(
            &self,
            liquidacion_id: &str,
        ) -> Result<Option<Liquidacion>, ApiError> {
            if liquidacion_id == "existe" {
                Ok(Some(liquidacion()))
            } else {
                Ok(None)
            }
        }
    }

    struct VentasFake;

    #[async_trait]
    impl VentasPort for VentasFake {
        async fn estado_cuenta(&self, vendedor_id: &str) -> Result<EstadoCuenta, ApiError> {
            Ok(EstadoCuenta {
                vendedor_id: vendedor_id.into(),
                ventas: vec![],
                comisiones_pendientes: "0.00".into(),
                liquidaciones: vec![],
            })
        }

        async fn estado_cuenta_empresa(
            &self,
            empresa_id: &str,
        ) -> Result<EstadoCuentaEmpresa, ApiError> {
            Ok(EstadoCuentaEmpresa {
                empresa_id: empresa_id.into(),
                ventas: vec![],
                comisiones_pendientes: "0.00".into(),
                liquidaciones: vec![],
            })
        }
    }

    struct ReportesFake;

    #[async_trait]
    impl ReportesPort for ReportesFake {
        async fn kpis(&self) -> Result<ReporteKpi, ApiError> {
            Ok(crate::reportes::construir_reporte(
                0,
                0,
                0,
                0,
                crate::clients::InventarioResumen::default(),
                0,
            ))
        }
    }

    struct ComisionesFake;

    #[async_trait]
    impl ComisionesPort for ComisionesFake {
        fn calcular(&self, venta_cents: i64, participacion: i64) -> Result<Calculo, ApiError> {
            calcular_con_tasa(venta_cents, participacion, 0.12)
        }

        fn tasa(&self) -> f64 {
            0.12
        }

        async fn comisiones_de(&self, _vendedor_id: &str) -> Result<Vec<Comision>, ApiError> {
            Ok(vec![])
        }

        async fn comisiones_por_orden(&self, _order_id: &str) -> Result<Vec<Comision>, ApiError> {
            Ok(vec![])
        }
    }

    fn estado_con(caps: Arc<dyn Capacidades>) -> AppState {
        AppState::con_dependencias(
            config(),
            Arc::new(Metrics::nueva("commissions-service")),
            Arc::new(ComisionesFake),
            Arc::new(LiquidacionesFake),
            Arc::new(VentasFake),
            Arc::new(ReportesFake),
            caps,
        )
    }

    fn headers(rol: &str, tenant: Option<&str>) -> Vec<(&'static str, String)> {
        let mut valores = vec![
            ("x-user-id", "v-1".to_string()),
            ("x-user-rol", rol.to_string()),
        ];
        if let Some(tenant) = tenant {
            valores.push(("x-tenant", tenant.to_string()));
        }
        valores
    }

    async fn pedir(
        app: axum::Router,
        metodo: &str,
        ruta: &str,
        valores: Vec<(&'static str, String)>,
    ) -> Response {
        let mut constructor = HttpRequest::builder().method(metodo).uri(ruta);
        for (clave, valor) in valores {
            constructor = constructor.header(clave, valor);
        }
        app.oneshot(constructor.body(Body::empty()).expect("request"))
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn debe_responder_salud_sin_envelope() {
        let app = build_router(estado_de_prueba(config()));
        let response = pedir(app, "GET", "/health", vec![]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["servicio"], "commissions-service");
        assert!(valor["uptime_seg"].is_number());
        assert!(valor.get("data").is_none());
    }

    #[tokio::test]
    async fn debe_exponer_metricas_en_texto_prometheus() {
        let state = estado_de_prueba(config());
        let app = build_router(state);
        let _ = pedir(app.clone(), "GET", "/health", vec![]).await;
        let response = pedir(app, "GET", "/metrics", vec![]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let texto = cuerpo(response).await;
        assert!(texto.contains("core_engine_http_requests_total"));
        assert!(texto.contains("ruta=\"/health\""));
    }

    #[tokio::test]
    async fn debe_responder_404_estructurado_en_rutas_desconocidas() {
        let app = build_router(estado_de_prueba(config()));
        let response = pedir(app, "GET", "/api/v2/desconocido", vec![]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_servir_el_openapi_versionado() {
        let app = build_router(estado_de_prueba(config()));
        let response = pedir(app, "GET", "/docs-json", vec![]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-servicio"], "commissions-service");
        assert!(valor["paths"]["/api/v2/vendedores/me/ventas"].is_object());
    }

    #[tokio::test]
    async fn debe_envolver_una_respuesta_publica_y_propagar_request_id() {
        let app = build_router(estado_con(caps(true)));
        let mut valores = headers("vendedor", Some("e-1"));
        valores.push(("x-request-id", "req-42".to_string()));
        let response = pedir(app, "GET", "/api/v2/vendedores/me/ventas", valores).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-request-id"], "req-42");
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["vendedor_id"], "v-1");
        assert_eq!(valor["meta"]["path"], "/api/v2/vendedores/me/ventas");
        assert_eq!(valor["meta"]["method"], "GET");
        assert_eq!(valor["meta"]["statusCode"], 200);
    }

    #[tokio::test]
    async fn debe_responder_401_sin_contexto_autenticado() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(app, "GET", "/api/v2/vendedores/me/ventas", vec![]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_AUTORIZADO");
        assert_eq!(valor["mensaje"], "Se requiere un token valido.");
    }

    #[tokio::test]
    async fn debe_responder_403_con_un_rol_incorrecto() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(
            app,
            "GET",
            "/api/v2/vendedores/me/ventas",
            headers("comprador", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "ACCESO_DENEGADO");
        assert_eq!(
            valor["mensaje"],
            "Rol \"comprador\" no tiene acceso a este recurso."
        );
    }

    #[tokio::test]
    async fn debe_exigir_tenant_para_consultar_la_capacidad() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(
            app,
            "GET",
            "/api/v2/vendedores/me/ventas",
            headers("vendedor", None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "TENANT_REQUERIDO");
    }

    #[tokio::test]
    async fn debe_responder_403_cuando_la_capacidad_esta_inactiva() {
        let app = build_router(estado_con(caps(false)));
        let response = pedir(
            app,
            "GET",
            "/api/v2/vendedores/me/ventas",
            headers("vendedor", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "CAPACIDAD_NO_ACTIVA");
        assert_eq!(valor["detalles"]["capacidades"][0], "liquidaciones");
    }

    #[tokio::test]
    async fn debe_responder_503_cuando_identity_no_esta_disponible() {
        let app = build_router(estado_con(Arc::new(CapsFijas {
            filas: vec![],
            falla: true,
        })));
        let response = pedir(
            app,
            "GET",
            "/api/v2/vendedores/me/ventas",
            headers("vendedor", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "SERVICIO_NO_DISPONIBLE");
    }

    #[tokio::test]
    async fn debe_crear_el_corte_con_201_y_envelope() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(
            app,
            "POST",
            "/api/v2/admin/liquidaciones/corte",
            headers("admin", None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"][0]["id"], "l-1");
        assert_eq!(valor["meta"]["statusCode"], 201);
    }

    #[tokio::test]
    async fn debe_devolver_404_al_pagar_una_liquidacion_inexistente() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(
            app,
            "POST",
            "/api/v2/admin/liquidaciones/no-existe/pagar",
            headers("super_admin", None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_pagar_una_liquidacion_existente_con_201() {
        let app = build_router(estado_con(caps(true)));
        let response = pedir(
            app,
            "POST",
            "/api/v2/admin/liquidaciones/existe/pagar?estado=pagada",
            headers("admin", None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["id"], "l-1");
        assert_eq!(valor["meta"]["statusCode"], 201);
    }
}
