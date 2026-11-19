//! Consulta de pedidos con control de acceso por dominio (Tabla 15).
//!
//! Réplica de `pedidos.service.ts`: el comprador solo ve sus compras, el
//! vendedor y el empresario ven solo sus líneas, y la respuesta se recorta
//! también en el timeline (los eventos contienen el snapshot completo).

use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::order_state::ItemOrden;
use crate::views::{OrderView, TimelineEvento};
use serde_json::Value;

pub struct Pedidos {
    pub tasa_comision: f64,
}

impl Pedidos {
    pub fn detalle_para(
        &self,
        orden: OrderView,
        usuario: &UsuarioContexto,
    ) -> Result<OrderView, ApiError> {
        if usuario.es_admin_global() {
            return Ok(orden);
        }
        if usuario.rol == "comprador" && orden.comprador_id == usuario.user_id {
            return Ok(orden);
        }
        if usuario.rol == "vendedor" {
            let items: Vec<ItemOrden> = orden
                .items
                .iter()
                .filter(|i| i.vendedor_id.as_deref() == Some(usuario.user_id.as_str()))
                .cloned()
                .collect();
            if !items.is_empty() {
                return self.recortar(orden, items);
            }
        }
        if usuario.rol == "empresario" {
            if let Some(tenant) = usuario.tenant_id.as_deref() {
                let negocio = usuario.negocio_id.as_deref();
                let items: Vec<ItemOrden> = orden
                    .items
                    .iter()
                    .filter(|i| {
                        i.empresa_id == tenant
                            && negocio.is_none_or(|n| i.negocio_id.as_deref() == Some(n))
                    })
                    .cloned()
                    .collect();
                if !items.is_empty() {
                    return self.recortar(orden, items);
                }
            }
        }
        if usuario.rol != "vendedor" && usuario.rol != "comprador" {
            if let (Some(tenant), Some(negocio)) =
                (usuario.tenant_id.as_deref(), usuario.negocio_id.as_deref())
            {
                let items: Vec<ItemOrden> = orden
                    .items
                    .iter()
                    .filter(|i| i.empresa_id == tenant && i.negocio_id.as_deref() == Some(negocio))
                    .cloned()
                    .collect();
                if !items.is_empty() {
                    return self.recortar(orden, items);
                }
            }
        }
        Err(ApiError::acceso_denegado("No tiene acceso a esta orden."))
    }

