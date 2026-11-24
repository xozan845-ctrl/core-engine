//! Proyección financiera del cap. 8: modelo de cálculo mes a mes (tabla 8.7)
//! con fórmulas cerradas y auditables; punto de equilibrio (8.7), indicadores
//! de rentabilidad VAN/TIR/payback (8.11) y margen de seguridad.
//!
//! Todas las cifras están en centavos. El crate no depende de PostgreSQL para
//! calcular: la persistencia es una capa aparte.

mod calculos;
mod persistencia;

mod prelude {
    pub(crate) use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
    pub(crate) use crate::errors::{error_db, ApiError};
    pub(crate) use crate::money;
    pub(crate) use chrono::{DateTime, Utc};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Value};
    pub(crate) use sqlx::Row;
}
use self::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupuestosProyeccion {
    pub horizonte_meses: i64,
    /// Rampa de vendedores activos (tabla 8.6: 20 al mes 1, +5 netos al mes).
    pub vendedores_iniciales: i64,
    pub entrada_vendedores_mes: i64,
    pub churn_tasa: f64,
    /// Rampa de pedidos por vendedor (tabla 8.6: 25..42, "+1 a +2 al mes").
    pub pedidos_por_vendedor: Vec<f64>,
    pub ticket_promedio_cents: i64,
    pub comision_tasa: f64,
    pub costos_fijos_cents: i64,
    #[serde(default)]
    pub costos_fijos_desde_mes7_cents: Option<i64>,
    pub inversion_inicial_cents: i64,
    #[serde(default)]
    pub tasa_descuento_mensual: Option<f64>,
}

impl SupuestosProyeccion {
    pub fn supuestos_informe() -> Self {
        Self {
            horizonte_meses: 12,
            vendedores_iniciales: 20,
            entrada_vendedores_mes: 5,
            churn_tasa: 0.0,
            pedidos_por_vendedor: vec![
                25.0, 27.0, 29.0, 31.0, 33.0, 35.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0,
            ],
            ticket_promedio_cents: 45_000,
            comision_tasa: 0.12,
            costos_fijos_cents: 200_000,
            costos_fijos_desde_mes7_cents: Some(300_000),
            inversion_inicial_cents: 5_850_000,
            tasa_descuento_mensual: Some(0.015),
        }
    }

    pub fn validar(&self) -> Result<(), ApiError> {
        if !(1..=60).contains(&self.horizonte_meses) {
            return Err(ApiError::validacion(json!([
                {"campo": "horizonte_meses", "restricciones": ["min:1", "max:60"]}
            ])));
        }
        if self.pedidos_por_vendedor.len() as i64 != self.horizonte_meses {
            return Err(ApiError::dominio(
                "RAMPRA_INCOMPLETA",
                format!(
                    "La rampa de pedidos por vendedor debe tener {} valores (tabla 8.6).",
                    self.horizonte_meses
                ),
            ));
        }
        if self.comision_tasa < 0.0 || self.comision_tasa >= 1.0 {
            return Err(ApiError::dominio(
                "TASA_INVALIDA",
                "La tasa de comision debe estar entre 0 y 1.",
            ));
        }
        Ok(())
    }

