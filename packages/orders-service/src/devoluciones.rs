//! RMA: devoluciones parciales por línea.
//!
//! La devolución total sigue siendo una transición de la orden. Esta entidad
//! permite devolver cantidades concretas, restockear en inventory y emitir el
//! evento que compensa la comisión proporcional.

use crate::auth::UsuarioContexto;
use crate::clients::ClientesPort;
use crate::db::{iso_ms, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::outbox::Outbox;
use crate::pagos::Pagos;
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
    pub base_cents: i64,
    pub impuesto_cents: i64,
    pub tasa_impuesto_bps: i64,
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
    pub base_cents: i64,
    pub impuesto_cents: i64,
    pub total_cents: i64,
    pub moneda: String,
    pub reembolso_estado: String,
    pub reembolsado_cents: i64,
    pub reembolso_error: String,
    pub nota_credito_id: Option<String>,
    pub nota_credito_estado: String,
    pub nota_credito_error: String,
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
    pagos: Arc<Pagos>,
    outbox: Outbox,
}

impl Devoluciones {
    pub fn nueva(
        db: Arc<Db>,
        vistas: Arc<OrderViewRepository>,
        clientes: Arc<dyn ClientesPort>,
        pagos: Arc<Pagos>,
        outbox: Outbox,
    ) -> Self {
        Self {
            db,
            vistas,
            clientes,
            pagos,
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
        let mut base_total = 0i64;
        let mut impuesto_total = 0i64;
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
            let factor = if linea.cantidad > 0 {
                item.cantidad as f64 / linea.cantidad as f64
            } else {
                0.0
            };
            let total_linea = if linea.total_linea_cents > 0 {
                (linea.total_linea_cents as f64 * factor).round() as i64
            } else {
                neto
            };
            let impuesto = if linea.impuesto_cents > 0 {
                (linea.impuesto_cents as f64 * factor).round() as i64
            } else {
                0
            };
            let base = (total_linea - impuesto).max(0);
            total += total_linea;
            base_total += base;
            impuesto_total += impuesto;
            lineas.push((
                linea,
                item.cantidad,
                descuento,
                base,
                impuesto,
                total_linea,
                item.restock,
            ));
        }
        let id = Uuid::new_v4();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.devoluciones
               (id, order_id, empresa_id, negocio_id, motivo, estado,
                base_cents, impuesto_cents, total_cents, moneda, creado_por)
             VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5, 'solicitada',
                     $6, $7, $8, $9, $10::uuid)",
        )
        .bind(id)
        .bind(order_id)
        .bind(&orden.items[0].empresa_id)
        .bind(&orden.negocio_id)
        .bind(input.motivo.trim())
        .bind(base_total as i32)
        .bind(impuesto_total as i32)
        .bind(total as i32)
        .bind(&orden.moneda)
        .bind(&actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        for (linea, cantidad, descuento, base, impuesto, total_linea, restock) in lineas {
            sqlx::query(
                "INSERT INTO orders.devolucion_lineas
                   (devolucion_id, oferta_id, sku, cantidad, precio_unitario_cents,
                    descuento_cents, base_cents, impuesto_cents, tasa_impuesto_bps,
                    total_cents, restock, ubicacion_id)
                 VALUES ($1, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::uuid)",
            )
            .bind(id)
            .bind(&linea.oferta_id)
            .bind(&linea.sku)
            .bind(cantidad as i32)
            .bind(linea.precio_unitario_cents as i32)
            .bind(descuento as i32)
            .bind(base as i32)
            .bind(impuesto as i32)
            .bind(linea.tasa_impuesto_bps as i32)
            .bind(total_linea as i32)
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
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado,
                    base_cents, impuesto_cents, total_cents, moneda,
                    reembolso_estado, reembolsado_cents, reembolso_error,
                    nota_credito_id, nota_credito_estado, nota_credito_error,
                    creado_por, creado_en, actualizado_en
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
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado,
                    base_cents, impuesto_cents, total_cents, moneda,
                    reembolso_estado, reembolsado_cents, reembolso_error,
                    nota_credito_id, nota_credito_estado, nota_credito_error,
                    creado_por, creado_en, actualizado_en
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
                    base_cents, impuesto_cents, tasa_impuesto_bps, total_cents,
                    restock, ubicacion_id
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
        // El reembolso y la nota de crédito son efectos posteriores: si fallan,
        // la devolución queda recibida y se pueden reintentar por API.
        let _ = self.reembolsar_devolucion(id, actor).await;
        let _ = self.emitir_nota_credito(id, actor).await;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    pub async fn reembolsar_devolucion(
        &self,
        id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        let devolucion = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if matches!(
            devolucion.reembolso_estado.as_str(),
            "reembolsado" | "no_aplica"
        ) {
            return Ok(devolucion);
        }
        if devolucion.total_cents <= 0 {
            self.actualizar_reembolso(id, "no_aplica", 0, "").await?;
            return self.obtener(id).await?.ok_or_else(ApiError::interno);
        }
        let pagos = self.pagos.listar_por_orden(&devolucion.order_id).await?;
        let mut restante = (devolucion.total_cents - devolucion.reembolsado_cents).max(0);
        let mut reembolsado = devolucion.reembolsado_cents;
        let mut error = String::new();
        let mut habia_pago = false;
        for pago in pagos.iter().filter(|pago| {
            matches!(pago.estado.as_str(), "confirmado" | "reembolsado_parcial")
                && pago.monto_cents > pago.monto_reembolsado_cents
        }) {
            habia_pago = true;
            if restante <= 0 {
                break;
            }
            let disponible = pago.monto_cents - pago.monto_reembolsado_cents;
            let monto = restante.min(disponible);
            match self
                .pagos
                .reembolsar(&pago.id, monto, "devolucion RMA", &actor.user_id)
                .await
            {
                Ok(_) => {
                    restante -= monto;
                    reembolsado += monto;
                }
                Err(fallo) => {
                    error = fallo.mensaje;
                    break;
                }
            }
        }
        let estado = if reembolsado >= devolucion.total_cents {
            "reembolsado"
        } else if reembolsado > 0 {
            "parcial"
        } else if habia_pago {
            "pendiente"
        } else {
            "no_aplica"
        };
        if estado == "no_aplica" {
            error.clear();
        }
        self.actualizar_reembolso(id, estado, reembolsado, &error)
            .await?;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    pub async fn emitir_nota_credito(
        &self,
        id: &str,
        _actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        let devolucion = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if matches!(
            devolucion.nota_credito_estado.as_str(),
            "emitida" | "no_aplica"
        ) {
            return Ok(devolucion);
        }
        if devolucion.total_cents <= 0 {
            self.actualizar_nota_credito(id, None, "no_aplica", "")
                .await?;
            return self.obtener(id).await?.ok_or_else(ApiError::interno);
        }
        let orden = self
            .vistas
            .encontrar(&self.db, &devolucion.order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", &devolucion.order_id))?;
        let payload = json!({
            "orden_id": devolucion.order_id,
            "devolucion_id": devolucion.id,
            "empresa_id": devolucion.empresa_id,
            "negocio_id": devolucion.negocio_id,
            "parte_id": orden.parte_id,
            "base_cents": devolucion.base_cents,
            "iva_cents": devolucion.impuesto_cents,
            "total_cents": devolucion.total_cents,
            "moneda": devolucion.moneda,
            "items": devolucion.lineas.iter().map(|linea| json!({
                "sku": linea.sku,
                "cantidad": linea.cantidad,
                "base_cents": linea.base_cents,
                "iva_cents": linea.impuesto_cents,
                "total_cents": linea.total_cents,
            })).collect::<Vec<_>>(),
        });
        match self.clientes.nota_credito(&payload).await {
            Some(comprobante_id) => {
                self.actualizar_nota_credito(id, Some(&comprobante_id), "emitida", "")
                    .await?;
            }
            None => {
                self.actualizar_nota_credito(id, None, "fallida", "finance-service no disponible")
                    .await?;
            }
        }
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    async fn actualizar_reembolso(
        &self,
        id: &str,
        estado: &str,
        reembolsado_cents: i64,
        error: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE orders.devoluciones
                SET reembolso_estado = $2,
                    reembolsado_cents = $3,
                    reembolso_error = $4,
                    actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .bind(estado)
        .bind(reembolsado_cents as i32)
        .bind(error)
        .execute(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        Ok(())
    }

    async fn actualizar_nota_credito(
        &self,
        id: &str,
        comprobante_id: Option<&str>,
        estado: &str,
        error: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE orders.devoluciones
                SET nota_credito_id = $2::uuid,
                    nota_credito_estado = $3,
                    nota_credito_error = $4,
                    actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .bind(comprobante_id)
        .bind(estado)
        .bind(error)
        .execute(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        Ok(())
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
        base_cents: i64::from(fila.try_get::<i32, _>("base_cents").unwrap_or(0)),
        impuesto_cents: i64::from(fila.try_get::<i32, _>("impuesto_cents").unwrap_or(0)),
        total_cents: i64::from(
            fila.try_get::<i32, _>("total_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        reembolso_estado: fila
            .try_get("reembolso_estado")
            .unwrap_or_else(|_| "pendiente".to_string()),
        reembolsado_cents: i64::from(fila.try_get::<i32, _>("reembolsado_cents").unwrap_or(0)),
        reembolso_error: fila.try_get("reembolso_error").unwrap_or_default(),
        nota_credito_id: uuid_opcional(fila, "nota_credito_id")
            .map_err(crate::event_store::error_db)?,
        nota_credito_estado: fila
            .try_get("nota_credito_estado")
            .unwrap_or_else(|_| "pendiente".to_string()),
        nota_credito_error: fila.try_get("nota_credito_error").unwrap_or_default(),
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
                    base_cents: i64::from(linea.try_get::<i32, _>("base_cents").unwrap_or(0)),
                    impuesto_cents: i64::from(
                        linea.try_get::<i32, _>("impuesto_cents").unwrap_or(0),
                    ),
                    tasa_impuesto_bps: i64::from(
                        linea.try_get::<i32, _>("tasa_impuesto_bps").unwrap_or(0),
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
