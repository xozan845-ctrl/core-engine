//! Readiness agregada del gateway (R-CD-12).

use crate::config::Config;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Servicio {
    pub nombre: &'static str,
    pub puerto: u16,
}

pub const SERVICIOS_PRODUCCION: [Servicio; 9] = [
    Servicio {
        nombre: "identity",
        puerto: 3001,
    },
    Servicio {
        nombre: "catalog",
        puerto: 3002,
    },
    Servicio {
        nombre: "stores",
        puerto: 3003,
    },
    Servicio {
        nombre: "orders",
        puerto: 3004,
    },
    Servicio {
        nombre: "logistics",
        puerto: 3005,
    },
    Servicio {
        nombre: "scheduling",
        puerto: 3011,
    },
    Servicio {
        nombre: "finance",
        puerto: 3007,
    },
    Servicio {
        nombre: "field",
        puerto: 3008,
    },
    Servicio {
        nombre: "storage",
        puerto: 3010,
    },
];

pub const SERVICIOS_STAGE: [Servicio; 10] = [
    Servicio {
        nombre: "identity",
        puerto: 3001,
    },
    Servicio {
        nombre: "catalog",
        puerto: 3002,
    },
    Servicio {
        nombre: "stores",
        puerto: 3003,
    },
    Servicio {
        nombre: "orders",
        puerto: 3004,
    },
    Servicio {
        nombre: "logistics",
        puerto: 3005,
    },
    Servicio {
        nombre: "scheduling",
        puerto: 3011,
    },
    Servicio {
        nombre: "finance",
        puerto: 3007,
    },
    Servicio {
        nombre: "field",
        puerto: 3008,
    },
    Servicio {
        nombre: "storage",
        puerto: 3010,
    },
    Servicio {
        nombre: "commissions",
        puerto: 3006,
    },
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EstadoReadiness {
    pub status: String,
    pub servicios: BTreeMap<String, String>,
}

pub fn servicios(entorno: &str) -> Vec<Servicio> {
    match entorno.trim().to_lowercase().as_str() {
        "stage" | "staging" => SERVICIOS_STAGE.to_vec(),
        _ => SERVICIOS_PRODUCCION.to_vec(),
    }
}

pub async fn evaluar(config: &Config, timeout_ms: u64) -> EstadoReadiness {
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .unwrap_or_default();
    let lista = servicios(&config.entorno);
    let mut estado = BTreeMap::new();
    for servicio in lista {
        let url = format!(
            "http://{}-service:{}/health",
            servicio.nombre, servicio.puerto
        );
        let valor = match cliente.get(&url).send().await {
            Ok(respuesta) if respuesta.status().is_success() => "ok".to_string(),
            Ok(respuesta) => format!("error:{}", respuesta.status().as_u16()),
            Err(_) => "caido".to_string(),
        };
        estado.insert(servicio.nombre.to_string(), valor);
    }
    let status = if estado.values().all(|valor| valor == "ok") {
        "ok"
    } else {
        "degraded"
    };
    EstadoReadiness {
        status: status.to_string(),
        servicios: estado,
    }
}

pub fn codigo_readiness(estado: &EstadoReadiness) -> u16 {
    if estado.status == "ok" {
        200
    } else {
        503
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_elegir_la_lista_por_entorno() {
        assert_eq!(servicios("stage").len(), 10);
        assert_eq!(servicios("production").len(), 9);
        assert_eq!(servicios("").len(), 9);
    }

    #[test]
    fn debe_mapear_estado_a_http() {
        let ok = EstadoReadiness {
            status: "ok".into(),
            servicios: BTreeMap::new(),
        };
        assert_eq!(codigo_readiness(&ok), 200);
        let malo = EstadoReadiness {
            status: "degraded".into(),
            servicios: BTreeMap::new(),
        };
        assert_eq!(codigo_readiness(&malo), 503);
    }
}
