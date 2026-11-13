//! Rutas HTTP de logística: guías de despacho, asignación de flota y
//! transiciones del ciclo de vida de la orden.

use crate::auth::{UsuarioContexto, ROLES_ORDEN};
use crate::envios::{envio_json, validar_estado_envio, validar_estado_orden};
use crate::errors::ApiError;
use crate::http::Cuerpo;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

const CAP_FULFILLMENT: &[&str] = &["fulfillment_delivery"];
const CAP_RUTAS: &[&str] = &["logistica_rutas"];
const ROLES_FLOTA: [&str; 7] = [
    "super_admin",
    "empresario",
    "admin",
    "logistica",
    "coordinador",
    "supervisor",
    "operativo",
];
const ROLES_ADMIN: [&str; 2] = ["super_admin", "admin"];
const ROLES_EMPRESA: [&str; 1] = ["empresario"];

fn error_campo(campo: &str, restricciones: &[&str]) -> Value {
    json!({ "campo": campo, "restricciones": restricciones })
}

fn validar_asignar(cuerpo: &Value) -> Result<String, ApiError> {
    match cuerpo.get("proveedor_tipo").and_then(Value::as_str) {
        Some(tipo @ ("plataforma" | "empresa")) => Ok(tipo.to_string()),
        _ => Err(ApiError::validacion(Value::Array(vec![error_campo(
            "proveedor_tipo",
            &["isIn"],
        )]))),
    }
}

fn validar_avanzar_estado(cuerpo: &Value) -> Result<(String, Option<String>), ApiError> {
    let mut errores = Vec::new();
    let estado = cuerpo.get("estado").and_then(Value::as_str);
    if estado.is_none() {
        errores.push(error_campo("estado", &["isIn"]));
    }
    let motivo = cuerpo.get("motivo");
    if motivo.is_some_and(|valor| !valor.is_null() && !valor.is_string()) {
        errores.push(error_campo("motivo", &["isString"]));
    }
    if !errores.is_empty() {
        return Err(ApiError::validacion(Value::Array(errores)));
    }
    let estado = estado.expect("estado validado").to_string();
    if ![
        "en_preparacion",
        "enviada",
        "entregada",
        "cancelada",
        "devuelta",
    ]
    .contains(&estado.as_str())
    {
        return Err(ApiError::validacion(Value::Array(vec![error_campo(
            "estado",
            &["isIn"],
        )])));
    }
    let motivo = motivo
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|valor| !valor.is_empty());
    Ok((estado, motivo))
}

fn validar_avanzar_envio(cuerpo: &Value) -> Result<(String, Option<String>), ApiError> {
    let mut errores = Vec::new();
    let estado = cuerpo.get("estado").and_then(Value::as_str);
    if !matches!(estado, Some("enviada" | "entregada" | "cancelada")) {
        errores.push(error_campo("estado", &["isIn"]));
    }
    let motivo = cuerpo.get("motivo");
    if motivo.is_some_and(|valor| !valor.is_null() && !valor.is_string()) {
        errores.push(error_campo("motivo", &["isString"]));
    }
    if !errores.is_empty() {
        return Err(ApiError::validacion(Value::Array(errores)));
    }
    let estado = estado.expect("estado validado").to_string();
    validar_estado_envio(&estado)?;
    let motivo = motivo
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|valor| !valor.is_empty());
    Ok((estado, motivo))
}

/// `PATCH /api/v2/orders/{id}/estado` (Tabla 21: JWT admin/logística).
pub async fn avanzar_estado_orden(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    usuario.exigir_roles(&ROLES_ORDEN, &["logistica.ordenes.actualizar"])?;
    let (estado, motivo) = validar_avanzar_estado(&cuerpo)?;
    // Chequeo del controlador original: el pipe ya validó el enum, así que en
    // HTTP es inalcanzable; se conserva por paridad estructural.
    validar_estado_orden(&estado)?;

    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        let tenant = usuario.tenant()?.to_string();
        let orden = state
            .envios
            .avanzar_estado_orden(
                &id,
                &estado,
                motivo.as_deref(),
                Some(&tenant),
                Some(&usuario.user_id),
            )
            .await?;
        return Ok(Json(serde_json::to_value(orden).unwrap_or(Value::Null)));
    }

    let orden = state
        .envios
        .avanzar_estado_orden(
            &id,
            &estado,
            motivo.as_deref(),
            None,
            Some(&usuario.user_id),
        )
        .await?;
    Ok(Json(serde_json::to_value(orden).unwrap_or(Value::Null)))
}

