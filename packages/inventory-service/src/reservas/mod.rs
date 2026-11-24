//! Reserva y reintegro de stock de la saga de órdenes (RN-03/RN-06).
//!
//! Portado de `catalog-service` (ADR-0018) con el mismo contrato de eventos y
//! las mismas garantías: stock + lotes + series + outbox + idempotencia en una
//! sola transacción.

use crate::db::fila_a_json;
use crate::db::interno;
use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

mod procesados;
mod reintegros;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linea {
    pub empresa_id: String,
    pub sku: String,
    pub cantidad: i64,
    pub item_id: Option<String>,
    pub ubicacion_id: Option<String>,
}

pub struct Reservas;

impl Reservas {
    pub async fn reservar_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let mut lineas = expandir_componentes(tx, &lineas_de(items)).await?;
        // Orden estable de locks: dos órdenes con los mismos ítems en orden
        // distinto no deben bloquearse mutuamente.
        lineas.sort_by(|a, b| {
            (&a.empresa_id, &a.sku, &a.item_id, &a.ubicacion_id).cmp(&(
                &b.empresa_id,
                &b.sku,
                &b.item_id,
                &b.ubicacion_id,
            ))
        });
        sqlx::query("SAVEPOINT reserva_stock")
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

        let mut fallidos: Vec<Value> = Vec::new();
        for item in &lineas {
            let ubicacion_uuid = match item.ubicacion_id.as_deref() {
                Some(valor) => match Uuid::parse_str(valor) {
                    Ok(uuid) => Some(uuid),
                    Err(_) => {
                        fallidos.push(fallido(item, "ubicacion_invalida"));
                        continue;
                    }
                },
                None => None,
            };
            if let Some(ubicacion) = ubicacion_uuid {
                let activa: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM inventory.ubicaciones
                      WHERE id = $1::uuid AND empresa_id = $2::uuid AND activo",
                )
                .bind(ubicacion)
                .bind(&item.empresa_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(interno)?;
                if activa.is_none() {
                    fallidos.push(fallido(item, "ubicacion_invalida"));
                    continue;
                }
            }
            let lock = sqlx::query(
                "SELECT COALESCE((
                          SELECT SUM(s.stock)::int
                            FROM inventory.item_stock_ubicacion s
                           WHERE s.item_id = i.id
                             AND ($3::uuid IS NULL OR s.ubicacion_id = $3::uuid)
                        ), 0) AS stock,
                        i.id AS item_id, i.track_serial, i.garantia_meses,
                        EXISTS (SELECT 1 FROM item.items v
                                 WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL) AS tiene_variantes
                   FROM item.items i
                  WHERE i.empresa_id = $1::uuid AND LOWER(i.codigo) = LOWER($2)
                    AND i.tipo = 'producto' AND i.estado = 'activo'
                    AND i.eliminado_en IS NULL FOR UPDATE",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(ubicacion_uuid)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?;
            let Some(lock) = lock else {
                fallidos.push(fallido(item, "stock_insuficiente"));
                continue;
            };
            let stock: Option<i32> = lock.get("stock");
            if (stock.unwrap_or(0) as i64) < item.cantidad {
                fallidos.push(fallido(item, "stock_insuficiente"));
                continue;
            }
            let item_id: Uuid = lock.get("item_id");
            let track_serial: bool = lock.get("track_serial");
            let garantia_meses: Option<i32> = lock.get("garantia_meses");
            let tiene_variantes: bool = lock.get("tiene_variantes");
            if tiene_variantes {
                fallidos.push(fallido(item, "variante_requerida"));
                continue;
            }

