use super::prelude::*;
use super::*;

impl Tributacion {
    pub async fn guardar_configuracion_fiscal(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        jurisdiccion: &str,
    ) -> Result<Jurisdiccion, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "Empresa inválida."))?;
        let negocio = negocio_id
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::dominio("NEGOCIO_INVALIDO", "Negocio inválido."))?;
        let codigo = jurisdiccion.trim().to_uppercase();
        let existe = sqlx::query(
            "SELECT codigo_pais FROM finance.jurisdicciones
              WHERE codigo_pais = $1 AND activa",
        )
        .bind(&codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado("Jurisdiccion", &codigo));
        }
        match negocio {
            Some(negocio) => {
                sqlx::query(
                    "INSERT INTO finance.configuracion_fiscal
                       (empresa_id, negocio_id, jurisdiccion, actualizado_en)
                     VALUES ($1, $2, $3, NOW())
                     ON CONFLICT (empresa_id, negocio_id) WHERE negocio_id IS NOT NULL
                     DO UPDATE SET jurisdiccion = EXCLUDED.jurisdiccion,
                                   actualizado_en = NOW()",
                )
                .bind(empresa)
                .bind(negocio)
                .bind(&codigo)
                .execute(self.db.pool())
                .await
                .map_err(error_db)?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO finance.configuracion_fiscal
                       (empresa_id, negocio_id, jurisdiccion, actualizado_en)
                     VALUES ($1, NULL, $2, NOW())
                     ON CONFLICT (empresa_id) WHERE negocio_id IS NULL
                     DO UPDATE SET jurisdiccion = EXCLUDED.jurisdiccion,
                                   actualizado_en = NOW()",
                )
                .bind(empresa)
                .bind(&codigo)
                .execute(self.db.pool())
                .await
                .map_err(error_db)?;
            }
        }
        self.jurisdiccion_de(empresa_id, negocio_id)
            .await?
            .ok_or_else(ApiError::interno)
    }

    pub async fn configuracion_facturacion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<ConfiguracionFacturacion, ApiError> {
        let fila = sqlx::query(
            "SELECT jurisdiccion, facturacion_electronica, formato_pdf,
                    proveedor_electronico, ambiente_electronico,
                    impuesto_activo, precios_con_impuesto
               FROM finance.configuracion_fiscal
              WHERE empresa_id = $1::uuid
                AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let jurisdiccion = self
            .jurisdiccion_de(empresa_id, negocio_id)
            .await?
            .ok_or_else(|| {
                ApiError::dominio("SIN_JURISDICCION", "No hay jurisdicción configurada.")
            })?;
        let (electronica, pdf, proveedor, ambiente, impuesto_activo, precios_con_impuesto) =
            match fila {
                Some(fila) => (
                    fila.try_get("facturacion_electronica").unwrap_or(false),
                    fila.try_get("formato_pdf").unwrap_or(false),
                    fila.try_get::<String, _>("proveedor_electronico")
                        .unwrap_or_else(|_| "simulado".to_string()),
                    fila.try_get::<String, _>("ambiente_electronico")
                        .unwrap_or_else(|_| "pruebas".to_string()),
                    fila.try_get("impuesto_activo").unwrap_or(false),
                    fila.try_get("precios_con_impuesto").unwrap_or(true),
                ),
                None => (
                    false,
                    false,
                    "simulado".to_string(),
                    "pruebas".to_string(),
                    false,
                    true,
                ),
            };
        let tasas = self.tasas_de(Some(&jurisdiccion.codigo_pais)).await?;
        Ok(ConfiguracionFacturacion {
            tasas,
            retencion_activa: jurisdiccion.retencion_activa,
            tasa_retencion: jurisdiccion.tasa_retencion,
            retencion_codigo: jurisdiccion.retencion_codigo.clone(),
            jurisdiccion,
            facturacion_electronica: electronica,
            formato_pdf: pdf,
            proveedor_electronico: proveedor,
            ambiente_electronico: ambiente,
            impuesto_activo,
            precios_con_impuesto,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn guardar_configuracion_facturacion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        jurisdiccion: &str,
        facturacion_electronica: bool,
        formato_pdf: bool,
        proveedor_electronico: &str,
        ambiente_electronico: &str,
        impuesto_activo: bool,
        precios_con_impuesto: bool,
    ) -> Result<ConfiguracionFacturacion, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "Empresa inválida."))?;
        let negocio = negocio_id
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::dominio("NEGOCIO_INVALIDO", "Negocio inválido."))?;
        let codigo = jurisdiccion.trim().to_uppercase();
        if !["simulado", "http", "dgi"].contains(&proveedor_electronico) {
            return Err(ApiError::dominio(
                "PROVEEDOR_ELECTRONICO_INVALIDO",
                "El proveedor debe ser simulado, http o dgi.",
            ));
        }
        if !["pruebas", "produccion"].contains(&ambiente_electronico) {
            return Err(ApiError::dominio(
                "AMBIENTE_ELECTRONICO_INVALIDO",
                "El ambiente debe ser pruebas o produccion.",
            ));
        }
        let existe = sqlx::query(
            "SELECT codigo_pais FROM finance.jurisdicciones
              WHERE codigo_pais = $1 AND activa",
        )
        .bind(&codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado("Jurisdiccion", &codigo));
        }
        match negocio {
            Some(negocio) => {
                sqlx::query(
                    "INSERT INTO finance.configuracion_fiscal
                       (empresa_id, negocio_id, jurisdiccion, facturacion_electronica,
                        formato_pdf, proveedor_electronico, ambiente_electronico,
                        impuesto_activo, precios_con_impuesto, actualizado_en)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
                     ON CONFLICT (empresa_id, negocio_id) WHERE negocio_id IS NOT NULL
                     DO UPDATE SET jurisdiccion = EXCLUDED.jurisdiccion,
                                   facturacion_electronica = EXCLUDED.facturacion_electronica,
                                   formato_pdf = EXCLUDED.formato_pdf,
                                   proveedor_electronico = EXCLUDED.proveedor_electronico,
                                   ambiente_electronico = EXCLUDED.ambiente_electronico,
                                   impuesto_activo = EXCLUDED.impuesto_activo,
                                   precios_con_impuesto = EXCLUDED.precios_con_impuesto,
                                   actualizado_en = NOW()",
                )
                .bind(empresa)
                .bind(negocio)
                .bind(&codigo)
                .bind(facturacion_electronica)
                .bind(formato_pdf)
                .bind(proveedor_electronico)
                .bind(ambiente_electronico)
                .bind(impuesto_activo)
                .bind(precios_con_impuesto)
                .execute(self.db.pool())
                .await
                .map_err(error_db)?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO finance.configuracion_fiscal
                       (empresa_id, negocio_id, jurisdiccion, facturacion_electronica,
                        formato_pdf, proveedor_electronico, ambiente_electronico,
                        impuesto_activo, precios_con_impuesto, actualizado_en)
                     VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, $8, NOW())
                     ON CONFLICT (empresa_id) WHERE negocio_id IS NULL
                     DO UPDATE SET jurisdiccion = EXCLUDED.jurisdiccion,
                                   facturacion_electronica = EXCLUDED.facturacion_electronica,
                                   formato_pdf = EXCLUDED.formato_pdf,
                                   proveedor_electronico = EXCLUDED.proveedor_electronico,
                                   ambiente_electronico = EXCLUDED.ambiente_electronico,
                                   impuesto_activo = EXCLUDED.impuesto_activo,
                                   precios_con_impuesto = EXCLUDED.precios_con_impuesto,
                                   actualizado_en = NOW()",
                )
                .bind(empresa)
                .bind(&codigo)
                .bind(facturacion_electronica)
                .bind(formato_pdf)
                .bind(proveedor_electronico)
                .bind(ambiente_electronico)
                .bind(impuesto_activo)
                .bind(precios_con_impuesto)
                .execute(self.db.pool())
                .await
                .map_err(error_db)?;
            }
        }
        self.configuracion_facturacion(empresa_id, negocio_id).await
    }
}
