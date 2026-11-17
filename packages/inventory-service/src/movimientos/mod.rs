//! Movimientos de inventario: entradas, salidas, ajustes y traslados.
//!
//! Extraído de `catalog-service` (ADR-0018). Cada entrada, salida o ajuste
//! publica `stock.updated` por el Outbox para que `stores-service` refresque
//! el stock visible de sus ofertas (RN-02).

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::outbox::Outbox;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

const MAX_LINEAS: usize = 200;
const TIPOS_MOVIMIENTO: [&str; 4] = ["entrada", "salida", "ajuste", "traslado"];
const COLUMNAS: &str = "id, empresa_id, tipo, motivo, referencia,
  ubicacion_origen, ubicacion_destino, lineas, creado_por, creado_en";

pub struct Movimientos {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
}

#[derive(Debug, Clone, PartialEq)]
struct ItemResuelto {
    id: String,
    sku: String,
    nombre: String,
    stock: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct CabeceraMovimiento {
    tipo: String,
    origen: String,
    destino: String,
}

impl Movimientos {
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>) -> Self {
        Self { db, outbox }
    }

    pub async fn listar(&self, empresa_id: &str, filtros: &Value) -> Result<Value, ApiError> {
        let paginacion = parsear_paginacion(
            filtro_texto(filtros, "pagina").as_deref(),
            filtro_texto(filtros, "limite").as_deref(),
        );
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec!["m.empresa_id = $1::uuid".to_string()];
        if let Some(tipo) = filtro_texto(filtros, "tipo").filter(|valor| !valor.is_empty()) {
            params.push(tipo);
            condiciones.push(format!("m.tipo = ${}", params.len()));
        }
        if let Some(negocio) = filtro_texto(filtros, "negocio_id").filter(|valor| !valor.is_empty())
        {
            params.push(negocio);
            condiciones.push(format!(
                "EXISTS (
                   SELECT 1
                     FROM jsonb_array_elements(m.lineas) AS linea
                     JOIN item.items i
                       ON i.id = NULLIF(linea->>'item_id', '')::uuid
                    WHERE i.negocio_id = ${}::uuid
                 )",
                params.len()
            ));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let total: i32 = {
            let sql = format!(
                "SELECT COUNT(*)::int AS n FROM inventory.movimientos_inventario m {where_clause}"
            );
            let mut consulta = sqlx::query(&sql);
            for parametro in &params {
                consulta = consulta.bind(parametro);
            }
            consulta
                .fetch_one(&mut *tx)
                .await
                .map_err(interno)?
                .get("n")
        };
        let sql = format!(
            "SELECT {COLUMNAS}
               FROM inventory.movimientos_inventario m {where_clause}
              ORDER BY creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind((paginacion.pagina - 1) * paginacion.limite)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let movimientos = filas
            .iter()
            .map(|fila| {
                fila_a_json(fila)
                    .map_err(interno)
                    .map(|valor| serializar(&valor))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(movimientos, total as i64, paginacion))
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        datos: &Value,
        usuario_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let cabecera = validar_cabecera(datos)?;
        let permitir_stock_negativo = self
            .configuracion(empresa_id)
            .await?
            .get("permitir_stock_negativo")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let lineas = datos
            .get("lineas")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let items = self.resolver_items(empresa_id, &lineas).await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut lineas_salida = Vec::with_capacity(lineas.len());
        for (indice, linea) in lineas.iter().enumerate() {
            let item = &items[indice];
            let cantidad = entero_de(linea.get("cantidad")).ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad debe ser un entero mayor o igual a 0.",
                )
            })?;
            if cantidad < 0 {
                return Err(ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad debe ser un entero mayor o igual a 0.",
                ));
            }
            let anterior = item.stock;
            let nuevo = calcular_stock(&cabecera.tipo, anterior, cantidad);
            if nuevo < 0 && !permitir_stock_negativo {
                return Err(ApiError::dominio(
                    "STOCK_INSUFICIENTE",
                    format!(
                        "Stock insuficiente para {} (disponible {}).",
                        item.sku, anterior
                    ),
                ));
            }
            if cabecera.tipo != "traslado" {
                crate::stock::aplicar_delta_en_transaccion(
                    &mut tx,
                    empresa_id,
                    &item.id,
                    nuevo - anterior,
                )
                .await?;
                let motivo = datos
                    .get("motivo")
                    .filter(|valor| !valor.is_null())
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("movimiento {}", cabecera.tipo));
                sqlx::query(
                    "INSERT INTO inventory.ajustes_stock
                       (id, empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo,
                        realizado_en)
                     VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3, $4, $5, NOW())",
                )
                .bind(empresa_id)
                .bind(&item.id)
                .bind(anterior)
                .bind(nuevo)
                .bind(motivo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            lineas_salida.push(json!({
                "item_id": item.id,
                "sku": item.sku,
                "nombre": item.nombre,
                "cantidad": cantidad,
                "stock_anterior": anterior,
                "stock_nuevo": nuevo,
            }));
        }

        let motivo = texto_opcional(datos, "motivo").unwrap_or_default();
        let referencia = texto_opcional(datos, "referencia").unwrap_or_default();
        let lineas_json = Value::Array(lineas_salida.clone()).to_string();
        let fila = sqlx::query(&format!(
            "INSERT INTO inventory.movimientos_inventario
               (empresa_id, tipo, motivo, referencia, ubicacion_origen, ubicacion_destino,
                lineas, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7::jsonb, $8::uuid)
             RETURNING {COLUMNAS}"
        ))
        .bind(empresa_id)
        .bind(&cabecera.tipo)
        .bind(&motivo)
        .bind(&referencia)
        .bind(&cabecera.origen)
        .bind(&cabecera.destino)
        .bind(lineas_json)
        .bind(usuario_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let movimiento = fila_a_json(&fila).map_err(interno)?;
        if cabecera.tipo != "traslado" {
            if let Some(outbox) = &self.outbox {
                let items = lineas_salida
                    .iter()
                    .map(|linea| {
                        json!({
                            "empresa_id": empresa_id,
                            "sku": linea.get("sku").cloned().unwrap_or(Value::Null),
                            "stock_restante": linea.get("stock_nuevo").cloned().unwrap_or(json!(0)),
                        })
                    })
                    .collect();
                outbox
                    .insertar_stock_updated(
                        &mut tx,
                        &format!(
                            "movimiento-{}",
                            movimiento.get("id").and_then(Value::as_str).unwrap_or("")
                        ),
                        &cabecera.tipo,
                        items,
                    )
                    .await?;
            }
        }
        tx.commit().await.map_err(interno)?;
        Ok(serializar(&movimiento))
    }

    async fn configuracion(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT permitir_stock_negativo
               FROM inventory.configuracion_inventario WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(match fila {
            Some(fila) => {
                json!({"permitir_stock_negativo": fila.get::<bool, _>("permitir_stock_negativo")})
            }
            None => json!({"permitir_stock_negativo": false}),
        })
    }

    async fn resolver_items(
        &self,
        empresa_id: &str,
        lineas: &[Value],
    ) -> Result<Vec<ItemResuelto>, ApiError> {
        let mut resueltos = Vec::with_capacity(lineas.len());
        for linea in lineas {
            resueltos.push(
                self.buscar_item(
                    empresa_id,
                    linea.get("item_id").and_then(Value::as_str),
                    linea.get("sku").and_then(Value::as_str).unwrap_or(""),
                )
                .await?,
            );
        }
        Ok(resueltos)
    }

    async fn buscar_item(
        &self,
        empresa_id: &str,
        item_id: Option<&str>,
        sku: &str,
    ) -> Result<ItemResuelto, ApiError> {
        let item_id = item_id.filter(|valor| !valor.is_empty());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match item_id {
            Some(item_id) => sqlx::query(
                "SELECT i.id, i.codigo AS sku, i.nombre,
                        COALESCE((
                          SELECT SUM(s.stock)::int
                            FROM inventory.item_stock_ubicacion s
                           WHERE s.item_id = i.id
                        ), 0) AS stock
                   FROM item.items i
                  WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto' AND i.id = $2::uuid
                    AND i.eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT i.id, i.codigo AS sku, i.nombre,
                        COALESCE((
                          SELECT SUM(s.stock)::int
                            FROM inventory.item_stock_ubicacion s
                           WHERE s.item_id = i.id
                        ), 0) AS stock
                   FROM item.items i
                  WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto'
                    AND upper(i.codigo) = upper($2) AND i.eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(sku)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
        };
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Producto", item_id.unwrap_or(sku)));
        };
        let id: Uuid = fila.try_get("id").map_err(interno)?;
        let stock: Option<i32> = fila.try_get("stock").map_err(interno)?;
        Ok(ItemResuelto {
            id: id.to_string(),
            sku: fila.try_get("sku").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            stock: stock.unwrap_or(0) as i64,
        })
    }
}