    pub fn costos_etapa2(&self) -> i64 {
        self.costos_fijos_desde_mes7_cents
            .unwrap_or(self.costos_fijos_cents)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilaProyeccion {
    pub mes: i64,
    pub vendedores: i64,
    pub pedidos: i64,
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
    pub flujo_acumulado_cents: i64,
    pub costo_fijo_por_pedido_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TotalesProyeccion {
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EscenarioEquilibrio {
    pub escenario: String,
    pub costo_variable_por_pedido_cents: i64,
    pub contribucion_por_pedido_cents: i64,
    pub pedidos_equilibrio: i64,
    pub gmv_equilibrio_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MargenSeguridad {
    pub mes: i64,
    pub gmv_cents: i64,
    pub margen_seguridad: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoberturaPorVendedor {
    pub pedidos_por_vendedor_mes: i64,
    pub comision_por_pedido_cents: i64,
    pub comision_bruta_vendedor_cents: i64,
    pub contribucion_media_cents: i64,
    pub vendedores_para_cubrir_costos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EconomiaDeEscala {
    pub costo_fijo_por_pedido_inicio_cents: i64,
    pub costo_fijo_por_pedido_mes6_cents: i64,
    pub costo_fijo_por_pedido_fin_cents: i64,
    pub multiplicador_escala: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndicadoresProyeccion {
    pub payback_mes: Option<i64>,
    pub van_cents: i64,
    pub tir_mensual: Option<f64>,
    pub tir_anual: Option<f64>,
    pub tir_no_estable: bool,
    pub roi: f64,
    pub margen_neto: f64,
    pub punto_equilibrio_caja_cents: i64,
    pub pedidos_equilibrio_caja: i64,
    pub escenarios_equilibrio: Vec<EscenarioEquilibrio>,
    pub margen_seguridad_por_mes: Vec<MargenSeguridad>,
    pub cobertura_por_vendedor: CoberturaPorVendedor,
    pub economia_de_escala: EconomiaDeEscala,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensibilidadComisionGmv {
    pub tasa_comision: f64,
    pub flujo_mes12_cents: Vec<VariacionFlujo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariacionFlujo {
    pub variacion_gmv: f64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensibilidadVan {
    pub tasa_anual: f64,
    pub tasa_mensual: f64,
    pub van_cents: i64,
    pub relacion_van_inversion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaybackPorEscenario {
    pub escenario: String,
    pub descripcion: String,
    pub flujo_acumulado_cents: i64,
    pub payback_mes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumenAnual {
    pub anio: i64,
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
    pub margen_neto: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcumuladoBienal {
    pub gmv_cents: i64,
    pub ingresos_cents: i64,
    pub costos_cents: i64,
    pub flujo_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mes24 {
    pub gmv_cents: i64,
    pub incremento_vs_mes12: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanBienal {
    pub inversion_inicial_cents: i64,
    pub anios: Vec<ResumenAnual>,
    pub filas: Vec<FilaProyeccion>,
    pub acumulado_24_meses: AcumuladoBienal,
    pub roi: f64,
    pub mes_24: Mes24,
    pub indicadores: IndicadoresProyeccion,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Proyeccion {
    pub id: String,
    pub nombre: String,
    pub horizonte_meses: i64,
    pub supuestos: SupuestosProyeccion,
    pub filas: Vec<FilaProyeccion>,
    pub totales: TotalesProyeccion,
    pub inversion_inicial_cents: i64,
    pub indicadores: IndicadoresProyeccion,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProyeccionResumen {
    pub id: String,
    pub nombre: String,
    pub horizonte_meses: i64,
    pub creado_en: String,
}

/// Cuerpo de los estudios de sensibilidad (8.7, 8.10, 8.11, 8.12).
#[derive(Debug, Clone, Serialize)]
pub struct EstudiosSensibilidad {
    pub supuestos: SupuestosProyeccion,
    pub matriz_comision_gmv: Vec<SensibilidadComisionGmv>,
    pub van_por_tasa: Vec<SensibilidadVan>,
    pub payback_por_escenario: Vec<PaybackPorEscenario>,
    pub cobertura_por_vendedor: CoberturaPorVendedor,
}

pub struct Proyecciones {
    db: std::sync::Arc<Db>,
}

impl Proyecciones {
    pub fn nueva(db: std::sync::Arc<Db>) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supuestos_base() -> SupuestosProyeccion {
        SupuestosProyeccion::supuestos_informe()
    }

    #[test]
    fn debe_calcular_la_tabla_del_informe() {
        let (filas, totales) = Proyecciones::calcular(&supuestos_base()).expect("calculo");
        assert_eq!(filas.len(), 12);
        assert_eq!(filas[0].mes, 1);
        assert_eq!(filas[0].vendedores, 20);
        assert_eq!(filas[0].pedidos, 500);
        assert_eq!(filas[0].gmv_cents, 22_500_000);
        assert_eq!(filas[0].ingresos_cents, 2_700_000);
        assert_eq!(filas[0].costos_cents, 200_000);
        assert_eq!(filas[0].flujo_cents, 2_500_000);
        assert_eq!(filas[11].vendedores, 75);
        assert_eq!(filas[11].pedidos, 3_150);
        assert_eq!(filas[11].gmv_cents, 141_750_000);
        assert_eq!(
            totales.gmv_cents,
            filas.iter().map(|f| f.gmv_cents).sum::<i64>()
        );
    }

    #[test]
    fn debe_rechazar_una_rampa_incompleta() {
        let mut supuestos = supuestos_base();
        supuestos.pedidos_por_vendedor.pop();
        let error = Proyecciones::calcular(&supuestos).expect_err("rampa");
        assert_eq!(error.codigo, "RAMPRA_INCOMPLETA");
    }

    #[test]
    fn debe_calcular_equilibrio_y_cobertura() {
        let supuestos = supuestos_base();
        let (filas, _) = Proyecciones::calcular(&supuestos).expect("calculo");
        let indicadores = Proyecciones::indicadores(&filas, &supuestos);
        assert_eq!(indicadores.punto_equilibrio_caja_cents, 2_500_000);
        assert_eq!(indicadores.pedidos_equilibrio_caja, 56);
        assert_eq!(indicadores.escenarios_equilibrio.len(), 3);
        assert_eq!(
            indicadores.cobertura_por_vendedor.comision_por_pedido_cents,
            5_400
        );
        assert_eq!(
            indicadores
                .cobertura_por_vendedor
                .comision_bruta_vendedor_cents,
            226_800
        );
        assert_eq!(
            indicadores
                .cobertura_por_vendedor
                .vendedores_para_cubrir_costos,
            4
        );
        assert_eq!(
            indicadores
                .economia_de_escala
                .costo_fijo_por_pedido_inicio_cents,
            400
        );
        assert!(indicadores.payback_mes.is_some());
    }

    #[test]
    fn debe_generar_sensibilidad_y_paybacks() {
        let supuestos = supuestos_base();
        let matriz = Proyecciones::sensibilidad_comision_gmv(&supuestos).expect("matriz");
        assert_eq!(matriz.len(), 4);
        assert_eq!(matriz[0].flujo_mes12_cents.len(), 3);
        let paybacks = Proyecciones::payback_por_escenario(&supuestos).expect("paybacks");
        assert_eq!(paybacks.len(), 3);
        assert!(paybacks
            .iter()
            .all(|escenario| escenario.payback_mes.is_some()));
    }

    #[test]
    fn debe_construir_el_plan_bienal() {
        let plan = Proyecciones::plan_bienal(&supuestos_base()).expect("plan");
        assert_eq!(plan.filas.len(), 24);
        assert_eq!(plan.anios.len(), 2);
        assert!(plan.roi > 0.0);
        assert!(plan.mes_24.gmv_cents > plan.filas[11].gmv_cents);
        assert!(plan.mes_24.incremento_vs_mes12 > 0.0);
    }
}
