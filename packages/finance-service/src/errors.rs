//! Errores de dominio y su traducción HTTP.
//!
//! Réplica de `errors.ts` + `domain-error.filter.ts`: todo error sale como
//! `{codigo, mensaje, detalles?}` y sin `meta` (R-C-8).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub codigo: String,
    pub mensaje: String,
    pub detalles: Option<Box<Value>>,
    pub status: StatusCode,
}

impl ApiError {
    pub fn nueva(
        codigo: impl Into<String>,
        mensaje: impl Into<String>,
        status: StatusCode,
    ) -> Self {
        Self {
            codigo: codigo.into(),
            mensaje: mensaje.into(),
            detalles: None,
            status,
        }
    }

    pub fn con_detalles(mut self, detalles: Value) -> Self {
        self.detalles = Some(Box::new(detalles));
        self
    }

    pub fn dominio(codigo: impl Into<String>, mensaje: impl Into<String>) -> Self {
        Self::nueva(codigo, mensaje, StatusCode::BAD_REQUEST)
    }

    pub fn no_encontrado(entidad: &str, id: &str) -> Self {
        Self::nueva(
            "NO_ENCONTRADO",
            format!("{entidad} {id} no existe."),
            StatusCode::NOT_FOUND,
        )
    }

    pub fn conflicto(mensaje: impl Into<String>) -> Self {
        Self::nueva("CONFLICTO", mensaje, StatusCode::CONFLICT)
    }

    pub fn no_autorizado(mensaje: impl Into<String>) -> Self {
        Self::nueva("NO_AUTORIZADO", mensaje, StatusCode::UNAUTHORIZED)
    }

    pub fn acceso_denegado(mensaje: impl Into<String>) -> Self {
        Self::nueva("ACCESO_DENEGADO", mensaje, StatusCode::FORBIDDEN)
    }

    pub fn servicio_no_disponible(servicio: &str) -> Self {
        Self::nueva(
            "SERVICIO_NO_DISPONIBLE",
            format!("El servicio \"{servicio}\" no está disponible."),
            StatusCode::SERVICE_UNAVAILABLE,
        )
    }

    /// Capacidad de tenant faltante (403 con detalles, como `CapabilityDisabledError`).
    pub fn capacidad_no_activa(capacidades: &[&str], empresa_id: &str) -> Self {
        Self::nueva(
            "CAPACIDAD_NO_ACTIVA",
            format!(
                "La empresa no tiene activas las capacidades requeridas: {}.",
                capacidades.join(", ")
            ),
            StatusCode::FORBIDDEN,
        )
        .con_detalles(json!({
            "empresa_id": empresa_id,
            "capacidades": capacidades,
        }))
    }

    pub fn validacion(detalles: Value) -> Self {
        Self::nueva(
            "VALIDACION",
            "La solicitud no cumple las reglas de validacion.",
            StatusCode::BAD_REQUEST,
        )
        .con_detalles(detalles)
    }

    pub fn interno() -> Self {
        Self::nueva(
            "ERROR_INTERNO",
            "Error interno del servidor.",
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    }

    pub fn cuerpo(&self) -> Value {
        match &self.detalles {
            Some(detalles) => json!({
                "codigo": &self.codigo,
                "mensaje": &self.mensaje,
                "detalles": detalles.as_ref(),
            }),
            None => json!({
                "codigo": &self.codigo,
                "mensaje": &self.mensaje,
            }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.cuerpo())).into_response()
    }
}

/// Código estructurado por status HTTP para errores no clasificados.
pub fn codigo_de_status(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "SOLICITUD_INVALIDA",
        401 => "NO_AUTORIZADO",
        403 => "ACCESO_DENEGADO",
        404 => "NO_ENCONTRADO",
        409 => "CONFLICTO",
        429 => "DEMASIADAS_PETICIONES",
        _ => "ERROR_HTTP",
    }
}

/// Traduce un fallo de PostgreSQL al error HTTP genérico (equivalente al filtro
/// global de Nest: 500 con código `ERROR_INTERNO`).
pub fn error_db(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "db",
        crate::campos! { "msg": "Fallo de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

/// `true` si el error es una violación de unicidad (SQLSTATE 23505), que el
/// contrato traduce a 409.
pub fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|db| db.code())
        .map(|codigo| codigo == "23505")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_serializar_el_cuerpo_sin_detalles_cuando_no_los_hay() {
        let error = ApiError::no_encontrado("Orden", "abc");
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert_eq!(
            error.cuerpo(),
            json!({
                "codigo": "NO_ENCONTRADO",
                "mensaje": "Orden abc no existe.",
            })
        );
    }

    #[test]
    fn debe_conservar_el_codigo_de_dominio_y_los_detalles() {
        let error = ApiError::validacion(json!([{"campo": "estado", "restricciones": ["isIn"]}]));
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        let cuerpo = error.cuerpo();
        assert_eq!(cuerpo["codigo"], "VALIDACION");
        assert_eq!(cuerpo["detalles"][0]["campo"], "estado");
    }

    #[test]
    fn debe_mapear_la_capacidad_faltante_a_403_con_detalles() {
        let error = ApiError::capacidad_no_activa(&["logistica_rutas"], "empresa-1");
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.codigo, "CAPACIDAD_NO_ACTIVA");
        assert_eq!(
            error.cuerpo()["detalles"]["capacidades"][0],
            "logistica_rutas"
        );
    }

    #[test]
    fn debe_mapear_los_status_conocidos_a_su_codigo() {
        assert_eq!(
            codigo_de_status(StatusCode::BAD_REQUEST),
            "SOLICITUD_INVALIDA"
        );
        assert_eq!(
            codigo_de_status(StatusCode::TOO_MANY_REQUESTS),
            "DEMASIADAS_PETICIONES"
        );
        assert_eq!(codigo_de_status(StatusCode::IM_A_TEAPOT), "ERROR_HTTP");
    }
}
