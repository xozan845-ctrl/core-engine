use super::comunes::*;
use super::prelude::*;
use super::*;
use serde_json::json;

impl Personal {
    pub async fn crear_invitacion(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearInvitacion,
    ) -> Result<Value, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        let correo = validar_correo_invitacion(&datos.correo)?;
        let rol_negocio = self
            .assert_rol(empresa_id, &datos.rol_empresa_id, true)
            .await?;
        if let Some(tienda_id) = datos.tienda_id.as_deref() {
            self.assert_tienda(empresa_id, tienda_id).await?;
        }
        let puesto_negocio = if let Some(puesto_id) = datos.puesto_id.as_deref() {
            self.assert_puesto(empresa_id, puesto_id, datos.tienda_id.as_deref())
                .await?
        } else {
            None
        };
        validar_objeto(datos.metadata.as_ref(), "metadata")?;
        let objetivo = datos
            .negocio_id
            .clone()
            .or_else(|| contexto.negocio_id.clone());
        let negocio_id = self
            .negocios
            .resolver(empresa_id, objetivo.as_deref())
            .await?;
        Self::exigir_gestion(&contexto, negocio_id.as_deref())?;
        validar_alcance_negocio(negocio_id.as_deref(), rol_negocio.as_deref(), "rol")?;
        validar_alcance_negocio(negocio_id.as_deref(), puesto_negocio.as_deref(), "puesto")?;
        let token = token_aleatorio();
        let expira = Utc::now() + Duration::hours(TOKEN_TTL_HORAS);
        let tipo = datos.tipo.as_deref().unwrap_or("invitacion");
        if tipo != "invitacion" && tipo != "contratacion" {
            return Err(ApiError::validacion(json!([
                {"campo": "tipo", "restricciones": ["isIn"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "WITH creada AS (
               INSERT INTO identity.invitaciones_personal
                 (empresa_id, correo, nombre, tipo, rol_empresa_id, tienda_id,
                  puesto_id, negocio_id, metadata, token_hash, expira_en,
                  creada_por_usuario_id)
               VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::uuid, $7::uuid,
                       $8::uuid, COALESCE($9::jsonb, '{}'::jsonb), $10, $11, $12::uuid)
               RETURNING id, empresa_id, correo, nombre, tipo, rol_empresa_id,
                         tienda_id, puesto_id, negocio_id, estado, expira_en
             )
             SELECT to_jsonb(c) || jsonb_build_object(
                      'rol_nombre', r.nombre,
                      'empresa_nombre', e.nombre
                    ) AS datos
               FROM creada c
               JOIN identity.roles_empresa r ON r.id = c.rol_empresa_id
               JOIN identity.empresas e ON e.id = c.empresa_id",
        )
        .bind(empresa_id)
        .bind(&correo)
        .bind(
            datos
                .nombre
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty()),
        )
        .bind(tipo)
        .bind(&datos.rol_empresa_id)
        .bind(datos.tienda_id.as_deref())
        .bind(datos.puesto_id.as_deref())
        .bind(negocio_id.as_deref())
        .bind(datos.metadata.clone())
        .bind(hash_token(&token))
        .bind(expira)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            negocio_id.as_deref(),
            "invitacion.creada",
            "invitacion_personal",
            None,
            json!({
                "correo": correo,
                "rol_empresa_id": datos.rol_empresa_id,
                "tipo": tipo,
            }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        let fila = fila
            .ok_or_else(|| ApiError::conflicto("No se pudo generar el enlace de incorporación."))?;
        let mut datos_invitacion: Value = fila.try_get("datos").map_err(interno)?;
        let url = format!("{}/incorporacion/{token}", self.frontend_url);
        self.enviar_invitacion(&datos_invitacion, &url).await;
        if let Some(objeto) = datos_invitacion.as_object_mut() {
            objeto.insert("url_invitacion".into(), json!(url));
        }
        Ok(normalizar_fechas(datos_invitacion, &["expira_en"]))
    }

    pub async fn listar_invitaciones(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        Self::exigir_lectura(&contexto)?;
        let empresa_global = contexto.empresa_global;
        let negocio_id = contexto.negocio_id.clone();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(i) || jsonb_build_object('rol_nombre', r.nombre) AS datos
               FROM identity.invitaciones_personal i
               JOIN identity.roles_empresa r ON r.id = i.rol_empresa_id
              WHERE i.empresa_id = $1::uuid
                AND ($2::boolean
                     OR i.negocio_id IS NULL
                     OR ($3::uuid IS NOT NULL AND i.negocio_id = $3::uuid))
              ORDER BY i.creada_en DESC",
        )
        .bind(empresa_id)
        .bind(empresa_global)
        .bind(negocio_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(datos, &["expira_en", "creada_en"]))
            })
            .collect()
    }

