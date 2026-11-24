use super::comunes::*;
use super::configuracion::fusionar_config_tenant;
use super::prelude::*;
use super::proveedores::*;
use super::*;

impl Pagos {
    /// Secreto de webhook configurado por el proveedor/tenant. `None` deja que
    /// el handler use el secreto global como compatibilidad.
    pub async fn secreto_webhook(&self, pago_id: &str) -> Result<Option<String>, ApiError> {
        let Some(pago) = self.obtener(pago_id).await? else {
            return Ok(None);
        };
        let Some(empresa_id) = pago.empresa_id.as_deref() else {
            return Ok(None);
        };
        let Some(config) = self.configuracion(empresa_id, None).await? else {
            return Ok(None);
        };
        if config.proveedor_codigo != pago.proveedor {
            return Ok(None);
        }
        let mut base = config.config.clone();
        if let Ok(proveedores) = self.listar_proveedores().await {
            if let Some(proveedor) = proveedores.into_iter().find(|p| p.codigo == pago.proveedor) {
                base = fusionar_config_tenant(&proveedor.config, &base);
            }
        }
        Ok(base
            .get("webhook_secreto_env")
            .and_then(Value::as_str)
            .and_then(|variable| std::env::var(variable).ok())
            .filter(|valor| !valor.is_empty()))
    }

    pub(super) async fn proveedor_de_pago(
        &self,
        pago: &Pago,
    ) -> Result<Arc<dyn ProveedorPagos>, ApiError> {
        let Some(empresa_id) = pago.empresa_id.as_deref() else {
            return Ok(Arc::clone(&self.proveedor_default));
        };
        match self
            .proveedor_por_codigo(empresa_id, None, &pago.proveedor)
            .await
        {
            Ok(proveedor) => Ok(proveedor),
            Err(_) if pago.proveedor == self.proveedor_default.nombre() => {
                Ok(Arc::clone(&self.proveedor_default))
            }
            Err(error) => Err(error),
        }
    }

    /// Resuelve un webhook sin confiar en su contenido: busca el pago por la
    /// referencia y consulta el estado real a la pasarela. Devuelve el pago y
    /// la decisión; el llamador mueve la orden con la saga existente.
    pub async fn resolver_webhook(
        &self,
        proveedor: &str,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<(Pago, EstadoExterno), ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en, confirmado_en, creado_en
               FROM orders.pagos
              WHERE referencia = $1 AND proveedor = $2
              ORDER BY creado_en DESC LIMIT 1",
        )
        .bind(referencia)
        .bind(proveedor)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Pago", referencia));
        };
        let pago = fila_pago(&fila)?;
        let pago = self
            .guardar_referencia_proveedor(pago, proveedor, datos)
            .await?;
        if matches!(
            pago.estado.as_str(),
            "confirmado" | "reembolsado" | "reembolsado_parcial"
        ) {
            return Ok((pago, EstadoExterno::Aprobado));
        }
        let proveedor = self.proveedor_de_pago(&pago).await?;
        let estado = proveedor
            .consultar(referencia_operativa(&pago), datos)
            .await?;
        Ok((pago, estado))
    }

    /// Persiste el identificador de la transacción que informa la pasarela.
    /// Tilopay lo necesita para consultar y reembolsar; Pagadito usa el token
    /// de la transacción para consultar el estado.
    async fn guardar_referencia_proveedor(
        &self,
        mut pago: Pago,
        proveedor: &str,
        datos: Option<&Value>,
    ) -> Result<Pago, ApiError> {
        if !pago.referencia_proveedor.is_empty() {
            return Ok(pago);
        }
        let Some(valor) = referencia_proveedor_de(proveedor, datos) else {
            return Ok(pago);
        };
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "UPDATE orders.pagos
                SET referencia_proveedor = $2, actualizado_en = NOW()
              WHERE id = $1::uuid AND referencia_proveedor = ''",
        )
        .bind(&pago.id)
        .bind(&valor)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        pago.referencia_proveedor = valor;
        Ok(pago)
    }
}

/// Identificador de la transacción en la pasarela, si el aviso lo trae.
pub(super) fn referencia_proveedor_de(proveedor: &str, datos: Option<&Value>) -> Option<String> {
    let datos = datos?;
    let valor = match proveedor {
        "tilopay" => datos.get("orderNumber").and_then(Value::as_str),
        "pagadito" => datos
            .get("resource")
            .and_then(|recurso| recurso.get("token"))
            .and_then(Value::as_str),
        _ => None,
    }?
    .trim();
    (!valor.is_empty()).then(|| valor.to_string())
}
