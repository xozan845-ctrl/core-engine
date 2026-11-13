//! Personal: roles, puestos, asignaciones, invitaciones e incorporaciones.
//!
//! Réplica de `personal/personal.service.ts`. Las tiendas son autoridad de
//! `stores-service`; identity solo guarda la referencia y las valida por API
//! interna.

use crate::auth::{hash_contrasena, UsuarioContexto};
use crate::db::{iso_ms, Db};
use crate::dominio::auth::Auth;
use crate::dominio::usuarios::{correo_valido, Usuario};
use crate::email::{Email, EmailMessage};
use crate::errors::ApiError;
use crate::stores::Stores;
use axum::http::StatusCode;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const TOKEN_TTL_HORAS: i64 = 72;
const CODIGO: &str = "código";

#[derive(Debug, Clone)]
pub struct CrearRol {
    pub codigo: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub permisos: Value,
}

#[derive(Debug, Clone)]
pub struct CrearPuesto {
    pub codigo: String,
    pub nombre: String,
    pub tipo: Option<String>,
    pub tienda_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct AsignarPersonal {
    pub usuario_id: String,
    pub rol_empresa_id: String,
    pub tienda_id: Option<String>,
    pub puesto_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CrearInvitacion {
    pub correo: String,
    pub nombre: Option<String>,
    pub tipo: Option<String>,
    pub rol_empresa_id: String,
    pub tienda_id: Option<String>,
    pub puesto_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct AceptarInvitacion {
    pub nombre: String,
    pub contrasena: String,
}

pub struct Personal {
    db: Arc<Db>,
    auth: Arc<Auth>,
    email: Arc<dyn Email>,
    stores: Arc<Stores>,
    frontend_url: String,
}

impl Personal {
    pub fn nueva(
        db: Arc<Db>,
        auth: Arc<Auth>,
        email: Arc<dyn Email>,
        stores: Arc<Stores>,
        frontend_url: String,
    ) -> Self {
        Self {
            db,
            auth,
            email,
            stores,
            frontend_url,
        }
    }

    pub async fn crear_rol(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearRol,
    ) -> Result<Value, ApiError> {
        self.assert_empresario(actor, empresa_id).await?;
        let codigo = validar_codigo(&datos.codigo, CODIGO)?;
        let nombre = datos.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "NOMBRE_INVALIDO",
                "El nombre debe tener entre 2 y 120 caracteres.",
            ));
        }
        let permisos = validar_permisos(&datos.permisos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.roles_empresa AS r
               (empresa_id, codigo, nombre, descripcion, permisos, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5::jsonb, $6::uuid)
             RETURNING to_jsonb(r) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(nombre)
        .bind(datos.descripcion.as_deref().unwrap_or("").trim())
        .bind(permisos)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_codigo)?;
        tx.commit().await.map_err(interno)?;
        let fila =
            fila.ok_or_else(|| ApiError::dominio("ROL_FALLIDO", "No se pudo crear el rol."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn listar_roles(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        self.assert_acceso(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(r) AS datos FROM identity.roles_empresa r
              WHERE r.empresa_id = $1::uuid ORDER BY r.nombre",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub async fn actualizar_rol(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        rol_id: &str,
        activo: bool,
    ) -> Result<Value, ApiError> {
        self.assert_empresario(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.roles_empresa AS r
                SET activo = $1, actualizado_en = NOW()
              WHERE r.id = $2::uuid AND r.empresa_id = $3::uuid
              RETURNING to_jsonb(r) AS datos",
        )
        .bind(activo)
        .bind(rol_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Rol", rol_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn crear_puesto(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearPuesto,
    ) -> Result<Value, ApiError> {
        self.assert_empresario(actor, empresa_id).await?;
        let codigo = validar_codigo(&datos.codigo, CODIGO)?;
        let tipo = validar_codigo(datos.tipo.as_deref().unwrap_or("operativo"), "tipo")?;
        let nombre = datos.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "NOMBRE_INVALIDO",
                "El nombre debe tener entre 2 y 120 caracteres.",
            ));
        }
        if let Some(tienda_id) = datos.tienda_id.as_deref() {
            self.assert_tienda(empresa_id, tienda_id).await?;
        }
        validar_objeto(datos.metadata.as_ref(), "metadata")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.puestos_empresa AS p
               (empresa_id, codigo, nombre, tipo, tienda_id, metadata, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid, COALESCE($6::jsonb, '{}'::jsonb), $7::uuid)
             RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(nombre)
        .bind(&tipo)
        .bind(datos.tienda_id.as_deref())
        .bind(datos.metadata.clone())
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_codigo)?;
        tx.commit().await.map_err(interno)?;
        let fila =
            fila.ok_or_else(|| ApiError::dominio("PUESTO_FALLIDO", "No se pudo crear el puesto."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn listar_puestos(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        self.assert_acceso(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(p) AS datos FROM identity.puestos_empresa p
              WHERE p.empresa_id = $1::uuid ORDER BY p.nombre",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub async fn asignar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: AsignarPersonal,
    ) -> Result<Value, ApiError> {
        self.assert_empresario(actor, empresa_id).await?;
        self.assert_miembro(empresa_id, &datos.usuario_id).await?;
        self.assert_rol(empresa_id, &datos.rol_empresa_id, false)
            .await?;
        if datos.tienda_id.is_none() && datos.puesto_id.is_none() {
            return Err(ApiError::dominio(
                "ALCANCE_REQUERIDO",
                "La asignación debe incluir tienda o puesto.",
            ));
        }
        if let Some(tienda_id) = datos.tienda_id.as_deref() {
            self.assert_tienda(empresa_id, tienda_id).await?;
        }
        if let Some(puesto_id) = datos.puesto_id.as_deref() {
            self.assert_puesto(empresa_id, puesto_id, datos.tienda_id.as_deref())
                .await?;
        }
        validar_objeto(datos.metadata.as_ref(), "metadata")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.asignaciones_personal AS a
               (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id,
                metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid,
                     COALESCE($6::jsonb, '{}'::jsonb), $7::uuid)
             RETURNING to_jsonb(a) AS datos",
        )
        .bind(empresa_id)
        .bind(&datos.usuario_id)
        .bind(&datos.rol_empresa_id)
        .bind(datos.tienda_id.as_deref())
        .bind(datos.puesto_id.as_deref())
        .bind(datos.metadata.clone())
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| {
            ApiError::dominio("ASIGNACION_FALLIDA", "No se pudo crear la asignación.")
        })?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["inicia_en", "termina_en", "creado_en"],
        ))
    }

    pub async fn listar_asignaciones(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        self.assert_acceso(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(a) || jsonb_build_object(
                      'usuario_nombre', u.nombre,
                      'usuario_correo', u.correo,
                      'rol_codigo', r.codigo,
                      'rol_nombre', r.nombre,
                      'puesto_nombre', p.nombre
                    ) AS datos
               FROM identity.asignaciones_personal a
               JOIN identity.usuarios u ON u.id = a.usuario_id
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
               LEFT JOIN identity.puestos_empresa p
                 ON p.id = a.puesto_id AND p.empresa_id = a.empresa_id
              WHERE a.empresa_id = $1::uuid
              ORDER BY a.creado_en DESC",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let mut asignaciones: Vec<Value> = filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &["inicia_en", "termina_en", "creado_en"],
                ))
            })
            .collect::<Result<_, _>>()?;
        let ids: Vec<String> = asignaciones
            .iter()
            .filter_map(|fila| fila.get("tienda_id").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if ids.is_empty() {
            return Ok(asignaciones);
        }
        let tiendas = self.stores.por_ids(&ids).await?;
        let nombres: HashMap<String, String> = tiendas
            .unwrap_or_default()
            .into_iter()
            .map(|tienda| (tienda.id, tienda.nombre))
            .collect();
        for asignacion in &mut asignaciones {
            let tienda_id = asignacion
                .get("tienda_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(tienda_id) = tienda_id {
                if let Some(objeto) = asignacion.as_object_mut() {
                    objeto.insert("tienda_nombre".into(), json!(nombres.get(&tienda_id)));
                }
            }
        }
        Ok(asignaciones)
    }

    pub async fn crear_invitacion(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearInvitacion,
    ) -> Result<Value, ApiError> {
        self.assert_empresario(actor, empresa_id).await?;
        let correo = validar_correo_invitacion(&datos.correo)?;
        self.assert_rol(empresa_id, &datos.rol_empresa_id, true)
            .await?;
        if let Some(tienda_id) = datos.tienda_id.as_deref() {
            self.assert_tienda(empresa_id, tienda_id).await?;
        }
        if let Some(puesto_id) = datos.puesto_id.as_deref() {
            self.assert_puesto(empresa_id, puesto_id, datos.tienda_id.as_deref())
                .await?;
        }
        validar_objeto(datos.metadata.as_ref(), "metadata")?;
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
                  puesto_id, metadata, token_hash, expira_en, creada_por_usuario_id)
               VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::uuid, $7::uuid,
                       COALESCE($8::jsonb, '{}'::jsonb), $9, $10, $11::uuid)
               RETURNING id, empresa_id, correo, nombre, tipo, rol_empresa_id,
                         tienda_id, puesto_id, estado, expira_en
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
        .bind(datos.metadata.clone())
        .bind(hash_token(&token))
        .bind(expira)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
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
        self.assert_acceso(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(i) || jsonb_build_object('rol_nombre', r.nombre) AS datos
               FROM identity.invitaciones_personal i
               JOIN identity.roles_empresa r ON r.id = i.rol_empresa_id
              WHERE i.empresa_id = $1::uuid
              ORDER BY i.creada_en DESC",
        )
        .bind(empresa_id)
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
        self.assert_empresario(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
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
        let metadata = invitacion
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({}));
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
               (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id, metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid,
                     COALESCE($6::jsonb, '{}'::jsonb), $2::uuid)",
        )
        .bind(&empresa_id)
        .bind(usuario_id.to_string())
        .bind(&rol_empresa_id)
        .bind(tienda_id.as_deref())
        .bind(puesto_id.as_deref())
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
        tx.commit().await.map_err(interno)?;

        let creado_en: DateTime<Utc> = fila.try_get("creado_en").map_err(interno)?;
        let usuario = Usuario {
            id: usuario_id.to_string(),
            nombre: nombre_usuario,
            correo: correo.clone(),
            rol: "operativo".to_string(),
            tenant_id: Some(empresa_id),
            personal_id: None,
            participacion_porcentaje: None,
            rol_empresa_id: Some(rol_empresa_id.clone()),
            avatar_objeto_id: None,
            avatar_url: None,
            creado_en: iso_ms(creado_en),
        };
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

    async fn assert_acceso(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.rol != "empresario" || actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No tienes acceso a esta empresa.",
            ));
        }
        self.assert_empresario(actor, empresa_id).await
    }

    async fn assert_empresario(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.rol != "empresario" || actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "Solo el empresario activo puede administrar personal.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let miembro = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal a
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
              WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                AND r.codigo = 'empresario' AND a.estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::acceso_denegado(
                "El usuario no es empresario activo de esta empresa.",
            ));
        }
        Ok(())
    }

    async fn assert_rol(
        &self,
        empresa_id: &str,
        rol_id: &str,
        activo: bool,
    ) -> Result<(), ApiError> {
        let sql = format!(
            "SELECT id FROM identity.roles_empresa
              WHERE id = $1::uuid AND empresa_id = $2::uuid{}",
            if activo { " AND activo = true" } else { "" }
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&sql)
            .bind(rol_id)
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Rol", rol_id));
        }
        Ok(())
    }

    async fn assert_puesto(
        &self,
        empresa_id: &str,
        puesto_id: &str,
        tienda_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT tienda_id FROM identity.puestos_empresa
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND activo = true",
        )
        .bind(puesto_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Puesto", puesto_id))?;
        tx.commit().await.map_err(interno)?;
        let tienda_puesto: Option<Uuid> = fila.try_get("tienda_id").map_err(interno)?;
        if let (Some(tienda_puesto), Some(tienda)) = (tienda_puesto, tienda_id) {
            if tienda_puesto.to_string() != tienda {
                return Err(ApiError::conflicto("El puesto pertenece a otra tienda."));
            }
        }
        Ok(())
    }

    async fn assert_tienda(&self, empresa_id: &str, tienda_id: &str) -> Result<(), ApiError> {
        let tienda = self
            .stores
            .obtener(tienda_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Tienda", tienda_id))?;
        if tienda.empresa_id != empresa_id {
            return Err(ApiError::no_encontrado("Tienda", tienda_id));
        }
        Ok(())
    }

    async fn assert_miembro(&self, empresa_id: &str, usuario_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal
              WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(usuario_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Miembro", usuario_id));
        }
        Ok(())
    }
}

