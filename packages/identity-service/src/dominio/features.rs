//! Mapa capacidad → features observables por el cliente.
//!
//! El manifiesto (`GET /api/v2/empresas/:id/features`) expone qué puede hacer
//! una empresa combinando sus capacidades activas. Este mapa es la única
//! fuente de verdad de esa traducción: si una capacidad nueva no aparece aquí,
//! el cliente no la verá aunque esté activa.

/// Features que enciende cada capacidad. Los códigos son estables y los usa
/// el cliente para componer pantallas/acciones (no son rutas HTTP).
pub const FEATURES_POR_CAPACIDAD: &[(&str, &[&str])] = &[
    (
        "catalogo_items",
        &[
            "catalogo.items",
            "catalogo.precios",
            "catalogo.atributos",
            "catalogo.imagenes",
            "catalogo.componentes",
            "catalogo.series",
            "catalogo.combos",
            "catalogo.tarifas",
            "catalogo.escalas",
            "catalogo.importar",
        ],
    ),
    (
        "inventario_stock",
        &[
            "inventario.stock",
            "inventario.ubicaciones",
            "inventario.transferencias",
            "inventario.movimientos",
            "inventario.proveedores",
        ],
    ),
    ("inventario_lotes", &["inventario.lotes", "inventario.fefo"]),
    (
        "codigo_barras",
        &["catalogo.codigo_barras", "catalogo.escanear"],
    ),
    (
        "agenda_recursos",
        &[
            "agenda.recursos",
            "agenda.disponibilidad",
            "agenda.citas",
            "agenda.bloqueos",
            "agenda.recordatorios",
        ],
    ),
    ("historia_clinica", &["registros.historia_clinica"]),
    ("recetas", &["registros.recetas"]),
    ("registros", &["registros.genericos"]),
    ("fulfillment_delivery", &["fulfillment.delivery"]),
    ("fulfillment_pickup", &["fulfillment.pickup"]),
    ("fulfillment_onsite", &["fulfillment.onsite"]),
    ("fulfillment_digital", &["fulfillment.digital"]),
    (
        "logistica_rutas",
        &[
            "logistica.rutas",
            "logistica.vehiculos",
            "logistica.paradas",
            "logistica.guias",
        ],
    ),
    (
        "operacion_campo",
        &[
            "campo.personal",
            "campo.asistencia",
            "campo.visitas",
            "campo.incidencias",
            "campo.tracking",
            "campo.sync",
            "campo.clientes",
        ],
    ),
    ("cotizaciones", &["ordenes.cotizaciones"]),
    ("contratos", &["ordenes.contratos", "ordenes.depositos"]),
    ("suscripciones", &["ordenes.suscripciones"]),
    ("mesas", &["ordenes.mesas", "ordenes.comandas"]),
    ("ventas_credito", &["finanzas.credito"]),
    (
        "facturacion",
        &[
            "finanzas.ledger",
            "finanzas.facturacion",
            "finanzas.impuestos",
            "finanzas.comprobantes",
            "finanzas.compras",
            "finanzas.gastos",
            "finanzas.bancos",
            "finanzas.proyecciones",
        ],
    ),
    (
        "liquidaciones",
        &["comisiones.liquidaciones", "comisiones.reportes"],
    ),
    (
        "inteligencia_mercado",
        &[
            "inteligencia.demanda",
            "inteligencia.forecast",
            "inteligencia.anomalias",
            "inteligencia.mapa_calor",
            "inteligencia.scores",
        ],
    ),
];

/// Features de una capacidad; vacío si la capacidad no está mapeada.
pub fn features_de(capacidad: &str) -> &'static [&'static str] {
    FEATURES_POR_CAPACIDAD
        .iter()
        .find(|(codigo, _)| *codigo == capacidad)
        .map(|(_, features)| *features)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_mapear_las_capacidades_conocidas() {
        assert!(features_de("catalogo_items").contains(&"catalogo.items"));
        assert!(features_de("agenda_recursos").contains(&"agenda.citas"));
        assert!(features_de("mesas").contains(&"ordenes.comandas"));
    }

    #[test]
    fn debe_devolver_vacio_para_una_capacidad_desconocida() {
        assert!(features_de("no_existe").is_empty());
    }

    #[test]
    fn no_debe_haber_features_duplicadas_entre_capacidades() {
        let mut vistos = std::collections::HashSet::new();
        for (capacidad, features) in FEATURES_POR_CAPACIDAD {
            for feature in *features {
                assert!(
                    vistos.insert(*feature),
                    "feature duplicada: {feature} (capacidad {capacidad})"
                );
            }
        }
    }
}
