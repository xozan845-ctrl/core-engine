//! Carrito del comprador (RN-05): no reserva stock, expira a los 30 min y se
//! enriquece en vivo contra `stores-service` (RN-01).

use crate::clients::{ClientesPort, OfertaExterna};
use crate::config::CARRITO_EXPIRACION_MS;
use crate::db::{iso_ms, Db};
use crate::errors::ApiError;
use crate::event_store::error_db;
use crate::money;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};

pub const CANTIDAD_MAXIMA_POR_LINEA: i64 = 99;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemCarrito {
    pub oferta_id: String,
    pub cantidad: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ItemCarritoEnriquecido {
    pub oferta_id: String,
    pub cantidad: i64,
    pub tienda_id: String,
    pub vendedor_id: String,
    pub sku: String,
    pub producto_nombre: String,
    pub precio_venta: String,
    pub stock: i64,
    pub estado: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CarritoVista {
    pub items: Vec<ItemCarritoEnriquecido>,
    pub total: String,
    pub expira_en: Option<String>,
}

pub struct Carrito {
    db: std::sync::Arc<Db>,
    clientes: std::sync::Arc<dyn ClientesPort>,
}

impl Carrito {
    pub fn nueva(db: std::sync::Arc<Db>, clientes: std::sync::Arc<dyn ClientesPort>) -> Self {
        Self { db, clientes }
    }

    async fn purgar_expirados(&self) -> Result<(), ApiError> {
        sqlx::query(
            "DELETE FROM orders.carritos
             WHERE actualizado_en < NOW() - ($1::int * INTERVAL '1 millisecond')",
        )
        .bind(CARRITO_EXPIRACION_MS)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }

    pub async fn obtener(&self, comprador_id: &str) -> Result<CarritoVista, ApiError> {
        self.purgar_expirados().await?;
        let fila = sqlx::query(
            "SELECT comprador_id, items_json, total_cents, actualizado_en
             FROM orders.carritos WHERE comprador_id = $1",
        )
        .bind(crate::event_store::parsear_uuid(comprador_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        match fila {
            None => Ok(vista_vacia()),
            Some(fila) => self.enriquecer(&fila).await,
        }
    }

    pub async fn agregar(
        &self,
        comprador_id: &str,
        oferta_id: &str,
        cantidad: i64,
    ) -> Result<CarritoVista, ApiError> {
        self.purgar_expirados().await?;
        let oferta = self
            .clientes
            .ofertas(&[oferta_id.to_string()])
            .await
            .into_iter()
            .next();
        let Some(oferta) = oferta else {
            return Err(ApiError::no_encontrado("Oferta", oferta_id));
        };
        if oferta.estado != "activa" {
            return Err(ApiError::dominio(
                "OFERTA_NO_DISPONIBLE",
                "La oferta no esta disponible.",
            ));
        }
        let actuales = self.items_de(comprador_id).await?;
        let previa = actuales.iter().find(|i| i.oferta_id == oferta_id);
        let cantidad_nueva = previa.map(|i| i.cantidad).unwrap_or(0) + cantidad;
        if cantidad_nueva > CANTIDAD_MAXIMA_POR_LINEA {
            return Err(ApiError::dominio(
                "CANTIDAD_MAXIMA",
                format!("Maximo {CANTIDAD_MAXIMA_POR_LINEA} unidades por producto."),
            ));
        }
        let mut items: Vec<ItemCarrito> = actuales
            .iter()
            .filter(|i| i.oferta_id != oferta_id)
            .cloned()
            .collect();
        items.push(ItemCarrito {
            oferta_id: oferta_id.to_string(),
            cantidad: cantidad_nueva,
        });
        self.persistir(comprador_id, &items).await
    }

    pub async fn actualizar_cantidad(
        &self,
        comprador_id: &str,
        oferta_id: &str,
        cantidad: i64,
    ) -> Result<CarritoVista, ApiError> {
        self.purgar_expirados().await?;
        let items = self.items_de(comprador_id).await?;
        if !items.iter().any(|i| i.oferta_id == oferta_id) {
            return Err(ApiError::no_encontrado("Item del carrito", oferta_id));
        }
        if cantidad <= 0 {
            return self.quitar(comprador_id, oferta_id).await;
        }
        let mut nuevos: Vec<ItemCarrito> = items
            .iter()
            .filter(|i| i.oferta_id != oferta_id)
            .cloned()
            .collect();
        nuevos.push(ItemCarrito {
            oferta_id: oferta_id.to_string(),
            cantidad,
        });
        self.persistir(comprador_id, &nuevos).await
    }

    pub async fn quitar(
        &self,
        comprador_id: &str,
        oferta_id: &str,
    ) -> Result<CarritoVista, ApiError> {
        self.purgar_expirados().await?;
        let items = self.items_de(comprador_id).await?;
        if !items.iter().any(|i| i.oferta_id == oferta_id) {
            return Err(ApiError::no_encontrado("Item del carrito", oferta_id));
        }
        let nuevos: Vec<ItemCarrito> = items
            .into_iter()
            .filter(|i| i.oferta_id != oferta_id)
            .collect();
        self.persistir(comprador_id, &nuevos).await
    }

    pub async fn vaciar(&self, comprador_id: &str) -> Result<CarritoVista, ApiError> {
        self.purgar_expirados().await?;
        sqlx::query("DELETE FROM orders.carritos WHERE comprador_id = $1")
            .bind(crate::event_store::parsear_uuid(comprador_id)?)
            .execute(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(vista_vacia())
    }

    pub async fn items_para_checkout(
        &self,
        comprador_id: &str,
    ) -> Result<Vec<ItemCarrito>, ApiError> {
        self.purgar_expirados().await?;
        self.items_de(comprador_id).await
    }

    pub async fn vaciar_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        comprador_id: &str,
    ) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM orders.carritos WHERE comprador_id = $1")
            .bind(crate::event_store::parsear_uuid(comprador_id)?)
            .execute(&mut **tx)
            .await
            .map_err(error_db)?;
        Ok(())
    }

    async fn items_de(&self, comprador_id: &str) -> Result<Vec<ItemCarrito>, ApiError> {
        let fila = sqlx::query("SELECT items_json FROM orders.carritos WHERE comprador_id = $1")
            .bind(crate::event_store::parsear_uuid(comprador_id)?)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(fila
            .and_then(|fila| fila.try_get::<Value, _>("items_json").ok())
            .and_then(|valor| serde_json::from_value(valor).ok())
            .unwrap_or_default())
    }

    async fn persistir(
        &self,
        comprador_id: &str,
        items: &[ItemCarrito],
    ) -> Result<CarritoVista, ApiError> {
        let ids: Vec<String> = items.iter().map(|i| i.oferta_id.clone()).collect();
        let ofertas = self.clientes.ofertas(&ids).await;
        let por_id: std::collections::HashMap<&str, &OfertaExterna> =
            ofertas.iter().map(|o| (o.id.as_str(), o)).collect();
        let mut total = 0_i64;
        let mut validos = Vec::new();
        for item in items {
            let Some(oferta) = por_id.get(item.oferta_id.as_str()) else {
                continue;
            };
            total += money::multiplicar(money::parsear(&oferta.precio_venta)?, item.cantidad);
            validos.push(item.clone());
        }
        sqlx::query(
            "INSERT INTO orders.carritos (comprador_id, items_json, total_cents, actualizado_en)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (comprador_id)
             DO UPDATE SET items_json = EXCLUDED.items_json,
                           total_cents = EXCLUDED.total_cents,
                           actualizado_en = NOW()",
        )
        .bind(crate::event_store::parsear_uuid(comprador_id)?)
        .bind(sqlx::types::Json(&validos))
        .bind(total)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        self.obtener(comprador_id).await
    }

    async fn enriquecer(&self, fila: &sqlx::postgres::PgRow) -> Result<CarritoVista, ApiError> {
        let items: Vec<ItemCarrito> = fila
            .try_get::<Value, _>("items_json")
            .ok()
            .and_then(|valor| serde_json::from_value(valor).ok())
            .unwrap_or_default();
        if items.is_empty() {
            return Ok(vista_vacia());
        }
        let ids: Vec<String> = items.iter().map(|i| i.oferta_id.clone()).collect();
        let ofertas = self.clientes.ofertas(&ids).await;
        let por_id: std::collections::HashMap<&str, &OfertaExterna> =
            ofertas.iter().map(|o| (o.id.as_str(), o)).collect();
        let mut total = 0_i64;
        let mut enriquecidos = Vec::new();
        for item in &items {
            let Some(oferta) = por_id.get(item.oferta_id.as_str()) else {
                continue;
            };
            total += money::multiplicar(money::parsear(&oferta.precio_venta)?, item.cantidad);
            enriquecidos.push(ItemCarritoEnriquecido {
                oferta_id: item.oferta_id.clone(),
                cantidad: item.cantidad,
                tienda_id: oferta.tienda_id.clone(),
                vendedor_id: oferta.vendedor_id.clone(),
                sku: oferta.sku.clone(),
                producto_nombre: oferta.producto_nombre.clone(),
                precio_venta: oferta.precio_venta.clone(),
                stock: oferta.stock.unwrap_or(0),
                estado: oferta.estado.clone(),
            });
        }
        let actualizado: chrono::DateTime<Utc> =
            fila.try_get("actualizado_en").map_err(error_db)?;
        let expira = actualizado + Duration::milliseconds(CARRITO_EXPIRACION_MS);
        Ok(CarritoVista {
            items: enriquecidos,
            total: money::string(total),
            expira_en: Some(iso_ms(expira)),
        })
    }
}

fn vista_vacia() -> CarritoVista {
    CarritoVista {
        items: Vec::new(),
        total: "0.00".to_string(),
        expira_en: None,
    }
}

/// Convierte líneas del carrito en el comando de checkout.
pub fn lineas_de_checkout(items: &[ItemCarrito]) -> Vec<LineaCheckout> {
    items
        .iter()
        .map(|item| LineaCheckout {
            oferta_id: item.oferta_id.clone(),
            cantidad: item.cantidad,
            fulfillment: Some("none".to_string()),
            fulfillment_config: None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineaCheckout {
    pub oferta_id: String,
    pub cantidad: i64,
    pub fulfillment: Option<String>,
    pub fulfillment_config: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_convertir_lineas_del_carrito_al_checkout() {
        let lineas = lineas_de_checkout(&[ItemCarrito {
            oferta_id: "o-1".into(),
            cantidad: 2,
        }]);
        assert_eq!(lineas[0].oferta_id, "o-1");
        assert_eq!(lineas[0].cantidad, 2);
        assert_eq!(lineas[0].fulfillment.as_deref(), Some("none"));
    }

    #[test]
    fn debe_exponer_la_vista_vacia_con_total_cero() {
        assert_eq!(vista_vacia().total, "0.00");
        assert!(vista_vacia().items.is_empty());
    }
}
