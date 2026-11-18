//! RMA: devoluciones parciales por línea.
//!
//! La devolución total sigue siendo una transición de la orden. Esta entidad
//! permite devolver cantidades concretas, restockear en inventory y emitir el
//! evento que compensa la comisión proporcional.

use crate::auth::UsuarioContexto;
use crate::clients::ClientesPort;
use crate::db::{iso_ms, uuid_texto, Db};
use crate::errors::ApiError;
use crate::outbox::Outbox;
use crate::views::OrderViewRepository;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineaDevolucion {
    pub id: String,
    pub oferta_id: String,
    pub sku: String,
    pub cantidad: i64,
    pub precio_unitario_cents: i64,
    pub descuento_cents: i64,
    pub total_cents: i64,
    pub restock: bool,
    pub ubicacion_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Devolucion {
    pub id: String,
    pub order_id: String,
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub motivo: String,
    pub estado: String,
    pub total_cents: i64,
    pub moneda: String,
    pub creado_por: String,
    pub creado_en: String,
    pub actualizado_en: String,
    pub lineas: Vec<LineaDevolucion>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearDevolucionInput {
    #[serde(default)]
    pub motivo: String,
    #[serde(default)]
    pub items: Vec<ItemDevolucionInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDevolucionInput {
    pub oferta_id: String,
    pub cantidad: i64,
    #[serde(default = "restock_por_defecto")]
    pub restock: bool,
}

fn restock_por_defecto() -> bool {
    true
}

pub struct Devoluciones {
    db: Arc<Db>,
    vistas: Arc<OrderViewRepository>,
    clientes: Arc<dyn ClientesPort>,
    outbox: Outbox,
}

impl Devoluciones {
    pub fn nueva(
        db: Arc<Db>,
        vistas: Arc<OrderViewRepository>,
        clientes: Arc<dyn ClientesPort>,
        outbox: Outbox,
    ) -> Self {
        Self {
            db,
            vistas,
            clientes,
            outbox,
        }
    }

    pub async fn crear(
        &self,
        order_id: &str,
        actor: &UsuarioContexto,
        input: CrearDevolucionInput,
    ) -> Result<Devolucion, ApiError> {
        let orden = self
            .vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        autorizar(&orden, actor)?;
        if input.items.is_empty() {
            return Err(ApiError::dominio(
                "DEVOLUCION_VACIA",
                "La devolución debe incluir al menos una línea.",
            ));
        }
        let ya_devuelto = self.cantidades_devueltas(order_id).await?;
        let mut lineas = Vec::with_capacity(input.items.len());
        let mut total = 0i64;
        for item in &input.items {
            if item.cantidad <= 0 {
                return Err(ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad devuelta debe ser positiva.",
                ));
            }
            let linea = orden
                .items
                .iter()
                .find(|linea| linea.oferta_id == item.oferta_id)
                .ok_or_else(|| {
                    ApiError::dominio("LINEA_NO_ENCONTRADA", "La línea no pertenece a la orden.")
                })?;
            let devuelto = ya_devuelto.get(&item.oferta_id).copied().unwrap_or(0);
            if devuelto + item.cantidad > linea.cantidad {
                return Err(ApiError::dominio(
                    "CANTIDAD_EXCEDIDA",
                    "La cantidad devuelta supera lo comprado.",
                ));
            }
            let descuento = if linea.cantidad > 0 {
                ((linea.descuento_cents as f64 * item.cantidad as f64) / linea.cantidad as f64)
                    .round() as i64
            } else {
                0
            };
            let neto =
                (linea.precio_unitario_cents.saturating_mul(item.cantidad) - descuento).max(0);
            total += neto;
            lineas.push((linea, item.cantidad, descuento, neto, item.restock));
        }
        let id = Uuid::new_v4();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.devoluciones
               (id, order_id, empresa_id, negocio_id, motivo, estado, total_cents,
                moneda, creado_por)
             VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5, 'solicitada', $6, $7, $8::uuid)",
        )
        .bind(id)
        .bind(order_id)
        .bind(&orden.items[0].empresa_id)
        .bind(&orden.negocio_id)
        .bind(input.motivo.trim())
        .bind(total as i32)
        .bind(&orden.moneda)
        .bind(&actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        for (linea, cantidad, descuento, neto, restock) in lineas {
            sqlx::query(
                "INSERT INTO orders.devolucion_lineas
                   (devolucion_id, oferta_id, sku, cantidad, precio_unitario_cents,
                    descuento_cents, total_cents, restock, ubicacion_id)
                 VALUES ($1, $2::uuid, $3, $4, $5, $6, $7, $8, $9::uuid)",
            )
            .bind(id)
            .bind(&linea.oferta_id)
            .bind(&linea.sku)
            .bind(cantidad as i32)
            .bind(linea.precio_unitario_cents as i32)
            .bind(descuento as i32)
            .bind(neto as i32)
            .bind(restock)
            .bind(&linea.ubicacion_id)
            .execute(&mut *tx)
            .await
            .map_err(crate::event_store::error_db)?;
        }
        tx.commit().await.map_err(crate::event_store::error_db)?;
        self.obtener(&id.to_string())
            .await?
            .ok_or_else(ApiError::interno)
    }

    pub async fn listar_por_orden(
        &self,
        order_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Devolucion>, ApiError> {
        let orden = self
            .vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        autorizar(&orden, actor)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado, total_cents,
                    moneda, creado_por, creado_en, actualizado_en
               FROM orders.devoluciones WHERE order_id = $1::uuid
              ORDER BY creado_en DESC",
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let id = uuid_texto(&fila, "id").map_err(crate::event_store::error_db)?;
            if let Some(devolucion) = self.obtener(&id).await? {
                salida.push(devolucion);
            }
        }
        Ok(salida)
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Devolucion>, ApiError> {
        if uuid::Uuid::parse_str(id).is_err() {
            return Ok(None);
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado, total_cents,
                    moneda, creado_por, creado_en, actualizado_en
               FROM orders.devoluciones WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            tx.commit().await.map_err(crate::event_store::error_db)?;
            return Ok(None);
        };
        let lineas = sqlx::query(
            "SELECT id, oferta_id, sku, cantidad, precio_unitario_cents, descuento_cents,
                    total_cents, restock, ubicacion_id
               FROM orders.devolucion_lineas WHERE devolucion_id = $1::uuid
              ORDER BY oferta_id",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(Some(fila_devolucion(&fila, &lineas)?))
    }

    pub async fn aprobar(&self, id: &str, actor: &UsuarioContexto) -> Result<Devolucion, ApiError> {
        self.cambiar_estado(id, "aprobada", actor).await
    }

    pub async fn rechazar(
        &self,
        id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        self.cambiar_estado(id, "rechazada", actor).await
    }

    pub async fn recibir(&self, id: &str, actor: &UsuarioContexto) -> Result<Devolucion, ApiError> {
        let devolucion = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if !matches!(devolucion.estado.as_str(), "solicitada" | "aprobada") {
            return Err(ApiError::conflicto(format!(
                "La devolución no puede recibirse en estado {}.",
                devolucion.estado
            )));
        }
        let items: Vec<Value> = devolucion
            .lineas
            .iter()
            .filter(|linea| linea.restock)
            .map(|linea| {
                json!({
                    "empresa_id": devolucion.empresa_id,
                    "sku": linea.sku,
                    "cantidad": linea.cantidad,
                    "ubicacion_id": linea.ubicacion_id,
                })
            })
            .collect();
        if !items.is_empty()
            && !self
                .clientes
                .devolver_stock(&devolucion.order_id, id, &items)
                .await
        {
            return Err(ApiError::servicio_no_disponible("inventory-service"));
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "UPDATE orders.devoluciones SET estado = 'recibida', actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let evento = json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": "devolucion.recibida",
            "ocurrido_en": iso_ms(Utc::now()),
            "data": {
                "order_id": devolucion.order_id,
                "devolucion_id": devolucion.id,
                "actor_id": actor.user_id,
                "items": devolucion.lineas.iter().map(|linea| json!({
                    "linea_clave": linea.oferta_id,
                    "sku": linea.sku,
                    "cantidad": linea.cantidad,
                    "monto_cents": linea.total_cents,
                })).collect::<Vec<_>>(),
            }
        });
        let sql = format!(
            "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
             VALUES ($1::uuid, $2, $3::jsonb, 'pendiente', NOW())",
            self.outbox.tabla()
        );
        sqlx::query(&sql)
            .bind(evento["event_id"].as_str().unwrap_or_default())
            .bind("devolucion.recibida")
            .bind(evento.to_string())
            .execute(&mut *tx)
            .await
            .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    async fn cambiar_estado(
        &self,
        id: &str,
        estado: &str,
        actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        if !actor.es_admin_global() {
            return Err(ApiError::acceso_denegado(
                "Solo un administrador puede cambiar el estado de la devolución.",
            ));
        }
        let actual = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if actual.estado != "solicitada" {
            return Err(ApiError::conflicto("La devolución ya fue procesada."));
        }
        sqlx::query(
            "UPDATE orders.devoluciones SET estado = $2, actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .bind(estado)
        .execute(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    async fn cantidades_devueltas(
        &self,
        order_id: &str,
    ) -> Result<std::collections::HashMap<String, i64>, ApiError> {
        let filas = sqlx::query(
            "SELECT l.oferta_id::text AS oferta_id, SUM(l.cantidad)::bigint AS cantidad
               FROM orders.devolucion_lineas l
               JOIN orders.devoluciones d ON d.id = l.devolucion_id
              WHERE d.order_id = $1::uuid AND d.estado <> 'rechazada'
              GROUP BY l.oferta_id",
        )
        .bind(order_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        let mut salida = std::collections::HashMap::new();
        for fila in filas {
            salida.insert(
                fila.try_get::<String, _>("oferta_id")
                    .map_err(crate::event_store::error_db)?,
                fila.try_get::<i64, _>("cantidad")
                    .map_err(crate::event_store::error_db)?,
            );
        }
        Ok(salida)
    }
}

fn autorizar(orden: &crate::views::OrderView, actor: &UsuarioContexto) -> Result<(), ApiError> {
    if actor.es_admin_global()
        || (actor.rol == "comprador" && orden.comprador_id == actor.user_id)
        || (actor.rol == "empresario"
            && actor.tenant_id.as_deref()
                == orden.items.first().map(|item| item.empresa_id.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado(
            "No tienes acceso a las devoluciones de esta orden.",
        ))
    }
}

fn fila_devolucion(
    fila: &sqlx::postgres::PgRow,
    lineas: &[sqlx::postgres::PgRow],
) -> Result<Devolucion, ApiError> {
    let creado: DateTime<Utc> = fila
        .try_get("creado_en")
        .map_err(crate::event_store::error_db)?;
    let actualizado: DateTime<Utc> = fila
        .try_get("actualizado_en")
        .map_err(crate::event_store::error_db)?;
    Ok(Devolucion {
        id: uuid_texto(fila, "id").map_err(crate::event_store::error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(crate::event_store::error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        negocio_id: crate::db::uuid_opcional(fila, "negocio_id")
            .map_err(crate::event_store::error_db)?,
        motivo: fila
            .try_get("motivo")
            .map_err(crate::event_store::error_db)?,
        estado: fila
            .try_get("estado")
            .map_err(crate::event_store::error_db)?,
        total_cents: i64::from(
            fila.try_get::<i32, _>("total_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        creado_por: uuid_texto(fila, "creado_por").map_err(crate::event_store::error_db)?,
        creado_en: iso_ms(creado),
        actualizado_en: iso_ms(actualizado),
        lineas: lineas
            .iter()
            .map(|linea| {
                Ok(LineaDevolucion {
                    id: uuid_texto(linea, "id").map_err(crate::event_store::error_db)?,
                    oferta_id: uuid_texto(linea, "oferta_id")
                        .map_err(crate::event_store::error_db)?,
                    sku: linea.try_get("sku").map_err(crate::event_store::error_db)?,
                    cantidad: i64::from(
                        linea
                            .try_get::<i32, _>("cantidad")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    precio_unitario_cents: i64::from(
                        linea
                            .try_get::<i32, _>("precio_unitario_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    descuento_cents: i64::from(
                        linea
                            .try_get::<i32, _>("descuento_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    total_cents: i64::from(
                        linea
                            .try_get::<i32, _>("total_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    restock: linea
                        .try_get("restock")
                        .map_err(crate::event_store::error_db)?,
                    ubicacion_id: crate::db::uuid_opcional(linea, "ubicacion_id")
                        .map_err(crate::event_store::error_db)?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
    })
}
