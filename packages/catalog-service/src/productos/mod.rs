//! Catálogo maestro **por empresa** (ADR-27) con variantes e imágenes (ADR-28).
//!
//! Réplica de `productos.service.ts`: un producto es un item `tipo='producto'`,
//! una variante es un item hijo (`parent_item_id`) y las imágenes viven en
//! `catalog.item_imagenes`. Las reservas/órdenes siguen operando por
//! `(empresa, código)` y el padre agrega stock y precio mínimo.
//!
//! La entrada y la salida pública son `serde_json::Value` para conservar el
//! contrato exacto de la API y del consumidor de pedidos.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use crate::money::Money;
use crate::outbox::Outbox;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub const EMPRESA_LEGACY_ID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_IMAGENES: i64 = 3;
const MAX_VARIANTES: usize = 50;
const URL_IMAGEN_PUBLICA: &str = "/api/v2/storage/public/objetos";

/// Columnas de lectura compartidas por listado, detalle y variantes.
const COLUMNAS_PRODUCTO: &str = "i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.categoria, i.precio_cents, i.precio_cents AS precio_base_cents, i.costo_cents,
  i.marca, i.codigo_barras, i.unidad, i.permite_fracciones, i.unidad_compra,
  i.contenido_cantidad, i.contenido_unidad, i.tipo_precio, i.tipo_costo,
  i.stock_minimo, i.stock_maximo, i.garantia_meses, i.es_materia_prima, i.ubicacion,
  i.actualizado_en, i.stock, i.estado, i.creado_en,
  i.parent_item_id AS producto_padre_id, i.atributos,
  COALESCE((SELECT MIN(c.precio_cents) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.precio_cents) AS precio_desde_cents,
  COALESCE((SELECT SUM(c.stock) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.stock) AS stock_total";

/// Existencia de variantes vendibles del item.
const EXISTE_VARIANTES: &str = "EXISTS (SELECT 1 FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes";

#[derive(Debug, Clone, PartialEq)]
struct Metadatos {
    costo_cents: i64,
    marca: String,
    codigo_barras: String,
    unidad: String,
    permite_fracciones: bool,
    unidad_compra: String,
    contenido_cantidad: i64,
    contenido_unidad: String,
    tipo_precio: String,
    tipo_costo: String,
    stock_minimo: i64,
    stock_maximo: Option<i64>,
    garantia_meses: Option<i64>,
    es_materia_prima: bool,
    ubicacion: String,
    estado_operativo: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ImagenInput {
    objeto_id: String,
    orden: Option<i64>,
    es_principal: bool,
}

#[derive(Debug, Clone)]
struct VarianteInput {
    sku: String,
    nombre: Option<String>,
    atributos: Value,
    precio_base: Option<Value>,
    stock: Option<Value>,
    imagenes: Vec<ImagenInput>,
}

/// Línea de reserva/reintegro. `item_id` solo viaja cuando el llamador lo
/// conoce (permite expandir la composición BOM).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Linea {
    #[serde(default)]
    empresa_id: String,
    #[serde(default)]
    sku: String,
    #[serde(default)]
    cantidad: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_id: Option<String>,
}

/// Parámetro heterogéneo para el UPDATE dinámico de `actualizar`.
enum Param {
    Texto(String),
    Entero(i64),
    OpcionalEntero(Option<i64>),
    Booleano(bool),
    Json(String),
}

pub struct Productos {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    items: Option<Arc<Items>>,
    identidad_url: String,
    storage_url: String,
    stores_url: String,
    clave_interna: String,
    timeout_identidad_ms: u64,
}

impl Productos {
    /// `outbox` e `items` son opcionales igual que en NestJS (`@Optional`):
    /// sin outbox no se emite `stock.updated` al ajustar stock a mano y sin
    /// items no se validan los atributos contra JSON Schema.
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>, items: Option<Arc<Items>>) -> Self {
        Self {
            db,
            outbox,
            items,
            identidad_url: variable("IDENTITY_SERVICE_URL", "http://identity-service:3001"),
            storage_url: variable("STORAGE_SERVICE_URL", "http://storage-service:3010"),
            stores_url: variable("STORES_SERVICE_URL", "http://stores-service:3003"),
            clave_interna: env::var("INTERNAL_API_KEY").unwrap_or_default(),
            timeout_identidad_ms: env::var("INTERNAL_REQUEST_TIMEOUT_MS")
                .ok()
                .and_then(|valor| valor.trim().parse::<u64>().ok())
                .filter(|valor| *valor > 0)
                .unwrap_or(3_000),
        }
    }

    pub async fn listar(&self, filtros: &Value) -> Result<Value, ApiError> {
        let empresa_id = exigir_empresa(filtros.get("empresa_id"))?;
        let paginacion = paginacion_de(filtros);
        let mut params: Vec<String> = vec![empresa_id];
        let mut condiciones: Vec<String> = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.tipo = 'producto'".to_string(),
            "i.parent_item_id IS NULL".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        if !filtros
            .get("incluir_inactivos")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            condiciones.push("i.estado = 'activo'".to_string());
        }
        if let Some(q) = filtros.get("q").filter(|valor| valor_verdadero(valor)) {
            params.push(format!("%{}%", texto_js(q).to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0})",
                params.len()
            ));
        }
        if let Some(estado) = filtros.get("estado").filter(|valor| valor_verdadero(valor)) {
            condiciones.push(if texto_js(estado) == "disponible" {
                "i.stock > 0".to_string()
            } else {
                "i.stock = 0".to_string()
            });
        }
        if let Some(categoria) = filtros
            .get("categoria")
            .filter(|valor| valor_verdadero(valor))
        {
            params.push(texto_js(categoria));
            condiciones.push(format!("LOWER(i.categoria) = LOWER(${})", params.len()));
        }
        let where_sql = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM catalog.items i {where_sql}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let sql = format!(
            "SELECT {COLUMNAS_PRODUCTO},
                    {EXISTE_VARIANTES}
               FROM catalog.items i {where_sql}
              ORDER BY i.creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let ids = filas
            .iter()
            .map(|fila| fila.get::<Uuid, _>("id").to_string())
            .collect::<Vec<_>>();
        let imagenes = self.imagenes_de(&ids).await?;
        let items = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(interno)?;
                let id = texto(&valor, "id");
                let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                serializar(&valor, propias, Vec::new())
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn encontrar_por_id(
        &self,
        id: &str,
        empresa_id: Option<&str>,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE i.id = $1::uuid AND i.tipo = 'producto' AND i.eliminado_en IS NULL"
        );
        if empresa_id.is_some() {
            sql.push_str(" AND i.empresa_id = $2::uuid");
        }
        if !incluir_inactivos {
            sql.push_str(" AND i.estado = 'activo'");
        }
        let mut consulta = sqlx::query(&sql).bind(id);
        if let Some(empresa) = empresa_id {
            consulta = consulta.bind(empresa);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    pub async fn encontrar_por_sku(
        &self,
        sku: &str,
        empresa_id: Option<&str>,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        let mut sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE LOWER(i.codigo) = LOWER($1) AND i.tipo = 'producto'
                AND i.eliminado_en IS NULL"
        );
        if empresa_id.is_some() {
            sql.push_str(" AND i.empresa_id = $2::uuid");
        }
        if !incluir_inactivos {
            sql.push_str(" AND i.estado = 'activo'");
        }
        let mut consulta = sqlx::query(&sql).bind(sku);
        if let Some(empresa) = empresa_id {
            consulta = consulta.bind(empresa);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    pub async fn encontrar_por_codigo_barras(
        &self,
        codigo: &str,
        empresa_id: &str,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        let codigo_barras = codigo.trim();
        if codigo_barras.is_empty() {
            return Ok(None);
        }
        let filtro_estado = if incluir_inactivos {
            ""
        } else {
            "AND i.estado = 'activo'"
        };
        let sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE i.empresa_id = $1::uuid AND i.codigo_barras = $2
                AND i.tipo = 'producto' AND i.eliminado_en IS NULL
                {filtro_estado}
              LIMIT 1"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&sql)
            .bind(empresa_id)
            .bind(codigo_barras)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    /// Consulta por lote de códigos. Un código puede repetirse en empresas
    /// distintas, por eso la fila incluye `empresa_id` (ADR-27).
    pub async fn lote_por_skus(&self, skus: &[String]) -> Result<Value, ApiError> {
        if skus.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let mut unicos: Vec<String> = Vec::new();
        let mut vistos: HashSet<String> = HashSet::new();
        for sku in skus {
            let normalizado = sku.to_lowercase();
            if vistos.insert(normalizado.clone()) {
                unicos.push(normalizado);
            }
        }
        let marcadores = (1..=unicos.len())
            .map(|indice| format!("${indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock,
                    CASE WHEN i.stock > 0 THEN 'disponible' ELSE 'agotado' END AS estado
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
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    /// Stock restante por producto (alimenta `stock.updated` para RN-02).
    pub async fn stock_actual_de(&self, items: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let salida = self.stock_actual_de_en_transaccion(&mut tx, items).await?;
        tx.commit().await.map_err(interno)?;
        Ok(salida)
    }

    pub async fn stock_actual_de_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let claves = claves_stock(items);
        if claves.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let filas = sqlx::query(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock AS stock_restante
               FROM catalog.items i
               JOIN jsonb_to_recordset($1::jsonb) AS x(empresa_id uuid, sku text)
                 ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
              WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL",
        )
        .bind(Value::Array(claves).to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    pub async fn crear(&self, datos: &Value, opciones: Option<&Value>) -> Result<Value, ApiError> {
        let empresa_id = texto(datos, "empresa_id");
        let sku = normalizar_sku(&texto(datos, "sku"))?;
        let stock = validar_stock(datos.get("stock"))?;
        let precio = validar_precio(datos.get("precio_base"))?;
        let metadatos = normalizar_metadatos(datos)?;
        let variantes = variantes_de(datos);
        if variantes.len() > MAX_VARIANTES {
            return Err(ApiError::dominio(
                "DEMASIADAS_VARIANTES",
                format!("Un producto admite como máximo {MAX_VARIANTES} variantes."),
            ));
        }
        let mut skus: HashSet<String> = HashSet::new();
        skus.insert(sku.clone());
        for variante in &variantes {
            let variante_sku = normalizar_sku(&variante.sku)?;
            validar_stock(variante.stock.as_ref())?;
            validar_precio(variante.precio_base.as_ref())?;
            if !skus.insert(variante_sku.clone()) {
                return Err(ApiError::conflicto(format!(
                    "El SKU {variante_sku} está repetido en la solicitud."
                )));
            }
        }
        if self.sku_existe(&empresa_id, &sku).await? {
            return Err(ApiError::conflicto(format!(
                "Ya existe un producto con el SKU {sku} en esta empresa."
            )));
        }
        for variante in &variantes {
            let variante_sku = normalizar_sku(&variante.sku)?;
            if self.sku_existe(&empresa_id, &variante_sku).await? {
                return Err(ApiError::conflicto(format!(
                    "Ya existe un producto con el SKU {variante_sku} en esta empresa."
                )));
            }
        }
        let empresa_validada = opciones
            .and_then(|opciones| {
                opciones
                    .get("empresa_validada")
                    .or_else(|| opciones.get("empresaValidada"))
            })
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !empresa_validada {
            self.validar_empresa_catalogo(&empresa_id).await?;
        }
        let categoria = categoria_de(datos);
        if let Some(items) = &self.items {
            items
                .validar_atributos(&empresa_id, "producto", &categoria, atributos_de(datos))
                .await?;
        }
        self.validar_codigo_barras(&empresa_id, &metadatos.codigo_barras, None)
            .await?;
        let imagenes = imagenes_de(datos.get("imagenes"));
        self.validar_imagenes(&empresa_id, &imagenes).await?;
        for variante in &variantes {
            if let Some(items) = &self.items {
                items
                    .validar_atributos(
                        &empresa_id,
                        "producto",
                        &categoria,
                        variante.atributos.clone(),
                    )
                    .await?;
            }
            self.validar_imagenes(&empresa_id, &variante.imagenes)
                .await?;
        }

        let padre_id = Uuid::new_v4();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO catalog.items
               (id, empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
                costo_cents, marca, codigo_barras, unidad, permite_fracciones,
                unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
                tipo_costo, stock_minimo, stock_maximo, garantia_meses,
                es_materia_prima, ubicacion, estado, track_inventory, stock,
                parent_item_id, atributos, actualizado_en)
             VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                     $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, true, $24,
                     NULL, $25::jsonb, NOW())",
        )
        .bind(padre_id)
        .bind(&empresa_id)
        .bind(&sku)
        .bind(texto(datos, "nombre").trim().to_string())
        .bind(texto(datos, "descripcion").trim().to_string())
        .bind(categoria.trim().to_string())
        .bind(precio.centavos)
        .bind(metadatos.costo_cents)
        .bind(&metadatos.marca)
        .bind(&metadatos.codigo_barras)
        .bind(&metadatos.unidad)
        .bind(metadatos.permite_fracciones)
        .bind(&metadatos.unidad_compra)
        .bind(metadatos.contenido_cantidad)
        .bind(&metadatos.contenido_unidad)
        .bind(&metadatos.tipo_precio)
        .bind(&metadatos.tipo_costo)
        .bind(metadatos.stock_minimo)
        .bind(metadatos.stock_maximo)
        .bind(metadatos.garantia_meses)
        .bind(metadatos.es_materia_prima)
        .bind(&metadatos.ubicacion)
        .bind(&metadatos.estado_operativo)
        .bind(stock)
        .bind(atributos_de(datos).to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        registrar_historico(
            &mut tx,
            &empresa_id,
            &padre_id.to_string(),
            None,
            precio.centavos,
        )
        .await?;

        for variante in &variantes {
            let variante_id = Uuid::new_v4();
            let variante_sku = normalizar_sku(&variante.sku)?;
            let variante_precio = validar_precio(variante.precio_base.as_ref())?;
            let variante_stock = validar_stock(variante.stock.as_ref())?;
            let nombre = variante
                .nombre
                .clone()
                .unwrap_or_else(|| texto(datos, "nombre"))
                .trim()
                .to_string();
            sqlx::query(
                "INSERT INTO catalog.items
                   (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                    precio_cents, track_inventory, stock, parent_item_id, atributos)
                 VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, $7, true, $8, $9::uuid, $10::jsonb)",
            )
            .bind(variante_id)
            .bind(&empresa_id)
            .bind(&variante_sku)
            .bind(nombre)
            .bind(texto(datos, "descripcion").trim().to_string())
            .bind(categoria.trim().to_string())
            .bind(variante_precio.centavos)
            .bind(variante_stock)
            .bind(padre_id)
            .bind(variante.atributos.to_string())
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            registrar_historico(
                &mut tx,
                &empresa_id,
                &variante_id.to_string(),
                None,
                variante_precio.centavos,
            )
            .await?;
            insertar_imagenes(&mut tx, &empresa_id, variante_id, &variante.imagenes).await?;
        }

        insertar_imagenes(&mut tx, &empresa_id, padre_id, &imagenes).await?;
        if !variantes.is_empty() {
            recalcular_padre(&mut tx, padre_id).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(&padre_id.to_string(), Some(&empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", &padre_id.to_string()))
    }

    /// Agrega una variante a un producto padre (ADR-28).
    pub async fn crear_variante(
        &self,
        padre_id: &str,
        datos: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        if padre
            .get("producto_padre_id")
            .and_then(Value::as_str)
            .is_some()
        {
            return Err(ApiError::dominio(
                "NO_ES_PADRE",
                "Solo un producto padre puede recibir variantes.",
            ));
        }
        let variantes_actuales = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        if variantes_actuales >= MAX_VARIANTES {
            return Err(ApiError::dominio(
                "DEMASIADAS_VARIANTES",
                format!("Un producto admite como máximo {MAX_VARIANTES} variantes."),
            ));
        }
        self.validar_sin_ofertas_activas(padre_id).await?;
        let categoria = texto(&padre, "categoria");
        let atributos = atributos_de(datos);
        if let Some(items) = &self.items {
            items
                .validar_atributos(empresa_id, "producto", &categoria, atributos)
                .await?;
        }
        let sku = normalizar_sku(&texto(datos, "sku"))?;
        let stock = validar_stock(datos.get("stock"))?;
        let precio = validar_precio(datos.get("precio_base"))?;
        if self.sku_existe(empresa_id, &sku).await? {
            return Err(ApiError::conflicto(format!(
                "Ya existe un producto con el SKU {sku} en esta empresa."
            )));
        }
        let imagenes = imagenes_de(datos.get("imagenes"));
        self.validar_imagenes(empresa_id, &imagenes).await?;

        let variante_id = Uuid::new_v4();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO catalog.items
               (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                precio_cents, track_inventory, stock, parent_item_id, atributos)
             VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, $7, true, $8, $9::uuid, $10::jsonb)",
        )
        .bind(variante_id)
        .bind(empresa_id)
        .bind(&sku)
        .bind(
            datos
                .get("nombre")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    padre
                        .get("nombre")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default()
                .trim()
                .to_string(),
        )
        .bind(texto(&padre, "descripcion"))
        .bind(&categoria)
        .bind(precio.centavos)
        .bind(stock)
        .bind(padre_id)
        .bind(atributos_de(datos).to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        registrar_historico(
            &mut tx,
            empresa_id,
            &variante_id.to_string(),
            None,
            precio.centavos,
        )
        .await?;
        insertar_imagenes(&mut tx, empresa_id, variante_id, &imagenes).await?;
        recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(&variante_id.to_string(), Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Variante", &variante_id.to_string()))
    }

    /// H-03: edición del catálogo. Los cambios de precio base van al histórico
    /// (RN-08) y los ajustes de stock se auditan (RN-02).
    pub async fn actualizar(
        &self,
        id: &str,
        cambios: &Value,
        empresa_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let previo = self
            .encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))?;
        let tiene_variantes = previo
            .get("tiene_variantes")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if tiene_variantes
            && (cambios.get("precio_base").is_some() || cambios.get("stock").is_some())
        {
            return Err(ApiError::dominio(
                "PRODUCTO_CON_VARIANTES",
                "El precio y el stock de un producto con variantes se editan en cada variante.",
            ));
        }
        let metadatos = combinar_metadatos(&previo, cambios)?;
        if cambios.get("atributos").is_some() {
            let categoria = cambios
                .get("categoria")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    previo
                        .get("categoria")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if let Some(items) = &self.items {
                items
                    .validar_atributos(
                        &texto(&previo, "empresa_id"),
                        "producto",
                        &categoria,
                        cambios.get("atributos").cloned().unwrap_or(Value::Null),
                    )
                    .await?;
            }
        }
        self.validar_codigo_barras(
            &texto(&previo, "empresa_id"),
            &metadatos.codigo_barras,
            Some(id),
        )
        .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut sets: Vec<String> = Vec::new();
        let mut params: Vec<Param> = Vec::new();
        if let Some(nombre) = cambios.get("nombre").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "nombre",
                Param::Texto(nombre.trim().to_string()),
            );
        }
        if let Some(descripcion) = cambios.get("descripcion").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "descripcion",
                Param::Texto(descripcion.trim().to_string()),
            );
        }
        if let Some(categoria) = cambios.get("categoria").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "categoria",
                Param::Texto(categoria.trim().to_string()),
            );
        }

        agregar_set(
            &mut sets,
            &mut params,
            "costo_cents",
            Param::Entero(metadatos.costo_cents),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "marca",
            Param::Texto(metadatos.marca),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "codigo_barras",
            Param::Texto(metadatos.codigo_barras),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "unidad",
            Param::Texto(metadatos.unidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "permite_fracciones",
            Param::Booleano(metadatos.permite_fracciones),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "unidad_compra",
            Param::Texto(metadatos.unidad_compra),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "contenido_cantidad",
            Param::Entero(metadatos.contenido_cantidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "contenido_unidad",
            Param::Texto(metadatos.contenido_unidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "tipo_precio",
            Param::Texto(metadatos.tipo_precio),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "tipo_costo",
            Param::Texto(metadatos.tipo_costo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "stock_minimo",
            Param::Entero(metadatos.stock_minimo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "stock_maximo",
            Param::OpcionalEntero(metadatos.stock_maximo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "garantia_meses",
            Param::OpcionalEntero(metadatos.garantia_meses),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "es_materia_prima",
            Param::Booleano(metadatos.es_materia_prima),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "ubicacion",
            Param::Texto(metadatos.ubicacion),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "estado",
            Param::Texto(metadatos.estado_operativo),
        );
        sets.push("actualizado_en = NOW()".to_string());

        if cambios.get("atributos").is_some() {
            let atributos = cambios.get("atributos").cloned().unwrap_or(Value::Null);
            params.push(Param::Json(atributos.to_string()));
            sets.push(format!("atributos = ${}::jsonb", params.len()));
        }

        if cambios.get("precio_base").is_some() {
            let nuevo = validar_precio(cambios.get("precio_base"))?;
            agregar_set(
                &mut sets,
                &mut params,
                "precio_cents",
                Param::Entero(nuevo.centavos),
            );
            let anterior = Money::parsear(&texto(&previo, "precio_base"))?.centavos;
            registrar_historico(
                &mut tx,
                &texto(&previo, "empresa_id"),
                &texto(&previo, "id"),
                Some(anterior),
                nuevo.centavos,
            )
            .await?;
        }

        if cambios.get("stock").is_some() {
            let nuevo = validar_stock(cambios.get("stock"))?;
            agregar_set(&mut sets, &mut params, "stock", Param::Entero(nuevo));
            let anterior = previo.get("stock").and_then(Value::as_i64).unwrap_or(0);
            let motivo = cambios
                .get("motivo")
                .and_then(Value::as_str)
                .unwrap_or("ajuste manual");
            sqlx::query(
                "INSERT INTO catalog.ajustes_stock
                   (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, NOW())",
            )
            .bind(texto(&previo, "empresa_id"))
            .bind(texto(&previo, "id"))
            .bind(anterior)
            .bind(nuevo)
            .bind(motivo)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            // RN-02: las ofertas vivas deben reflejar el stock ajustado a mano.
            if let Some(outbox) = &self.outbox {
                outbox
                    .insertar_en_transaccion(
                        &mut tx,
                        "stock.updated",
                        json!({
                            "order_id": format!("ajuste-{id}"),
                            "tipo": "ajuste",
                            "items": [{
                                "empresa_id": texto(&previo, "empresa_id"),
                                "sku": texto(&previo, "sku"),
                                "stock_restante": nuevo,
                            }],
                        }),
                    )
                    .await?;
            }
        }

        params.push(Param::Texto(id.to_string()));
        if !sets.is_empty() {
            let sql = format!(
                "UPDATE catalog.items SET {} WHERE id = ${}::uuid",
                sets.join(", "),
                params.len()
            );
            let mut consulta = sqlx::query(&sql);
            for parametro in params {
                consulta = match parametro {
                    Param::Texto(valor) => consulta.bind(valor),
                    Param::Entero(valor) => consulta.bind(valor),
                    Param::OpcionalEntero(valor) => consulta.bind(valor),
                    Param::Booleano(valor) => consulta.bind(valor),
                    Param::Json(valor) => consulta.bind(valor),
                };
            }
            consulta.execute(&mut *tx).await.map_err(interno)?;
        }
        if let Some(padre_id) = previo.get("producto_padre_id").and_then(Value::as_str) {
            recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))
    }

    pub async fn actualizar_variante(
        &self,
        padre_id: &str,
        variante_id: &str,
        cambios: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        let pertenece = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(|variantes| {
                variantes
                    .iter()
                    .any(|variante| variante.get("id").and_then(Value::as_str) == Some(variante_id))
            })
            .unwrap_or(false);
        if !pertenece {
            return Err(ApiError::no_encontrado("Variante", variante_id));
        }
        self.actualizar(variante_id, cambios, Some(empresa_id))
            .await
    }

    /// Baja lógica del producto y de sus variantes (R-DB-5).
    pub async fn eliminar(&self, id: &str, empresa_id: Option<&str>) -> Result<(), ApiError> {
        self.encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))?;
        self.validar_sin_ofertas_activas(id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        match empresa_id {
            Some(empresa) => {
                sqlx::query(
                    "UPDATE catalog.items
                        SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
                      WHERE id = $1::uuid AND tipo = 'producto' AND eliminado_en IS NULL
                        AND empresa_id = $2::uuid",
                )
                .bind(id)
                .bind(empresa)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            None => {
                sqlx::query(
                    "UPDATE catalog.items
                        SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
                      WHERE id = $1::uuid AND tipo = 'producto' AND eliminado_en IS NULL",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
        }
        sqlx::query(
            "UPDATE catalog.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE parent_item_id = $1::uuid AND eliminado_en IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn eliminar_variante(
        &self,
        padre_id: &str,
        variante_id: &str,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        let pertenece = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(|variantes| {
                variantes
                    .iter()
                    .any(|variante| variante.get("id").and_then(Value::as_str) == Some(variante_id))
            })
            .unwrap_or(false);
        if !pertenece {
            return Err(ApiError::no_encontrado("Variante", variante_id));
        }
        self.validar_sin_ofertas_activas(variante_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE catalog.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND parent_item_id = $3::uuid
                AND eliminado_en IS NULL",
        )
        .bind(variante_id)
        .bind(empresa_id)
        .bind(padre_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    /// Agrega una imagen (máx. 3) a un producto o variante.
    pub async fn agregar_imagen(
        &self,
        producto_id: &str,
        imagen: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        self.encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))?;
        let imagen = imagen_de_valor(imagen);
        self.validar_imagenes(empresa_id, std::slice::from_ref(&imagen))
            .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let actuales = sqlx::query(
            "SELECT id, item_id, objeto_id, orden, es_principal
               FROM catalog.item_imagenes
              WHERE item_id = $1::uuid
              ORDER BY orden
              FOR UPDATE",
        )
        .bind(producto_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        if actuales.len() as i64 >= MAX_IMAGENES {
            return Err(ApiError::dominio(
                "LIMITE_IMAGENES",
                format!("Cada producto o variante admite hasta {MAX_IMAGENES} imágenes."),
            ));
        }
        let usados: HashSet<i64> = actuales
            .iter()
            .map(|fila| fila.get::<i32, _>("orden") as i64)
            .collect();
        let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&usados));
        if !(0..MAX_IMAGENES).contains(&orden) || usados.contains(&orden) {
            return Err(ApiError::dominio(
                "IMAGEN_ORDEN_INVALIDO",
                "El orden de la imagen no es válido.",
            ));
        }
        let principal = imagen.es_principal;
        if principal {
            sqlx::query(
                "UPDATE catalog.item_imagenes SET es_principal = false WHERE item_id = $1::uuid",
            )
            .bind(producto_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        sqlx::query(
            "INSERT INTO catalog.item_imagenes
               (id, empresa_id, item_id, objeto_id, orden, es_principal)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(empresa_id)
        .bind(producto_id)
        .bind(&imagen.objeto_id)
        .bind(orden)
        .bind(principal)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        let habia_principal = actuales
            .iter()
            .any(|fila| fila.get::<bool, _>("es_principal"));
        if !principal && !habia_principal {
            promover_primera_imagen(&mut tx, producto_id).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))
    }

    pub async fn eliminar_imagen(
        &self,
        producto_id: &str,
        imagen_id: &str,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        let producto = self
            .encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))?;
        let imagen = producto
            .get("imagenes")
            .and_then(Value::as_array)
            .and_then(|imagenes| {
                imagenes
                    .iter()
                    .find(|imagen| imagen.get("id").and_then(Value::as_str) == Some(imagen_id))
            });
        let Some(imagen) = imagen else {
            return Err(ApiError::no_encontrado("Imagen", imagen_id));
        };
        // La oferta congela el objeto de la imagen principal: si está viva,
        // borrarla dejaría una URL rota en la tienda pública.
        if imagen
            .get("es_principal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.validar_sin_ofertas_activas(producto_id).await?;
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let eliminada = sqlx::query(
            "DELETE FROM catalog.item_imagenes
              WHERE id = $1::uuid AND item_id = $2::uuid AND empresa_id = $3::uuid
              RETURNING es_principal",
        )
        .bind(imagen_id)
        .bind(producto_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if eliminada
            .as_ref()
            .map(|fila| fila.get::<bool, _>("es_principal"))
            .unwrap_or(false)
        {
            promover_primera_imagen(&mut tx, producto_id).await?;
        }
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    /// RN-03: descuento atómico de stock dentro de una transacción propia.
    pub async fn reservar_stock(&self, order_id: &str, items: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let salida = self
            .reservar_stock_en_transaccion(&mut tx, order_id, items)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(salida)
    }

    /// RN-03: descuento atómico de stock dentro de una transacción.
    ///
    /// Si alguna línea no alcanza stock se rechaza la reserva completa: se
    /// deshace el SAVEPOINT y la transacción del llamador sigue viva para que
    /// registre la idempotencia y emita `stock.fallido`.
    pub async fn reservar_stock_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = lineas_de(items);
        let lineas = self.expandir_componentes_lineas(tx, &lineas).await?;
        sqlx::query("SAVEPOINT reserva_stock")
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

        let mut fallidos: Vec<Value> = Vec::new();
        for item in &lineas {
            // Bloqueo pesimista requerido por la regla dorada (Data Integrity).
            let lock = sqlx::query(
                "SELECT i.stock, i.id AS item_id, i.track_serial, i.garantia_meses,
                        EXISTS (SELECT 1 FROM catalog.items v
                                 WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL) AS tiene_variantes
                   FROM catalog.items i
                  WHERE i.empresa_id = $1::uuid AND LOWER(i.codigo) = LOWER($2)
                    AND i.tipo = 'producto' AND i.estado = 'activo'
                    AND i.eliminado_en IS NULL FOR UPDATE",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
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
                   FROM catalog.lotes_inventario
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
                                "UPDATE catalog.lotes_inventario
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
                                "INSERT INTO catalog.reservas_lotes
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
                       FROM catalog.item_series s
                       LEFT JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
                      WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid
                        AND s.estado = 'disponible'
                      ORDER BY (u.predeterminada IS TRUE) DESC, s.creado_en, s.id
                      LIMIT $3
                      FOR UPDATE OF s",
                )
                .bind(&item.empresa_id)
                .bind(item_id)
                .bind(item.cantidad)
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
                    "UPDATE catalog.item_series
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

            sqlx::query(
                "UPDATE catalog.items
                    SET stock = stock - $3, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL
                  RETURNING codigo",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
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
            "INSERT INTO catalog.reservas_ordenes (order_id, items_json, creado_en)
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
        let stock_restante = self
            .stock_actual_de_en_transaccion(tx, &Value::Array(claves))
            .await?;
        Ok(json!({
            "ok": true,
            "fallidos": fallidos,
            "stock_restante": stock_restante,
            "reservados": lineas_json(&lineas),
        }))
    }

    /// Explota la composición (BOM) de cada línea: si el item tiene
    /// componentes se reservan las hojas multiplicadas por la cantidad vendida.
    pub async fn expandir_componentes(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = lineas_de(items);
        let expandidas = self.expandir_componentes_lineas(tx, &lineas).await?;
        Ok(lineas_json(&expandidas))
    }

    async fn expandir_componentes_lineas(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &[Linea],
    ) -> Result<Vec<Linea>, ApiError> {
        let mut acumuladas: Vec<Linea> = Vec::new();
        let mut indices: HashMap<String, usize> = HashMap::new();
        for item in items {
            let item_id = item.item_id.as_deref().unwrap_or("");
            if item_id.is_empty() {
                acumular(&mut acumuladas, &mut indices, item.clone());
                continue;
            }
            let resultado = sqlx::query(
                "WITH RECURSIVE bom(item_id, cantidad, ruta) AS (
                   SELECT c.componente_item_id, c.cantidad, ARRAY[$1::uuid, c.componente_item_id]
                     FROM catalog.item_componentes c
                    WHERE c.empresa_id = $2::uuid AND c.item_id = $1::uuid
                   UNION ALL
                   SELECT c.componente_item_id, bom.cantidad * c.cantidad, bom.ruta || c.componente_item_id
                     FROM bom
                     JOIN catalog.item_componentes c
                       ON c.item_id = bom.item_id AND c.empresa_id = $2::uuid
                    WHERE NOT (c.componente_item_id = ANY(bom.ruta))
                      AND array_length(bom.ruta, 1) < 6
                 )
                 SELECT i.id AS item_id, i.codigo AS sku, SUM(bom.cantidad)::int AS cantidad
                   FROM bom
                   JOIN catalog.items i
                     ON i.id = bom.item_id AND i.eliminado_en IS NULL AND i.tipo = 'producto'
                  WHERE NOT EXISTS (
                    SELECT 1 FROM catalog.item_componentes c2
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
                    },
                );
            }
        }
        Ok(acumuladas)
    }

    /// RN-06: devolución aprobada revierte stock por `(empresa, SKU)`.
    pub async fn reintegrar_stock(&self, order_id: &str, items: &Value) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.reintegrar_stock_en_transaccion(&mut tx, order_id, items)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    /// Marca como devueltas las unidades vendidas de una orden. Si hay
    /// devolución parcial toma primero las unidades más antiguas.
    pub async fn devolver_series_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        for item in lineas_de(items) {
            sqlx::query(
                "WITH objetivo AS (
                   SELECT s.id
                     FROM catalog.item_series s
                     JOIN catalog.items i ON i.id = s.item_id
                    WHERE s.order_id = $1::uuid AND s.estado = 'vendido'
                      AND i.empresa_id = $2::uuid AND LOWER(i.codigo) = LOWER($3)
                    ORDER BY s.vendido_en, s.id
                    LIMIT $4
                    FOR UPDATE OF s
                 )
                 UPDATE catalog.item_series s
                    SET estado = 'devuelto', devuelto_en = NOW(),
                        ubicacion_id = COALESCE((
                          SELECT u.id FROM catalog.ubicaciones u
                           WHERE u.empresa_id = $2::uuid AND u.predeterminada AND u.activo
                           LIMIT 1
                        ), s.ubicacion_id),
                        actualizado_en = NOW()
                   FROM objetivo o
                  WHERE s.id = o.id",
            )
            .bind(order_id)
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        Ok(())
    }

    pub async fn reintegrar_stock_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        let lineas = lineas_de(items);
        for item in &lineas {
            sqlx::query(
                "UPDATE catalog.items
                    SET stock = stock + $3, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        sqlx::query(
            "INSERT INTO catalog.ajustes_stock
               (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
             SELECT i.empresa_id, i.id, i.stock - x.cantidad, i.stock,
                    'devolucion orden ' || $1, NOW()
               FROM catalog.items i
               JOIN jsonb_to_recordset($2::jsonb) AS x(empresa_id uuid, sku text, cantidad int)
                 ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
              WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL",
        )
        .bind(order_id)
        .bind(lineas_json(&lineas).to_string())
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        // Restaura exactamente las asignaciones FEFO de la reserva.
        sqlx::query(
            "UPDATE catalog.lotes_inventario l
                SET cantidad_disponible = LEAST(l.cantidad_inicial, l.cantidad_disponible + r.cantidad),
                    actualizado_en = NOW()
               FROM catalog.reservas_lotes r
              WHERE r.lote_id = l.id AND r.order_id = $1::uuid",
        )
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        sqlx::query("DELETE FROM catalog.reservas_lotes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        // La reserva de esta orden ya no está activa tras reintegrar el stock.
        sqlx::query("DELETE FROM catalog.reservas_ordenes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        Ok(())
    }

    /// Claves de eventos ya procesadas (idempotencia, doc 5.2).
    pub async fn esta_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let procesado = self
            .esta_procesado_en_transaccion(&mut tx, event_id)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(procesado)
    }

    pub async fn esta_procesado_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
    ) -> Result<bool, ApiError> {
        let fila =
            sqlx::query("SELECT 1 FROM catalog.eventos_procesados WHERE event_id = $1::uuid")
                .bind(event_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn registrar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.registrar_procesado_en_transaccion(&mut tx, event_id, tipo)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn registrar_procesado_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
        tipo: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO catalog.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1::uuid, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        Ok(())
    }

    pub async fn resumen_inventario(&self, empresa_id: Option<&str>) -> Result<Value, ApiError> {
        let (filtro, params): (&str, Vec<&str>) = match empresa_id {
            Some(empresa) => (
                "WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto' AND i.eliminado_en IS NULL
                   AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)",
                vec![empresa],
            ),
            None => (
                "WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL
                   AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)",
                Vec::new(),
            ),
        };
        let sql = format!(
            "SELECT COUNT(*)::int AS productos,
                    COUNT(*) FILTER (WHERE i.stock = 0)::int AS agotados,
                    COALESCE(SUM(i.stock), 0)::int AS stock_total,
                    COALESCE(SUM(i.stock * i.precio_cents), 0)::int AS valor_inventario_cents
               FROM catalog.items i {filtro}"
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in params {
            consulta = consulta.bind(parametro);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
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

    // ── helpers de persistencia ───────────────────────────────────────────

    async fn detallar(&self, fila: &Value) -> Result<Value, ApiError> {
        let hijos = if fila
            .get("producto_padre_id")
            .map(Value::is_null)
            .unwrap_or(true)
        {
            let sql = format!(
                "SELECT {COLUMNAS_PRODUCTO}, false AS tiene_variantes
                   FROM catalog.items i
                  WHERE i.parent_item_id = $1::uuid AND i.tipo = 'producto'
                    AND i.eliminado_en IS NULL
                  ORDER BY i.creado_en ASC"
            );
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let filas = sqlx::query(&sql)
                .bind(texto(fila, "id"))
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            filas
        } else {
            Vec::new()
        };
        let mut ids = vec![texto(fila, "id")];
        let valores_hijos = hijos
            .iter()
            .map(|hijo| fila_a_json(hijo).map_err(interno))
            .collect::<Result<Vec<_>, ApiError>>()?;
        ids.extend(valores_hijos.iter().map(|hijo| texto(hijo, "id")));
        let imagenes = self.imagenes_de(&ids).await?;
        let variantes = valores_hijos
            .iter()
            .map(|hijo| {
                let id = texto(hijo, "id");
                let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                serializar(hijo, propias, Vec::new())
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        let id = texto(fila, "id");
        let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
        serializar(fila, propias, variantes)
    }

    async fn imagenes_de(&self, ids: &[String]) -> Result<HashMap<String, Vec<Value>>, ApiError> {
        let mut mapa: HashMap<String, Vec<Value>> = HashMap::new();
        if ids.is_empty() {
            return Ok(mapa);
        }
        let ids = ids
            .iter()
            .map(|id| Uuid::parse_str(id).map_err(interno))
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
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(interno)?;
            let item_id = texto(&valor, "item_id");
            mapa.entry(item_id)
                .or_default()
                .push(serializar_imagen(&valor));
        }
        Ok(mapa)
    }

    async fn validar_codigo_barras(
        &self,
        empresa_id: &str,
        codigo_barras: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if codigo_barras.is_empty() {
            return Ok(());
        }
        let mut sql = "SELECT 1 FROM catalog.items
                        WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL"
            .to_string();
        if excluir_id.is_some() {
            sql.push_str(" AND id <> $3::uuid");
        }
        sql.push_str(" LIMIT 1");
        let mut consulta = sqlx::query(&sql).bind(empresa_id).bind(codigo_barras);
        if let Some(excluir) = excluir_id {
            consulta = consulta.bind(excluir);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "El código de barras {codigo_barras} ya está registrado."
            )));
        }
        Ok(())
    }

    async fn sku_existe(&self, empresa_id: &str, sku: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM catalog.items
              WHERE empresa_id = $1::uuid AND tipo = 'producto'
                AND LOWER(codigo) = LOWER($2) AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(sku)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    async fn validar_imagenes(
        &self,
        empresa_id: &str,
        imagenes: &[ImagenInput],
    ) -> Result<(), ApiError> {
        if imagenes.len() as i64 > MAX_IMAGENES {
            return Err(ApiError::dominio(
                "LIMITE_IMAGENES",
                format!("Cada producto o variante admite hasta {MAX_IMAGENES} imágenes."),
            ));
        }
        let mut ordenes: HashSet<i64> = HashSet::new();
        let mut principales = 0usize;
        for imagen in imagenes {
            if !es_uuid(&imagen.objeto_id) {
                return Err(ApiError::dominio(
                    "IMAGEN_INVALIDA",
                    "El objeto de la imagen no es un UUID válido.",
                ));
            }
            let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&ordenes));
            if !(0..MAX_IMAGENES).contains(&orden) || ordenes.contains(&orden) {
                return Err(ApiError::dominio(
                    "IMAGEN_ORDEN_INVALIDO",
                    "El orden de la imagen no es válido.",
                ));
            }
            ordenes.insert(orden);
            if imagen.es_principal {
                principales += 1;
            }
        }
        if principales > 1 {
            return Err(ApiError::dominio(
                "IMAGEN_PRINCIPAL_UNICA",
                "Solo puede haber una imagen principal.",
            ));
        }
        let mut unicos: Vec<String> = Vec::new();
        let mut vistos: HashSet<String> = HashSet::new();
        for imagen in imagenes {
            if vistos.insert(imagen.objeto_id.clone()) {
                unicos.push(imagen.objeto_id.clone());
            }
        }
        for objeto_id in unicos {
            self.validar_objeto_imagen(empresa_id, &objeto_id).await?;
        }
        Ok(())
    }

    /// ADR-28: no se puede crear/eliminar una variante ni borrar la imagen
    /// principal mientras exista una oferta activa que la referencie.
    async fn validar_sin_ofertas_activas(&self, producto_id: &str) -> Result<(), ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(4_000))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "OFERTAS_NO_VERIFICABLES",
                    "No se pudo verificar si el producto tiene ofertas activas.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/ofertas/producto/{}/activas",
                self.stores_url,
                urlencoding::encode(producto_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "OFERTAS_NO_VERIFICABLES",
                    "No se pudo verificar si el producto tiene ofertas activas.",
                )
            })?;
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "OFERTAS_NO_VERIFICABLES",
                "No se pudo verificar si el producto tiene ofertas activas.",
            ));
        }
        let datos = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "OFERTAS_NO_VERIFICABLES",
                "No se pudo verificar si el producto tiene ofertas activas.",
            )
        })?;
        if datos.get("total").and_then(Value::as_i64).unwrap_or(0) > 0 {
            return Err(ApiError::dominio(
                "OFERTAS_ACTIVAS",
                "El producto tiene ofertas activas; desactívalas antes de cambiar sus variantes o imágenes.",
            ));
        }
        Ok(())
    }

    /// ADR-27: solo una empresa activa con la capacidad de catálogo tiene
    /// items. Falla cerrado si identity no responde (R-DB-1).
    pub async fn validar_empresa_catalogo(&self, empresa_id: &str) -> Result<(), ApiError> {
        if empresa_id.is_empty() || empresa_id == EMPRESA_LEGACY_ID {
            return Err(ApiError::dominio(
                "EMPRESA_NO_PUBLICA",
                "La empresa comodín del catálogo legado no publica productos.",
            ));
        }
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_identidad_ms))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "EMPRESA_NO_VERIFICABLE",
                    "No se pudo verificar la empresa del catálogo.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/empresas/{}",
                self.identidad_url,
                urlencoding::encode(empresa_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "EMPRESA_NO_VERIFICABLE",
                    "No se pudo verificar la empresa del catálogo.",
                )
            })?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudo verificar la empresa del catálogo.",
            ));
        }
        let empresa = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudo verificar la empresa del catálogo.",
            )
        })?;
        if empresa.get("estado").and_then(Value::as_str) != Some("activa") {
            return Err(ApiError::dominio(
                "EMPRESA_INACTIVA",
                "La empresa debe estar activa para operar su catálogo.",
            ));
        }
        let capacidades = self.capacidades_catalogo(empresa_id).await?;
        let tiene_capacidad = capacidades.iter().any(|capacidad| {
            capacidad.get("empresa_id").and_then(Value::as_str) == Some(empresa_id)
                && capacidad.get("capacidad_codigo").and_then(Value::as_str)
                    == Some("catalogo_items")
                && capacidad.get("activo").and_then(Value::as_bool) == Some(true)
        });
        if !tiene_capacidad {
            return Err(ApiError::dominio(
                "EMPRESA_SIN_CATALOGO",
                "La empresa no tiene activa la capacidad de catálogo.",
            ));
        }
        Ok(())
    }

    async fn capacidades_catalogo(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let error = || {
            ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudieron verificar las capacidades de la empresa.",
            )
        };
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_identidad_ms))
            .build()
            .map_err(|_| error())?;
        let respuesta = cliente
            .post(format!(
                "{}/internal/empresas/capacidades",
                self.identidad_url
            ))
            .header("x-internal-key", &self.clave_interna)
            .json(&json!({
                "empresa_ids": [empresa_id],
                "capacidades": ["catalogo_items"],
            }))
            .send()
            .await
            .map_err(|_| error())?;
        if !respuesta.status().is_success() {
            return Err(error());
        }
        let datos = respuesta.json::<Value>().await.map_err(|_| error())?;
        Ok(datos
            .get("capacidades")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn validar_objeto_imagen(
        &self,
        empresa_id: &str,
        objeto_id: &str,
    ) -> Result<(), ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(4_000))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "IMAGEN_NO_VERIFICABLE",
                    "No se pudo verificar la imagen en storage.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/storage/objetos/{}?tenant_id={}",
                self.storage_url,
                urlencoding::encode(objeto_id),
                urlencoding::encode(empresa_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "IMAGEN_NO_VERIFICABLE",
                    "No se pudo verificar la imagen en storage.",
                )
            })?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::dominio(
                "IMAGEN_NO_ENCONTRADA",
                "La imagen no existe en el almacenamiento.",
            ));
        }
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "IMAGEN_NO_VERIFICABLE",
                "No se pudo verificar la imagen en storage.",
            ));
        }
        let objeto = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "IMAGEN_NO_VERIFICABLE",
                "No se pudo verificar la imagen en storage.",
            )
        })?;
        if objeto.get("tenant_id").and_then(Value::as_str) != Some(empresa_id) {
            return Err(ApiError::dominio(
                "IMAGEN_OTRA_EMPRESA",
                "La imagen pertenece a otra empresa.",
            ));
        }
        if objeto.get("estado").and_then(Value::as_str) != Some("activo") {
            return Err(ApiError::dominio(
                "IMAGEN_NO_ACTIVA",
                "La imagen debe estar confirmada y activa.",
            ));
        }
        let content_type = objeto
            .get("content_type")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !content_type.starts_with("image/") || content_type == "image/svg+xml" {
            return Err(ApiError::dominio(
                "IMAGEN_TIPO_INVALIDO",
                "El objeto no es una imagen raster permitida.",
            ));
        }
        if objeto.get("publico").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::dominio(
                "IMAGEN_NO_PUBLICA",
                "La imagen debe subirse con publico=true para servirse en el catálogo.",
            ));
        }
        Ok(())
    }
}

