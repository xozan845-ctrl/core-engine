use super::*;

pub(crate) const ESTADOS_ACTIVOS: [&str; 3] = ["reservada", "confirmada", "en_atencion"];

pub(crate) async fn validar_disponibilidad_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    recurso: Uuid,
    inicia: &str,
    termina: &str,
) -> Result<(), ApiError> {
    let disponible = sqlx::query(
        "SELECT 1 FROM scheduling.disponibilidad d
         WHERE d.recurso_id = $1
           AND d.dia_semana = EXTRACT(DOW FROM ($2::timestamptz AT TIME ZONE d.zona_horaria))::int
           AND d.inicia_hora <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::time)
           AND d.termina_hora >= (($3::timestamptz AT TIME ZONE d.zona_horaria)::time)
           AND d.vigente_desde <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date)
           AND (d.vigente_hasta IS NULL OR d.vigente_hasta >= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date))
         LIMIT 1",
    )
    .bind(recurso)
    .bind(inicia)
    .bind(termina)
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if disponible.is_none() {
        return Err(ApiError::recurso_no_disponible(
            "El recurso no tiene disponibilidad en ese horario.",
        ));
    }
    Ok(())
}

pub(crate) async fn crear_recordatorios(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    cita: Uuid,
    inicia_en: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO scheduling.recordatorios (id, empresa_id, cita_id, tipo, programado_en)
         SELECT gen_random_uuid(), $1, $2, tipo, ($3::timestamptz - intervalo)
         FROM (VALUES ('24h'::text, INTERVAL '24 hours'), ('2h'::text, INTERVAL '2 hours')) AS x(tipo, intervalo)
         WHERE ($3::timestamptz - intervalo) > NOW()
         ON CONFLICT (cita_id, tipo) DO NOTHING",
    )
    .bind(empresa)
    .bind(cita)
    .bind(inicia_en)
    .execute(&mut **tx)
    .await
    .map_err(error_db)?;
    Ok(())
}

pub(crate) fn fila_cita(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "parte_id": uuid_opcional(fila, "parte_id").map_err(error_db)?,
        "order_id": uuid_opcional(fila, "order_id").map_err(error_db)?,
        "item_id": uuid_opcional(fila, "item_id").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "inicia_en": timestamp(fila, "inicia_en").map_err(error_db)?,
        "termina_en": timestamp(fila, "termina_en").map_err(error_db)?,
        "notas": fila.try_get::<String, _>("notas").map_err(error_db)?,
        "metadata": jsonb(fila, "metadata").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
        "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
    }))
}

pub(crate) fn fila_recordatorio(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "cita_id": uuid_texto(fila, "cita_id").map_err(error_db)?,
        "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
        "programado_en": timestamp(fila, "programado_en").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "enviado_en": timestamp_opcional(fila, "enviado_en").map_err(error_db)?,
    }))
}

pub(crate) fn configuracion_requerida() -> ApiError {
    ApiError::dominio(
        "CITA_CONFIGURACION_REQUERIDA",
        "Una reserva requiere un servicio onsite con recurso_id, inicia_en y termina_en.",
    )
}

pub(crate) fn texto_config(config: &Value, clave: &str) -> Option<String> {
    config
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
}

/// Puerto de agenda para que los handlers no dependan de PostgreSQL en tests.
pub(crate) async fn recurso_de_empresa(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    recurso: Uuid,
) -> Result<(), ApiError> {
    let fila = sqlx::query("SELECT id FROM scheduling.recursos WHERE id = $1 AND empresa_id = $2")
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
    if fila.is_none() {
        return Err(ApiError::no_encontrado("Recurso", &recurso.to_string()));
    }
    Ok(())
}

pub(crate) async fn recurso_de_empresa_bloqueado(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    recurso: Uuid,
) -> Result<(), ApiError> {
    let fila = sqlx::query(
        "SELECT id FROM scheduling.recursos WHERE id = $1 AND empresa_id = $2 FOR UPDATE",
    )
    .bind(recurso)
    .bind(empresa)
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if fila.is_none() {
        return Err(ApiError::no_encontrado("Recurso", &recurso.to_string()));
    }
    Ok(())
}

