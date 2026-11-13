//! Inventario: configuración, niveles de stock, proveedores y lotes.
//!
//! Réplica de `inventario.service.ts` (niveles H-06, proveedores y lotes con
//! consumo FEFO) y de la configuración de inventario de
//! `operaciones.service.ts`. Los montos se serializan como `"1150.00"` y las
//! fechas `date` como medianoche local en ISO 8601 con milisegundos, igual que
//! `node-postgres`.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use chrono::{Local, NaiveDate, TimeZone};
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// Métodos de costeo admitidos por `catalog.configuracion_inventario`.
pub const METODOS_COSTEO: [&str; 3] = ["promedio", "fifo", "ueps"];

pub struct Inventario {
    db: Arc<Db>,
}

/// Lote candidato para el consumo FEFO (ya ordenado por la consulta).
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoteDisponible {
    id: String,
    codigo_lote: String,
    cantidad_disponible: i64,
    vencido: bool,
}

/// Consumo calculado para un lote.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Consumo {
    lote_id: String,
    codigo_lote: String,
    cantidad: i64,
}

fn texto(datos: &Value, clave: &str) -> String {
    datos
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `Number.isInteger(valor)`: acepta enteros y decimales exactos (`1.0`).
fn entero(valor: &Value) -> Option<i64> {
    valor.as_i64().or_else(|| {
        valor
            .as_f64()
            .filter(|numero| numero.is_finite() && numero.fract() == 0.0)
            .and_then(|numero| {
                if numero >= i64::MIN as f64 && numero <= i64::MAX as f64 {
                    Some(numero as i64)
                } else {
                    None
                }
            })
    })
}

/// `date` de PostgreSQL: `node-postgres` la entrega como medianoche local y
/// `JSON.stringify` la convierte a ISO UTC con milisegundos.
fn fecha_iso(fecha: NaiveDate) -> String {
    match fecha.and_hms_opt(0, 0, 0).and_then(|medianoche| {
        Local
            .from_local_datetime(&medianoche)
            .single()
            .map(|valor| valor.with_timezone(&chrono::Utc))
    }) {
        Some(valor) => crate::db::iso_ms(valor),
        None => format!("{}T00:00:00.000Z", fecha.format("%Y-%m-%d")),
    }
}

fn lote_a_json(fila: &PgRow) -> Result<Value, ApiError> {
    let mut valor = fila_a_json(fila).map_err(interno)?;
    let fecha: Option<NaiveDate> = fila.try_get("vence_en").map_err(interno)?;
    if let Some(objeto) = valor.as_object_mut() {
        objeto.insert(
            "vence_en".into(),
            fecha
                .map(|fecha| Value::String(fecha_iso(fecha)))
                .unwrap_or(Value::Null),
        );
    }
    Ok(valor)
}

/// Reparte `cantidad` entre los lotes vigentes en el orden recibido (FEFO).
/// Devuelve la cantidad que falta cuando el total no alcanza.
fn plan_consumo(lotes: &[LoteDisponible], cantidad: i64) -> Result<Vec<Consumo>, i64> {
    let mut restante = cantidad;
    let mut plan = Vec::new();
    for lote in lotes.iter().filter(|lote| !lote.vencido) {
        if restante == 0 {
            break;
        }
        let consumir = restante.min(lote.cantidad_disponible);
        plan.push(Consumo {
            lote_id: lote.id.clone(),
            codigo_lote: lote.codigo_lote.clone(),
            cantidad: consumir,
        });
        restante -= consumir;
    }
    if restante > 0 {
        Err(restante)
    } else {
        Ok(plan)
    }
}

fn booleano_configuracion(cambios: &Value, clave: &str, actual: bool) -> bool {
    match cambios.get(clave) {
        None | Some(Value::Null) => actual,
        Some(valor) => valor.as_bool().unwrap_or(actual),
    }
}

impl Inventario {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Configuración de inventario de la empresa o los valores por defecto.
    pub async fn configuracion(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                    actualizar_costo_en_compra, unidad_defecto, actualizado_en
               FROM catalog.configuracion_inventario WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Ok(json!({
                "empresa_id": empresa_id,
                "metodo_costeo": "promedio",
                "permitir_stock_negativo": false,
                "alerta_stock_bajo": true,
                "actualizar_costo_en_compra": true,
                "unidad_defecto": "Unidad (u)",
                "actualizado_en": "1970-01-01T00:00:00.000Z",
            })),
        }
    }

    /// Upsert de la configuración: cada campo ausente conserva el valor actual.
    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        cambios: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.configuracion(empresa_id).await?;
        let actual_metodo = actual
            .get("metodo_costeo")
            .and_then(Value::as_str)
            .unwrap_or("promedio")
            .to_string();
        let metodo = match cambios.get("metodo_costeo") {
            None | Some(Value::Null) => actual_metodo,
            Some(Value::String(valor)) => valor.clone(),
            Some(_) => String::new(),
        };
        if !METODOS_COSTEO.contains(&metodo.as_str()) {
            return Err(ApiError::dominio(
                "METODO_COSTEO_INVALIDO",
                "El método de costeo no es válido.",
            ));
        }
        let permitir_stock_negativo = booleano_configuracion(
            cambios,
            "permitir_stock_negativo",
            actual
                .get("permitir_stock_negativo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
        let alerta_stock_bajo = booleano_configuracion(
            cambios,
            "alerta_stock_bajo",
            actual
                .get("alerta_stock_bajo")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
        let actualizar_costo_en_compra = booleano_configuracion(
            cambios,
            "actualizar_costo_en_compra",
            actual
                .get("actualizar_costo_en_compra")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
        let unidad_actual = actual
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .unwrap_or("Unidad (u)")
            .trim()
            .to_string();
        let unidad = match cambios.get("unidad_defecto") {
            None | Some(Value::Null) => unidad_actual,
            Some(Value::String(valor)) => valor.trim().to_string(),
            Some(_) => unidad_actual,
        };
        let unidad_defecto = if unidad.is_empty() {
            "Unidad (u)".to_string()
        } else {
            unidad
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO catalog.configuracion_inventario
               (empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                actualizar_costo_en_compra, unidad_defecto)
             VALUES ($1::uuid, $2, $3, $4, $5, $6)
             ON CONFLICT (empresa_id) DO UPDATE SET
               metodo_costeo = EXCLUDED.metodo_costeo,
               permitir_stock_negativo = EXCLUDED.permitir_stock_negativo,
               alerta_stock_bajo = EXCLUDED.alerta_stock_bajo,
               actualizar_costo_en_compra = EXCLUDED.actualizar_costo_en_compra,
               unidad_defecto = EXCLUDED.unidad_defecto,
               actualizado_en = NOW()
             RETURNING empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                       actualizar_costo_en_compra, unidad_defecto, actualizado_en",
        )
        .bind(empresa_id)
        .bind(&metodo)
        .bind(permitir_stock_negativo)
        .bind(alerta_stock_bajo)
        .bind(actualizar_costo_en_compra)
        .bind(&unidad_defecto)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Err(ApiError::dominio(
                "CONFIGURACION_NO_GUARDADA",
                "No se pudo guardar la configuración.",
            )),
        }
    }

    /// Niveles de stock de la bodega desde la proyección CQRS (H-06).
    pub async fn listar(&self, empresa_id: Option<&str>) -> Result<Vec<Value>, ApiError> {
        let mut condiciones = vec![
            "i.tipo = 'producto'".to_string(),
            "i.eliminado_en IS NULL".to_string(),
            "NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)"
                .to_string(),
        ];
        let mut params: Vec<String> = Vec::new();
        if let Some(empresa) = empresa_id.filter(|valor| !valor.is_empty()) {
            params.push(empresa.to_string());
            condiciones.insert(0, "i.empresa_id = $1::uuid".to_string());
        }
        let filtro = format!("WHERE {}", condiciones.join(" AND "));
        let sql = format!(
            "WITH reservas AS (
               SELECT COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000') AS empresa_id,
                      LOWER(item->>'sku') AS sku,
                      SUM((item->>'cantidad')::int) AS reservado
                 FROM catalog.reservas_ordenes ro,
                      jsonb_array_elements(ro.items_json) AS item
                WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
                GROUP BY 1, 2
             )
             SELECT i.empresa_id, i.codigo AS sku, i.nombre,
                    i.stock + COALESCE(r.reservado, 0) AS stock_fisico,
                    COALESCE(r.reservado, 0)::int AS reservado,
                    i.stock AS disponible,
                    CASE WHEN i.stock > 0 THEN 'disponible' ELSE 'agotado' END AS estado,
                    i.precio_cents AS precio_base_cents,
                    (i.stock * i.precio_cents)::int AS valor_disponible_cents,
                    ((i.stock + COALESCE(r.reservado, 0)) * i.precio_cents)::int AS valor_total_cents
               FROM catalog.items i
               LEFT JOIN reservas r
                 ON r.empresa_id = i.empresa_id::text AND r.sku = LOWER(i.codigo)
              {filtro}
              ORDER BY estado DESC, i.nombre ASC"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let mut lineas = Vec::with_capacity(filas.len());
        for fila in &filas {
            let valor = fila_a_json(fila).map_err(interno)?;
            let precio = valor
                .get("precio_base_cents")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let mut linea = Map::new();
            linea.insert(
                "empresa_id".into(),
                valor.get("empresa_id").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "sku".into(),
                valor.get("sku").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "nombre".into(),
                valor.get("nombre").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "stock".into(),
                valor.get("stock_fisico").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "disponible".into(),
                valor.get("disponible").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "reservado".into(),
                valor.get("reservado").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "estado".into(),
                valor.get("estado").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "precio_base".into(),
                Value::String(Money::desde_centavos(precio)?.string()),
            );
            linea.insert(
                "valor_disponible_cents".into(),
                valor
                    .get("valor_disponible_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            linea.insert(
                "valor_total_cents".into(),
                valor
                    .get("valor_total_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            lineas.push(Value::Object(linea));
        }
        Ok(lineas)
    }

    /// Totales del inventario para las rutas internas.
    pub async fn resumen(&self, empresa_id: Option<&str>) -> Result<Value, ApiError> {
        let filtro = if empresa_id.is_some_and(|valor| !valor.is_empty()) {
            "WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto' AND i.eliminado_en IS NULL
               AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)"
        } else {
            "WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL
               AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)"
        };
        let sql = format!(
            "SELECT COUNT(*)::int AS productos,
                    COUNT(*) FILTER (WHERE i.stock = 0)::int AS agotados,
                    COALESCE(SUM(i.stock), 0)::int AS stock_total,
                    COALESCE(SUM(i.stock * i.precio_cents), 0)::int AS valor_inventario_cents
               FROM catalog.items i {filtro}"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&sql);
        let fila = match empresa_id.filter(|valor| !valor.is_empty()) {
            Some(empresa) => fila.bind(empresa),
            None => fila,
        }
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Ok(json!({
                "productos": 0,
                "agotados": 0,
                "stock_total": 0,
                "valor_inventario_cents": 0,
            })),
        }
    }

    pub async fn crear_proveedor(&self, datos: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO catalog.proveedores (empresa_id, codigo, nombre, correo, telefono)
             VALUES ($1::uuid, UPPER($2), $3, $4, $5)
             RETURNING id, empresa_id, codigo, nombre, correo, telefono, activo",
        )
        .bind(texto(datos, "empresa_id"))
        .bind(texto(datos, "codigo"))
        .bind(texto(datos, "nombre"))
        .bind(texto(datos, "correo"))
        .bind(texto(datos, "telefono"))
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Err(ApiError::dominio(
                "PROVEEDOR_NO_CREADO",
                "No se pudo crear el proveedor.",
            )),
        }
    }

    pub async fn crear_lote(&self, datos: &Value) -> Result<Value, ApiError> {
        let empresa_id = texto(datos, "empresa_id");
        let item_id = texto(datos, "item_id");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let item = sqlx::query(
            "SELECT track_inventory FROM catalog.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activo'",
        )
        .bind(&item_id)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(item) = item else {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        };
        if !item.get::<bool, _>("track_inventory") {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "Los lotes solo aplican a items con inventario.",
            ));
        }
        let cantidad = datos
            .get("cantidad_inicial")
            .and_then(entero)
            .filter(|valor| *valor >= 0)
            .ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad inicial debe ser un entero mayor o igual a cero.",
                )
            })?;
        let cantidad = i32::try_from(cantidad).map_err(|_| ApiError::interno())?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO catalog.lotes_inventario
               (empresa_id, item_id, proveedor_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5::date, $6, $6)
             RETURNING id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
                       cantidad_inicial, cantidad_disponible",
        )
        .bind(&empresa_id)
        .bind(&item_id)
        .bind(datos.get("proveedor_id").and_then(Value::as_str))
        .bind(texto(datos, "codigo_lote"))
        .bind(datos.get("vence_en").and_then(Value::as_str))
        .bind(cantidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => lote_a_json(&fila),
            None => Err(ApiError::dominio(
                "LOTE_NO_CREADO",
                "No se pudo crear el lote.",
            )),
        }
    }

    /// Consume cantidades aplicando FEFO (primero vence, primero sale).
    pub async fn consumir_lote(&self, datos: &Value) -> Result<Value, ApiError> {
        let cantidad = datos
            .get("cantidad")
            .and_then(entero)
            .filter(|valor| *valor > 0)
            .ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad a consumir debe ser un entero mayor que cero.",
                )
            })?;
        let empresa_id = texto(datos, "empresa_id");
        let item_id = texto(datos, "item_id");
        let solicitado = match datos.get("codigo_lote") {
            None | Some(Value::Null) => false,
            Some(Value::String(valor)) => !valor.is_empty(),
            Some(Value::Bool(valor)) => *valor,
            Some(Value::Number(valor)) => {
                valor.as_f64().map(|numero| numero != 0.0).unwrap_or(true)
            }
            Some(_) => true,
        };
        let codigo = datos
            .get("codigo_lote")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty());

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let item = sqlx::query(
            "SELECT track_inventory FROM catalog.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activo'
              FOR UPDATE",
        )
        .bind(&item_id)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(item) = item else {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        };
        if !item.get::<bool, _>("track_inventory") {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "Los lotes solo aplican a items con inventario.",
            ));
        }

        let filas = sqlx::query(
            "SELECT id, codigo_lote, cantidad_disponible,
                    (vence_en IS NOT NULL AND vence_en < CURRENT_DATE) AS vencido
               FROM catalog.lotes_inventario
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND cantidad_disponible > 0
                AND ($3::text IS NULL OR codigo_lote = $3)
              ORDER BY vence_en NULLS LAST, creado_en, id
              FOR UPDATE",
        )
        .bind(&empresa_id)
        .bind(&item_id)
        .bind(codigo)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;

        let lotes = filas
            .iter()
            .map(|fila| LoteDisponible {
                id: fila.get::<Uuid, _>("id").to_string(),
                codigo_lote: fila.get("codigo_lote"),
                cantidad_disponible: i64::from(fila.get::<i32, _>("cantidad_disponible")),
                vencido: fila.get("vencido"),
            })
            .collect::<Vec<_>>();

        if solicitado && lotes.iter().any(|lote| lote.vencido) {
            return Err(ApiError::dominio(
                "LOTE_VENCIDO",
                "El lote solicitado está vencido.",
            ));
        }

        let plan = plan_consumo(&lotes, cantidad).map_err(|faltante| {
            ApiError::dominio(
                "STOCK_LOTE_INSUFICIENTE",
                format!("No hay cantidad suficiente en lotes vigentes; faltan {faltante}."),
            )
        })?;

        for consumo in &plan {
            let consumir = i32::try_from(consumo.cantidad).map_err(|_| ApiError::interno())?;
            sqlx::query(
                "UPDATE catalog.lotes_inventario
                    SET cantidad_disponible = cantidad_disponible - $1,
                        actualizado_en = NOW()
                  WHERE id = $2::uuid",
            )
            .bind(consumir)
            .bind(&consumo.lote_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }

        let cantidad_i32 = i32::try_from(cantidad).map_err(|_| ApiError::interno())?;
        let stock = sqlx::query(
            "UPDATE catalog.items
                SET stock = stock - $3, actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND stock >= $3
              RETURNING stock",
        )
        .bind(&item_id)
        .bind(&empresa_id)
        .bind(cantidad_i32)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if stock.is_none() {
            return Err(ApiError::dominio(
                "STOCK_INSUFICIENTE",
                "El stock del item no alcanza para el consumo.",
            ));
        }
        tx.commit().await.map_err(interno)?;

        let consumidos = plan
            .iter()
            .map(|consumo| {
                json!({
                    "lote_id": consumo.lote_id,
                    "codigo_lote": consumo.codigo_lote,
                    "cantidad": consumo.cantidad,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "consumidos": consumidos,
            "total_consumido": cantidad,
        }))
    }

    pub async fn listar_lotes(
        &self,
        empresa_id: &str,
        item_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut condiciones = vec!["empresa_id = $1::uuid".to_string()];
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        if let Some(item) = item_id.filter(|valor| !valor.is_empty()) {
            params.push(item.to_string());
            condiciones.push("item_id = $2::uuid".to_string());
        }
        let sql = format!(
            "SELECT id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
                    cantidad_inicial, cantidad_disponible
               FROM catalog.lotes_inventario
              WHERE {}
              ORDER BY vence_en NULLS LAST, codigo_lote",
            condiciones.join(" AND ")
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(lote_a_json).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lote(id: &str, disponible: i64, vencido: bool) -> LoteDisponible {
        LoteDisponible {
            id: id.to_string(),
            codigo_lote: id.to_uppercase(),
            cantidad_disponible: disponible,
            vencido,
        }
    }

    #[test]
    fn debe_repartir_el_consumo_en_orden_fefo() {
        let lotes = vec![lote("l1", 5, false), lote("l2", 10, false)];
        let plan = plan_consumo(&lotes, 8).expect("plan");
        assert_eq!(
            plan,
            vec![
                Consumo {
                    lote_id: "l1".into(),
                    codigo_lote: "L1".into(),
                    cantidad: 5
                },
                Consumo {
                    lote_id: "l2".into(),
                    codigo_lote: "L2".into(),
                    cantidad: 3
                },
            ]
        );
    }

    #[test]
    fn debe_omitir_lotes_vencidos_y_reportar_el_faltante() {
        let lotes = vec![lote("l1", 5, true), lote("l2", 2, false)];
        let plan = plan_consumo(&lotes, 2).expect("plan");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].cantidad, 2);
        assert_eq!(plan_consumo(&lotes, 5).expect_err("faltante"), 3);
        assert!(plan_consumo(&lotes, 2).is_ok());
    }

    #[test]
    fn debe_aceptar_enteros_exactos_y_rechazar_fracciones() {
        assert_eq!(entero(&json!(10)), Some(10));
        assert_eq!(entero(&json!(10.0)), Some(10));
        assert_eq!(entero(&json!(10.5)), None);
        assert_eq!(entero(&json!("10")), None);
        assert_eq!(entero(&Value::Null), None);
    }

    #[test]
    fn debe_serializar_fechas_como_medianoche_local() {
        let fecha = NaiveDate::from_ymd_opt(2027, 1, 31).expect("fecha");
        let iso = fecha_iso(fecha);
        assert!(iso.ends_with(".000Z"));
        let parseada = chrono::DateTime::parse_from_rfc3339(&iso).expect("iso");
        assert_eq!(parseada.with_timezone(&Local).date_naive(), fecha);
    }

    #[test]
    fn debe_validar_los_metodos_de_costeo() {
        assert!(METODOS_COSTEO.contains(&"promedio"));
        assert!(METODOS_COSTEO.contains(&"fifo"));
        assert!(METODOS_COSTEO.contains(&"ueps"));
        assert!(!METODOS_COSTEO.contains(&"lifo"));
    }
}
