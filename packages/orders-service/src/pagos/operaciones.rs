use super::comunes::*;
use super::prelude::*;
use super::*;
use serde_json::json;

impl Pagos {
    pub async fn confirmar(&self, pago_id: &str, actor_id: &str) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "confirmado", "", actor_id).await
    }

    pub async fn rechazar(
        &self,
        pago_id: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "rechazado", motivo, actor_id).await
    }

    async fn procesar(
        &self,
        pago_id: &str,
        estado: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET estado = $2,
                    motivo_rechazo = CASE WHEN $2 = 'rechazado' THEN $3 ELSE motivo_rechazo END,
                    confirmado_por = $4::uuid,
                    confirmado_en = NOW(),
                    actualizado_en = NOW()
              WHERE id = $1::uuid AND estado = 'pendiente'
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .bind(estado)
        .bind(motivo)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            let existe: Option<String> =
                sqlx::query_scalar("SELECT estado FROM orders.pagos WHERE id = $1::uuid")
                    .bind(pago_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(crate::event_store::error_db)?;
            return match existe {
                Some(estado) => Err(ApiError::conflicto(format!(
                    "El pago ya fue procesado (estado {estado})."
                ))),
                None => Err(ApiError::no_encontrado("Pago", pago_id)),
            };
        };
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    /// Deja constancia de un pago aprobado que no pudo aplicarse (por ejemplo,
    /// capturado después de que la orden expirara). No cambia su estado; un
    /// operador decide el reembolso con el flujo existente.
    pub async fn registrar_incidencia(
        &self,
        pago: &Pago,
        tipo: &str,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        self.registrar_evento(
            &mut tx,
            &pago.id,
            tipo,
            pago.monto_cents,
            motivo,
            None,
            json!({ "estado": pago.estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(())
    }

    /// Registra el pago de una orden. Los métodos automáticos nacen
    /// confirmados; los manuales quedan pendientes hasta la confirmación. Si
    /// el método declara `cargo_externo` (o es `simulado`), se crea el cargo en
    /// la pasarela antes de persistir la referencia.
    pub async fn registrar(
        &self,
        order_id: &str,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
        metodo: &MetodoPago,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<Option<String>, ApiError> {
        let cargo_externo = metodo
            .config
            .get("cargo_externo")
            .and_then(Value::as_bool)
            .unwrap_or(metodo.codigo == "simulado");
        let (proveedor_codigo, proveedor) = self.proveedor_para(empresa_id, negocio_id).await?;
        let simulado_permitido = metodo.codigo == "simulado"
            || metodo
                .config
                .get("simulado")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if cargo_externo && proveedor.nombre() == "simulado" && !simulado_permitido {
            return Err(ApiError::dominio(
                "PAGO_SIMULADO_NO_PERMITIDO",
                "El método de pago requiere una pasarela real configurada.",
            ));
        }
        // Una pasarela real nunca nace confirmada: el dinero todavía no se ha
        // cobrado. Solo el simulado puede dar por pagada la orden al instante.
        let estado = if metodo.requiere_confirmacion
            || (cargo_externo && proveedor.nombre() != "simulado")
        {
            "pendiente"
        } else {
            "confirmado"
        };
        let cargo = if cargo_externo {
            Some(proveedor.crear_cargo(order_id, monto_cents, moneda).await?)
        } else {
            None
        };
        let proveedor = if cargo.is_some() {
            proveedor_codigo
        } else {
            "manual".to_string()
        };
        let referencia = cargo
            .as_ref()
            .map(|cargo| cargo.referencia.clone())
            .unwrap_or_default();
        let pago_url = cargo.and_then(|cargo| cargo.url);
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.pagos
               (order_id, empresa_id, metodo_codigo, estado, monto_cents, moneda,
                referencia, proveedor, confirmado_en)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8,
                     CASE WHEN $4 = 'confirmado' THEN NOW() ELSE NULL END)",
        )
        .bind(order_id)
        .bind(empresa_id)
        .bind(&metodo.codigo)
        .bind(estado)
        .bind(monto_cents)
        .bind(moneda)
        .bind(&referencia)
        .bind(&proveedor)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(pago_url)
    }

    /// Reembolso total o parcial. Actualiza el acumulado y deja auditoría.
    pub async fn reembolsar(
        &self,
        pago_id: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        if !matches!(pago.estado.as_str(), "confirmado" | "reembolsado_parcial") {
            return Err(ApiError::conflicto(format!(
                "El pago no admite reembolso en estado {}.",
                pago.estado
            )));
        }
        if monto_cents <= 0 || pago.monto_reembolsado_cents + monto_cents > pago.monto_cents {
            return Err(ApiError::dominio(
                "MONTO_REEMBOLSO_INVALIDO",
                "El reembolso debe ser positivo y no superar el monto del pago.",
            ));
        }
        if !pago.referencia.is_empty() {
            self.proveedor_de_pago(&pago)
                .await?
                .reembolsar(referencia_operativa(&pago), monto_cents)
                .await?;
        }
        let total_reembolsado = pago.monto_reembolsado_cents + monto_cents;
        let estado = if total_reembolsado == pago.monto_cents {
            "reembolsado"
        } else {
            "reembolsado_parcial"
        };
        let actualizado = self
            .actualizar_estado(
                pago_id,
                estado,
                total_reembolsado,
                "pago.reembolsado",
                monto_cents,
                motivo,
                actor_id,
            )
            .await?;
        Ok(actualizado)
    }

    /// Contracargo (chargeback) del emisor. No mueve la orden por sí solo.
    pub async fn contracargar(
        &self,
        pago_id: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        if pago.estado == "contracargo" {
            return Err(ApiError::conflicto("El pago ya tiene un contracargo."));
        }
        if monto_cents <= 0 || monto_cents > pago.monto_cents {
            return Err(ApiError::dominio(
                "MONTO_CONTRACARGO_INVALIDO",
                "El contracargo debe ser positivo y no superar el pago.",
            ));
        }
        if !pago.referencia.is_empty() {
            self.proveedor_de_pago(&pago)
                .await?
                .contracargar(referencia_operativa(&pago), monto_cents, motivo)
                .await?;
        }
        let total_reembolsado = (pago.monto_reembolsado_cents + monto_cents).min(pago.monto_cents);
        self.actualizar_estado(
            pago_id,
            "contracargo",
            total_reembolsado,
            "pago.contracargo",
            monto_cents,
            motivo,
            actor_id,
        )
        .await
    }

    /// Marca el pago como conciliado por un operador.
    pub async fn conciliar(&self, pago_id: &str, actor_id: &str) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET conciliado_en = COALESCE(conciliado_en, NOW()), actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        self.registrar_evento(
            &mut tx,
            pago_id,
            "pago.conciliado",
            0,
            "",
            Some(actor_id),
            json!({ "estado": pago.estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    #[allow(clippy::too_many_arguments)]
    async fn actualizar_estado(
        &self,
        pago_id: &str,
        estado: &str,
        monto_reembolsado_cents: i64,
        evento: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET estado = $2, monto_reembolsado_cents = $3, actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .bind(estado)
        .bind(monto_reembolsado_cents)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        self.registrar_evento(
            &mut tx,
            pago_id,
            evento,
            monto_cents,
            motivo,
            Some(actor_id),
            json!({ "estado": estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    #[allow(clippy::too_many_arguments)]
    async fn registrar_evento(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        pago_id: &str,
        tipo: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: Option<&str>,
        payload: Value,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO orders.pagos_eventos
               (pago_id, tipo, monto_cents, motivo, actor_id, payload)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::jsonb)",
        )
        .bind(pago_id)
        .bind(tipo)
        .bind(monto_cents as i32)
        .bind(motivo)
        .bind(actor_id)
        .bind(payload)
        .execute(&mut **tx)
        .await
        .map_err(crate::event_store::error_db)?;
        Ok(())
    }
}
