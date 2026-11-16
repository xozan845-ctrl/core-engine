use super::*;

impl Productos {
    pub async fn esta_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let procesado = self
            .esta_procesado_en_transaccion(&mut tx, event_id)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(procesado)
    }

    pub async fn esta_procesado_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
    ) -> Result<bool, ApiError> {
        let fila =
            sqlx::query("SELECT 1 FROM catalog.eventos_procesados WHERE event_id = $1::uuid")
                .bind(event_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn registrar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.registrar_procesado_en_transaccion(&mut tx, event_id, tipo)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn registrar_procesado_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
        tipo: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO catalog.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1::uuid, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        Ok(())
    }
}
