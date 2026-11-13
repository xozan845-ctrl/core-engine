//! Serial/IMEI por unidad (Fase P1.3, ADR-0001).
//!
//! Réplica de `items/series.service.ts`. Un item con `track_serial` mantiene
//! una fila por unidad física. La venta la marca como vendida y calcula
//! `garantia_hasta` con los meses del item; la devolución la marca como
//! devuelta. El stock agregado sigue siendo la autoridad de disponibilidad.
//!
//! `asignar`, `liberar` y `devolver_en_transaccion` concentran las mutaciones
//! que el consumidor de pedidos comparte con la reserva/devolución de stock
//! (`productos.service.ts`): reciben la misma `Transaction` para que stock,
//! series y Outbox sean atómicos.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

const ESTADOS_SERIE: [&str; 5] = ["disponible", "reservado", "vendido", "devuelto", "baja"];
/// Estados que un operador puede fijar manualmente.
const ESTADOS_MANUALES: [&str; 2] = ["disponible", "baja"];

const COLUMNAS: &str = "id, empresa_id, item_id, ubicacion_id, serial, imei, estado,
  order_id, vendido_en, garantia_hasta::text AS garantia_hasta, devuelto_en, metadata,
  creado_en, actualizado_en";

const MENSAJE_CONFLICTO: &str = "El serial o IMEI ya existe en esta empresa.";

pub struct Series {
    db: Arc<Db>,
}