    pub fn para_empresa(
        &self,
        orden: OrderView,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<OrderView, ApiError> {
        let items = orden
            .items
            .iter()
            .filter(|i| {
                i.empresa_id == empresa_id
                    && negocio_id.is_none_or(|n| i.negocio_id.as_deref() == Some(n))
            })
            .cloned()
            .collect();
        self.recortar(orden, items)
    }

    pub fn para_vendedor(
        &self,
        orden: OrderView,
        vendedor_id: &str,
    ) -> Result<OrderView, ApiError> {
        let items = orden
            .items
            .iter()
            .filter(|i| i.vendedor_id.as_deref() == Some(vendedor_id))
            .cloned()
            .collect();
        self.recortar(orden, items)
    }

    /// Oculta del timeline las líneas que no pertenecen al actor.
    pub fn timeline_visible(
        &self,
        eventos: Vec<TimelineEvento>,
        items_visibles: &[ItemOrden],
    ) -> Vec<TimelineEvento> {
        let ofertas: std::collections::HashSet<&str> = items_visibles
            .iter()
            .map(|i| i.oferta_id.as_str())
            .collect();
        eventos
            .into_iter()
            .map(|evento| {
                let Some(objeto) = evento.payload.as_object() else {
                    return evento;
                };
                let Some(items) = objeto.get("items").and_then(Value::as_array) else {
                    return evento;
                };
                let filtrados: Vec<Value> = items
                    .iter()
                    .filter(|item| {
                        item.get("oferta_id")
                            .and_then(Value::as_str)
                            .is_some_and(|oferta| ofertas.contains(oferta))
                    })
                    .cloned()
                    .collect();
                let total = self.total_de_json(&filtrados);
                let mut payload = objeto.clone();
                payload.insert("items".to_string(), Value::Array(filtrados));
                payload.insert("total_cents".to_string(), Value::from(total));
                TimelineEvento {
                    payload: Value::Object(payload),
                    ..evento
                }
            })
            .collect()
    }

    fn recortar(&self, orden: OrderView, items: Vec<ItemOrden>) -> Result<OrderView, ApiError> {
        if items.is_empty() {
            return Err(ApiError::acceso_denegado("No tiene acceso a esta orden."));
        }
        let total_bruto = self.total_de(&items);
        let bruto_orden = orden
            .total_cents
            .saturating_add(orden.descuento_cupon_cents)
            .max(1);
        let cupon_visible = if orden.descuento_cupon_cents > 0 {
            ((orden.descuento_cupon_cents as f64 * total_bruto as f64) / bruto_orden as f64).round()
                as i64
        } else {
            0
        };
        let con_totales = items.iter().any(|item| item.total_linea_cents > 0);
        let total = if con_totales {
            self.total_de(&items)
        } else {
            total_bruto.saturating_sub(cupon_visible)
        };
        let impuesto = items.iter().map(|item| item.impuesto_cents).sum();
        let comision = if orden.total_cents <= 0 {
            0
        } else {
            ((orden.comision_cents as f64 * total as f64) / orden.total_cents as f64).round() as i64
        };
        Ok(OrderView {
            vendedor_id: items[0].vendedor_id.clone(),
            tienda_id: items[0].tienda_id.clone(),
            total_cents: total,
            impuesto_cents: impuesto,
            comision_cents: comision,
            descuento_cupon_cents: cupon_visible,
            items,
            ..orden
        })
    }

    fn total_de(&self, items: &[ItemOrden]) -> i64 {
        items
            .iter()
            .map(|item| {
                if item.total_linea_cents > 0 {
                    item.total_linea_cents
                } else {
                    item.precio_unitario_cents
                        .saturating_mul(item.cantidad)
                        .saturating_sub(item.descuento_cents)
                }
            })
            .sum()
    }

    fn total_de_json(&self, items: &[Value]) -> i64 {
        items
            .iter()
            .map(|item| {
                if let Some(total) = item.get("total_linea_cents").and_then(Value::as_i64) {
                    if total > 0 {
                        return total;
                    }
                }
                item.get("precio_unitario_cents")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    * item.get("cantidad").and_then(Value::as_i64).unwrap_or(0)
                    - item
                        .get("descuento_cents")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::OrderView;
    use serde_json::json;

    fn item(oferta: &str, vendedor: &str, empresa: &str, precio: i64, cantidad: i64) -> ItemOrden {
        ItemOrden {
            oferta_id: oferta.into(),
            sku: "SKU".into(),
            producto_nombre: "Producto".into(),
            cantidad,
            precio_unitario_cents: precio,
            descuento_cents: 0,
            impuesto_cents: 0,
            tasa_impuesto_bps: 0,
            total_linea_cents: 0,
            promocion_id: None,
            vendedor_id: Some(vendedor.into()),
            tienda_id: "t-1".into(),
            empresa_id: empresa.into(),
            negocio_id: None,
            participacion_porcentaje: Some(10),
            comision: true,
            ubicacion_id: None,
            item_type: "producto".into(),
            item_id: "i-1".into(),
            fulfillment: None,
            fulfillment_config: None,
        }
    }

    fn orden() -> OrderView {
        OrderView {
            id: "o-1".into(),
            vendedor_id: Some("v-1".into()),
            comprador_id: "c-1".into(),
            parte_id: None,
            tienda_id: "t-1".into(),
            negocio_id: None,
            estado: "pagada".into(),
            total_cents: 300,
            impuesto_cents: 0,
            comision_cents: 36,
            descuento_cupon_cents: 0,
            cupon_codigo: None,
            moneda: "NIO".into(),
            items: vec![
                item("of-1", "v-1", "e-1", 100, 1),
                item("of-2", "v-2", "e-2", 200, 1),
            ],
            direccion_envio: Value::Null,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
            actualizado_en: "2026-01-01T00:00:00.000Z".into(),
            tipo_orden: "compra".into(),
            metodo_pago: "simulado".into(),
            fulfillment_config: json!({}),
            pagada_en: None,
            enviada_en: None,
            entregada_en: None,
            cancelada_en: None,
            devuelta_en: None,
        }
    }

    #[test]
    fn debe_recortar_la_orden_del_vendedor() {
        let pedidos = Pedidos {
            tasa_comision: 0.12,
        };
        let usuario = UsuarioContexto {
            user_id: "v-1".into(),
            rol: "vendedor".into(),
            ..Default::default()
        };
        let recortada = pedidos.detalle_para(orden(), &usuario).expect("acceso");
        assert_eq!(recortada.items.len(), 1);
        assert_eq!(recortada.total_cents, 100);
        assert_eq!(recortada.comision_cents, 12);
    }

    #[test]
    fn debe_repartir_el_cupon_al_recortar_la_orden() {
        let pedidos = Pedidos {
            tasa_comision: 0.12,
        };
        let mut con_cupon = orden();
        con_cupon.descuento_cupon_cents = 30;
        con_cupon.cupon_codigo = Some("VERANO".into());
        con_cupon.total_cents = 270;
        let usuario = UsuarioContexto {
            user_id: "v-1".into(),
            rol: "vendedor".into(),
            ..Default::default()
        };
        let recortada = pedidos.detalle_para(con_cupon, &usuario).expect("acceso");
        assert_eq!(recortada.descuento_cupon_cents, 10);
        assert_eq!(recortada.total_cents, 90);
        assert_eq!(recortada.cupon_codigo.as_deref(), Some("VERANO"));
    }

    #[test]
    fn debe_negar_una_orden_ajena_al_comprador() {
        let pedidos = Pedidos {
            tasa_comision: 0.12,
        };
        let usuario = UsuarioContexto {
            user_id: "c-2".into(),
            rol: "comprador".into(),
            ..Default::default()
        };
        assert_eq!(
            pedidos
                .detalle_para(orden(), &usuario)
                .expect_err("403")
                .codigo,
            "ACCESO_DENEGADO"
        );
    }

    #[test]
    fn debe_filtrar_el_timeline_por_ofertas_visibles() {
        let pedidos = Pedidos {
            tasa_comision: 0.12,
        };
        let eventos = vec![TimelineEvento {
            id: 1,
            tipo: "OrderCreatedEvent".into(),
            payload: json!({
                "items": [
                    {"oferta_id": "of-1", "precio_unitario_cents": 100, "cantidad": 1},
                    {"oferta_id": "of-2", "precio_unitario_cents": 200, "cantidad": 1}
                ],
                "total_cents": 300
            }),
            version: 1,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
        }];
        let visibles = pedidos.timeline_visible(eventos, &[item("of-1", "v-1", "e-1", 100, 1)]);
        assert_eq!(
            visibles[0].payload["items"]
                .as_array()
                .expect("items")
                .len(),
            1
        );
        assert_eq!(visibles[0].payload["total_cents"], 100);
    }
}
