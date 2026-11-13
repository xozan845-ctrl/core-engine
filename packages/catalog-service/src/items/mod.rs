//! Items unificados del catálogo: producto, servicio y alquiler.
//!
//! La tabla `catalog.items` es la fuente; `catalog.items_vista` añade
//! `item_id`. Atributos, variantes e imágenes viven aquí.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

pub const TIPOS: [&str; 3] = ["producto", "servicio", "alquiler"];

const COLUMNAS: &str = "item_id, empresa_id, tipo, codigo, nombre, descripcion,
  categoria, precio_cents, costo_cents, track_inventory, track_serial, stock,
  duration_minutes, atributos, parent_item_id, marca, codigo_barras, unidad,
  permite_fracciones, unidad_compra, contenido_cantidad, contenido_unidad,
  tipo_precio, tipo_costo, stock_minimo, stock_maximo, garantia_meses,
  es_materia_prima, ubicacion, estado, creado_en, actualizado_en";

const COLUMNAS_RETURNING: &str = "id AS item_id, empresa_id, tipo, codigo, nombre, descripcion,
  categoria, precio_cents, costo_cents, track_inventory, track_serial, stock,
  duration_minutes, atributos, parent_item_id, marca, codigo_barras, unidad,
  permite_fracciones, unidad_compra, contenido_cantidad, contenido_unidad,
  tipo_precio, tipo_costo, stock_minimo, stock_maximo, garantia_meses,
  es_materia_prima, ubicacion, estado, creado_en, actualizado_en";

pub struct Items {
    db: Arc<Db>,
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn texto_opcional<'a>(valor: &'a Value, clave: &str) -> Option<&'a str> {
    valor.get(clave).and_then(Value::as_str).map(str::trim)
}

fn entero(valor: &Value, clave: &str) -> Option<i64> {
    valor.get(clave).and_then(Value::as_i64)
}

fn booleano(valor: &Value, clave: &str) -> Option<bool> {
    valor.get(clave).and_then(Value::as_bool)
}

