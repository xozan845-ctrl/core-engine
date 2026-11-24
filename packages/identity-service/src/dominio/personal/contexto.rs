use super::comunes::*;
use super::prelude::*;
use super::*;

impl Personal {
    pub(super) async fn contexto(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<ContextoPersonal, ApiError> {
        if actor.es_admin_global() {
            return Ok(ContextoPersonal {
                empresa_global: true,
                negocio_id: actor.negocio_id.clone(),
                asignaciones: vec![AsignacionActor {
                    negocio_id: actor.negocio_id.clone(),
                    rol_negocio_id: None,
                    codigo: "empresario".to_string(),
                    permisos: Vec::new(),
                }],
            });
        }
        if actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No tienes acceso a esta empresa.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT a.negocio_id, r.negocio_id AS rol_negocio_id, r.codigo, r.permisos
               FROM identity.asignaciones_personal a
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
              WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                AND a.estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if filas.is_empty() {
            return Err(ApiError::acceso_denegado(
                "No tienes una asignación activa en esta empresa.",
            ));
        }
        let mut asignaciones = Vec::with_capacity(filas.len());
        for fila in filas {
            let negocio_id: Option<Uuid> = fila.try_get("negocio_id").map_err(interno)?;
            let rol_negocio_id: Option<Uuid> = fila.try_get("rol_negocio_id").map_err(interno)?;
            let codigo: String = fila.try_get("codigo").map_err(interno)?;
            let permisos: Value = fila.try_get("permisos").map_err(interno)?;
            let acciones = permisos
                .get("acciones")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            asignaciones.push(AsignacionActor {
                negocio_id: negocio_id.map(|id| id.to_string()),
                rol_negocio_id: rol_negocio_id.map(|id| id.to_string()),
                codigo,
                permisos: acciones,
            });
        }
        let empresa_global = asignaciones.iter().any(|asignacion| {
            asignacion.rol_negocio_id.is_none() && asignacion.codigo == "empresario"
        });
        let negocio_id = actor
            .negocio_id
            .as_ref()
            .filter(|negocio| {
                asignaciones.iter().any(|asignacion| {
                    asignacion.negocio_id.as_deref() == Some(negocio.as_str())
                        || asignacion.rol_negocio_id.as_deref() == Some(negocio.as_str())
                })
            })
            .cloned()
            .or_else(|| {
                asignaciones
                    .iter()
                    .find_map(|asignacion| asignacion.negocio_id.clone())
            })
            .or_else(|| {
                asignaciones
                    .iter()
                    .find_map(|asignacion| asignacion.rol_negocio_id.clone())
            });
        Ok(ContextoPersonal {
            empresa_global,
            negocio_id,
            asignaciones,
        })
    }

    pub(super) fn exigir_lectura(contexto: &ContextoPersonal) -> Result<(), ApiError> {
        if contexto.puede_leer() {
            return Ok(());
        }
        Err(ApiError::acceso_denegado(
            "Tu rol no permite consultar el personal de esta empresa.",
        ))
    }

    pub(super) fn exigir_gestion(
        contexto: &ContextoPersonal,
        negocio_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if contexto.autorizado(AccionPersonal::Gestionar, negocio_id) {
            return Ok(());
        }
        Err(ApiError::acceso_denegado(
            "Tu rol no permite administrar personal en este negocio.",
        ))
    }
}
