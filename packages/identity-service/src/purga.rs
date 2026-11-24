//! Purga coordinada de datos de un tenant en otros servicios.
//!
//! Identity es el dueño del ciclo de vida del tenant, pero no de sus datos.
//! Tras anonimizar su propio esquema, hace fan-out a los servicios
//! configurados en `TENANT_PURGE_SERVICES` (`nombre=url,...`) y reporta el
//! resultado por servicio sin revertir la purga de identidad.

use crate::errors::ApiError;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicioPurga {
    pub nombre: String,
    pub url: String,
}

impl ServicioPurga {
    /// Parsea `nombre=url,nombre=url`; ignora entradas vacías.
    pub fn parsear(valor: &str) -> Vec<Self> {
        valor
            .split(',')
            .filter_map(|entrada| {
                let (nombre, url) = entrada.split_once('=')?;
                let nombre = nombre.trim();
                let url = url.trim().trim_end_matches('/');
                if nombre.is_empty() || url.is_empty() {
                    return None;
                }
                Some(Self {
                    nombre: nombre.to_string(),
                    url: url.to_string(),
                })
            })
            .collect()
    }
}

pub struct PurgaExterna {
    servicios: Vec<ServicioPurga>,
    clave: Option<String>,
    cliente: reqwest::Client,
}

impl PurgaExterna {
    pub fn nueva(servicios: Vec<ServicioPurga>, clave: Option<String>) -> Self {
        Self {
            servicios,
            clave,
            cliente: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub fn vacia() -> Self {
        Self::nueva(Vec::new(), None)
    }

    pub fn servicios(&self) -> &[ServicioPurga] {
        &self.servicios
    }

    /// Ejecuta la purga en cada servicio. Un fallo no cancela los demás.
    pub async fn ejecutar(
        &self,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Vec<Value> {
        let mut resultados = Vec::with_capacity(self.servicios.len());
        for servicio in &self.servicios {
            resultados.push(
                self.purgar_servicio(servicio, empresa_id, negocio_ids, usuario_ids)
                    .await,
            );
        }
        resultados
    }

    /// Recolecta la exportación de cada servicio. Un fallo se reporta sin
    /// impedir el resto del archivo.
    pub async fn exportar(
        &self,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Vec<Value> {
        let mut resultados = Vec::with_capacity(self.servicios.len());
        for servicio in &self.servicios {
            resultados.push(
                self.exportar_servicio(servicio, empresa_id, negocio_ids, usuario_ids)
                    .await,
            );
        }
        resultados
    }

    async fn exportar_servicio(
        &self,
        servicio: &ServicioPurga,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Value {
        let url = format!(
            "{}/internal/{}/tenant/exportar",
            servicio.url, servicio.nombre
        );
        let mut peticion = self.cliente.post(&url).json(&json!({
            "empresa_id": empresa_id,
            "negocio_ids": negocio_ids,
            "usuario_ids": usuario_ids,
        }));
        if let Some(clave) = &self.clave {
            peticion = peticion.header("x-internal-key", clave);
        }
        match peticion.send().await {
            Ok(respuesta) if respuesta.status().is_success() => {
                let cuerpo: Value = respuesta.json().await.unwrap_or_else(|_| json!({}));
                json!({
                    "servicio": servicio.nombre,
                    "estado": "completado",
                    "datos": cuerpo.get("datos").cloned().unwrap_or(Value::Null),
                })
            }
            Ok(respuesta) => json!({
                "servicio": servicio.nombre,
                "estado": "fallido",
                "error": format!("http {}", respuesta.status().as_u16()),
            }),
            Err(error) => json!({
                "servicio": servicio.nombre,
                "estado": "fallido",
                "error": error.to_string(),
            }),
        }
    }

    async fn purgar_servicio(
        &self,
        servicio: &ServicioPurga,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Value {
        let url = format!(
            "{}/internal/{}/tenant/purgar",
            servicio.url, servicio.nombre
        );
        let mut peticion = self.cliente.post(&url).json(&json!({
            "empresa_id": empresa_id,
            "negocio_ids": negocio_ids,
            "usuario_ids": usuario_ids,
        }));
        if let Some(clave) = &self.clave {
            peticion = peticion.header("x-internal-key", clave);
        }
        match peticion.send().await {
            Ok(respuesta) if respuesta.status().is_success() => {
                let cuerpo: Value = respuesta.json().await.unwrap_or_else(|_| json!({}));
                json!({
                    "servicio": servicio.nombre,
                    "estado": cuerpo.get("estado").and_then(Value::as_str).unwrap_or("completado"),
                    "detalle": cuerpo.get("detalle").cloned().unwrap_or(Value::Null),
                })
            }
            Ok(respuesta) => json!({
                "servicio": servicio.nombre,
                "estado": "fallido",
                "detalle": { "http": respuesta.status().as_u16() },
            }),
            Err(error) => json!({
                "servicio": servicio.nombre,
                "estado": "fallido",
                "detalle": { "error": error.to_string() },
            }),
        }
    }
}

/// Contrato del endpoint interno que cada servicio implementa.
pub fn validar_purga(dto: &Value) -> Result<(String, Vec<String>, Vec<String>), ApiError> {
    let empresa_id = dto
        .get("empresa_id")
        .and_then(Value::as_str)
        .filter(|valor| uuid::Uuid::parse_str(valor).is_ok())
        .ok_or_else(|| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
    let lista = |clave: &str| {
        dto.get(clave)
            .and_then(Value::as_array)
            .map(|valores| {
                valores
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|valor| uuid::Uuid::parse_str(valor).is_ok())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    Ok((
        empresa_id.to_string(),
        lista("negocio_ids"),
        lista("usuario_ids"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_parsear_servicios() {
        let servicios = ServicioPurga::parsear("storage=http://s:1, orders=http://o:2/ ,,");
        assert_eq!(servicios.len(), 2);
        assert_eq!(servicios[0].url, "http://s:1");
        assert_eq!(servicios[1].nombre, "orders");
        assert_eq!(servicios[1].url, "http://o:2");
    }
}
