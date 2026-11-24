use super::prelude::*;

pub(super) fn codigo_strict(valor: &str, error: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    if !es_codigo(&codigo) {
        return Err(ApiError::dominio(
            error,
            "El código contiene caracteres no permitidos.",
        ));
    }
    Ok(codigo)
}

pub(super) fn es_codigo(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub(super) fn es_codigo_pack(valor: &str) -> bool {
    let Some(resto) = valor.strip_prefix("pack_") else {
        return false;
    };
    let mut chars = resto.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&resto.len())
        && resto
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub(super) fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if matches!(&error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")) {
        return ApiError::conflicto("Ya existe un registro con ese código.");
    }
    interno(error)
}

pub(super) fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}
