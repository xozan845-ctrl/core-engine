//! Idempotencia HTTP para mutaciones críticas.
//!
//! Si el cliente envía `Idempotency-Key`, la primera solicitud se ejecuta y su
//! respuesta 2xx se guarda. Los reintentos con la misma clave y el mismo
//! payload devuelven la respuesta original sin volver a ejecutar el handler.
//! Reutilizar la clave con otro método, ruta o cuerpo responde `422`.
//!
//! La tabla vive en `orders.idempotencia_http`; el alcance es por actor
//! (`x-user-id`) y clave.

use crate::db::Db;
use crate::errors::ApiError;
use crate::event_store::error_db;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;

pub const CLAVE_CABECERA: &str = "idempotency-key";
const CLAVE_MAXIMA: usize = 255;
const LIMITE_CUERPO: usize = 1024 * 1024;
const RETENCION_DIAS: i64 = 7;
const PROCESO_TTL_MINUTOS: i64 = 15;

pub struct Idempotencia {
    db: Arc<Db>,
}

enum Inicio {
    Nueva,
    Replay {
        status: StatusCode,
        content_type: String,
        cuerpo: Vec<u8>,
    },
}

impl Idempotencia {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    async fn iniciar(
        &self,
        actor: &str,
        clave: &str,
        metodo: &str,
        ruta: &str,
        hash: &str,
    ) -> Result<Inicio, ApiError> {
        let insertado = sqlx::query(
            "INSERT INTO orders.idempotencia_http
               (actor_id, clave, metodo, ruta, request_hash, estado)
             VALUES ($1, $2, $3, $4, $5, 'en_proceso')
             ON CONFLICT (actor_id, clave) DO NOTHING",
        )
        .bind(actor)
        .bind(clave)
        .bind(metodo)
        .bind(ruta)
        .bind(hash)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        if insertado.rows_affected() == 1 {
            return Ok(Inicio::Nueva);
        }

        let fila = sqlx::query(
            "SELECT metodo, ruta, request_hash, estado, status, content_type, respuesta
               FROM orders.idempotencia_http
              WHERE actor_id = $1 AND clave = $2",
        )
        .bind(actor)
        .bind(clave)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            // La limpieza borró la fila entre el INSERT y el SELECT.
            return Err(ApiError::interno());
        };
        let estado: String = fila.try_get("estado").map_err(error_db)?;
        if estado != "completada" {
            return Err(ApiError::nueva(
                "IDEMPOTENCIA_EN_PROCESO",
                "Ya hay una solicitud con esta Idempotency-Key en proceso.",
                StatusCode::CONFLICT,
            ));
        }

        let metodo_guardado: String = fila.try_get("metodo").map_err(error_db)?;
        let ruta_guardada: String = fila.try_get("ruta").map_err(error_db)?;
        let hash_guardado: String = fila.try_get("request_hash").map_err(error_db)?;
        if metodo_guardado != metodo || ruta_guardada != ruta || hash_guardado != hash {
            return Err(ApiError::nueva(
                "IDEMPOTENCIA_CLAVE_REUTILIZADA",
                "La Idempotency-Key ya se usó con otro método, ruta o cuerpo.",
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }

        let status: i32 = fila.try_get("status").map_err(error_db)?;
        let content_type: Option<String> = fila.try_get("content_type").map_err(error_db)?;
        let respuesta: Option<Vec<u8>> = fila.try_get("respuesta").map_err(error_db)?;
        Ok(Inicio::Replay {
            status: StatusCode::from_u16(status as u16).unwrap_or(StatusCode::OK),
            content_type: content_type
                .unwrap_or_else(|| "application/json; charset=utf-8".to_string()),
            cuerpo: respuesta.unwrap_or_default(),
        })
    }

    async fn completar(
        &self,
        actor: &str,
        clave: &str,
        status: StatusCode,
        content_type: &str,
        cuerpo: &[u8],
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE orders.idempotencia_http
                SET estado = 'completada',
                    status = $3,
                    content_type = $4,
                    respuesta = $5,
                    completado_en = NOW()
              WHERE actor_id = $1 AND clave = $2 AND estado = 'en_proceso'",
        )
        .bind(actor)
        .bind(clave)
        .bind(status.as_u16() as i32)
        .bind(content_type)
        .bind(cuerpo)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }

    async fn abortar(&self, actor: &str, clave: &str) -> Result<(), ApiError> {
        sqlx::query(
            "DELETE FROM orders.idempotencia_http
              WHERE actor_id = $1 AND clave = $2 AND estado = 'en_proceso'",
        )
        .bind(actor)
        .bind(clave)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }

    /// Borra respuestas antiguas y solicitudes que quedaron `en_proceso`
    /// (proceso caído). Se ejecuta periódicamente desde `main`.
    pub async fn limpiar(&self) -> Result<u64, ApiError> {
        let resultado = sqlx::query(
            "DELETE FROM orders.idempotencia_http
              WHERE (estado = 'completada'
                     AND completado_en < NOW() - ($1::int * INTERVAL '1 day'))
                 OR (estado = 'en_proceso'
                     AND creado_en < NOW() - ($2::int * INTERVAL '1 minute'))",
        )
        .bind(RETENCION_DIAS)
        .bind(PROCESO_TTL_MINUTOS)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(resultado.rows_affected())
    }
}

