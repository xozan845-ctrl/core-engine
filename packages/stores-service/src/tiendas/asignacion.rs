use super::*;

impl Tiendas {
    /// Asignación atómica de vendedor (solo si la tienda sigue libre).
    pub async fn asignar_vendedor(
        &self,
        tienda_id: &str,
        empresa_id: &str,
        usuario_id: &str,
        participacion: i64,
    ) -> Result<(), ApiError> {
        if !(0..=100).contains(&participacion) {
            return Err(ApiError::dominio(
                "PARTICIPACION_INVALIDA",
                "La participación debe estar entre 0 y 100.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(
            "UPDATE stores.tiendas
                SET vendedor_id = $1::uuid, propietario_usuario_id = $1::uuid,
                    participacion_porcentaje = $2::int, actualizado_en = NOW()
              WHERE id = $3::uuid AND empresa_id = $4::uuid AND vendedor_id IS NULL
            RETURNING id",
        )
        .bind(usuario_id)
        .bind(participacion)
        .bind(tienda_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        if fila.is_none() {
            return Err(ApiError::conflicto_codigo(
                "La tienda ya tiene un vendedor asignado.",
                "TIENDA_YA_ASIGNADA",
                serde_json::json!({ "tienda_id": tienda_id }),
            ));
        }
        Ok(())
    }

    /// Compensación: libera la tienda si la transacción del llamador falla.
    pub async fn desasignar_vendedor(
        &self,
        tienda_id: &str,
        usuario_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        sqlx::query(
            "UPDATE stores.tiendas
                SET vendedor_id = NULL, participacion_porcentaje = NULL, actualizado_en = NOW()
              WHERE id = $1::uuid AND vendedor_id = $2::uuid",
        )
        .bind(tienda_id)
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        Ok(())
    }
}