// ── helpers puros ─────────────────────────────────────────────────────────

fn variable(clave: &str, por_defecto: &str) -> String {
    env::var(clave)
        .ok()
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| por_defecto.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_js(valor: &Value) -> String {
    match valor {
        Value::String(texto) => texto.clone(),
        Value::Number(numero) => numero.to_string(),
        Value::Bool(valor) => valor.to_string(),
        Value::Null => "null".to_string(),
        otro => otro.to_string(),
    }
}

fn valor_verdadero(valor: &Value) -> bool {
    match valor {
        Value::Null => false,
        Value::Bool(valor) => *valor,
        Value::Number(numero) => numero.as_f64().map(|valor| valor != 0.0).unwrap_or(true),
        Value::String(texto) => !texto.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn entero_json(valor: &Value) -> Option<i64> {
    match valor {
        Value::Number(numero) => numero.as_i64().or_else(|| {
            numero
                .as_f64()
                .filter(|valor| valor.is_finite() && valor.fract() == 0.0)
                .map(|valor| valor as i64)
        }),
        _ => None,
    }
}

/// Los agregados `SUM(int)` llegan como `int8`; `fila_a_json` los representa
/// como string igual que node-postgres. El contrato de producto los expone
/// como número.
fn entero_valor(valor: &Value) -> Option<i64> {
    match valor {
        Value::String(texto) => texto.trim().parse::<i64>().ok(),
        otro => entero_json(otro),
    }
}

fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(indice, byte)| {
            if matches!(indice, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn monto_de_valor(valor: &Value) -> Result<Money, ApiError> {
    match valor {
        Value::String(texto) => Money::parsear(texto),
        Value::Number(numero) => Money::parsear(&numero.to_string()),
        Value::Bool(valor) => Money::parsear(if *valor { "true" } else { "false" }),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn validar_precio(valor: Option<&Value>) -> Result<Money, ApiError> {
    let Some(valor) = valor else {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        ));
    };
    let precio = monto_de_valor(valor)?;
    if precio.centavos <= 0 {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio base debe ser mayor a 0.",
        ));
    }
    Ok(precio)
}

fn validar_stock(valor: Option<&Value>) -> Result<i64, ApiError> {
    match valor.and_then(entero_json) {
        Some(stock) if stock >= 0 => Ok(stock),
        _ => Err(ApiError::dominio(
            "STOCK_INVALIDO",
            "El stock debe ser un entero mayor o igual a 0.",
        )),
    }
}

fn normalizar_sku(valor: &str) -> Result<String, ApiError> {
    let sku = valor.trim().to_uppercase();
    let valido = (2..=32).contains(&sku.len())
        && sku.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "SKU_INVALIDO",
            "El SKU solo admite letras, numeros y guiones (2-32).",
        ));
    }
    Ok(sku)
}

