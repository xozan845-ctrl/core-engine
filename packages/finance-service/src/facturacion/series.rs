use super::prelude::*;
use super::*;

impl Facturacion {
    pub async fn series(&self) -> Result<Vec<SerieComprobante>, ApiError> {
        self.series_de(None, None).await
    }

    pub async fn series_de(
        &self,
        negocio_id: Option<&str>,
        empresa_id: Option<&str>,
    ) -> Result<Vec<SerieComprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
             FROM finance.series_comprobantes
              WHERE ($1::uuid IS NULL OR negocio_id IS NULL OR negocio_id = $1::uuid)
                AND ($2::uuid IS NULL OR empresa_id IS NULL OR empresa_id = $2::uuid)
              ORDER BY serie",
        )
        .bind(negocio_id)
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(serie_de_fila).collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn crear_serie(
        &self,
        serie: &str,
        tipo: &str,
        prefijo: &str,
        ruc_emisor: &str,
        jurisdiccion: &str,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
        activa: bool,
    ) -> Result<SerieComprobante, ApiError> {
        for (campo, valor) in [("empresa_id", empresa_id), ("negocio_id", negocio_id)] {
            if let Some(valor) = valor {
                uuid::Uuid::parse_str(valor).map_err(|_| {
                    ApiError::validacion(json!([
                        {"campo": campo, "restricciones": ["isUuid"]}
                    ]))
                })?;
            }
        }
        let serie = serie.trim();
        let tipo = tipo.trim().to_uppercase();
        let prefijo = prefijo.trim();
        let jurisdiccion = jurisdiccion.trim().to_uppercase();
        if !(1..=40).contains(&serie.chars().count())
            || !serie
                .chars()
                .all(|caracter| caracter.is_ascii_alphanumeric() || caracter == '-')
        {
            return Err(ApiError::validacion(json!([
                {"campo": "serie", "restricciones": ["matches:^[A-Za-z0-9-]{1,40}$"]}
            ])));
        }
        if !["FACTURA", "NOTA_CREDITO"].contains(&tipo.as_str()) {
            return Err(ApiError::validacion(json!([
                {"campo": "tipo", "restricciones": ["isIn:FACTURA,NOTA_CREDITO"]}
            ])));
        }
        if prefijo.is_empty() || prefijo.chars().count() > 20 {
            return Err(ApiError::validacion(json!([
                {"campo": "prefijo", "restricciones": ["isLength:1,20"]}
            ])));
        }
        if ruc_emisor.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "ruc_emisor", "restricciones": ["isString"]}
            ])));
        }
        let region =
            sqlx::query("SELECT 1 FROM finance.jurisdicciones WHERE codigo_pais = $1 AND activa")
                .bind(&jurisdiccion)
                .fetch_optional(self.db.pool())
                .await
                .map_err(error_db)?;
        if region.is_none() {
            return Err(ApiError::no_encontrado("Jurisdiccion", &jurisdiccion));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.series_comprobantes
               (serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual,
                ruc_emisor, jurisdiccion, activa)
             VALUES ($1, $2::uuid, $3::uuid, $4, $5, 0, $6, $7, $8)
             RETURNING serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual,
                       ruc_emisor, jurisdiccion, activa",
        )
        .bind(serie)
        .bind(empresa_id)
        .bind(negocio_id)
        .bind(&tipo)
        .bind(prefijo)
        .bind(ruc_emisor.trim())
        .bind(&jurisdiccion)
        .bind(activa)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| {
            if crate::errors::es_violacion_unicidad(&error) {
                ApiError::conflicto("La serie ya existe.")
            } else {
                error_db(error)
            }
        })?;
        Ok(serie_de_fila(&fila))
    }

    pub async fn empresa_de_serie(&self, serie: &str) -> Result<Option<String>, ApiError> {
        let fila = sqlx::query(
            "SELECT empresa_id::text AS empresa_id
               FROM finance.series_comprobantes WHERE serie = $1",
        )
        .bind(serie)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila
            .and_then(|fila| fila.try_get("empresa_id").ok())
            .flatten())
    }

    pub async fn actualizar_serie(
        &self,
        serie: &str,
        activa: Option<bool>,
        prefijo: Option<&str>,
    ) -> Result<SerieComprobante, ApiError> {
        let fila = sqlx::query(
            "UPDATE finance.series_comprobantes
                SET activa = COALESCE($2, activa),
                    prefijo = COALESCE($3, prefijo)
              WHERE serie = $1
              RETURNING serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual,
                        ruc_emisor, jurisdiccion, activa",
        )
        .bind(serie)
        .bind(activa)
        .bind(prefijo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Serie", serie))?;
        Ok(serie_de_fila(&fila))
    }
}
