//! Mesas y comandas del salón (Fase P2.5).
//!
//! Réplica de `salon.service.ts`: `mesas` modela el salón y `comandas` los
//! tickets de preparación por estación (cocina, bar, etc.). Una comanda puede
//! nacer de una orden o de items libres.

use crate::db::{jsonb, timestamp, timestamp_opcional, uuid_opcional, uuid_texto};
use crate::errors::ApiError;
use crate::event_store::{error_db, parsear_uuid};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgRow, Row};
use std::sync::Arc;
use uuid::Uuid;

pub const ESTADOS_MESA: [&str; 4] = ["libre", "ocupada", "reservada", "inactiva"];
pub const ESTADOS_COMANDA: [&str; 5] = [
    "pendiente",
    "en_preparacion",
    "lista",
    "entregada",
    "cancelada",
];

const COLUMNAS_MESA: &str =
    "id, empresa_id, codigo, nombre, capacidad, zona, estado, creado_en, actualizado_en";
const COLUMNAS_COMANDA: &str =
    "id, empresa_id, order_id, mesa_id, estacion, estado, notas, items_json,
     creado_en, actualizado_en, listo_en, entregado_en";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mesa {
    pub id: String,
    pub empresa_id: String,
    pub codigo: String,
    pub nombre: String,
    pub capacidad: i64,
    pub zona: String,
    pub estado: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComandaItem {
    pub sku: String,
    pub nombre: String,
    pub cantidad: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Comanda {
    pub id: String,
    pub empresa_id: String,
    pub order_id: Option<String>,
    pub mesa_id: Option<String>,
    pub estacion: String,
    pub estado: String,
    pub notas: String,
    pub items: Vec<ComandaItem>,
    pub creado_en: String,
    pub actualizado_en: String,
    pub listo_en: Option<String>,
    pub entregado_en: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CrearMesaInput {
    pub codigo: String,
    pub nombre: String,
    pub capacidad: Option<i64>,
    pub zona: Option<String>,
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ActualizarMesaInput {
    pub codigo: Option<String>,
    pub nombre: Option<String>,
    pub capacidad: Option<i64>,
    pub zona: Option<String>,
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CrearComandaInput {
    pub order_id: Option<String>,
    pub mesa_id: Option<String>,
    pub estacion: String,
    pub notas: Option<String>,
    pub items: Option<Vec<ComandaItem>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ActualizarComandaInput {
    pub estado: Option<String>,
    pub notas: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct FiltrosMesa {
    pub zona: Option<String>,
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct FiltrosComanda {
    pub estado: Option<String>,
    pub estacion: Option<String>,
    pub mesa_id: Option<String>,
}

pub struct Salon {
    db: Arc<crate::db::Db>,
}

impl Salon {
    pub fn nueva(db: Arc<crate::db::Db>) -> Self {
        Self { db }
    }

    // ── mesas ───────────────────────────────────────────────────────────────

    pub async fn listar_mesas(
        &self,
        empresa_id: &str,
        filtros: FiltrosMesa,
    ) -> Result<Vec<Mesa>, ApiError> {
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let zona = filtros.zona.as_deref().filter(|valor| !valor.is_empty());
        let estado = filtros.estado.as_deref().filter(|valor| !valor.is_empty());
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_MESA}
               FROM orders.mesas
              WHERE empresa_id = $1
                AND ($2::text IS NULL OR zona = $2)
                AND ($3::text IS NULL OR estado = $3)
              ORDER BY zona, codigo"
        ))
        .bind(empresa_uuid)
        .bind(zona)
        .bind(estado)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(mesa_de_fila).collect()
    }

    pub async fn crear_mesa(
        &self,
        empresa_id: &str,
        input: CrearMesaInput,
    ) -> Result<Mesa, ApiError> {
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let codigo = validar_codigo(&input.codigo)?;
        let nombre = validar_nombre(&input.nombre)?;
        let capacidad = validar_capacidad(input.capacidad.unwrap_or(1))?;
        let zona = normalizar_zona(input.zona.as_deref());
        let estado = validar_estado_mesa(input.estado.as_deref().unwrap_or("libre"))?;

        let resultado = sqlx::query(&format!(
            "INSERT INTO orders.mesas
               (empresa_id, codigo, nombre, capacidad, zona, estado)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING {COLUMNAS_MESA}"
        ))
        .bind(empresa_uuid)
        .bind(&codigo)
        .bind(&nombre)
        .bind(capacidad_como_i32(capacidad)?)
        .bind(&zona)
        .bind(&estado)
        .fetch_optional(self.db.pool())
        .await;

        let fila = match resultado {
            Ok(Some(fila)) => fila,
            Ok(None) => {
                return Err(ApiError::dominio(
                    "MESA_NO_CREADA",
                    "No se pudo crear la mesa.",
                ));
            }
            Err(error) if es_conflicto(&error) => {
                return Err(ApiError::conflicto(format!(
                    "Ya existe una mesa con el codigo {codigo}."
                )));
            }
            Err(error) => return Err(error_db(error)),
        };
        mesa_de_fila(&fila)
    }

    pub async fn actualizar_mesa(
        &self,
        empresa_id: &str,
        id: &str,
        input: ActualizarMesaInput,
    ) -> Result<Mesa, ApiError> {
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let mesa_uuid = validar_uuid(id, "mesa")?;
        let actual = self.mesa_por_uuid(empresa_uuid, mesa_uuid, id).await?;

        let codigo = match input.codigo {
            Some(valor) => validar_codigo(&valor)?,
            None => actual.codigo,
        };
        let nombre = match input.nombre {
            Some(valor) => validar_nombre(&valor)?,
            None => actual.nombre,
        };
        let capacidad = match input.capacidad {
            Some(valor) => validar_capacidad(valor)?,
            None => actual.capacidad,
        };
        let zona = match input.zona {
            Some(valor) => normalizar_zona(Some(&valor)),
            None => actual.zona,
        };
        let estado = match input.estado {
            Some(valor) => validar_estado_mesa(&valor)?,
            None => actual.estado,
        };

        let resultado = sqlx::query(&format!(
            "UPDATE orders.mesas
                SET codigo = $3, nombre = $4, capacidad = $5, zona = $6,
                    estado = $7, actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2
              RETURNING {COLUMNAS_MESA}"
        ))
        .bind(mesa_uuid)
        .bind(empresa_uuid)
        .bind(&codigo)
        .bind(&nombre)
        .bind(capacidad_como_i32(capacidad)?)
        .bind(&zona)
        .bind(&estado)
        .fetch_optional(self.db.pool())
        .await;

        let fila = match resultado {
            Ok(Some(fila)) => fila,
            Ok(None) => return Err(ApiError::no_encontrado("Mesa", id)),
            Err(error) if es_conflicto(&error) => {
                return Err(ApiError::conflicto(format!(
                    "Ya existe una mesa con el codigo {codigo}."
                )));
            }
            Err(error) => return Err(error_db(error)),
        };
        mesa_de_fila(&fila)
    }

    // ── comandas ────────────────────────────────────────────────────────────

    pub async fn listar_comandas(
        &self,
        empresa_id: &str,
        filtros: FiltrosComanda,
    ) -> Result<Vec<Comanda>, ApiError> {
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let estado = filtros.estado.as_deref().filter(|valor| !valor.is_empty());
        let estacion = filtros
            .estacion
            .as_deref()
            .filter(|valor| !valor.is_empty());
        let mesa_uuid = match filtros.mesa_id.as_deref().filter(|valor| !valor.is_empty()) {
            Some(valor) => Some(validar_uuid(valor, "mesa")?),
            None => None,
        };

        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_COMANDA}
               FROM orders.comandas
              WHERE empresa_id = $1
                AND ($2::text IS NULL OR estado = $2)
                AND ($3::text IS NULL OR estacion = $3)
                AND ($4::uuid IS NULL OR mesa_id = $4)
              ORDER BY creado_en DESC
              LIMIT 200"
        ))
        .bind(empresa_uuid)
        .bind(estado)
        .bind(estacion)
        .bind(mesa_uuid)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(comanda_de_fila).collect()
    }

    pub async fn crear_comanda(
        &self,
        empresa_id: &str,
        input: CrearComandaInput,
    ) -> Result<Comanda, ApiError> {
        let CrearComandaInput {
            order_id,
            mesa_id,
            estacion,
            notas,
            items,
        } = input;
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let order_uuid = validar_uuid_opcional(order_id.as_deref(), "orden")?;
        let mesa_uuid = validar_uuid_opcional(mesa_id.as_deref(), "mesa")?;
        let estacion = validar_estacion(&estacion)?;
        let notas = normalizar_notas(notas.as_deref());

        if order_uuid.is_none() && items.as_ref().is_none_or(|lista| lista.is_empty()) {
            return Err(ApiError::dominio(
                "COMANDA_SIN_ITEMS",
                "La comanda requiere una orden o una lista de items.",
            ));
        }
        if let Some(mesa_uuid) = mesa_uuid {
            self.mesa_por_uuid(empresa_uuid, mesa_uuid, mesa_id.as_deref().unwrap_or(""))
                .await?;
        }

        let mut items = match items {
            Some(lista) => validar_items(lista)?,
            None => Vec::new(),
        };
        if let Some(order_uuid) = order_uuid {
            let orden = sqlx::query("SELECT items_json FROM orders.orden_vista WHERE id = $1")
                .bind(order_uuid)
                .fetch_optional(self.db.pool())
                .await
                .map_err(error_db)?;
            let Some(orden) = orden else {
                return Err(ApiError::no_encontrado(
                    "Orden",
                    order_id.as_deref().unwrap_or(""),
                ));
            };
            let items_json = jsonb(&orden, "items_json").map_err(error_db)?;
            let de_empresa: Vec<Value> = items_json
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| pertenece_a_empresa(item, empresa_id))
                .cloned()
                .collect();
            if items.is_empty() {
                items = items_desde_orden(&de_empresa);
            }
            if items.is_empty() {
                return Err(ApiError::dominio(
                    "COMANDA_SIN_ITEMS",
                    "La orden no tiene items para preparar.",
                ));
            }
        }

        let items_json = serde_json::to_value(&items).map_err(|_| ApiError::interno())?;
        let resultado = sqlx::query(&format!(
            "INSERT INTO orders.comandas
               (empresa_id, order_id, mesa_id, estacion, notas, items_json)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb)
             RETURNING {COLUMNAS_COMANDA}"
        ))
        .bind(empresa_uuid)
        .bind(order_uuid)
        .bind(mesa_uuid)
        .bind(&estacion)
        .bind(&notas)
        .bind(sqlx::types::Json(items_json))
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = resultado else {
            return Err(ApiError::dominio(
                "COMANDA_NO_CREADA",
                "No se pudo crear la comanda.",
            ));
        };

        if let Some(mesa_uuid) = mesa_uuid {
            sqlx::query(
                "UPDATE orders.mesas
                    SET estado = 'ocupada', actualizado_en = NOW()
                  WHERE id = $1 AND empresa_id = $2
                    AND estado IN ('libre', 'reservada')",
            )
            .bind(mesa_uuid)
            .bind(empresa_uuid)
            .execute(self.db.pool())
            .await
            .map_err(error_db)?;
        }
        comanda_de_fila(&fila)
    }

    pub async fn actualizar_comanda(
        &self,
        empresa_id: &str,
        id: &str,
        input: ActualizarComandaInput,
    ) -> Result<Comanda, ApiError> {
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let comanda_uuid = validar_uuid(id, "comanda")?;
        let actual = self
            .comanda_por_uuid(empresa_uuid, comanda_uuid, id)
            .await?;

        let estado = input.estado.unwrap_or_else(|| actual.estado.clone());
        validar_transicion_comanda(&actual.estado, &estado)?;
        let notas = match input.notas {
            Some(valor) => recortar(valor.trim(), 500),
            None => actual.notas,
        };

        let fila = sqlx::query(&format!(
            "UPDATE orders.comandas
                SET estado = $3, notas = $4,
                    listo_en = CASE WHEN $3 = 'lista' THEN COALESCE(listo_en, NOW())
                                    ELSE listo_en END,
                    entregado_en = CASE WHEN $3 = 'entregada' THEN COALESCE(entregado_en, NOW())
                                        ELSE entregado_en END,
                    actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2
              RETURNING {COLUMNAS_COMANDA}"
        ))
        .bind(comanda_uuid)
        .bind(empresa_uuid)
        .bind(&estado)
        .bind(&notas)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Comanda", id));
        };
        comanda_de_fila(&fila)
    }

    // ── helpers de acceso ───────────────────────────────────────────────────

    async fn mesa_por_uuid(
        &self,
        empresa_uuid: Uuid,
        mesa_uuid: Uuid,
        mesa_id: &str,
    ) -> Result<Mesa, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_MESA}
               FROM orders.mesas
              WHERE id = $1 AND empresa_id = $2"
        ))
        .bind(mesa_uuid)
        .bind(empresa_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        match fila {
            Some(fila) => mesa_de_fila(&fila),
            None => Err(ApiError::no_encontrado("Mesa", mesa_id)),
        }
    }

    async fn comanda_por_uuid(
        &self,
        empresa_uuid: Uuid,
        comanda_uuid: Uuid,
        comanda_id: &str,
    ) -> Result<Comanda, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_COMANDA}
               FROM orders.comandas
              WHERE id = $1 AND empresa_id = $2"
        ))
        .bind(comanda_uuid)
        .bind(empresa_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        match fila {
            Some(fila) => comanda_de_fila(&fila),
            None => Err(ApiError::no_encontrado("Comanda", comanda_id)),
        }
    }
}