fn monto(valor: &Value, clave: &str) -> Result<Money, ApiError> {
    match valor.get(clave) {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn es_uuid(valor: &str) -> bool {
    Uuid::parse_str(valor).is_ok()
}

/// Valida un esquema JSON Schema (AJV `strict:false`).
pub fn compilar_esquema(esquema: &Value) -> Result<(), String> {
    jsonschema::validator_for(esquema)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Valida atributos contra un JSON Schema y devuelve los errores con el shape
/// `{instancia, parametro, mensaje}` del contrato.
pub fn errores_esquema(esquema: &Value, atributos: &Value) -> Vec<Value> {
    match jsonschema::validator_for(esquema) {
        Ok(validador) => validador
            .iter_errors(atributos)
            .map(|error| {
                json!({
                    "instancia": error.instance_path().to_string(),
                    "parametro": {},
                    "mensaje": error.to_string(),
                })
            })
            .collect(),
        Err(error) => vec![json!({
            "instancia": "",
            "parametro": {},
            "mensaje": error.to_string(),
        })],
    }
}

impl Items {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self, filtros: &Value) -> Result<Value, ApiError> {
        let empresa = la_empresa(filtros.get("empresa_id"))?;
        let paginacion = parsear_paginacion(
            filtros.get("pagina").and_then(Value::as_str),
            filtros.get("limite").and_then(Value::as_str),
        );
        let mut condiciones = vec!["empresa_id = $1".to_string()];
        let mut params: Vec<String> = vec![empresa.clone()];
        if let Some(tipo) = filtros.get("tipo") {
            let tipo = exigir_tipo(tipo)?;
            params.push(tipo.to_string());
            condiciones.push(format!("tipo = ${}", params.len()));
        }
        if let Some(categoria) = texto_opcional(filtros, "categoria").filter(|v| !v.is_empty()) {
            params.push(categoria.to_lowercase());
            condiciones.push(format!("LOWER(categoria) = ${}", params.len()));
        }
        if let Some(q) = texto_opcional(filtros, "q").filter(|v| !v.is_empty()) {
            params.push(format!("%{}%", q.to_lowercase()));
            condiciones.push(format!(
                "(LOWER(nombre) LIKE ${0} OR LOWER(codigo) LIKE ${0})",
                params.len()
            ));
        }
        let incluir_inactivos = filtros
            .get("incluir_inactivos")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !incluir_inactivos {
            condiciones.push("estado = 'activo'".to_string());
        }
        let where_clause = condiciones.join(" AND ");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM catalog.items_vista WHERE {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let limite = paginacion.limite;
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let sql = format!(
            "SELECT {COLUMNAS} FROM catalog.items_vista WHERE {where_clause}
             ORDER BY creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(limite)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let mut items = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(interno)?;
                serializar(&valor)
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        self.adjuntar_imagenes(&mut items).await?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn encontrar_por_id(
        &self,
        id: &str,
        empresa_id: Option<&str>,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match empresa_id {
            Some(empresa) => {
                sqlx::query(&format!(
                    "SELECT {COLUMNAS} FROM catalog.items_vista WHERE item_id = $1::uuid AND empresa_id = $2::uuid"
                ))
                .bind(id)
                .bind(empresa)
                .fetch_optional(&mut *tx)
                .await
            }
            None => {
                sqlx::query(&format!(
                    "SELECT {COLUMNAS} FROM catalog.items_vista WHERE item_id = $1::uuid"
                ))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
            }
        }
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let valor = fila_a_json(&fila).map_err(interno)?;
        let mut item = serializar(&valor)?;
        self.adjuntar_imagenes(std::slice::from_mut(&mut item))
            .await?;
        Ok(Some(item))
    }

    pub async fn lote_por_skus(&self, skus: &[String]) -> Result<Vec<Value>, ApiError> {
        if skus.is_empty() {
            return Ok(Vec::new());
        }
        let mut unicos: Vec<String> = skus
            .iter()
            .map(|sku| sku.trim().to_lowercase())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        unicos.sort();
        let marcadores = (1..=unicos.len())
            .map(|indice| format!("${indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock, i.track_serial,
                    (SELECT COUNT(*)::int FROM catalog.item_series s
                      WHERE s.item_id = i.id AND s.estado = 'disponible') AS seriales_disponibles,
                    CASE WHEN i.stock > 0
                          AND (NOT i.track_serial OR EXISTS (
                            SELECT 1 FROM catalog.item_series s
                             WHERE s.item_id = i.id AND s.estado = 'disponible'
                          ))
                         THEN 'disponible' ELSE 'agotado' END AS estado
               FROM catalog.items i
              WHERE LOWER(i.codigo) IN ({marcadores})
                AND i.tipo = 'producto' AND i.estado = 'activo' AND i.eliminado_en IS NULL
              ORDER BY i.creado_en DESC"
        );
        let mut consulta = sqlx::query(&sql);
        for sku in &unicos {
            consulta = consulta.bind(sku);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect()
    }

    pub async fn crear_item(&self, datos: &Value, empresa_id: &str) -> Result<Value, ApiError> {
        let tipo = exigir_tipo(datos.get("tipo").unwrap_or(&Value::Null))?;
        let codigo = normalizar_codigo(&texto(datos, "codigo"))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto(datos, "precio")?;
        let costo = match datos.get("costo") {
            Some(Value::Null) | None => Money::desde_centavos(0)?,
            Some(_) => monto(datos, "costo")?,
        };
        let track_inventory = if tipo == "producto" {
            booleano(datos, "track_inventory").unwrap_or(true)
        } else {
            false
        };
        if track_inventory {
            let stock = entero(datos, "stock").unwrap_or(-1);
            if stock < 0 {
                return Err(ApiError::dominio(
                    "STOCK_INVALIDO",
                    "El stock debe ser un entero no negativo.",
                ));
            }
        }
        let duration_minutes = if tipo == "servicio" {
            let duracion = entero(datos, "duration_minutes").unwrap_or(0);
            if !(1..=1440).contains(&duracion) {
                return Err(ApiError::dominio(
                    "DURACION_INVALIDA",
                    "La duración debe estar entre 1 y 1440 minutos.",
                ));
            }
            Some(duracion)
        } else {
            None
        };
        if let Some(padre) = texto_opcional(datos, "parent_item_id").filter(|v| !v.is_empty()) {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let fila = sqlx::query(
                "SELECT id, tipo, empresa_id FROM catalog.items WHERE id = $1::uuid AND eliminado_en IS NULL",
            )
            .bind(padre)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            let valido = fila.is_some_and(|fila| {
                let tipo_padre: String = fila.get("tipo");
                let empresa_padre: Uuid = fila.get("empresa_id");
                tipo_padre == tipo && empresa_padre.to_string() == empresa_id
            });
            if !valido {
                return Err(ApiError::dominio(
                    "ITEM_PADRE_INVALIDO",
                    "El item padre debe existir, ser del mismo tipo y de la misma empresa.",
                ));
            }
        }
        let atributos = self
            .validar_atributos(
                empresa_id,
                tipo,
                &texto(datos, "categoria"),
                datos.get("atributos").cloned().unwrap_or_else(|| json!({})),
            )
            .await?;
        self.exigir_codigo_libre(empresa_id, tipo, &codigo).await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado = sqlx::query(&format!(
            "INSERT INTO catalog.items
               (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
                costo_cents, track_inventory, stock, duration_minutes, atributos, estado,
                parent_item_id, marca, codigo_barras, unidad, permite_fracciones,
                unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
                tipo_costo, stock_minimo, stock_maximo, garantia_meses, es_materia_prima,
                ubicacion)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13, $14::uuid,
                     $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28)
             RETURNING {COLUMNAS_RETURNING}"
        ))
        .bind(empresa_id)
        .bind(tipo)
        .bind(&codigo)
        .bind(&nombre)
        .bind(texto(datos, "descripcion"))
        .bind({
            let categoria = texto(datos, "categoria");
            if categoria.is_empty() {
                "general".to_string()
            } else {
                categoria
            }
        })
        .bind(precio.centavos)
        .bind(costo.centavos)
        .bind(track_inventory)
        .bind(if track_inventory {
            entero(datos, "stock")
        } else {
            None
        })
        .bind(duration_minutes)
        .bind(atributos.to_string())
        .bind(if texto(datos, "estado") == "inactivo" {
            "inactivo"
        } else {
            "activo"
        })
        .bind(texto_opcional(datos, "parent_item_id").filter(|v| !v.is_empty()))
        .bind(texto(datos, "marca"))
        .bind(texto(datos, "codigo_barras"))
        .bind({
            let unidad = texto(datos, "unidad");
            if unidad.is_empty() {
                "Unidad (u)".to_string()
            } else {
                unidad
            }
        })
        .bind(booleano(datos, "permite_fracciones").unwrap_or(false))
        .bind({
            let unidad = texto(datos, "unidad_compra");
            if unidad.is_empty() {
                "Igual que la de venta".to_string()
            } else {
                unidad
            }
        })
        .bind(entero(datos, "contenido_cantidad").unwrap_or(0))
        .bind({
            let contenido = texto(datos, "contenido_unidad");
            if contenido.is_empty() {
                "Sin contenido".to_string()
            } else {
                contenido
            }
        })
        .bind(if texto(datos, "tipo_precio") == "escala" {
            "escala"
        } else {
            "fijo"
        })
        .bind(if texto(datos, "tipo_costo") == "escala" {
            "escala"
        } else {
            "fijo"
        })
        .bind(entero(datos, "stock_minimo").unwrap_or(0))
        .bind(entero(datos, "stock_maximo"))
        .bind(entero(datos, "garantia_meses"))
        .bind(booleano(datos, "es_materia_prima").unwrap_or(false))
        .bind(texto(datos, "ubicacion"))
        .fetch_optional(&mut *tx)
        .await;
        match resultado {
            Ok(fila) => {
                tx.commit().await.map_err(interno)?;
                let Some(fila) = fila else {
                    return Err(ApiError::dominio(
                        "ITEM_NO_CREADO",
                        "No se pudo crear el item.",
                    ));
                };
                serializar(&fila_a_json(&fila).map_err(interno)?)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "Ya existe un item con el código {codigo} en esta empresa."
                    )));
                }
                Err(interno(error))
            }
        }
    }

    pub async fn crear_alquiler(&self, datos: &Value, empresa_id: &str) -> Result<Value, ApiError> {
        let codigo = normalizar_codigo(&texto(datos, "codigo"))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto(datos, "precio")?;
        let costo = match datos.get("costo") {
            Some(Value::Null) | None => Money::desde_centavos(0)?,
            Some(_) => monto(datos, "costo")?,
        };
        let atributos = self
            .validar_atributos(
                empresa_id,
                "alquiler",
                &texto(datos, "categoria"),
                datos.get("atributos").cloned().unwrap_or_else(|| json!({})),
            )
            .await?;
        self.exigir_codigo_libre(empresa_id, "alquiler", &codigo)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.items
               (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
                costo_cents, track_inventory, stock, atributos, estado)
             VALUES ($1::uuid, 'alquiler', $2, $3, $4, $5, $6, $7, false, NULL, $8::jsonb, $9)
             RETURNING {COLUMNAS_RETURNING}"
        ))
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(texto(datos, "descripcion"))
        .bind({
            let categoria = texto(datos, "categoria");
            if categoria.is_empty() {
                "general".to_string()
            } else {
                categoria
            }
        })
        .bind(precio.centavos)
        .bind(costo.centavos)
        .bind(atributos.to_string())
        .bind(if texto(datos, "estado") == "inactivo" {
            "inactivo"
        } else {
            "activo"
        })
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "ITEM_NO_CREADO",
                "No se pudo crear el alquiler.",
            ));
        };
        serializar(&fila_a_json(&fila).map_err(interno)?)
    }

    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        if !es_uuid(id) {
            return Err(ApiError::dominio(
                "ITEM_INVALIDO",
                "El id del item no es válido.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        }
        Ok(())
    }

    pub async fn guardar_esquema(
        &self,
        empresa_id: Option<&str>,
        tipo: &Value,
        categoria: &str,
        esquema: &Value,
    ) -> Result<(), ApiError> {
        let tipo = exigir_tipo(tipo)?;
        let categoria = categoria.trim().to_lowercase();
        if categoria.is_empty() {
            return Err(ApiError::dominio(
                "CATEGORIA_INVALIDA",
                "La categoría es obligatoria.",
            ));
        }
        if !esquema.is_object() {
            return Err(ApiError::dominio(
                "ESQUEMA_INVALIDO",
                "El esquema debe ser un objeto JSON Schema.",
            ));
        }
        if let Err(mensaje) = compilar_esquema(esquema) {
            return Err(
                ApiError::dominio("ESQUEMA_INVALIDO", "El esquema JSON no es válido.")
                    .con_detalles(json!({ "mensaje": mensaje })),
            );
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO catalog.item_atributo_esquemas
               (empresa_id, tipo, categoria, esquema, version, activo)
             VALUES ($1::uuid, $2, $3, $4::jsonb,
                     COALESCE((SELECT MAX(version) + 1 FROM catalog.item_atributo_esquemas
                                WHERE empresa_id IS NOT DISTINCT FROM $1::uuid AND tipo = $2
                                  AND LOWER(categoria) = LOWER($3)), 1),
                     true)
             ON CONFLICT DO NOTHING",
        )
        .bind(empresa_id)
        .bind(tipo)
        .bind(&categoria)
        .bind(esquema.to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn esquemas(&self, empresa_id: Option<&str>) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, empresa_id, tipo, categoria, esquema, version, activo
               FROM catalog.item_atributo_esquemas
              WHERE activo = true AND (empresa_id IS NULL OR empresa_id = $1::uuid)
              ORDER BY empresa_id NULLS FIRST, tipo, categoria, version DESC",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect()
    }

    pub async fn validar_atributos(
        &self,
        empresa_id: &str,
        tipo: &str,
        categoria: &str,
        atributos: Value,
    ) -> Result<Value, ApiError> {
        if !atributos.is_object() {
            return Err(ApiError::dominio(
                "ATRIBUTOS_INVALIDOS",
                "attributes debe ser un objeto JSON.",
            ));
        }
        let categoria = if categoria.trim().is_empty() {
            "general"
        } else {
            categoria.trim()
        };
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT esquema
               FROM catalog.item_atributo_esquemas
              WHERE activo = true AND tipo = $1
                AND LOWER(categoria) IN (LOWER($2), 'general')
                AND (empresa_id = $3::uuid OR empresa_id IS NULL)
              ORDER BY (empresa_id IS NULL), (LOWER(categoria) = LOWER($2)) DESC, version DESC
              LIMIT 1",
        )
        .bind(tipo)
        .bind(categoria)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(atributos);
        };
        let esquema: Value = fila.try_get("esquema").map_err(interno)?;
        let errores = errores_esquema(&esquema, &atributos);
        if errores.is_empty() {
            Ok(atributos)
        } else {
            Err(ApiError::dominio(
                "ATRIBUTOS_INVALIDOS",
                "Los atributos no cumplen el JSON Schema de la categoría.",
            )
            .con_detalles(json!({ "errores": errores })))
        }
    }

    async fn exigir_codigo_libre(
        &self,
        empresa_id: &str,
        tipo: &str,
        codigo: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id FROM catalog.items
              WHERE empresa_id = $1::uuid AND tipo = $2 AND upper(codigo) = upper($3)
                AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(tipo)
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "Ya existe un item con el código {codigo} en esta empresa."
            )));
        }
        Ok(())
    }

    async fn adjuntar_imagenes(&self, items: &mut [Value]) -> Result<(), ApiError> {
        if items.is_empty() {
            return Ok(());
        }
        let ids = items
            .iter()
            .filter_map(|item| item.get("item_id").and_then(Value::as_str))
            .map(|id| Uuid::parse_str(id).map_err(|_| ApiError::interno()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, item_id, objeto_id, orden, es_principal
               FROM catalog.item_imagenes
              WHERE item_id = ANY($1::uuid[])
              ORDER BY item_id, orden",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let mut por_item: std::collections::HashMap<String, Vec<Value>> =
            std::collections::HashMap::new();
        for fila in filas {
            let item_id: Uuid = fila.get("item_id");
            let imagen = fila_a_json(&fila).map_err(interno)?;
            por_item
                .entry(item_id.to_string())
                .or_default()
                .push(json!({
                    "id": imagen.get("id"),
                    "objeto_id": imagen.get("objeto_id"),
                    "orden": imagen.get("orden"),
                    "es_principal": imagen.get("es_principal"),
                }));
        }
        for item in items {
            let item_id = item
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let imagenes = por_item.remove(&item_id).unwrap_or_default();
            if let Some(objeto) = item.as_object_mut() {
                objeto.insert("imagenes".into(), Value::Array(imagenes));
            }
        }
        Ok(())
    }
}

fn la_empresa(valor: Option<&Value>) -> Result<String, ApiError> {
    let empresa = valor.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if !es_uuid(empresa) {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "Indica una empresa válida.",
        ));
    }
    Ok(empresa.to_string())
}

