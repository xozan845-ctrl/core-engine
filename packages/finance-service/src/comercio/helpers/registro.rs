use super::super::*;

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

/// Marca una violación de unicidad de mesa/comanda como conflicto (helper
/// compartido con el resto del servicio).
pub fn conflicto_si_unicidad(error: sqlx::Error, mensaje: &str) -> ApiError {
    if es_violacion_unicidad(&error) {
        ApiError::conflicto(mensaje)
    } else {
        error_db(error)
    }
}
