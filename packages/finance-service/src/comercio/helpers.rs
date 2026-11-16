use super::*;

// ── utilidades libres ───────────────────────────────────────────────────────

pub fn a_registro(fila: &sqlx::postgres::PgRow) -> Registro {
    let datos: Value = jsonb(fila, "datos").unwrap_or_else(|_| json!({}));
    let mut objeto = datos.as_object().cloned().unwrap_or_default();
    objeto.insert(
        "id".into(),
        json!(uuid_texto(fila, "id").unwrap_or_default()),
    );
    objeto.insert(
        "createdAt".into(),
        json!(timestamp(fila, "creado_en").unwrap_or_default()),
    );
    objeto.insert(
        "updatedAt".into(),
        json!(timestamp(fila, "actualizado_en").unwrap_or_default()),
    );
    objeto
}

pub async fn insertar_en(
    tx: &mut Transaction<'_, Postgres>,
    recurso: &str,
    datos: Value,
    ambito: &Ambito,
) -> Result<sqlx::postgres::PgRow, ApiError> {
    sqlx::query(
        "INSERT INTO finance.workspace_registros (tenant_id, propietario_id, recurso, datos)
         VALUES ($1::uuid, $2::uuid, $3, $4::jsonb)
         RETURNING id, datos, creado_en, actualizado_en",
    )
    .bind(ambito.tenant_id.as_deref())
    .bind(ambito.propietario_id.as_deref())
    .bind(recurso)
    .bind(sqlx::types::Json(&datos))
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?
    .ok_or_else(|| ApiError::dominio("REGISTRO_NO_CREADO", "No se pudo crear el registro."))
}

pub async fn mover_banco(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    delta: i64,
    ambito: &Ambito,
) -> Result<(), ApiError> {
    if delta == 0 {
        return Ok(());
    }
    let uuid = parsear_uuid(id)?;
    let abs = delta.abs();
    let resultado = if delta < 0 {
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = jsonb_set(datos, '{balanceCents}',
                     to_jsonb((datos->>'balanceCents')::bigint - $3)),
                 actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
               AND (datos->>'balanceCents')::bigint >= $3
             RETURNING id",
        )
        .bind(uuid)
        .bind(ambito.tenant_id.as_deref())
        .bind(abs)
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?
    } else {
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = jsonb_set(datos, '{balanceCents}',
                     to_jsonb((datos->>'balanceCents')::bigint + $3)),
                 actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
             RETURNING id",
        )
        .bind(uuid)
        .bind(ambito.tenant_id.as_deref())
        .bind(abs)
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?
    };
    if resultado.is_some() {
        return Ok(());
    }
    let existe = sqlx::query(
        "SELECT 1 FROM finance.workspace_registros
         WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
           AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
           AND ($3::uuid IS NULL OR propietario_id = $3::uuid)",
    )
    .bind(uuid)
    .bind(ambito.tenant_id.as_deref())
    .bind(ambito.propietario_id.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if existe.is_none() {
        return Err(ApiError::no_encontrado("Cuenta bancaria", id));
    }
    Err(ApiError::dominio(
        "SALDO_INSUFICIENTE",
        "La cuenta no tiene saldo suficiente.",
    ))
}

pub fn con_defaults(mut objeto: Map<String, Value>, defaults: Value) -> Map<String, Value> {
    if let Some(defaults) = defaults.as_object() {
        for (clave, valor) in defaults {
            objeto.entry(clave.clone()).or_insert_with(|| valor.clone());
        }
    }
    objeto
}

pub fn total_lineas(lineas: &Value) -> i64 {
    let Some(lineas) = lineas.as_array() else {
        return 0;
    };
    lineas
        .iter()
        .map(|linea| {
            let precio = numero(linea.get("unitCostCents"));
            let cantidad = numero(linea.get("quantity"));
            (precio as f64 * cantidad as f64).round() as i64
        })
        .sum()
}

pub fn numero(valor: Option<&Value>) -> i64 {
    match valor {
        Some(Value::Number(numero)) => numero
            .as_i64()
            .or_else(|| numero.as_f64().map(|valor| valor.round() as i64))
            .unwrap_or(0),
        Some(Value::String(texto)) => texto.parse().unwrap_or(0),
        _ => 0,
    }
}