fn validar_codigo(valor: &str, campo: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            format!("{}_INVALIDO", campo.to_uppercase()),
            format!("{campo} no es válido."),
        ));
    }
    Ok(codigo)
}

fn validar_correo_invitacion(correo: &str) -> Result<String, ApiError> {
    let correo = correo.trim().to_lowercase();
    if !correo_valido(&correo)
        || !correo
            .split_once('@')
            .is_some_and(|(_, dominio)| dominio.contains('.'))
    {
        return Err(ApiError::dominio(
            "CORREO_INVALIDO",
            "El correo no tiene un formato válido.",
        ));
    }
    Ok(correo)
}

fn validar_objeto(valor: Option<&Value>, campo: &str) -> Result<(), ApiError> {
    if let Some(valor) = valor {
        if !valor.is_object() {
            return Err(ApiError::dominio(
                format!("{}_INVALIDO", campo.to_uppercase()),
                format!("{campo} debe ser un objeto JSON."),
            ));
        }
    }
    Ok(())
}

fn validar_permisos(permisos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = permisos.as_object() else {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos debe ser un objeto JSON.",
        ));
    };
    let Some(acciones) = objeto.get("acciones").and_then(Value::as_array) else {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos.acciones debe contener entre 1 y 200 acciones.",
        ));
    };
    if acciones.is_empty() || acciones.len() > 200 {
        return Err(ApiError::dominio(
            "PERMISOS_INVALIDOS",
            "permisos.acciones debe contener entre 1 y 200 acciones.",
        ));
    }
    let mut unicas: Vec<String> = Vec::new();
    for accion in acciones {
        let Some(accion) = accion.as_str() else {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "Cada acción debe usar el formato recurso.accion.",
            ));
        };
        if !accion_valida(accion) {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "Cada acción debe usar el formato recurso.accion.",
            ));
        }
        if !unicas.iter().any(|existente| existente == accion) {
            unicas.push(accion.to_string());
        }
    }
    let mut salida = Map::new();
    salida.insert(
        "version".into(),
        json!(objeto.get("version").and_then(Value::as_i64).unwrap_or(1)),
    );
    salida.insert("acciones".into(), json!(unicas));
    if let Some(recursos) = objeto.get("recursos") {
        let Some(recursos) = recursos.as_array() else {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "permisos.recursos debe ser una lista de códigos válidos.",
            ));
        };
        let mut limpios: Vec<String> = Vec::new();
        for recurso in recursos {
            let Some(recurso) = recurso.as_str() else {
                return Err(ApiError::dominio(
                    "PERMISOS_INVALIDOS",
                    "permisos.recursos debe ser una lista de códigos válidos.",
                ));
            };
            if validar_codigo(recurso, "recurso").is_err() {
                return Err(ApiError::dominio(
                    "PERMISOS_INVALIDOS",
                    "permisos.recursos debe ser una lista de códigos válidos.",
                ));
            }
            if !limpios.iter().any(|existente| existente == recurso) {
                limpios.push(recurso.to_string());
            }
        }
        salida.insert("recursos".into(), json!(limpios));
    }
    if let Some(condiciones) = objeto.get("condiciones") {
        if !condiciones.is_object() {
            return Err(ApiError::dominio(
                "PERMISOS_INVALIDOS",
                "permisos.condiciones debe ser un objeto JSON.",
            ));
        }
        salida.insert("condiciones".into(), condiciones.clone());
    }
    Ok(Value::Object(salida))
}

