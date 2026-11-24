use super::comunes::*;
use super::prelude::*;
use super::*;

impl Packs {
    pub async fn crear_modelo(
        &self,
        codigo: &str,
        nombre: &str,
        descripcion: Option<&str>,
        config: Option<Value>,
        activo: Option<bool>,
    ) -> Result<ModeloOperacion, ApiError> {
        let codigo = codigo_strict(codigo, "MODELO_OPERACION_INVALIDO")?;
        let nombre = nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "MODELO_OPERACION_INVALIDO",
                "El nombre del modelo operativo no es válido.",
            ));
        }
        let config = config.unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.modelos_operacion (codigo, nombre, descripcion, config, activo)
             VALUES ($1, $2, $3, $4::jsonb, COALESCE($5, true))
             RETURNING codigo, nombre, descripcion, config, activo",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(descripcion.unwrap_or("").trim())
        .bind(config)
        .bind(activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el modelo operativo."))?;
        tx.commit().await.map_err(interno)?;
        Ok(ModeloOperacion {
            codigo: fila.try_get("codigo").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            descripcion: fila.try_get("descripcion").map_err(interno)?,
            config: fila.try_get("config").map_err(interno)?,
            activo: fila.try_get("activo").map_err(interno)?,
        })
    }

    pub async fn crear_vertical(
        &self,
        codigo: &str,
        nombre: &str,
        modelo_operacion: &str,
        activo: Option<bool>,
    ) -> Result<VerticalOperacion, ApiError> {
        let codigo = codigo_strict(codigo, "VERTICAL_INVALIDA")?;
        let nombre = nombre.trim();
        let modelo = modelo_operacion.trim();
        if !(2..=120).contains(&nombre.chars().count())
            || !(2..=160).contains(&modelo.chars().count())
        {
            return Err(ApiError::dominio(
                "VERTICAL_INVALIDA",
                "Los datos de la vertical no son válidos.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let modelo_valido = sqlx::query(
            "SELECT codigo FROM identity.modelos_operacion WHERE codigo = $1 AND activo = true",
        )
        .bind(modelo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if modelo_valido.is_none() {
            return Err(ApiError::no_encontrado("Modelo operativo", modelo));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.verticales (codigo, nombre, modelo_operacion, activo)
             VALUES ($1, $2, $3, COALESCE($4, true))
             RETURNING codigo, nombre, modelo_operacion, activo",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(modelo)
        .bind(activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la vertical."))?;
        tx.commit().await.map_err(interno)?;
        Ok(VerticalOperacion {
            codigo: fila.try_get("codigo").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            modelo_operacion: fila.try_get("modelo_operacion").map_err(interno)?,
            activo: fila.try_get("activo").map_err(interno)?,
        })
    }

    pub async fn reemplazar_regiones(
        &self,
        codigo: &str,
        version: i32,
        mut regiones: Vec<PackRegionInput>,
    ) -> Result<Pack, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT 1 FROM identity.packs WHERE codigo = $1 AND version = $2 AND activo",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado(
                "Pack",
                &format!("{codigo}@{version}"),
            ));
        }
        if regiones.is_empty() {
            return Err(ApiError::dominio(
                "REGION_NO_DISPONIBLE",
                "El pack debe declarar al menos una región.",
            ));
        }
        if regiones
            .iter()
            .filter(|region| region.predeterminada == Some(true))
            .count()
            > 1
        {
            return Err(ApiError::dominio(
                "REGION_PREDETERMINADA_UNICA",
                "Un pack solo puede tener una región predeterminada.",
            ));
        }
        let mut predeterminada_asignada = regiones
            .iter()
            .any(|region| region.predeterminada == Some(true));
        for (indice, region) in regiones.iter_mut().enumerate() {
            region.codigo = region.codigo.trim().to_uppercase();
            let existe = sqlx::query(
                "SELECT 1 FROM finance.jurisdicciones
                  WHERE codigo_pais = $1 AND activa = true",
            )
            .bind(&region.codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if existe.is_none() {
                return Err(ApiError::no_encontrado("Region tributaria", &region.codigo));
            }
            if !predeterminada_asignada && indice == 0 {
                region.predeterminada = Some(true);
                predeterminada_asignada = true;
            }
        }
        let usadas = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT region_tributaria
               FROM identity.empresas
              WHERE pack_codigo = $1 AND pack_version = $2
                AND region_tributaria IS NOT NULL",
        )
        .bind(codigo)
        .bind(version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let en_uso: Vec<String> = usadas
            .into_iter()
            .filter(|region| {
                !regiones
                    .iter()
                    .any(|nueva| nueva.codigo == *region && nueva.activo.unwrap_or(true))
            })
            .collect();
        if !en_uso.is_empty() {
            return Err(ApiError::dominio(
                "REGION_EN_USO",
                "No se puede retirar una región que ya usan empresas de este pack.",
            )
            .con_detalles(json!({ "regiones": en_uso })));
        }
        sqlx::query(
            "DELETE FROM identity.pack_regiones
              WHERE pack_codigo = $1 AND pack_version = $2",
        )
        .bind(codigo)
        .bind(version)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for region in &regiones {
            sqlx::query(
                "INSERT INTO identity.pack_regiones
                   (pack_codigo, pack_version, region_codigo, predeterminada, activo)
                 VALUES ($1, $2, $3, $4, COALESCE($5, true))",
            )
            .bind(codigo)
            .bind(version)
            .bind(&region.codigo)
            .bind(region.predeterminada.unwrap_or(false))
            .bind(region.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
        }
        tx.commit().await.map_err(interno)?;
        self.obtener(codigo, i64::from(version)).await
    }

    pub async fn crear(&self, input: CrearPack) -> Result<Pack, ApiError> {
        let codigo = input.codigo.trim().to_lowercase();
        let vertical = codigo_strict(&input.vertical_codigo, "VERTICAL_INVALIDA")?;
        if !es_codigo_pack(&codigo) {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "El código del pack debe empezar por pack_.",
            ));
        }
        if input.version < 1 {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "La versión del pack debe ser un entero positivo.",
            ));
        }
        let nombre = input.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "El nombre del pack no es válido.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let vertical_activa =
            sqlx::query("SELECT 1 FROM identity.verticales WHERE codigo = $1 AND activo = true")
                .bind(&vertical)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
        if vertical_activa.is_none() {
            return Err(ApiError::no_encontrado("Vertical", &vertical));
        }
        sqlx::query(
            "INSERT INTO identity.packs (codigo, version, vertical_codigo, nombre, descripcion, config)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb)",
        )
        .bind(&codigo)
        .bind(input.version)
        .bind(&vertical)
        .bind(nombre)
        .bind(input.descripcion.as_deref().unwrap_or("").trim())
        .bind(input.config.unwrap_or_else(|| json!({})))
        .execute(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?;

        for modelo in input.modelos {
            let modelo_codigo = codigo_strict(&modelo.codigo, "MODELO_OPERACION_INVALIDO")?;
            let insertado = sqlx::query(
                "INSERT INTO identity.pack_modelos
                   (pack_codigo, pack_version, modelo_codigo, config, activo)
                 SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
                   FROM identity.modelos_operacion
                  WHERE codigo = $3 AND activo = true",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&modelo_codigo)
            .bind(modelo.config.unwrap_or_else(|| json!({})))
            .bind(modelo.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
            if insertado.rows_affected() == 0 {
                return Err(ApiError::no_encontrado("Modelo operativo", &modelo_codigo));
            }
        }

        for capacidad in input.capacidades {
            let capacidad_codigo = codigo_strict(&capacidad.codigo, "CAPACIDAD_INVALIDA")?;
            let insertada = sqlx::query(
                "INSERT INTO identity.pack_capacidades
                   (pack_codigo, pack_version, capacidad_codigo, config, activo)
                 SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
                   FROM identity.capacidades
                  WHERE codigo = $3 AND activo = true",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&capacidad_codigo)
            .bind(capacidad.config.unwrap_or_else(|| json!({})))
            .bind(capacidad.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
            if insertada.rows_affected() == 0 {
                return Err(ApiError::no_encontrado("Capacidad", &capacidad_codigo));
            }
        }

        sqlx::query(
            "INSERT INTO identity.vertical_capacidades AS vc
               (vertical_codigo, capacidad_codigo, config, activo)
             SELECT $1, pc.capacidad_codigo, pc.config, pc.activo
               FROM identity.pack_capacidades pc
              WHERE pc.pack_codigo = $2 AND pc.pack_version = $3
             ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING",
        )
        .bind(&vertical)
        .bind(&codigo)
        .bind(input.version)
        .execute(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?;

        // Regiones tributarias del pack. Si el cliente no las declara, se
        // asignan las regiones activas de la plataforma (compatibilidad y
        // arranque con un solo país). El admin puede restringirlas después.
        let mut regiones = input.regiones;
        if regiones.is_empty() {
            // Una versión nueva hereda las regiones de la versión anterior.
            // Si es la primera versión, se usan las regiones activas.
            let anteriores = sqlx::query(
                "SELECT region_codigo, predeterminada, activo
                   FROM identity.pack_regiones
                  WHERE pack_codigo = $1 AND pack_version < $2 AND activo = true
                  ORDER BY predeterminada DESC, region_codigo",
            )
            .bind(&codigo)
            .bind(input.version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            if anteriores.is_empty() {
                let filas = sqlx::query(
                    "SELECT codigo_pais FROM finance.jurisdicciones
                      WHERE activa = true ORDER BY codigo_pais",
                )
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;
                regiones = filas
                    .iter()
                    .map(|fila| {
                        Ok(PackRegionInput {
                            codigo: fila.try_get("codigo_pais").map_err(interno)?,
                            predeterminada: None,
                            activo: None,
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
            } else {
                regiones = anteriores
                    .iter()
                    .map(|fila| {
                        Ok(PackRegionInput {
                            codigo: fila.try_get("region_codigo").map_err(interno)?,
                            predeterminada: Some(fila.try_get("predeterminada").map_err(interno)?),
                            activo: Some(fila.try_get("activo").map_err(interno)?),
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
            }
        }
        if regiones.is_empty() {
            return Err(ApiError::dominio(
                "REGION_NO_DISPONIBLE",
                "No hay regiones tributarias activas para asignar al pack.",
            ));
        }
        if regiones
            .iter()
            .filter(|region| region.predeterminada == Some(true))
            .count()
            > 1
        {
            return Err(ApiError::dominio(
                "REGION_PREDETERMINADA_UNICA",
                "Un pack solo puede tener una región predeterminada.",
            ));
        }
        let mut predeterminada_asignada = regiones
            .iter()
            .any(|region| region.predeterminada == Some(true));
        for (indice, region) in regiones.iter().enumerate() {
            let region_codigo = region.codigo.trim().to_uppercase();
            let existe = sqlx::query(
                "SELECT 1 FROM finance.jurisdicciones
                  WHERE codigo_pais = $1 AND activa = true",
            )
            .bind(&region_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if existe.is_none() {
                return Err(ApiError::no_encontrado("Region tributaria", &region_codigo));
            }
            let es_predeterminada =
                region.predeterminada == Some(true) || (!predeterminada_asignada && indice == 0);
            predeterminada_asignada |= es_predeterminada;
            sqlx::query(
                "INSERT INTO identity.pack_regiones
                   (pack_codigo, pack_version, region_codigo, predeterminada, activo)
                 VALUES ($1, $2, $3, $4, COALESCE($5, true))",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&region_codigo)
            .bind(es_predeterminada)
            .bind(region.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
        }

        tx.commit().await.map_err(interno)?;
        self.obtener(&codigo, input.version).await
    }
}
