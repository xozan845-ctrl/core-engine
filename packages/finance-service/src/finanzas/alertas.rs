//! Jerarquía del tablero de métricas con reglas de alerta (tabla 8.10):
//! cada capa tiene su frecuencia, su umbral y una acción concreta.

use crate::finanzas::kpis::KpisFinancieros;
use crate::finanzas::proyecciones::Proyecciones;
use chrono::{Datelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableroAlerta {
    pub capa: String,
    pub frecuencia: String,
    pub indicador: String,
    pub estado: String,
    pub umbral: String,
    pub valor: Option<f64>,
    pub accion: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle: Option<String>,
    pub fecha_esperada: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tablero {
    pub periodo: String,
    pub kpis: KpisFinancieros,
    pub alertas: Vec<TableroAlerta>,
}

#[derive(Debug, Clone, Default)]
pub struct OpcionesTablero {
    pub mes: Option<String>,
    pub costo_entrega_por_pedido_cents: Option<i64>,
}

pub struct Alertas {
    kpis: Arc<crate::finanzas::kpis::Kpis>,
    proyecciones: Arc<Proyecciones>,
    db: Arc<crate::db::Db>,
}

impl Alertas {
    pub fn nueva(
        kpis: Arc<crate::finanzas::kpis::Kpis>,
        proyecciones: Arc<Proyecciones>,
        db: Arc<crate::db::Db>,
    ) -> Self {
        Self {
            kpis,
            proyecciones,
            db,
        }
    }

    /// Capas que deben evaluarse hoy según el ritual de revisión (doc 8.8).
    pub fn capas_debidas(hoy: chrono::DateTime<Utc>) -> Vec<String> {
        let dia = hoy.day();
        let es_lunes = hoy.weekday() == Weekday::Mon;
        let mut debidas = Vec::new();
        if es_lunes {
            debidas.push("operacion".to_string());
        }
        if dia == 1 || dia == 15 {
            debidas.push("adquisicion".to_string());
            debidas.push("caja".to_string());
        }
        if dia == 1 {
            debidas.push("north_star".to_string());
        }
        debidas
    }

    /// Tablero completo: KPIs + reglas de alerta de todas las capas.
    pub async fn tablero(
        &self,
        opciones: OpcionesTablero,
    ) -> Result<Tablero, crate::errors::ApiError> {
        let kpis = self.kpis.kpis(opciones.mes.as_deref()).await?;
        let debidas = Self::capas_debidas(Utc::now());
        let proyeccion = self.ultima_proyeccion().await?;

        let north_star = match &proyeccion {
            None => (
                None,
                "SIN_DATO".to_string(),
                "No hay proyeccion guardada para comparar.".to_string(),
            ),
            Some(datos) => match datos.desvio_gmv {
                None => (
                    None,
                    "SIN_DATO".to_string(),
                    "No hay GMV real del periodo para comparar.".to_string(),
                ),
                Some(desvio) => (
                    Some(desvio),
                    if desvio > 0.2 { "ALERTA" } else { "OK" }.to_string(),
                    format!(
                        "GMV real {} vs proyectado {} del periodo.",
                        kpis.gmv, datos.gmv_proyectado
                    ),
                ),
            },
        };
        let activacion = kpis.activacion_primeros_15_dias;
        let devoluciones = kpis.tasa_devoluciones;
        let costo_entrega = opciones.costo_entrega_por_pedido_cents;
        let liquidacion = kpis.liquidacion_a_tiempo;

        let alertas = vec![
            Self::regla(
                "north_star",
                "MENSUAL",
                "GMV vs proyeccion (8.6)",
                debidas.iter().any(|capa| capa == "north_star"),
                north_star.0,
                "desvio > 20 %",
                &north_star.1,
                "Revisar supuestos (8.12).",
                Some(north_star.2),
            ),
            Self::regla(
                "adquisicion",
                "QUINCENAL",
                "Activacion de vendedores (15 dias)",
                debidas.iter().any(|capa| capa == "adquisicion"),
                activacion,
                "< 50 %",
                match activacion {
                    None => "SIN_DATO",
                    Some(valor) if valor < 0.5 => "ALERTA",
                    Some(_) => "OK",
                },
                "Onboarding asistido en 7 dias.",
                None,
            ),
            Self::regla(
                "operacion",
                "SEMANAL",
                "Tasa de devoluciones",
                debidas.iter().any(|capa| capa == "operacion"),
                Some(devoluciones),
                "> 5 %",
                if devoluciones > 0.05 { "ALERTA" } else { "OK" },
                "Contingencia (cap. 5.2).",
                None,
            ),
            Self::regla(
                "operacion",
                "SEMANAL",
                "Costo last-mile por pedido",
                debidas.iter().any(|capa| capa == "operacion"),
                costo_entrega.map(|valor| valor as f64 / 100.0),
                "> C$ 45",
                match costo_entrega {
                    None => "SIN_DATO",
                    Some(valor) if valor > 4_500 => "ALERTA",
                    Some(_) => "OK",
                },
                "Tarifa de entrega al comprador o reduccion de zona (8.12).",
                Some(
                    "Se evalua cuando el piloto provee el costo real del courier (8.12)."
                        .to_string(),
                ),
            ),
            Self::regla(
                "caja",
                "QUINCENAL",
                "Liquidacion a tiempo (RN-07)",
                debidas.iter().any(|capa| capa == "caja"),
                liquidacion,
                "100 %",
                match liquidacion {
                    None => "SIN_DATO",
                    Some(valor) if valor < 1.0 => "ALERTA",
                    Some(_) => "OK",
                },
                "Alerta inmediata: cualquier retraso en liquidacion.",
                liquidacion.map(|valor| {
                    format!("Cortes pagados a tiempo: {} %.", (valor * 100.0).round())
                }),
            ),
        ];

        Ok(Tablero {
            periodo: kpis.periodo.clone(),
            kpis,
            alertas,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn regla(
        capa: &str,
        frecuencia: &str,
        indicador: &str,
        evaluable: bool,
        valor: Option<f64>,
        umbral: &str,
        regla: &str,
        accion: &str,
        detalle: Option<String>,
    ) -> TableroAlerta {
        let _ = evaluable;
        TableroAlerta {
            capa: capa.to_string(),
            frecuencia: frecuencia.to_string(),
            indicador: indicador.to_string(),
            estado: match regla {
                "ALERTA" => "ALERTA",
                "SIN_DATO" => "SIN_DATO",
                _ => "OK",
            }
            .to_string(),
            umbral: umbral.to_string(),
            valor,
            accion: accion.to_string(),
            detalle,
            fecha_esperada: frecuencia.to_string(),
        }
    }

    /// Desvío del GMV real frente a la última proyección guardada (8.6).
    async fn ultima_proyeccion(&self) -> Result<Option<ProyeccionDesvio>, crate::errors::ApiError> {
        let listado = self.proyecciones.listar().await?;
        let Some(primera) = listado.first() else {
            return Ok(None);
        };
        let Some(proyeccion) = self.proyecciones.obtener(&primera.id).await? else {
            return Ok(None);
        };
        let periodo = Utc::now().format("%Y-%m").to_string();
        let mes_numero: i64 = periodo
            .get(5..7)
            .and_then(|valor| valor.parse().ok())
            .unwrap_or(0);
        let Some(fila) = proyeccion.filas.iter().find(|fila| fila.mes == mes_numero) else {
            return Ok(None);
        };
        let gmv_real = self.proyecciones.leer_gmv_real(&self.db, &periodo).await?;
        let Some(gmv_real) = gmv_real else {
            return Ok(Some(ProyeccionDesvio {
                gmv_proyectado: String::new(),
                desvio_gmv: None,
            }));
        };
        if fila.gmv_cents == 0 {
            return Ok(Some(ProyeccionDesvio {
                gmv_proyectado: format!("{} C$", fila.gmv_cents / 100),
                desvio_gmv: None,
            }));
        }
        let desvio = (gmv_real - fila.gmv_cents) as f64 / fila.gmv_cents as f64;
        Ok(Some(ProyeccionDesvio {
            gmv_proyectado: format!("{}.{:02} C$", fila.gmv_cents / 100, fila.gmv_cents % 100),
            desvio_gmv: Some((desvio * 10_000.0).round() / 10_000.0),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct ProyeccionDesvio {
    pub gmv_proyectado: String,
    pub desvio_gmv: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn debe_marcar_las_capas_del_ritual() {
        let dia1 = Utc.with_ymd_and_hms(2026, 11, 1, 9, 0, 0).unwrap();
        let capas = Alertas::capas_debidas(dia1);
        assert!(capas.contains(&"north_star".to_string()));
        assert!(capas.contains(&"adquisicion".to_string()));
        assert!(capas.contains(&"caja".to_string()));
        assert!(!capas.contains(&"operacion".to_string()));
    }

    #[test]
    fn debe_marcar_solo_operacion_un_lunes_cualquiera() {
        let lunes = Utc.with_ymd_and_hms(2026, 11, 9, 9, 0, 0).unwrap();
        assert_eq!(Alertas::capas_debidas(lunes), vec!["operacion"]);
    }

    #[test]
    fn debe_no_marcar_nada_un_dia_normal() {
        let miercoles = Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap();
        assert!(Alertas::capas_debidas(miercoles).is_empty());
    }

    #[test]
    fn debe_mapear_estados_de_alerta() {
        let alerta = Alertas::regla(
            "caja",
            "QUINCENAL",
            "x",
            true,
            Some(0.5),
            "100 %",
            "ALERTA",
            "accion",
            None,
        );
        assert_eq!(alerta.estado, "ALERTA");
        assert_eq!(alerta.fecha_esperada, "QUINCENAL");
        let ok = Alertas::regla("x", "y", "z", true, None, "u", "OK", "a", None);
        assert_eq!(ok.estado, "OK");
        let sin = Alertas::regla("x", "y", "z", true, None, "u", "SIN_DATO", "a", None);
        assert_eq!(sin.estado, "SIN_DATO");
    }
}