fn validar_cabecera(datos: &Value) -> Result<CabeceraMovimiento, ApiError> {
    let tipo = datos
        .get("tipo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !TIPOS_MOVIMIENTO.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_MOVIMIENTO_INVALIDO",
            "El tipo de movimiento no es válido.",
        ));
    }
    let lineas = datos
        .get("lineas")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if lineas.is_empty() {
        return Err(ApiError::dominio(
            "LINEAS_REQUERIDAS",
            "El movimiento requiere al menos una línea.",
        ));
    }
    if lineas.len() > MAX_LINEAS {
        return Err(ApiError::dominio(
            "DEMASIADAS_LINEAS",
            format!("El máximo es {MAX_LINEAS} líneas."),
        ));
    }
    let origen = texto_opcional(datos, "ubicacion_origen").unwrap_or_default();
    let destino = texto_opcional(datos, "ubicacion_destino").unwrap_or_default();
    if tipo == "traslado" && (origen.is_empty() || destino.is_empty() || origen == destino) {
        return Err(ApiError::dominio(
            "TRASLADO_INVALIDO",
            "Un traslado requiere ubicación de origen y destino distintas.",
        ));
    }
    Ok(CabeceraMovimiento {
        tipo,
        origen,
        destino,
    })
}

fn calcular_stock(tipo: &str, anterior: i64, cantidad: i64) -> i64 {
    match tipo {
        "entrada" => anterior.saturating_add(cantidad),
        "salida" => anterior.saturating_sub(cantidad),
        "ajuste" => cantidad,
        _ => anterior,
    }
}

