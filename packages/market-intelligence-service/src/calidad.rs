//! Data Contract y score de completitud de `venta.geolocalizada`.
//!
//! Réplica del JSON Schema draft-07 + reglas de negocio de
//! `DataQualityService`, incluida la persistencia en `invalid_events`.

use crate::db::Db;
use crate::logs;
use crate::metrics::Metrics;
use serde_json::{Map, Value};
use std::sync::Arc;

const EVENTOS_INVALIDOS: &str = "core_engine_intelligence_invalid_events_total";
const COMPLETENESS_HIGH: &str = "core_engine_intelligence_data_quality_high_total";
const COMPLETENESS_MED: &str = "core_engine_intelligence_data_quality_medium_total";
const COMPLETENESS_LOW: &str = "core_engine_intelligence_data_quality_low_total";

const OPCIONALES: [&str; 10] = [
    "lat",
    "lng",
    "precision",
    "velocidad",
    "rumbo",
    "tipo_actividad",
    "resultado_visita",
    "distancia_cliente_metros",
    "rango_edad",
    "genero",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoCalidad {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub completeness_score: f64,
    pub quality_tier: String,
}

pub struct CalidadDatos {
    metrics: Arc<Metrics>,
    db: Option<Arc<Db>>,
}

impl CalidadDatos {
    pub fn nueva(metrics: Arc<Metrics>, db: Arc<Db>) -> Self {
        Self {
            metrics,
            db: Some(db),
        }
    }

    /// Variante sin PostgreSQL para pruebas de validación y arranque aislado.
    pub fn solo_metricas(metrics: Arc<Metrics>) -> Self {
        Self { metrics, db: None }
    }

    pub fn validar(&self, event: &Value) -> ResultadoCalidad {
        let schema_errors = validar_schema(event);
        let mut errors = schema_errors;
        let schema_ok = errors.is_empty();

        if let Some(lat) = event.get("lat").and_then(Value::as_f64) {
            if !(-90.0..=90.0).contains(&lat) {
                errors.push("lat fuera de rango válido [-90, 90]".to_string());
            }
        }
        if let Some(monto) = event.get("monto_cents").and_then(Value::as_f64) {
            if monto <= 0.0 {
                errors.push("monto_cents debe ser positivo".to_string());
            }
        }

        let is_valid = schema_ok && errors.is_empty();
        if !is_valid {
            logs::warn(
                "inteligencia.calidad",
                crate::campos! {
                    "msg": "Evento inválido detectado",
                    "errores": errors,
                },
            );
            self.metrics.incrementar_contador(EVENTOS_INVALIDOS);
        }

        let mut rellenos = 0_usize;
        for campo in OPCIONALES {
            let presente = event.get(campo).is_some_and(|valor| {
                !valor.is_null() && !matches!(valor, Value::String(texto) if texto.is_empty())
            });
            if presente {
                rellenos += 1;
            }
        }
        let completeness_score = rellenos as f64 / OPCIONALES.len() as f64;
        let quality_tier = if completeness_score > 0.9 {
            "HIGH"
        } else if completeness_score > 0.5 {
            "MEDIUM"
        } else {
            "LOW"
        };
        self.metrics.incrementar_contador(match quality_tier {
            "HIGH" => COMPLETENESS_HIGH,
            "MEDIUM" => COMPLETENESS_MED,
            _ => COMPLETENESS_LOW,
        });

        ResultadoCalidad {
            is_valid,
            errors,
            completeness_score,
            quality_tier: quality_tier.to_string(),
        }
    }

    /// Fire-and-forget: un fallo de auditoría no debe detener el pipeline.
    pub async fn registrar_invalido(
        &self,
        event_id: Option<&str>,
        event_tipo: &str,
        errores: &[String],
        payload: &Value,
    ) {
        let Some(db) = &self.db else {
            return;
        };
        let resultado = sqlx::query(
            "INSERT INTO intelligence.invalid_events (event_id, event_tipo, errores, payload)
             VALUES ($1, $2, $3, $4::jsonb)",
        )
        .bind(event_id)
        .bind(event_tipo)
        .bind(errores.to_vec())
        .bind(payload.to_string())
        .execute(db.pool())
        .await;
        if let Err(error) = resultado {
            logs::error(
                "inteligencia.calidad",
                crate::campos! {
                    "msg": "No se pudo persistir evento inválido",
                    "err": error.to_string(),
                },
            );
        }
    }
}

fn validar_schema(event: &Value) -> Vec<String> {
    let Some(objeto) = event.as_object() else {
        return vec!["must be object".to_string()];
    };
    let mut errors = Vec::new();

    for requerido in ["order_id", "vendedor_id", "skus", "monto_cents"] {
        if !objeto.contains_key(requerido) {
            errors.push(format!(" must have required property '{requerido}'"));
        }
    }

    let permitidos = [
        "order_id",
        "vendedor_id",
        "skus",
        "monto_cents",
        "lat",
        "lng",
        "precision",
        "velocidad",
        "rumbo",
        "tipo_actividad",
        "resultado_visita",
        "distancia_cliente_metros",
        "rango_edad",
        "genero",
    ];
    if objeto
        .keys()
        .any(|clave| !permitidos.contains(&clave.as_str()))
    {
        errors.push("must NOT have additional properties".to_string());
    }

    validar_texto(objeto, "order_id", &mut errors);
    validar_texto(objeto, "vendedor_id", &mut errors);
    validar_texto(objeto, "tipo_actividad", &mut errors);
    validar_texto(objeto, "resultado_visita", &mut errors);
    validar_texto(objeto, "rango_edad", &mut errors);

    match objeto.get("skus") {
        Some(Value::Array(skus)) => {
            if skus.is_empty() {
                errors.push("/skus must NOT have fewer than 1 items".to_string());
            }
            for (indice, sku) in skus.iter().enumerate() {
                if !sku.is_string() {
                    errors.push(format!("/skus/{indice} must be string"));
                }
            }
        }
        Some(_) => errors.push("/skus must be array".to_string()),
        None => {}
    }

    validar_entero(objeto, "monto_cents", 0.0, f64::INFINITY, &mut errors);
    validar_numero(objeto, "lat", -90.0, 90.0, &mut errors);
    validar_numero(objeto, "lng", -180.0, 180.0, &mut errors);
    validar_numero(objeto, "precision", 0.0, f64::INFINITY, &mut errors);
    validar_numero(objeto, "velocidad", 0.0, f64::INFINITY, &mut errors);
    validar_numero(objeto, "rumbo", 0.0, 360.0, &mut errors);
    validar_numero(
        objeto,
        "distancia_cliente_metros",
        0.0,
        f64::INFINITY,
        &mut errors,
    );

    if let Some(genero) = objeto.get("genero") {
        if !genero.is_string() {
            errors.push("/genero must be string".to_string());
        } else if !matches!(genero.as_str(), Some("M" | "F" | "NS")) {
            errors.push("/genero must be equal to one of the allowed values".to_string());
        }
    }

    errors
}

fn validar_texto(objeto: &Map<String, Value>, campo: &str, errors: &mut Vec<String>) {
    if let Some(valor) = objeto.get(campo) {
        if !valor.is_string() {
            errors.push(format!("/{campo} must be string"));
        }
    }
}

fn validar_numero(
    objeto: &Map<String, Value>,
    campo: &str,
    minimo: f64,
    maximo: f64,
    errors: &mut Vec<String>,
) {
    let Some(valor) = objeto.get(campo) else {
        return;
    };
    let Some(numero) = valor.as_f64() else {
        errors.push(format!("/{campo} must be number"));
        return;
    };
    if numero < minimo {
        errors.push(format!("/{campo} must be >= {minimo}"));
    }
    if numero > maximo {
        errors.push(format!("/{campo} must be <= {maximo}"));
    }
}

fn validar_entero(
    objeto: &Map<String, Value>,
    campo: &str,
    minimo: f64,
    maximo: f64,
    errors: &mut Vec<String>,
) {
    let Some(valor) = objeto.get(campo) else {
        return;
    };
    let Some(numero) = valor.as_f64() else {
        errors.push(format!("/{campo} must be integer"));
        return;
    };
    if numero.fract() != 0.0 {
        errors.push(format!("/{campo} must be integer"));
    }
    if numero < minimo {
        errors.push(format!("/{campo} must be >= {minimo}"));
    }
    if numero > maximo {
        errors.push(format!("/{campo} must be <= {maximo}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;

    fn servicio() -> CalidadDatos {
        CalidadDatos::solo_metricas(Arc::new(Metrics::nueva("market-intelligence-service")))
    }

    fn venta(sobre: Value) -> Value {
        let mut base = serde_json::json!({
            "order_id": "o-1",
            "vendedor_id": "v-1",
            "skus": ["SKU-1"],
            "monto_cents": 1000,
        });
        if let (Some(objeto), Some(extra)) = (base.as_object_mut(), sobre.as_object()) {
            for (clave, valor) in extra {
                objeto.insert(clave.clone(), valor.clone());
            }
        }
        base
    }

    #[test]
    fn debe_marcar_high_con_todos_los_opcionales() {
        let servicio = servicio();
        let resultado = servicio.validar(&venta(serde_json::json!({
            "lat": 12.1, "lng": -86.2, "precision": 5, "velocidad": 20, "rumbo": 180,
            "tipo_actividad": "visita", "resultado_visita": "exitosa",
            "distancia_cliente_metros": 250, "rango_edad": "25-34", "genero": "NS"
        })));
        assert!(resultado.is_valid);
        assert_eq!(resultado.completeness_score, 1.0);
        assert_eq!(resultado.quality_tier, "HIGH");
        assert!(resultado.errors.is_empty());
    }

    #[test]
    fn debe_calcular_medium_low_y_contar_el_cero() {
        let servicio = servicio();
        let medio = servicio.validar(&venta(serde_json::json!({
            "lat": 12, "lng": -86, "precision": 4, "velocidad": 10, "rumbo": 90, "tipo_actividad": "venta"
        })));
        assert_eq!(medio.completeness_score, 0.6);
        assert_eq!(medio.quality_tier, "MEDIUM");

        let cero = servicio.validar(&venta(serde_json::json!({
            "lat": 0, "lng": 0, "precision": 0, "velocidad": 0, "rumbo": 0,
            "tipo_actividad": "", "resultado_visita": ""
        })));
        assert_eq!(cero.completeness_score, 0.5);
        assert_eq!(cero.quality_tier, "LOW");
    }

    #[test]
    fn debe_rechazar_requeridos_y_propiedades_desconocidas() {
        let servicio = servicio();
        let resultado =
            servicio.validar(&serde_json::json!({"order_id": "o-1", "inesperado": true}));
        assert!(!resultado.is_valid);
        assert!(resultado
            .errors
            .iter()
            .any(|error| error.contains("must have required property")));
        assert!(resultado
            .errors
            .iter()
            .any(|error| error.contains("must NOT have additional properties")));
    }

    #[test]
    fn debe_aplicar_reglas_de_negocio_y_tipos() {
        let servicio = servicio();
        let resultado = servicio.validar(&venta(serde_json::json!({"monto_cents": 0, "lat": 91})));
        assert!(!resultado.is_valid);
        assert!(resultado
            .errors
            .contains(&"monto_cents debe ser positivo".to_string()));
        assert!(resultado
            .errors
            .contains(&"lat fuera de rango válido [-90, 90]".to_string()));

        let tipos = servicio.validar(&venta(serde_json::json!({
            "skus": [], "monto_cents": 100.5, "lng": 181, "genero": "X"
        })));
        assert!(!tipos.is_valid);
        assert!(tipos.errors.iter().any(|error| error.contains("/skus")));
        assert!(tipos.errors.iter().any(|error| error.contains("/lng")));
        assert!(tipos.errors.iter().any(|error| error.contains("/genero")));
    }
}
