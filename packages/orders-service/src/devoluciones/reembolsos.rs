use super::prelude::*;
use super::*;

impl Devoluciones {
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
            self.actualizar_reembolso(id, "no_aplica", 0, "", None)
                .await?;
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
        let confirmado = reembolsado - devolucion.reembolsado_cents;
        let evento = (confirmado > 0).then(|| {
            json!({
                "event_id": Uuid::new_v4().to_string(),
                "tipo": "devolucion.reembolsada",
                "ocurrido_en": iso_ms(Utc::now()),
                "data": {
                    "order_id": devolucion.order_id,
                    "devolucion_id": devolucion.id,
                    "empresa_id": devolucion.empresa_id,
                    "negocio_id": devolucion.negocio_id,
                    "actor_id": actor.user_id,
                    "moneda": devolucion.moneda,
                    "monto_cents": confirmado,
                    "reembolsado_cents": reembolsado,
                    "total_cents": devolucion.total_cents,
                    "estado": estado,
                    "items": devolucion.lineas.iter().map(|linea| json!({
                        "linea_clave": linea.oferta_id,
                        "sku": linea.sku,
                        "cantidad": linea.cantidad,
                        "base_cents": linea.base_cents,
                        "impuesto_cents": linea.impuesto_cents,
                        "monto_cents": linea.total_cents,
                    })).collect::<Vec<_>>(),
                }
            })
        });
        self.actualizar_reembolso(id, estado, reembolsado, &error, evento)
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
        evento: Option<Value>,
    ) -> Result<(), ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
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
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        if let Some(evento) = evento {
            let sql = format!(
                "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
                 VALUES ($1::uuid, $2, $3::jsonb, 'pendiente', NOW())",
                self.outbox.tabla()
            );
            sqlx::query(&sql)
                .bind(evento["event_id"].as_str().unwrap_or_default())
                .bind(evento["tipo"].as_str().unwrap_or_default())
                .bind(evento.to_string())
                .execute(&mut *tx)
                .await
                .map_err(crate::event_store::error_db)?;
        }
        tx.commit().await.map_err(crate::event_store::error_db)?;
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
}
