//! Operaciones avanzadas del catálogo (R-PS-04).
//!
//! Réplica de `operaciones.service.ts`: importación de productos, escaneo por
//! código y combos. Las reglas de cálculo (filas válidas/omitidas, costos y
//! márgenes de combos) viven en funciones puras para poder probarlas sin base
//! de datos. Precios, promociones e inventario pertenecen a `pricing-service`
//! e `inventory-service` (ADR-0018).
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
use crate::proyecciones::Proyecciones;
use crate::servicios::Servicios;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_IMPORTACION: usize = 200;
const SELECT_COMBO: &str = "SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.codigo_barras, i.estado,
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
 FROM item.items i
 LEFT JOIN item.item_componentes ic ON ic.item_id = i.id
 LEFT JOIN item.items ci
   ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL";

#[derive(Debug, Clone, PartialEq)]
struct ItemResuelto {
    id: String,
    sku: String,
    nombre: String,
    #[allow(dead_code)]
    costo_cents: i64,
    es_combo: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponenteResuelto {
    item_id: String,
    sku: String,
    nombre: String,
    cantidad: i64,
}

/// Operaciones de catálogo por empresa: importación, escaneo y combos.
pub struct Operaciones {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    productos: Arc<Productos>,
    servicios: Arc<Servicios>,
    proyecciones: Option<Arc<Proyecciones>>,
}

impl Operaciones {
    pub fn nueva(
        db: Arc<Db>,
        outbox: Option<Arc<Outbox>>,
        proyecciones: Option<Arc<Proyecciones>>,
    ) -> Self {
        let items = Arc::new(match proyecciones.clone() {
            Some(proyecciones) => Items::con_proyecciones(db.clone(), proyecciones),
            None => Items::nueva(db.clone()),
        });
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
            proyecciones,
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
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM item.items i {where_clause}");
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
                "INSERT INTO item.items
                   (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                    costo_cents, track_inventory, atributos, estado,
                    codigo_barras, es_combo)
                 VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, 'combo', 0, false,
                         '{}'::jsonb, $6, $7, true)",
            )
            .bind(&id)
            .bind(empresa_id)
            .bind(&sku)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&estado)
            .bind(&codigo_barras)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO item.item_componentes
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
        if let Some(proyecciones) = &self.proyecciones {
            if let Err(error) = proyecciones
                .fijar_precio(empresa_id, &id, precio.centavos)
                .await
            {
                let mut tx = self.db.transaccion().await.map_err(interno)?;
                sqlx::query("DELETE FROM item.items WHERE id = $1::uuid")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await
                    .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                return Err(error);
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
        let nuevo_precio = datos
            .get("precio")
            .map(|_| monto_de(datos.get("precio")))
            .transpose()?
            .map(|monto| monto.centavos);
        let estado = match datos.get("estado") {
            Some(_) => estado_operativo(datos),
            None => texto(&actual, "estado"),
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ejecucion: Result<(), sqlx::Error> = async {
            let actualizado = sqlx::query(
                "UPDATE item.items
                    SET nombre = $3, descripcion = $4, codigo_barras = $5,
                        estado = $6, actualizado_en = NOW()
                  WHERE id = $1::uuid AND empresa_id = $2::uuid
                    AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL",
            )
            .bind(id)
            .bind(empresa_id)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&codigo_barras)
            .bind(&estado)
            .execute(&mut *tx)
            .await?;
            if actualizado.rows_affected() == 0 {
                return Err(sqlx::Error::RowNotFound);
            }
            sqlx::query(
                "DELETE FROM item.item_componentes WHERE item_id = $1::uuid AND empresa_id = $2::uuid",
            )
            .bind(id)
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO item.item_componentes
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
        if let (Some(precio_cents), Some(proyecciones)) = (nuevo_precio, &self.proyecciones) {
            proyecciones
                .fijar_precio(empresa_id, id, precio_cents)
                .await?;
        }
        self.obtener_combo(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Combo", id))
    }

    pub async fn eliminar_combo(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE item.items SET eliminado_en = NOW(), actualizado_en = NOW()
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
                "SELECT id, costo_cents, nombre FROM item.items
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
            resultado.push(json!({
                "id": fila.get("id").cloned().unwrap_or(Value::Null),
                "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
                "sku": fila.get("sku").cloned().unwrap_or(Value::Null),
                "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
                "descripcion": fila.get("descripcion").cloned().unwrap_or(Value::Null),
                "codigo_barras": fila.get("codigo_barras").cloned().unwrap_or(Value::Null),
                "costo_total": Money::desde_centavos(costo_total)?.string(),
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
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL
                    AND id <> $3::uuid"
            }
            None => {
                "SELECT 1 FROM item.items
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
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL
                    AND id <> $3::uuid
                  LIMIT 1"
            }
            None => {
                "SELECT 1 FROM item.items
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
                "SELECT id, codigo AS sku, nombre, costo_cents, es_combo
                   FROM item.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto' AND id = $2::uuid
                    AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, es_combo
                   FROM item.items
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
        Ok(ItemResuelto {
            id: id.to_string(),
            sku: fila.try_get("sku").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            costo_cents: costo_cents.unwrap_or(0) as i64,
            es_combo: fila.try_get("es_combo").map_err(interno)?,
        })
    }
}

// ── funciones puras (probadas sin base de datos) ─────────────────────────

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
    fn debe_interpretar_cantidades_como_number_de_javascript() {
        assert_eq!(entero_de(Some(&json!(5))), Some(5));
        assert_eq!(entero_de(Some(&json!(5.0))), Some(5));
        assert_eq!(entero_de(Some(&json!("5"))), Some(5));
        assert_eq!(entero_de(Some(&json!(""))), Some(0));
        assert_eq!(entero_de(Some(&json!(1.5))), None);
    }
}