pub(crate) fn fila_recurso(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
        "nombre": fila.try_get::<String, _>("nombre").map_err(error_db)?,
        "capacidad": fila.try_get::<i32, _>("capacidad").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "atributos": jsonb(fila, "atributos").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
        "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
    }))
}

pub(crate) fn fila_disponibilidad(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "dia_semana": fila.try_get::<i16, _>("dia_semana").map_err(error_db)?,
        "inicia_hora": hora(fila, "inicia_hora").map_err(error_db)?,
        "termina_hora": hora(fila, "termina_hora").map_err(error_db)?,
        "zona_horaria": fila.try_get::<String, _>("zona_horaria").map_err(error_db)?,
        "vigente_desde": fecha(fila, "vigente_desde").map_err(error_db)?,
        "vigente_hasta": fecha_opcional(fila, "vigente_hasta").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
    }))
}

pub(crate) fn fila_bloqueo(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "inicia_en": timestamp(fila, "inicia_en").map_err(error_db)?,
        "termina_en": timestamp(fila, "termina_en").map_err(error_db)?,
        "motivo": fila.try_get::<String, _>("motivo").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
    }))
}

// ── Errores y conversiones ─────────────────────────────────────────────────

pub fn error_db(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "scheduling",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::interno())
}

// ── Validación (equivalente a DtoValidationPipe + class-validator) ─────────

pub(crate) fn error_campo(campo: &str, restricciones: &[&str]) -> Value {
    json!({ "campo": campo, "restricciones": restricciones })
}

pub(crate) fn error_validacion(errores: Vec<Value>) -> ApiError {
    ApiError::validacion(Value::Array(errores))
}

pub fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (indice, byte) in bytes.iter().enumerate() {
        match indice {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            14 => {
                if !(b'1'..=b'5').contains(byte) {
                    return false;
                }
            }
            19 => {
                if !matches!(byte, b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// `IsDateString` (validator.js isISO8601): fecha, fecha-hora o RFC 3339.
pub fn es_iso8601(valor: &str) -> bool {
    DateTime::parse_from_rfc3339(valor).is_ok()
        || NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S").is_ok()
}

pub(crate) fn instante_ms(valor: &str) -> Option<i64> {
    if let Ok(fecha) = DateTime::parse_from_rfc3339(valor) {
        return Some(fecha.timestamp_millis());
    }
    if let Ok(fecha) = NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(Utc.from_utc_datetime(&fecha).timestamp_millis());
    }
    if let Ok(fecha) = NaiveDate::parse_from_str(valor, "%Y-%m-%d") {
        return fecha
            .and_hms_opt(0, 0, 0)
            .map(|fecha| Utc.from_utc_datetime(&fecha).timestamp_millis());
    }
    None
}

pub fn validar_periodo(inicia: &str, termina: &str) -> Result<(), ApiError> {
    let Some(inicio_ms) = instante_ms(inicia) else {
        return Err(periodo_invalido());
    };
    let Some(fin_ms) = instante_ms(termina) else {
        return Err(periodo_invalido());
    };
    if inicio_ms >= fin_ms {
        return Err(periodo_invalido());
    }
    Ok(())
}

pub(crate) fn periodo_invalido() -> ApiError {
    ApiError::dominio(
        "PERIODO_INVALIDO",
        "La hora de inicio debe ser anterior a la hora de finalizacion.",
    )
}

pub(crate) fn objeto(valor: &Value) -> Map<String, Value> {
    valor.as_object().cloned().unwrap_or_default()
}

pub(crate) fn opcional_iso(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) if es_iso8601(valor) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isDateString"]));
            None
        }
    }
}

pub(crate) fn opcional_string(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isString"]));
            None
        }
    }
}

