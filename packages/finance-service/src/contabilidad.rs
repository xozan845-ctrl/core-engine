//! Contabilidad por partida doble (cap. 4.4: libros y registros conformes a la
//! DGI). La bitácora es append-only: un asiento nunca se edita ni se borra, se
//! anula (auditoría). Cada asiento se publica como `asiento.registrado`.

use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, ApiError};
use crate::money;
use crate::outbox::{Outbox, ASIENTO_REGISTRADO};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetalleAsiento {
    pub cuenta_codigo: String,
    pub debe_cents: i64,
    pub haber_cents: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concepto: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orden: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ParametrosAsiento {
    pub concepto: String,
    pub tipo: String,
    pub referencia_tipo: Option<String>,
    pub referencia_id: Option<String>,
    pub fecha: Option<DateTime<Utc>>,
    pub moneda: Option<String>,
    pub creado_por: Option<String>,
    pub detalles: Vec<DetalleAsiento>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Asiento {
    pub id: String,
    pub fecha: String,
    pub concepto: String,
    pub tipo: String,
    pub referencia_tipo: Option<String>,
    pub referencia_id: Option<String>,
    pub moneda: String,
    pub estado: String,
    pub creado_por: Option<String>,
    pub creado_en: String,
    pub debe_cents: i64,
    pub haber_cents: i64,
    pub detalles: Vec<DetalleAsiento>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Cuenta {
    pub codigo: String,
    pub nombre: String,
    pub tipo: String,
    pub naturaleza: String,
    pub nivel: i64,
    pub estado: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LibroIva {
    pub periodo: String,
    pub comprobantes: i64,
    pub base_gravada_cents: i64,
    pub iva_cents: i64,
    pub exento_cents: i64,
    pub total_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibroMayor {
    pub cuenta: Option<Cuenta>,
    pub saldo_inicial_cents: i64,
    pub saldo_final_cents: i64,
    pub movimientos: Vec<Value>,
}

const TIPOS_ASIENTO: [&str; 6] = [
    "INGRESO", "EGRESO", "AJUSTE", "CIERRE", "APERTURA", "MANUAL",
];

pub struct Contabilidad {
    db: std::sync::Arc<Db>,
    outbox: Outbox,
}

impl Contabilidad {
    pub fn nueva(db: std::sync::Arc<Db>, outbox: Outbox) -> Self {
        Self { db, outbox }
    }

    /// Partida doble: debe == haber y al menos un movimiento por lado.
    pub fn validar_partida_doble(detalles: &[DetalleAsiento]) -> Result<(), ApiError> {
        if detalles.len() < 2 {
            return Err(ApiError::dominio(
                "PARTIDA_DOBLE_INVALIDA",
                "Un asiento requiere al menos dos movimientos.",
            ));
        }
        let debe: i64 = detalles.iter().map(|detalle| detalle.debe_cents).sum();
        let haber: i64 = detalles.iter().map(|detalle| detalle.haber_cents).sum();
        if debe <= 0 || debe != haber {
            return Err(ApiError::dominio(
                "PARTIDA_DOBLE_DESCUADRADA",
                format!("El asiento no cuadra: debe {debe} no es igual a haber {haber}."),
            ));
        }
        Ok(())
    }

    pub fn tipo_valido(tipo: &str) -> bool {
        TIPOS_ASIENTO.contains(&tipo)
    }

    /// Inserta asiento + detalles + evento DENTRO de una transacción (ADR-03).
    pub async fn registrar_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        params: &ParametrosAsiento,
    ) -> Result<Asiento, ApiError> {
        Self::validar_partida_doble(&params.detalles)?;
        let tipo = if params.tipo.is_empty() {
            "MANUAL".to_string()
        } else {
            params.tipo.clone()
        };
        if !Self::tipo_valido(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_ASIENTO_INVALIDO",
                "El tipo de asiento no es valido.",
            ));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.asientos
               (fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, creado_por)
             VALUES (COALESCE($1::timestamptz, NOW()), $2, $3, $4, $5, COALESCE($6, 'C$'), $7)
             RETURNING id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda,
                       estado, creado_por, creado_en",
        )
        .bind(params.fecha)
        .bind(&params.concepto)
        .bind(&tipo)
        .bind(&params.referencia_tipo)
        .bind(&params.referencia_id)
        .bind(&params.moneda)
        .bind(&params.creado_por)
        .fetch_one(&mut **tx)
        .await
        .map_err(error_db)?;

        let id = uuid_texto(&fila, "id").map_err(error_db)?;
        for (indice, detalle) in params.detalles.iter().enumerate() {
            sqlx::query(
                "INSERT INTO finance.asiento_detalles
                   (asiento_id, cuenta_codigo, debe_cents, haber_cents, concepto, orden)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(sqlx::types::Uuid::parse_str(&id).map_err(|_| ApiError::interno())?)
            .bind(&detalle.cuenta_codigo)
            .bind(detalle.debe_cents)
            .bind(detalle.haber_cents)
            .bind(&detalle.concepto)
            .bind(detalle.orden.unwrap_or(indice as i64))
            .execute(&mut **tx)
            .await
            .map_err(error_db)?;
        }

        let debe: i64 = params
            .detalles
            .iter()
            .map(|detalle| detalle.debe_cents)
            .sum();
        let haber: i64 = params
            .detalles
            .iter()
            .map(|detalle| detalle.haber_cents)
            .sum();
        let asiento = Asiento {
            id,
            fecha: timestamp(&fila, "fecha").map_err(error_db)?,
            concepto: fila.try_get("concepto").map_err(error_db)?,
            tipo: fila.try_get("tipo").map_err(error_db)?,
            referencia_tipo: fila.try_get("referencia_tipo").map_err(error_db)?,
            referencia_id: fila.try_get("referencia_id").map_err(error_db)?,
            moneda: fila.try_get("moneda").map_err(error_db)?,
            estado: fila.try_get("estado").map_err(error_db)?,
            creado_por: fila.try_get("creado_por").map_err(error_db)?,
            creado_en: timestamp(&fila, "creado_en").map_err(error_db)?,
            debe_cents: debe,
            haber_cents: haber,
            detalles: params.detalles.clone(),
        };

        self.outbox
            .insertar_en_transaccion(
                tx,
                ASIENTO_REGISTRADO,
                json!({
                    "asiento_id": asiento.id,
                    "fecha": asiento.fecha,
                    "tipo": asiento.tipo,
                    "concepto": asiento.concepto,
                    "referencia_tipo": asiento.referencia_tipo,
                    "referencia_id": asiento.referencia_id,
                    "debe_cents": debe,
                    "haber_cents": haber,
                    "moneda": asiento.moneda,
                }),
            )
            .await?;
        Ok(asiento)
    }

    /// Registra un asiento en su propia transacción con publicación por outbox.
    pub async fn registrar(&self, params: ParametrosAsiento) -> Result<Asiento, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let asiento = self.registrar_en_transaccion(&mut tx, &params).await?;
        tx.commit().await.map_err(error_db)?;
        Ok(asiento)
    }

    /// Anula un asiento: solo cambia su estado (append-only, auditoría DGI).
    pub async fn anular(&self, id: &str) -> Result<Option<Asiento>, ApiError> {
        let uuid = sqlx::types::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El asiento debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.asientos SET estado = 'ANULADO'
             WHERE id = $1 AND estado = 'REGISTRADO'
             RETURNING id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda,
                       estado, creado_por, creado_en",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            let existe: Option<sqlx::types::Uuid> =
                sqlx::query_scalar("SELECT id FROM finance.asientos WHERE id = $1")
                    .bind(uuid)
                    .fetch_optional(self.db.pool())
                    .await
                    .map_err(error_db)?;
            return Ok(existe.map(|_| Asiento {
                id: id.to_string(),
                fecha: String::new(),
                concepto: String::new(),
                tipo: String::new(),
                referencia_tipo: None,
                referencia_id: None,
                moneda: String::new(),
                estado: "ANULADO".to_string(),
                creado_por: None,
                creado_en: String::new(),
                debe_cents: 0,
                haber_cents: 0,
                detalles: Vec::new(),
            }));
        };
        let existente = self.asiento_de_fila(&fila).await?;
        Ok(Some(existente))
    }

    async fn asiento_de_fila(&self, fila: &sqlx::postgres::PgRow) -> Result<Asiento, ApiError> {
        let id = uuid_texto(fila, "id").map_err(error_db)?;
        let detalles: Vec<DetalleAsiento> = sqlx::query(
            "SELECT cuenta_codigo, debe_cents, haber_cents, concepto, orden
             FROM finance.asiento_detalles WHERE asiento_id = $1 ORDER BY orden",
        )
        .bind(sqlx::types::Uuid::parse_str(&id).map_err(|_| ApiError::interno())?)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?
        .iter()
        .map(|detalle| DetalleAsiento {
            cuenta_codigo: detalle.try_get("cuenta_codigo").unwrap_or_default(),
            debe_cents: detalle.try_get("debe_cents").unwrap_or(0),
            haber_cents: detalle.try_get("haber_cents").unwrap_or(0),
            concepto: detalle.try_get("concepto").unwrap_or(None),
            orden: detalle.try_get("orden").unwrap_or(None),
        })
        .collect();
        let debe: i64 = detalles.iter().map(|detalle| detalle.debe_cents).sum();
        let haber: i64 = detalles.iter().map(|detalle| detalle.haber_cents).sum();
        Ok(Asiento {
            id,
            fecha: timestamp(fila, "fecha").map_err(error_db)?,
            concepto: fila.try_get("concepto").map_err(error_db)?,
            tipo: fila.try_get("tipo").map_err(error_db)?,
            referencia_tipo: fila.try_get("referencia_tipo").map_err(error_db)?,
            referencia_id: fila.try_get("referencia_id").map_err(error_db)?,
            moneda: fila.try_get("moneda").map_err(error_db)?,
            estado: fila.try_get("estado").map_err(error_db)?,
            creado_por: fila.try_get("creado_por").map_err(error_db)?,
            creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
            debe_cents: debe,
            haber_cents: haber,
            detalles,
        })
    }

    /// Libro diario: bitácora cronológica con totales y detalle (DGI).
    pub async fn libro_diario(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
        limite: i64,
    ) -> Result<Vec<Asiento>, ApiError> {
        let limite = limite.clamp(1, 500);
        let filas = sqlx::query(
            "SELECT id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado,
                    creado_por, creado_en, debe_cents, haber_cents, detalles_jsonb AS detalles
             FROM finance.asientos_vista
             WHERE ($1::timestamptz IS NULL OR fecha >= $1)
               AND ($2::timestamptz IS NULL OR fecha <= $2)
             ORDER BY fecha DESC, id
             LIMIT $3",
        )
        .bind(desde)
        .bind(hasta)
        .bind(limite)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        Ok(filas
            .iter()
            .map(|fila| {
                let detalles: Value = jsonb(fila, "detalles").unwrap_or_else(|_| json!([]));
                Asiento {
                    id: uuid_texto(fila, "id").unwrap_or_default(),
                    fecha: timestamp(fila, "fecha").unwrap_or_default(),
                    concepto: fila.try_get("concepto").unwrap_or_default(),
                    tipo: fila.try_get("tipo").unwrap_or_default(),
                    referencia_tipo: fila.try_get("referencia_tipo").unwrap_or(None),
                    referencia_id: fila.try_get("referencia_id").unwrap_or(None),
                    moneda: fila.try_get("moneda").unwrap_or_default(),
                    estado: fila.try_get("estado").unwrap_or_default(),
                    creado_por: fila.try_get("creado_por").unwrap_or(None),
                    creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
                    debe_cents: fila.try_get::<i32, _>("debe_cents").unwrap_or(0) as i64,
                    haber_cents: fila.try_get::<i32, _>("haber_cents").unwrap_or(0) as i64,
                    detalles: serde_json::from_value(detalles).unwrap_or_default(),
                }
            })
            .collect())
    }

    /// Libro mayor: saldo inicial + movimientos + saldo final por cuenta.
    pub async fn libro_mayor(
        &self,
        cuenta_codigo: &str,
        hasta: Option<DateTime<Utc>>,
    ) -> Result<LibroMayor, ApiError> {
        let cuenta = self.cuenta(cuenta_codigo).await?;
        let Some(cuenta) = cuenta else {
            return Err(ApiError::dominio(
                "CUENTA_INEXISTENTE",
                format!("La cuenta {cuenta_codigo} no existe en el plan."),
            ));
        };
        let fin = hasta.unwrap_or_else(Utc::now);
        let inicio_mes = format!("{}-01", iso_ms(fin).get(0..7).unwrap_or("2026-01"));

        let previo: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(debe_cents) - SUM(haber_cents), 0)::int AS saldo
             FROM finance.libro_mayor_mv
             WHERE cuenta_codigo = $1 AND periodo < $2::date",
        )
        .bind(cuenta_codigo)
        .bind(&inicio_mes)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let saldo_en = |debe: i64, haber: i64| {
            if cuenta.naturaleza == "DEUDORA" {
                debe - haber
            } else {
                haber - debe
            }
        };
        let saldo_inicial = saldo_en(previo.unwrap_or(0) as i64, 0);

        let filas = sqlx::query(
            "SELECT m AS movimiento FROM finance.libro_mayor_mv,
                    jsonb_array_elements(movimientos_jsonb) AS m
             WHERE cuenta_codigo = $1 AND periodo = $2::date
             ORDER BY (m->>'fecha')::timestamptz, m->>'asiento_id'",
        )
        .bind(cuenta_codigo)
        .bind(&inicio_mes)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        let movimientos: Vec<Value> = filas
            .iter()
            .map(|fila| jsonb(fila, "movimiento").unwrap_or(Value::Null))
            .collect();
        let flujo: i64 = movimientos
            .iter()
            .map(|movimiento| {
                let debe = movimiento
                    .get("debe_cents")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let haber = movimiento
                    .get("haber_cents")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                saldo_en(debe, haber)
            })
            .sum();

        Ok(LibroMayor {
            cuenta: Some(cuenta),
            saldo_inicial_cents: saldo_inicial,
            saldo_final_cents: saldo_inicial + flujo,
            movimientos,
        })
    }

    /// Suma de ingresos del periodo (declaraciones de IR/IVA, cap. 4.4).
    pub async fn ingresos_del_periodo(
        &self,
        desde: DateTime<Utc>,
        hasta: DateTime<Utc>,
    ) -> Result<i64, ApiError> {
        let total: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(lm.debe_cents - lm.haber_cents), 0)::int AS total
             FROM finance.libro_mayor_mv lm
             JOIN finance.plan_cuentas c ON c.codigo = lm.cuenta_codigo
             WHERE c.tipo = 'INGRESO'
               AND lm.periodo >= date_trunc('month', $1::timestamptz)::date
               AND lm.periodo <= date_trunc('month', $2::timestamptz)::date",
        )
        .bind(desde)
        .bind(hasta)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(total.unwrap_or(0) as i64)
    }

    /// Suma de gastos y costos del periodo (base de la renta gravable).
    pub async fn gastos_del_periodo(
        &self,
        desde: DateTime<Utc>,
        hasta: DateTime<Utc>,
    ) -> Result<i64, ApiError> {
        let total: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(lm.haber_cents - lm.debe_cents), 0)::int AS total
             FROM finance.libro_mayor_mv lm
             JOIN finance.plan_cuentas c ON c.codigo = lm.cuenta_codigo
             WHERE c.tipo IN ('GASTO', 'COSTO')
               AND lm.periodo >= date_trunc('month', $1::timestamptz)::date
               AND lm.periodo <= date_trunc('month', $2::timestamptz)::date",
        )
        .bind(desde)
        .bind(hasta)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(total.unwrap_or(0) as i64)
    }

    /// Plan de cuentas (libros contables conformes a la DGI).
    pub async fn plan_de_cuentas(&self) -> Result<Vec<Cuenta>, ApiError> {
        let filas = sqlx::query(
            "SELECT codigo, nombre, tipo, naturaleza, nivel, estado
             FROM finance.plan_cuentas ORDER BY codigo",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(cuenta_de_fila).collect())
    }

    async fn cuenta(&self, codigo: &str) -> Result<Option<Cuenta>, ApiError> {
        let fila = sqlx::query(
            "SELECT codigo, nombre, tipo, naturaleza, nivel, estado
             FROM finance.plan_cuentas WHERE codigo = $1",
        )
        .bind(codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(cuenta_de_fila))
    }

    pub async fn crear_cuenta(
        &self,
        codigo: &str,
        nombre: &str,
        tipo: &str,
        naturaleza: &str,
        nivel: i64,
    ) -> Result<Cuenta, ApiError> {
        if !codigo_valido(codigo) {
            return Err(ApiError::dominio(
                "CODIGO_CUENTA_INVALIDO",
                "El codigo de cuenta debe ser jerarquico (ej. 1.1.3).",
            ));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.plan_cuentas (codigo, nombre, tipo, naturaleza, nivel)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING codigo, nombre, tipo, naturaleza, nivel, estado",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(tipo)
        .bind(naturaleza)
        .bind(nivel)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| {
            if crate::errors::es_violacion_unicidad(&error) {
                ApiError::dominio("CUENTA_DUPLICADA", format!("La cuenta {codigo} ya existe."))
            } else {
                error_db(error)
            }
        })?;
        fila.as_ref()
            .map(cuenta_de_fila)
            .ok_or_else(ApiError::interno)
    }

    pub async fn estado_cuenta(
        &self,
        codigo: &str,
        estado: &str,
    ) -> Result<Option<Cuenta>, ApiError> {
        let fila = sqlx::query(
            "UPDATE finance.plan_cuentas SET estado = $2
             WHERE codigo = $1 RETURNING codigo, nombre, tipo, naturaleza, nivel, estado",
        )
        .bind(codigo)
        .bind(estado)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(cuenta_de_fila))
    }

    /// Libro de ventas (DGI): consolidado mensual de comprobantes emitidos.
    pub async fn libro_ventas(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
    ) -> Result<Vec<LibroIva>, ApiError> {
        let filas = sqlx::query(
            "SELECT TO_CHAR(emitido_en, 'YYYY-MM') AS periodo,
                    COUNT(*)::int AS comprobantes,
                    COALESCE(SUM(base_gravada_cents), 0)::int AS base_gravada_cents,
                    COALESCE(SUM(iva_cents), 0)::int AS iva_cents,
                    COALESCE(SUM(exento_cents), 0)::int AS exento_cents,
                    COALESCE(SUM(total_cents), 0)::int AS total_cents
             FROM finance.comprobantes
             WHERE estado = 'EMITIDO'
               AND ($1::timestamptz IS NULL OR emitido_en >= $1)
               AND ($2::timestamptz IS NULL OR emitido_en <= $2)
             GROUP BY TO_CHAR(emitido_en, 'YYYY-MM')
             ORDER BY periodo",
        )
        .bind(desde)
        .bind(hasta)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(libro_iva_de_fila).collect())
    }

    /// Libro de compras (DGI): en el MVP el crédito fiscal es cero.
    pub async fn libro_compras(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
    ) -> Result<Vec<LibroIva>, ApiError> {
        let filas = sqlx::query(
            "SELECT TO_CHAR(creado_en, 'YYYY-MM') AS periodo,
                    0::int AS comprobantes,
                    0::int AS base_gravada_cents,
                    0::int AS iva_cents,
                    0::int AS exento_cents,
                    0::int AS total_cents
             FROM finance.comprobantes
             WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
               AND ($2::timestamptz IS NULL OR creado_en <= $2)
             GROUP BY TO_CHAR(creado_en, 'YYYY-MM')
             ORDER BY periodo",
        )
        .bind(desde)
        .bind(hasta)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(libro_iva_de_fila).collect())
    }

    /// Formato moneda para respuestas (doc 5.7: montos con dos decimales).
    pub fn formatear(centavos: i64) -> String {
        money::string(centavos)
    }

    /// Fecha ISO corta para respuestas de fechas de negocio.
    pub fn fecha_corta(fecha: DateTime<Utc>) -> String {
        iso_ms(fecha).chars().take(10).collect()
    }

    /// Lee un timestamp opcional de un `jsonb` (utilería de proyecciones).
    pub fn timestamp_json(valor: &Value) -> Option<String> {
        valor.as_str().map(str::to_string)
    }

    /// Compatibilidad: expone el pool para módulos que comparten la conexión.
    pub fn db(&self) -> &Db {
        &self.db
    }
}

