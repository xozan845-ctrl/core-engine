//! Operaciones avanzadas del catálogo (R-PS-04).
//!
//! Réplica de `operaciones.service.ts`: configuración de inventario,
//! importación de productos, escaneo por código, combos, promociones y
//! movimientos/traslados. Las reglas de cálculo (filas válidas/omitidas,
//! costos y márgenes de combos, vigencias y cantidades de promociones, stock
//! resultante de cada movimiento) viven en funciones puras para poder
//! probarlas sin base de datos.
//!
//! Reutiliza los módulos hermanos ya portados: `Productos::crear` (con
//! `empresa_validada`), `Productos::validar_empresa_catalogo`,
//! `Productos::encontrar_por_codigo_barras` y
//! `Servicios::encontrar_por_codigo`. Todos reciben y devuelven `Value` para
//! conservar el contrato JSON.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use crate::money::Money;
use crate::outbox::Outbox;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use crate::productos::Productos;
use crate::servicios::Servicios;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_IMPORTACION: usize = 200;
const MAX_LINEAS: usize = 200;
const UNIDAD_DEFECTO: &str = "Unidad (u)";
const EVENTO_STOCK_UPDATED: &str = "stock.updated";
const METODOS_COSTEO: [&str; 3] = ["promedio", "fifo", "ueps"];
const TIPOS_PROMOCION: [&str; 4] = [
    "2x1",
    "descuento_cantidad",
    "descuento_porcentaje",
    "regalo",
];
const TIPOS_MOVIMIENTO: [&str; 4] = ["entrada", "salida", "ajuste", "traslado"];

const COLUMNAS_CONFIGURACION: &str = "empresa_id, metodo_costeo, permitir_stock_negativo,
  alerta_stock_bajo, actualizar_costo_en_compra, unidad_defecto, actualizado_en";

const SELECT_COMBO: &str = "SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.codigo_barras, i.precio_cents, i.estado,
  COALESCE(
    jsonb_agg(
      jsonb_build_object(
        'item_id', ci.id,
        'sku', ci.codigo,
        'nombre', ci.nombre,
        'cantidad', ic.cantidad
      )
      ORDER BY ci.nombre
    ) FILTER (WHERE ic.id IS NOT NULL),
    '[]'::jsonb
  ) AS componentes,
  i.creado_en, i.actualizado_en
 FROM catalog.items i
 LEFT JOIN catalog.item_componentes ic ON ic.item_id = i.id
 LEFT JOIN catalog.items ci
   ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL";

const COLUMNAS_PROMOCION: &str = "id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";

const RETURNING_PROMOCION: &str = "RETURNING id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";

const COLUMNAS_MOVIMIENTO: &str = "id, empresa_id, tipo, motivo, referencia,
  ubicacion_origen, ubicacion_destino, lineas, creado_por, creado_en";

/// Operaciones de catálogo por empresa.
pub struct Operaciones {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    productos: Arc<Productos>,
    servicios: Arc<Servicios>,
}