            let lotes = sqlx::query(
                "SELECT id, cantidad_disponible,
                        (vence_en IS NOT NULL AND vence_en < CURRENT_DATE) AS vencido
                   FROM inventory.lotes_inventario
                  WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND cantidad_disponible > 0
                  ORDER BY vence_en NULLS LAST, creado_en, id
                  FOR UPDATE",
            )
            .bind(&item.empresa_id)
            .bind(item_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(interno)?;
            if !lotes.is_empty() {
                let disponibles = lotes
                    .iter()
                    .map(|lote| {
                        (
                            lote.get::<Uuid, _>("id"),
                            lote.get::<i32, _>("cantidad_disponible") as i64,
                            lote.get::<bool, _>("vencido"),
                        )
                    })
                    .collect::<Vec<_>>();
                match planificar_fefo(item.cantidad, &disponibles) {
                    Ok(asignaciones) => {
                        for (lote_id, cantidad) in asignaciones {
                            sqlx::query(
                                "UPDATE inventory.lotes_inventario
                                    SET cantidad_disponible = cantidad_disponible - $1,
                                        actualizado_en = NOW()
                                  WHERE id = $2::uuid",
                            )
                            .bind(cantidad)
                            .bind(lote_id)
                            .execute(&mut **tx)
                            .await
                            .map_err(interno)?;
                            sqlx::query(
                                "INSERT INTO inventory.reservas_lotes
                                   (order_id, lote_id, empresa_id, item_id, cantidad)
                                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5)",
                            )
                            .bind(order_id)
                            .bind(lote_id)
                            .bind(&item.empresa_id)
                            .bind(item_id)
                            .bind(cantidad)
                            .execute(&mut **tx)
                            .await
                            .map_err(interno)?;
                        }
                    }
                    Err(_) => {
                        fallidos.push(fallido(item, "lote_vencido"));
                        continue;
                    }
                }
            }

            if track_serial {
                let series = sqlx::query(
                    "SELECT s.id
                       FROM item.item_series s
                       LEFT JOIN inventory.ubicaciones u ON u.id = s.ubicacion_id
                      WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid
                        AND s.estado = 'disponible'
                        AND ($4::uuid IS NULL OR s.ubicacion_id = $4::uuid)
                      ORDER BY (u.predeterminada IS TRUE) DESC, s.creado_en, s.id
                      LIMIT $3
                      FOR UPDATE OF s",
                )
                .bind(&item.empresa_id)
                .bind(item_id)
                .bind(item.cantidad)
                .bind(ubicacion_uuid)
                .fetch_all(&mut **tx)
                .await
                .map_err(interno)?;
                if !seriales_suficientes(series.len(), item.cantidad) {
                    fallidos.push(fallido(item, "serial_insuficiente"));
                    continue;
                }
                let ids = series
                    .iter()
                    .map(|fila| fila.get::<Uuid, _>("id"))
                    .collect::<Vec<_>>();
                sqlx::query(
                    "UPDATE item.item_series
                        SET estado = 'vendido', order_id = $1::uuid, vendido_en = NOW(),
                            ubicacion_id = NULL,
                            garantia_hasta = CASE WHEN $3::int IS NULL THEN NULL
                              ELSE (CURRENT_DATE + ($3::int || ' months')::interval)::date END,
                            actualizado_en = NOW()
                      WHERE id = ANY($2::uuid[])",
                )
                .bind(order_id)
                .bind(&ids)
                .bind(garantia_meses)
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            }

            if let Some(ubicacion) = ubicacion_uuid {
                crate::stock::aplicar_delta_en_ubicacion_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    ubicacion,
                    -item.cantidad,
                )
                .await?;
            } else {
                crate::stock::aplicar_delta_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    -item.cantidad,
                )
                .await?;
            }
        }

        if !fallidos.is_empty() {
            sqlx::query("ROLLBACK TO SAVEPOINT reserva_stock")
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            sqlx::query("RELEASE SAVEPOINT reserva_stock")
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            return Ok(json!({ "ok": false, "fallidos": fallidos }));
        }

        sqlx::query(
            "INSERT INTO inventory.reservas_ordenes (order_id, items_json, creado_en)
             VALUES ($1::uuid, $2::jsonb, NOW())",
        )
        .bind(order_id)
        .bind(lineas_json(&lineas).to_string())
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        sqlx::query("RELEASE SAVEPOINT reserva_stock")
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

        let claves = claves_stock_de_lineas(&lineas);
        let stock_restante: Value =
            Self::stock_actual_de_en_transaccion(tx, &Value::Array(claves)).await?;
        Ok(json!({
            "ok": true,
            "fallidos": fallidos,
            "stock_restante": stock_restante,
            "reservados": lineas_json(&lineas),
        }))
    }
}

