//! Suscripciones y órdenes recurrentes (Fase P2.3).
//!
//! Réplica de `suscripciones.service.ts`: cada vencimiento crea una orden
//! normal con el precio y stock vigentes de la oferta. La reclamación de la
//! suscripción es atómica para que dos workers no generen la misma renovación.

use crate::checkout::{Checkout, ComandoCrear, LineaEntrada};
use crate::db::{timestamp, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::event_store::{error_db, parsear_uuid};
use chrono::{Days, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub const INTERVALOS: [&str; 3] = ["semanal", "mensual", "anual"];
pub const FULFILLMENTS_SUSCRIPCION: [&str; 5] = ["delivery", "pickup", "onsite", "digital", "none"];

const COLUMNAS: &str = "id, empresa_id, oferta_id, item_id, cantidad, comprador_id,
  cliente_parte_id, tipo_orden, metodo_pago, fulfillment, intervalo, estado,
  proxima_ejecucion, ultima_ejecucion, fin_en, metadata, creado_por, creado_en,
  actualizado_en";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suscripcion {
    pub id: String,
    pub empresa_id: String,
    pub oferta_id: String,
    pub item_id: Option<String>,
    pub cantidad: i64,
    pub comprador_id: String,
    pub cliente_parte_id: Option<String>,
    pub tipo_orden: String,
    pub metodo_pago: String,
    pub fulfillment: String,
    pub intervalo: String,
    pub estado: String,
    pub proxima_ejecucion: String,
    pub ultima_ejecucion: Option<String>,
    pub fin_en: Option<String>,
    pub metadata: Value,
    pub creado_por: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SuscripcionEjecucion {
    pub id: String,
    pub suscripcion_id: String,
    pub order_id: Option<String>,
    pub estado: String,
    pub error: String,
    pub ejecutada_en: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CrearSuscripcionInput {
    #[serde(default)]
    pub oferta_id: String,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub cantidad: Option<i64>,
    #[serde(default)]
    pub comprador_id: Option<String>,
    #[serde(default)]
    pub cliente_parte_id: Option<String>,
    #[serde(default)]
    pub metodo_pago: Option<String>,
    #[serde(default)]
    pub fulfillment: Option<String>,
    #[serde(default)]
    pub intervalo: String,
    #[serde(default)]
    pub proxima_ejecucion: Option<String>,
    #[serde(default)]
    pub fin_en: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FiltrosSuscripcion {
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub comprador_id: Option<String>,
    #[serde(default)]
    pub limite: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RenovarResultado {
    pub suscripcion: Suscripcion,
    pub order_id: String,
}

pub struct Suscripciones {
    db: Arc<Db>,
    checkout: Arc<Checkout>,
}

impl Suscripciones {
    pub fn nueva(db: Arc<Db>, checkout: Arc<Checkout>) -> Self {
        Self { db, checkout }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        filtros: FiltrosSuscripcion,
    ) -> Result<Vec<Suscripcion>, ApiError> {
        let mut sql = format!(
            "SELECT {COLUMNAS}
               FROM orders.suscripciones
              WHERE empresa_id = $1::uuid"
        );
        let mut parametros: Vec<&str> = Vec::new();
        if let Some(estado) = filtros.estado.as_deref().filter(|valor| !valor.is_empty()) {
            parametros.push(estado);
            sql.push_str(&format!(" AND estado = ${}", parametros.len() + 1));
        }
        if let Some(comprador) = filtros
            .comprador_id
            .as_deref()
            .filter(|valor| !valor.is_empty())
        {
            parametros.push(comprador);
            sql.push_str(&format!(
                " AND comprador_id = ${}::uuid",
                parametros.len() + 1
            ));
        }
        sql.push_str(&format!(
            " ORDER BY proxima_ejecucion, creado_en DESC LIMIT ${}",
            parametros.len() + 2
        ));

        let mut consulta = sqlx::query(&sql).bind(empresa_id);
        for parametro in &parametros {
            consulta = consulta.bind(*parametro);
        }
        consulta = consulta.bind(limite(filtros.limite.as_deref()));
        let filas = consulta.fetch_all(self.db.pool()).await.map_err(error_db)?;
        filas.iter().map(suscripcion_de_fila).collect()
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        creado_por: &str,
        input: CrearSuscripcionInput,
    ) -> Result<Suscripcion, ApiError> {
        let oferta_id = validar_uuid(&input.oferta_id, "oferta")?;
        let item_id = validar_uuid_opcional(input.item_id.as_deref(), "item")?;
        let cantidad = validar_cantidad(input.cantidad)?;
        let comprador_id = validar_uuid(
            input.comprador_id.as_deref().unwrap_or(creado_por),
            "comprador",
        )?;
        let cliente_parte_id = validar_uuid_opcional(input.cliente_parte_id.as_deref(), "parte")?;
        let metodo_pago = if input.metodo_pago.as_deref() == Some("credito") {
            "credito"
        } else {
            "simulado"
        };
        let fulfillment = validar_fulfillment(input.fulfillment.as_deref())?;
        let intervalo = validar_intervalo(&input.intervalo)?;
        let proxima = match validar_fecha(input.proxima_ejecucion.as_deref(), "proxima_ejecucion")?
        {
            Some(fecha) => fecha,
            None => Utc::now().date_naive(),
        };
        let fin_en = validar_fecha(input.fin_en.as_deref(), "fin_en")?;
        if fin_en.is_some_and(|fin| fin < proxima) {
            return Err(ApiError::dominio(
                "FECHAS_INVALIDAS",
                "La fecha fin no puede ser anterior a la proxima ejecucion.",
            ));
        }
        if metodo_pago == "credito" && cliente_parte_id.is_none() {
            return Err(ApiError::dominio(
                "CREDITO_REQUIERE_PARTE",
                "Una suscripcion a credito requiere cliente_parte_id.",
            ));
        }
        let metadata = validar_metadata(input.metadata)?;

        let fila = sqlx::query(&format!(
            "INSERT INTO orders.suscripciones
               (empresa_id, oferta_id, item_id, cantidad, comprador_id, cliente_parte_id,
                metodo_pago, fulfillment, intervalo, proxima_ejecucion, fin_en,
                metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5::uuid, $6::uuid, $7, $8, $9,
                     $10::date, $11::date, $12::jsonb, $13::uuid)
             RETURNING {COLUMNAS}"
        ))
        .bind(empresa_id)
        .bind(oferta_id)
        .bind(item_id)
        // `cantidad` ya está validada en 1..=99 y la columna es `integer`.
        .bind(cantidad as i32)
        .bind(comprador_id)
        .bind(cliente_parte_id)
        .bind(metodo_pago)
        .bind(fulfillment.as_str())
        .bind(intervalo.as_str())
        .bind(proxima)
        .bind(fin_en)
        .bind(sqlx::types::Json(metadata))
        .bind(creado_por)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "SUSCRIPCION_NO_CREADA",
                "No se pudo crear la suscripcion.",
            ));
        };
        suscripcion_de_fila(&fila)
    }

    pub async fn obtener(&self, empresa_id: &str, id: &str) -> Result<Suscripcion, ApiError> {
        let suscripcion_id = validar_uuid(id, "suscripcion")?;
        self.buscar(empresa_id, suscripcion_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Suscripcion", id))
    }

    pub async fn pausar(&self, empresa_id: &str, id: &str) -> Result<Suscripcion, ApiError> {
        self.cambiar_estado(empresa_id, id, "pausada").await
    }

    pub async fn reanudar(&self, empresa_id: &str, id: &str) -> Result<Suscripcion, ApiError> {
        self.cambiar_estado(empresa_id, id, "activa").await
    }

    pub async fn cancelar(&self, empresa_id: &str, id: &str) -> Result<Suscripcion, ApiError> {
        self.cambiar_estado(empresa_id, id, "cancelada").await
    }

    pub async fn renovar(
        &self,
        empresa_id: &str,
        id: &str,
        forzar: bool,
    ) -> Result<RenovarResultado, ApiError> {
        let suscripcion_id = validar_uuid(id, "suscripcion")?;
        let reclamada = self.reclamar(empresa_id, suscripcion_id, forzar).await?;

        let comando = ComandoCrear {
            tipo_orden: "compra".to_string(),
            items: Some(vec![LineaEntrada {
                oferta_id: reclamada.oferta_id.clone(),
                cantidad: reclamada.cantidad,
                fulfillment: Some(reclamada.fulfillment.clone()),
                fulfillment_config: None,
            }]),
            metodo_pago: Some(reclamada.metodo_pago.clone()),
            cliente_parte_id: reclamada.cliente_parte_id.clone(),
            ..Default::default()
        };

        match self
            .checkout
            .ejecutar(&comando, &reclamada.comprador_id)
            .await
        {
            Ok(vista) => {
                self.registrar_ejecucion(
                    empresa_id,
                    suscripcion_id,
                    Some(vista.id.as_str()),
                    "creada",
                    "",
                )
                .await?;
                Ok(RenovarResultado {
                    suscripcion: reclamada,
                    order_id: vista.id,
                })
            }
            Err(error) => {
                sqlx::query(
                    "UPDATE orders.suscripciones
                        SET proxima_ejecucion = CURRENT_DATE, actualizado_en = NOW()
                      WHERE id = $1::uuid AND empresa_id = $2::uuid",
                )
                .bind(suscripcion_id)
                .bind(empresa_id)
                .execute(self.db.pool())
                .await
                .map_err(error_db)?;
                self.registrar_ejecucion(
                    empresa_id,
                    suscripcion_id,
                    None,
                    "error",
                    &truncar(&error.mensaje, 500),
                )
                .await?;
                Err(error)
            }
        }
    }

    pub async fn renovar_vencidas(&self) -> Result<u64, ApiError> {
        let filas = sqlx::query(
            "SELECT id, empresa_id
               FROM orders.suscripciones
              WHERE estado = 'activa' AND proxima_ejecucion <= CURRENT_DATE
                AND (fin_en IS NULL OR proxima_ejecucion <= fin_en)
              ORDER BY proxima_ejecucion
              LIMIT 50",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        let mut renovadas = 0u64;
        for fila in filas {
            let (Ok(id), Ok(empresa_id)) =
                (uuid_texto(&fila, "id"), uuid_texto(&fila, "empresa_id"))
            else {
                continue;
            };
            // El error queda en suscripcion_ejecuciones; el siguiente ciclo
            // reintenta.
            if self.renovar(&empresa_id, &id, false).await.is_ok() {
                renovadas += 1;
            }
        }
        Ok(renovadas)
    }

    pub async fn listar_ejecuciones(
        &self,
        empresa_id: &str,
        id: &str,
    ) -> Result<Vec<SuscripcionEjecucion>, ApiError> {
        let suscripcion_id = validar_uuid(id, "suscripcion")?;
        if self.buscar(empresa_id, suscripcion_id).await?.is_none() {
            return Err(ApiError::no_encontrado("Suscripcion", id));
        }
        let filas = sqlx::query(
            "SELECT id, suscripcion_id, order_id, estado, error, ejecutada_en
               FROM orders.suscripcion_ejecuciones
              WHERE empresa_id = $1::uuid AND suscripcion_id = $2::uuid
              ORDER BY ejecutada_en DESC",
        )
        .bind(empresa_id)
        .bind(suscripcion_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(ejecucion_de_fila).collect()
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    /// Reclama la suscripción en una transacción: bloquea la fila, valida el
    /// vencimiento y programa la siguiente ejecución antes de crear la orden.
    async fn reclamar(
        &self,
        empresa_id: &str,
        suscripcion_id: Uuid,
        forzar: bool,
    ) -> Result<Suscripcion, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "SELECT GREATEST(proxima_ejecucion, CURRENT_DATE) AS base, intervalo
               FROM orders.suscripciones
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activa'
                AND ($3::boolean OR proxima_ejecucion <= CURRENT_DATE)
                AND (fin_en IS NULL OR proxima_ejecucion <= fin_en)
              FOR UPDATE",
        )
        .bind(suscripcion_id)
        .bind(empresa_id)
        .bind(forzar)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(no_renovable());
        };
        let base: NaiveDate = fila.try_get("base").map_err(error_db)?;
        let intervalo: String = fila.try_get("intervalo").map_err(error_db)?;
        let proxima = siguiente_fecha(base, &intervalo);

        let actualizada = sqlx::query(&format!(
            "UPDATE orders.suscripciones
                SET ultima_ejecucion = CURRENT_DATE,
                    proxima_ejecucion = $3::date,
                    actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
              RETURNING {COLUMNAS}"
        ))
        .bind(suscripcion_id)
        .bind(empresa_id)
        .bind(proxima)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;

        match actualizada {
            Some(fila) => suscripcion_de_fila(&fila),
            None => Err(no_renovable()),
        }
    }

    async fn cambiar_estado(
        &self,
        empresa_id: &str,
        id: &str,
        estado: &str,
    ) -> Result<Suscripcion, ApiError> {
        let suscripcion_id = validar_uuid(id, "suscripcion")?;
        let actual = self
            .buscar(empresa_id, suscripcion_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Suscripcion", id))?;
        if !transicion_permitida(&actual.estado, estado) {
            return Err(ApiError::conflicto(format!(
                "No se puede pasar de {} a {}.",
                actual.estado, estado
            )));
        }
        let fila = sqlx::query(&format!(
            "UPDATE orders.suscripciones
                SET estado = $3, actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
              RETURNING {COLUMNAS}"
        ))
        .bind(suscripcion_id)
        .bind(empresa_id)
        .bind(estado)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Suscripcion", id));
        };
        suscripcion_de_fila(&fila)
    }

    async fn buscar(
        &self,
        empresa_id: &str,
        suscripcion_id: Uuid,
    ) -> Result<Option<Suscripcion>, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM orders.suscripciones
              WHERE id = $1::uuid AND empresa_id = $2::uuid"
        ))
        .bind(suscripcion_id)
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.as_ref().map(suscripcion_de_fila).transpose()
    }

    async fn registrar_ejecucion(
        &self,
        empresa_id: &str,
        suscripcion_id: Uuid,
        order_id: Option<&str>,
        estado: &str,
        error: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO orders.suscripcion_ejecuciones
               (empresa_id, suscripcion_id, order_id, estado, error)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5)",
        )
        .bind(empresa_id)
        .bind(suscripcion_id)
        .bind(order_id)
        .bind(estado)
        .bind(error)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

