//! Logs estructurados y contexto de correlación.
//!
//! Réplica de `logging/logger.ts`: cada línea es JSON con `nivel`, `ts`,
//! `servicio` y `request_id`, y el contexto viaja por tarea (equivalente a
//! `AsyncLocalStorage`).

use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Value};
use std::future::Future;
use tokio::task_local;

#[derive(Debug, Clone, Default)]
pub struct ContextoCorrelacion {
    pub request_id: String,
    pub servicio: String,
    pub usuario_id: Option<String>,
    pub rol: Option<String>,
    pub tenant_id: Option<String>,
}

task_local! {
    static CONTEXTO: ContextoCorrelacion;
}

/// Ejecuta un futuro dentro del contexto de correlación de la petición.
pub async fn ejecutar_con_contexto<F, T>(contexto: ContextoCorrelacion, futuro: F) -> T
where
    F: Future<Output = T>,
{
    CONTEXTO.scope(contexto, futuro).await
}

pub fn contexto_actual() -> Option<ContextoCorrelacion> {
    CONTEXTO.try_with(|ctx| ctx.clone()).ok()
}

fn linea(nivel: &str, servicio: &str, campos: Map<String, Value>) -> String {
    let ctx = contexto_actual();
    let mut salida = Map::new();
    salida.insert("nivel".into(), Value::String(nivel.into()));
    salida.insert(
        "ts".into(),
        Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
    );
    salida.insert("servicio".into(), Value::String(servicio.to_string()));
    if let Some(ctx) = &ctx {
        salida.insert("request_id".into(), Value::String(ctx.request_id.clone()));
    }
    for (clave, valor) in campos {
        salida.insert(clave, valor);
    }
    Value::Object(salida).to_string()
}

pub fn info(servicio: &str, campos: Map<String, Value>) {
    println!("{}", linea("info", servicio, campos));
}

pub fn warn(servicio: &str, campos: Map<String, Value>) {
    eprintln!("{}", linea("warn", servicio, campos));
}

pub fn error(servicio: &str, campos: Map<String, Value>) {
    eprintln!("{}", linea("error", servicio, campos));
}

/// Azúcar para construir campos sin repetir `Map::new()`.
#[macro_export]
macro_rules! campos {
    ($($clave:literal : $valor:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut mapa = serde_json::Map::new();
        $(mapa.insert($clave.to_string(), serde_json::json!($valor));)*
        mapa
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn debe_incluir_el_request_id_del_contexto() {
        let ctx = ContextoCorrelacion {
            request_id: "req-1".into(),
            servicio: "field-service".into(),
            ..Default::default()
        };
        let texto = ejecutar_con_contexto(ctx, async {
            linea("info", "fallback", campos! { "msg": "peticion" })
        })
        .await;
        let valor: Value = serde_json::from_str(&texto).expect("json");
        assert_eq!(valor["request_id"], "req-1");
        assert_eq!(valor["servicio"], "fallback");
        assert_eq!(valor["nivel"], "info");
        assert_eq!(valor["msg"], "peticion");
        assert!(valor["ts"].as_str().expect("ts").ends_with('Z'));
    }

    #[tokio::test]
    async fn debe_usar_el_servicio_por_defecto_sin_contexto() {
        let texto = linea("warn", "http", campos! { "msg": "sin contexto" });
        let valor: Value = serde_json::from_str(&texto).expect("json");
        assert_eq!(valor["servicio"], "http");
        assert!(valor.get("request_id").is_none());
    }
}
