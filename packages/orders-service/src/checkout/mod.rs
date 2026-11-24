//! `CreateOrderCommandHandler`: valida las reglas (RN-01/03/04/05), persiste el
//! evento y el outbox en la misma transacción y devuelve la vista de la orden.
//! Además aplica transiciones (logística → orders) y replay.

mod calculos;
mod comunes;
mod ejecutar;
mod transiciones;

mod prelude {
    pub(crate) use crate::carrito::{Carrito, ItemCarrito};
    pub(crate) use crate::clients::{
        ClientesPort, ConfiguracionExterna, ConfiguracionFiscal, ConfiguracionOperativa,
        CreditoEstado, CuponExterno, OfertaExterna, PreciosExternos, PromocionLinea,
    };
    pub(crate) use crate::db::{iso_ms, Db};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use crate::event_store::{error_db, parsear_uuid, OrderEventStore};
    pub(crate) use crate::fidelizacion::Fidelizacion;
    pub(crate) use crate::fulfillments::Fulfillments;
    pub(crate) use crate::money;
    pub(crate) use crate::order_state::ItemOrden;
    pub(crate) use crate::outbox::Outbox;
    pub(crate) use crate::pagos::Pagos;
    pub(crate) use crate::pedidos::Pedidos;
    pub(crate) use crate::tipos_orden::TiposOrden;
    pub(crate) use crate::views::{OrderView, OrderViewRepository};
    pub(crate) use chrono::{Duration, Utc};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Map, Value};
    pub(crate) use std::collections::{HashMap, HashSet};
    pub(crate) use std::sync::Arc;
    pub(crate) use uuid::Uuid;
}
use self::prelude::*;