impl Series {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.item_series
              WHERE empresa_id = $1 AND item_id = $2
              ORDER BY estado, creado_en"
        ))
        .bind(empresa_id)
        .bind(item_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        item_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        if !item.track_serial {
            return Err(ApiError::dominio(
                "SERIALIZACION_NO_ACTIVA",
                "Activa la serializacion del item antes de registrar unidades.",
            ));
        }
        let serial = validar_serial(&texto_o_vacio(input.get("serial")))?;
        let imei = validar_imei(&texto_o_vacio(input.get("imei")))?;
        let ubicacion_id = self
            .validar_ubicacion(empresa_id, input.get("ubicacion_id"))
            .await?;
        let metadata = metadata_entrada(input);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "INSERT INTO catalog.item_series
                   (empresa_id, item_id, ubicacion_id, serial, imei, metadata)
                 VALUES ($1, $2, $3, $4, $5, $6::jsonb)
                 RETURNING {COLUMNAS}"
            ))
            .bind(empresa_id)
            .bind(item_id)
            .bind(ubicacion_id)
            .bind(&serial)
            .bind(&imei)
            .bind(metadata.to_string())
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::dominio(
                    "SERIE_NO_CREADA",
                    "No se pudo registrar la unidad.",
                ))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error))
            }
        }
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        serie_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar(empresa_id, serie_id).await?;

        let serial = match input.get("serial") {
            Some(valor) => validar_serial(&texto_js(valor))?,
            None => texto_de(&actual, "serial"),
        };
        let imei = match input.get("imei") {
            Some(valor) => validar_imei(&texto_js(valor))?,
            None => texto_de(&actual, "imei"),
        };
        let ubicacion_id = match input.get("ubicacion_id") {
            Some(valor) => self.validar_ubicacion(empresa_id, Some(valor)).await?,
            None => actual
                .get("ubicacion_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        let estado = match input.get("estado") {
            Some(valor) => validar_estado_manual(&texto_js(valor))?,
            None => texto_de(&actual, "estado"),
        };
        let metadata = match input.get("metadata") {
            Some(valor) => valor.clone(),
            None => actual.get("metadata").cloned().unwrap_or_else(|| json!({})),
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "UPDATE catalog.item_series
                    SET serial = $1, imei = $2, ubicacion_id = $3, estado = $4,
                        metadata = $5::jsonb,
                        devuelto_en = CASE WHEN $4 = 'disponible' THEN NULL ELSE devuelto_en END,
                        actualizado_en = NOW()
                  WHERE id = $6 AND empresa_id = $7
                  RETURNING {COLUMNAS}"
            ))
            .bind(&serial)
            .bind(&imei)
            .bind(ubicacion_id)
            .bind(&estado)
            .bind(metadata.to_string())
            .bind(serie_id)
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::no_encontrado("Serie", serie_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error))
            }
        }
    }

    pub async fn buscar_por_serial(
        &self,
        empresa_id: &str,
        serial: &Value,
    ) -> Result<Value, ApiError> {
        let serial = validar_serial(&texto_o_vacio(Some(serial)))?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT s.id, s.empresa_id, s.item_id, s.ubicacion_id, s.serial, s.imei,
                    s.estado, s.order_id, s.vendido_en,
                    s.garantia_hasta::text AS garantia_hasta, s.devuelto_en,
                    s.metadata, s.creado_en, s.actualizado_en,
                    i.codigo AS sku, i.nombre AS nombre_item, i.garantia_meses,
                    (s.garantia_hasta IS NOT NULL AND s.garantia_hasta >= CURRENT_DATE)
                      AS garantia_vigente
               FROM catalog.item_series s
               JOIN catalog.items i ON i.id = s.item_id
              WHERE s.empresa_id = $1 AND lower(s.serial) = lower($2)",
        )
        .bind(empresa_id)
        .bind(&serial)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Serie", &serial)),
        }
    }

    pub async fn habilitar_serializacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        activo: bool,
    ) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        if activo && (item.tipo != "producto" || !item.track_inventory) {
            return Err(ApiError::dominio(
                "SERIALIZACION_NO_APLICA",
                "Solo los productos con inventario pueden serializarse.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.items
                SET track_serial = $1, actualizado_en = NOW()
              WHERE id = $2 AND empresa_id = $3
              RETURNING id AS item_id, track_serial",
        )
        .bind(activo)
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Item", item_id)),
        }
    }

    /// Marca como vendidas `cantidad` unidades disponibles de cada línea y
    /// calcula la garantía con `garantia_meses` del item.
    ///
    /// `items` es el arreglo de líneas `{empresa_id, sku, cantidad, item_id?}`
    /// (el `item_id` es opcional, igual que en la reserva de stock). Si una
    /// línea serializada no tiene unidades suficientes se devuelve
    /// `SERIAL_INSUFICIENTE` para que el llamador revierta la transacción.
    pub async fn asignar(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = items.as_array().ok_or_else(|| {
            ApiError::dominio("ITEMS_INVALIDOS", "Las lineas deben ser un arreglo.")
        })?;
        let mut asignadas: i64 = 0;
        for linea in lineas {
            let empresa_id = texto_de(linea, "empresa_id");
            let sku = texto_de(linea, "sku");
            let cantidad = validar_cantidad(item_cantidad(linea))?;
            let item_id = linea
                .get("item_id")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string);
            let item = self
                .item_para_serie(tx, &empresa_id, item_id.as_deref(), &sku)
                .await?;
            let Some(item) = item else {
                return Err(ApiError::no_encontrado(
                    "Item",
                    item_id.as_deref().unwrap_or(&sku),
                ));
            };
            if !item.track_serial {
                continue;
            }

            let filas = sqlx::query(
                "SELECT s.id
                   FROM catalog.item_series s
                   LEFT JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
                  WHERE s.empresa_id = $1 AND s.item_id = $2 AND s.estado = 'disponible'
                  ORDER BY (u.predeterminada IS TRUE) DESC, s.creado_en, s.id
                  LIMIT $3
                  FOR UPDATE OF s",
            )
            .bind(&empresa_id)
            .bind(&item.id)
            .bind(cantidad)
            .fetch_all(&mut **tx)
            .await
            .map_err(interno)?;
            if (filas.len() as i64) < cantidad {
                return Err(ApiError::dominio(
                    "SERIAL_INSUFICIENTE",
                    "No hay suficientes unidades serializadas disponibles.",
                )
                .con_detalles(json!({
                    "item_id": item.id,
                    "sku": sku,
                    "requeridas": cantidad,
                    "disponibles": filas.len(),
                })));
            }
            let ids = filas
                .iter()
                .map(|fila| fila.get::<Uuid, _>("id"))
                .collect::<Vec<_>>();

            sqlx::query(
                "UPDATE catalog.item_series
                    SET estado = 'vendido', order_id = $1, vendido_en = NOW(),
                        ubicacion_id = NULL,
                        garantia_hasta = CASE WHEN $3::int IS NULL THEN NULL
                          ELSE (CURRENT_DATE + ($3::int || ' months')::interval)::date END,
                        actualizado_en = NOW()
                  WHERE id = ANY($2::uuid[])",
            )
            .bind(order_id)
            .bind(ids.as_slice())
            .bind(item.garantia_meses)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
            asignadas += ids.len() as i64;
        }
        Ok(json!({ "ok": true, "asignadas": asignadas }))
    }

    /// Libera las unidades que una orden tenía asignadas (por ejemplo al
    /// cancelar antes de la entrega): vuelven a `disponible` y a la ubicación
    /// predeterminada de la empresa.
    pub async fn liberar(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
    ) -> Result<Value, ApiError> {
        let resultado = sqlx::query(
            "UPDATE catalog.item_series s
                SET estado = 'disponible', order_id = NULL, vendido_en = NULL,
                    garantia_hasta = NULL, devuelto_en = NULL,
                    ubicacion_id = COALESCE((
                      SELECT u.id FROM catalog.ubicaciones u
                       WHERE u.empresa_id = s.empresa_id AND u.predeterminada AND u.activo
                       LIMIT 1
                    ), s.ubicacion_id),
                    actualizado_en = NOW()
              WHERE s.order_id = $1 AND s.estado IN ('vendido', 'reservado')",
        )
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        Ok(json!({ "ok": true, "liberadas": resultado.rows_affected() as i64 }))
    }

    /// Marca como devueltas las unidades vendidas de una orden. Toma primero
    /// las más antiguas y restaura la ubicación predeterminada. Se ejecuta en
    /// la misma transacción que el reintegro de stock (RN-06).
    pub async fn devolver_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = items.as_array().ok_or_else(|| {
            ApiError::dominio("ITEMS_INVALIDOS", "Las lineas deben ser un arreglo.")
        })?;
        for item in lineas {
            let empresa_id = texto_de(item, "empresa_id");
            let sku = texto_de(item, "sku");
            let cantidad = validar_cantidad(item_cantidad(item))?;
            sqlx::query(
                "WITH objetivo AS (
                   SELECT s.id
                     FROM catalog.item_series s
                     JOIN catalog.items i ON i.id = s.item_id
                    WHERE s.order_id = $1 AND s.estado = 'vendido'
                      AND i.empresa_id = $2 AND LOWER(i.codigo) = LOWER($3)
                    ORDER BY s.vendido_en, s.id
                    LIMIT $4
                    FOR UPDATE OF s
                 )
                 UPDATE catalog.item_series s
                    SET estado = 'devuelto', devuelto_en = NOW(),
                        ubicacion_id = COALESCE((
                          SELECT u.id FROM catalog.ubicaciones u
                           WHERE u.empresa_id = $2 AND u.predeterminada AND u.activo
                           LIMIT 1
                        ), s.ubicacion_id),
                        actualizado_en = NOW()
                   FROM objetivo o
                  WHERE s.id = o.id",
            )
            .bind(order_id)
            .bind(&empresa_id)
            .bind(&sku)
            .bind(cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        Ok(json!({ "ok": true }))
    }

    async fn buscar(&self, empresa_id: &str, serie_id: &str) -> Result<Value, ApiError> {
        let id = validar_uuid(serie_id, "serie")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.item_series
              WHERE id = $1 AND empresa_id = $2"
        ))
        .bind(&id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Serie", &id)),
        }
    }

    async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemSerializado, ApiError> {
        let id = validar_uuid(item_id, "item")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, tipo, track_inventory, track_serial, codigo, nombre, garantia_meses
               FROM catalog.items
              WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(&id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::no_encontrado("Item", &id))?;
        let valor = fila_json(&fila)?;
        Ok(ItemSerializado {
            tipo: texto_de(&valor, "tipo"),
            track_inventory: valor
                .get("track_inventory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            track_serial: valor
                .get("track_serial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    async fn validar_ubicacion(
        &self,
        empresa_id: &str,
        ubicacion_id: Option<&Value>,
    ) -> Result<Option<String>, ApiError> {
        let Some(valor) = ubicacion_id else {
            return Ok(None);
        };
        if valor.is_null() {
            return Ok(None);
        }
        let ubicacion = texto_js(valor);
        if ubicacion.is_empty() {
            return Ok(None);
        }
        let id = validar_uuid(&ubicacion, "ubicacion")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id FROM catalog.ubicaciones WHERE id = $1 AND empresa_id = $2 AND activo",
        )
        .bind(&id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(
                fila.try_get::<Uuid, _>("id").map_err(interno)?.to_string(),
            )),
            None => Err(ApiError::no_encontrado("Ubicacion", &id)),
        }
    }

    async fn item_para_serie(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        empresa_id: &str,
        item_id: Option<&str>,
        sku: &str,
    ) -> Result<Option<ItemSerializadoCompleto>, ApiError> {
        let fila = match item_id {
            Some(id) => sqlx::query(
                "SELECT id, track_serial, garantia_meses
                       FROM catalog.items
                      WHERE id = $1 AND empresa_id = $2
                        AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(id)
            .bind(empresa_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id, track_serial, garantia_meses
                       FROM catalog.items
                      WHERE empresa_id = $1 AND LOWER(codigo) = LOWER($2)
                        AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(sku)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?,
        };
        let Some(fila) = fila else {
            return Ok(None);
        };
        Ok(Some(ItemSerializadoCompleto {
            id: fila.try_get::<Uuid, _>("id").map_err(interno)?.to_string(),
            track_serial: fila.try_get("track_serial").map_err(interno)?,
            garantia_meses: fila
                .try_get::<Option<i32>, _>("garantia_meses")
                .map_err(interno)?,
        }))
    }
}

struct ItemSerializado {
    tipo: String,
    track_inventory: bool,
    track_serial: bool,
}

struct ItemSerializadoCompleto {
    id: String,
    track_serial: bool,
    garantia_meses: Option<i32>,
}

fn filas_json(filas: &[PgRow]) -> Result<Value, ApiError> {
    let valores = filas
        .iter()
        .map(fila_a_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(interno)?;
    Ok(Value::Array(valores))
}

fn fila_json(fila: &PgRow) -> Result<Value, ApiError> {
    fila_a_json(fila).map_err(interno)
}

fn error_conflicto(error: sqlx::Error) -> ApiError {
    if es_conflicto(&error) {
        ApiError::conflicto(MENSAJE_CONFLICTO)
    } else {
        interno(error)
    }
}

fn es_conflicto(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|base| base.code())
        .is_some_and(|codigo| codigo == "23505" || codigo == "23P01")
}

fn texto_de(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_o_vacio(valor: Option<&Value>) -> String {
    match valor {
        None | Some(Value::Null) => String::new(),
        Some(valor) => texto_js(valor),
    }
}

fn texto_js(valor: &Value) -> String {
    match valor {
        Value::Null => "null".to_string(),
        Value::Bool(valor) => valor.to_string(),
        Value::Number(valor) => valor.to_string(),
        Value::String(valor) => valor.clone(),
        Value::Array(valores) => valores.iter().map(texto_js).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn item_cantidad(linea: &Value) -> &Value {
    linea.get("cantidad").unwrap_or(&Value::Null)
}

fn validar_cantidad(valor: &Value) -> Result<i64, ApiError> {
    let cantidad = numero_entero(valor).filter(|valor| *valor >= 1);
    cantidad.ok_or_else(|| {
        ApiError::dominio(
            "CANTIDAD_INVALIDA",
            "La cantidad debe ser un entero positivo.",
        )
    })
}

fn numero_entero(valor: &Value) -> Option<i64> {
    let numero = match valor {
        Value::Number(numero) => numero.as_f64()?,
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                0.0
            } else {
                texto.parse::<f64>().ok()?
            }
        }
        Value::Bool(valor) => {
            if *valor {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        _ => return None,
    };
    if !numero.is_finite() || numero.fract() != 0.0 {
        return None;
    }
    if numero < i64::MIN as f64 || numero > i64::MAX as f64 {
        return None;
    }
    Some(numero as i64)
}

fn validar_serial(valor: &str) -> Result<String, ApiError> {
    let serial = valor.trim();
    let valido = (1..=120).contains(&serial.len())
        && serial.chars().all(|caracter| {
            caracter.is_ascii_alphanumeric() || matches!(caracter, '.' | '_' | '-')
        });
    if !valido {
        return Err(ApiError::dominio(
            "SERIAL_INVALIDO",
            "El serial debe tener entre 1 y 120 caracteres alfanumericos, punto, guion o guion bajo.",
        ));
    }
    Ok(serial.to_string())
}

fn validar_imei(valor: &str) -> Result<String, ApiError> {
    let imei = valor.trim();
    if imei.len() > 20 || !imei.chars().all(|caracter| caracter.is_ascii_digit()) {
        return Err(ApiError::dominio(
            "IMEI_INVALIDO",
            "El IMEI debe contener solo digitos (maximo 20).",
        ));
    }
    Ok(imei.to_string())
}

fn validar_estado_manual(valor: &str) -> Result<String, ApiError> {
    let estado = valor.trim();
    if !ESTADOS_SERIE.contains(&estado) || !ESTADOS_MANUALES.contains(&estado) {
        return Err(ApiError::dominio(
            "ESTADO_SERIE_INVALIDO",
            "Solo se puede fijar manualmente el estado disponible o baja.",
        ));
    }
    Ok(estado.to_string())
}

fn validar_uuid(valor: &str, campo: &str) -> Result<String, ApiError> {
    if !es_uuid(valor) {
        return Err(ApiError::dominio(
            "ID_INVALIDO",
            format!("El {campo} debe ser un UUID."),
        ));
    }
    Ok(valor.to_string())
}

fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(indice, byte)| {
        if matches!(indice, 8 | 13 | 18 | 23) {
            *byte == b'-'
        } else {
            byte.is_ascii_hexdigit()
        }
    })
}

fn metadata_entrada(input: &Value) -> Value {
    input
        .get("metadata")
        .filter(|valor| !valor.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn debe_validar_seriales_e_imei() {
        assert_eq!(
            validar_serial(" SN-2026.0001 ").expect("serial"),
            "SN-2026.0001"
        );
        assert_eq!(
            validar_serial("").expect_err("vacio").codigo,
            "SERIAL_INVALIDO"
        );
        assert_eq!(
            validar_serial(&"x".repeat(121)).expect_err("largo").codigo,
            "SERIAL_INVALIDO"
        );
        assert_eq!(
            validar_serial("SN/1").expect_err("simbolo").codigo,
            "SERIAL_INVALIDO"
        );
        assert_eq!(validar_imei("").expect("imei"), "");
        assert_eq!(
            validar_imei("356789012345678").expect("imei"),
            "356789012345678"
        );
        assert_eq!(
            validar_imei("356789012345678901234")
                .expect_err("largo")
                .codigo,
            "IMEI_INVALIDO"
        );
        assert_eq!(
            validar_imei("12a").expect_err("letra").codigo,
            "IMEI_INVALIDO"
        );
    }

    #[test]
    fn debe_permitir_solo_los_estados_manuales() {
        assert_eq!(
            validar_estado_manual(" disponible ").expect("estado"),
            "disponible"
        );
        assert_eq!(validar_estado_manual("baja").expect("estado"), "baja");
        assert_eq!(
            validar_estado_manual("vendido")
                .expect_err("vendido")
                .codigo,
            "ESTADO_SERIE_INVALIDO"
        );
        assert_eq!(
            validar_estado_manual("otro").expect_err("otro").codigo,
            "ESTADO_SERIE_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_uuid_con_el_regex_del_contrato() {
        assert!(es_uuid("11111111-1111-4111-8111-111111111111"));
        assert!(!es_uuid("11111111111141118111111111111111"));
        assert!(!es_uuid("11111111-1111-4111-8111-11111111111z"));
    }

    #[test]
    fn debe_usar_metadata_vacia_cuando_no_se_envia() {
        assert_eq!(metadata_entrada(&json!({})), json!({}));
        assert_eq!(metadata_entrada(&json!({"metadata": null})), json!({}));
        assert_eq!(
            metadata_entrada(&json!({"metadata": {"color": "rojo"}})),
            json!({"color": "rojo"})
        );
    }

    #[test]
    fn debe_calcular_la_vigencia_de_la_garantia() {
        fn vigente(garantia: Option<NaiveDate>, hoy: NaiveDate) -> bool {
            garantia.is_some_and(|fecha| fecha >= hoy)
        }
        let hoy = NaiveDate::parse_from_str("2026-11-13", "%Y-%m-%d").expect("hoy");
        assert!(vigente(
            Some(NaiveDate::parse_from_str("2026-11-13", "%Y-%m-%d").expect("fecha")),
            hoy
        ));
        assert!(!vigente(
            Some(NaiveDate::parse_from_str("2026-11-12", "%Y-%m-%d").expect("fecha")),
            hoy
        ));
        assert!(!vigente(None, hoy));
    }
}