fn serializar(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "motivo": fila.get("motivo").and_then(Value::as_str).unwrap_or(""),
        "referencia": fila.get("referencia").and_then(Value::as_str).unwrap_or(""),
        "ubicacion_origen": fila.get("ubicacion_origen").and_then(Value::as_str).unwrap_or(""),
        "ubicacion_destino": fila.get("ubicacion_destino").and_then(Value::as_str).unwrap_or(""),
        "lineas": fila.get("lineas").cloned().unwrap_or_else(|| json!([])),
        "creado_por": fila.get("creado_por").cloned().unwrap_or(Value::Null),
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
    })
}

fn texto_opcional(valor: &Value, clave: &str) -> Option<String> {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .map(|texto| texto.trim().to_string())
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(Value::Number(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

fn entero_de(valor: Option<&Value>) -> Option<i64> {
    match valor? {
        Value::Number(numero) => numero
            .as_i64()
            .or_else(|| numero.as_f64().and_then(f64_a_entero)),
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                Some(0)
            } else {
                texto
                    .parse::<i64>()
                    .ok()
                    .or_else(|| texto.parse::<f64>().ok().and_then(f64_a_entero))
            }
        }
        _ => None,
    }
}

fn f64_a_entero(valor: f64) -> Option<i64> {
    if valor.is_finite()
        && valor.fract() == 0.0
        && valor >= i64::MIN as f64
        && valor <= i64::MAX as f64
    {
        Some(valor as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_calcular_el_stock_por_tipo() {
        assert_eq!(calcular_stock("entrada", 10, 3), 13);
        assert_eq!(calcular_stock("salida", 10, 3), 7);
        assert_eq!(calcular_stock("ajuste", 10, 3), 3);
        assert_eq!(calcular_stock("traslado", 10, 3), 10);
    }

    #[test]
    fn debe_serializar_y_filtrar_movimientos() {
        let movimiento = serializar(&json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "tipo": "entrada",
            "lineas": [{"sku": "SKU-1", "cantidad": 2}],
        }));
        assert_eq!(movimiento["id"], "11111111-1111-4111-8111-111111111111");
        assert_eq!(movimiento["motivo"], "");
        assert_eq!(movimiento["referencia"], "");
        assert_eq!(movimiento["lineas"][0]["sku"], "SKU-1");

        assert_eq!(
            filtro_texto(&json!({"tipo": "entrada"}), "tipo"),
            Some("entrada".into())
        );
        assert_eq!(filtro_texto(&json!({"tipo": 7}), "tipo"), Some("7".into()));
        assert_eq!(filtro_texto(&json!({"tipo": null}), "tipo"), None);
        assert_eq!(
            texto_opcional(&json!({"motivo": "  compra  "}), "motivo"),
            Some("compra".into())
        );
        assert_eq!(texto_opcional(&json!({}), "motivo"), None);
        assert_eq!(entero_de(Some(&json!("5"))), Some(5));
        assert_eq!(entero_de(Some(&json!(1.5))), None);
        assert_eq!(entero_de(None), None);
    }

    #[test]
    fn debe_validar_la_cabecera_del_movimiento() {
        let error = validar_cabecera(&json!({"tipo": "otro", "lineas": [{}]})).expect_err("tipo");
        assert_eq!(error.codigo, "TIPO_MOVIMIENTO_INVALIDO");
        let error =
            validar_cabecera(&json!({"tipo": "entrada", "lineas": []})).expect_err("lineas");
        assert_eq!(error.codigo, "LINEAS_REQUERIDAS");
        let error = validar_cabecera(&json!({
            "tipo": "traslado",
            "lineas": [{}],
            "ubicacion_origen": "a",
            "ubicacion_destino": "a"
        }))
        .expect_err("traslado");
        assert_eq!(error.codigo, "TRASLADO_INVALIDO");
        assert!(validar_cabecera(&json!({"tipo": "entrada", "lineas": [{}]})).is_ok());
    }
}
