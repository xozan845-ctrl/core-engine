//! Proxy HTTP hacia los microservicios y tabla de destinos (Tabla 21).

use crate::auth::aplicar_auth;
use crate::config::Config;
use crate::errors::ApiError;
use crate::policy::encontrar_politica;
use crate::rate_limit::es_media_publica;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use regex::Regex;
use std::time::Duration;
use uuid::Uuid;

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
            nombre: "identity",
            url: config.identity_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/auth/", &["*"]),
                patron(r"^/api/v2/usuarios(/|$)", &["*"]),
                patron(r"^/api/v2/empresas(/|$)", &["*"]),
                patron(r"^/api/v2/incorporaciones(/|$)", &["*"]),
                patron(r"^/api/v2/packs(/|$)", &["GET", "POST"]),
                patron(r"^/api/v2/configuracion(/|$)", &["*"]),
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
            nombre: "catalog",
            url: config.catalog_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/catalog/", &["*"]),
                patron(r"^/api/v2/admin/inventario(/|$)", &["*"]),
            ],
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
            ],
        },
        Destino {
            nombre: "orders",
            url: config.orders_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/orders(/|$)", &["*"]),
                patron(r"^/api/v2/carrito(/|$)", &["*"]),
            ],
        },
        Destino {
            nombre: "commissions",
            url: config.commissions_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/vendedores/me/(ventas|liquidaciones)", &["*"]),
                patron(r"^/api/v2/empresarios/me/(liquidaciones|ventas)", &["*"]),
                patron(r"^/api/v2/admin/reportes", &["*"]),
                patron(r"^/api/v2/admin/liquidaciones", &["*"]),
            ],
        },
        Destino {
            nombre: "finance",
            url: config.finance_url.clone(),
            patrones: vec![
                patron(r"^/api/v2/finanzas(/|$)", &["*"]),
                patron(r"^/api/v2/comercio(/|$)", &["*"]),
                patron(r"^/api/v2/vendedores/me/fiscal", &["*"]),
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
    let cuerpo = if matches!(metodo, Method::GET | Method::HEAD | Method::OPTIONS) {
        Vec::new()
    } else {
        match request.into_body().collect().await {
            Ok(bytes) => bytes.to_bytes().to_vec(),
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

    let mut headers = HeaderMap::new();
    if let Ok(valor) = HeaderValue::from_str(&request_id) {
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
            HeaderValue::from_str(&valor),
        ) {
            headers.insert(nombre, valor);
        }
    }
    if es_media {
        if let Some(rango) = rango.as_deref() {
            if let Ok(valor) = HeaderValue::from_str(rango) {
                headers.insert(header::RANGE, valor);
            }
        }
    }

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
            encontrar_destino(&destinos, "/api/v2/finanzas/resumen", "GET").map(|d| d.nombre),
            Some("finance")
        );
        assert!(encontrar_destino(&destinos, "/api/v2/no-existe", "GET").is_none());
    }
}
