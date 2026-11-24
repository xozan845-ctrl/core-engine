use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    /// Cierra el tenant y registra una solicitud de eliminación con retención.
    pub async fn solicitar_eliminacion(
        &self,
        empresa_id: &str,
        motivo: Option<&str>,
        retencion_dias: i32,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !(0..=3650).contains(&retencion_dias) {
            return Err(ApiError::dominio(
                "RETENCION_INVALIDA",
                "La retención debe estar entre 0 y 3650 días.",
            ));
        }
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let estado: Option<String> = sqlx::query_scalar(
            "SELECT estado FROM identity.empresas WHERE id = $1::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(estado) = estado else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };
        let pendiente: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado = 'pendiente' FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if pendiente.is_some() {
            return Err(ApiError::conflicto(
                "Ya existe una solicitud de eliminación pendiente para esta empresa.",
            ));
        }
        if estado != "inactiva" {
            sqlx::query("UPDATE identity.empresas SET estado = 'inactiva' WHERE id = $1::uuid")
                .bind(empresa_id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            revocar_sesiones_tenant(&mut tx, empresa_id).await?;
            auditoria::registrar(
                &mut tx,
                actor_id,
                empresa_id,
                None,
                "empresa.estado",
                "empresa",
                Some(empresa_id),
                json!({
                    "anterior": estado,
                    "nuevo": "inactiva",
                    "motivo": "eliminacion_solicitada",
                }),
            )
            .await?;
        }
        let fila = sqlx::query(
            "INSERT INTO identity.solicitudes_eliminacion_tenant AS s
               (empresa_id, motivo, retencion_hasta, solicitado_por)
             VALUES ($1::uuid, $2, CURRENT_DATE + $3::int, $4::uuid)
             RETURNING to_jsonb(s) AS datos,
                       (retencion_hasta <= CURRENT_DATE) AS purgable",
        )
        .bind(empresa_id)
        .bind(motivo.unwrap_or(""))
        .bind(retencion_dias)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let purgable: bool = fila.try_get("purgable").map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_solicitada",
            "empresa",
            Some(empresa_id),
            json!({ "retencion_dias": retencion_dias, "purgable": purgable }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "solicitud": datos, "purgable": purgable }))
    }

    pub async fn consultar_eliminacion(&self, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(empresa_id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(s) AS datos,
                    (s.estado = 'pendiente' AND s.retencion_hasta <= CURRENT_DATE) AS purgable
               FROM identity.solicitudes_eliminacion_tenant s
              WHERE s.empresa_id = $1::uuid
              ORDER BY s.creado_en DESC
              LIMIT 1",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                let purgable: bool = fila.try_get("purgable").map_err(interno)?;
                Ok(Some(json!({ "solicitud": datos, "purgable": purgable })))
            }
            None => Ok(None),
        }
    }

    pub async fn cancelar_eliminacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, (retencion_hasta <= CURRENT_DATE) AS vencida
               FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado IN ('pendiente', 'purgada')
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        };
        let vencida: bool = fila.try_get("vencida").map_err(interno)?;
        if vencida {
            return Err(ApiError::conflicto(
                "La retención ya venció; la solicitud no se puede cancelar.",
            ));
        }
        let fila = sqlx::query(
            "UPDATE identity.solicitudes_eliminacion_tenant AS s
                SET estado = 'cancelada', actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND estado = 'pendiente'
              RETURNING to_jsonb(s) AS datos",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_cancelada",
            "empresa",
            Some(empresa_id),
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "solicitud": datos, "purgable": false }))
    }

    /// Anonimiza los datos de identidad del tenant una vez vencida la
    /// retención. Conserva las filas ancla (`empresas`, `usuarios`, `partes`)
    /// para no romper referencias de otros servicios.
    pub async fn purgar_eliminacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, (retencion_hasta <= CURRENT_DATE) AS vencida
               FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado IN ('pendiente', 'purgada')
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        };
        let solicitud_id: Uuid = fila.try_get("id").map_err(interno)?;
        let vencida: bool = fila.try_get("vencida").map_err(interno)?;
        if !vencida {
            return Err(ApiError::conflicto(
                "La retención aún no vence; no se puede purgar la identidad.",
            ));
        }

        let negocio_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM identity.negocios WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let usuario_ids: Vec<String> =
            sqlx::query_scalar("SELECT id::text FROM identity.usuarios WHERE tenant_id = $1::uuid")
                .bind(empresa_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;

        sqlx::query(
            "UPDATE identity.empresas
                SET nombre = 'Empresa eliminada',
                    slug = 'eliminada-' || replace(id::text, '-', ''),
                    estado = 'inactiva',
                    ruc = '', razon_social = '', nombre_comercial = '',
                    descripcion = '', telefono = '', correo_contacto = '',
                    sitio_web = '', direccion = '', ciudad = '',
                    departamento = '', pais = '', moneda = 'NIO',
                    zona_horaria = 'UTC', sector = '',
                    representante_legal = '', horario_atencion = '',
                    logo_objeto_id = NULL,
                    background_objeto_id = NULL,
                    background_media_type = NULL,
                    background_poster_objeto_id = NULL,
                    background_alt = '', background_overlay = 0,
                    background_position = 'center', background_activo = false,
                    plantilla_override = NULL, tema_override = '{}'::jsonb,
                    actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        sqlx::query(
            "UPDATE identity.usuarios
                SET nombre = 'Usuario eliminado',
                    correo = 'eliminado+' || replace(id::text, '-', '')
                             || '@core-engine.invalid',
                    contrasena_hash = '!eliminado',
                    personal_id = NULL,
                    participacion_porcentaje = NULL,
                    avatar_objeto_id = NULL,
                    actualizado_en = NOW()
              WHERE tenant_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        sqlx::query(
            "DELETE FROM identity.sesiones
              WHERE usuario_id IN (
                  SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
              )",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "DELETE FROM identity.password_reset_tokens
              WHERE usuario_id IN (
                  SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
              )",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for tabla in [
            "identity.api_keys",
            "identity.asignaciones_personal",
            "identity.invitaciones_personal",
            "identity.empresa_slugs",
            "identity.exportacion_jobs",
        ] {
            sqlx::query(&format!("DELETE FROM {tabla} WHERE empresa_id = $1::uuid"))
                .bind(empresa_id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
        }

        sqlx::query(
            "UPDATE identity.partes
                SET nombre = 'Parte eliminada', nombre_comercial = '',
                    identificador_valor = '', metadata = '{}'::jsonb,
                    estado = 'archivado', eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.medios_contacto
                SET valor = '', etiqueta = '', metadata = '{}'::jsonb,
                    verificado_en = NULL, eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.direcciones_partes
                SET etiqueta = '', linea_1 = '', linea_2 = '', ciudad = '',
                    departamento = '', pais = '', codigo_postal = '',
                    referencias = '', latitud = NULL, longitud = NULL,
                    metadata = '{}'::jsonb,
                    eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.relaciones_partes
                SET metadata = '{}'::jsonb, activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.partes_registros
                SET titulo = 'Registro eliminado', datos = '{}'::jsonb,
                    adjuntos = '[]'::jsonb,
                    eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.extensiones_empresa_valores
                SET valores = '{}'::jsonb, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.negocios
                SET nombre = 'Negocio eliminado', estado = 'inactivo',
                    archivado_en = COALESCE(archivado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        let fila = sqlx::query(
            "UPDATE identity.solicitudes_eliminacion_tenant AS s
                SET estado = 'purgada', purgado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING to_jsonb(s) AS datos",
        )
        .bind(solicitud_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        // El rastro previo se conserva sin PII; el evento de purga se inserta
        // después y queda como constancia de la operación.
        sqlx::query(
            "UPDATE identity.auditoria
                SET actor_usuario_id = NULL, request_id = NULL, datos = '{}'::jsonb
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_purgada",
            "empresa",
            Some(empresa_id),
            json!({ "solicitud_id": solicitud_id.to_string() }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        let servicios = self
            .purga
            .ejecutar(empresa_id, &negocio_ids, &usuario_ids)
            .await;
        Ok(json!({
            "solicitud": datos,
            "purgable": false,
            "servicios": servicios,
        }))
    }
}
