//! Listas de precio y escalas por cantidad (Fase P1.1, ADR-0001).
//!
//! Réplica de `items/precios.service.ts`. Una lista agrupa precios por segmento
//! (mayorista, VIP, convenio) y puede ser la predeterminada de la empresa.
//! `lista_id NULL` es la escala base del item: es la que da contenido real a
//! `items.tipo_precio = 'escala'`.
//!
//! La resolución es una lectura: no reserva stock ni crea órdenes. Los
//! servicios que cotizan (orders, stores) pueden degradar al precio de la
//! oferta si el catálogo no responde.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use std::sync::Arc;

const CODIGO: &str = r"^[a-z][a-z0-9_-]{1,39}$";
const SEGMENTO: &str = r"^[a-z][a-z0-9_-]{0,39}$";
const MAX_CANTIDAD: i64 = 1_000_000;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

const COLUMNAS_LISTA: &str = "id, empresa_id, codigo, nombre, descripcion, moneda,
  segmento_codigo, predeterminada, activo, creado_en, actualizado_en";
const COLUMNAS_PRECIO: &str = "id, empresa_id, lista_id, item_id, desde_cantidad,
  hasta_cantidad, precio_cents, creado_en, actualizado_en";

const MENSAJE_CONFLICTO_LISTA: &str = "La lista ya existe o el segmento ya tiene una lista activa.";
const MENSAJE_CONFLICTO_ESCALA: &str = "La escala se solapa con otra del mismo item.";

pub struct Precios {
    db: Arc<Db>,
}

impl Precios {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    // ── listas ─────────────────────────────────────────────────────────────

