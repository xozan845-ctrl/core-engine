use super::prelude::*;
use super::*;

impl Tributacion {
    pub async fn regimenes_de(
        &self,
        jurisdiccion: Option<&str>,
    ) -> Result<Vec<RegimenFiscal>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
                    condicion_ingresos_anuales_cents, estado
             FROM finance.regimenes_fiscales
             WHERE ($1::text IS NULL OR jurisdiccion = $1)
             ORDER BY jurisdiccion, codigo",
        )
        .bind(jurisdiccion)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(regimen_de_fila).collect())
    }

    pub async fn crear_regimen(
        &self,
        datos: CrearRegimen,
    ) -> Result<Option<RegimenFiscal>, ApiError> {
        if datos.jurisdiccion.trim().len() < 2 {
            return Err(ApiError::validacion(json!([
                {"campo": "jurisdiccion", "restricciones": ["minLength:2"]}
            ])));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.regimenes_fiscales
               (jurisdiccion, codigo, nombre, descripcion, periodicidad,
                condicion_ingresos_anuales_cents)
             VALUES ($1, $2, $3, COALESCE($4, ''), COALESCE($5, 'MENSUAL'), $6)
             RETURNING id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
                       condicion_ingresos_anuales_cents, estado",
        )
        .bind(datos.jurisdiccion.to_uppercase())
        .bind(datos.codigo)
        .bind(datos.nombre)
        .bind(datos.descripcion)
        .bind(datos.periodicidad)
        .bind(
            datos
                .condicion_ingresos_anuales_cents
                .map(|valor| valor as i32),
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(regimen_de_fila))
    }

    // ── sujetos fiscales ───────────────────────────────────────────────────

    pub async fn sujetos(&self) -> Result<Vec<SujetoTributario>, ApiError> {
        let filas = sqlx::query(
            "SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
                    s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
             FROM finance.sujetos_fiscales s
             LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
             ORDER BY s.creado_en DESC LIMIT 300",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(sujeto_de_fila).collect())
    }

    pub async fn sujeto_fiscal_de(
        &self,
        usuario_id: &str,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(usuario_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El usuario debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
                    s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
             FROM finance.sujetos_fiscales s
             LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
             WHERE s.id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    pub async fn registrar_sujeto(
        &self,
        datos: RegistrarSujeto,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(&datos.id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El sujeto debe ser un UUID."))?;
        if datos.razon_social.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "razon_social", "restricciones": ["isNotEmpty"]}
            ])));
        }
        let jurisdiccion = match datos.jurisdiccion {
            Some(jurisdiccion) => jurisdiccion,
            None => self
                .jurisdiccion_activa()
                .await?
                .map(|jurisdiccion| jurisdiccion.codigo_pais)
                .ok_or_else(|| {
                    ApiError::dominio(
                        "SIN_JURISDICCION",
                        "No hay jurisdiccion activa configurada.",
                    )
                })?,
        };
        let regimen =
            match datos.regimen_id {
                Some(regimen) => Some(uuid::Uuid::parse_str(&regimen).map_err(|_| {
                    ApiError::dominio("ID_INVALIDO", "El regimen debe ser un UUID.")
                })?),
                None => None,
            };
        let fila = sqlx::query(
            "INSERT INTO finance.sujetos_fiscales
               (id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma)
             VALUES ($1, $2, $3, $4, $5, COALESCE($6, FALSE))
             RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma,
                       estado, creado_en",
        )
        .bind(uuid)
        .bind(jurisdiccion)
        .bind(regimen)
        .bind(datos.razon_social)
        .bind(datos.ruc)
        .bind(datos.es_plataforma.unwrap_or(false))
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    pub async fn dar_de_baja(
        &self,
        usuario_id: &str,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(usuario_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El usuario debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.sujetos_fiscales SET estado = 'BAJA'
             WHERE id = $1 AND estado = 'ACTIVO'
             RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma,
                       estado, creado_en",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    // ── declaraciones ──────────────────────────────────────────────────────
}
