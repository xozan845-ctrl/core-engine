//! Handlers de la configuración visible del sitio.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::media::BACKGROUND_POSICIONES;
use crate::dominio::tema::PLANTILLAS;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::Json;

const ACTUALIZAR: &[ReglaCampo] = &[
    campo("background_objeto_id", false, &[Regla::Uuid]),
    campo("background_poster_objeto_id", false, &[Regla::Uuid]),
    campo(
        "background_alt",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "background_overlay",
        false,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
    campo(
        "background_position",
        false,
        &[Regla::En(&BACKGROUND_POSICIONES)],
    ),
    campo("background_activo", false, &[Regla::Booleano]),
    campo("panel_ai_background_objeto_id", false, &[Regla::Uuid]),
    campo(
        "panel_ai_background_poster_objeto_id",
        false,
        &[Regla::Uuid],
    ),
    campo(
        "panel_ai_background_alt",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "panel_ai_background_overlay",
        false,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
    campo(
        "panel_ai_background_position",
        false,
        &[Regla::En(&BACKGROUND_POSICIONES)],
    ),
    campo("panel_ai_background_activo", false, &[Regla::Booleano]),
    campo("plantilla", false, &[Regla::En(&PLANTILLAS)]),
    campo("tema", false, &[Regla::Objeto]),
];

/// GET /api/v2/configuracion/sitio
pub async fn obtener(State(state): State<AppState>) -> Result<Response, ApiError> {
    let configuracion = state.servicios()?.configuracion.obtener().await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(configuracion),
    )
        .into_response())
}

/// PATCH /api/v2/configuracion/sitio
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ACTUALIZAR, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let configuracion = state
        .servicios()?
        .configuracion
        .actualizar(&dto, &actor.user_id)
        .await?;
    Ok(Json(configuracion).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_fondos_y_tema_del_dto() {
        assert!(validar(ACTUALIZAR, &json!({})).is_ok());
        assert!(validar(
            ACTUALIZAR,
            &json!({
                "background_objeto_id": "11111111-1111-4111-8111-111111111111",
                "background_overlay": 40,
                "background_position": "top",
                "plantilla": "minimal",
                "tema": {"color_primario": "#FFF"}
            })
        )
        .is_ok());
        assert!(validar(ACTUALIZAR, &json!({"background_objeto_id": "no-uuid"})).is_err());
        assert!(validar(ACTUALIZAR, &json!({"background_overlay": 101})).is_err());
        assert!(validar(ACTUALIZAR, &json!({"plantilla": "otra"})).is_err());
        assert!(validar(ACTUALIZAR, &json!({"tema": []})).is_err());
    }
}