fn suscripcion_de_fila(fila: &PgRow) -> Result<Suscripcion, ApiError> {
    Ok(Suscripcion {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        oferta_id: uuid_texto(fila, "oferta_id").map_err(error_db)?,
        item_id: uuid_opcional(fila, "item_id").map_err(error_db)?,
        // La columna es `integer`; sqlx exige el tipo exacto (INT4 -> i32).
        cantidad: fila.try_get::<i32, _>("cantidad").map_err(error_db)? as i64,
        comprador_id: uuid_texto(fila, "comprador_id").map_err(error_db)?,
        cliente_parte_id: uuid_opcional(fila, "cliente_parte_id").map_err(error_db)?,
        // El contrato expone siempre `compra`, como hace `serializar` en TS.
        tipo_orden: "compra".to_string(),
        metodo_pago: fila.try_get("metodo_pago").map_err(error_db)?,
        fulfillment: fila.try_get("fulfillment").map_err(error_db)?,
        intervalo: fila.try_get("intervalo").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        proxima_ejecucion: fecha_de_fila(fila, "proxima_ejecucion")?,
        ultima_ejecucion: fecha_opcional_de_fila(fila, "ultima_ejecucion")?,
        fin_en: fecha_opcional_de_fila(fila, "fin_en")?,
        metadata: fila
            .try_get::<Option<Value>, _>("metadata")
            .map_err(error_db)?
            .unwrap_or_else(|| json!({})),
        creado_por: uuid_texto(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn ejecucion_de_fila(fila: &PgRow) -> Result<SuscripcionEjecucion, ApiError> {
    Ok(SuscripcionEjecucion {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        suscripcion_id: uuid_texto(fila, "suscripcion_id").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        error: fila
            .try_get::<Option<String>, _>("error")
            .map_err(error_db)?
            .unwrap_or_default(),
        ejecutada_en: timestamp(fila, "ejecutada_en").map_err(error_db)?,
    })
}

fn fecha_de_fila(fila: &PgRow, columna: &str) -> Result<String, ApiError> {
    fila.try_get::<NaiveDate, _>(columna)
        .map(|fecha| fecha.format("%Y-%m-%d").to_string())
        .map_err(error_db)
}

fn fecha_opcional_de_fila(fila: &PgRow, columna: &str) -> Result<Option<String>, ApiError> {
    fila.try_get::<Option<NaiveDate>, _>(columna)
        .map(|fecha| fecha.map(|valor| valor.format("%Y-%m-%d").to_string()))
        .map_err(error_db)
}

/// Próxima ejecución con el mismo redondeo de PostgreSQL: los meses y años se
/// recortan al último día del mes destino (31-ene + 1 mes = 28/29-feb).
fn siguiente_fecha(base: NaiveDate, intervalo: &str) -> NaiveDate {
    match intervalo {
        "semanal" => base.checked_add_days(Days::new(7)).unwrap_or(base),
        "mensual" => base.checked_add_months(Months::new(1)).unwrap_or(base),
        _ => base.checked_add_months(Months::new(12)).unwrap_or(base),
    }
}

fn transicion_permitida(actual: &str, nueva: &str) -> bool {
    match actual {
        "activa" => matches!(nueva, "pausada" | "cancelada"),
        "pausada" => matches!(nueva, "activa" | "cancelada"),
        _ => false,
    }
}

fn validar_cantidad(valor: Option<i64>) -> Result<i64, ApiError> {
    let cantidad = valor.unwrap_or(1);
    if !(1..=99).contains(&cantidad) {
        return Err(ApiError::dominio(
            "CANTIDAD_INVALIDA",
            "La cantidad debe ser un entero entre 1 y 99.",
        ));
    }
    Ok(cantidad)
}

fn validar_intervalo(valor: &str) -> Result<String, ApiError> {
    if INTERVALOS.contains(&valor) {
        Ok(valor.to_string())
    } else {
        Err(ApiError::dominio(
            "INTERVALO_INVALIDO",
            "El intervalo debe ser semanal, mensual o anual.",
        ))
    }
}

fn validar_fulfillment(valor: Option<&str>) -> Result<String, ApiError> {
    let fulfillment = valor.unwrap_or("none");
    if FULFILLMENTS_SUSCRIPCION.contains(&fulfillment) {
        Ok(fulfillment.to_string())
    } else {
        Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "El fulfillment de la suscripcion no es valido.",
        ))
    }
}

fn validar_metadata(valor: Option<Value>) -> Result<Value, ApiError> {
    match valor {
        None | Some(Value::Null) => Ok(json!({})),
        Some(valor) if valor.is_object() => Ok(valor),
        Some(_) => Err(ApiError::dominio(
            "METADATA_INVALIDA",
            "La metadata debe ser un objeto.",
        )),
    }
}

fn validar_fecha(valor: Option<&str>, campo: &str) -> Result<Option<NaiveDate>, ApiError> {
    let Some(texto) = valor.filter(|texto| !texto.is_empty()) else {
        return Ok(None);
    };
    let forma_valida = texto.len() == 10
        && texto.bytes().enumerate().all(|(indice, byte)| {
            if indice == 4 || indice == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });
    if !forma_valida {
        return Err(fecha_invalida(campo));
    }
    NaiveDate::parse_from_str(texto, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| fecha_invalida(campo))
}

fn validar_uuid_opcional(valor: Option<&str>, campo: &str) -> Result<Option<Uuid>, ApiError> {
    match valor {
        None | Some("") => Ok(None),
        Some(texto) => validar_uuid(texto, campo).map(Some),
    }
}

fn validar_uuid(valor: &str, campo: &str) -> Result<Uuid, ApiError> {
    match parsear_uuid(valor) {
        Ok(uuid) if uuid.hyphenated().to_string().eq_ignore_ascii_case(valor) => Ok(uuid),
        _ => Err(ApiError::dominio(
            "ID_INVALIDO",
            format!("El {campo} debe ser un UUID."),
        )),
    }
}

fn fecha_invalida(campo: &str) -> ApiError {
    ApiError::dominio("FECHA_INVALIDA", format!("El {campo} debe ser YYYY-MM-DD."))
}

fn no_renovable() -> ApiError {
    ApiError::nueva(
        "SUSCRIPCION_NO_RENOVABLE",
        "La suscripcion no esta activa o aun no vence.",
        axum::http::StatusCode::CONFLICT,
    )
}

/// `Math.min(Math.max(Number(valor) || 50, 1), 200)` de TS.
fn limite(valor: Option<&str>) -> i64 {
    let numero = valor
        .and_then(|texto| texto.trim().parse::<f64>().ok())
        .filter(|numero| !numero.is_nan())
        .unwrap_or(0.0);
    let numero = if numero == 0.0 { 50.0 } else { numero };
    numero.clamp(1.0, 200.0).trunc() as i64
}

/// `String.prototype.slice(0, 500)`: recorta por caracteres, no por bytes.
fn truncar(texto: &str, maximo: usize) -> String {
    texto.chars().take(maximo).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha valida")
    }

    #[test]
    fn debe_avanzar_una_semana() {
        assert_eq!(
            siguiente_fecha(fecha(2026, 12, 28), "semanal"),
            fecha(2027, 1, 4)
        );
        assert_eq!(
            siguiente_fecha(fecha(2026, 11, 13), "semanal"),
            fecha(2026, 11, 20)
        );
    }

    #[test]
    fn debe_avanzar_un_mes_recortando_al_ultimo_dia() {
        assert_eq!(
            siguiente_fecha(fecha(2026, 1, 31), "mensual"),
            fecha(2026, 2, 28)
        );
        assert_eq!(
            siguiente_fecha(fecha(2028, 1, 31), "mensual"),
            fecha(2028, 2, 29)
        );
        assert_eq!(
            siguiente_fecha(fecha(2026, 3, 31), "mensual"),
            fecha(2026, 4, 30)
        );
        assert_eq!(
            siguiente_fecha(fecha(2026, 1, 15), "mensual"),
            fecha(2026, 2, 15)
        );
    }

    #[test]
    fn debe_avanzar_un_anio_recortando_el_29_de_febrero() {
        assert_eq!(
            siguiente_fecha(fecha(2024, 2, 29), "anual"),
            fecha(2025, 2, 28)
        );
        assert_eq!(
            siguiente_fecha(fecha(2026, 7, 15), "anual"),
            fecha(2027, 7, 15)
        );
    }

    #[test]
    fn debe_tratar_cualquier_intervalo_desconocido_como_anual() {
        assert_eq!(
            siguiente_fecha(fecha(2026, 7, 15), "trimestral"),
            fecha(2027, 7, 15)
        );
    }

    #[test]
    fn debe_validar_las_transiciones_de_estado() {
        assert!(transicion_permitida("activa", "pausada"));
        assert!(transicion_permitida("activa", "cancelada"));
        assert!(!transicion_permitida("activa", "activa"));
        assert!(transicion_permitida("pausada", "activa"));
        assert!(transicion_permitida("pausada", "cancelada"));
        assert!(!transicion_permitida("pausada", "pausada"));
        assert!(!transicion_permitida("cancelada", "activa"));
        assert!(!transicion_permitida("cancelada", "cancelada"));
        assert!(!transicion_permitida("desconocida", "activa"));
    }

    #[test]
    fn debe_validar_fechas_con_forma_y_calendario() {
        assert_eq!(
            validar_fecha(Some("2026-02-28"), "fin_en").expect("fecha"),
            Some(fecha(2026, 2, 28))
        );
        assert_eq!(validar_fecha(None, "fin_en").expect("nula"), None);
        assert_eq!(validar_fecha(Some(""), "fin_en").expect("vacia"), None);
        assert_eq!(
            validar_fecha(Some("2026-02-30"), "fin_en")
                .expect_err("invalida")
                .codigo,
            "FECHA_INVALIDA"
        );
        assert_eq!(
            validar_fecha(Some("2026-2-03"), "fin_en")
                .expect_err("forma")
                .codigo,
            "FECHA_INVALIDA"
        );
        assert_eq!(
            validar_fecha(Some("hola"), "fin_en")
                .expect_err("texto")
                .codigo,
            "FECHA_INVALIDA"
        );
    }

    #[test]
    fn debe_validar_la_cantidad_entre_uno_y_noventa_y_nueve() {
        assert_eq!(validar_cantidad(None).expect("default"), 1);
        assert_eq!(validar_cantidad(Some(99)).expect("maximo"), 99);
        for invalida in [Some(0), Some(100), Some(-1)] {
            assert_eq!(
                validar_cantidad(invalida).expect_err("invalida").codigo,
                "CANTIDAD_INVALIDA"
            );
        }
    }

    #[test]
    fn debe_validar_intervalo_y_fulfillment() {
        for intervalo in INTERVALOS {
            assert_eq!(validar_intervalo(intervalo).expect("intervalo"), intervalo);
        }
        assert_eq!(
            validar_intervalo("diario").expect_err("intervalo").codigo,
            "INTERVALO_INVALIDO"
        );
        assert_eq!(
            validar_fulfillment(None).expect("default"),
            "none".to_string()
        );
        for fulfillment in FULFILLMENTS_SUSCRIPCION {
            assert_eq!(
                validar_fulfillment(Some(fulfillment)).expect("fulfillment"),
                fulfillment
            );
        }
        assert_eq!(
            validar_fulfillment(Some("dron"))
                .expect_err("fulfillment")
                .codigo,
            "FULFILLMENT_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_la_metadata_como_objeto() {
        assert_eq!(validar_metadata(None).expect("nula"), json!({}));
        assert_eq!(
            validar_metadata(Some(Value::Null)).expect("null"),
            json!({})
        );
        assert_eq!(
            validar_metadata(Some(json!({"origen": "web"}))).expect("objeto"),
            json!({"origen": "web"})
        );
        assert_eq!(
            validar_metadata(Some(json!([])))
                .expect_err("arreglo")
                .codigo,
            "METADATA_INVALIDA"
        );
    }

    #[test]
    fn debe_calcular_el_limite_como_javascript() {
        assert_eq!(limite(None), 50);
        assert_eq!(limite(Some("")), 50);
        assert_eq!(limite(Some("abc")), 50);
        // `Number("0") || 50` en TS: cero es falsy y cae al default.
        assert_eq!(limite(Some("0")), 50);
        assert_eq!(limite(Some("-5")), 1);
        assert_eq!(limite(Some("0.4")), 1);
        assert_eq!(limite(Some("10")), 10);
        assert_eq!(limite(Some(" 25 ")), 25);
        assert_eq!(limite(Some("1.9")), 1);
        assert_eq!(limite(Some("999")), 200);
        assert_eq!(limite(Some("Infinity")), 200);
    }

    #[test]
    fn debe_rechazar_identificadores_que_no_son_uuid_canonicos() {
        let valido =
            validar_uuid("123E4567-E89B-12D3-A456-426614174000", "suscripcion").expect("uuid");
        assert_eq!(valido.to_string(), "123e4567-e89b-12d3-a456-426614174000");
        for invalido in ["", "no-uuid", "123e4567e89b12d3a456426614174000"] {
            assert_eq!(
                validar_uuid(invalido, "suscripcion")
                    .expect_err("invalido")
                    .codigo,
                "ID_INVALIDO"
            );
        }
        assert_eq!(
            validar_uuid_opcional(Some(""), "item").expect("vacio"),
            None
        );
        assert!(validar_uuid_opcional(None, "item").expect("nulo").is_none());
    }

    #[test]
    fn debe_truncar_los_errores_a_quinientos_caracteres() {
        let largo = "á".repeat(600);
        assert_eq!(truncar(&largo, 500).chars().count(), 500);
        assert_eq!(truncar("corto", 500), "corto");
    }
}