// ── validación pura ─────────────────────────────────────────────────────────

fn validar_uuid(valor: &str, campo: &str) -> Result<Uuid, ApiError> {
    parsear_uuid(valor)
        .map_err(|_| ApiError::dominio("ID_INVALIDO", format!("El {campo} debe ser un UUID.")))
}

fn validar_uuid_opcional(valor: Option<&str>, campo: &str) -> Result<Option<Uuid>, ApiError> {
    match valor {
        None | Some("") => Ok(None),
        Some(valor) => validar_uuid(valor, campo).map(Some),
    }
}

fn validar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim();
    let valido = (1..=20).contains(&codigo.chars().count())
        && codigo
            .chars()
            .all(|caracter| caracter.is_ascii_alphanumeric() || caracter == '_' || caracter == '-');
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            "El codigo de mesa no es valido.",
        ));
    }
    Ok(codigo.to_string())
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    let largo = nombre.chars().count();
    if !(1..=120).contains(&largo) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 1 y 120 caracteres.",
        ));
    }
    Ok(nombre.to_string())
}

fn validar_capacidad(valor: i64) -> Result<i64, ApiError> {
    if !(1..=999).contains(&valor) {
        return Err(ApiError::dominio(
            "CAPACIDAD_INVALIDA",
            "La capacidad debe estar entre 1 y 999.",
        ));
    }
    Ok(valor)
}