    pub async fn listas(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM catalog.listas_precio
              WHERE empresa_id = $1
              ORDER BY predeterminada DESC, nombre"
        ))
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear_lista(&self, empresa_id: &str, input: &Value) -> Result<Value, ApiError> {
        let codigo = validar_codigo(&texto(input, "codigo"), "codigo")?;
        let nombre = validar_nombre(&texto(input, "nombre"))?;
        let descripcion = recortar(texto(input, "descripcion"), 500);
        let moneda = match input.get("moneda") {
            None | Some(Value::Null) => "NIO".to_string(),
            Some(valor) => texto_js(valor),
        };
        let moneda = validar_moneda(&moneda)?;
        let segmento = validar_segmento(&texto(input, "segmento_codigo"))?;
        let predeterminada = input
            .get("predeterminada")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            if predeterminada {
                sqlx::query(
                    "UPDATE catalog.listas_precio
                        SET predeterminada = false, actualizado_en = NOW()
                      WHERE empresa_id = $1 AND predeterminada",
                )
                .bind(empresa_id)
                .execute(&mut *tx)
                .await?;
            }
            let fila = sqlx::query(&format!(
                "INSERT INTO catalog.listas_precio
                   (empresa_id, codigo, nombre, descripcion, moneda, segmento_codigo, predeterminada)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 RETURNING {COLUMNAS_LISTA}"
            ))
            .bind(empresa_id)
            .bind(&codigo)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&moneda)
            .bind(&segmento)
            .bind(predeterminada)
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
                    "LISTA_NO_CREADA",
                    "No se pudo crear la lista.",
                ))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_LISTA))
            }
        }
    }

    pub async fn actualizar_lista(
        &self,
        empresa_id: &str,
        lista_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_lista(empresa_id, lista_id).await?;

        let codigo = match input.get("codigo") {
            Some(valor) => validar_codigo(&texto_js(valor), "codigo")?,
            None => texto_de(&actual, "codigo"),
        };
        let nombre = match input.get("nombre") {
            Some(valor) => validar_nombre(&texto_js(valor))?,
            None => texto_de(&actual, "nombre"),
        };
        let descripcion = match input.get("descripcion") {
            Some(valor) => recortar(texto_js(valor).trim().to_string(), 500),
            None => texto_de(&actual, "descripcion"),
        };
        let moneda = match input.get("moneda") {
            Some(valor) => validar_moneda(&texto_js(valor))?,
            None => texto_de(&actual, "moneda"),
        };
        let segmento = match input.get("segmento_codigo") {
            Some(valor) => validar_segmento(&texto_js(valor))?,
            None => texto_de(&actual, "segmento_codigo"),
        };
        let activo_actual = booleano_de(&actual, "activo");
        let activo = match input.get("activo") {
            Some(valor) => valor.as_bool().unwrap_or(false),
            None => activo_actual,
        };
        let predeterminada_actual = booleano_de(&actual, "predeterminada");
        let predeterminada = match input.get("predeterminada") {
            Some(valor) => valor.as_bool().unwrap_or(false),
            None => predeterminada_actual,
        } && activo;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            if predeterminada && !predeterminada_actual {
                sqlx::query(
                    "UPDATE catalog.listas_precio
                        SET predeterminada = false, actualizado_en = NOW()
                      WHERE empresa_id = $1 AND predeterminada AND id <> $2",
                )
                .bind(empresa_id)
                .bind(lista_id)
                .execute(&mut *tx)
                .await?;
            }
            let fila = sqlx::query(&format!(
                "UPDATE catalog.listas_precio
                    SET codigo = $1, nombre = $2, descripcion = $3, moneda = $4,
                        segmento_codigo = $5, predeterminada = $6, activo = $7,
                        actualizado_en = NOW()
                  WHERE id = $8 AND empresa_id = $9
                  RETURNING {COLUMNAS_LISTA}"
            ))
            .bind(&codigo)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&moneda)
            .bind(&segmento)
            .bind(predeterminada)
            .bind(activo)
            .bind(lista_id)
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
                Err(ApiError::no_encontrado("Lista de precios", lista_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_LISTA))
            }
        }
    }

    // ── precios de una lista ───────────────────────────────────────────────

    pub async fn precios(&self, empresa_id: &str, lista_id: &str) -> Result<Value, ApiError> {
        self.buscar_lista(empresa_id, lista_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM catalog.lista_precio_items
              WHERE empresa_id = $1 AND lista_id = $2
              ORDER BY item_id, desde_cantidad"
        ))
        .bind(empresa_id)
        .bind(lista_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        input: &Value,
    ) -> Result<Value, ApiError> {
        if let Some(lista) = lista_id {
            if !lista.is_empty() {
                self.buscar_lista(empresa_id, lista).await?;
            }
        }
        let item = self
            .verificar_item(empresa_id, &texto(input, "item_id"))
            .await?;
        let desde_por_defecto = json!(1);
        let desde = input.get("desde_cantidad").unwrap_or(&desde_por_defecto);
        let hasta = input.get("hasta_cantidad").filter(|valor| !valor.is_null());
        let Some(precio) = input.get("precio_cents") else {
            return Err(ApiError::dominio(
                "PRECIO_INVALIDO",
                "El precio debe ser un entero no negativo.",
            ));
        };
        let rango = validar_rango_cantidad(desde, hasta, precio)?;

        let lista_bd = lista_id.filter(|valor| !valor.is_empty());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "INSERT INTO catalog.lista_precio_items
                   (empresa_id, lista_id, item_id, desde_cantidad, hasta_cantidad, precio_cents)
                 VALUES ($1, $2::uuid, $3, $4::bigint, $5::bigint, $6::bigint)
                 RETURNING {COLUMNAS_PRECIO}"
            ))
            .bind(empresa_id)
            .bind(lista_bd)
            .bind(&item.id)
            .bind(rango.desde)
            .bind(rango.hasta)
            .bind(rango.precio)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        let fila = match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)?
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                return Err(ApiError::dominio(
                    "PRECIO_NO_CREADO",
                    "No se pudo guardar el precio.",
                ));
            }
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(error_conflicto(error, MENSAJE_CONFLICTO_ESCALA));
            }
        };

        if lista_bd.is_none() {
            self.marcar_escala(empresa_id, &item.id).await?;
        }
        Ok(fila)
    }

    pub async fn actualizar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, lista_id, precio_id).await?;

        let desde_actual = json!(entero_de(&actual, "desde_cantidad").unwrap_or(1));
        let desde = input
            .get("desde_cantidad")
            .filter(|valor| !valor.is_null())
            .unwrap_or(&desde_actual);

        let hasta_actual = actual.get("hasta_cantidad").and_then(Value::as_i64);
        let hasta_actual_valor = hasta_actual.map(|valor| json!(valor));
        let hasta = match input.get("hasta_cantidad") {
            Some(Value::Null) => None,
            Some(valor) => Some(valor),
            None => hasta_actual_valor.as_ref(),
        };

        let precio_actual = json!(entero_de(&actual, "precio_cents").unwrap_or(0));
        let precio = input
            .get("precio_cents")
            .filter(|valor| !valor.is_null())
            .unwrap_or(&precio_actual);

        let rango = validar_rango_cantidad(desde, hasta, precio)?;
        let lista_bd = lista_id;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "UPDATE catalog.lista_precio_items
                    SET desde_cantidad = $1::bigint, hasta_cantidad = $2::bigint,
                        precio_cents = $3::bigint, actualizado_en = NOW()
                  WHERE id = $4 AND empresa_id = $5
                    AND lista_id IS NOT DISTINCT FROM $6::uuid
                  RETURNING {COLUMNAS_PRECIO}"
            ))
            .bind(rango.desde)
            .bind(rango.hasta)
            .bind(rango.precio)
            .bind(precio_id)
            .bind(empresa_id)
            .bind(lista_bd)
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
                Err(ApiError::no_encontrado("Precio", precio_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_ESCALA))
            }
        }
    }

    pub async fn eliminar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, lista_id, precio_id).await?;
        let lista_bd = lista_id;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "DELETE FROM catalog.lista_precio_items
              WHERE id = $1 AND empresa_id = $2
                AND lista_id IS NOT DISTINCT FROM $3::uuid
              RETURNING id",
        )
        .bind(precio_id)
        .bind(empresa_id)
        .bind(lista_bd)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if fila.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Precio", precio_id));
        }
        tx.commit().await.map_err(interno)?;

        if lista_bd.is_none() {
            self.actualizar_tipo_precio(empresa_id, &texto_de(&actual, "item_id"))
                .await?;
        }
        Ok(json!({ "ok": true }))
    }

    // ── escalas base del item ───────────────────────────────────────────────

    pub async fn escalas(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM catalog.lista_precio_items
              WHERE empresa_id = $1 AND item_id = $2 AND lista_id IS NULL
              ORDER BY desde_cantidad"
        ))
        .bind(empresa_id)
        .bind(&item.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let mut entrada = Map::new();
        if let Value::Object(objeto) = input {
            entrada = objeto.clone();
        }
        entrada.insert("item_id".into(), Value::String(item_id.to_string()));
        self.crear_precio(empresa_id, None, &Value::Object(entrada))
            .await
    }

    pub async fn actualizar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        escala_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, None, escala_id).await?;
        if texto_de(&actual, "item_id") != item_id {
            return Err(ApiError::no_encontrado("Escala", escala_id));
        }
        self.actualizar_precio(empresa_id, None, escala_id, input)
            .await
    }

    pub async fn eliminar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        escala_id: &str,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, None, escala_id).await?;
        if texto_de(&actual, "item_id") != item_id {
            return Err(ApiError::no_encontrado("Escala", escala_id));
        }
        self.eliminar_precio(empresa_id, None, escala_id).await
    }

    // ── resolución ──────────────────────────────────────────────────────────

    /// Resuelve el precio unitario y total de un item para una cantidad.
    ///
    /// `filtros` admite `lista_codigo` y/o `segmento_codigo`; sin filtros se usa
    /// la lista predeterminada activa y, si no hay, la escala base del item.
    pub async fn precio_de(
        &self,
        empresa_id: &str,
        item_id: &str,
        cantidad: &Value,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let cantidad = validar_cantidad(cantidad)?;
        let item = self.verificar_item(empresa_id, item_id).await?;
        let lista = self.resolver_lista(empresa_id, filtros).await?;
        let lista_id = lista
            .as_ref()
            .and_then(|valor| valor.get("id"))
            .and_then(Value::as_str);
        let escala = self
            .buscar_escala(empresa_id, &item.id, lista_id, cantidad)
            .await?;
        let origen = if escala.is_some() {
            if lista.is_some() {
                "lista"
            } else {
                "escala"
            }
        } else {
            "base"
        };
        let precio_unitario = escala
            .as_ref()
            .and_then(|valor| entero_de(valor, "precio_cents"))
            .unwrap_or(item.precio_cents);
        let precio_total = multiplicar(precio_unitario, cantidad)?;

        Ok(json!({
            "item_id": item.id,
            "cantidad": cantidad,
            "lista": lista,
            "origen": origen,
            "escala": escala,
            "precio_unitario_cents": precio_unitario,
            "precio_total_cents": precio_total,
        }))
    }

    /// Resuelve precios por lote con el shape del endpoint interno:
    /// `{empresa_id, lista_codigo?, segmento_codigo?, lineas:[{clave,item_id,cantidad}]}`.
    pub async fn precios_lote(&self, input: &Value) -> Result<Value, ApiError> {
        let empresa = input
            .get("empresa_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !es_uuid(&empresa) {
            return Err(ApiError::dominio(
                "EMPRESA_REQUERIDA",
                "Indica una empresa válida.",
            ));
        }
        let lineas = input
            .get("lineas")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ApiError::validacion(json!([
                    {"campo": "lineas", "restricciones": ["isArray", "arrayMinSize"]}
                ]))
            })?;
        if lineas.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lineas", "restricciones": ["arrayMinSize"]}
            ])));
        }

        let filtros = json!({
            "lista_codigo": input.get("lista_codigo").cloned().unwrap_or(Value::Null),
            "segmento_codigo": input.get("segmento_codigo").cloned().unwrap_or(Value::Null),
        });
        let mut precios = Vec::with_capacity(lineas.len());
        for linea in lineas {
            let clave = linea.get("clave").map(texto_js).unwrap_or_default();
            let item_id = linea.get("item_id").map(texto_js).unwrap_or_default();
            let cantidad = linea.get("cantidad").cloned().unwrap_or(Value::Null);
            let resuelto = self
                .precio_de(&empresa, &item_id, &cantidad, &filtros)
                .await?;
            precios.push(json!({
                "clave": clave,
                "origen": resuelto.get("origen").cloned().unwrap_or(Value::Null),
                "precio_unitario_cents": resuelto
                    .get("precio_unitario_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
                "precio_total_cents": resuelto
                    .get("precio_total_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
            }));
        }
        Ok(json!({ "precios": precios }))
    }

    async fn resolver_lista(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Option<Value>, ApiError> {
        if let Some(codigo) = filtro_texto(filtros, "lista_codigo") {
            let codigo = validar_codigo(&codigo, "lista_codigo")?;
            let fila = self
                .consulta_lista(
                    &format!(
                        "SELECT {COLUMNAS_LISTA}
                           FROM catalog.listas_precio
                          WHERE empresa_id = $1 AND lower(codigo) = $2 AND activo"
                    ),
                    empresa_id,
                    &codigo,
                )
                .await?;
            let fila = fila.ok_or_else(|| ApiError::no_encontrado("Lista de precios", &codigo))?;
            return Ok(Some(resumen_lista(&fila)));
        }
        if let Some(segmento) = filtro_texto(filtros, "segmento_codigo") {
            let segmento = validar_segmento(&segmento)?;
            let fila = self
                .consulta_lista(
                    &format!(
                        "SELECT {COLUMNAS_LISTA}
                           FROM catalog.listas_precio
                          WHERE empresa_id = $1 AND segmento_codigo = $2 AND activo
                          ORDER BY predeterminada DESC, creado_en
                          LIMIT 1"
                    ),
                    empresa_id,
                    &segmento,
                )
                .await?;
            let fila = fila.ok_or_else(|| {
                ApiError::no_encontrado("Lista de precios del segmento", &segmento)
            })?;
            return Ok(Some(resumen_lista(&fila)));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM catalog.listas_precio
              WHERE empresa_id = $1 AND predeterminada AND activo
              LIMIT 1"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(resumen_lista(&fila_json(&fila)?))),
            None => Ok(None),
        }
    }

    async fn consulta_lista(
        &self,
        sql: &str,
        empresa_id: &str,
        valor: &str,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(sql)
            .bind(empresa_id)
            .bind(valor)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(fila_json(&fila)?)),
            None => Ok(None),
        }
    }

    async fn buscar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        lista_id: Option<&str>,
        cantidad: i64,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM catalog.lista_precio_items
              WHERE empresa_id = $1 AND item_id = $2
                AND lista_id IS NOT DISTINCT FROM $3::uuid
                AND desde_cantidad <= $4::bigint
                AND (hasta_cantidad IS NULL OR hasta_cantidad >= $4::bigint)
              ORDER BY desde_cantidad DESC
              LIMIT 1"
        ))
        .bind(empresa_id)
        .bind(item_id)
        .bind(lista_id)
        .bind(cantidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(fila_json(&fila)?)),
            None => Ok(None),
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemPrecio, ApiError> {
        let id = validar_uuid(item_id, "item")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, precio_cents, tipo_precio
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
        Ok(ItemPrecio {
            id: texto_de(&valor, "id"),
            precio_cents: entero_de(&valor, "precio_cents").unwrap_or(0),
            tipo_precio: texto_de(&valor, "tipo_precio"),
        })
    }

    async fn buscar_lista(&self, empresa_id: &str, lista_id: &str) -> Result<Value, ApiError> {
        let id = validar_uuid(lista_id, "lista")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM catalog.listas_precio
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
            None => Err(ApiError::no_encontrado("Lista de precios", &id)),
        }
    }

    async fn buscar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
    ) -> Result<Value, ApiError> {
        let id = validar_uuid(precio_id, "precio")?;
        let lista_bd = lista_id;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM catalog.lista_precio_items
              WHERE id = $1 AND empresa_id = $2
                AND lista_id IS NOT DISTINCT FROM $3::uuid"
        ))
        .bind(&id)
        .bind(empresa_id)
        .bind(lista_bd)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Precio", &id)),
        }
    }

    async fn marcar_escala(&self, empresa_id: &str, item_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE catalog.items
                SET tipo_precio = 'escala', actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2 AND tipo_precio <> 'escala'",
        )
        .bind(item_id)
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    async fn actualizar_tipo_precio(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE catalog.items i
                SET tipo_precio = CASE WHEN EXISTS (
                      SELECT 1 FROM catalog.lista_precio_items p
                       WHERE p.empresa_id = i.empresa_id AND p.item_id = i.id
                         AND p.lista_id IS NULL
                    ) THEN 'escala' ELSE 'fijo' END,
                    actualizado_en = NOW()
              WHERE i.id = $1 AND i.empresa_id = $2",
        )
        .bind(item_id)
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}