fn cuenta_de_fila(fila: &sqlx::postgres::PgRow) -> Cuenta {
    Cuenta {
        codigo: fila.try_get("codigo").unwrap_or_default(),
        nombre: fila.try_get("nombre").unwrap_or_default(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        naturaleza: fila.try_get("naturaleza").unwrap_or_default(),
        nivel: fila.try_get::<i32, _>("nivel").unwrap_or(1) as i64,
        estado: fila.try_get("estado").unwrap_or_default(),
    }
}

fn libro_iva_de_fila(fila: &sqlx::postgres::PgRow) -> LibroIva {
    LibroIva {
        periodo: fila.try_get("periodo").unwrap_or_default(),
        comprobantes: fila.try_get::<i32, _>("comprobantes").unwrap_or(0) as i64,
        base_gravada_cents: fila.try_get::<i32, _>("base_gravada_cents").unwrap_or(0) as i64,
        iva_cents: fila.try_get::<i32, _>("iva_cents").unwrap_or(0) as i64,
        exento_cents: fila.try_get::<i32, _>("exento_cents").unwrap_or(0) as i64,
        total_cents: fila.try_get::<i32, _>("total_cents").unwrap_or(0) as i64,
    }
}

fn codigo_valido(codigo: &str) -> bool {
    let partes: Vec<&str> = codigo.split('.').collect();
    !partes.is_empty()
        && partes
            .iter()
            .all(|parte| !parte.is_empty() && parte.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detalle(cuenta: &str, debe: i64, haber: i64) -> DetalleAsiento {
        DetalleAsiento {
            cuenta_codigo: cuenta.to_string(),
            debe_cents: debe,
            haber_cents: haber,
            concepto: None,
            orden: None,
        }
    }

    #[test]
    fn debe_rechazar_un_asiento_con_menos_de_dos_lineas() {
        let error =
            Contabilidad::validar_partida_doble(&[detalle("1.1.1", 100, 0)]).expect_err("invalido");
        assert_eq!(error.codigo, "PARTIDA_DOBLE_INVALIDA");
    }

    #[test]
    fn debe_rechazar_un_asiento_descuadrado() {
        let error = Contabilidad::validar_partida_doble(&[
            detalle("1.1.1", 100, 0),
            detalle("2.1.2", 0, 90),
        ])
        .expect_err("descuadrado");
        assert_eq!(error.codigo, "PARTIDA_DOBLE_DESCUADRADA");
    }

    #[test]
    fn debe_rechazar_un_asiento_en_cero() {
        let error =
            Contabilidad::validar_partida_doble(&[detalle("1.1.1", 0, 0), detalle("2.1.2", 0, 0)])
                .expect_err("cero");
        assert_eq!(error.codigo, "PARTIDA_DOBLE_DESCUADRADA");
    }

    #[test]
    fn debe_aceptar_un_asiento_cuadrado() {
        Contabilidad::validar_partida_doble(&[
            detalle("1.1.1", 115_000, 0),
            detalle("2.1.2", 0, 115_000),
        ])
        .expect("valido");
    }

    #[test]
    fn debe_validar_codigos_jerarquicos() {
        assert!(codigo_valido("1"));
        assert!(codigo_valido("1.1.3"));
        assert!(!codigo_valido(""));
        assert!(!codigo_valido("1..3"));
        assert!(!codigo_valido("A.1"));
    }

    #[test]
    fn debe_validar_los_tipos_de_asiento() {
        for tipo in TIPOS_ASIENTO {
            assert!(Contabilidad::tipo_valido(tipo));
        }
        assert!(!Contabilidad::tipo_valido("OTRO"));
    }
}