fn capacidad_como_i32(valor: i64) -> Result<i32, ApiError> {
    i32::try_from(valor).map_err(|_| ApiError::interno())
}

fn validar_estado_mesa(valor: &str) -> Result<String, ApiError> {
    if !ESTADOS_MESA.contains(&valor) {
        return Err(ApiError::dominio(
            "ESTADO_MESA_INVALIDO",
            "El estado de la mesa no es valido.",
        ));
    }
    Ok(valor.to_string())
}

fn normalizar_zona(valor: Option<&str>) -> String {
    recortar(valor.unwrap_or_default().trim(), 80)
}

fn validar_estacion(valor: &str) -> Result<String, ApiError> {
    let estacion = valor.trim();
    let largo = estacion.chars().count();
    if !(2..=80).contains(&largo) {
        return Err(ApiError::dominio(
            "ESTACION_INVALIDA",
            "La estacion debe tener entre 2 y 80 caracteres.",
        ));
    }
    Ok(estacion.to_string())
}

fn normalizar_notas(valor: Option<&str>) -> String {
    recortar(valor.unwrap_or_default().trim(), 500)
}

fn recortar(valor: &str, maximo: usize) -> String {
    valor.chars().take(maximo).collect()
}

fn validar_items(items: Vec<ComandaItem>) -> Result<Vec<ComandaItem>, ApiError> {
    if items.is_empty() {
        return Err(ApiError::dominio(
            "COMANDA_SIN_ITEMS",
            "La comanda requiere al menos un item.",
        ));
    }
    items
        .into_iter()
        .map(|item| {
            let sku = item.sku.trim().to_string();
            let nombre = item.nombre.trim().to_string();
            if sku.is_empty() || nombre.is_empty() || item.cantidad < 1 {
                return Err(ApiError::dominio(
                    "ITEM_COMANDA_INVALIDO",
                    "Cada item requiere sku, nombre y cantidad.",
                ));
            }
            let notas = item
                .notas
                .filter(|notas| !notas.is_empty())
                .map(|notas| recortar(notas.trim(), 300));
            Ok(ComandaItem {
                sku,
                nombre,
                cantidad: item.cantidad,
                notas,
            })
        })
        .collect()
}