#[derive(Debug, Clone, PartialEq)]
struct ItemResuelto {
    id: String,
    sku: String,
    nombre: String,
    #[allow(dead_code)]
    costo_cents: i64,
    stock: i64,
    es_combo: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponenteResuelto {
    item_id: String,
    sku: String,
    nombre: String,
    cantidad: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct PromocionNormalizada {
    nombre: String,
    tipo: String,
    item_id: Option<String>,
    sku: String,
    cantidad_requerida: Option<i64>,
    cantidad_gratis: Option<i64>,
    descuento_porcentaje: Option<i64>,
    fecha_desde: String,
    fecha_hasta: String,
    estado: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CabeceraMovimiento {
    tipo: String,
    origen: String,
    destino: String,
}

impl Operaciones {
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>) -> Self {
        // Productos y servicios comparten la misma instancia de Items para que
        // la validación de atributos (JSON Schema) sea consistente.
        let items = Arc::new(Items::nueva(db.clone()));
        let productos = Arc::new(Productos::nueva(
            db.clone(),
            outbox.clone(),
            Some(items.clone()),
        ));
        let servicios = Arc::new(Servicios::con_productos(
            db.clone(),
            productos.clone(),
            Some(items),
        ));
        Self {
            productos,
            servicios,
            db,
            outbox,
        }
    }

    // ── configuración ────────────────────────────────────────────────────

    pub async fn configuracion(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_CONFIGURACION}
               FROM catalog.configuracion_inventario WHERE empresa_id = $1::uuid"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => serializar_configuracion(&fila_a_json(&fila).map_err(interno)?),
            None => Ok(configuracion_por_defecto(empresa_id)),
        }
    }

    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        cambios: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.configuracion(empresa_id).await?;
        let metodo = cambios
            .get("metodo_costeo")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                actual
                    .get("metodo_costeo")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            });
        if !METODOS_COSTEO.contains(&metodo) {
            return Err(ApiError::dominio(
                "METODO_COSTEO_INVALIDO",
                "El método de costeo no es válido.",
            ));
        }
        let booleano = |clave: &str| {
            cambios
                .get(clave)
                .and_then(Value::as_bool)
                .unwrap_or_else(|| actual.get(clave).and_then(Value::as_bool).unwrap_or(false))
        };
        let unidad = cambios
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_else(|| {
                actual
                    .get("unidad_defecto")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            })
            .to_string();
        let unidad = if unidad.is_empty() {
            UNIDAD_DEFECTO.to_string()
        } else {
            unidad
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
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
             RETURNING {COLUMNAS_CONFIGURACION}"
        ))
        .bind(empresa_id)
        .bind(metodo)
        .bind(booleano("permitir_stock_negativo"))
        .bind(booleano("alerta_stock_bajo"))
        .bind(booleano("actualizar_costo_en_compra"))
        .bind(&unidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => serializar_configuracion(&fila_a_json(&fila).map_err(interno)?),
            None => Err(ApiError::dominio(
                "CONFIGURACION_NO_GUARDADA",
                "No se pudo guardar la configuración.",
            )),
        }
    }

    // ── importación ──────────────────────────────────────────────────────

    pub async fn importar(&self, empresa_id: &str, filas: &[Value]) -> Result<Value, ApiError> {
        if filas.is_empty() {
            return Err(ApiError::dominio(
                "IMPORTACION_VACIA",
                "La importación no contiene filas.",
            ));
        }
        if filas.len() > MAX_IMPORTACION {
            return Err(ApiError::dominio(
                "IMPORTACION_DEMASIADO_GRANDE",
                format!("El máximo es {MAX_IMPORTACION} filas por importación."),
            ));
        }
        self.productos.validar_empresa_catalogo(empresa_id).await?;
        let mut importados = 0usize;
        let mut omitidos = 0usize;
        let mut errores: Vec<String> = Vec::new();
        let opciones = json!({ "empresa_validada": true });
        for (indice, fila) in filas.iter().enumerate() {
            let datos = datos_producto_importado(fila, empresa_id);
            match self.productos.crear(&datos, Some(&opciones)).await {
                Ok(_) => importados += 1,
                Err(error) if error.codigo == "CONFLICTO" => omitidos += 1,
                Err(error) => {
                    errores.push(format!("Fila {}: {}", indice + 1, error.mensaje));
                }
            }
        }
        Ok(json!({
            "importados": importados,
            "omitidos": omitidos,
            "errores": errores,
        }))
    }

    // ── escaneo ──────────────────────────────────────────────────────────

    pub async fn escanear(&self, empresa_id: &str, codigo: &str) -> Result<Value, ApiError> {
        let valor = codigo.trim();
        if valor.is_empty() {
            return Err(ApiError::dominio(
                "CODIGO_REQUERIDO",
                "Indica un código para escanear.",
            ));
        }
        if let Some(producto) = self
            .productos
            .encontrar_por_codigo_barras(valor, empresa_id, false)
            .await?
        {
            return Ok(json!({
                "tipo": "producto",
                "producto": producto,
                "servicio": Value::Null,
            }));
        }
        if let Some(servicio) = self
            .servicios
            .encontrar_por_codigo(valor, empresa_id)
            .await?
        {
            return Ok(json!({
                "tipo": "servicio",
                "producto": Value::Null,
                "servicio": servicio,
            }));
        }
        Err(ApiError::no_encontrado("Código", valor))
    }

    // ── combos ───────────────────────────────────────────────────────────

    pub async fn listar_combos(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.tipo = 'producto'".to_string(),
            "i.es_combo".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        if let Some(q) = filtro_texto(filtros, "q").filter(|valor| !valor.is_empty()) {
            params.push(format!("%{}%", q.to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0}
                  OR LOWER(i.codigo_barras) LIKE ${0})",
                params.len()
            ));
        }
        if let Some(estado) = filtro_texto(filtros, "estado").filter(|valor| !valor.is_empty()) {
            params.push(estado);
            condiciones.push(format!("i.estado = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM catalog.items i {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");

        let sql = format!(
            "{SELECT_COMBO}
             {where_clause}
             GROUP BY i.id
             ORDER BY i.creado_en DESC
             LIMIT ${} OFFSET ${}",
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

        let mut filas_json = Vec::with_capacity(filas.len());
        for fila in &filas {
            filas_json.push(fila_a_json(fila).map_err(interno)?);
        }
        let combos = self.hidratar_combos(filas_json).await?;
        Ok(crear_pagina(combos, total as i64, paginacion))
    }

    pub async fn obtener_combo(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "{SELECT_COMBO}
             WHERE i.id = $1::uuid AND i.empresa_id = $2::uuid
               AND i.tipo = 'producto' AND i.es_combo AND i.eliminado_en IS NULL
             GROUP BY i.id"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let valor = fila_a_json(&fila).map_err(interno)?;
        Ok(self.hidratar_combos(vec![valor]).await?.into_iter().next())
    }

    pub async fn crear_combo(&self, empresa_id: &str, datos: &Value) -> Result<Value, ApiError> {
        let sku = normalizar_sku(datos.get("sku").and_then(Value::as_str).unwrap_or(""))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto_de(datos.get("precio"))?;
        let componentes = self
            .resolver_componentes(empresa_id, datos.get("componentes"))
            .await?;
        let codigo_barras = texto_opcional(datos, "codigo_barras").unwrap_or_default();
        self.validar_combo_unico(empresa_id, &sku, &codigo_barras, None)
            .await?;

        let id = Uuid::new_v4().to_string();
        let descripcion = texto_opcional(datos, "descripcion").unwrap_or_default();
        let estado = estado_operativo(datos);
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ejecucion: Result<(), sqlx::Error> = async {
            sqlx::query(
                "INSERT INTO catalog.items
                   (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                    precio_cents, costo_cents, track_inventory, stock, atributos, estado,
                    codigo_barras, es_combo)
                 VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, 'combo', $6, 0, false,
                         NULL, '{}'::jsonb, $7, $8, true)",
            )
            .bind(&id)
            .bind(empresa_id)
            .bind(&sku)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(precio.centavos)
            .bind(&estado)
            .bind(&codigo_barras)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO catalog.item_componentes
                       (empresa_id, item_id, componente_item_id, cantidad)
                     VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
                )
                .bind(empresa_id)
                .bind(&id)
                .bind(&componente.item_id)
                .bind(componente.cantidad)
                .execute(&mut *tx)
                .await?;
            }
            Ok(())
        }
        .await;
        match ejecucion {
            Ok(()) => tx.commit().await.map_err(interno)?,
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "Ya existe un item con el SKU {sku}."
                    )));
                }
                return Err(interno(error));
            }
        }
        self.obtener_combo(&id, empresa_id)
            .await?
            .ok_or_else(ApiError::interno)
    }

    pub async fn actualizar_combo(
        &self,
        id: &str,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self
            .obtener_combo(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Combo", id))?;

        let componentes_input = match datos.get("componentes") {
            Some(valor) => valor.clone(),
            None => Value::Array(
                actual
                    .get("componentes")
                    .and_then(Value::as_array)
                    .map(|componentes| {
                        componentes
                            .iter()
                            .map(|componente| {
                                json!({
                                    "item_id": componente.get("item_id"),
                                    "sku": componente.get("sku"),
                                    "cantidad": componente.get("cantidad"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
        };
        let componentes = self
            .resolver_componentes(empresa_id, Some(&componentes_input))
            .await?;

        let codigo_actual = actual
            .get("codigo_barras")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let codigo_barras = match datos.get("codigo_barras") {
            Some(_) => texto_opcional(datos, "codigo_barras").unwrap_or_default(),
            None => codigo_actual.clone(),
        };
        if codigo_barras != codigo_actual {
            self.validar_combo_unico(empresa_id, actual_sku(&actual), &codigo_barras, Some(id))
                .await?;
        }

        let nombre = match datos.get("nombre") {
            Some(_) => validar_nombre(&texto(datos, "nombre"))?,
            None => texto(&actual, "nombre"),
        };
        let descripcion = match datos.get("descripcion") {
            Some(_) => texto_opcional(datos, "descripcion").unwrap_or_default(),
            None => texto(&actual, "descripcion"),
        };
        let precio_cents = match datos.get("precio") {
            Some(_) => monto_de(datos.get("precio"))?.centavos,
            None => Money::parsear(&texto(&actual, "precio"))?.centavos,
        };
        let estado = match datos.get("estado") {
            Some(_) => estado_operativo(datos),
            None => texto(&actual, "estado"),
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ejecucion: Result<(), sqlx::Error> = async {
            let actualizado = sqlx::query(
                "UPDATE catalog.items
                    SET nombre = $3, descripcion = $4, codigo_barras = $5,
                        precio_cents = $6, estado = $7, actualizado_en = NOW()
                  WHERE id = $1::uuid AND empresa_id = $2::uuid
                    AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL",
            )
            .bind(id)
            .bind(empresa_id)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&codigo_barras)
            .bind(precio_cents)
            .bind(&estado)
            .execute(&mut *tx)
            .await?;
            if actualizado.rows_affected() == 0 {
                return Err(sqlx::Error::RowNotFound);
            }
            sqlx::query(
                "DELETE FROM catalog.item_componentes WHERE item_id = $1::uuid AND empresa_id = $2::uuid",
            )
            .bind(id)
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO catalog.item_componentes
                       (empresa_id, item_id, componente_item_id, cantidad)
                     VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
                )
                .bind(empresa_id)
                .bind(id)
                .bind(&componente.item_id)
                .bind(componente.cantidad)
                .execute(&mut *tx)
                .await?;
            }
            Ok(())
        }
        .await;
        match ejecucion {
            Ok(()) => tx.commit().await.map_err(interno)?,
            Err(sqlx::Error::RowNotFound) => {
                let _ = tx.rollback().await;
                return Err(ApiError::no_encontrado("Combo", id));
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "El código de barras {codigo_barras} ya está registrado."
                    )));
                }
                return Err(interno(error));
            }
        }
        self.obtener_combo(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Combo", id))
    }

    pub async fn eliminar_combo(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.items SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Combo", id));
        }
        Ok(())
    }

    // ── promociones ──────────────────────────────────────────────────────

    pub async fn listar_promociones(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec![
            "empresa_id = $1::uuid".to_string(),
            "eliminado_en IS NULL".to_string(),
        ];
        if let Some(estado) = filtro_texto(filtros, "estado").filter(|valor| !valor.is_empty()) {
            params.push(estado);
            condiciones.push(format!("estado = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM catalog.promociones {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let sql = format!(
            "SELECT {COLUMNAS_PROMOCION}
               FROM catalog.promociones {where_clause}
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

        let mut promociones = Vec::with_capacity(filas.len());
        for fila in &filas {
            promociones.push(serializar_promocion(&fila_a_json(fila).map_err(interno)?));
        }
        Ok(crear_pagina(promociones, total as i64, paginacion))
    }

    pub async fn obtener_promocion(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PROMOCION}
               FROM catalog.promociones
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| fila_a_json(&fila).map(|valor| serializar_promocion(&valor)))
            .transpose()
            .map_err(interno)
    }

    pub async fn crear_promocion(
        &self,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let normalizada = self.normalizar_promocion(empresa_id, datos).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.promociones
               (empresa_id, nombre, tipo, item_id, sku, cantidad_requerida, cantidad_gratis,
                descuento_porcentaje, fecha_desde, fecha_hasta, estado)
             VALUES ($1::uuid, $2, $3, $4::uuid, $5, $6, $7, $8, $9::date, $10::date, $11)
             {RETURNING_PROMOCION}"
        ))
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar_promocion(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::dominio(
                "PROMOCION_NO_CREADA",
                "No se pudo crear la promoción.",
            )),
        }
    }

    pub async fn actualizar_promocion(
        &self,
        id: &str,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self
            .obtener_promocion(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Promoción", id))?;

        let mut combinada = Map::new();
        for clave in ["nombre", "tipo", "sku", "estado"] {
            let valor = datos
                .get(clave)
                .filter(|valor| !valor.is_null())
                .cloned()
                .or_else(|| actual.get(clave).cloned())
                .unwrap_or(Value::Null);
            combinada.insert(clave.to_string(), valor);
        }
        let item_id = datos
            .get("item_id")
            .filter(|valor| !valor.is_null())
            .cloned()
            .or_else(|| {
                actual
                    .get("item_id")
                    .filter(|valor| !valor.is_null())
                    .cloned()
            })
            .unwrap_or(Value::Null);
        combinada.insert("item_id".into(), item_id);
        for clave in [
            "cantidad_requerida",
            "cantidad_gratis",
            "descuento_porcentaje",
            "fecha_desde",
            "fecha_hasta",
        ] {
            let valor = match datos.get(clave) {
                Some(valor) => valor.clone(),
                None => actual.get(clave).cloned().unwrap_or(Value::Null),
            };
            combinada.insert(clave.to_string(), valor);
        }

        let normalizada = self
            .normalizar_promocion(empresa_id, &Value::Object(combinada))
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE catalog.promociones SET
               nombre = $3, tipo = $4, item_id = $5::uuid, sku = $6, cantidad_requerida = $7,
               cantidad_gratis = $8, descuento_porcentaje = $9, fecha_desde = $10::date,
               fecha_hasta = $11::date, estado = $12, actualizado_en = NOW()
             WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
             {RETURNING_PROMOCION}"
        ))
        .bind(id)
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar_promocion(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::no_encontrado("Promoción", id)),
        }
    }

    pub async fn eliminar_promocion(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.promociones SET eliminado_en = NOW(), actualizado_en = NOW()
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
            return Err(ApiError::no_encontrado("Promoción", id));
        }
        Ok(())
    }

    // ── movimientos de inventario ────────────────────────────────────────

    pub async fn movimientos(&self, empresa_id: &str, filtros: &Value) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec!["empresa_id = $1::uuid".to_string()];
        if let Some(tipo) = filtro_texto(filtros, "tipo").filter(|valor| !valor.is_empty()) {
            params.push(tipo);
            condiciones.push(format!("tipo = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM catalog.movimientos_inventario {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let sql = format!(
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM catalog.movimientos_inventario {where_clause}
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

        let mut movimientos = Vec::with_capacity(filas.len());
        for fila in &filas {
            movimientos.push(serializar_movimiento(&fila_a_json(fila).map_err(interno)?));
        }
        Ok(crear_pagina(movimientos, total as i64, paginacion))
    }

    pub async fn crear_movimiento(
        &self,
        empresa_id: &str,
        datos: &Value,
        usuario_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let cabecera = validar_cabecera_movimiento(datos)?;
        let config = self.configuracion(empresa_id).await?;
        let permitir_stock_negativo = config
            .get("permitir_stock_negativo")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let lineas = datos
            .get("lineas")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let productos = self.resolver_productos(empresa_id, &lineas).await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut lineas_salida: Vec<Value> = Vec::with_capacity(lineas.len());
        let mut stock_items: Vec<Value> = Vec::new();
        for (indice, linea) in lineas.iter().enumerate() {
            let producto = &productos[indice];
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
            let anterior = producto.stock;
            let nuevo = calcular_stock(&cabecera.tipo, anterior, cantidad);
            if nuevo < 0 && !permitir_stock_negativo {
                return Err(ApiError::dominio(
                    "STOCK_INSUFICIENTE",
                    format!(
                        "Stock insuficiente para {} (disponible {}).",
                        producto.sku, anterior
                    ),
                ));
            }
            if cabecera.tipo != "traslado" {
                sqlx::query(
                    "UPDATE catalog.items
                        SET stock = $3, actualizado_en = NOW()
                      WHERE id = $1::uuid AND empresa_id = $2::uuid AND tipo = 'producto'
                        AND eliminado_en IS NULL",
                )
                .bind(&producto.id)
                .bind(empresa_id)
                .bind(nuevo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
                let motivo = datos
                    .get("motivo")
                    .filter(|valor| !valor.is_null())
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("movimiento {}", cabecera.tipo));
                sqlx::query(
                    "INSERT INTO catalog.ajustes_stock
                       (id, empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo,
                        realizado_en)
                     VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3, $4, $5, NOW())",
                )
                .bind(empresa_id)
                .bind(&producto.id)
                .bind(anterior)
                .bind(nuevo)
                .bind(motivo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
                stock_items.push(json!({
                    "empresa_id": empresa_id,
                    "sku": producto.sku,
                    "stock_restante": nuevo,
                }));
            }
            lineas_salida.push(json!({
                "item_id": producto.id,
                "sku": producto.sku,
                "nombre": producto.nombre,
                "cantidad": cantidad,
                "stock_anterior": anterior,
                "stock_nuevo": nuevo,
            }));
        }

        let motivo = texto_opcional(datos, "motivo").unwrap_or_default();
        let referencia = texto_opcional(datos, "referencia").unwrap_or_default();
        let lineas_json = Value::Array(lineas_salida).to_string();
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.movimientos_inventario
               (empresa_id, tipo, motivo, referencia, ubicacion_origen, ubicacion_destino,
                lineas, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7::jsonb, $8)
             RETURNING {COLUMNAS_MOVIMIENTO}"
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
        let movimiento_id = movimiento
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if let Some(outbox) = &self.outbox {
            if !stock_items.is_empty() {
                let tipo_evento = match cabecera.tipo.as_str() {
                    "entrada" => "entrada",
                    "salida" => "salida",
                    _ => "ajuste",
                };
                outbox
                    .insertar_en_transaccion(
                        &mut tx,
                        EVENTO_STOCK_UPDATED,
                        json!({
                            "order_id": format!("movimiento-{movimiento_id}"),
                            "tipo": tipo_evento,
                            "items": stock_items,
                        }),
                    )
                    .await?;
            }
        }
        tx.commit().await.map_err(interno)?;
        Ok(serializar_movimiento(&movimiento))
    }

    // ── helpers ──────────────────────────────────────────────────────────

    async fn hidratar_combos(&self, filas: Vec<Value>) -> Result<Vec<Value>, ApiError> {
        let mut ids: Vec<String> = Vec::new();
        for fila in &filas {
            let componentes = fila
                .get("componentes")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for componente in componentes {
                if let Some(item_id) = componente.get("item_id").and_then(Value::as_str) {
                    if !ids.iter().any(|existente| existente == item_id) {
                        ids.push(item_id.to_string());
                    }
                }
            }
        }

        let mut items: HashMap<String, (i64, String)> = HashMap::new();
        let uuids: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();
        if !uuids.is_empty() {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let filas_items = sqlx::query(
                "SELECT id, costo_cents, nombre FROM catalog.items
                  WHERE id = ANY($1::uuid[]) AND eliminado_en IS NULL",
            )
            .bind(&uuids)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            for fila in filas_items {
                let id: Uuid = fila.try_get("id").map_err(interno)?;
                let costo: Option<i32> = fila.try_get("costo_cents").map_err(interno)?;
                let nombre: Option<String> = fila.try_get("nombre").map_err(interno)?;
                items.insert(
                    id.to_string(),
                    (costo.unwrap_or(0) as i64, nombre.unwrap_or_default()),
                );
            }
        }

        let mut resultado = Vec::with_capacity(filas.len());
        for fila in &filas {
            let componentes_fila = fila
                .get("componentes")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut componentes = Vec::with_capacity(componentes_fila.len());
            let mut costo_total: i64 = 0;
            for componente in componentes_fila {
                let item_id = componente
                    .get("item_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let cantidad = componente
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let (costo, nombre_item) = items
                    .get(&item_id)
                    .cloned()
                    .unwrap_or_else(|| (0, texto(componente, "nombre")));
                let subtotal = costo.saturating_mul(cantidad);
                costo_total = costo_total.saturating_add(subtotal);
                componentes.push(json!({
                    "item_id": item_id,
                    "sku": componente.get("sku").cloned().unwrap_or(Value::Null),
                    "nombre": nombre_item,
                    "cantidad": cantidad,
                    "costo_unitario": Money::desde_centavos(costo)?.string(),
                    "subtotal": Money::desde_centavos(subtotal)?.string(),
                }));
            }
            let precio_cents = fila
                .get("precio_cents")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            resultado.push(json!({
                "id": fila.get("id").cloned().unwrap_or(Value::Null),
                "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
                "sku": fila.get("sku").cloned().unwrap_or(Value::Null),
                "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
                "descripcion": fila.get("descripcion").cloned().unwrap_or(Value::Null),
                "codigo_barras": fila.get("codigo_barras").cloned().unwrap_or(Value::Null),
                "precio": Money::desde_centavos(precio_cents)?.string(),
                "costo_total": Money::desde_centavos(costo_total)?.string(),
                "margen": calcular_margen(precio_cents, costo_total),
                "estado": estado_serializado(fila.get("estado")),
                "componentes": componentes,
                "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
                "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(resultado)
    }

    async fn resolver_componentes(
        &self,
        empresa_id: &str,
        componentes: Option<&Value>,
    ) -> Result<Vec<ComponenteResuelto>, ApiError> {
        let Some(componentes) = componentes.and_then(Value::as_array) else {
            return Err(ApiError::dominio(
                "COMBO_SIN_COMPONENTES",
                "El combo requiere al menos un componente.",
            ));
        };
        if componentes.is_empty() {
            return Err(ApiError::dominio(
                "COMBO_SIN_COMPONENTES",
                "El combo requiere al menos un componente.",
            ));
        }
        let mut vistos: Vec<String> = Vec::new();
        let mut resultado = Vec::with_capacity(componentes.len());
        for componente in componentes {
            let cantidad = cantidad_componente(componente)?;
            let item = self
                .buscar_item(
                    empresa_id,
                    componente.get("item_id").and_then(Value::as_str),
                    componente.get("sku").and_then(Value::as_str).unwrap_or(""),
                )
                .await?;
            if item.es_combo {
                return Err(ApiError::dominio(
                    "COMPONENTE_INVALIDO",
                    "Un combo no puede incluir otro combo; usa sus productos base.",
                ));
            }
            registrar_componente_unico(&mut vistos, &item.id)?;
            resultado.push(ComponenteResuelto {
                item_id: item.id,
                sku: item.sku,
                nombre: item.nombre,
                cantidad,
            });
        }
        Ok(resultado)
    }

    async fn validar_combo_unico(
        &self,
        empresa_id: &str,
        sku: &str,
        codigo_barras: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_combo = match excluir_id {
            Some(_) => {
                "SELECT 1 FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL
                    AND id <> $3::uuid"
            }
            None => {
                "SELECT 1 FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL"
            }
        };
        let mut consulta = sqlx::query(sql_combo).bind(empresa_id).bind(sku);
        if let Some(excluir_id) = excluir_id {
            consulta = consulta.bind(excluir_id);
        }
        let combo = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        if combo.is_some() {
            return Err(ApiError::conflicto(format!(
                "Ya existe un item con el SKU {sku}."
            )));
        }
        if codigo_barras.is_empty() {
            return Ok(());
        }
        let sql_barcode = match excluir_id {
            Some(_) => {
                "SELECT 1 FROM catalog.items
                  WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL
                    AND id <> $3::uuid
                  LIMIT 1"
            }
            None => {
                "SELECT 1 FROM catalog.items
                  WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL
                  LIMIT 1"
            }
        };
        let mut consulta = sqlx::query(sql_barcode)
            .bind(empresa_id)
            .bind(codigo_barras);
        if let Some(excluir_id) = excluir_id {
            consulta = consulta.bind(excluir_id);
        }
        let barcode = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if barcode.is_some() {
            return Err(ApiError::conflicto(format!(
                "El código de barras {codigo_barras} ya está registrado."
            )));
        }
        Ok(())
    }

    async fn normalizar_promocion(
        &self,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<PromocionNormalizada, ApiError> {
        let mut normalizada = validar_promocion(datos)?;
        let item_id = datos
            .get("item_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        let sku = datos
            .get("sku")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        if item_id.is_some() || sku.is_some() {
            let item = self
                .buscar_item(empresa_id, item_id, sku.unwrap_or(""))
                .await?;
            normalizada.item_id = Some(item.id);
            normalizada.sku = item.sku;
        }
        Ok(normalizada)
    }

    async fn resolver_productos(
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
                "SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
                   FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto' AND id = $2::uuid
                    AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
                   FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL",
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
        let costo_cents: Option<i32> = fila.try_get("costo_cents").map_err(interno)?;
        let stock: Option<i32> = fila.try_get("stock").map_err(interno)?;
        Ok(ItemResuelto {
            id: id.to_string(),
            sku: fila.try_get("sku").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            costo_cents: costo_cents.unwrap_or(0) as i64,
            stock: stock.unwrap_or(0) as i64,
            es_combo: fila.try_get("es_combo").map_err(interno)?,
        })
    }
}

// ── funciones puras (probadas sin base de datos) ─────────────────────────

fn configuracion_por_defecto(empresa_id: &str) -> Value {
    json!({
        "empresa_id": empresa_id,
        "metodo_costeo": "promedio",
        "permitir_stock_negativo": false,
        "alerta_stock_bajo": true,
        "actualizar_costo_en_compra": true,
        "unidad_defecto": UNIDAD_DEFECTO,
        "actualizado_en": "1970-01-01T00:00:00.000Z",
    })
}

fn serializar_configuracion(fila: &Value) -> Result<Value, ApiError> {
    Ok(json!({
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "metodo_costeo": fila.get("metodo_costeo").cloned().unwrap_or(Value::Null),
        "permitir_stock_negativo": fila
            .get("permitir_stock_negativo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "alerta_stock_bajo": fila
            .get("alerta_stock_bajo")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "actualizar_costo_en_compra": fila
            .get("actualizar_costo_en_compra")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "unidad_defecto": fila
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .unwrap_or(UNIDAD_DEFECTO),
        "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
    }))
}

/// Construye el `datos` que consume `Productos::crear`, conservando el orden y
/// los campos opcionales de `ImportarProductoFila`.
fn datos_producto_importado(fila: &Value, empresa_id: &str) -> Value {
    let mut datos = Map::new();
    datos.insert("empresa_id".into(), Value::String(empresa_id.to_string()));
    for clave in [
        "sku",
        "nombre",
        "descripcion",
        "categoria",
        "precio_base",
        "stock",
        "costo",
        "marca",
        "codigo_barras",
        "unidad",
        "stock_minimo",
        "stock_maximo",
        "es_materia_prima",
        "ubicacion",
    ] {
        if let Some(valor) = fila.get(clave) {
            if !valor.is_null() {
                datos.insert(clave.to_string(), valor.clone());
            }
        }
    }
    Value::Object(datos)
}

fn serializar_promocion(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "item_id": fila.get("item_id").cloned().unwrap_or(Value::Null),
        "sku": fila.get("sku").and_then(Value::as_str).unwrap_or(""),
        "cantidad_requerida": fila.get("cantidad_requerida").cloned().unwrap_or(Value::Null),
        "cantidad_gratis": fila.get("cantidad_gratis").cloned().unwrap_or(Value::Null),
        "descuento_porcentaje": fila
            .get("descuento_porcentaje")
            .cloned()
            .unwrap_or(Value::Null),
        "fecha_desde": fecha_corta(fila.get("fecha_desde")),
        "fecha_hasta": fecha_corta(fila.get("fecha_hasta")),
        "estado": estado_serializado(fila.get("estado")),
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
        "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
    })
}

fn serializar_movimiento(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "motivo": fila.get("motivo").and_then(Value::as_str).unwrap_or(""),
        "referencia": fila.get("referencia").and_then(Value::as_str).unwrap_or(""),
        "ubicacion_origen": fila
            .get("ubicacion_origen")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "ubicacion_destino": fila
            .get("ubicacion_destino")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "lineas": fila.get("lineas").cloned().unwrap_or_else(|| json!([])),
        "creado_por": fila.get("creado_por").cloned().unwrap_or(Value::Null),
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
    })
}

fn validar_promocion(datos: &Value) -> Result<PromocionNormalizada, ApiError> {
    let nombre = validar_nombre(&texto(datos, "nombre"))?;
    let tipo = datos
        .get("tipo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !TIPOS_PROMOCION.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_PROMOCION_INVALIDO",
            "El tipo de promoción no es válido.",
        ));
    }
    let mut cantidad_requerida = entero_de(datos.get("cantidad_requerida"));
    let mut cantidad_gratis = entero_de(datos.get("cantidad_gratis"));
    let descuento = entero_de(datos.get("descuento_porcentaje"));
    match tipo.as_str() {
        "2x1" => {
            cantidad_requerida = Some(2);
            cantidad_gratis = Some(1);
        }
        "regalo" => {
            if cantidad_requerida.unwrap_or(0) < 1 || cantidad_gratis.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "La promoción de regalo requiere cantidades.",
                ));
            }
        }
        "descuento_cantidad" => {
            if cantidad_requerida.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "Indica la cantidad requerida.",
                ));
            }
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
        _ => {
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
    }
    let fecha_desde = fecha_corta(datos.get("fecha_desde"));
    let fecha_hasta = fecha_corta(datos.get("fecha_hasta"));
    if fecha_desde.is_empty() || fecha_hasta.is_empty() || fecha_hasta < fecha_desde {
        return Err(ApiError::dominio(
            "VIGENCIA_INVALIDA",
            "La vigencia de la promoción no es válida.",
        ));
    }
    Ok(PromocionNormalizada {
        nombre,
        tipo,
        item_id: None,
        sku: String::new(),
        cantidad_requerida,
        cantidad_gratis,
        descuento_porcentaje: descuento,
        fecha_desde,
        fecha_hasta,
        estado: estado_operativo(datos),
    })
}