pub fn lineas_con_id(lineas: &[LineaCompra]) -> Value {
    Value::Array(
        lineas
            .iter()
            .map(|linea| {
                json!({
                    "id": Uuid::new_v4().to_string(),
                    "productId": linea.product_id,
                    "name": linea.name,
                    "sku": linea.sku.clone().unwrap_or_default(),
                    "quantity": linea.quantity,
                    "unitCostCents": linea.unit_cost_cents,
                    "salePriceCents": linea.sale_price_cents,
                })
            })
            .collect(),
    )
}

pub fn clave_cliente(datos: &Registro) -> String {
    let nombre = datos
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let telefono: String = datos
        .get("phone")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    format!("{nombre}|{telefono}")
}

/// Dígito verificador EAN-13 (pesos 1 y 3 alternados desde la izquierda).
pub fn digito_ean13(base: &str) -> u32 {
    let suma: u32 = base
        .chars()
        .enumerate()
        .filter_map(|(indice, digito)| {
            digito
                .to_digit(10)
                .map(|valor| valor * if indice % 2 == 0 { 1 } else { 3 })
        })
        .sum();
    (10 - (suma % 10)) % 10
}

pub fn generar_codigo(simbologia: &str) -> String {
    if simbologia == "EAN13" {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let base: String = bytes
            .iter()
            .flat_map(|byte| [byte / 10 % 10, byte % 10])
            .take(12)
            .map(|digito| digito.to_string())
            .collect();
        let digito = digito_ean13(&base);
        return format!("{base}{digito}");
    }
    let millis = Utc::now().timestamp_millis() as u64;
    format!("C128-{:X}", millis)
}

pub fn hoy() -> String {
    iso_ms(Utc::now()).chars().take(10).collect()
}

pub fn saldo(registros: &[Registro]) -> i64 {
    registros
        .iter()
        .map(|registro| {
            let monto = numero(registro.get("amountCents"));
            if registro.get("type").and_then(Value::as_str) == Some("ingreso") {
                monto
            } else {
                -monto
            }
        })
        .sum()
}

pub fn suma(registros: &[Registro]) -> i64 {
    registros
        .iter()
        .map(|registro| numero(registro.get("amountCents")))
        .sum()
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::dominio("ID_INVALIDO", "El id debe ser un UUID."))
}

pub fn validar_texto(valor: &str, campo: &str, minimo: usize) -> Result<(), ApiError> {
    if valor.trim().len() < minimo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": [format!("minLength:{minimo}")]}
        ])));
    }
    Ok(())
}

pub fn validar_in(valor: &str, campo: &str, permitidos: &[&str]) -> Result<(), ApiError> {
    if !permitidos.contains(&valor) {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isIn"]}
        ])));
    }
    Ok(())
}

pub fn validar_rango(valor: f64, campo: &str, minimo: f64, maximo: f64) -> Result<(), ApiError> {
    if !valor.is_finite() || valor < minimo || valor > maximo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["min", "max"]}
        ])));
    }
    Ok(())
}

pub fn validar_fecha(valor: &str, campo: &str) -> Result<(), ApiError> {
    let ok =
        valor.len() >= 10 && chrono::NaiveDate::parse_from_str(&valor[..10], "%Y-%m-%d").is_ok();
    if !ok {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isDateString"]}
        ])));
    }
    Ok(())
}

pub fn validar_email(valor: &str, campo: &str) -> Result<(), ApiError> {
    let (usuario, dominio) = valor.split_once('@').unwrap_or_default();
    if usuario.is_empty() || !dominio.contains('.') || dominio.starts_with('.') {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isEmail"]}
        ])));
    }
    Ok(())
}

pub fn validar_lineas(lineas: &[LineaCompra]) -> Result<(), ApiError> {
    for (indice, linea) in lineas.iter().enumerate() {
        if linea.name.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.name"), "restricciones": ["minLength:1"]}
            ])));
        }
        if !linea.quantity.is_finite() || linea.quantity < 1.0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.quantity"), "restricciones": ["min:1"]}
            ])));
        }
        if linea.unit_cost_cents < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.unitCostCents"), "restricciones": ["min:0"]}
            ])));
        }
    }
    Ok(())
}

/// Marca una violación de unicidad de mesa/comanda como conflicto (helper
/// compartido con el resto del servicio).
pub fn conflicto_si_unicidad(error: sqlx::Error, mensaje: &str) -> ApiError {
    if es_violacion_unicidad(&error) {
        ApiError::conflicto(mensaje)
    } else {
        error_db(error)
    }
}