fn validar_transicion_comanda(actual: &str, nuevo: &str) -> Result<(), ApiError> {
    let permitidas: &[&str] = match actual {
        "pendiente" => &["pendiente", "en_preparacion", "cancelada"],
        "en_preparacion" => &["en_preparacion", "lista", "cancelada"],
        "lista" => &["lista", "entregada", "cancelada"],
        _ => &[],
    };
    if !permitidas.contains(&nuevo) {
        return Err(ApiError::conflicto(format!(
            "No se puede pasar de {actual} a {nuevo}."
        )));
    }
    Ok(())
}

fn pertenece_a_empresa(item: &Value, empresa_id: &str) -> bool {
    item.get("empresa_id")
        .and_then(Value::as_str)
        .is_none_or(|item_empresa| item_empresa.is_empty() || item_empresa == empresa_id)
}

fn items_desde_orden(items: &[Value]) -> Vec<ComandaItem> {
    items
        .iter()
        .filter_map(|item| {
            let sku = item
                .get("sku")
                .and_then(Value::as_str)
                .filter(|sku| !sku.is_empty())?;
            let cantidad = item
                .get("cantidad")
                .and_then(Value::as_i64)
                .filter(|cantidad| *cantidad > 0)?;
            let nombre = item
                .get("producto_nombre")
                .and_then(Value::as_str)
                .unwrap_or(sku);
            Some(ComandaItem {
                sku: sku.to_string(),
                nombre: nombre.to_string(),
                cantidad,
                notas: None,
            })
        })
        .collect()
}