fn validar_cabecera_movimiento(datos: &Value) -> Result<CabeceraMovimiento, ApiError> {
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

fn cantidad_componente(componente: &Value) -> Result<i64, ApiError> {
    entero_de(componente.get("cantidad"))
        .filter(|cantidad| *cantidad >= 1)
        .ok_or_else(|| {
            ApiError::dominio(
                "CANTIDAD_INVALIDA",
                "La cantidad del componente debe ser un entero positivo.",
            )
        })
}

fn registrar_componente_unico(vistos: &mut Vec<String>, item_id: &str) -> Result<(), ApiError> {
    if vistos.iter().any(|visto| visto == item_id) {
        return Err(ApiError::dominio(
            "COMPONENTE_DUPLICADO",
            "Un producto no puede repetirse en el combo.",
        ));
    }
    vistos.push(item_id.to_string());
    Ok(())
}

fn calcular_stock(tipo: &str, anterior: i64, cantidad: i64) -> i64 {
    match tipo {
        "entrada" => anterior.saturating_add(cantidad),
        "salida" => anterior.saturating_sub(cantidad),
        "ajuste" => cantidad,
        _ => anterior,
    }
}

/// `Math.round(((precio - costo) / precio) * 1000) / 10`, con el redondeo de
/// JavaScript (mitad hacia +∞) para conservar el `margen` del contrato.
fn calcular_margen(precio_cents: i64, costo_total_cents: i64) -> f64 {
    if precio_cents <= 0 {
        return 0.0;
    }
    let proporcion = (precio_cents - costo_total_cents) as f64 / precio_cents as f64;
    redondear_js(proporcion * 1000.0) / 10.0
}

fn redondear_js(valor: f64) -> f64 {
    if valor.is_sign_negative() {
        -((-valor + 0.5).floor())
    } else {
        (valor + 0.5).floor()
    }
}

fn descuento_valido(descuento: Option<i64>) -> bool {
    matches!(descuento, Some(valor) if (1..=100).contains(&valor))
}

fn normalizar_sku(valor: &str) -> Result<String, ApiError> {
    let sku = valor.trim().to_uppercase();
    let longitud = sku.chars().count();
    let valido = (2..=32).contains(&longitud)
        && sku.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "SKU_INVALIDO",
            "El SKU solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(sku)
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

fn monto_de(valor: Option<&Value>) -> Result<Money, ApiError> {
    match valor {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        Some(otro) => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            format!(
                "Formato de monto invalido: \"{}\". Use numero con hasta 2 decimales.",
                otro
            ),
        )),
        None => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
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

fn fecha_corta(valor: Option<&Value>) -> String {
    let texto = match valor {
        Some(Value::String(texto)) => texto.clone(),
        Some(Value::Null) | None => String::new(),
        Some(otro) => otro.to_string(),
    };
    texto.chars().take(10).collect()
}

fn estado_operativo(datos: &Value) -> String {
    if datos.get("estado").and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

fn estado_serializado(valor: Option<&Value>) -> String {
    if valor.and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

fn actual_sku(actual: &Value) -> &str {
    actual.get("sku").and_then(Value::as_str).unwrap_or("")
}

fn es_uuid(valor: &str) -> bool {
    Uuid::parse_str(valor).is_ok()
}

fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => matches!(db.code().as_deref(), Some("23505") | Some("23P01")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_mapear_filas_de_importacion_sin_campos_ausentes() {
        let fila = json!({
            "sku": "SKU-1",
            "nombre": "Producto",
            "precio_base": "10.00",
            "stock": 5,
            "costo": "4.50",
            "codigo_barras": "750",
            "stock_maximo": null
        });
        let datos = datos_producto_importado(&fila, "empresa-1");
        assert_eq!(datos["empresa_id"], "empresa-1");
        assert_eq!(datos["sku"], "SKU-1");
        assert_eq!(datos["precio_base"], "10.00");
        assert_eq!(datos["costo"], "4.50");
        assert!(datos.get("descripcion").is_none());
        assert!(datos.get("stock_maximo").is_none());
    }

    #[test]
    fn debe_normalizar_sku_y_validar_nombres() {
        assert_eq!(normalizar_sku(" sku-1 ").expect("sku"), "SKU-1");
        assert_eq!(
            normalizar_sku("a").expect_err("corto").codigo,
            "SKU_INVALIDO"
        );
        assert_eq!(
            normalizar_sku("skú").expect_err("simbolo").codigo,
            "SKU_INVALIDO"
        );
        assert_eq!(validar_nombre(" Producto ").expect("nombre"), "Producto");
        assert_eq!(
            validar_nombre("x").expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
    }

    #[test]
    fn debe_calcular_el_margen_del_combo_como_el_contrato() {
        // precio 30.00, costo 20.00 -> 33.3
        assert_eq!(calcular_margen(3000, 2000), 33.3);
        // precio 0 -> 0 sin dividir entre cero.
        assert_eq!(calcular_margen(0, 2000), 0.0);
        // Sin costo -> 100 %.
        assert_eq!(calcular_margen(3000, 0), 100.0);
        // Costo mayor que el precio: margen negativo redondeado como JS.
        assert_eq!(calcular_margen(3000, 3010), -0.3);
    }

    #[test]
    fn debe_validar_promociones_y_normalizar_2x1() {
        let dos_por_uno = validar_promocion(&json!({
            "nombre": "2x1",
            "tipo": "2x1",
            "fecha_desde": "2026-10-01T00:00:00.000Z",
            "fecha_hasta": "2026-12-31",
        }))
        .expect("2x1");
        assert_eq!(dos_por_uno.cantidad_requerida, Some(2));
        assert_eq!(dos_por_uno.cantidad_gratis, Some(1));
        assert_eq!(dos_por_uno.fecha_desde, "2026-10-01");

        let regalo_sin_cantidades = validar_promocion(&json!({
            "nombre": "Regalo",
            "tipo": "regalo",
            "fecha_desde": "2026-10-01",
            "fecha_hasta": "2026-12-31",
        }))
        .expect_err("regalo");
        assert_eq!(regalo_sin_cantidades.codigo, "PROMOCION_INVALIDA");

        let descuento_invalido = validar_promocion(&json!({
            "nombre": "Descuento",
            "tipo": "descuento_porcentaje",
            "descuento_porcentaje": 200,
            "fecha_desde": "2026-10-01",
            "fecha_hasta": "2026-12-31",
        }))
        .expect_err("descuento");
        assert_eq!(descuento_invalido.codigo, "PROMOCION_INVALIDA");

        let vigencia_invertida = validar_promocion(&json!({
            "nombre": "Promo",
            "tipo": "2x1",
            "fecha_desde": "2026-12-31",
            "fecha_hasta": "2026-10-01",
        }))
        .expect_err("vigencia");
        assert_eq!(vigencia_invertida.codigo, "VIGENCIA_INVALIDA");

        let tipo_invalido = validar_promocion(&json!({
            "nombre": "Promo",
            "tipo": "otro",
            "fecha_desde": "2026-10-01",
            "fecha_hasta": "2026-12-31",
        }))
        .expect_err("tipo");
        assert_eq!(tipo_invalido.codigo, "TIPO_PROMOCION_INVALIDO");

        let descuento_cantidad = validar_promocion(&json!({
            "nombre": "Descuento",
            "tipo": "descuento_cantidad",
            "cantidad_requerida": 3,
            "descuento_porcentaje": 10,
            "fecha_desde": "2026-10-01",
            "fecha_hasta": "2026-12-31",
        }))
        .expect("descuento_cantidad");
        assert_eq!(descuento_cantidad.cantidad_requerida, Some(3));
        assert_eq!(descuento_cantidad.descuento_porcentaje, Some(10));
    }

    #[test]
    fn debe_validar_cantidades_y_duplicados_de_componentes() {
        assert_eq!(
            cantidad_componente(&json!({"cantidad": 2})).expect("cantidad"),
            2
        );
        assert_eq!(
            cantidad_componente(&json!({"cantidad": 0}))
                .expect_err("cero")
                .codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            cantidad_componente(&json!({"cantidad": 1.5}))
                .expect_err("fraccion")
                .codigo,
            "CANTIDAD_INVALIDA"
        );

        let mut vistos = Vec::new();
        registrar_componente_unico(&mut vistos, "item-1").expect("primero");
        assert_eq!(
            registrar_componente_unico(&mut vistos, "item-1")
                .expect_err("duplicado")
                .codigo,
            "COMPONENTE_DUPLICADO"
        );
    }

    #[test]
    fn debe_calcular_el_stock_de_cada_movimiento() {
        assert_eq!(calcular_stock("entrada", 10, 5), 15);
        assert_eq!(calcular_stock("salida", 10, 5), 5);
        assert_eq!(calcular_stock("ajuste", 10, 3), 3);
        assert_eq!(calcular_stock("traslado", 10, 5), 10);
    }

    #[test]
    fn debe_validar_cabeceras_de_movimiento() {
        let cabecera = validar_cabecera_movimiento(&json!({
            "tipo": "traslado",
            "ubicacion_origen": "A",
            "ubicacion_destino": "B",
            "lineas": [{"sku": "SKU-1", "cantidad": 1}],
        }))
        .expect("traslado");
        assert_eq!(cabecera.tipo, "traslado");
        assert_eq!(cabecera.origen, "A");

        let sin_lineas = validar_cabecera_movimiento(&json!({
            "tipo": "entrada",
            "lineas": [],
        }))
        .expect_err("sin lineas");
        assert_eq!(sin_lineas.codigo, "LINEAS_REQUERIDAS");

        let traslado_invalido = validar_cabecera_movimiento(&json!({
            "tipo": "traslado",
            "ubicacion_origen": "A",
            "ubicacion_destino": "A",
            "lineas": [{"sku": "SKU-1", "cantidad": 1}],
        }))
        .expect_err("traslado invalido");
        assert_eq!(traslado_invalido.codigo, "TRASLADO_INVALIDO");

        let demasiadas = validar_cabecera_movimiento(&json!({
            "tipo": "entrada",
            "lineas": (0..201).map(|_| json!({"sku": "SKU-1", "cantidad": 1}))
                .collect::<Vec<_>>(),
        }))
        .expect_err("demasiadas");
        assert_eq!(demasiadas.codigo, "DEMASIADAS_LINEAS");
    }

    #[test]
    fn debe_interpretar_cantidades_como_number_de_javascript() {
        assert_eq!(entero_de(Some(&json!(5))), Some(5));
        assert_eq!(entero_de(Some(&json!(5.0))), Some(5));
        assert_eq!(entero_de(Some(&json!("5"))), Some(5));
        assert_eq!(entero_de(Some(&json!(""))), Some(0));
        assert_eq!(entero_de(Some(&json!(1.5))), None);
    }
}
