use super::*;

impl Comercio {
    pub async fn listar_tipos_cambio(
        &self,
        base: Option<&str>,
        quote: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let base = base.map(normalizar_moneda).transpose()?;
        let quote = quote.map(normalizar_moneda).transpose()?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(t) AS datos FROM finance.tipos_cambio t
              WHERE ($1::text IS NULL OR t.base = $1)
                AND ($2::text IS NULL OR t.quote = $2)
              ORDER BY t.base, t.quote, t.vigente_desde DESC",
        )
        .bind(&base)
        .bind(&quote)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(error_db)?;
                Ok(datos)
            })
            .collect()
    }

    pub async fn tipo_cambio_vigente(
        &self,
        base: &str,
        quote: &str,
    ) -> Result<Option<Value>, ApiError> {
        let base = normalizar_moneda(base)?;
        let quote = normalizar_moneda(quote)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(t) AS datos FROM finance.tipos_cambio t
              WHERE t.base = $1 AND t.quote = $2
              ORDER BY t.vigente_desde DESC LIMIT 1",
        )
        .bind(&base)
        .bind(&quote)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.map(|fila| {
            let datos: Value = fila.try_get("datos").map_err(error_db)?;
            Ok(datos)
        })
        .transpose()
    }

    pub async fn registrar_tipo_cambio(
        &self,
        base: &str,
        quote: &str,
        tasa: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        let base = normalizar_moneda(base)?;
        let quote = normalizar_moneda(quote)?;
        if base == quote {
            return Err(ApiError::validacion(json!([
                {"campo": "quote", "restricciones": ["distintaDeBase"]}
            ])));
        }
        let tasa = tasa.trim();
        let valida = tasa
            .parse::<f64>()
            .ok()
            .is_some_and(|valor| valor.is_finite() && valor > 0.0);
        if !valida {
            return Err(ApiError::validacion(json!([
                {"campo": "tasa", "restricciones": ["numeroPositivo"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.tipos_cambio AS t
               (base, quote, tasa, vigente_desde, creado_por)
             VALUES ($1, $2, $3::numeric, NOW(), $4::uuid)
             RETURNING to_jsonb(t) AS datos",
        )
        .bind(&base)
        .bind(&quote)
        .bind(tasa)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(ApiError::interno)?;
        tx.commit().await.map_err(error_db)?;
        let datos: Value = fila.try_get("datos").map_err(error_db)?;
        Ok(datos)
    }

    // ── consultas y CRUD genérico ───────────────────────────────────────────
}
