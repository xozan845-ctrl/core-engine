use super::prelude::*;
use super::*;

impl Tributacion {
    pub async fn jurisdiccion_activa(&self) -> Result<Option<Jurisdiccion>, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_JURISDICCION}
               FROM finance.jurisdicciones WHERE activa
              ORDER BY codigo_pais LIMIT 1"
        ))
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(jurisdiccion_de_fila))
    }

    /// Jurisdicción efectiva del tenant: negocio → empresa → primera activa
    /// (solo como compatibilidad para datos legacy sin configuración).
    pub async fn jurisdiccion_de(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<Jurisdiccion>, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {}
               FROM finance.configuracion_fiscal c
               JOIN finance.jurisdicciones j ON j.codigo_pais = c.jurisdiccion
              WHERE c.empresa_id = $1::uuid
                AND (c.negocio_id = $2::uuid OR c.negocio_id IS NULL)
              ORDER BY (c.negocio_id IS NOT NULL) DESC
              LIMIT 1",
            columnas_jurisdiccion("j")
        ))
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some(fila) = fila {
            return Ok(Some(jurisdiccion_de_fila(&fila)));
        }

        // La selección del onboarding vive en identity; finance la usa como
        // fallback cuando no hay override explícito de negocio/empresa. No se
        // exige `activa`: desactivar una región impide elegirla de nuevo, pero
        // no debe cambiar el régimen de un tenant existente en silencio.
        let region = sqlx::query(&format!(
            "SELECT {}
               FROM identity.empresas e
               JOIN finance.jurisdicciones j ON j.codigo_pais = e.region_tributaria
              WHERE e.id = $1::uuid",
            columnas_jurisdiccion("j")
        ))
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        match region {
            Some(fila) => Ok(Some(jurisdiccion_de_fila(&fila))),
            None => self.jurisdiccion_activa().await,
        }
    }

    pub async fn listar_jurisdicciones(&self) -> Result<Vec<Jurisdiccion>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_JURISDICCION}
               FROM finance.jurisdicciones ORDER BY codigo_pais"
        ))
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(jurisdiccion_de_fila).collect())
    }

    pub async fn actualizar_jurisdiccion(
        &self,
        codigo: &str,
        dto: ActualizarJurisdiccion,
    ) -> Result<Jurisdiccion, ApiError> {
        let codigo = codigo.trim().to_uppercase();
        let nueva_tasa_iva = dto.tasa_iva_por_mil;
        for (campo, valor) in [
            ("tasa_iva_por_mil", dto.tasa_iva_por_mil),
            ("tasa_ir_por_mil", dto.tasa_ir_por_mil),
            ("tasa_retencion_por_mil", dto.tasa_retencion_por_mil),
        ] {
            if valor.is_some_and(|valor| !(0..=1000).contains(&valor)) {
                return Err(ApiError::validacion(json!([
                    {"campo": campo, "restricciones": ["min:0", "max:1000"]}
                ])));
            }
        }
        if let Some(default) = dto.tasa_default_codigo.as_deref() {
            let existe = sqlx::query(
                "SELECT 1 FROM finance.tasas_impuesto
                  WHERE jurisdiccion = $1 AND codigo = $2 AND activa",
            )
            .bind(&codigo)
            .bind(default.trim().to_uppercase())
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            if existe.is_none() {
                return Err(ApiError::dominio(
                    "TASA_DEFAULT_INEXISTENTE",
                    "La tasa predeterminada no existe o está inactiva.",
                ));
            }
        }
        let fila = sqlx::query(&format!(
            "UPDATE finance.jurisdicciones
                SET nombre = COALESCE($2, nombre),
                    activa = COALESCE($3, activa),
                    tasa_iva = COALESCE($4, tasa_iva),
                    tasa_ir = COALESCE($5, tasa_ir),
                    tasa_default_codigo = COALESCE($6, tasa_default_codigo),
                    retencion_activa = COALESCE($7, retencion_activa),
                    tasa_retencion = COALESCE($8, tasa_retencion),
                    retencion_codigo = COALESCE($9, retencion_codigo),
                    locale = COALESCE($10, locale),
                    zona_horaria = COALESCE($11, zona_horaria),
                    documento_fiscal = COALESCE($12::jsonb, documento_fiscal),
                    perfil = COALESCE($13::jsonb, perfil),
                    leyes = COALESCE($14::jsonb, leyes)
              WHERE codigo_pais = $1
              RETURNING {COLUMNAS_JURISDICCION}"
        ))
        .bind(&codigo)
        .bind(dto.nombre)
        .bind(dto.activa)
        .bind(dto.tasa_iva_por_mil.map(|valor| valor as f64 / 1000.0))
        .bind(dto.tasa_ir_por_mil.map(|valor| valor as f64 / 1000.0))
        .bind(
            dto.tasa_default_codigo
                .map(|valor| valor.trim().to_uppercase()),
        )
        .bind(dto.retencion_activa)
        .bind(
            dto.tasa_retencion_por_mil
                .map(|valor| valor as f64 / 1000.0),
        )
        .bind(dto.retencion_codigo)
        .bind(dto.locale)
        .bind(dto.zona_horaria)
        .bind(dto.documento_fiscal.map(sqlx::types::Json))
        .bind(dto.perfil.map(sqlx::types::Json))
        .bind(dto.leyes.map(sqlx::types::Json))
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Jurisdiccion", &codigo))?;
        if let Some(valor) = nueva_tasa_iva {
            sqlx::query(
                "UPDATE finance.tasas_impuesto
                    SET tasa = $2, actualizado_en = NOW()
                  WHERE jurisdiccion = $1 AND codigo = 'GENERAL'",
            )
            .bind(&codigo)
            .bind(valor as f64 / 1000.0)
            .execute(self.db.pool())
            .await
            .map_err(error_db)?;
        }
        Ok(jurisdiccion_de_fila(&fila))
    }

    /// Tasas activas de una región (o de todas si no se filtra).
    pub async fn tasas_de(
        &self,
        jurisdiccion: Option<&str>,
    ) -> Result<Vec<TasaImpuesto>, ApiError> {
        let filas = sqlx::query(
            "SELECT codigo, nombre, tasa, exenta, activa
               FROM finance.tasas_impuesto
              WHERE ($1::text IS NULL OR jurisdiccion = $1)
                AND activa = true
              ORDER BY tasa DESC, codigo",
        )
        .bind(jurisdiccion)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(tasa_de_fila).collect())
    }

    /// Reemplaza/actualiza el catálogo de tasas de una región.
    ///
    /// No borra tasas: las ausentes quedan inactivas para conservar el
    /// histórico de comprobantes. `GENERAL` es obligatoria y el código default
    /// debe seguir existiendo.
    pub async fn guardar_tasas(
        &self,
        jurisdiccion: &str,
        tasas: Vec<CrearTasaImpuesto>,
    ) -> Result<Vec<TasaImpuesto>, ApiError> {
        let codigo_region = jurisdiccion.trim().to_uppercase();
        if codigo_region.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "jurisdiccion", "restricciones": ["isString"]}
            ])));
        }
        let region = sqlx::query(
            "SELECT tasa_default_codigo FROM finance.jurisdicciones
              WHERE codigo_pais = $1",
        )
        .bind(&codigo_region)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Jurisdiccion", &codigo_region))?;
        let default_actual: String = region
            .try_get("tasa_default_codigo")
            .unwrap_or_else(|_| "GENERAL".to_string());

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let mut activos = std::collections::HashSet::new();
        let mut general_tasa = None;
        for tasa in &tasas {
            let codigo = tasa.codigo.trim().to_uppercase();
            if codigo.is_empty() {
                return Err(ApiError::validacion(json!([
                    {"campo": "codigo", "restricciones": ["isString"]}
                ])));
            }
            if !codigo.chars().all(|caracter| {
                caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '_'
            }) {
                return Err(ApiError::validacion(json!([
                    {"campo": "codigo", "restricciones": ["matches:^[A-Z][A-Z0-9_]{1,39}$"]}
                ])));
            }
            if !(0..=1000).contains(&tasa.tasa_por_mil) {
                return Err(ApiError::validacion(json!([
                    {"campo": "tasa_por_mil", "restricciones": ["min:0", "max:1000"]}
                ])));
            }
            if codigo == "GENERAL" && tasa.activa == Some(false) {
                return Err(ApiError::dominio(
                    "TASA_GENERAL_REQUERIDA",
                    "La tasa GENERAL no puede desactivarse.",
                ));
            }
            let exenta = tasa.exenta || tasa.tasa_por_mil == 0;
            if exenta && tasa.tasa_por_mil != 0 {
                return Err(ApiError::dominio(
                    "TASA_EXENTA_INVALIDA",
                    "Una tasa exenta debe tener 0 por mil.",
                ));
            }
            let nombre = tasa.nombre.trim();
            if !(2..=120).contains(&nombre.chars().count()) {
                return Err(ApiError::validacion(json!([
                    {"campo": "nombre", "restricciones": ["isLength:2,120"]}
                ])));
            }
            sqlx::query(
                "INSERT INTO finance.tasas_impuesto AS t
                   (jurisdiccion, codigo, nombre, tasa, exenta, activa, actualizado_en)
                 VALUES ($1, $2, $3, $4, $5, COALESCE($6, true), NOW())
                 ON CONFLICT (jurisdiccion, codigo) DO UPDATE
                   SET nombre = EXCLUDED.nombre,
                       tasa = EXCLUDED.tasa,
                       exenta = EXCLUDED.exenta,
                       activa = EXCLUDED.activa,
                       actualizado_en = NOW()",
            )
            .bind(&codigo_region)
            .bind(&codigo)
            .bind(nombre)
            .bind(tasa.tasa_por_mil as f64 / 1000.0)
            .bind(exenta)
            .bind(tasa.activa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            if codigo == "GENERAL" {
                general_tasa = Some(tasa.tasa_por_mil as f64 / 1000.0);
            }
            if tasa.activa.unwrap_or(true) {
                activos.insert(codigo);
            }
        }
        if !activos.contains("GENERAL") {
            return Err(ApiError::dominio(
                "TASA_GENERAL_REQUERIDA",
                "El catálogo debe incluir la tasa GENERAL activa.",
            ));
        }
        let codigos_activos: Vec<String> = activos.iter().cloned().collect();
        sqlx::query(
            "UPDATE finance.tasas_impuesto
                SET activa = false, actualizado_en = NOW()
              WHERE jurisdiccion = $1 AND NOT (codigo = ANY($2))",
        )
        .bind(&codigo_region)
        .bind(&codigos_activos)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        let default_nuevo = if activos.contains(default_actual.as_str()) {
            default_actual
        } else {
            "GENERAL".to_string()
        };
        sqlx::query(
            "UPDATE finance.jurisdicciones
                SET tasa_default_codigo = $2,
                    tasa_iva = COALESCE($3, tasa_iva)
              WHERE codigo_pais = $1",
        )
        .bind(&codigo_region)
        .bind(&default_nuevo)
        .bind(general_tasa)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        self.tasas_de(Some(&codigo_region)).await
    }

    pub async fn crear_jurisdiccion(
        &self,
        datos: CrearJurisdiccion,
    ) -> Result<Jurisdiccion, ApiError> {
        if datos.codigo_pais.trim().len() < 2 {
            return Err(ApiError::validacion(json!([
                {"campo": "codigo_pais", "restricciones": ["minLength:2"]}
            ])));
        }
        if datos.tasa_iva_por_mil < 0 || datos.tasa_ir_por_mil < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "tasa_iva_por_mil", "restricciones": ["min:0"]},
                {"campo": "tasa_ir_por_mil", "restricciones": ["min:0"]}
            ])));
        }
        if let Some(codigo) = datos.tasa_default_codigo.as_deref() {
            if !codigo.trim().eq_ignore_ascii_case("GENERAL") {
                return Err(ApiError::dominio(
                    "TASA_DEFAULT_INVALIDA",
                    "Al crear una región la tasa predeterminada debe ser GENERAL; cámbiala después de configurar el catálogo.",
                ));
            }
        }
        let fila = sqlx::query(&format!(
            "INSERT INTO finance.jurisdicciones
               (codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                periodicidad_declaracion, leyes, pais_iso, subdivision, moneda_iso,
                locale, zona_horaria, documento_fiscal, perfil,
                tasa_default_codigo, retencion_activa, tasa_retencion,
                retencion_codigo)
             VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'MENSUAL'),
                     COALESCE($8::jsonb, '{{}}'), $9, $10, $11,
                     COALESCE($12, 'es'), COALESCE($13, 'UTC'),
                     COALESCE($14::jsonb, '{{}}'::jsonb),
                     COALESCE($15::jsonb, '{{}}'::jsonb),
                     COALESCE($16, 'GENERAL'), COALESCE($17, false),
                     COALESCE($18, 0), COALESCE($19, 'IR'))
             RETURNING {COLUMNAS_JURISDICCION}"
        ))
        .bind(datos.codigo_pais.to_uppercase())
        .bind(datos.nombre)
        .bind(datos.moneda)
        .bind(datos.simbolo_moneda)
        .bind(datos.tasa_iva_por_mil as f64 / 1000.0)
        .bind(datos.tasa_ir_por_mil as f64 / 1000.0)
        .bind(datos.periodicidad_declaracion)
        .bind(datos.leyes.map(sqlx::types::Json))
        .bind(datos.pais_iso)
        .bind(datos.subdivision)
        .bind(datos.moneda_iso)
        .bind(datos.locale)
        .bind(datos.zona_horaria)
        .bind(datos.documento_fiscal.map(sqlx::types::Json))
        .bind(datos.perfil.map(sqlx::types::Json))
        .bind(datos.tasa_default_codigo)
        .bind(datos.retencion_activa)
        .bind(
            datos
                .tasa_retencion_por_mil
                .map(|valor| valor as f64 / 1000.0),
        )
        .bind(datos.retencion_codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| {
            if es_violacion_unicidad(&error) {
                ApiError::conflicto("La jurisdiccion ya existe.")
            } else {
                error_db(error)
            }
        })?
        .ok_or_else(|| ApiError::conflicto("La jurisdiccion ya existe."))?;
        let creada = jurisdiccion_de_fila(&fila);
        sqlx::query(
            "INSERT INTO finance.tasas_impuesto
               (jurisdiccion, codigo, nombre, tasa, exenta)
             VALUES ($1, 'GENERAL', 'Tasa general', $2, false),
                    ($1, 'EXENTA', 'Exento', 0, true)
             ON CONFLICT (jurisdiccion, codigo) DO NOTHING",
        )
        .bind(&creada.codigo_pais)
        .bind(creada.tasa_iva)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(creada)
    }

    // ── regímenes ──────────────────────────────────────────────────────────
}
