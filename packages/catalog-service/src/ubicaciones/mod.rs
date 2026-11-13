//! Multi-bodega/sucursal (Fase P1.2, ADR-0001).
//!
//! Réplica de `ubicaciones.service.ts`. `catalog.items.stock` es el total de la
//! empresa y `item_stock_ubicacion` su desglose: la primera ubicación absorbe
//! el stock existente, las transferencias no alteran el total y `fijar_stock`
//! resincroniza el total del item dentro de la misma transacción.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::{transaccion, Items};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

/// Tipos de ubicación admitidos por el contrato.
pub const TIPOS_UBICACION: [&str; 4] = ["bodega", "sucursal", "almacen", "otro"];

const COLUMNAS: &str = "id, empresa_id, codigo, nombre, tipo, direccion, predeterminada,
  activo, creado_en, actualizado_en";

pub struct Ubicaciones {
    db: Arc<Db>,
    items: Arc<Items>,
}

struct ItemVerificado {
    id: String,
    codigo: String,
    nombre: String,
    stock: Option<i32>,
    track_inventory: bool,
}

impl Ubicaciones {
    pub fn nueva(db: Arc<Db>, items: Arc<Items>) -> Self {
        Self { db, items }
    }

    pub async fn listar(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.ubicaciones
              WHERE empresa_id = $1::uuid
              ORDER BY predeterminada DESC, nombre"
        ))
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

    pub async fn crear(&self, empresa_id: &str, input: &Value) -> Result<Value, ApiError> {
        let codigo = validar_codigo(input.get("codigo"))?;
        let nombre = validar_nombre(input.get("nombre"))?;
        let tipo = validar_tipo(input.get("tipo"))?;
        let direccion = direccion_crear(input.get("direccion"));
        let predeterminada = input.get("predeterminada").and_then(Value::as_bool) == Some(true);

        let mut tx = transaccion(&self.items).await?;
        if predeterminada {
            sqlx::query(
                "UPDATE catalog.ubicaciones SET predeterminada = false, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND predeterminada",
            )
            .bind(empresa_id)
            .execute(&mut *tx)
            .await
            .map_err(error_ubicacion)?;
        }
        let creada = sqlx::query(&format!(
            "INSERT INTO catalog.ubicaciones
               (empresa_id, codigo, nombre, tipo, direccion, predeterminada)
             VALUES ($1::uuid, $2, $3, $4, $5, $6)
             RETURNING {COLUMNAS}"
        ))
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(&tipo)
        .bind(&direccion)
        .bind(predeterminada)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_ubicacion)?;
        let Some(ubicacion) = creada else {
            return Err(ApiError::interno());
        };
        let ubicacion_id: Uuid = ubicacion.try_get("id").map_err(interno)?;

        let total: i32 = sqlx::query(
            "SELECT COUNT(*)::int AS total FROM catalog.ubicaciones WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_ubicacion)?
        .get("total");
        // La primera ubicación absorbe el stock que ya existía en el item.
        if total == 1 {
            sqlx::query(
                "INSERT INTO catalog.item_stock_ubicacion (empresa_id, item_id, ubicacion_id, stock)
                 SELECT i.empresa_id, i.id, $2::uuid, COALESCE(i.stock, 0)
                   FROM catalog.items i
                  WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto'
                    AND i.track_inventory AND i.eliminado_en IS NULL
                 ON CONFLICT (item_id, ubicacion_id) DO NOTHING",
            )
            .bind(empresa_id)
            .bind(ubicacion_id)
            .execute(&mut *tx)
            .await
            .map_err(error_ubicacion)?;
        }
        tx.commit().await.map_err(interno)?;
        fila_a_json(&ubicacion).map_err(interno)
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar(empresa_id, ubicacion_id).await?;
        let actual_codigo = campo_texto(&actual, "codigo");
        let actual_nombre = campo_texto(&actual, "nombre");
        let actual_tipo = campo_texto(&actual, "tipo");
        let actual_direccion = campo_texto(&actual, "direccion");
        let actual_predeterminada = campo_bool(&actual, "predeterminada");
        let actual_activo = campo_bool(&actual, "activo");

        let codigo = match input.get("codigo") {
            None => actual_codigo,
            Some(valor) => validar_codigo(Some(valor))?,
        };
        let nombre = match input.get("nombre") {
            None => actual_nombre,
            Some(valor) => validar_nombre(Some(valor))?,
        };
        let tipo = match input.get("tipo") {
            None => actual_tipo,
            Some(valor) => validar_tipo(Some(valor))?,
        };
        let direccion = match input.get("direccion") {
            None => actual_direccion,
            Some(valor) => direccion_actualizar(valor),
        };
        let activo = match input.get("activo") {
            None => actual_activo,
            Some(valor) => valor.as_bool() == Some(true),
        };
        let predeterminada = match input.get("predeterminada") {
            None => actual_predeterminada,
            Some(valor) => valor.as_bool() == Some(true),
        } && activo;

        if !activo && actual_activo {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let ocupada: i32 = sqlx::query(
                "SELECT COALESCE(SUM(stock), 0)::int AS total
                   FROM catalog.item_stock_ubicacion
                  WHERE empresa_id = $1::uuid AND ubicacion_id = $2::uuid AND stock > 0",
            )
            .bind(empresa_id)
            .bind(ubicacion_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("total");
            tx.commit().await.map_err(interno)?;
            if ocupada > 0 {
                return Err(ApiError::conflicto(
                    "No se puede desactivar una ubicacion con stock.",
                ));
            }
        }

        let mut tx = transaccion(&self.items).await?;
        if predeterminada && !actual_predeterminada {
            sqlx::query(
                "UPDATE catalog.ubicaciones SET predeterminada = false, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND predeterminada AND id <> $2::uuid",
            )
            .bind(empresa_id)
            .bind(ubicacion_id)
            .execute(&mut *tx)
            .await
            .map_err(error_ubicacion)?;
        }
        let fila = sqlx::query(&format!(
            "UPDATE catalog.ubicaciones
                SET codigo = $1, nombre = $2, tipo = $3, direccion = $4,
                    predeterminada = $5, activo = $6, actualizado_en = NOW()
              WHERE id = $7::uuid AND empresa_id = $8::uuid
              RETURNING {COLUMNAS}"
        ))
        .bind(&codigo)
        .bind(&nombre)
        .bind(&tipo)
        .bind(&direccion)
        .bind(predeterminada)
        .bind(activo)
        .bind(ubicacion_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_ubicacion)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Ubicacion", ubicacion_id));
        };
        tx.commit().await.map_err(interno)?;
        fila_a_json(&fila).map_err(interno)
    }

    /// Desglose de stock de un item por ubicación.
    pub async fn stock_por_item(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT s.ubicacion_id, u.codigo, u.nombre, s.stock
               FROM catalog.item_stock_ubicacion s
               JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
              WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid
              ORDER BY u.predeterminada DESC, u.nombre",
        )
        .bind(empresa_id)
        .bind(item_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let ubicaciones = filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect::<Result<Vec<_>, _>>()?;
        let total = sumar_stocks(&ubicaciones);
        Ok(json!({
            "item_id": item.id,
            "sku": item.codigo,
            "nombre": item.nombre,
            "stock_total": item.stock,
            "total_ubicaciones": total,
            "ubicaciones": ubicaciones,
        }))
    }

    /// Stock de todos los items de una ubicación.
    pub async fn stock_de_ubicacion(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        self.buscar(empresa_id, ubicacion_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
                    s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
               FROM catalog.item_stock_ubicacion s
               JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
               JOIN catalog.items i ON i.id = s.item_id
              WHERE s.empresa_id = $1::uuid AND s.ubicacion_id = $2::uuid AND i.eliminado_en IS NULL
              ORDER BY i.nombre",
        )
        .bind(empresa_id)
        .bind(ubicacion_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect()
    }

    /// Fija el stock de un item en una ubicación y resincroniza el total.
    pub async fn fijar_stock(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
        item_id: &str,
        stock: &Value,
    ) -> Result<Value, ApiError> {
        self.buscar_activa(empresa_id, ubicacion_id).await?;
        let item = self.verificar_item(empresa_id, item_id).await?;
        if !item.track_inventory {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "El item no controla inventario.",
            ));
        }
        let stock = validar_stock(stock)?;
        let stock_i32 = i32::try_from(stock).map_err(|_| ApiError::interno())?;

        let mut tx = transaccion(&self.items).await?;
        sqlx::query(
            "INSERT INTO catalog.item_stock_ubicacion AS s
               (empresa_id, item_id, ubicacion_id, stock)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             ON CONFLICT (item_id, ubicacion_id) DO UPDATE
               SET stock = EXCLUDED.stock, actualizado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(ubicacion_id)
        .bind(stock_i32)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sincronizar_total(&mut tx, empresa_id, item_id).await?;
        let fila = sqlx::query(
            "SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
                    s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
               FROM catalog.item_stock_ubicacion s
               JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
               JOIN catalog.items i ON i.id = s.item_id
              WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid AND s.ubicacion_id = $3::uuid",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(ubicacion_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::interno());
        };
        tx.commit().await.map_err(interno)?;
        fila_a_json(&fila).map_err(interno)
    }

    /// Traslada stock entre dos ubicaciones sin alterar el total del item.
    pub async fn transferir(&self, empresa_id: &str, input: &Value) -> Result<Value, ApiError> {
        let item_id = auto_texto(input.get("item_id"));
        let item = self.verificar_item(empresa_id, &item_id).await?;
        let cantidad = validar_cantidad(input.get("cantidad"))?;
        let origen_id = auto_texto(input.get("origen_id"));
        let destino_id = auto_texto(input.get("destino_id"));
        if origen_id == destino_id {
            return Err(ApiError::dominio(
                "TRASLADO_INVALIDO",
                "El origen y el destino deben ser distintos.",
            ));
        }
        self.buscar_activa(empresa_id, &origen_id).await?;
        self.buscar_activa(empresa_id, &destino_id).await?;

        let mut tx = transaccion(&self.items).await?;
        let origen = sqlx::query(
            "SELECT stock FROM catalog.item_stock_ubicacion
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND ubicacion_id = $3::uuid
              FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(&item.id)
        .bind(&origen_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let disponible = origen
            .as_ref()
            .map(|fila| i64::from(fila.get::<i32, _>("stock")))
            .unwrap_or(0);
        if origen.is_none() || disponible < cantidad {
            return Err(ApiError::conflicto_codigo(
                "La ubicacion de origen no tiene stock suficiente.",
                "STOCK_UBICACION_INSUFICIENTE",
                json!({ "disponible": disponible }),
            ));
        }
        let cantidad_i32 = i32::try_from(cantidad).map_err(|_| ApiError::interno())?;
        sqlx::query(
            "UPDATE catalog.item_stock_ubicacion
                SET stock = stock - $1, actualizado_en = NOW()
              WHERE empresa_id = $2::uuid AND item_id = $3::uuid AND ubicacion_id = $4::uuid",
        )
        .bind(cantidad_i32)
        .bind(empresa_id)
        .bind(&item.id)
        .bind(&origen_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "INSERT INTO catalog.item_stock_ubicacion AS s
               (empresa_id, item_id, ubicacion_id, stock)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             ON CONFLICT (item_id, ubicacion_id) DO UPDATE
               SET stock = s.stock + EXCLUDED.stock, actualizado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(&item.id)
        .bind(&destino_id)
        .bind(cantidad_i32)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        let filas = sqlx::query(
            "SELECT s.ubicacion_id, u.codigo AS ubicacion_codigo, u.nombre AS ubicacion_nombre,
                    s.item_id, i.codigo AS sku, i.nombre, s.stock, i.stock AS stock_total
               FROM catalog.item_stock_ubicacion s
               JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
               JOIN catalog.items i ON i.id = s.item_id
              WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid
                AND s.ubicacion_id IN ($3::uuid, $4::uuid)",
        )
        .bind(empresa_id)
        .bind(&item.id)
        .bind(&origen_id)
        .bind(&destino_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let mut por_ubicacion = std::collections::HashMap::new();
        for fila in &filas {
            let valor = fila_a_json(fila).map_err(interno)?;
            if let Some(id) = valor.get("ubicacion_id").and_then(Value::as_str) {
                por_ubicacion.insert(id.to_string(), valor);
            }
        }
        // `JSON.stringify` omite las claves con valor `undefined`, igual que el
        // contrato original cuando una fila no aparece.
        let mut resultado = Map::new();
        if let Some(valor) = por_ubicacion.remove(&origen_id) {
            resultado.insert("origen".into(), valor);
        }
        if let Some(valor) = por_ubicacion.remove(&destino_id) {
            resultado.insert("destino".into(), valor);
        }
        Ok(Value::Object(resultado))
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    async fn buscar(&self, empresa_id: &str, ubicacion_id: &str) -> Result<Value, ApiError> {
        validar_uuid(ubicacion_id, "ubicacion")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.ubicaciones
              WHERE id = $1::uuid AND empresa_id = $2::uuid"
        ))
        .bind(ubicacion_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Err(ApiError::no_encontrado("Ubicacion", ubicacion_id)),
        }
    }

    async fn buscar_activa(&self, empresa_id: &str, ubicacion_id: &str) -> Result<Value, ApiError> {
        let ubicacion = self.buscar(empresa_id, ubicacion_id).await?;
        if !campo_bool(&ubicacion, "activo") {
            return Err(ApiError::nueva(
                "UBICACION_INACTIVA",
                "La ubicacion esta inactiva.",
                axum::http::StatusCode::CONFLICT,
            ));
        }
        Ok(ubicacion)
    }

    async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemVerificado, ApiError> {
        validar_uuid(item_id, "item")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, codigo, nombre, stock, track_inventory
               FROM catalog.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Item", item_id));
        };
        Ok(ItemVerificado {
            id: fila.get::<Uuid, _>("id").to_string(),
            codigo: fila.get("codigo"),
            nombre: fila.get("nombre"),
            stock: fila.try_get("stock").map_err(interno)?,
            track_inventory: fila.get("track_inventory"),
        })
    }
}

async fn sincronizar_total(
    tx: &mut Transaction<'static, Postgres>,
    empresa_id: &str,
    item_id: &str,
) -> Result<(), ApiError> {
    sqlx::query("SELECT set_config('catalog.stock_sync', 'on', true)")
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
    sqlx::query(
        "UPDATE catalog.items
            SET stock = (
                  SELECT COALESCE(SUM(stock), 0)::int
                    FROM catalog.item_stock_ubicacion
                   WHERE empresa_id = $1::uuid AND item_id = $2::uuid
                ),
                actualizado_en = NOW()
          WHERE id = $2::uuid AND empresa_id = $1::uuid",
    )
    .bind(empresa_id)
    .bind(item_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    sqlx::query("SELECT set_config('catalog.stock_sync', 'off', true)")
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
    Ok(())
}

fn campo_texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn campo_bool(valor: &Value, clave: &str) -> bool {
    valor.get(clave).and_then(Value::as_bool).unwrap_or(false)
}

/// `String(valor)`: incluye `null` como `"null"`, igual que JavaScript.
fn cadena_js(valor: &Value) -> String {
    match valor {
        Value::Null => "null".to_string(),
        Value::String(texto) => texto.clone(),
        Value::Bool(valor) => valor.to_string(),
        Value::Number(numero) => numero.to_string(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                otro => cadena_js(otro),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// `String(valor ?? '')` de JavaScript.
fn auto_texto(valor: Option<&Value>) -> String {
    match valor {
        None | Some(Value::Null) => String::new(),
        Some(valor) => cadena_js(valor),
    }
}

/// `Number(valor)` de JavaScript para los campos numéricos del contrato.
fn numero_js(valor: &Value) -> Option<f64> {
    match valor {
        Value::Null => Some(0.0),
        Value::Bool(valor) => Some(if *valor { 1.0 } else { 0.0 }),
        Value::Number(numero) => numero.as_f64(),
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                Some(0.0)
            } else {
                texto.parse::<f64>().ok()
            }
        }
        Value::Array(items) => match items.len() {
            0 => Some(0.0),
            1 => numero_js(&items[0]),
            _ => None,
        },
        Value::Object(_) => None,
    }
}

fn validar_codigo(valor: Option<&Value>) -> Result<String, ApiError> {
    let codigo = auto_texto(valor).trim().to_lowercase();
    let mut caracteres = codigo.chars();
    let valido = matches!(caracteres.next(), Some(caracter) if caracter.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo.chars().all(|caracter| {
            caracter.is_ascii_lowercase()
                || caracter.is_ascii_digit()
                || caracter == '-'
                || caracter == '_'
        });
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            "El codigo debe empezar con letra y usar minusculas, numeros, guion o guion bajo.",
        ));
    }
    Ok(codigo)
}

fn validar_nombre(valor: Option<&Value>) -> Result<String, ApiError> {
    let nombre = auto_texto(valor).trim().to_string();
    let longitud = nombre.chars().count();
    if !(2..=120).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 120 caracteres.",
        ));
    }
    Ok(nombre)
}