pub fn exigir_tipo(valor: &Value) -> Result<&str, ApiError> {
    valor
        .as_str()
        .filter(|tipo| TIPOS.contains(tipo))
        .ok_or_else(|| {
            ApiError::dominio(
                "TIPO_ITEM_INVALIDO",
                "El tipo debe ser producto, servicio o alquiler.",
            )
        })
}

fn normalizar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = (2..=32).contains(&codigo.len())
        && codigo.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            "El código solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim().to_string();
    let longitud = nombre.chars().count();
    if !(2..=200).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(nombre)
}

fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

/// Serializa una fila de `items_vista` en el orden del contrato `Item`.
pub fn serializar(fila: &Value) -> Result<Value, ApiError> {
    let texto = |clave: &str| {
        fila.get(clave)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let opcional = |clave: &str| fila.get(clave).cloned().unwrap_or(Value::Null);
    let entero = |clave: &str| fila.get(clave).and_then(Value::as_i64);
    let mut objeto = Map::new();
    objeto.insert("item_id".into(), Value::String(texto("item_id")));
    objeto.insert("empresa_id".into(), Value::String(texto("empresa_id")));
    objeto.insert("tipo".into(), Value::String(texto("tipo")));
    objeto.insert("codigo".into(), Value::String(texto("codigo")));
    objeto.insert("nombre".into(), Value::String(texto("nombre")));
    objeto.insert("descripcion".into(), Value::String(texto("descripcion")));
    objeto.insert("categoria".into(), Value::String(texto("categoria")));
    objeto.insert(
        "precio".into(),
        Value::String(Money::desde_centavos(entero("precio_cents").unwrap_or(0))?.string()),
    );
    objeto.insert(
        "costo".into(),
        Value::String(Money::desde_centavos(entero("costo_cents").unwrap_or(0))?.string()),
    );
    objeto.insert(
        "track_inventory".into(),
        Value::Bool(
            fila.get("track_inventory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "track_serial".into(),
        Value::Bool(
            fila.get("track_serial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert("stock".into(), opcional("stock"));
    objeto.insert("duration_minutes".into(), opcional("duration_minutes"));
    objeto.insert(
        "atributos".into(),
        fila.get("atributos").cloned().unwrap_or_else(|| json!({})),
    );
    objeto.insert("parent_item_id".into(), opcional("parent_item_id"));
    objeto.insert(
        "marca".into(),
        Value::String(
            fila.get("marca")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert(
        "codigo_barras".into(),
        Value::String(
            fila.get("codigo_barras")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert(
        "unidad".into(),
        Value::String(
            fila.get("unidad")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Unidad (u)")
                .to_string(),
        ),
    );
    objeto.insert(
        "permite_fracciones".into(),
        Value::Bool(
            fila.get("permite_fracciones")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "unidad_compra".into(),
        Value::String(
            fila.get("unidad_compra")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Igual que la de venta")
                .to_string(),
        ),
    );
    objeto.insert(
        "contenido_cantidad".into(),
        Value::from(entero("contenido_cantidad").unwrap_or(0)),
    );
    objeto.insert(
        "contenido_unidad".into(),
        Value::String(
            fila.get("contenido_unidad")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Sin contenido")
                .to_string(),
        ),
    );
    objeto.insert(
        "tipo_precio".into(),
        Value::String(
            if texto("tipo_precio") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "tipo_costo".into(),
        Value::String(
            if texto("tipo_costo") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "stock_minimo".into(),
        Value::from(entero("stock_minimo").unwrap_or(0)),
    );
    objeto.insert("stock_maximo".into(), opcional("stock_maximo"));
    objeto.insert("garantia_meses".into(), opcional("garantia_meses"));
    objeto.insert(
        "es_materia_prima".into(),
        Value::Bool(
            fila.get("es_materia_prima")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "ubicacion".into(),
        Value::String(
            fila.get("ubicacion")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert("imagenes".into(), json!([]));
    objeto.insert(
        "estado".into(),
        Value::String(
            if texto("estado") == "inactivo" {
                "inactivo"
            } else {
                "activo"
            }
            .to_string(),
        ),
    );
    objeto.insert("creado_en".into(), Value::String(texto("creado_en")));
    objeto.insert(
        "actualizado_en".into(),
        Value::String(texto("actualizado_en")),
    );
    Ok(Value::Object(objeto))
}

/// Transacción con claims de RLS (reutilizada por módulos hermanos).
pub async fn transaccion(items: &Items) -> Result<Transaction<'static, Postgres>, ApiError> {
    items.db.transaccion().await.map_err(interno)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_y_validar_codigos_y_nombres() {
        assert_eq!(normalizar_codigo(" sku-1 ").expect("codigo"), "SKU-1");
        assert_eq!(
            normalizar_codigo("a").expect_err("corto").codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            normalizar_codigo("skú").expect_err("simbolo").codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(validar_nombre(" Producto ").expect("nombre"), "Producto");
        assert_eq!(
            validar_nombre("x").expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_esquemas_json() {
        assert!(compilar_esquema(&json!({
            "type": "object",
            "properties": {"color": {"type": "string"}},
            "required": ["color"],
            "additionalProperties": false
        }))
        .is_ok());
        let errores = errores_esquema(
            &json!({
                "type": "object",
                "properties": {"color": {"type": "string"}},
                "required": ["color"],
                "additionalProperties": false
            }),
            &json!({"otro": 1}),
        );
        assert!(!errores.is_empty());
        assert!(errores[0].get("instancia").is_some());
    }

    #[test]
    fn debe_serializar_item_con_montos_y_defaults() {
        let fila = json!({
            "item_id": "11111111-1111-4111-8111-111111111111",
            "empresa_id": "22222222-2222-4222-8222-222222222222",
            "tipo": "producto",
            "codigo": "SKU-1",
            "nombre": "Producto",
            "descripcion": "",
            "categoria": "general",
            "precio_cents": 115000,
            "costo_cents": 0,
            "track_inventory": true,
            "track_serial": false,
            "stock": 5,
            "duration_minutes": null,
            "atributos": {},
            "parent_item_id": null,
            "marca": null,
            "codigo_barras": null,
            "unidad": null,
            "permite_fracciones": null,
            "unidad_compra": null,
            "contenido_cantidad": null,
            "contenido_unidad": null,
            "tipo_precio": null,
            "tipo_costo": null,
            "stock_minimo": null,
            "stock_maximo": null,
            "garantia_meses": null,
            "es_materia_prima": null,
            "ubicacion": null,
            "estado": "activo",
            "creado_en": "2026-11-13T00:00:00.000Z",
            "actualizado_en": "2026-11-13T00:00:00.000Z"
        });
        let item = serializar(&fila).expect("item");
        assert_eq!(item["precio"], "1150.00");
        assert_eq!(item["unidad"], "Unidad (u)");
        assert_eq!(item["tipo_precio"], "fijo");
        assert_eq!(item["imagenes"], json!([]));
    }
}
