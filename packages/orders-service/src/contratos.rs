//! Contratos con deposito/garantia (Fase P2.2).
//!
//! Réplica de `pedidos/contratos.service.ts`: alquileres, servicios y acuerdos
//! que retienen un deposito. El contrato puede vincularse a una orden y
//! conserva su historial de movimientos en `orders.contrato_eventos`.

use crate::db::{timestamp, timestamp_opcional, uuid_opcional, uuid_texto};
use crate::errors::ApiError;
use crate::event_store::{error_db, parsear_uuid};
use crate::money;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const TIPOS_CONTRATO: [&str; 4] = ["alquiler", "servicio", "compra", "otro"];
pub const ESTADOS_CONTRATO: [&str; 4] = ["borrador", "activo", "finalizado", "cancelado"];

const COLUMNAS: &str = "id, empresa_id, order_id, parte_id, item_id, tipo, titulo,
  descripcion, estado, inicia_en, termina_en, deposito_cents, garantia_cents,
  retenido_cents, moneda, metadata, creado_por, creado_en, actualizado_en,
  finalizado_en";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Contrato {
    pub id: String,
    pub empresa_id: String,
    pub order_id: Option<String>,
    pub parte_id: Option<String>,
    pub item_id: Option<String>,
    pub tipo: String,
    pub titulo: String,
    pub descripcion: String,
    pub estado: String,
    pub inicia_en: Option<String>,
    pub termina_en: Option<String>,
    pub deposito: String,
    pub garantia: String,
    pub retenido: String,
    pub moneda: String,
    pub metadata: Value,
    pub creado_por: String,
    pub creado_en: String,
    pub actualizado_en: String,
    pub finalizado_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContratoEvento {
    pub id: String,
    pub contrato_id: String,
    pub tipo: String,
    pub monto: String,
    pub motivo: String,
    pub creado_por: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CrearContratoInput {
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub titulo: String,
    #[serde(default)]
    pub descripcion: Option<String>,
    #[serde(default)]
    pub inicia_en: Option<String>,
    #[serde(default)]
    pub termina_en: Option<String>,
    #[serde(default)]
    pub deposito: Option<String>,
    #[serde(default)]
    pub garantia: Option<String>,
    #[serde(default)]
    pub moneda: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActualizarContratoInput {
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub titulo: Option<String>,
    #[serde(default)]
    pub descripcion: Option<String>,
    #[serde(default)]
    pub inicia_en: Option<String>,
    #[serde(default)]
    pub termina_en: Option<String>,
    #[serde(default)]
    pub deposito: Option<String>,
    #[serde(default)]
    pub garantia: Option<String>,
    #[serde(default)]
    pub moneda: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FinalizarContratoInput {
    #[serde(default)]
    pub retenido: Option<String>,
    #[serde(default)]
    pub motivo: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltrosContrato {
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub pagina: Option<String>,
    #[serde(default)]
    pub limite: Option<String>,
}

pub struct Contratos {
    db: std::sync::Arc<crate::db::Db>,
}

impl Contratos {
    pub fn nueva(db: std::sync::Arc<crate::db::Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        filtros: FiltrosContrato,
    ) -> Result<Vec<Contrato>, ApiError> {
        let estado = filtros.estado.filter(|valor| !valor.is_empty());
        let order_id = filtros
            .order_id
            .filter(|valor| !valor.is_empty())
            .map(|valor| validar_uuid(&valor, "orden"))
            .transpose()?;
        let parte_id = filtros
            .parte_id
            .filter(|valor| !valor.is_empty())
            .map(|valor| validar_uuid(&valor, "parte"))
            .transpose()?;

        let mut sql = format!("SELECT {COLUMNAS} FROM orders.contratos WHERE empresa_id = $1");
        let mut indice = 1;
        if estado.is_some() {
            indice += 1;
            sql.push_str(&format!(" AND estado = ${indice}"));
        }
        if order_id.is_some() {
            indice += 1;
            sql.push_str(&format!(" AND order_id = ${indice}"));
        }
        if parte_id.is_some() {
            indice += 1;
            sql.push_str(&format!(" AND parte_id = ${indice}"));
        }
        let limite = limite_de(filtros.limite.as_deref());
        let pagina = pagina_de(filtros.pagina.as_deref());
        let offset = ((pagina - 1.0) * limite as f64) as i64;
        indice += 1;
        sql.push_str(&format!(" ORDER BY creado_en DESC LIMIT ${indice}"));
        indice += 1;
        sql.push_str(&format!(" OFFSET ${indice}"));

        let mut consulta = sqlx::query(&sql).bind(parsear_uuid(empresa_id)?);
        if let Some(valor) = estado.as_deref() {
            consulta = consulta.bind(valor);
        }
        if let Some(valor) = order_id {
            consulta = consulta.bind(valor);
        }
        if let Some(valor) = parte_id {
            consulta = consulta.bind(valor);
        }
        let filas = consulta
            .bind(limite)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;
        filas.iter().map(contrato_de_fila).collect()
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        creado_por: &str,
        input: CrearContratoInput,
    ) -> Result<Contrato, ApiError> {
        let titulo = validar_titulo(&input.titulo)?;
        let tipo = validar_tipo(input.tipo.as_deref())?;
        let descripcion = recortar(input.descripcion.as_deref(), 2000);
        let (inicia, termina) =
            validar_fechas(input.inicia_en.as_deref(), input.termina_en.as_deref())?;
        let deposito = monto(input.deposito.as_deref())?;
        let garantia = monto(input.garantia.as_deref())?;
        let moneda = validar_moneda(input.moneda.as_deref())?;
        let metadata = validar_metadata(input.metadata)?;
        let order_id = validar_uuid_opcional(input.order_id.as_deref(), "orden")?;
        let parte_id = validar_uuid_opcional(input.parte_id.as_deref(), "parte")?;
        let item_id = validar_uuid_opcional(input.item_id.as_deref(), "item")?;

        let mut parte_final = parte_id;
        if let Some(order_id) = order_id {
            let orden = sqlx::query("SELECT parte_id FROM orders.orden_vista WHERE id = $1")
                .bind(order_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(error_db)?;
            let Some(orden) = orden else {
                return Err(ApiError::no_encontrado(
                    "Orden",
                    input.order_id.as_deref().unwrap_or_default(),
                ));
            };
            if parte_final.is_none() {
                parte_final = orden.try_get("parte_id").map_err(error_db)?;
            }
        }

        let sql = format!(
            "INSERT INTO orders.contratos
               (empresa_id, order_id, parte_id, item_id, tipo, titulo, descripcion,
                inicia_en, termina_en, deposito_cents, garantia_cents, moneda,
                metadata, creado_por)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                     $13::jsonb, $14)
             RETURNING {COLUMNAS}"
        );
        let fila = sqlx::query(&sql)
            .bind(parsear_uuid(empresa_id)?)
            .bind(order_id)
            .bind(parte_final)
            .bind(item_id)
            .bind(&tipo)
            .bind(&titulo)
            .bind(&descripcion)
            .bind(inicia)
            .bind(termina)
            .bind(deposito)
            .bind(garantia)
            .bind(&moneda)
            .bind(sqlx::types::Json(&metadata))
            .bind(parsear_uuid(creado_por)?)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "CONTRATO_NO_CREADO",
                "No se pudo crear el contrato.",
            ));
        };
        contrato_de_fila(&fila)
    }

    pub async fn obtener(&self, empresa_id: &str, id: &str) -> Result<Contrato, ApiError> {
        let fila = self.buscar_fila(empresa_id, id).await?;
        contrato_de_fila(&fila)
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        id: &str,
        input: ActualizarContratoInput,
    ) -> Result<Contrato, ApiError> {
        let actual = self.buscar_fila(empresa_id, id).await?;
        let estado_actual: String = actual.try_get("estado").map_err(error_db)?;
        if estado_actual == "finalizado" || estado_actual == "cancelado" {
            return Err(ApiError::conflicto("El contrato ya esta cerrado."));
        }
        let estado = input.estado.unwrap_or_else(|| estado_actual.clone());
        validar_transicion(&estado_actual, &estado)?;

        let titulo_actual: String = actual.try_get("titulo").map_err(error_db)?;
        let tipo_actual: String = actual.try_get("tipo").map_err(error_db)?;
        let descripcion_actual: String = actual.try_get("descripcion").map_err(error_db)?;
        let deposito_actual = centavos(&actual, "deposito_cents")?;
        let garantia_actual = centavos(&actual, "garantia_cents")?;
        let retenido_actual = centavos(&actual, "retenido_cents")?;
        let moneda_actual: String = actual.try_get("moneda").map_err(error_db)?;
        let metadata_actual: Option<Value> = actual.try_get("metadata").map_err(error_db)?;
        let inicia_actual: Option<NaiveDate> = actual.try_get("inicia_en").map_err(error_db)?;
        let termina_actual: Option<NaiveDate> = actual.try_get("termina_en").map_err(error_db)?;
        let creado_por: Uuid = actual.try_get("creado_por").map_err(error_db)?;

        let titulo = match input.titulo.as_deref() {
            Some(valor) => validar_titulo(valor)?,
            None => titulo_actual,
        };
        let tipo = match input.tipo.as_deref() {
            Some(valor) => validar_tipo(Some(valor))?,
            None => tipo_actual,
        };
        let descripcion = match input.descripcion.as_deref() {
            Some(valor) => recortar(Some(valor), 2000),
            None => descripcion_actual,
        };
        let inicia = match input.inicia_en.as_deref() {
            Some(valor) => validar_fecha(Some(valor), "inicia_en")?,
            None => inicia_actual,
        };
        let termina = match input.termina_en.as_deref() {
            Some(valor) => validar_fecha(Some(valor), "termina_en")?,
            None => termina_actual,
        };
        validar_rango(inicia, termina)?;
        let deposito = match input.deposito.as_deref() {
            Some(valor) => money::parsear(valor)?,
            None => deposito_actual,
        };
        if retenido_actual > deposito {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                "El deposito no puede ser menor que lo retenido.",
            ));
        }
        let garantia = match input.garantia.as_deref() {
            Some(valor) => money::parsear(valor)?,
            None => garantia_actual,
        };
        let moneda = match input.moneda.as_deref() {
            Some(valor) => validar_moneda(Some(valor))?,
            None => moneda_actual,
        };
        let metadata = match input.metadata {
            Some(valor) => validar_metadata(Some(valor))?,
            None => metadata_actual
                .filter(|valor| !valor.is_null())
                .unwrap_or_else(|| json!({})),
        };

        let empresa_uuid = parsear_uuid(empresa_id)?;
        let contrato_uuid = validar_uuid(id, "contrato")?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let sql = format!(
            "UPDATE orders.contratos
                SET titulo = $2, tipo = $3, descripcion = $4, estado = $5,
                    inicia_en = $6, termina_en = $7, deposito_cents = $8,
                    garantia_cents = $9, moneda = $10, metadata = $11::jsonb,
                    actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $12
              RETURNING {COLUMNAS}"
        );
        let fila = sqlx::query(&sql)
            .bind(contrato_uuid)
            .bind(&titulo)
            .bind(&tipo)
            .bind(&descripcion)
            .bind(&estado)
            .bind(inicia)
            .bind(termina)
            .bind(deposito)
            .bind(garantia)
            .bind(&moneda)
            .bind(sqlx::types::Json(&metadata))
            .bind(empresa_uuid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Contrato", id));
        };
        if estado == "activo" && estado_actual != "activo" && deposito > 0 {
            registrar_evento(
                &mut tx,
                empresa_uuid,
                contrato_uuid,
                "deposito_recibido",
                deposito,
                "Deposito al activar el contrato",
                creado_por,
            )
            .await?;
        }
        if estado == "cancelado" && estado_actual != "cancelado" {
            registrar_evento(
                &mut tx,
                empresa_uuid,
                contrato_uuid,
                "cancelacion",
                0,
                "Contrato cancelado",
                creado_por,
            )
            .await?;
            if estado_actual == "activo" && deposito_actual > 0 {
                registrar_evento(
                    &mut tx,
                    empresa_uuid,
                    contrato_uuid,
                    "deposito_devuelto",
                    deposito_actual,
                    "Devolucion por cancelacion",
                    creado_por,
                )
                .await?;
            }
        }
        tx.commit().await.map_err(error_db)?;
        contrato_de_fila(&fila)
    }

    pub async fn finalizar(
        &self,
        empresa_id: &str,
        id: &str,
        actor: &str,
        input: FinalizarContratoInput,
    ) -> Result<Contrato, ApiError> {
        let actual = self.buscar_fila(empresa_id, id).await?;
        let estado_actual: String = actual.try_get("estado").map_err(error_db)?;
        if estado_actual != "activo" {
            return Err(ApiError::conflicto(
                "Solo un contrato activo puede finalizarse.",
            ));
        }
        let deposito_actual = centavos(&actual, "deposito_cents")?;
        let retenido = monto(input.retenido.as_deref())?;
        if retenido > deposito_actual {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                "Lo retenido no puede superar el deposito.",
            ));
        }
        let devuelto = deposito_actual - retenido;
        let motivo = recortar(input.motivo.as_deref(), 500);

        let empresa_uuid = parsear_uuid(empresa_id)?;
        let contrato_uuid = validar_uuid(id, "contrato")?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let sql = format!(
            "UPDATE orders.contratos
                SET estado = 'finalizado', retenido_cents = $3, finalizado_en = NOW(),
                    actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2 AND estado = 'activo'
              RETURNING {COLUMNAS}"
        );
        let fila = sqlx::query(&sql)
            .bind(contrato_uuid)
            .bind(empresa_uuid)
            .bind(retenido)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::conflicto("El contrato ya no esta activo."));
        };
        if retenido > 0 {
            registrar_evento(
                &mut tx,
                empresa_uuid,
                contrato_uuid,
                "retencion",
                retenido,
                &motivo,
                parsear_uuid(actor)?,
            )
            .await?;
        }
        if devuelto > 0 {
            registrar_evento(
                &mut tx,
                empresa_uuid,
                contrato_uuid,
                "deposito_devuelto",
                devuelto,
                &motivo,
                parsear_uuid(actor)?,
            )
            .await?;
        }
        tx.commit().await.map_err(error_db)?;
        contrato_de_fila(&fila)
    }

    pub async fn listar_eventos(
        &self,
        empresa_id: &str,
        id: &str,
    ) -> Result<Vec<ContratoEvento>, ApiError> {
        self.buscar_fila(empresa_id, id).await?;
        let filas = sqlx::query(
            "SELECT id, contrato_id, tipo, monto_cents, motivo, creado_por, creado_en
               FROM orders.contrato_eventos
              WHERE empresa_id = $1 AND contrato_id = $2
              ORDER BY creado_en, id",
        )
        .bind(parsear_uuid(empresa_id)?)
        .bind(validar_uuid(id, "contrato")?)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(evento_de_fila).collect()
    }

    async fn buscar_fila(&self, empresa_id: &str, id: &str) -> Result<PgRow, ApiError> {
        let contrato_id = validar_uuid(id, "contrato")?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.contratos WHERE id = $1 AND empresa_id = $2"
        ))
        .bind(contrato_id)
        .bind(parsear_uuid(empresa_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.ok_or_else(|| ApiError::no_encontrado("Contrato", id))
    }
}