fn validar_tipo(valor: Option<&Value>) -> Result<String, ApiError> {
    let tipo = match valor {
        None | Some(Value::Null) => "bodega".to_string(),
        Some(valor) => cadena_js(valor).trim().to_string(),
    };
    if !TIPOS_UBICACION.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_UBICACION_INVALIDO",
            "El tipo de ubicacion no es valido.",
        ));
    }
    Ok(tipo)
}

fn validar_stock(valor: &Value) -> Result<i64, ApiError> {
    match numero_js(valor) {
        Some(numero) if numero.is_finite() && numero.fract() == 0.0 && numero >= 0.0 => {
            Ok(numero as i64)
        }
        _ => Err(ApiError::dominio(
            "STOCK_INVALIDO",
            "El stock debe ser un entero no negativo.",
        )),
    }
}

fn validar_cantidad(valor: Option<&Value>) -> Result<i64, ApiError> {
    match valor.and_then(numero_js) {
        Some(numero) if numero.is_finite() && numero.fract() == 0.0 && numero >= 1.0 => {
            Ok(numero as i64)
        }
        _ => Err(ApiError::dominio(
            "CANTIDAD_INVALIDA",
            "La cantidad debe ser un entero positivo.",
        )),
    }
}

fn validar_uuid(valor: &str, campo: &str) -> Result<(), ApiError> {
    if Uuid::parse_str(valor).is_err() {
        return Err(ApiError::dominio(
            "ID_INVALIDO",
            format!("El {campo} debe ser un UUID."),
        ));
    }
    Ok(())
}

