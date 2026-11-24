use super::prelude::*;

pub(super) fn validar_codigo(valor: &str, campo: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            format!("{}_INVALIDO", campo.to_uppercase()),
            format!("{campo} no es válido."),
        ));
    }
    Ok(codigo)
}

pub(super) fn validar_correo_invitacion(correo: &str) -> Result<String, ApiError> {
    let correo = correo.trim().to_lowercase();
    if !correo_valido(&correo)
        || !correo
            .split_once('@')
            .is_some_and(|(_, dominio)| dominio.contains('.'))
    {
        return Err(ApiError::dominio(
            "CORREO_INVALIDO",
            "El correo no tiene un formato válido.",
        ));
    }
    Ok(correo)
}

pub(super) fn validar_objeto(valor: Option<&Value>, campo: &str) -> Result<(), ApiError> {
    if let Some(valor) = valor {
        if !valor.is_object() {
            return Err(ApiError::dominio(
                format!("{}_INVALIDO", campo.to_uppercase()),
                format!("{campo} debe ser un objeto JSON."),
            ));
        }
    }
    Ok(())
}

pub(super) fn validar_alcance_negocio(
    negocio: Option<&str>,
    recurso: Option<&str>,
    nombre: &str,
) -> Result<(), ApiError> {
    if let (Some(negocio), Some(recurso)) = (negocio, recurso) {
        if negocio != recurso {
            return Err(ApiError::dominio(
                "NEGOCIO_INCOHERENTE",
                format!("El {nombre} pertenece a otro negocio."),
            ));
        }
    }
    Ok(())
}

pub(super) fn acciones_de(permisos: &Value) -> Vec<String> {
    permisos
        .get("acciones")
        .and_then(Value::as_array)
        .map(|acciones| {
            acciones
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn validar_permisos(permisos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = permisos.as_object() else {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos debe ser un objeto JSON.",
        ));
    };
    let Some(acciones) = objeto.get("acciones").and_then(Value::as_array) else {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos.acciones debe contener entre 1 y 200 acciones.",
        ));
    };
    if acciones.is_empty() || acciones.len() > 200 {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos.acciones debe contener entre 1 y 200 acciones.",
        ));
    }
    let mut unicas: Vec<String> = Vec::new();
    for accion in acciones {
        let Some(accion) = accion.as_str() else {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "Cada acción debe usar el formato recurso.accion.",
            ));
        };
        if !accion_valida(accion) {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "Cada acción debe usar el formato recurso.accion.",
            ));
        }
        if !unicas.iter().any(|existente| existente == accion) {
            unicas.push(accion.to_string());
        }
    }
    let mut salida = Map::new();
    salida.insert(
        "version".into(),
        json!(objeto.get("version").and_then(Value::as_i64).unwrap_or(1)),
    );
    salida.insert("acciones".into(), json!(unicas));
    if let Some(recursos) = objeto.get("recursos") {
        let Some(recursos) = recursos.as_array() else {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "permisos.recursos debe ser una lista de códigos válidos.",
            ));
        };
        let mut limpios: Vec<String> = Vec::new();
        for recurso in recursos {
            let Some(recurso) = recurso.as_str() else {
                return Err(ApiError::dominio(
                    "PERMISOS_INVALIDOS",
                    "permisos.recursos debe ser una lista de códigos válidos.",
                ));
            };
            if validar_codigo(recurso, "recurso").is_err() {
                return Err(ApiError::dominio(
                    "PERMISOS_INVALIDOS",
                    "permisos.recursos debe ser una lista de códigos válidos.",
                ));
            }
            if !limpios.iter().any(|existente| existente == recurso) {
                limpios.push(recurso.to_string());
            }
        }
        salida.insert("recursos".into(), json!(limpios));
    }
    if let Some(condiciones) = objeto.get("condiciones") {
        if !condiciones.is_object() {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "permisos.condiciones debe ser un objeto JSON.",
            ));
        }
        salida.insert("condiciones".into(), condiciones.clone());
    }
    Ok(Value::Object(salida))
}

pub(super) fn accion_valida(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=80).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
}

pub(super) fn escapar_html(valor: &str) -> String {
    valor
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn token_aleatorio() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub(super) fn validar_vigente(datos: &Value) -> Result<(), ApiError> {
    let estado = datos.get("estado").and_then(Value::as_str).unwrap_or("");
    let expira = datos
        .get("expira_en")
        .and_then(Value::as_str)
        .and_then(|valor| chrono::DateTime::parse_from_rfc3339(valor).ok());
    if estado != "pendiente" || expira.is_none_or(|fecha| fecha.with_timezone(&Utc) <= Utc::now()) {
        return Err(invitacion_no_valida());
    }
    Ok(())
}

pub(super) fn invitacion_no_valida() -> ApiError {
    ApiError::nueva(
        "INVITACION_NO_VALIDA",
        "La invitación no existe, expiró o ya fue utilizada.",
        StatusCode::GONE,
    )
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

pub(super) fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn conflicto_codigo(error: sqlx::Error) -> ApiError {
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