async fn expandir_componentes(
    tx: &mut Transaction<'_, Postgres>,
    items: &[Linea],
) -> Result<Vec<Linea>, ApiError> {
    let mut acumuladas: Vec<Linea> = Vec::new();
    let mut indices: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in items {
        let item_id = item.item_id.as_deref().unwrap_or("");
        if item_id.is_empty() {
            acumular(&mut acumuladas, &mut indices, item.clone());
            continue;
        }
        let resultado = sqlx::query(
            "WITH RECURSIVE bom(item_id, cantidad, ruta) AS (
               SELECT c.componente_item_id, c.cantidad, ARRAY[$1::uuid, c.componente_item_id]
                 FROM item.item_componentes c
                WHERE c.empresa_id = $2::uuid AND c.item_id = $1::uuid
               UNION ALL
               SELECT c.componente_item_id, bom.cantidad * c.cantidad, bom.ruta || c.componente_item_id
                 FROM bom
                 JOIN item.item_componentes c
                   ON c.item_id = bom.item_id AND c.empresa_id = $2::uuid
                WHERE NOT (c.componente_item_id = ANY(bom.ruta))
                  AND array_length(bom.ruta, 1) < 6
             )
             SELECT i.id AS item_id, i.codigo AS sku, SUM(bom.cantidad)::int AS cantidad
               FROM bom
               JOIN item.items i
                 ON i.id = bom.item_id AND i.eliminado_en IS NULL AND i.tipo = 'producto'
              WHERE NOT EXISTS (
                SELECT 1 FROM item.item_componentes c2
                 WHERE c2.item_id = i.id AND c2.empresa_id = $2::uuid
              )
              GROUP BY i.id, i.codigo",
        )
        .bind(item_id)
        .bind(&item.empresa_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(interno)?;
        if resultado.is_empty() {
            acumular(&mut acumuladas, &mut indices, item.clone());
            continue;
        }
        for fila in resultado {
            let componente_id: Uuid = fila.get("item_id");
            let sku: String = fila.get("sku");
            let cantidad: i32 = fila.get("cantidad");
            acumular(
                &mut acumuladas,
                &mut indices,
                Linea {
                    empresa_id: item.empresa_id.clone(),
                    sku,
                    cantidad: (cantidad as i64) * item.cantidad,
                    item_id: Some(componente_id.to_string()),
                    ubicacion_id: item.ubicacion_id.clone(),
                },
            );
        }
    }
    Ok(acumuladas)
}

fn planificar_fefo(cantidad: i64, lotes: &[(Uuid, i64, bool)]) -> Result<Vec<(Uuid, i64)>, i64> {
    let mut restante = cantidad;
    let mut asignaciones = Vec::new();
    for (lote_id, disponible, vencido) in lotes {
        if *vencido || restante == 0 {
            continue;
        }
        let consumir = restante.min(*disponible);
        if consumir > 0 {
            asignaciones.push((*lote_id, consumir));
            restante -= consumir;
        }
    }
    if restante > 0 {
        Err(restante)
    } else {
        Ok(asignaciones)
    }
}

fn seriales_suficientes(disponibles: usize, requeridas: i64) -> bool {
    disponibles as i64 >= requeridas
}

fn fallido(linea: &Linea, motivo: &str) -> Value {
    json!({
        "empresa_id": linea.empresa_id,
        "sku": linea.sku,
        "cantidad": linea.cantidad,
        "motivo": motivo,
    })
}

