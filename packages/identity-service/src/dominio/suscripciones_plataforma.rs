//! Suscripción de plataforma para operar más de un negocio (ADR-0046).
//!
//! El cobro es manual en el MVP: el empresario solicita el plan, un
//! administrador confirma el pago y la suscripción queda activa por el periodo
//! configurado. El precio y el periodo viven en `identity.planes_plataforma`;
//! el código solo aplica la regla de expansión.

use crate::auth::UsuarioContexto;
use crate::db::Db;
use crate::errors::ApiError;
use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

const ESTADOS: [&str; 4] = ["pendiente", "activa", "vencida", "cancelada"];

pub struct SuscripcionesPlataforma {
    db: Arc<Db>,
    plan_codigo: String,
    empresas_gratis: i64,
    negocios_gratis: i64,
}

impl SuscripcionesPlataforma {
    pub fn nueva(
        db: Arc<Db>,
        plan_codigo: String,
        empresas_gratis: i64,
        negocios_gratis: i64,
    ) -> Self {
        Self {
            db,
            plan_codigo,
            empresas_gratis: empresas_gratis.max(0),
            negocios_gratis: negocios_gratis.max(0),
        }
    }

    pub fn negocios_gratis(&self) -> i64 {
        self.negocios_gratis
    }

    /// Plan comercial del MVP, con el precio vigente en base de datos.
    pub async fn plan(&self) -> Result<Value, ApiError> {
        let fila = sqlx::query(
            "SELECT to_jsonb(p) AS datos
               FROM identity.planes_plataforma p
              WHERE p.codigo = $1 AND p.activo",
        )
        .bind(&self.plan_codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio(
                "PLAN_NO_DISPONIBLE",
                "El plan de plataforma no está configurado.",
            )
        })?;
        fila.try_get("datos").map_err(interno)
    }

    pub async fn listar_planes(&self) -> Result<Vec<Value>, ApiError> {
        let filas = sqlx::query(
            "SELECT to_jsonb(p) AS datos
               FROM identity.planes_plataforma p
              WHERE p.activo
              ORDER BY p.precio_cents, p.codigo",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    /// Solicita el plan para el empresario autenticado. Queda `pendiente`
    /// hasta que un administrador confirme el pago.
    pub async fn crear(
        &self,
        actor: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        if !actor.es_admin_global() && actor.rol != "empresario" {
            return Err(ApiError::acceso_denegado(
                "Solo un empresario puede solicitar el plan de expansión.",
            ));
        }
        let empresa = self.empresa_del_actor(actor, empresa_id).await?;
        let activa: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM identity.suscripciones_plataforma s
                 WHERE s.usuario_id = $1::uuid
                   AND s.estado = 'activa'
                   AND (s.fin_en IS NULL OR s.fin_en > NOW())
            )",
        )
        .bind(&actor.user_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)?;
        if activa {
            return Err(ApiError::conflicto(
                "Ya existe una suscripción de plataforma activa.",
            ));
        }
        // Una suscripción vencida sigue con estado `activa`; se cierra antes de
        // insertar la nueva para no violar el índice único parcial.
        sqlx::query(
            "UPDATE identity.suscripciones_plataforma
                SET estado = 'vencida', actualizado_en = NOW()
              WHERE usuario_id = $1::uuid AND estado = 'activa'
                AND fin_en IS NOT NULL AND fin_en <= NOW()",
        )
        .bind(&actor.user_id)
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.suscripciones_plataforma
               (usuario_id, empresa_id, plan_codigo, estado, origen,
                precio_cents, moneda, proveedor, creado_por)
             SELECT $1::uuid, $2::uuid, p.codigo, 'pendiente', 'manual',
                    p.precio_cents, p.moneda, 'manual', $1::uuid
               FROM identity.planes_plataforma p
              WHERE p.codigo = $3 AND p.activo
             RETURNING to_jsonb(suscripciones_plataforma) AS datos",
        )
        .bind(&actor.user_id)
        .bind(empresa.as_deref())
        .bind(&self.plan_codigo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio(
                "PLAN_NO_DISPONIBLE",
                "El plan de plataforma no está configurado.",
            )
        })?;
        fila.try_get("datos").map_err(interno)
    }

    /// Última suscripción del actor (activa, pendiente o histórica).
    pub async fn actual(&self, actor: &UsuarioContexto) -> Result<Value, ApiError> {
        let fila = sqlx::query(
            "SELECT to_jsonb(s) || jsonb_build_object(
                      'vigente', (s.estado = 'activa'
                                  AND (s.fin_en IS NULL OR s.fin_en > NOW()))
                    ) AS datos
               FROM identity.suscripciones_plataforma s
              WHERE s.usuario_id = $1::uuid
              ORDER BY s.creado_en DESC
              LIMIT 1",
        )
        .bind(&actor.user_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        Ok(fila
            .map(|fila| fila.try_get("datos"))
            .transpose()
            .map_err(interno)?
            .unwrap_or(Value::Null))
    }

    pub async fn listar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: Option<&str>,
        estado: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        if let Some(estado) = estado {
            if !ESTADOS.contains(&estado) {
                return Err(ApiError::dominio(
                    "ESTADO_INVALIDO",
                    "El estado debe ser pendiente, activa, vencida o cancelada.",
                ));
            }
        }
        let usuario = if actor.es_admin_global() {
            None
        } else {
            Some(actor.user_id.clone())
        };
        let filas = sqlx::query(
            "SELECT to_jsonb(s) || jsonb_build_object(
                      'vigente', (s.estado = 'activa'
                                  AND (s.fin_en IS NULL OR s.fin_en > NOW()))
                    ) AS datos
               FROM identity.suscripciones_plataforma s
              WHERE ($1::uuid IS NULL OR s.empresa_id = $1::uuid)
                AND ($2::text IS NULL OR s.estado = $2)
                AND ($3::uuid IS NULL OR s.usuario_id = $3::uuid)
              ORDER BY s.creado_en DESC",
        )
        .bind(empresa_id)
        .bind(estado)
        .bind(usuario.as_deref())
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    /// Confirma el pago y activa/renueva el periodo. Solo lo llama un
    /// administrador; el handler HTTP valida el rol.
    pub async fn confirmar(&self, id: &str, referencia: &str) -> Result<Value, ApiError> {
        let fila = sqlx::query(
            "WITH actual AS (
                SELECT s.id,
                       GREATEST(COALESCE(s.fin_en, NOW()), NOW()) AS base,
                       p.periodo_meses
                  FROM identity.suscripciones_plataforma s
                  JOIN identity.planes_plataforma p ON p.codigo = s.plan_codigo
                 WHERE s.id = $1::uuid
             )
             UPDATE identity.suscripciones_plataforma s
                SET estado = 'activa',
                    origen = CASE WHEN s.origen = 'legacy' THEN 'manual' ELSE s.origen END,
                    inicio_en = COALESCE(s.inicio_en, NOW()),
                    fin_en = actual.base + make_interval(months => actual.periodo_meses),
                    proximo_cobro = actual.base + make_interval(months => actual.periodo_meses),
                    referencia = $2,
                    actualizado_en = NOW()
               FROM actual
              WHERE s.id = actual.id
             RETURNING to_jsonb(s) AS datos",
        )
        .bind(id)
        .bind(referencia.trim())
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Suscripción", id))?;
        fila.try_get("datos").map_err(interno)
    }

    pub async fn cancelar(&self, id: &str) -> Result<Value, ApiError> {
        let fila = sqlx::query(
            "UPDATE identity.suscripciones_plataforma
                SET estado = 'cancelada', fin_en = NOW(), proximo_cobro = NULL,
                    actualizado_en = NOW()
              WHERE id = $1::uuid
             RETURNING to_jsonb(suscripciones_plataforma) AS datos",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Suscripción", id))?;
        fila.try_get("datos").map_err(interno)
    }

    pub async fn activa_para_usuario(&self, usuario_id: &str) -> Result<bool, ApiError> {
        sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM identity.suscripciones_plataforma s
                 WHERE s.usuario_id = $1::uuid
                   AND s.estado = 'activa'
                   AND (s.fin_en IS NULL OR s.fin_en > NOW())
            )",
        )
        .bind(usuario_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)
    }

    /// Exige plan activo cuando la cantidad actual ya alcanzó el cupo gratuito.
    pub async fn exigir_expansion(
        &self,
        usuario_id: &str,
        cantidad_actual: i64,
        gratuitos: i64,
    ) -> Result<(), ApiError> {
        if cantidad_actual < gratuitos {
            return Ok(());
        }
        if self.activa_para_usuario(usuario_id).await? {
            return Ok(());
        }
        let plan = self.plan().await?;
        Err(ApiError::nueva(
            "SUSCRIPCION_REQUERIDA",
            "Para operar más de un negocio se requiere la suscripción mensual de la plataforma.",
            StatusCode::PAYMENT_REQUIRED,
        )
        .con_detalles(json!({
            "plan": plan.get("codigo"),
            "nombre": plan.get("nombre"),
            "precio_cents": plan.get("precio_cents"),
            "moneda": plan.get("moneda"),
            "periodo_meses": plan.get("periodo_meses"),
        })))
    }

    /// Regla de alta de una empresa adicional para el empresario autenticado.
    pub async fn exigir_empresa_extra(&self, actor: &UsuarioContexto) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        let existentes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM identity.empresas WHERE creado_por = $1::uuid",
        )
        .bind(&actor.user_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)?;
        self.exigir_expansion(&actor.user_id, existentes, self.empresas_gratis)
            .await
    }

    async fn empresa_del_actor(
        &self,
        actor: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        if let Some(empresa_id) = empresa_id {
            let permitida: bool = if actor.es_admin_global() {
                sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM identity.empresas WHERE id = $1::uuid)",
                )
                .bind(empresa_id)
                .fetch_one(self.db.pool())
                .await
                .map_err(interno)?
            } else {
                sqlx::query_scalar(
                    "SELECT EXISTS (
                        SELECT 1 FROM identity.empresas
                         WHERE id = $1::uuid AND creado_por = $2::uuid
                    )",
                )
                .bind(empresa_id)
                .bind(&actor.user_id)
                .fetch_one(self.db.pool())
                .await
                .map_err(interno)?
            };
            if !permitida {
                return Err(ApiError::acceso_denegado(
                    "La empresa indicada no pertenece al empresario.",
                ));
            }
            return Ok(Some(empresa_id.to_string()));
        }
        let propia: Option<String> = sqlx::query_scalar(
            "SELECT id::text FROM identity.empresas
              WHERE creado_por = $1::uuid
              ORDER BY creado_en
              LIMIT 1",
        )
        .bind(&actor.user_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        Ok(propia)
    }
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}