    pub async fn revocar_invitacion(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        invitacion_id: &str,
    ) -> Result<Value, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let invitacion = sqlx::query(
            "SELECT negocio_id FROM identity.invitaciones_personal
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'pendiente'",
        )
        .bind(invitacion_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Invitación", invitacion_id))?;
        let negocio_id: Option<Uuid> = invitacion.try_get("negocio_id").map_err(interno)?;
        Self::exigir_gestion(&contexto, negocio_id.map(|id| id.to_string()).as_deref())?;
        let fila = sqlx::query(
            "UPDATE identity.invitaciones_personal
                SET estado = 'revocada', revocada_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'pendiente'
              RETURNING id",
        )
        .bind(invitacion_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Invitación", invitacion_id))?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            negocio_id.map(|id| id.to_string()).as_deref(),
            "invitacion.revocada",
            "invitacion_personal",
            Some(invitacion_id),
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        let _: Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    pub async fn consultar_invitacion(&self, token: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let datos = self.buscar_vigente(&mut tx, token, false).await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({
            "valida": true,
            "correo": datos.get("correo"),
            "nombre": datos.get("nombre"),
            "tipo": datos.get("tipo"),
            "empresa_nombre": datos.get("empresa_nombre"),
            "rol_nombre": datos.get("rol_nombre"),
            "tienda_id": datos.get("tienda_id"),
            "puesto_id": datos.get("puesto_id"),
            "expira_en": normalizar_fechas(datos.clone(), &["expira_en"])["expira_en"],
        }))
    }

    pub async fn aceptar_invitacion(
        &self,
        token: &str,
        datos: AceptarInvitacion,
    ) -> Result<Value, ApiError> {
        let nombre = datos.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "NOMBRE_INVALIDO",
                "El nombre debe tener entre 2 y 120 caracteres.",
            ));
        }
        if datos.contrasena.chars().count() < 8 {
            return Err(ApiError::dominio(
                "CONTRASENA_DEBIL",
                "La contraseña debe tener al menos 8 caracteres.",
            ));
        }
        let nombre_usuario = nombre.to_string();
        let hash = hash_contrasena(&datos.contrasena)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let invitacion = self.buscar_vigente(&mut tx, token, true).await?;
        let correo = texto(&invitacion, "correo");
        let existente =
            sqlx::query("SELECT 1 FROM identity.usuarios WHERE LOWER(correo) = LOWER($1)")
                .bind(&correo)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
        if existente.is_some() {
            return Err(ApiError::nueva(
                "CORREO_YA_REGISTRADO",
                "El correo ya tiene una cuenta. Inicia sesión para aceptar la incorporación.",
                StatusCode::CONFLICT,
            ));
        }
        let empresa_id = texto(&invitacion, "empresa_id");
        let rol_empresa_id = texto(&invitacion, "rol_empresa_id");
        let tienda_id = invitacion
            .get("tienda_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let puesto_id = invitacion
            .get("puesto_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let negocio_id = invitacion
            .get("negocio_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let metadata = invitacion
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let permisos = invitacion
            .get("rol_permisos")
            .and_then(|valor| valor.get("acciones"))
            .and_then(Value::as_array)
            .map(|acciones| {
                acciones
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let fila = sqlx::query(
            "INSERT INTO identity.usuarios
               (nombre, correo, contrasena_hash, rol, tenant_id)
             VALUES ($1, $2, $3, 'operativo', $4::uuid)
             RETURNING id, nombre, correo, rol, tenant_id, creado_en",
        )
        .bind(nombre)
        .bind(&correo)
        .bind(hash)
        .bind(&empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let usuario_id: Uuid = fila.try_get("id").map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.asignaciones_personal AS a
               (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id,
                negocio_id, metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::uuid,
                     COALESCE($7::jsonb, '{}'::jsonb), $2::uuid)",
        )
        .bind(&empresa_id)
        .bind(usuario_id.to_string())
        .bind(&rol_empresa_id)
        .bind(tienda_id.as_deref())
        .bind(puesto_id.as_deref())
        .bind(negocio_id.as_deref())
        .bind(metadata)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.invitaciones_personal
                SET estado = 'aceptada', aceptada_en = NOW(), aceptada_por_usuario_id = $1::uuid
              WHERE id = $2::uuid",
        )
        .bind(usuario_id.to_string())
        .bind(texto(&invitacion, "id"))
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            &usuario_id.to_string(),
            &empresa_id,
            negocio_id.as_deref(),
            "invitacion.aceptada",
            "invitacion_personal",
            Some(&texto(&invitacion, "id")),
            json!({ "correo": correo }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;

        let creado_en: DateTime<Utc> = fila.try_get("creado_en").map_err(interno)?;
        let usuario = Usuario {
            id: usuario_id.to_string(),
            nombre: nombre_usuario,
            correo: correo.clone(),
            telefono: None,
            rol: "operativo".to_string(),
            tenant_id: Some(empresa_id),
            personal_id: None,
            participacion_porcentaje: None,
            rol_empresa_id: Some(rol_empresa_id.clone()),
            negocio_id: negocio_id.clone(),
            permisos,
            avatar_objeto_id: None,
            avatar_url: None,
            creado_en: iso_ms(creado_en),
        };
        // Aceptar la invitación prueba el control del correo.
        self.auth
            .marcar_correo_verificado(&usuario_id.to_string())
            .await?;
        let sesion = self.auth.crear_sesion(&usuario).await?;
        let mut salida = serde_json::to_value(&sesion).map_err(|_| ApiError::interno())?;
        if let Some(usuario_salida) = salida.get_mut("usuario").and_then(Value::as_object_mut) {
            usuario_salida.insert("rol_empresa_id".into(), json!(rol_empresa_id));
            usuario_salida.insert("tienda_id".into(), json!(tienda_id));
            usuario_salida.insert("puesto_id".into(), json!(puesto_id));
        }
        Ok(salida)
    }

    async fn enviar_invitacion(&self, datos: &Value, url: &str) {
        let nombre = datos
            .get("nombre")
            .and_then(Value::as_str)
            .filter(|valor| !valor.trim().is_empty())
            .unwrap_or("Hola");
        let empresa = texto(datos, "empresa_nombre");
        let rol = texto(datos, "rol_nombre");
        let nombre_html = escapar_html(nombre);
        let empresa_html = escapar_html(&empresa);
        let rol_html = escapar_html(&rol);
        let mensaje = EmailMessage {
            to: texto(datos, "correo"),
            subject: format!("Invitación a {empresa}"),
            text: format!(
                "{nombre}, te invitaron a unirte a {empresa} como {rol}. Acepta la invitación (válida 72 horas): {url}"
            ),
            html: Some(format!(
                "<p>{nombre_html}, te invitaron a unirte a <strong>{empresa_html}</strong> como {rol_html}.</p><p><a href=\"{url}\">Aceptar invitación</a> (válida 72 horas).</p>"
            )),
        };
        let email = Arc::clone(&self.email);
        let envio = tokio::task::spawn_blocking(move || email.enviar(&mensaje)).await;
        if !matches!(envio, Ok(Ok(()))) {
            crate::logs::error(
                "identity.personal",
                crate::campos! { "msg": "email_invitacion_fallido" },
            );
        }
    }

    async fn buscar_vigente(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        bloqueo: bool,
    ) -> Result<Value, ApiError> {
        let sufijo = if bloqueo { " FOR UPDATE OF i" } else { "" };
        let sql = format!(
            "SELECT to_jsonb(i) || jsonb_build_object(
                      'rol_nombre', r.nombre,
                      'rol_permisos', r.permisos,
                      'empresa_nombre', e.nombre
                    ) AS datos
               FROM identity.invitaciones_personal i
               JOIN identity.roles_empresa r ON r.id = i.rol_empresa_id
               JOIN identity.empresas e ON e.id = i.empresa_id
              WHERE i.token_hash = $1 AND e.estado = 'activa'{sufijo}"
        );
        let fila = sqlx::query(&sql)
            .bind(hash_token(token))
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(invitacion_no_valida());
        };
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        validar_vigente(&datos)?;
        Ok(datos)
    }
}
