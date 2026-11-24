use super::prelude::*;
use super::*;

impl Fidelizacion {
    /// Acumula puntos por una orden pagada. Idempotente por `(tarjeta, orden)`.
    /// Devuelve `None` cuando no hay programa activo, no hay tarjeta, el total
    /// no genera puntos o la orden ya fue acumulada.
    pub async fn acumular_por_orden(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        total_cents: i64,
    ) -> Result<Option<Movimiento>, ApiError> {
        if total_cents <= 0 || parte_id.is_empty() || order_id.is_empty() {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let programa = sqlx::query(
            "SELECT puntos_por_unidad FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid AND activo",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(programa) = programa else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        let por_unidad: i32 = programa.try_get("puntos_por_unidad").map_err(error_db)?;
        let puntos = (total_cents * i64::from(por_unidad) / 100) as i32;
        if puntos <= 0 {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        }
        let tarjeta = sqlx::query(
            "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(tarjeta) = tarjeta else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        }
        let tarjeta_id = uuid_texto(&tarjeta, "id").map_err(error_db)?;
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        let saldo = actuales + puntos;
        let insertado = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id,
                descripcion)
             VALUES ($1::uuid, $2::uuid, 'acumulacion', $3, $4, $5::uuid, $6)
             ON CONFLICT (tarjeta_id, order_id)
               WHERE tipo = 'acumulacion' AND order_id IS NOT NULL
               DO NOTHING
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(puntos)
        .bind(saldo)
        .bind(order_id)
        .bind("Puntos por orden pagada")
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = insertado else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_movimiento(&fila).map(Some)
    }

    /// Recompensas activas de las empresas donde el comprador tiene tarjeta.
    pub async fn listar_recompensas_de_comprador(
        &self,
        comprador_id: &str,
    ) -> Result<Vec<Recompensa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_RECOMPENSA}
               FROM orders.fidelizacion_recompensas r
              WHERE r.activo
                AND r.empresa_id IN (
                    SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                     WHERE t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $1::uuid AND parte_id IS NOT NULL))
              ORDER BY r.puntos, r.codigo"
        ))
        .bind(comprador_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recompensa).collect()
    }

    /// Canje iniciado por el comprador: valida que la tarjeta pertenezca a una
    /// de sus órdenes y delega en `canjear` con la empresa de la tarjeta.
    pub async fn canjear_de_comprador(
        &self,
        comprador_id: &str,
        dto: &Value,
    ) -> Result<Canje, ApiError> {
        let tarjeta_ref = texto(dto, "tarjeta_id");
        let codigo = texto(dto, "codigo_tarjeta").to_uppercase();
        if tarjeta_ref.is_empty() && codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "tarjeta_id", "restricciones": ["isUuid"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let empresa: Option<uuid::Uuid> = if tarjeta_ref.is_empty() {
            sqlx::query_scalar(
                "SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                  WHERE t.codigo = $1
                    AND t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
            )
            .bind(&codigo)
            .bind(comprador_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
        } else {
            sqlx::query_scalar(
                "SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                  WHERE t.id = $1::uuid
                    AND t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
            )
            .bind(&tarjeta_ref)
            .bind(comprador_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
        };
        let Some(empresa) = empresa else {
            let referencia = if tarjeta_ref.is_empty() {
                codigo.as_str()
            } else {
                tarjeta_ref.as_str()
            };
            return Err(ApiError::no_encontrado("Tarjeta", referencia));
        };
        tx.commit().await.map_err(error_db)?;
        self.canjear(&empresa.to_string(), dto, comprador_id).await
    }

    /// Expira los puntos de tarjetas activas sin movimientos durante
    /// `expira_meses`. Devuelve cuántas tarjetas se expiraron.
    ///
    /// Es idempotente: una tarjeta con saldo 0 se omite, y la expiración se
    /// registra como movimiento `expiracion` para conservar el historial.
    pub async fn expirar_inactivas(&self) -> Result<u64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT t.id, t.empresa_id, t.puntos
               FROM orders.fidelizacion_tarjetas t
               JOIN orders.fidelizacion_programas p ON p.id = t.programa_id
              WHERE p.activo
                AND p.expira_meses IS NOT NULL
                AND t.estado = 'activa'
                AND t.puntos > 0
                AND COALESCE(
                      (SELECT MAX(m.creado_en)
                         FROM orders.fidelizacion_movimientos m
                        WHERE m.tarjeta_id = t.id),
                      t.creado_en
                    ) < NOW() - make_interval(months => p.expira_meses)
              FOR UPDATE OF t",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        let mut expiradas = 0_u64;
        for fila in &filas {
            let tarjeta_id = uuid_texto(fila, "id").map_err(error_db)?;
            let empresa_id = uuid_texto(fila, "empresa_id").map_err(error_db)?;
            let puntos: i32 = fila.try_get("puntos").map_err(error_db)?;
            sqlx::query(
                "INSERT INTO orders.fidelizacion_movimientos
                   (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, descripcion)
                 VALUES ($1::uuid, $2::uuid, 'expiracion', $3, 0,
                         'Puntos expirados por inactividad')",
            )
            .bind(&empresa_id)
            .bind(&tarjeta_id)
            .bind(-puntos)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            sqlx::query(
                "UPDATE orders.fidelizacion_tarjetas
                    SET puntos = 0, actualizado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(&tarjeta_id)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            expiradas += 1;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(expiradas)
    }
}