/// `GET /api/v2/admin/envios`: guías de despacho (JWT admin).
pub async fn listar_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN, &[])?;
    state.exigir_capacidades(&usuario, CAP_FULFILLMENT).await?;
    let filas = state.envios.listar().await?;
    Ok(Json(serde_json::to_value(filas).unwrap_or(Value::Null)))
}

/// `GET /api/v2/empresarios/me/envios`: envíos de la empresa activa.
pub async fn listar_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    usuario.exigir_roles(&ROLES_EMPRESA, &["logistica.envios.leer"])?;
    state.exigir_capacidades(&usuario, CAP_FULFILLMENT).await?;
    let tenant = usuario.tenant()?.to_string();
    let filas = state.envios.de_empresa(&tenant).await?;
    Ok(Json(serde_json::to_value(filas).unwrap_or(Value::Null)))
}

/// `POST /api/v2/empresarios/me/envios/{id}/asignacion`: elige la flota de un
/// envío híbrido.
pub async fn asignar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    usuario.exigir_roles(&ROLES_EMPRESA, &["logistica.envios.asignar"])?;
    state.exigir_capacidades(&usuario, CAP_FULFILLMENT).await?;
    let tenant = usuario.tenant()?.to_string();
    let tipo = validar_asignar(&cuerpo)?;
    let envio = state
        .envios
        .asignar_proveedor(&id, &tenant, &tipo, Some(&usuario.user_id))
        .await?;
    Ok(Json(envio_json(&envio)))
}

/// `PATCH /api/v2/envios/{id}/estado`: avanza un envío de la flota activa.
pub async fn avanzar_envio(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    usuario.exigir_roles(&ROLES_FLOTA, &["logistica.envios.actualizar"])?;
    state.exigir_capacidades(&usuario, CAP_RUTAS).await?;
    let tenant = usuario.tenant()?.to_string();
    let (estado, motivo) = validar_avanzar_envio(&cuerpo)?;
    let envio = state
        .envios
        .avanzar_estado_envio(
            &id,
            &tenant,
            &estado,
            motivo.as_deref(),
            Some(&usuario.user_id),
        )
        .await?;
    Ok(Json(envio_json(&envio)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_el_estado_objetivo_de_la_orden() {
        let (estado, motivo) =
            validar_avanzar_estado(&json!({"estado": "enviada", "motivo": "despacho"}))
                .expect("valido");
        assert_eq!(estado, "enviada");
        assert_eq!(motivo.as_deref(), Some("despacho"));

        let error = validar_avanzar_estado(&json!({"estado": "creada"})).expect_err("invalido");
        assert_eq!(error.codigo, "VALIDACION");
        assert_eq!(error.cuerpo()["detalles"][0]["restricciones"][0], "isIn");
    }

    #[test]
    fn debe_validar_motivo_y_estado_del_envio() {
        let error = validar_avanzar_envio(&json!({"estado": "entregada", "motivo": 3}))
            .expect_err("motivo");
        assert_eq!(error.cuerpo()["detalles"][0]["campo"], "motivo");
        assert!(validar_avanzar_envio(&json!({"estado": "entregada"})).is_ok());
    }

    #[test]
    fn debe_validar_el_proveedor_asignado() {
        assert_eq!(
            validar_asignar(&json!({"proveedor_tipo": "empresa"})).expect("valido"),
            "empresa"
        );
        assert_eq!(
            validar_asignar(&json!({"proveedor_tipo": "otra"}))
                .expect_err("invalido")
                .codigo,
            "VALIDACION"
        );
    }

    #[test]
    fn debe_exponer_los_roles_de_cada_ruta() {
        assert!(ROLES_FLOTA.contains(&"operativo"));
        assert_eq!(ROLES_ADMIN, ["super_admin", "admin"]);
        assert_eq!(ROLES_EMPRESA, ["empresario"]);
        assert!(crate::auth::ROLES_LOGISTICA.contains(&"supervisor"));
    }
}