/// Middleware que aplica idempotencia solo cuando el cliente envía la cabecera.
pub async fn middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let metodo = request.method().clone();
    if !aplica(&metodo, request.uri().path()) {
        return next.run(request).await;
    }
    let Some(clave) = clave_de(request.headers()) else {
        return next.run(request).await;
    };
    if clave.len() > CLAVE_MAXIMA {
        return ApiError::validacion(json!([{
            "campo": CLAVE_CABECERA,
            "restricciones": ["maxLength"],
            "maxLength": CLAVE_MAXIMA,
        }]))
        .into_response();
    }
    let actor = actor_de(request.headers());
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let (partes, cuerpo) = request.into_parts();
    let bytes = match axum::body::to_bytes(cuerpo, LIMITE_CUERPO).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return ApiError::nueva(
                "HTTP_413",
                "El cuerpo de la peticion excede el limite permitido.",
                StatusCode::PAYLOAD_TOO_LARGE,
            )
            .into_response()
        }
    };
    let request = Request::from_parts(partes, Body::from(bytes.clone()));
    let hash = hash_solicitud(metodo.as_str(), &ruta, &bytes);
    let db = match state.db() {
        Ok(db) => Arc::clone(db),
        Err(error) => return error.into_response(),
    };
    let idempotencia = Idempotencia::nueva(db);

    match idempotencia
        .iniciar(&actor, &clave, metodo.as_str(), &ruta, &hash)
        .await
    {
        Ok(Inicio::Nueva) => {}
        Ok(Inicio::Replay {
            status,
            content_type,
            cuerpo,
        }) => {
            return Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, content_type)
                .header("idempotency-replayed", "true")
                .body(Body::from(cuerpo))
                .unwrap_or_else(|_| ApiError::interno().into_response());
        }
        Err(error) => return error.into_response(),
    }

    let response = next.run(request).await;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("application/json; charset=utf-8")
        .to_string();
    let (partes, cuerpo) = response.into_parts();
    let bytes = match cuerpo.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => {
            let _ = idempotencia.abortar(&actor, &clave).await;
            return ApiError::interno().into_response();
        }
    };

    if status.is_success() {
        if let Err(error) = idempotencia
            .completar(&actor, &clave, status, &content_type, &bytes)
            .await
        {
            // La operación ya ocurrió: no se responde error para no incitar un
            // reintento que duplicaría el efecto. La clave queda en proceso y
            // la limpieza la retira.
            crate::logs::error(
                "orders.idempotencia",
                crate::campos! {
                    "msg": "No se pudo guardar la respuesta idempotente",
                    "err": error.mensaje,
                },
            );
        }
    } else {
        let _ = idempotencia.abortar(&actor, &clave).await;
    }
    Response::from_parts(partes, Body::from(bytes))
}

fn aplica(metodo: &Method, ruta: &str) -> bool {
    matches!(
        *metodo,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) && ruta.starts_with("/api/v2/")
}

fn clave_de(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CLAVE_CABECERA)
        .and_then(|valor| valor.to_str().ok())
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
}

fn actor_de(headers: &HeaderMap) -> String {
    headers
        .get("x-user-id")
        .and_then(|valor| valor.to_str().ok())
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or("anonimo")
        .to_string()
}

fn hash_solicitud(metodo: &str, ruta: &str, cuerpo: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(metodo.as_bytes());
    hasher.update(b"\n");
    hasher.update(ruta.as_bytes());
    hasher.update(b"\n");
    hasher.update(cuerpo);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_aplicar_solo_a_mutaciones_publicas() {
        assert!(aplica(&Method::POST, "/api/v2/orders"));
        assert!(aplica(&Method::PATCH, "/api/v2/orders/1/estado"));
        assert!(!aplica(&Method::GET, "/api/v2/orders"));
        assert!(!aplica(&Method::POST, "/internal/orders/1/transicion"));
        assert!(!aplica(&Method::POST, "/health"));
    }

    #[test]
    fn debe_generar_un_hash_estable_y_sensible_al_cuerpo() {
        let uno = hash_solicitud("POST", "/api/v2/orders", br#"{"a":1}"#);
        let dos = hash_solicitud("POST", "/api/v2/orders", br#"{"a":1}"#);
        let otro = hash_solicitud("POST", "/api/v2/orders", br#"{"a":2}"#);
        assert_eq!(uno, dos);
        assert_ne!(uno, otro);
        assert_eq!(uno.len(), 64);
    }

    #[test]
    fn debe_ignorar_una_clave_vacia() {
        let mut headers = HeaderMap::new();
        headers.insert(CLAVE_CABECERA, "   ".parse().expect("header"));
        assert!(clave_de(&headers).is_none());
    }
}