fn es_conflicto(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(base)
            if matches!(base.code().as_deref(), Some("23505" | "23P01"))
    )
}

// ── serialización de filas ──────────────────────────────────────────────────

fn mesa_de_fila(fila: &PgRow) -> Result<Mesa, ApiError> {
    let capacidad: i32 = fila.try_get("capacidad").map_err(error_db)?;
    Ok(Mesa {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        codigo: fila.try_get("codigo").map_err(error_db)?,
        nombre: fila.try_get("nombre").map_err(error_db)?,
        capacidad: i64::from(capacidad),
        zona: fila.try_get("zona").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn comanda_de_fila(fila: &PgRow) -> Result<Comanda, ApiError> {
    let items_json = jsonb(fila, "items_json").map_err(error_db)?;
    Ok(Comanda {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        mesa_id: uuid_opcional(fila, "mesa_id").map_err(error_db)?,
        estacion: fila.try_get("estacion").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        notas: fila.try_get("notas").map_err(error_db)?,
        items: serde_json::from_value(items_json).unwrap_or_default(),
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
        listo_en: timestamp_opcional(fila, "listo_en").map_err(error_db)?,
        entregado_en: timestamp_opcional(fila, "entregado_en").map_err(error_db)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_el_codigo_de_mesa() {
        assert_eq!(validar_codigo(" M-01 ").expect("codigo"), "M-01");
        assert_eq!(validar_codigo("mesa_1").expect("codigo"), "mesa_1");
        for invalido in ["", "M 01", "MESA.CON.PUNTO", &"x".repeat(21)] {
            assert_eq!(
                validar_codigo(invalido).expect_err("invalido").codigo,
                "CODIGO_INVALIDO"
            );
        }
    }

    #[test]
    fn debe_validar_y_recortar_el_nombre() {
        assert_eq!(validar_nombre("  Mesa 1  ").expect("nombre"), "Mesa 1");
        assert_eq!(
            validar_nombre("").expect_err("vacio").codigo,
            "NOMBRE_INVALIDO"
        );
        assert_eq!(
            validar_nombre(&"x".repeat(121)).expect_err("largo").codigo,
            "NOMBRE_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_la_capacidad() {
        assert_eq!(validar_capacidad(1).expect("minima"), 1);
        assert_eq!(validar_capacidad(999).expect("maxima"), 999);
        for invalida in [0, 1000, -1] {
            assert_eq!(
                validar_capacidad(invalida).expect_err("invalida").codigo,
                "CAPACIDAD_INVALIDA"
            );
        }
    }

    #[test]
    fn debe_validar_el_estado_de_la_mesa() {
        for estado in ESTADOS_MESA {
            assert_eq!(validar_estado_mesa(estado).expect("estado"), estado);
        }
        assert_eq!(
            validar_estado_mesa("rota").expect_err("invalido").codigo,
            "ESTADO_MESA_INVALIDO"
        );
    }

    #[test]
    fn debe_normalizar_zona_y_notas() {
        assert_eq!(normalizar_zona(None), "");
        assert_eq!(normalizar_zona(Some("  terraza  ")), "terraza");
        assert_eq!(normalizar_zona(Some(&"z".repeat(100))).chars().count(), 80);
        assert_eq!(normalizar_notas(Some("  sin sal  ")), "sin sal");
        assert_eq!(
            normalizar_notas(Some(&"n".repeat(600))).chars().count(),
            500
        );
    }

    #[test]
    fn debe_validar_la_estacion() {
        assert_eq!(validar_estacion(" cocina ").expect("estacion"), "cocina");
        for invalida in ["", "x", &"x".repeat(81)] {
            assert_eq!(
                validar_estacion(invalida).expect_err("invalida").codigo,
                "ESTACION_INVALIDA"
            );
        }
    }

    #[test]
    fn debe_validar_los_items_de_la_comanda() {
        let items = validar_items(vec![ComandaItem {
            sku: " PLATO-1 ".into(),
            nombre: " Plato ".into(),
            cantidad: 2,
            notas: Some("  sin sal  ".into()),
        }])
        .expect("items");
        assert_eq!(items[0].sku, "PLATO-1");
        assert_eq!(items[0].nombre, "Plato");
        assert_eq!(items[0].notas.as_deref(), Some("sin sal"));

        assert_eq!(
            validar_items(Vec::new()).expect_err("vacio").codigo,
            "COMANDA_SIN_ITEMS"
        );
        let error = validar_items(vec![ComandaItem {
            sku: String::new(),
            nombre: "Plato".into(),
            cantidad: 1,
            notas: None,
        }])
        .expect_err("invalido");
        assert_eq!(error.codigo, "ITEM_COMANDA_INVALIDO");
    }

    #[test]
    fn debe_omitir_las_notas_vacias_de_un_item() {
        let items = validar_items(vec![ComandaItem {
            sku: "SKU".into(),
            nombre: "Producto".into(),
            cantidad: 1,
            notas: Some(String::new()),
        }])
        .expect("items");
        assert_eq!(items[0].notas, None);
        assert_eq!(
            serde_json::to_value(&items[0]).expect("json"),
            json!({"sku": "SKU", "nombre": "Producto", "cantidad": 1})
        );
    }

    #[test]
    fn debe_validar_las_transiciones_de_comanda() {
        for (actual, nuevo) in [
            ("pendiente", "pendiente"),
            ("pendiente", "en_preparacion"),
            ("pendiente", "cancelada"),
            ("en_preparacion", "lista"),
            ("en_preparacion", "cancelada"),
            ("lista", "entregada"),
            ("lista", "cancelada"),
        ] {
            assert!(validar_transicion_comanda(actual, nuevo).is_ok());
        }
        for (actual, nuevo) in [
            ("pendiente", "lista"),
            ("pendiente", "entregada"),
            ("en_preparacion", "entregada"),
            ("lista", "pendiente"),
            ("entregada", "cancelada"),
            ("cancelada", "pendiente"),
        ] {
            let error = validar_transicion_comanda(actual, nuevo).expect_err("invalida");
            assert_eq!(error.codigo, "CONFLICTO");
        }
    }

    #[test]
    fn debe_derivar_items_desde_la_orden() {
        let items = items_desde_orden(&[
            json!({"sku": "PLATO-1", "producto_nombre": "Plato", "cantidad": 2}),
            json!({"sku": "BEB-1", "cantidad": 1}),
            json!({"sku": "", "producto_nombre": "Sin sku", "cantidad": 1}),
            json!({"sku": "CERO", "producto_nombre": "Cero", "cantidad": 0}),
        ]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].nombre, "Plato");
        assert_eq!(items[1].nombre, "BEB-1");
        assert_eq!(items[1].notas, None);
    }

    #[test]
    fn debe_filtrar_items_por_empresa() {
        assert!(pertenece_a_empresa(&json!({"sku": "A"}), "e-1"));
        assert!(pertenece_a_empresa(
            &json!({"sku": "A", "empresa_id": ""}),
            "e-1"
        ));
        assert!(pertenece_a_empresa(
            &json!({"sku": "A", "empresa_id": "e-1"}),
            "e-1"
        ));
        assert!(!pertenece_a_empresa(
            &json!({"sku": "A", "empresa_id": "e-2"}),
            "e-1"
        ));
    }

    #[test]
    fn debe_serializar_comandas_con_los_nombres_del_contrato() {
        let comanda = Comanda {
            id: "c-1".into(),
            empresa_id: "e-1".into(),
            order_id: None,
            mesa_id: Some("m-1".into()),
            estacion: "cocina".into(),
            estado: "pendiente".into(),
            notas: String::new(),
            items: Vec::new(),
            creado_en: "2026-11-13T00:00:00.000Z".into(),
            actualizado_en: "2026-11-13T00:00:00.000Z".into(),
            listo_en: None,
            entregado_en: None,
        };
        let valor = serde_json::to_value(comanda).expect("json");
        assert_eq!(valor["order_id"], Value::Null);
        assert_eq!(valor["mesa_id"], "m-1");
        assert_eq!(valor["listo_en"], Value::Null);
        assert_eq!(valor["entregado_en"], Value::Null);
        assert_eq!(valor["items"], json!([]));
    }
}
