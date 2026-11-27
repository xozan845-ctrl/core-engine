//! Proxy HTTP hacia los microservicios y tabla de destinos (Tabla 21).

use crate::auth::aplicar_auth;
use crate::config::Config;
use crate::errors::ApiError;
use crate::policy::encontrar_politica;
use crate::rate_limit::{es_media_publica, ip_cliente};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use regex::Regex;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Límite de cuerpo que el gateway materializa en memoria antes de reenviar.
/// Los servicios tienen su propio límite de 100 KiB para JSON; este margen
/// cubre uploads pequeños y evita que un POST grande tumbe el perímetro.
const MAX_CUERPO_BYTES: usize = 2 * 1024 * 1024;

fn cuerpo_excede(longitud: usize) -> bool {
    longitud > MAX_CUERPO_BYTES
}

pub struct Destino {
    pub nombre: &'static str,
    pub url: String,
    pub patrones: Vec<(Regex, Vec<&'static str>)>,
}

fn patron(patron: &str, metodos: &[&'static str]) -> (Regex, Vec<&'static str>) {
    (
        Regex::new(patron).expect("regex de destino"),
        metodos.to_vec(),
    )
}

pub fn destinos(config: &Config) -> Vec<Destino> {
    vec![
        Destino {
            nombre: "orders",
            url: config.orders_url.clone(),
            patrones: vec![patron(
                r"^/api/v2/empresas/[^/]+/configuracion-pagos$",
                &["*"],
            )],
        },
        Destino {
            nombre: "identity",
            url: config.identity_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/auth/", &["*"]),
                patron(r"^/api/v2/usuarios(/|$)", &["*"]),
                patron(r"^/api/v2/empresas(/|$)", &["*"]),
                patron(r"^/api/v2/incorporaciones(/|$)", &["*"]),
                patron(r"^/api/v2/packs(/|$)", &["GET", "POST", "PUT"]),
                patron(r"^/api/v2/configuracion(/|$)", &["*"]),
                patron(r"^/api/v2/onboarding(/|$)", &["*"]),
                patron(r"^/api/v2/plataforma(/|$)", &["*"]),
                patron(r"^/api/v2/admin/plataforma(/|$)", &["*"]),
                patron(r"^/api/v2/api-keys(/|$)", &["*"]),
                patron(r"^/api/v2/exportaciones(/|$)", &["*"]),
                patron(r"^/api/v2/negocios(/|$)", &["*"]),
                patron(r"^/api/v2/regiones-tributarias$", &["GET"]),
                patron(r"^/api/v2/admin/auditoria(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "storage",
            url: config.storage_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/storage(/|$)", &["*"]),
                patron(r"^/api/v2/admin/storage(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "inventory",
            url: config.inventory_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/admin/inventario(/|$)", &["*"]),
                patron(r"^/api/v2/admin/ubicaciones(/|$)", &["*"]),
                patron(r"^/api/v2/catalog/inventario(/|$)", &["*"]),
                patron(r"^/api/v2/catalog/configuracion-inventario$", &["*"]),
            ],
        },
        Destino {
            nombre: "pricing",
            url: config.pricing_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/catalog/listas-precio(/|$)", &["*"]),
                patron(
                    r"^/api/v2/catalog/items/[^/]+/(precio|escalas|tarifas|cotizacion)(/|$)",
                    &["*"],
                ),
                patron(r"^/api/v2/catalog/promociones(/|$)", &["*"]),
                patron(r"^/api/v2/catalog/cupones(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "item",
            url: config.item_url.clone(),
            patrones: vec![patron(r"^/api/v2/catalog/", &["*"])],
        },
        Destino {
            nombre: "stores",
            url: config.stores_url.clone(),
            patrones: vec![
                patron(
                    r"^/api/v2/vendedores/(productos|tienda|me/(ofertas|tienda))",
                    &["*"],
                ),
                patron(r"^/api/v2/empresarios/me/tiendas$", &["GET"]),
                patron(r"^/api/v2/tiendas/", &["*"]),
                patron(r"^/api/v2/tienda(/|$)", &["*"]),
                patron(r"^/api/v2/comprador(/|$)", &["*"]),
                patron(r"^/api/v2/admin/resenas(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "scheduling",
            url: config.scheduling_url.clone(),
            patrones: vec![patron(r"^/api/v2/scheduling(/|$)", &["*"])],
        },
        Destino {
            nombre: "logistics",
            url: config.logistics_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/orders/[^/]+/estado", &["PATCH", "POST"]),
                patron(r"^/api/v2/admin/envios", &["*"]),
                patron(r"^/api/v2/empresarios/me/envios", &["*"]),
                patron(r"^/api/v2/envios/[^/]+/estado$", &["PATCH"]),
                patron(r"^/api/v2/envios/[^/]+/prueba-entrega$", &["POST"]),
            ],
        },
        Destino {
            nombre: "orders",
            url: config.orders_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/orders(/|$)", &["*"]),
                patron(r"^/api/v2/carrito(/|$)", &["*"]),
                patron(r"^/api/v2/pagos/webhook$", &["POST"]),
                patron(r"^/api/v2/pagos/(tilopay|pagadito)/webhook$", &["POST"]),
                patron(r"^/api/v2/contratos(/|$)", &["*"]),
                patron(r"^/api/v2/suscripciones(/|$)", &["*"]),
                patron(r"^/api/v2/mesas(/|$)", &["*"]),
                patron(r"^/api/v2/comandas(/|$)", &["*"]),
                patron(r"^/api/v2/fidelizacion(/|$)", &["*"]),
                patron(r"^/api/v2/pedidos(/|$)", &["*"]),
                patron(r"^/api/v2/entregas-digitales(/|$)", &["*"]),
                patron(
                    r"^/api/v2/admin/(pagos|metodos-pago|tipos-orden|fulfillments|devoluciones|proveedores-pago)(/|$)",
                    &["*"],
                ),
            ],
        },
        Destino {
            nombre: "commissions",
            url: config.commissions_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/vendedores/me/(ventas|liquidaciones)", &["*"]),
                patron(r"^/api/v2/vendedores/me/dispersion$", &["GET"]),
                patron(r"^/api/v2/empresarios/me/(liquidaciones|ventas)", &["*"]),
                patron(r"^/api/v2/empresarios/me/dispersion$", &["GET"]),
                patron(r"^/api/v2/admin/reportes", &["*"]),
                patron(r"^/api/v2/admin/liquidaciones", &["*"]),
                patron(r"^/api/v2/admin/dispersion", &["*"]),
                patron(r"^/api/v2/empresarios/me/reportes/negocios$", &["*"]),
            ],
        },
        Destino {
            nombre: "finance",
            url: config.finance_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/finanzas(/|$)", &["*"]),
                patron(r"^/api/v2/comercio(/|$)", &["*"]),
                patron(r"^/api/v2/vendedores/me/fiscal", &["*"]),
                patron(r"^/api/v2/admin/tipos-cambio(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "field",
            url: config.field_url.clone(),
            patrones: vec![patron(r"^/api/v2/field/", &["*"])],
        },
        Destino {
            nombre: "intelligence",
            url: config.intelligence_url.clone(),
            patrones: vec![patron(r"^/api/v2/inteligencia(/|$)", &["*"])],
        },
        Destino {
            nombre: "notifications",
            url: config.notifications_url.clone(),
            patrones: vec![patron(r"^/api/v2/(admin/)?notifications(/|$)", &["*"])],
        },
        Destino {
            nombre: "analytics",
            url: config.analytics_url.clone(),
            patrones: vec![patron(r"^/api/v2/analytics(/|$)", &["*"])],
        },
    ]
}

pub fn encontrar_destino<'a>(
    destinos: &'a [Destino],
    ruta: &str,
    metodo: &str,
) -> Option<&'a Destino> {
    destinos.iter().find(|destino| {
        destino.patrones.iter().any(|(patron, metodos)| {
            patron.is_match(ruta)
                && (metodos
                    .iter()
                    .any(|m| *m == "*" || m.eq_ignore_ascii_case(metodo)))
        })
    })
}

/// Construye el `HeaderMap` limpio que viaja al microservicio. No copia
/// cabeceras del cliente salvo la clave de idempotencia y `Range` en media.
fn construir_cabeceras(
    request_id: &str,
    es_media: bool,
    auth: &HashMap<String, String>,
    rango: Option<&str>,
    idempotency_key: Option<&str>,
    user_agent: Option<&str>,
    ip_cliente: Option<&str>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(valor) = HeaderValue::from_str(request_id) {
        headers.insert("x-request-id", valor);
    }
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(if es_media {
            "application/octet-stream"
        } else {
            "application/json"
        }),
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static(if es_media {
            "image/*,video/*,*/*"
        } else {
            "application/json"
        }),
    );
    for (clave, valor) in auth {
        if let (Ok(nombre), Ok(valor)) = (
            axum::http::header::HeaderName::from_bytes(clave.as_bytes()),
            HeaderValue::from_str(valor),
        ) {
            headers.insert(nombre, valor);
        }
    }
    if es_media {
        if let Some(rango) = rango {
            if let Ok(valor) = HeaderValue::from_str(rango) {
                headers.insert(header::RANGE, valor);
            }
        }
    }
    if let Some(clave) = idempotency_key {
        if let Ok(valor) = HeaderValue::from_str(clave) {
            headers.insert("idempotency-key", valor);
        }
    }
    if let Some(agent) = user_agent {
        if let Ok(valor) = HeaderValue::from_str(agent) {
            headers.insert(header::USER_AGENT, valor);
        }
    }
    if let Some(ip) = ip_cliente {
        if let Ok(valor) = HeaderValue::from_str(ip) {
            headers.insert("x-client-ip", valor);
        }
    }
    headers
}

pub async fn reenviar(State(state): State<AppState>, request: Request) -> Response {
    let metodo = request.method().clone();
    let ruta = request.uri().path().to_string();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| format!("gw-{}", Uuid::new_v4()));
    let rango = request
        .headers()
        .get(header::RANGE)
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string);
    let idempotency_key = request
        .headers()
        .get("idempotency-key")
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string);
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string);
    let ruta_query = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| ruta.clone());
    let politica = encontrar_politica(&ruta, metodo.as_str());
    let auth = match aplicar_auth(
        request.headers(),
        metodo.as_str(),
        &ruta,
        politica,
        &state.config.jwt_secret,
    ) {
        Ok(auth) => auth,
        Err(error) => return error.into_response(),
    };
    let Some(destino) = encontrar_destino(&state.destinos, &ruta, metodo.as_str()) else {
        return ApiError::nueva(
            "NO_ENCONTRADO",
            format!("No existe ruta {} {}.", metodo, ruta),
            StatusCode::NOT_FOUND,
        )
        .into_response();
    };
    let es_media = es_media_publica(metodo.as_str(), &ruta);
    let ip_cliente = ip_cliente(request.headers(), state.config.confiar_en_forwarded);
    let cuerpo = if matches!(metodo, Method::GET | Method::HEAD | Method::OPTIONS) {
        Vec::new()
    } else {
        if let Some(longitud) = request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|valor| valor.to_str().ok())
            .and_then(|valor| valor.parse::<usize>().ok())
        {
            if cuerpo_excede(longitud) {
                return ApiError::nueva(
                    "HTTP_413",
                    "El cuerpo de la peticion excede el limite permitido.",
                    StatusCode::PAYLOAD_TOO_LARGE,
                )
                .into_response();
            }
        }
        match request.into_body().collect().await {
            Ok(bytes) => {
                let bytes = bytes.to_bytes();
                if cuerpo_excede(bytes.len()) {
                    return ApiError::nueva(
                        "HTTP_413",
                        "El cuerpo de la peticion excede el limite permitido.",
                        StatusCode::PAYLOAD_TOO_LARGE,
                    )
                    .into_response();
                }
                bytes.to_vec()
            }
            Err(_) => {
                return ApiError::nueva(
                    "HTTP_413",
                    "El cuerpo de la peticion excede el limite permitido.",
                    StatusCode::PAYLOAD_TOO_LARGE,
                )
                .into_response()
            }
        }
    };

    let headers = construir_cabeceras(
        &request_id,
        es_media,
        &auth,
        rango.as_deref(),
        idempotency_key.as_deref(),
        user_agent.as_deref(),
        ip_cliente.as_deref(),
    );

    let url = format!("{}{}", destino.url, ruta_query);
    let cliente = state.http.clone();
    let peticion = cliente
        .request(
            reqwest::Method::from_bytes(metodo.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .headers(headers)
        .timeout(Duration::from_millis(state.config.proxy_timeout_ms));
    let peticion = if cuerpo.is_empty() {
        peticion
    } else {
        peticion.body(cuerpo)
    };
    match peticion.send().await {
        Ok(respuesta) => {
            let estado = respuesta.status();
            let cabeceras = respuesta.headers().clone();
            let cuerpo = respuesta.bytes_stream();
            let mut salida = Response::new(Body::from_stream(cuerpo));
            *salida.status_mut() = estado;
            let no_reenviables = [
                "connection",
                "keep-alive",
                "transfer-encoding",
                "upgrade",
                "proxy-authenticate",
                "proxy-authorization",
                "te",
                "trailer",
            ];
            for (clave, valor) in cabeceras.iter() {
                if no_reenviables.contains(&clave.as_str()) {
                    continue;
                }
                salida.headers_mut().insert(clave.clone(), valor.clone());
            }
            salida
        }
        Err(error) if error.is_timeout() => ApiError::nueva(
            "DESTINO_TIMEOUT",
            format!(
                "El servicio de destino no respondio en {} ms.",
                state.config.proxy_timeout_ms
            ),
            StatusCode::GATEWAY_TIMEOUT,
        )
        .con_detalles(serde_json::json!({
            "servicio": destino.nombre,
            "timeout_ms": state.config.proxy_timeout_ms,
        }))
        .into_response(),
        Err(_) => ApiError::nueva(
            "DESTINO_NO_DISPONIBLE",
            "El servicio de destino no respondio (circuit breaker del gateway).",
            StatusCode::BAD_GATEWAY,
        )
        .con_detalles(serde_json::json!({ "servicio": destino.nombre }))
        .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config")
    }

    #[test]
    fn debe_reenviar_la_clave_de_idempotencia_sin_copiar_otras_cabeceras() {
        let mut auth = HashMap::new();
        auth.insert("x-user-id".to_string(), "u-1".to_string());
        let headers = construir_cabeceras(
            "req-1",
            false,
            &auth,
            None,
            Some("clave-idempotente-1"),
            Some("agente-test"),
            Some("1.2.3.4"),
        );
        assert_eq!(
            headers
                .get(header::USER_AGENT)
                .and_then(|valor| valor.to_str().ok()),
            Some("agente-test")
        );
        assert_eq!(
            headers
                .get("idempotency-key")
                .and_then(|valor| valor.to_str().ok()),
            Some("clave-idempotente-1")
        );
        assert_eq!(
            headers
                .get("x-user-id")
                .and_then(|valor| valor.to_str().ok()),
            Some("u-1")
        );
        assert_eq!(
            headers
                .get("x-client-ip")
                .and_then(|valor| valor.to_str().ok()),
            Some("1.2.3.4")
        );
        assert!(headers.get("x-tenant").is_none());
    }

    #[test]
    fn debe_rechazar_cuerpos_sobre_el_limite() {
        assert!(!cuerpo_excede(0));
        assert!(!cuerpo_excede(MAX_CUERPO_BYTES));
        assert!(cuerpo_excede(MAX_CUERPO_BYTES + 1));
    }

    #[test]
    fn todas_las_operaciones_publicas_del_contrato_tienen_destino() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/api/core-engine.openapi.json"))
                .expect("openapi publico");
        let destinos = destinos(&config());
        let mut faltantes = Vec::new();
        for (ruta, operaciones) in spec["paths"].as_object().expect("paths") {
            for metodo in operaciones.as_object().expect("metodos").keys() {
                if encontrar_destino(&destinos, ruta, metodo).is_none() {
                    faltantes.push(format!("{} {}", metodo.to_uppercase(), ruta));
                }
            }
        }
        assert!(
            faltantes.is_empty(),
            "operaciones publicas sin destino en el gateway:\n{}",
            faltantes.join("\n")
        );
    }

    #[test]
    fn debe_enrutar_por_servicio_y_metodo() {
        let destinos = destinos(&config());
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/orders", "POST").map(|d| d.nombre),
            Some("orders")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/orders/1/estado", "PATCH").map(|d| d.nombre),
            Some("logistics")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/pagos/webhook", "POST").map(|d| d.nombre),
            Some("orders")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/finanzas/resumen", "GET").map(|d| d.nombre),
            Some("finance")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/admin/dispersion/payouts", "GET")
                .map(|d| d.nombre),
            Some("commissions")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/vendedores/me/dispersion", "GET")
                .map(|d| d.nombre),
            Some("commissions")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/admin/inventario", "GET").map(|d| d.nombre),
            Some("inventory")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/catalog/inventario/movimientos", "GET")
                .map(|d| d.nombre),
            Some("inventory")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/catalog/items", "GET").map(|d| d.nombre),
            Some("item")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/analytics/empresa", "GET").map(|d| d.nombre),
            Some("analytics")
        );
        assert_eq!(
            encontrar_destino(&destinos, "/api/v2/analytics/reportes", "POST").map(|d| d.nombre),
            Some("analytics")
        );
        assert!(encontrar_destino(&destinos, "/api/v2/no-existe", "GET").is_none());
    }
}