struct ItemPrecio {
    id: String,
    precio_cents: i64,
    tipo_precio: String,
}

#[derive(Debug)]
struct RangoCantidad {
    desde: i64,
    hasta: Option<i64>,
    precio: i64,
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

fn error_conflicto(error: sqlx::Error, mensaje: &str) -> ApiError {
    if es_conflicto(&error) {
        ApiError::conflicto(mensaje)
    } else {
        interno(error)
    }
}

fn es_conflicto(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|base| base.code())
        .is_some_and(|codigo| codigo == "23P01" || codigo == "23505")
}

fn texto_de(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn entero_de(valor: &Value, clave: &str) -> Option<i64> {
    valor.get(clave).and_then(Value::as_i64)
}

fn booleano_de(valor: &Value, clave: &str) -> bool {
    valor.get(clave).and_then(Value::as_bool).unwrap_or(false)
}

fn texto(input: &Value, clave: &str) -> String {
    texto_o_vacio(input.get(clave))
}

fn texto_o_vacio(valor: Option<&Value>) -> String {
    match valor {
        None | Some(Value::Null) => String::new(),
        Some(valor) => texto_js(valor),
    }
}

/// `String(valor)` de JavaScript para los escalares que usa el contrato.
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

fn recortar(valor: String, maximo: usize) -> String {
    valor.chars().take(maximo).collect()
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        None | Some(Value::Null) => None,
        Some(Value::Bool(false)) => None,
        Some(Value::Number(numero)) if numero.as_f64() == Some(0.0) => None,
        Some(Value::String(texto)) if texto.is_empty() => None,
        Some(valor) => Some(texto_js(valor)),
    }
}