fn categoria_de(datos: &Value) -> String {
    match datos.get("categoria") {
        Some(Value::String(categoria)) => categoria.clone(),
        _ => "general".to_string(),
    }
}

fn atributos_de(datos: &Value) -> Value {
    datos
        .get("atributos")
        .filter(|valor| !valor.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn paginacion_de(filtros: &Value) -> crate::paginacion::Paginacion {
    let pagina = filtros.get("pagina").map(texto_js);
    let limite = filtros.get("limite").map(texto_js);
    parsear_paginacion(pagina.as_deref(), limite.as_deref())
}

fn exigir_empresa(valor: Option<&Value>) -> Result<String, ApiError> {
    let empresa = valor.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if empresa.is_empty() {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "El catálogo se consulta por empresa (empresa_id).",
        ));
    }
    if empresa == EMPRESA_LEGACY_ID {
        return Err(ApiError::dominio(
            "EMPRESA_NO_PUBLICA",
            "La empresa comodín del catálogo legado no publica productos.",
        ));
    }
    Ok(empresa.to_string())
}

fn imagen_de_valor(valor: &Value) -> ImagenInput {
    ImagenInput {
        objeto_id: valor
            .get("objeto_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        orden: valor.get("orden").and_then(entero_json),
        es_principal: valor
            .get("es_principal")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn imagenes_de(valor: Option<&Value>) -> Vec<ImagenInput> {
    valor
        .and_then(Value::as_array)
        .map(|imagenes| imagenes.iter().map(imagen_de_valor).collect())
        .unwrap_or_default()
}

fn variantes_de(datos: &Value) -> Vec<VarianteInput> {
    datos
        .get("variantes")
        .and_then(Value::as_array)
        .map(|variantes| {
            variantes
                .iter()
                .map(|variante| VarianteInput {
                    sku: texto(variante, "sku"),
                    nombre: variante
                        .get("nombre")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    atributos: atributos_de(variante),
                    precio_base: variante.get("precio_base").cloned(),
                    stock: variante.get("stock").cloned(),
                    imagenes: imagenes_de(variante.get("imagenes")),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn siguiente_orden(usados: &HashSet<i64>) -> i64 {
    for indice in 0..MAX_IMAGENES {
        if !usados.contains(&indice) {
            return indice;
        }
    }
    -1
}

/// Reparte una cantidad entre lotes ya ordenados por FEFO, ignorando los
/// vencidos. Devuelve las asignaciones o el faltante si no alcanza.
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

fn normalizar_metadatos(datos: &Value) -> Result<Metadatos, ApiError> {
    let costo = match datos.get("costo") {
        Some(Value::Null) | None => Money::parsear("0")?,
        Some(valor) => monto_de_valor(valor)?,
    };
    let contenido_cantidad = metadato_entero(datos, "contenido_cantidad", 0)?;
    let stock_minimo = metadato_entero(datos, "stock_minimo", 0)?;
    let stock_maximo = metadato_entero_opcional(datos, "stock_maximo")?;
    let garantia_meses = metadato_entero_opcional(datos, "garantia_meses")?;
    if let Some(maximo) = stock_maximo {
        if maximo < stock_minimo {
            return Err(ApiError::dominio(
                "METADATO_INVALIDO",
                "stock_maximo no puede ser menor que stock_minimo.",
            ));
        }
    }
    Ok(Metadatos {
        costo_cents: costo.centavos,
        marca: texto(datos, "marca").trim().to_string(),
        codigo_barras: texto(datos, "codigo_barras").trim().to_string(),
        unidad: defecto(texto(datos, "unidad"), "Unidad (u)"),
        permite_fracciones: datos
            .get("permite_fracciones")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unidad_compra: defecto(texto(datos, "unidad_compra"), "Igual que la de venta"),
        contenido_cantidad,
        contenido_unidad: defecto(texto(datos, "contenido_unidad"), "Sin contenido"),
        tipo_precio: if texto(datos, "tipo_precio") == "escala" {
            "escala".to_string()
        } else {
            "fijo".to_string()
        },
        tipo_costo: if texto(datos, "tipo_costo") == "escala" {
            "escala".to_string()
        } else {
            "fijo".to_string()
        },
        stock_minimo,
        stock_maximo,
        garantia_meses,
        es_materia_prima: datos
            .get("es_materia_prima")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ubicacion: texto(datos, "ubicacion").trim().to_string(),
        estado_operativo: if texto(datos, "estado_operativo") == "inactivo" {
            "inactivo".to_string()
        } else {
            "activo".to_string()
        },
    })
}

fn defecto(valor: String, por_defecto: &str) -> String {
    let valor = valor.trim().to_string();
    if valor.is_empty() {
        por_defecto.to_string()
    } else {
        valor
    }
}

fn metadato_entero(datos: &Value, clave: &str, por_defecto: i64) -> Result<i64, ApiError> {
    match datos.get(clave) {
        Some(Value::Null) | None => Ok(por_defecto),
        Some(valor) => match entero_json(valor) {
            Some(numero) if numero >= 0 => Ok(numero),
            _ => Err(ApiError::dominio(
                "METADATO_INVALIDO",
                format!("{clave} debe ser un entero mayor o igual a 0."),
            )),
        },
    }
}

fn metadato_entero_opcional(datos: &Value, clave: &str) -> Result<Option<i64>, ApiError> {
    match datos.get(clave) {
        Some(Value::Null) | None => Ok(None),
        Some(valor) => match entero_json(valor) {
            Some(numero) if numero >= 0 => Ok(Some(numero)),
            _ => Err(ApiError::dominio(
                "METADATO_INVALIDO",
                format!("{clave} debe ser un entero mayor o igual a 0 o null."),
            )),
        },
    }
}

fn combinar_metadatos(previo: &Value, cambios: &Value) -> Result<Metadatos, ApiError> {
    let mut combinado = Map::new();
    for clave in [
        "costo",
        "marca",
        "codigo_barras",
        "unidad",
        "permite_fracciones",
        "unidad_compra",
        "contenido_cantidad",
        "contenido_unidad",
        "tipo_precio",
        "tipo_costo",
        "stock_minimo",
        "es_materia_prima",
        "ubicacion",
        "estado_operativo",
    ] {
        let valor = cambios
            .get(clave)
            .filter(|valor| !valor.is_null())
            .or_else(|| previo.get(clave))
            .cloned()
            .unwrap_or(Value::Null);
        combinado.insert(clave.to_string(), valor);
    }
    for clave in ["stock_maximo", "garantia_meses"] {
        let valor = if cambios.get(clave).is_some() {
            cambios.get(clave).cloned().unwrap_or(Value::Null)
        } else {
            previo.get(clave).cloned().unwrap_or(Value::Null)
        };
        combinado.insert(clave.to_string(), valor);
    }
    normalizar_metadatos(&Value::Object(combinado))
}

fn agregar_set(sets: &mut Vec<String>, params: &mut Vec<Param>, columna: &str, parametro: Param) {
    params.push(parametro);
    sets.push(format!("{columna} = ${}", params.len()));
}

async fn registrar_historico(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: &str,
    anterior: Option<i64>,
    nuevo: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO catalog.historico_precios
           (id, empresa_id, item_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
         VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3, $4, NOW())",
    )
    .bind(empresa_id)
    .bind(item_id)
    .bind(anterior)
    .bind(nuevo)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

/// Agrega stock y precio mínimo de las variantes al item padre.
async fn recalcular_padre(
    tx: &mut Transaction<'_, Postgres>,
    padre_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE catalog.items p
            SET stock = agregado.stock,
                precio_cents = agregado.precio,
                actualizado_en = NOW()
           FROM (
             SELECT SUM(c.stock)::int AS stock, MIN(c.precio_cents)::int AS precio
               FROM catalog.items c
              WHERE c.parent_item_id = $1::uuid AND c.eliminado_en IS NULL
           ) agregado
          WHERE p.id = $1::uuid AND agregado.stock IS NOT NULL",
    )
    .bind(padre_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

async fn insertar_imagenes(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: Uuid,
    imagenes: &[ImagenInput],
) -> Result<(), ApiError> {
    if imagenes.is_empty() {
        return Ok(());
    }
    let mut usados: HashSet<i64> = HashSet::new();
    let hay_principal = imagenes.iter().any(|imagen| imagen.es_principal);
    for imagen in imagenes {
        let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&usados));
        if !(0..MAX_IMAGENES).contains(&orden) || usados.contains(&orden) {
            return Err(ApiError::dominio(
                "IMAGEN_ORDEN_INVALIDO",
                "El orden de la imagen no es válido.",
            ));
        }
        usados.insert(orden);
        sqlx::query(
            "INSERT INTO catalog.item_imagenes
               (id, empresa_id, item_id, objeto_id, orden, es_principal)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(empresa_id)
        .bind(item_id)
        .bind(&imagen.objeto_id)
        .bind(orden)
        .bind(imagen.es_principal)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
    }
    if !hay_principal {
        promover_primera_imagen(tx, &item_id.to_string()).await?;
    }
    Ok(())
}

async fn promover_primera_imagen(
    tx: &mut Transaction<'_, Postgres>,
    item_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE catalog.item_imagenes
            SET es_principal = true
          WHERE id = (
            SELECT id FROM catalog.item_imagenes
             WHERE item_id = $1::uuid ORDER BY orden LIMIT 1
          )",
    )
    .bind(item_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

fn lineas_de(valor: &Value) -> Vec<Linea> {
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
                Value::Object(objeto)
            })
            .collect(),
    )
}

fn clave_linea(linea: &Linea) -> String {
    format!("{}:{}", linea.empresa_id, linea.sku.to_lowercase())
}

fn acumular(acumuladas: &mut Vec<Linea>, indices: &mut HashMap<String, usize>, linea: Linea) {
    let clave = clave_linea(&linea);
    match indices.get(&clave) {
        Some(indice) => acumuladas[*indice].cantidad += linea.cantidad,
        None => {
            indices.insert(clave, acumuladas.len());
            acumuladas.push(linea);
        }
    }
}

fn claves_stock_de_lineas(lineas: &[Linea]) -> Vec<Value> {
    let mut salida = Vec::new();
    let mut vistos: HashSet<String> = HashSet::new();
    for linea in lineas {
        let sku = linea.sku.to_lowercase();
        let clave = format!("{}:{sku}", linea.empresa_id);
        if vistos.insert(clave) {
            salida.push(json!({
                "empresa_id": linea.empresa_id,
                "sku": sku,
            }));
        }
    }
    salida
}

fn claves_stock(items: &Value) -> Vec<Value> {
    let mut salida = Vec::new();
    let mut vistos: HashSet<String> = HashSet::new();
    if let Some(items) = items.as_array() {
        for item in items {
            let empresa_id = texto(item, "empresa_id");
            let sku = texto(item, "sku").to_lowercase();
            let clave = format!("{empresa_id}:{sku}");
            if vistos.insert(clave) {
                salida.push(json!({
                    "empresa_id": empresa_id,
                    "sku": sku,
                }));
            }
        }
    }
    salida
}

fn fallido(linea: &Linea, motivo: &str) -> Value {
    json!({
        "empresa_id": linea.empresa_id,
        "sku": linea.sku,
        "cantidad": linea.cantidad,
        "motivo": motivo,
    })
}

fn serializar(fila: &Value, imagenes: &[Value], variantes: Vec<Value>) -> Result<Value, ApiError> {
    let precio = Money::desde_centavos(
        fila.get("precio_base_cents")
            .and_then(entero_valor)
            .unwrap_or(0),
    )?;
    let costo = Money::desde_centavos(fila.get("costo_cents").and_then(entero_valor).unwrap_or(0))?;
    let precio_desde = Money::desde_centavos(
        fila.get("precio_desde_cents")
            .and_then(entero_valor)
            .unwrap_or_else(|| {
                fila.get("precio_base_cents")
                    .and_then(entero_valor)
                    .unwrap_or(0)
            }),
    )?;
    let stock = fila.get("stock").and_then(entero_valor).unwrap_or(0);
    let mut objeto = Map::new();
    objeto.insert("id".into(), Value::String(texto(fila, "id")));
    objeto.insert(
        "empresa_id".into(),
        Value::String(texto(fila, "empresa_id")),
    );
    objeto.insert("sku".into(), Value::String(texto(fila, "sku")));
    objeto.insert("nombre".into(), Value::String(texto(fila, "nombre")));
    objeto.insert(
        "descripcion".into(),
        Value::String(texto(fila, "descripcion")),
    );
    objeto.insert("categoria".into(), Value::String(texto(fila, "categoria")));
    objeto.insert("precio_base".into(), Value::String(precio.string()));
    objeto.insert("stock".into(), json!(stock));
    objeto.insert(
        "estado".into(),
        Value::String(if stock > 0 { "disponible" } else { "agotado" }.to_string()),
    );
    objeto.insert(
        "estado_operativo".into(),
        Value::String(
            if texto(fila, "estado") == "inactivo" {
                "inactivo"
            } else {
                "activo"
            }
            .to_string(),
        ),
    );
    objeto.insert("costo".into(), Value::String(costo.string()));
    objeto.insert("marca".into(), Value::String(texto(fila, "marca")));
    objeto.insert(
        "codigo_barras".into(),
        Value::String(texto(fila, "codigo_barras")),
    );
    objeto.insert(
        "unidad".into(),
        Value::String(defecto(texto(fila, "unidad"), "Unidad (u)")),
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
        Value::String(defecto(
            texto(fila, "unidad_compra"),
            "Igual que la de venta",
        )),
    );
    objeto.insert(
        "contenido_cantidad".into(),
        json!(fila
            .get("contenido_cantidad")
            .and_then(entero_valor)
            .unwrap_or(0)),
    );
    objeto.insert(
        "contenido_unidad".into(),
        Value::String(defecto(texto(fila, "contenido_unidad"), "Sin contenido")),
    );
    objeto.insert(
        "tipo_precio".into(),
        Value::String(
            if texto(fila, "tipo_precio") == "escala" {
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
            if texto(fila, "tipo_costo") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "stock_minimo".into(),
        json!(fila.get("stock_minimo").and_then(entero_valor).unwrap_or(0)),
    );
    objeto.insert(
        "stock_maximo".into(),
        fila.get("stock_maximo").cloned().unwrap_or(Value::Null),
    );
    objeto.insert(
        "garantia_meses".into(),
        fila.get("garantia_meses").cloned().unwrap_or(Value::Null),
    );
    objeto.insert(
        "es_materia_prima".into(),
        Value::Bool(
            fila.get("es_materia_prima")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert("ubicacion".into(), Value::String(texto(fila, "ubicacion")));
    objeto.insert(
        "actualizado_en".into(),
        Value::String(if texto(fila, "actualizado_en").is_empty() {
            texto(fila, "creado_en")
        } else {
            texto(fila, "actualizado_en")
        }),
    );
    objeto.insert("creado_en".into(), Value::String(texto(fila, "creado_en")));
    objeto.insert(
        "producto_padre_id".into(),
        fila.get("producto_padre_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    objeto.insert(
        "atributos".into(),
        fila.get("atributos")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    objeto.insert(
        "tiene_variantes".into(),
        Value::Bool(
            fila.get("tiene_variantes")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || !variantes.is_empty(),
        ),
    );
    objeto.insert("imagenes".into(), Value::Array(imagenes.to_vec()));
    if !variantes.is_empty() {
        objeto.insert("variantes".into(), Value::Array(variantes));
    }
    objeto.insert("precio_desde".into(), Value::String(precio_desde.string()));
    objeto.insert(
        "stock_total".into(),
        json!(fila
            .get("stock_total")
            .and_then(entero_valor)
            .unwrap_or(stock)),
    );
    Ok(Value::Object(objeto))
}

fn serializar_imagen(fila: &Value) -> Value {
    json!({
        "id": texto(fila, "id"),
        "objeto_id": texto(fila, "objeto_id"),
        "url": format!("{URL_IMAGEN_PUBLICA}/{}", texto(fila, "objeto_id")),
        "orden": fila.get("orden").and_then(Value::as_i64).unwrap_or(0),
        "es_principal": fila.get("es_principal").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fila_producto() -> Value {
        json!({
            "id": "11111111-2222-4333-8444-555555555555",
            "empresa_id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
            "sku": "SKU-01",
            "nombre": "Teclado",
            "descripcion": "mecanico",
            "categoria": "perifericos",
            "precio_base_cents": 150000,
            "stock": 10,
            "estado": "activo",
            "creado_en": "2026-09-01T00:00:00.000Z",
            "color": null,
        })
    }

    #[test]
    fn debe_normalizar_y_validar_el_sku() {
        assert_eq!(normalizar_sku(" sku-01 ").expect("sku"), "SKU-01");
        assert_eq!(
            normalizar_sku("A").expect_err("corto").codigo,
            "SKU_INVALIDO"
        );
        assert_eq!(
            normalizar_sku("SKU@01").expect_err("caracter").codigo,
            "SKU_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_stock_y_precio() {
        assert_eq!(validar_stock(Some(&json!(10))).expect("stock"), 10);
        assert_eq!(validar_stock(Some(&json!(2.0))).expect("entero"), 2);
        assert_eq!(
            validar_stock(Some(&json!(-1)))
                .expect_err("negativo")
                .codigo,
            "STOCK_INVALIDO"
        );
        assert_eq!(
            validar_stock(Some(&json!(2.5)))
                .expect_err("decimal")
                .codigo,
            "STOCK_INVALIDO"
        );
        assert_eq!(
            validar_precio(Some(&json!("0"))).expect_err("cero").codigo,
            "PRECIO_INVALIDO"
        );
        assert_eq!(
            validar_precio(Some(&json!("1500.00")))
                .expect("precio")
                .centavos,
            150_000
        );
    }

    #[test]
    fn debe_normalizar_metadatos_con_defaults_estables() {
        let metadatos = normalizar_metadatos(&json!({
            "costo": "1000.00",
            "marca": " Core ",
            "codigo_barras": " 750 ",
            "stock_minimo": 2,
            "stock_maximo": 20,
            "garantia_meses": 12,
            "es_materia_prima": true,
            "ubicacion": " A-1 ",
            "estado_operativo": "inactivo",
        }))
        .expect("metadatos");
        assert_eq!(metadatos.costo_cents, 100_000);
        assert_eq!(metadatos.marca, "Core");
        assert_eq!(metadatos.codigo_barras, "750");
        assert_eq!(metadatos.unidad, "Unidad (u)");
        assert_eq!(metadatos.unidad_compra, "Igual que la de venta");
        assert_eq!(metadatos.contenido_unidad, "Sin contenido");
        assert_eq!(metadatos.tipo_precio, "fijo");
        assert_eq!(metadatos.tipo_costo, "fijo");
        assert_eq!(metadatos.ubicacion, "A-1");
        assert_eq!(metadatos.estado_operativo, "inactivo");
    }

    #[test]
    fn debe_rechazar_metadatos_invalidos() {
        assert_eq!(
            normalizar_metadatos(&json!({"contenido_cantidad": -1}))
                .expect_err("negativo")
                .codigo,
            "METADATO_INVALIDO"
        );
        assert_eq!(
            normalizar_metadatos(&json!({"stock_minimo": 30, "stock_maximo": 20}))
                .expect_err("rango")
                .mensaje,
            "stock_maximo no puede ser menor que stock_minimo."
        );
        assert_eq!(
            normalizar_metadatos(&json!({"garantia_meses": 1.5}))
                .expect_err("decimal")
                .codigo,
            "METADATO_INVALIDO"
        );
    }

    #[test]
    fn debe_combinar_metadatos_respetando_null_explicito() {
        let previo = serializar(&fila_producto(), &[], Vec::new()).expect("producto");
        let combinado = combinar_metadatos(
            &previo,
            &json!({"marca": "Nueva", "stock_maximo": null, "estado_operativo": "inactivo"}),
        )
        .expect("combinado");
        assert_eq!(combinado.marca, "Nueva");
        assert_eq!(combinado.stock_maximo, None);
        assert_eq!(combinado.estado_operativo, "inactivo");
        assert_eq!(combinado.unidad, "Unidad (u)");
    }

    #[test]
    fn debe_calcular_el_siguiente_orden_de_imagen() {
        assert_eq!(siguiente_orden(&HashSet::new()), 0);
        assert_eq!(siguiente_orden(&HashSet::from([0, 1])), 2);
        assert_eq!(siguiente_orden(&HashSet::from([0, 1, 2])), -1);
    }

    #[test]
    fn debe_validar_uuids_canonicos() {
        assert!(es_uuid("11111111-2222-4333-8444-555555555555"));
        assert!(es_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"));
        assert!(!es_uuid("11111111222243338444555555555555"));
        assert!(!es_uuid("no-uuid"));
    }

    #[test]
    fn debe_serializar_el_producto_con_montos_y_defaults() {
        let producto = serializar(&fila_producto(), &[], Vec::new()).expect("producto");
        assert_eq!(producto["precio_base"], "1500.00");
        assert_eq!(producto["precio_desde"], "1500.00");
        assert_eq!(producto["estado"], "disponible");
        assert_eq!(producto["estado_operativo"], "activo");
        assert_eq!(producto["unidad"], "Unidad (u)");
        assert_eq!(producto["unidad_compra"], "Igual que la de venta");
        assert_eq!(producto["contenido_unidad"], "Sin contenido");
        assert_eq!(producto["tipo_precio"], "fijo");
        assert_eq!(producto["stock_total"], 10);
        assert!(producto.get("variantes").is_none());
        assert_eq!(producto["imagenes"], json!([]));
    }

    #[test]
    fn debe_serializar_la_imagen_con_url_publica() {
        let imagen = serializar_imagen(&json!({
            "id": "33333333-4444-4555-8666-777777777777",
            "item_id": "11111111-2222-4333-8444-555555555555",
            "objeto_id": "22222222-3333-4444-8555-666666666666",
            "orden": 0,
            "es_principal": true,
        }));
        assert_eq!(
            imagen["url"],
            "/api/v2/storage/public/objetos/22222222-3333-4444-8555-666666666666"
        );
        assert_eq!(imagen["es_principal"], true);
    }

    #[test]
    fn debe_acumular_componentes_repetidos_sin_duplicar_lineas() {
        let mut acumuladas = Vec::new();
        let mut indices = HashMap::new();
        acumular(
            &mut acumuladas,
            &mut indices,
            Linea {
                empresa_id: "e-1".into(),
                sku: "HARINA".into(),
                cantidad: 1,
                item_id: Some("c-1".into()),
            },
        );
        acumular(
            &mut acumuladas,
            &mut indices,
            Linea {
                empresa_id: "e-1".into(),
                sku: "harina".into(),
                cantidad: 2,
                item_id: Some("c-1".into()),
            },
        );
        acumular(
            &mut acumuladas,
            &mut indices,
            Linea {
                empresa_id: "e-1".into(),
                sku: "SAL".into(),
                cantidad: 1,
                item_id: None,
            },
        );
        assert_eq!(acumuladas.len(), 2);
        assert_eq!(acumuladas[0].cantidad, 3);
        assert_eq!(acumuladas[1].sku, "SAL");
    }

    #[test]
    fn debe_repartir_lotes_en_orden_fefo_e_ignorar_vencidos() {
        let lote_1 = Uuid::new_v4();
        let lote_2 = Uuid::new_v4();
        let lote_vencido = Uuid::new_v4();
        let lotes = vec![
            (lote_vencido, 10, true),
            (lote_1, 3, false),
            (lote_2, 5, false),
        ];
        assert_eq!(
            planificar_fefo(6, &lotes).expect("fefo"),
            vec![(lote_1, 3), (lote_2, 3)]
        );
        assert_eq!(planificar_fefo(9, &lotes), Err(1));
        assert_eq!(planificar_fefo(0, &lotes).expect("cero"), Vec::new());
    }

    #[test]
    fn debe_exigir_series_suficientes_para_la_cantidad() {
        assert!(seriales_suficientes(3, 3));
        assert!(seriales_suficientes(4, 3));
        assert!(!seriales_suficientes(2, 3));
    }

    #[test]
    fn debe_deduplicar_claves_de_stock_por_empresa_y_sku() {
        let claves = claves_stock(&json!([
            {"empresa_id": "e-1", "sku": "SKU-01"},
            {"empresa_id": "e-1", "sku": "sku-01"},
            {"empresa_id": "e-2", "sku": "SKU-01"}
        ]));
        assert_eq!(
            claves,
            vec![
                json!({"empresa_id": "e-1", "sku": "sku-01"}),
                json!({"empresa_id": "e-2", "sku": "sku-01"}),
            ]
        );
    }

    #[test]
    fn debe_ignorar_items_inactivos_por_defecto_en_los_filtros() {
        let empresa = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
        assert_eq!(
            exigir_empresa(Some(&json!(empresa))).expect("empresa"),
            empresa
        );
        assert_eq!(
            exigir_empresa(None).expect_err("requerida").codigo,
            "EMPRESA_REQUERIDA"
        );
        assert_eq!(
            exigir_empresa(Some(&json!(EMPRESA_LEGACY_ID)))
                .expect_err("legado")
                .codigo,
            "EMPRESA_NO_PUBLICA"
        );
    }

    #[test]
    fn debe_leer_lineas_y_conservar_item_id_solo_si_viene() {
        let lineas = lineas_de(&json!([
            {"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 2, "item_id": "i-1"},
            {"empresa_id": "e-1", "sku": "SKU-02", "cantidad": 1, "item_id": ""}
        ]));
        assert_eq!(lineas[0].item_id.as_deref(), Some("i-1"));
        assert_eq!(lineas[1].item_id, None);
        assert_eq!(
            lineas_json(&lineas),
            json!([
                {"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 2, "item_id": "i-1"},
                {"empresa_id": "e-1", "sku": "SKU-02", "cantidad": 1}
            ])
        );
    }
}