pub fn validar_crear_recurso(valor: &Value) -> Result<CrearRecurso, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let tipo = match objeto.get("tipo").and_then(Value::as_str) {
        Some(valor) if TIPOS_RECURSO.contains(&valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("tipo", &["isIn"]));
            String::new()
        }
    };

    let nombre = match objeto.get("nombre").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("nombre", &["isString"]));
            String::new()
        }
    };

    let atributos = match objeto.get("atributos") {
        None | Some(Value::Null) => json!({}),
        Some(valor @ Value::Object(_)) => valor.clone(),
        Some(_) => {
            errores.push(error_campo("atributos", &["isObject"]));
            json!({})
        }
    };

    let capacidad = match objeto.get("capacidad") {
        None | Some(Value::Null) => 1,
        Some(valor) => match valor.as_i64() {
            Some(valor) if (1..=100_000).contains(&valor) => valor as i32,
            _ => {
                errores.push(error_campo("capacidad", &["isInt", "min"]));
                1
            }
        },
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearRecurso {
        tipo,
        nombre,
        capacidad,
        atributos,
    })
}

pub fn validar_crear_disponibilidad(valor: &Value) -> Result<CrearDisponibilidad, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };

    let dia_semana = match objeto.get("dia_semana").and_then(Value::as_i64) {
        Some(valor) => {
            if valor < 0 {
                errores.push(error_campo("dia_semana", &["min"]));
            } else if valor > 6 {
                errores.push(error_campo("dia_semana", &["max"]));
            }
            valor
        }
        None => {
            errores.push(error_campo("dia_semana", &["isInt"]));
            0
        }
    };

    let inicia_hora = match objeto.get("inicia_hora").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("inicia_hora", &["isString"]));
            String::new()
        }
    };
    let termina_hora = match objeto.get("termina_hora").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("termina_hora", &["isString"]));
            String::new()
        }
    };

    let zona_horaria = opcional_string(&objeto, "zona_horaria", &mut errores);
    let vigente_desde = opcional_iso(&objeto, "vigente_desde", &mut errores);
    let vigente_hasta = opcional_iso(&objeto, "vigente_hasta", &mut errores);

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearDisponibilidad {
        recurso_id,
        dia_semana,
        inicia_hora,
        termina_hora,
        zona_horaria,
        vigente_desde,
        vigente_hasta,
    })
}

pub fn validar_crear_bloqueo(valor: &Value) -> Result<CrearBloqueo, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let inicia_en = match objeto.get("inicia_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("inicia_en", &["isDateString"]));
            String::new()
        }
    };
    let termina_en = match objeto.get("termina_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("termina_en", &["isDateString"]));
            String::new()
        }
    };
    let motivo = opcional_string(&objeto, "motivo", &mut errores);

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearBloqueo {
        recurso_id,
        inicia_en,
        termina_en,
        motivo,
    })
}

pub(crate) fn opcional_uuid(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) if es_uuid(valor) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isUuid"]));
            None
        }
    }
}

pub fn validar_crear_cita(valor: &Value) -> Result<CrearCita, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let parte_id = match objeto.get("parte_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("parte_id", &["isUuid"]));
            String::new()
        }
    };
    let order_id = opcional_uuid(&objeto, "order_id", &mut errores);
    let item_id = opcional_uuid(&objeto, "item_id", &mut errores);

    let inicia_en = match objeto.get("inicia_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("inicia_en", &["isDateString"]));
            String::new()
        }
    };
    let termina_en = match objeto.get("termina_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("termina_en", &["isDateString"]));
            String::new()
        }
    };
    let notas = opcional_string(&objeto, "notas", &mut errores);
    let metadata = match objeto.get("metadata") {
        None | Some(Value::Null) => json!({}),
        Some(valor @ Value::Object(_)) => valor.clone(),
        Some(_) => {
            errores.push(error_campo("metadata", &["isObject"]));
            json!({})
        }
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearCita {
        recurso_id,
        parte_id,
        order_id,
        item_id,
        inicia_en,
        termina_en,
        notas,
        metadata,
    })
}

pub fn validar_verificar_disponibilidad(
    valor: &Value,
) -> Result<VerificarDisponibilidad, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let empresa_id = match objeto.get("empresa_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("empresa_id", &["isUuid"]));
            String::new()
        }
    };
    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let inicia_en = match objeto.get("inicia_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("inicia_en", &["isDateString"]));
            String::new()
        }
    };
    let termina_en = match objeto.get("termina_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("termina_en", &["isDateString"]));
            String::new()
        }
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(VerificarDisponibilidad {
        empresa_id,
        recurso_id,
        inicia_en,
        termina_en,
    })
}