fn direccion_crear(valor: Option<&Value>) -> String {
    auto_texto(valor).trim().chars().take(240).collect()
}

fn direccion_actualizar(valor: &Value) -> String {
    cadena_js(valor).trim().chars().take(240).collect()
}

fn sumar_stocks(ubicaciones: &[Value]) -> i64 {
    ubicaciones
        .iter()
        .filter_map(|fila| fila.get("stock").and_then(Value::as_i64))
        .sum()
}

fn es_conflicto(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(bd) => matches!(bd.code().as_deref(), Some("23505") | Some("23P01")),
        _ => false,
    }
}

fn error_ubicacion(error: sqlx::Error) -> ApiError {
    if es_conflicto(&error) {
        ApiError::conflicto("La ubicacion ya existe o ya hay una predeterminada.")
    } else {
        interno(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_codigos_y_nombres() {
        assert_eq!(
            validar_codigo(Some(&json!("Bodega-Central"))).expect("codigo"),
            "bodega-central"
        );
        assert_eq!(
            validar_codigo(Some(&json!(" a ")))
                .expect_err("corto")
                .codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            validar_codigo(Some(&json!("bodega!")))
                .expect_err("simbolo")
                .codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            validar_nombre(Some(&json!(" Central "))).expect("nombre"),
            "Central"
        );
        assert_eq!(
            validar_nombre(Some(&json!("x"))).expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_el_tipo_de_ubicacion() {
        assert_eq!(validar_tipo(None).expect("tipo"), "bodega");
        assert_eq!(
            validar_tipo(Some(&json!("sucursal"))).expect("tipo"),
            "sucursal"
        );
        assert_eq!(
            validar_tipo(Some(&json!("oficina")))
                .expect_err("tipo")
                .codigo,
            "TIPO_UBICACION_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_stock_y_cantidad_como_javascript() {
        assert_eq!(validar_stock(&json!(4)).expect("stock"), 4);
        assert_eq!(validar_stock(&Value::Null).expect("nulo"), 0);
        assert_eq!(validar_stock(&json!("7")).expect("texto"), 7);
        assert_eq!(
            validar_stock(&json!(-1)).expect_err("negativo").codigo,
            "STOCK_INVALIDO"
        );
        assert_eq!(
            validar_stock(&json!(1.5)).expect_err("fraccion").codigo,
            "STOCK_INVALIDO"
        );
        assert_eq!(validar_cantidad(Some(&json!(3))).expect("cantidad"), 3);
        assert_eq!(
            validar_cantidad(Some(&json!(0))).expect_err("cero").codigo,
            "CANTIDAD_INVALIDA"
        );
    }

    #[test]
    fn debe_replicar_la_conversion_de_texto_de_javascript() {
        assert_eq!(auto_texto(None), "");
        assert_eq!(auto_texto(Some(&Value::Null)), "");
        assert_eq!(auto_texto(Some(&json!("Bodega_Central"))), "Bodega_Central");
        assert_eq!(cadena_js(&Value::Null), "null");
        assert_eq!(cadena_js(&json!(true)), "true");
        assert_eq!(cadena_js(&json!([1, 2])), "1,2");
    }

    #[test]
    fn debe_sumar_el_stock_de_las_ubicaciones() {
        let filas = vec![
            json!({ "ubicacion_id": "a", "stock": 4 }),
            json!({ "ubicacion_id": "b", "stock": 6 }),
        ];
        assert_eq!(sumar_stocks(&filas), 10);
        assert_eq!(sumar_stocks(&[]), 0);
    }

    #[test]
    fn debe_recortar_la_direccion_a_240_caracteres() {
        let larga = "x".repeat(300);
        assert_eq!(direccion_crear(Some(&json!(larga))).chars().count(), 240);
        assert_eq!(direccion_crear(None), "");
        assert_eq!(direccion_actualizar(&Value::Null), "null");
    }
}
