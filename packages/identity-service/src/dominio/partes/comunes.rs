use super::prelude::*;

pub(super) fn validar_metadata(metadata: Option<&Value>) -> Result<(), ApiError> {
    if let Some(metadata) = metadata {
        if !metadata.is_object() {
            return Err(ApiError::dominio(
                "METADATA_INVALIDA",
                "La metadata debe ser un objeto JSON.",
            ));
        }
        if serde_json::to_vec(metadata)
            .map(|bytes| bytes.len())
            .unwrap_or(0)
            > 64 * 1024
        {
            return Err(ApiError::dominio(
                "METADATA_INVALIDA",
                "La metadata supera 64 KiB.",
            ));
        }
    }
    Ok(())
}

pub(super) fn validar_uuid(valor: &str, codigo: &str) -> Result<(), ApiError> {
    if !es_uuid(valor) {
        return Err(ApiError::dominio(
            codigo,
            "El identificador debe ser un UUID válido.",
        ));
    }
    Ok(())
}

pub(super) fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn texto_max(valor: &Value, maximo: usize) -> Result<String, ApiError> {
    let texto = valor.as_str().unwrap_or("").trim();
    if texto.chars().count() > maximo {
        return Err(ApiError::dominio(
            "VALOR_INVALIDO",
            format!("El valor no puede superar {maximo} caracteres."),
        ));
    }
    Ok(texto.to_string())
}

pub(super) fn normalizar_fechas(mut datos: Value, claves: &[&str]) -> Value {
    if let Some(objeto) = datos.as_object_mut() {
        for clave in claves {
            if let Some(valor) = objeto.get(*clave).and_then(Value::as_str) {
                if let Ok(fecha) = chrono::DateTime::parse_from_rfc3339(valor) {
                    objeto.insert(
                        (*clave).to_string(),
                        json!(iso_ms(fecha.with_timezone(&Utc))),
                    );
                }
            }
        }
    }
    datos
}

pub(super) fn es_unico(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

pub(super) fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}