async fn registrar_evento(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: Uuid,
    contrato_id: Uuid,
    tipo: &str,
    monto_cents: i64,
    motivo: &str,
    creado_por: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO orders.contrato_eventos
           (empresa_id, contrato_id, tipo, monto_cents, motivo, creado_por)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(empresa_id)
    .bind(contrato_id)
    .bind(tipo)
    .bind(monto_cents)
    .bind(motivo)
    .bind(creado_por)
    .execute(&mut **tx)
    .await
    .map_err(error_db)?;
    Ok(())
}

fn centavos(fila: &PgRow, columna: &str) -> Result<i64, ApiError> {
    fila.try_get::<i32, _>(columna)
        .map(i64::from)
        .map_err(error_db)
}

fn contrato_de_fila(fila: &PgRow) -> Result<Contrato, ApiError> {
    let metadata: Option<Value> = fila.try_get("metadata").map_err(error_db)?;
    Ok(Contrato {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        parte_id: uuid_opcional(fila, "parte_id").map_err(error_db)?,
        item_id: uuid_opcional(fila, "item_id").map_err(error_db)?,
        tipo: fila.try_get("tipo").map_err(error_db)?,
        titulo: fila.try_get("titulo").map_err(error_db)?,
        descripcion: fila.try_get("descripcion").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        inicia_en: fecha_texto(fila, "inicia_en")?,
        termina_en: fecha_texto(fila, "termina_en")?,
        deposito: money::string(centavos(fila, "deposito_cents")?),
        garantia: money::string(centavos(fila, "garantia_cents")?),
        retenido: money::string(centavos(fila, "retenido_cents")?),
        moneda: fila.try_get("moneda").map_err(error_db)?,
        metadata: metadata
            .filter(|valor| !valor.is_null())
            .unwrap_or_else(|| json!({})),
        creado_por: uuid_texto(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
        finalizado_en: timestamp_opcional(fila, "finalizado_en").map_err(error_db)?,
    })
}

fn evento_de_fila(fila: &PgRow) -> Result<ContratoEvento, ApiError> {
    Ok(ContratoEvento {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        contrato_id: uuid_texto(fila, "contrato_id").map_err(error_db)?,
        tipo: fila.try_get("tipo").map_err(error_db)?,
        monto: money::string(centavos(fila, "monto_cents")?),
        motivo: fila.try_get("motivo").map_err(error_db)?,
        creado_por: uuid_texto(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
    })
}

fn fecha_texto(fila: &PgRow, columna: &str) -> Result<Option<String>, ApiError> {
    let fecha: Option<NaiveDate> = fila.try_get(columna).map_err(error_db)?;
    Ok(fecha.map(|valor| valor.format("%Y-%m-%d").to_string()))
}

fn validar_titulo(valor: &str) -> Result<String, ApiError> {
    let titulo = valor.trim();
    if !(2..=200).contains(&titulo.chars().count()) {
        return Err(ApiError::dominio(
            "TITULO_INVALIDO",
            "El titulo debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(titulo.to_string())
}

fn validar_tipo(valor: Option<&str>) -> Result<String, ApiError> {
    let tipo = valor.unwrap_or("otro");
    if !TIPOS_CONTRATO.contains(&tipo) {
        return Err(ApiError::dominio(
            "TIPO_CONTRATO_INVALIDO",
            "El tipo de contrato no es valido.",
        ));
    }
    Ok(tipo.to_string())
}

fn validar_moneda(valor: Option<&str>) -> Result<String, ApiError> {
    let moneda = valor.unwrap_or("NIO").trim().to_uppercase();
    if moneda.len() != 3 || !moneda.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(ApiError::dominio(
            "MONEDA_INVALIDA",
            "La moneda debe ser un codigo ISO de 3 letras.",
        ));
    }
    Ok(moneda)
}

fn validar_metadata(valor: Option<Value>) -> Result<Value, ApiError> {
    match valor {
        None | Some(Value::Null) => Ok(json!({})),
        Some(Value::Object(mapa)) => Ok(Value::Object(mapa)),
        Some(_) => Err(ApiError::dominio(
            "METADATA_INVALIDA",
            "La metadata debe ser un objeto.",
        )),
    }
}

fn validar_fechas(
    inicia: Option<&str>,
    termina: Option<&str>,
) -> Result<(Option<NaiveDate>, Option<NaiveDate>), ApiError> {
    let inicia = validar_fecha(inicia, "inicia_en")?;
    let termina = validar_fecha(termina, "termina_en")?;
    validar_rango(inicia, termina)?;
    Ok((inicia, termina))
}

fn validar_fecha(valor: Option<&str>, campo: &str) -> Result<Option<NaiveDate>, ApiError> {
    let Some(texto) = valor.filter(|texto| !texto.is_empty()) else {
        return Ok(None);
    };
    if !es_formato_fecha(texto) {
        return Err(ApiError::dominio(
            "FECHA_INVALIDA",
            format!("El {campo} debe ser YYYY-MM-DD."),
        ));
    }
    NaiveDate::parse_from_str(texto, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| {
            ApiError::dominio("FECHA_INVALIDA", format!("El {campo} debe ser YYYY-MM-DD."))
        })
}

fn es_formato_fecha(texto: &str) -> bool {
    let bytes = texto.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(indice, byte)| indice == 4 || indice == 7 || byte.is_ascii_digit())
}

fn validar_rango(inicia: Option<NaiveDate>, termina: Option<NaiveDate>) -> Result<(), ApiError> {
    if matches!((inicia, termina), (Some(inicio), Some(fin)) if fin < inicio) {
        return Err(ApiError::dominio(
            "FECHAS_INVALIDAS",
            "La fecha final no puede ser anterior.",
        ));
    }
    Ok(())
}

fn validar_transicion(actual: &str, nuevo: &str) -> Result<(), ApiError> {
    let permitidas: &[&str] = match actual {
        "borrador" => &["borrador", "activo", "cancelado"],
        "activo" => &["activo", "cancelado"],
        _ => &[],
    };
    if !ESTADOS_CONTRATO.contains(&nuevo) || !permitidas.contains(&nuevo) {
        return Err(ApiError::conflicto(format!(
            "No se puede pasar de {actual} a {nuevo}."
        )));
    }
    Ok(())
}

fn validar_uuid(valor: &str, campo: &str) -> Result<Uuid, ApiError> {
    parsear_uuid(valor)
        .map_err(|_| ApiError::dominio("ID_INVALIDO", format!("El {campo} debe ser un UUID.")))
}

fn validar_uuid_opcional(valor: Option<&str>, campo: &str) -> Result<Option<Uuid>, ApiError> {
    match valor.filter(|texto| !texto.is_empty()) {
        None => Ok(None),
        Some(texto) => validar_uuid(texto, campo).map(Some),
    }
}

fn recortar(valor: Option<&str>, maximo: usize) -> String {
    valor
        .unwrap_or_default()
        .trim()
        .chars()
        .take(maximo)
        .collect()
}

fn monto(valor: Option<&str>) -> Result<i64, ApiError> {
    money::parsear(valor.unwrap_or("0"))
}

fn numero_de(valor: Option<&str>) -> Option<f64> {
    let numero = valor?.trim().parse::<f64>().ok()?;
    (!numero.is_nan()).then_some(numero)
}

fn limite_de(valor: Option<&str>) -> i64 {
    let numero = numero_de(valor).unwrap_or(0.0);
    let numero = if numero == 0.0 { 50.0 } else { numero };
    numero.clamp(1.0, 200.0) as i64
}

fn pagina_de(valor: Option<&str>) -> f64 {
    let numero = numero_de(valor).unwrap_or(0.0);
    let numero = if numero == 0.0 { 1.0 } else { numero };
    numero.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    const UUID_VALIDO: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn debe_normalizar_y_validar_el_titulo() {
        assert_eq!(validar_titulo("  Alquiler  ").expect("titulo"), "Alquiler");
        assert_eq!(validar_titulo("ab").expect("titulo"), "ab");
        assert_eq!(
            validar_titulo("x").expect_err("corto").codigo,
            "TITULO_INVALIDO"
        );
        assert_eq!(
            validar_titulo(&"a".repeat(201)).expect_err("largo").codigo,
            "TITULO_INVALIDO"
        );
        assert!(validar_titulo(&"a".repeat(200)).is_ok());
    }

    #[test]
    fn debe_validar_el_tipo_con_otro_por_defecto() {
        assert_eq!(validar_tipo(None).expect("tipo"), "otro");
        assert_eq!(validar_tipo(Some("alquiler")).expect("tipo"), "alquiler");
        assert_eq!(
            validar_tipo(Some("nave")).expect_err("invalido").codigo,
            "TIPO_CONTRATO_INVALIDO"
        );
    }

    #[test]
    fn debe_normalizar_y_validar_la_moneda() {
        assert_eq!(validar_moneda(None).expect("moneda"), "NIO");
        assert_eq!(validar_moneda(Some(" usd ")).expect("moneda"), "USD");
        assert_eq!(
            validar_moneda(Some("peso")).expect_err("invalida").codigo,
            "MONEDA_INVALIDA"
        );
        assert_eq!(
            validar_moneda(Some("NI")).expect_err("invalida").codigo,
            "MONEDA_INVALIDA"
        );
    }

    #[test]
    fn debe_validar_las_fechas_y_su_orden() {
        assert_eq!(validar_fecha(None, "inicia_en").expect("fecha"), None);
        assert_eq!(validar_fecha(Some(""), "inicia_en").expect("fecha"), None);
        assert_eq!(
            validar_fecha(Some("2026/12/01"), "inicia_en")
                .expect_err("formato")
                .codigo,
            "FECHA_INVALIDA"
        );
        assert_eq!(
            validar_fecha(Some("2026-02-30"), "inicia_en")
                .expect_err("calendario")
                .codigo,
            "FECHA_INVALIDA"
        );
        let (inicia, termina) =
            validar_fechas(Some("2026-12-01"), Some("2026-12-10")).expect("fechas");
        assert_eq!(inicia.expect("inicia").to_string(), "2026-12-01");
        assert_eq!(termina.expect("termina").to_string(), "2026-12-10");
        assert_eq!(
            validar_fechas(Some("2026-12-10"), Some("2026-12-01"))
                .expect_err("rango")
                .codigo,
            "FECHAS_INVALIDAS"
        );
    }

    #[test]
    fn debe_aceptar_solo_metadata_objeto() {
        assert_eq!(validar_metadata(None).expect("metadata"), json!({}));
        assert_eq!(
            validar_metadata(Some(Value::Null)).expect("metadata"),
            json!({})
        );
        let objeto = json!({"clave": "valor"});
        assert_eq!(
            validar_metadata(Some(objeto.clone())).expect("metadata"),
            objeto
        );
        assert_eq!(
            validar_metadata(Some(json!([1, 2])))
                .expect_err("invalida")
                .codigo,
            "METADATA_INVALIDA"
        );
    }

    #[test]
    fn debe_validar_las_transiciones_de_estado() {
        assert!(validar_transicion("borrador", "borrador").is_ok());
        assert!(validar_transicion("borrador", "activo").is_ok());
        assert!(validar_transicion("borrador", "cancelado").is_ok());
        assert!(validar_transicion("activo", "activo").is_ok());
        assert!(validar_transicion("activo", "cancelado").is_ok());
        let error = validar_transicion("activo", "borrador").expect_err("invalida");
        assert_eq!(error.codigo, "CONFLICTO");
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.mensaje, "No se puede pasar de activo a borrador.");
        assert!(validar_transicion("finalizado", "activo").is_err());
        assert!(validar_transicion("cancelado", "activo").is_err());
        assert!(validar_transicion("borrador", "desconocido").is_err());
    }

    #[test]
    fn debe_parsear_montos_con_cero_por_defecto() {
        assert_eq!(monto(None).expect("monto"), 0);
        assert_eq!(monto(Some("500.00")).expect("monto"), 50_000);
        assert_eq!(
            monto(Some("abc")).expect_err("invalido").codigo,
            "MONTO_INVALIDO"
        );
        assert_eq!(money::string(50_000), "500.00");
        assert_eq!(money::string(40_000), "400.00");
    }

    #[test]
    fn debe_aplicar_la_paginacion_de_javascript() {
        assert_eq!(limite_de(None), 50);
        assert_eq!(limite_de(Some("10")), 10);
        assert_eq!(limite_de(Some("0")), 50);
        assert_eq!(limite_de(Some("-5")), 1);
        assert_eq!(limite_de(Some("500")), 200);
        assert_eq!(limite_de(Some("abc")), 50);
        assert_eq!(pagina_de(None), 1.0);
        assert_eq!(pagina_de(Some("3")), 3.0);
        assert_eq!(pagina_de(Some("0")), 1.0);
        assert_eq!(pagina_de(Some("-2")), 1.0);
    }

    #[test]
    fn debe_validar_identificadores_opcionales() {
        assert_eq!(validar_uuid_opcional(None, "orden").expect("uuid"), None);
        assert_eq!(
            validar_uuid_opcional(Some(""), "orden").expect("uuid"),
            None
        );
        assert_eq!(
            validar_uuid_opcional(Some(UUID_VALIDO), "orden")
                .expect("uuid")
                .expect("uuid")
                .to_string(),
            UUID_VALIDO
        );
        assert_eq!(
            validar_uuid_opcional(Some("no-uuid"), "orden")
                .expect_err("invalido")
                .codigo,
            "ID_INVALIDO"
        );
    }

    #[test]
    fn debe_recortar_descripciones_y_motivos() {
        assert_eq!(recortar(None, 2000), "");
        assert_eq!(recortar(Some("  texto  "), 2000), "texto");
        assert_eq!(recortar(Some("123456"), 3), "123");
    }
}