pub fn lineas_de(valor: &Value) -> Vec<Linea> {
    valor
        .as_array()
        .map(|lineas| {
            lineas
                .iter()
                .map(|linea| Linea {
                    empresa_id: texto(linea, "empresa_id"),
                    sku: texto(linea, "sku"),
                    cantidad: linea.get("cantidad").and_then(entero_json).unwrap_or(0),
                    item_id: linea
                        .get("item_id")
                        .and_then(Value::as_str)
                        .filter(|valor| !valor.is_empty())
                        .map(str::to_string),
                    ubicacion_id: linea
                        .get("ubicacion_id")
                        .and_then(Value::as_str)
                        .filter(|valor| !valor.is_empty())
                        .map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn lineas_json(lineas: &[Linea]) -> Value {
    Value::Array(
        lineas
            .iter()
            .map(|linea| {
                let mut objeto = Map::new();
                objeto.insert("empresa_id".into(), Value::String(linea.empresa_id.clone()));
                objeto.insert("sku".into(), Value::String(linea.sku.clone()));
                objeto.insert("cantidad".into(), json!(linea.cantidad));
                if let Some(item_id) = &linea.item_id {
                    objeto.insert("item_id".into(), Value::String(item_id.clone()));
                }
                if let Some(ubicacion_id) = &linea.ubicacion_id {
                    objeto.insert("ubicacion_id".into(), Value::String(ubicacion_id.clone()));
                }
                Value::Object(objeto)
            })
            .collect(),
    )
}

pub fn claves_stock(valor: &Value) -> Vec<Value> {
    let mut salida = Vec::new();
    let mut vistas = std::collections::HashSet::new();
    for linea in valor.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let empresa_id = texto(linea, "empresa_id");
        let sku = texto(linea, "sku").to_lowercase();
        let ubicacion_id = linea
            .get("ubicacion_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string);
        let clave = format!(
            "{empresa_id}:{sku}:{}",
            ubicacion_id.as_deref().unwrap_or("")
        );
        if vistas.insert(clave) {
            salida.push(json!({
                "empresa_id": empresa_id,
                "sku": sku,
                "ubicacion_id": ubicacion_id,
            }));
        }
    }
    salida
}

fn claves_stock_de_lineas(lineas: &[Linea]) -> Vec<Value> {
    let mut salida = Vec::new();
    let mut vistas = std::collections::HashSet::new();
    for linea in lineas {
        let sku = linea.sku.to_lowercase();
        let clave = format!(
            "{}:{sku}:{}",
            linea.empresa_id,
            linea.ubicacion_id.as_deref().unwrap_or("")
        );
        if vistas.insert(clave) {
            salida.push(json!({
                "empresa_id": linea.empresa_id,
                "sku": sku,
                "ubicacion_id": linea.ubicacion_id,
            }));
        }
    }
    salida
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn entero_json(valor: &Value) -> Option<i64> {
    valor.as_i64().or_else(|| {
        valor
            .as_f64()
            .filter(|numero| numero.is_finite() && numero.fract() == 0.0)
            .map(|numero| numero as i64)
    })
}

fn acumular(
    acumuladas: &mut Vec<Linea>,
    indices: &mut std::collections::HashMap<String, usize>,
    linea: Linea,
) {
    let clave = format!(
        "{}:{}:{}",
        linea.empresa_id,
        linea.sku.to_lowercase(),
        linea.ubicacion_id.as_deref().unwrap_or("")
    );
    match indices.get(&clave) {
        Some(indice) => acumuladas[*indice].cantidad += linea.cantidad,
        None => {
            indices.insert(clave, acumuladas.len());
            acumuladas.push(linea);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_planificar_fefo_omitando_vencidos() {
        let l1 = Uuid::new_v4();
        let l2 = Uuid::new_v4();
        let plan = planificar_fefo(5, &[(l1, 2, false), (l2, 10, false)]).expect("plan");
        assert_eq!(plan, vec![(l1, 2), (l2, 3)]);
        assert!(planificar_fefo(20, &[(l1, 2, false)]).is_err());
    }

    #[test]
    fn debe_parsear_y_deduplicar_lineas() {
        let lineas = lineas_de(&json!([
            {"empresa_id": "e-1", "sku": "SKU-1", "cantidad": 2, "item_id": "i-1"},
            {"empresa_id": "e-1", "sku": "SKU-2", "cantidad": 3},
            {"empresa_id": "e-1", "sku": "SKU-3", "cantidad": 1, "ubicacion_id": "u-1"}
        ]));
        assert_eq!(lineas.len(), 3);
        assert_eq!(lineas[0].cantidad, 2);
        assert_eq!(lineas[1].cantidad, 3);
        assert_eq!(lineas[2].ubicacion_id.as_deref(), Some("u-1"));
        assert_eq!(
            claves_stock_de_lineas(&lineas),
            vec![
                json!({"empresa_id": "e-1", "sku": "sku-1", "ubicacion_id": null}),
                json!({"empresa_id": "e-1", "sku": "sku-2", "ubicacion_id": null}),
                json!({"empresa_id": "e-1", "sku": "sku-3", "ubicacion_id": "u-1"})
            ]
        );
    }

    #[test]
    fn debe_separar_claves_de_stock_por_ubicacion() {
        let claves = claves_stock(&json!([
            {"empresa_id": "e-1", "sku": "SKU-1", "ubicacion_id": "u-1"},
            {"empresa_id": "e-1", "sku": "SKU-1", "ubicacion_id": "u-2"},
            {"empresa_id": "e-1", "sku": "SKU-1"}
        ]));
        assert_eq!(claves.len(), 3);
    }
}
