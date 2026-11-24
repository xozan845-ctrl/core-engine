use super::prelude::*;
use super::*;

impl Fidelizacion {
    pub async fn programa(&self, empresa_id: &str) -> Result<Option<Programa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PROGRAMA} FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref().map(fila_programa).transpose()
    }

    pub async fn guardar_programa(
        &self,
        empresa_id: &str,
        dto: &Value,
    ) -> Result<Programa, ApiError> {
        let nombre = texto(dto, "nombre");
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let activo = dto.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let puntos_por_unidad = entero(dto, "puntos_por_unidad").unwrap_or(1);
        if !(0..=100_000).contains(&puntos_por_unidad) {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos_por_unidad", "restricciones": ["min", "max"]}
            ])));
        }
        let expira_meses = entero(dto, "expira_meses");
        if expira_meses.is_some_and(|meses| meses <= 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "expira_meses", "restricciones": ["min"]}
            ])));
        }
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_programas AS f
               (empresa_id, nombre, activo, puntos_por_unidad, expira_meses, config,
                actualizado_en)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, NOW())
             ON CONFLICT (empresa_id) DO UPDATE
               SET nombre = EXCLUDED.nombre,
                   activo = EXCLUDED.activo,
                   puntos_por_unidad = EXCLUDED.puntos_por_unidad,
                   expira_meses = EXCLUDED.expira_meses,
                   config = EXCLUDED.config,
                   actualizado_en = NOW()
             RETURNING {COLUMNAS_PROGRAMA}"
        ))
        .bind(empresa_id)
        .bind(&nombre)
        .bind(activo)
        .bind(puntos_por_unidad)
        .bind(expira_meses)
        .bind(&config)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_programa(&fila)
    }
}