fn validar_rango_cantidad(
    desde: &Value,
    hasta: Option<&Value>,
    precio: &Value,
) -> Result<RangoCantidad, ApiError> {
    let desde = validar_cantidad(desde)?;
    let hasta = match hasta {
        None => None,
        Some(valor) => Some(validar_cantidad(valor)?),
    };
    if let Some(hasta) = hasta {
        if hasta < desde {
            return Err(ApiError::dominio(
                "ESCALA_INVALIDA",
                "La cantidad final no puede ser menor que la inicial.",
            ));
        }
    }
    Ok(RangoCantidad {
        desde,
        hasta,
        precio: validar_precio(precio)?,
    })
}

fn validar_cantidad(valor: &Value) -> Result<i64, ApiError> {
    let cantidad = numero_entero(valor).filter(|valor| *valor >= 1 && *valor <= MAX_CANTIDAD);
    cantidad.ok_or_else(|| {
        ApiError::dominio(
            "CANTIDAD_INVALIDA",
            format!("La cantidad debe ser un entero entre 1 y {MAX_CANTIDAD}."),
        )
    })
}

fn validar_precio(valor: &Value) -> Result<i64, ApiError> {
    let precio = numero_entero(valor).filter(|valor| *valor >= 0);
    let Some(precio) = precio else {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio debe ser un entero no negativo.",
        ));
    };
    Money::desde_centavos(precio)
        .map(|monto| monto.centavos)
        .map_err(|_| {
            ApiError::dominio(
                "PRECIO_INVALIDO",
                "El precio debe ser un entero no negativo.",
            )
        })
}