fn accion_valida(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=80).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
}

fn escapar_html(valor: &str) -> String {
    valor
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn token_aleatorio() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn validar_vigente(datos: &Value) -> Result<(), ApiError> {
    let estado = datos.get("estado").and_then(Value::as_str).unwrap_or("");
    let expira = datos
        .get("expira_en")
        .and_then(Value::as_str)
        .and_then(|valor| chrono::DateTime::parse_from_rfc3339(valor).ok());
    if estado != "pendiente" || expira.is_none_or(|fecha| fecha.with_timezone(&Utc) <= Utc::now()) {
        return Err(invitacion_no_valida());
    }
    Ok(())
}

fn invitacion_no_valida() -> ApiError {
    ApiError::nueva(
        "INVITACION_NO_VALIDA",
        "La invitación no existe, expiró o ya fue utilizada.",
        StatusCode::GONE,
    )
}

fn normalizar_fechas(mut datos: Value, claves: &[&str]) -> Value {
    if let Some(objeto) = datos.as_object_mut() {
        for clave in claves {
            if let Some(valor) = objeto.get(*clave).and_then(Value::as_str) {
                if let Ok(fecha) = chrono::DateTime::parse_from_rfc3339(valor) {
                    objeto.insert(
                        (*clave).to_string(),
                        json!(iso_ms(fecha.with_timezone(&Utc))),
                    );
                }
            }
        }
    }
    datos
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn conflicto_codigo(error: sqlx::Error) -> ApiError {
    if matches!(&error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")) {
        return ApiError::conflicto("Ya existe un registro con ese código.");
    }
    interno(error)
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_permisos_y_deduplicar() {
        let permisos = validar_permisos(&json!({
            "acciones": ["catalogo.leer", "catalogo.leer", "ordenes.crear"],
            "recursos": ["tienda"]
        }))
        .expect("permisos");
        assert_eq!(permisos["acciones"].as_array().expect("acciones").len(), 2);
        assert_eq!(permisos["version"], 1);
        assert_eq!(permisos["recursos"], json!(["tienda"]));
        assert!(validar_permisos(&json!({"acciones": []})).is_err());
        assert!(validar_permisos(&json!({"acciones": ["MAL"]})).is_err());
    }

    #[test]
    fn debe_generar_token_y_hash_estables() {
        let token = token_aleatorio();
        assert_eq!(token.len(), 43);
        assert_eq!(hash_token(&token).len(), 64);
        assert_ne!(token, token_aleatorio());
    }

    #[test]
    fn debe_validar_correos_y_codigos() {
        assert!(validar_correo_invitacion("ana@tienda.test").is_ok());
        assert!(validar_correo_invitacion("ana@localhost").is_err());
        assert_eq!(
            validar_codigo("Estilista", "codigo").expect("codigo"),
            "estilista"
        );
        assert!(validar_codigo("1malo", "codigo").is_err());
    }

    #[test]
    fn debe_rechazar_una_invitacion_expirada() {
        let datos = json!({"estado": "pendiente", "expira_en": "2020-01-01T00:00:00Z"});
        assert_eq!(
            validar_vigente(&datos).expect_err("expirada").codigo,
            "INVITACION_NO_VALIDA"
        );
    }
}
