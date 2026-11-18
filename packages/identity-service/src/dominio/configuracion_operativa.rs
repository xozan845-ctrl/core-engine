//! Configuración operativa efectiva de una empresa.
//!
//! Se deriva de los modelos de operación del pack asignado. Los servicios la
//! consumen para validar defaults (tipos de ítem permitidos, fulfillment) sin
//! duplicar la lógica de negocio de cada rubro.

use crate::db::Db;
use crate::dominio::slug::es_uuid;
use crate::errors::ApiError;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackRef {
    pub codigo: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfiguracionOperativa {
    pub empresa_id: String,
    pub vertical_id: Option<String>,
    pub pack: Option<PackRef>,
    pub modelos: Vec<String>,
    pub items: Vec<String>,
    pub fulfillment: Vec<String>,
    pub requiere_agenda: bool,
    pub trazabilidad: bool,
    pub requiere_vendedor: bool,
    pub comision: bool,
}

pub struct ConfiguracionOperativaService {
    db: Arc<Db>,
}

impl ConfiguracionOperativaService {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn obtener(&self, empresa_id: &str) -> Result<ConfiguracionOperativa, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let mut tx = self.db.transaccion_servicio().await.map_err(interno)?;
        let empresa = sqlx::query(
            "SELECT vertical_id, pack_codigo, pack_version
               FROM identity.empresas WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let vertical_id: Option<String> = empresa.try_get("vertical_id").map_err(interno)?;
        let pack_codigo: Option<String> = empresa.try_get("pack_codigo").map_err(interno)?;
        let pack_version: Option<i32> = empresa.try_get("pack_version").map_err(interno)?;

        let mut modelos = Vec::new();
        let mut pack = None;
        if let (Some(codigo), Some(version)) = (pack_codigo.as_deref(), pack_version) {
            let filas = sqlx::query(
                "SELECT mo.codigo, mo.config
                   FROM identity.pack_modelos pm
                   JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
                  WHERE pm.pack_codigo = $1 AND pm.pack_version = $2
                    AND pm.activo = true AND mo.activo = true
                  ORDER BY mo.codigo",
            )
            .bind(codigo)
            .bind(version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            for fila in &filas {
                let codigo_modelo: String = fila.try_get("codigo").map_err(interno)?;
                let config: Value = fila.try_get("config").map_err(interno)?;
                modelos.push((codigo_modelo, config));
            }
            pack = Some(PackRef {
                codigo: codigo.to_string(),
                version,
            });
        }
        tx.commit().await.map_err(interno)?;

        let (items, fulfillment, requiere_agenda, trazabilidad, requiere_vendedor, comision) =
            combinar(&modelos);
        Ok(ConfiguracionOperativa {
            empresa_id: empresa_id.to_string(),
            vertical_id,
            pack,
            modelos: modelos.into_iter().map(|(codigo, _)| codigo).collect(),
            items,
            fulfillment,
            requiere_agenda,
            trazabilidad,
            requiere_vendedor,
            comision,
        })
    }
}

/// Une los defaults de todos los modelos del pack. Listas vacías significan
/// "sin restricción" para no romper packs antiguos sin `config`. Los flags
/// ausentes conservan el comportamiento de marketplace (`true`); basta que un
/// modelo declare `false` para desactivarlo en el pack.
fn combinar(modelos: &[(String, Value)]) -> (Vec<String>, Vec<String>, bool, bool, bool, bool) {
    let mut items = Vec::new();
    let mut fulfillment = Vec::new();
    let mut requiere_agenda = false;
    let mut trazabilidad = false;
    let mut requiere_vendedor = true;
    let mut comision = true;
    for (_, config) in modelos {
        if let Some(lista) = config.get("items").and_then(Value::as_array) {
            items.extend(lista.iter().filter_map(Value::as_str).map(str::to_string));
        }
        if let Some(lista) = config.get("fulfillment").and_then(Value::as_array) {
            fulfillment.extend(lista.iter().filter_map(Value::as_str).map(str::to_string));
        }
        if config.get("requiere_agenda").and_then(Value::as_bool) == Some(true) {
            requiere_agenda = true;
        }
        if config.get("trazabilidad").and_then(Value::as_bool) == Some(true) {
            trazabilidad = true;
        }
        if config.get("requiere_vendedor").and_then(Value::as_bool) == Some(false) {
            requiere_vendedor = false;
        }
        if config.get("comision").and_then(Value::as_bool) == Some(false) {
            comision = false;
        }
    }
    items.sort();
    items.dedup();
    fulfillment.sort();
    fulfillment.dedup();
    (
        items,
        fulfillment,
        requiere_agenda,
        trazabilidad,
        requiere_vendedor,
        comision,
    )
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "db",
        crate::campos! { "msg": "Error de PostgreSQL", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_unir_y_deduplicar_los_defaults() {
        let modelos = vec![
            ("comercio".to_string(), json!({ "items": ["producto"] })),
            (
                "mixto".to_string(),
                json!({
                    "items": ["servicio", "producto"],
                    "fulfillment": ["delivery", "pickup"],
                    "requiere_agenda": true,
                    "trazabilidad": true
                }),
            ),
        ];
        let (items, fulfillment, agenda, trazabilidad, requiere_vendedor, comision) =
            combinar(&modelos);
        assert_eq!(items, vec!["producto", "servicio"]);
        assert_eq!(fulfillment, vec!["delivery", "pickup"]);
        assert!(agenda);
        assert!(trazabilidad);
        assert!(requiere_vendedor);
        assert!(comision);
    }

    #[test]
    fn debe_devolver_listas_vacias_sin_config() {
        let (items, fulfillment, agenda, trazabilidad, requiere_vendedor, comision) =
            combinar(&[("comercio".to_string(), json!({}))]);
        assert!(items.is_empty());
        assert!(fulfillment.is_empty());
        assert!(!agenda);
        assert!(!trazabilidad);
        assert!(requiere_vendedor);
        assert!(comision);
    }

    #[test]
    fn debe_desactivar_marketplace_cuando_un_modelo_declara_venta_directa() {
        let modelos = vec![
            ("comercio".to_string(), json!({ "items": ["producto"] })),
            (
                "venta_directa".to_string(),
                json!({
                    "requiere_vendedor": false,
                    "comision": false
                }),
            ),
        ];
        let (_, _, _, _, requiere_vendedor, comision) = combinar(&modelos);
        assert!(!requiere_vendedor);
        assert!(!comision);
    }
}