/// `Number(valor)` + `Number.isInteger`, sin punto flotante en la salida.
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

fn validar_codigo(valor: &str, campo: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    if !coincide(&codigo, CODIGO) {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            format!(
                "El {campo} debe empezar con letra y usar minusculas, numeros, guion o guion bajo."
            ),
        ));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    let longitud = nombre.chars().count();
    if !(2..=120).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 120 caracteres.",
        ));
    }
    Ok(nombre.to_string())
}

fn validar_moneda(valor: &str) -> Result<String, ApiError> {
    let moneda = valor.trim().to_uppercase();
    let valida = moneda.len() == 3 && moneda.chars().all(|caracter| caracter.is_ascii_uppercase());
    if !valida {
        return Err(ApiError::dominio(
            "MONEDA_INVALIDA",
            "La moneda debe ser un codigo ISO de 3 letras.",
        ));
    }
    Ok(moneda)
}

fn validar_segmento(valor: &str) -> Result<String, ApiError> {
    let segmento = valor.trim().to_lowercase();
    if !segmento.is_empty() && !coincide(&segmento, SEGMENTO) {
        return Err(ApiError::dominio(
            "SEGMENTO_INVALIDO",
            "El segmento debe empezar con letra y usar minusculas, numeros, guion o guion bajo.",
        ));
    }
    Ok(segmento)
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

/// Regex del contrato: 8-4-4-4-12 hexadecimal con guiones.
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

/// Implementación acotada de `^[a-z][a-z0-9_-]{n,m}$` sin dependencias regex.
fn coincide(valor: &str, patron: &str) -> bool {
    if patron == CODIGO {
        let mut caracteres = valor.chars();
        matches!(caracteres.next(), Some(caracter) if caracter.is_ascii_lowercase()) && {
            let resto: Vec<char> = caracteres.collect();
            (1..=39).contains(&resto.len())
                && resto.iter().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || *caracter == '_'
                        || *caracter == '-'
                })
        }
    } else if patron == SEGMENTO {
        if valor.is_empty() {
            return true;
        }
        let mut caracteres = valor.chars();
        matches!(caracteres.next(), Some(caracter) if caracter.is_ascii_lowercase()) && {
            let resto: Vec<char> = caracteres.collect();
            resto.len() <= 39
                && resto.iter().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || *caracter == '_'
                        || *caracter == '-'
                })
        }
    } else {
        false
    }
}

