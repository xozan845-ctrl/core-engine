use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    /// Exporta los datos que posee identity para el tenant. Los datos de otros
    /// servicios se listan como pendientes en `alcance` para no fingir una
    /// exportación completa.
    pub async fn exportar(&self, empresa_id: &str, actor_id: &str) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let empresa: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e WHERE e.id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(empresa) = empresa else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };

        let asignaciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.creado_en), '[]'::jsonb)
               FROM identity.asignaciones_personal a WHERE a.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let usuarios = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                        'id', u.id, 'nombre', u.nombre, 'correo', u.correo,
                        'rol', u.rol, 'tenant_id', u.tenant_id,
                        'participacion_porcentaje', u.participacion_porcentaje,
                        'creado_en', u.creado_en
                    ) ORDER BY u.creado_en), '[]'::jsonb)
               FROM identity.usuarios u WHERE u.tenant_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let negocios = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(n) ORDER BY n.creado_en), '[]'::jsonb)
               FROM identity.negocios n WHERE n.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let partes = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.creado_en), '[]'::jsonb)
               FROM identity.partes p WHERE p.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let contactos = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.creado_en), '[]'::jsonb)
               FROM identity.medios_contacto c WHERE c.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let direcciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(d) ORDER BY d.creado_en), '[]'::jsonb)
               FROM identity.direcciones_partes d WHERE d.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let relaciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.creado_en), '[]'::jsonb)
               FROM identity.relaciones_partes r WHERE r.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let capacidades = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.capacidad_codigo), '[]'::jsonb)
               FROM identity.empresa_capacidades c WHERE c.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let extensiones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY v.creado_en), '[]'::jsonb)
               FROM identity.extensiones_empresa_valores v WHERE v.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let slugs = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY s.creado_en), '[]'::jsonb)
               FROM identity.empresa_slugs s WHERE s.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let auditoria = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(datos ORDER BY creado_en DESC), '[]'::jsonb)
               FROM (
                   SELECT to_jsonb(a) AS datos, a.creado_en
                     FROM identity.auditoria a
                    WHERE a.empresa_id = $1::uuid
                    ORDER BY a.creado_en DESC
                    LIMIT 1000
               ) reciente",
            empresa_id,
        )
        .await?;

        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.export",
            "empresa",
            Some(empresa_id),
            json!({ "formato": "core-engine.tenant-export.v1" }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;

        Ok(json!({
            "formato": "core-engine.tenant-export.v1",
            "generado_en": crate::db::iso_ms(Utc::now()),
            "empresa": Self::con_urls(&empresa),
            "asignaciones_personal": asignaciones,
            "usuarios": usuarios,
            "negocios": negocios,
            "partes": partes,
            "contactos": contactos,
            "direcciones": direcciones,
            "relaciones": relaciones,
            "capacidades": capacidades,
            "extensiones_valores": extensiones,
            "slugs": slugs,
            "auditoria": auditoria,
            "alcance": {
                "incluye": ["identity"],
                "pendiente": [
                    "orders", "catalog", "inventory", "pricing",
                    "finance", "logistics", "analytics"
                ]
            }
        }))
    }

    /// Crea un job de exportación asíncrona (identidad + servicios).
    pub async fn iniciar_exportacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let existe: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM identity.empresas WHERE id = $1::uuid")
                .bind(empresa_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.exportacion_jobs (empresa_id, solicitado_por)
             VALUES ($1::uuid, $2::uuid)
             RETURNING id, estado, formato, creado_en",
        )
        .bind(empresa_id)
        .bind(actor_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)?;
        let job_id: Uuid = fila.try_get("id").map_err(interno)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.export_solicitada",
            "empresa",
            Some(empresa_id),
            json!({ "job_id": job_id.to_string() }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({
            "job_id": job_id.to_string(),
            "estado": fila.try_get::<String, _>("estado").map_err(interno)?,
            "formato": fila.try_get::<String, _>("formato").map_err(interno)?,
            "creado_en": crate::db::iso_ms(fila.try_get::<chrono::DateTime<Utc>, _>("creado_en").map_err(interno)?),
        }))
    }

    pub async fn estado_exportacion(
        &self,
        empresa_id: &str,
        job_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) || !es_uuid(job_id) {
            return Err(ApiError::no_encontrado("Exportación", job_id));
        }
        let datos: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(j) - 'contenido' AS datos
               FROM identity.exportacion_jobs j
              WHERE j.id = $1::uuid AND j.empresa_id = $2::uuid",
        )
        .bind(job_id)
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        datos.ok_or_else(|| ApiError::no_encontrado("Exportación", job_id))
    }

    /// Devuelve el archivo firmado de un job completado.
    pub async fn descargar_exportacion(
        &self,
        job_id: &str,
    ) -> Result<DescargaExportacion, ApiError> {
        if !es_uuid(job_id) {
            return Err(ApiError::no_encontrado("Exportación", job_id));
        }
        let fila = sqlx::query(
            "SELECT empresa_id, formato, contenido, sha256, firma, estado
               FROM identity.exportacion_jobs WHERE id = $1::uuid",
        )
        .bind(job_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Exportación", job_id))?;
        let estado: String = fila.try_get("estado").map_err(interno)?;
        if estado != "completado" {
            return Err(ApiError::conflicto(
                "La exportación aún no está completada.",
            ));
        }
        let contenido: Option<Vec<u8>> = fila.try_get("contenido").map_err(interno)?;
        let contenido = contenido.ok_or_else(ApiError::interno)?;
        Ok(DescargaExportacion {
            empresa_id: fila
                .try_get::<Uuid, _>("empresa_id")
                .map_err(interno)?
                .to_string(),
            formato: fila.try_get("formato").map_err(interno)?,
            sha256: fila.try_get("sha256").map_err(interno)?,
            firma: fila.try_get("firma").map_err(interno)?,
            contenido,
        })
    }

    /// Reanuda jobs que quedaron pendientes o en proceso tras un reinicio.
    pub async fn reanudar_exportaciones(&self) -> Result<Vec<String>, ApiError> {
        sqlx::query(
            "UPDATE identity.exportacion_jobs
                SET estado = 'pendiente', iniciado_en = NULL
              WHERE estado = 'en_proceso'",
        )
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        sqlx::query_scalar(
            "SELECT id::text FROM identity.exportacion_jobs
              WHERE estado = 'pendiente' ORDER BY creado_en LIMIT 50",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)
    }

    /// Ejecuta el job en segundo plano. Es idempotente: un job completado no
    /// se vuelve a generar.
    pub async fn ejecutar_exportacion(&self, job_id: &str) -> Result<(), ApiError> {
        if !es_uuid(job_id) {
            return Ok(());
        }
        let fila = match sqlx::query(
            "SELECT empresa_id, solicitado_por, estado
               FROM identity.exportacion_jobs WHERE id = $1::uuid",
        )
        .bind(job_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        {
            Some(fila) => fila,
            None => return Ok(()),
        };
        let estado: String = fila.try_get("estado").map_err(interno)?;
        if estado == "completado" || estado == "en_proceso" {
            return Ok(());
        }
        let empresa_id: Uuid = fila.try_get("empresa_id").map_err(interno)?;
        let actor_id: Option<Uuid> = fila.try_get("solicitado_por").map_err(interno)?;
        let actor_id = actor_id
            .map(|valor| valor.to_string())
            .unwrap_or_else(|| "sistema".to_string());
        let actualizado = sqlx::query(
            "UPDATE identity.exportacion_jobs
                SET estado = 'en_proceso', iniciado_en = NOW(), error = NULL
              WHERE id = $1::uuid AND estado IN ('pendiente', 'fallido')",
        )
        .bind(job_id)
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        if actualizado.rows_affected() == 0 {
            return Ok(());
        }
        let empresa = empresa_id.to_string();
        let negocio_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM identity.negocios WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)?;
        let usuario_ids: Vec<String> =
            sqlx::query_scalar("SELECT id::text FROM identity.usuarios WHERE tenant_id = $1::uuid")
                .bind(empresa_id)
                .fetch_all(self.db.pool())
                .await
                .map_err(interno)?;

        let resultado: Result<(), ApiError> = async {
            let identidad = self.exportar(&empresa, &actor_id).await?;
            let servicios = self
                .purga
                .exportar(&empresa, &negocio_ids, &usuario_ids)
                .await;
            let cuerpo = json!({
                "formato": "core-engine.tenant-export.v2",
                "generado_en": crate::db::iso_ms(Utc::now()),
                "empresa_id": empresa,
                "identidad": identidad,
                "servicios": servicios,
            });
            let bytes = serde_json::to_vec(&cuerpo).map_err(|_| ApiError::interno())?;
            let sha = sha256_hex(&bytes);
            let firma = hmac_sha256_hex(self.firma_clave.as_bytes(), &bytes);
            let servicios_estado: Vec<Value> = cuerpo
                .get("servicios")
                .and_then(Value::as_array)
                .map(|valores| {
                    valores
                        .iter()
                        .map(|valor| {
                            json!({
                                "servicio": valor.get("servicio"),
                                "estado": valor.get("estado"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            sqlx::query(
                "UPDATE identity.exportacion_jobs
                    SET estado = 'completado', contenido = $2, tamano_bytes = $3,
                        sha256 = $4, firma = $5, servicios = $6::jsonb,
                        completado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(job_id)
            .bind(&bytes)
            .bind(bytes.len() as i64)
            .bind(&sha)
            .bind(&firma)
            .bind(sqlx::types::Json(&servicios_estado))
            .execute(self.db.pool())
            .await
            .map_err(interno)?;
            Ok(())
        }
        .await;
        if let Err(error) = resultado {
            let _ = sqlx::query(
                "UPDATE identity.exportacion_jobs
                    SET estado = 'fallido', error = $2, completado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(job_id)
            .bind(&error.mensaje)
            .execute(self.db.pool())
            .await;
            return Err(error);
        }
        Ok(())
    }
}