pub const ORDER_CREATED: &str = "order.created";
pub const ORDER_STATUS_UPDATED: &str = "order.status.updated";
pub const ORDER_CANCELLED: &str = "order.cancelled";
pub const PAYMENT_PROCESADO: &str = "payment.procesado";
pub const DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineaEntrada {
    pub oferta_id: String,
    pub cantidad: i64,
    #[serde(default)]
    pub fulfillment: Option<String>,
    #[serde(default)]
    pub fulfillment_config: Option<Value>,
    #[serde(default)]
    pub ubicacion_id: Option<String>,
    /// Código de tasa regional; si falta, se usa `atributos.impuesto_codigo`
    /// del ítem o la tasa predeterminada de la región.
    #[serde(default)]
    pub impuesto_codigo: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ComandoCrear {
    #[serde(default)]
    pub cliente_parte_id: Option<String>,
    #[serde(default)]
    pub tipo_orden: String,
    #[serde(default)]
    pub items: Option<Vec<LineaEntrada>>,
    #[serde(default)]
    pub usar_carrito: bool,
    #[serde(default)]
    pub metodo_pago: Option<String>,
    #[serde(default)]
    pub moneda_pago: Option<String>,
    #[serde(default)]
    pub lista_precio_codigo: Option<String>,
    #[serde(default)]
    pub segmento_codigo: Option<String>,
    #[serde(default)]
    pub cupon_codigo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LineaAgrupada {
    oferta_id: String,
    cantidad: i64,
    fulfillment: Option<String>,
    fulfillment_config: Option<Value>,
    ubicacion_id: Option<String>,
    impuesto_codigo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PrecioResuelto {
    origen: String,
    precio_unitario_cents: i64,
    promocion: Option<PromocionLinea>,
}

#[derive(Debug, Clone, PartialEq)]
struct EntregaPlan {
    empresa_id: String,
    item_id: String,
    sku: String,
    comprador_id: String,
    cliente_parte_id: Option<String>,
    recurso_url: String,
    token: String,
    max_descargas: Option<i64>,
    expira_en: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EstadoDerivado {
    pub order_id: String,
    pub comprador_id: String,
    pub items: Vec<ItemOrden>,
    pub total_cents: i64,
    pub estado: String,
    pub motivo: Option<String>,
    pub creado_en: String,
    pub actualizado_en: String,
    pub tipo_orden: String,
}

pub struct Checkout {
    pub db: Arc<Db>,
    pub clientes: Arc<dyn ClientesPort>,
    pub carrito: Arc<Carrito>,
    pub vistas: Arc<OrderViewRepository>,
    pub pedidos: Arc<Pedidos>,
    pub outbox: Outbox,
    pub pagos: Arc<Pagos>,
    pub tipos: Arc<TiposOrden>,
    pub fulfillments: Arc<Fulfillments>,
    pub tasa_comision: f64,
}

#[cfg(test)]
mod tests {
    use super::comunes::*;
    use super::*;
    use serde_json::json;

    fn promocion(tipo: &str) -> PromocionLinea {
        PromocionLinea {
            id: "p-1".into(),
            nombre: "Promo".into(),
            tipo: tipo.into(),
            cantidad_requerida: None,
            cantidad_gratis: None,
            descuento_porcentaje: None,
        }
    }

    #[test]
    fn debe_calcular_el_descuento_porcentual_y_el_2x1() {
        let mut porcentaje = promocion("descuento_porcentaje");
        porcentaje.descuento_porcentaje = Some(25);
        assert_eq!(
            descuento_de_promocion(Some(&porcentaje), 10_000, 2).expect("porcentaje"),
            5_000
        );

        let mut dos_por_uno = promocion("2x1");
        dos_por_uno.cantidad_requerida = Some(2);
        assert_eq!(
            descuento_de_promocion(Some(&dos_por_uno), 10_000, 4).expect("2x1"),
            20_000
        );
        assert_eq!(
            descuento_de_promocion(Some(&dos_por_uno), 10_000, 1).expect("sin 2x1"),
            0
        );
    }

    #[test]
    fn debe_tope_el_descuento_al_bruto() {
        let mut regalo = promocion("regalo");
        regalo.cantidad_requerida = Some(1);
        regalo.cantidad_gratis = Some(5);
        let descuento = descuento_de_promocion(Some(&regalo), 10_000, 2).expect("regalo");
        assert_eq!(descuento, 20_000);
    }

    #[test]
    fn debe_agrupar_lineas_de_la_misma_oferta() {
        let lineas = vec![
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
                impuesto_codigo: None,
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 2,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
                impuesto_codigo: None,
            },
        ];
        let agrupadas = agrupar(&lineas).expect("agrupar");
        assert_eq!(agrupadas.len(), 1);
        assert_eq!(agrupadas[0].cantidad, 3);
    }

    #[test]
    fn debe_rechazar_fulfillments_distintos_para_la_misma_oferta() {
        let lineas = vec![
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
                impuesto_codigo: None,
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("digital".into()),
                fulfillment_config: None,
                ubicacion_id: None,
                impuesto_codigo: None,
            },
        ];
        assert_eq!(
            agrupar(&lineas).expect_err("conflicto").codigo,
            "FULFILLMENT_CONFLICTO"
        );
    }

    #[test]
    fn debe_reconstruir_el_estado_desde_los_eventos() {
        let eventos = vec![
            crate::event_store::EventoDeOrden {
                event_id: "e-1".into(),
                aggregate_id: "o-1".into(),
                aggregate_type: "Order".into(),
                tipo: "OrderCreatedEvent".into(),
                payload: json!({
                    "order_id": "o-1",
                    "comprador_id": "c-1",
                    "items": [],
                    "total_cents": 100,
                    "estado": "creada",
                    "tipo_orden": "compra",
                    "creado_en": "2026-01-01T00:00:00.000Z"
                }),
                metadata: json!({}),
                version: 1,
                creado_en: "2026-01-01T00:00:00.000Z".into(),
            },
            crate::event_store::EventoDeOrden {
                event_id: "e-2".into(),
                aggregate_id: "o-1".into(),
                aggregate_type: "Order".into(),
                tipo: "OrderStatusUpdatedEvent".into(),
                payload: json!({"estado": "pagada", "motivo": "ok"}),
                metadata: json!({}),
                version: 2,
                creado_en: "2026-01-01T00:01:00.000Z".into(),
            },
        ];
        let estado = reconstruir_desde_eventos(&eventos).expect("estado");
        assert_eq!(estado.estado, "pagada");
        assert_eq!(estado.motivo.as_deref(), Some("ok"));
        assert_eq!(estado.total_cents, 100);
    }

    #[test]
    fn debe_ignorar_una_historia_sin_order_created() {
        let eventos = vec![crate::event_store::EventoDeOrden {
            event_id: "e-1".into(),
            aggregate_id: "o-1".into(),
            aggregate_type: "Order".into(),
            tipo: "OrderStatusUpdatedEvent".into(),
            payload: json!({}),
            metadata: json!({}),
            version: 1,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
        }];
        assert!(reconstruir_desde_eventos(&eventos).is_none());
    }

    #[test]
    fn debe_permitir_fulfillment_sin_restriccion() {
        let config = crate::clients::ConfiguracionOperativa::default();
        assert!(fulfillment_permitido(&config, Some("delivery")));
        assert!(fulfillment_permitido(&config, None));
    }

    #[test]
    fn debe_respetar_los_fulfillment_del_modelo() {
        let config = crate::clients::ConfiguracionOperativa {
            fulfillment: vec!["delivery".to_string(), "pickup".to_string()],
            ..Default::default()
        };
        assert!(fulfillment_permitido(&config, Some("delivery")));
        assert!(fulfillment_permitido(&config, Some("pickup")));
        assert!(fulfillment_permitido(&config, None));
        assert!(!fulfillment_permitido(&config, Some("onsite")));
    }

    fn permitidos() -> Vec<String> {
        ["delivery", "pickup", "onsite", "digital", "none"]
            .iter()
            .map(|valor| (*valor).to_string())
            .collect()
    }

    #[test]
    fn debe_validar_las_reglas_de_fulfillment() {
        assert_eq!(
            validar_fulfillment(None, "compra", "producto", &permitidos()).expect("none"),
            "none"
        );
        assert_eq!(
            validar_fulfillment(Some("onsite"), "reserva", "servicio", &permitidos())
                .expect("onsite"),
            "onsite"
        );
        assert!(
            validar_fulfillment(Some("delivery"), "reserva", "servicio", &permitidos()).is_err()
        );
        assert!(validar_fulfillment(Some("onsite"), "reserva", "producto", &permitidos()).is_err());
        assert!(validar_fulfillment(Some("otro"), "compra", "producto", &permitidos()).is_err());
    }

    fn item(fulfillment: Option<&str>, ubicacion_id: Option<&str>) -> ItemOrden {
        ItemOrden {
            oferta_id: "o-1".into(),
            sku: "SKU-1".into(),
            producto_nombre: "Producto".into(),
            cantidad: 2,
            precio_unitario_cents: 10_000,
            descuento_cents: 0,
            impuesto_cents: 0,
            tasa_impuesto_bps: 0,
            impuesto_codigo: None,
            total_linea_cents: 0,
            promocion_id: None,
            vendedor_id: None,
            tienda_id: "11111111-1111-1111-1111-111111111111".into(),
            empresa_id: "22222222-2222-2222-2222-222222222222".into(),
            negocio_id: None,
            participacion_porcentaje: None,
            comision: true,
            ubicacion_id: ubicacion_id.map(str::to_string),
            item_type: "producto".into(),
            item_id: "33333333-3333-3333-3333-333333333333".into(),
            fulfillment: fulfillment.map(str::to_string),
            fulfillment_config: None,
        }
    }

    #[test]
    fn debe_liberar_solo_las_lineas_que_comprometieron_stock() {
        let items = vec![
            item(
                Some("delivery"),
                Some("44444444-4444-4444-4444-444444444444"),
            ),
            item(Some("digital"), None),
        ];
        let liberables = items_para_liberacion(&items);
        assert_eq!(liberables.len(), 1);
        assert_eq!(liberables[0]["sku"], "SKU-1");
        assert_eq!(
            liberables[0]["ubicacion_id"],
            "44444444-4444-4444-4444-444444444444"
        );
    }

    #[test]
    fn debe_distinguir_una_orden_pagada_de_una_impaga() {
        for estado in ["pagada", "en_preparacion", "enviada", "entregada"] {
            assert!(estado_pagado(estado), "{estado}");
        }
        for estado in ["creada", "cancelada", "devuelta", "aceptada"] {
            assert!(!estado_pagado(estado), "{estado}");
        }
    }
}