fn multiplicar(precio: i64, cantidad: i64) -> Result<i64, ApiError> {
    let total = precio
        .checked_mul(cantidad)
        .filter(|total| *total <= MAX_SAFE_INTEGER)
        .ok_or_else(|| {
            ApiError::dominio(
                "TOTAL_INVALIDO",
                "El total del precio excede el rango permitido.",
            )
        })?;
    Ok(total)
}

fn resumen_lista(lista: &Value) -> Value {
    json!({
        "id": lista.get("id").cloned().unwrap_or(Value::Null),
        "codigo": lista.get("codigo").cloned().unwrap_or(Value::Null),
        "nombre": lista.get("nombre").cloned().unwrap_or(Value::Null),
        "segmento_codigo": lista.get("segmento_codigo").cloned().unwrap_or(Value::Null),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_codigos_nombres_monedas_y_segmentos() {
        assert_eq!(
            validar_codigo(" Mayorista ", "codigo").expect("codigo"),
            "mayorista"
        );
        assert_eq!(
            validar_codigo("Mayorista!", "codigo")
                .expect_err("simbolo")
                .codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            validar_codigo("a", "codigo").expect_err("corto").codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(validar_nombre(" VIP ").expect("nombre"), "VIP");
        assert_eq!(
            validar_nombre("x").expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
        assert_eq!(validar_moneda("usd").expect("moneda"), "USD");
        assert_eq!(
            validar_moneda("").expect_err("vacia").codigo,
            "MONEDA_INVALIDA"
        );
        assert_eq!(
            validar_moneda("peso").expect_err("moneda").codigo,
            "MONEDA_INVALIDA"
        );
        assert_eq!(validar_segmento("").expect("segmento"), "");
        assert_eq!(
            validar_segmento(" Mayorista ").expect("segmento"),
            "mayorista"
        );
        assert_eq!(
            validar_segmento("V I P").expect_err("segmento").codigo,
            "SEGMENTO_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_uuid_con_el_regex_del_contrato() {
        assert!(es_uuid("11111111-1111-4111-8111-111111111111"));
        assert!(es_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"));
        assert!(!es_uuid("11111111111141118111111111111111"));
        assert!(!es_uuid("11111111-1111-4111-8111-11111111111z"));
        assert!(!es_uuid(""));
    }

    #[test]
    fn debe_validar_cantidades_y_el_rango_de_escala() {
        assert_eq!(validar_cantidad(&json!(1)).expect("cantidad"), 1);
        assert_eq!(validar_cantidad(&json!("12")).expect("cantidad"), 12);
        assert_eq!(
            validar_cantidad(&json!(1_000_000)).expect("cantidad"),
            1_000_000
        );
        assert_eq!(
            validar_cantidad(&json!(0)).expect_err("cero").codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_cantidad(&json!(1.5)).expect_err("decimal").codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_cantidad(&json!(1_000_001))
                .expect_err("maximo")
                .codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_rango_cantidad(&json!(10), Some(&json!(5)), &json!(100))
                .expect_err("rango")
                .codigo,
            "ESCALA_INVALIDA"
        );
        assert_eq!(
            validar_rango_cantidad(&json!(1), None, &json!(-1))
                .expect_err("precio")
                .codigo,
            "PRECIO_INVALIDO"
        );
    }

    #[test]
    fn debe_multiplicar_sin_salir_del_rango_seguro() {
        assert_eq!(multiplicar(1_000, 3).expect("total"), 3_000);
        assert_eq!(
            multiplicar(MAX_SAFE_INTEGER, 2)
                .expect_err("desborde")
                .codigo,
            "TOTAL_INVALIDO"
        );
    }

    #[test]
    fn debe_resumir_la_lista_con_las_cuatro_columnas_del_contrato() {
        let resumen = resumen_lista(&json!({
            "id": "33333333-3333-4333-8333-333333333333",
            "codigo": "mayorista",
            "nombre": "Mayorista",
            "segmento_codigo": "mayorista",
            "moneda": "NIO",
        }));
        assert_eq!(
            resumen,
            json!({
                "id": "33333333-3333-4333-8333-333333333333",
                "codigo": "mayorista",
                "nombre": "Mayorista",
                "segmento_codigo": "mayorista",
            })
        );
    }
}
