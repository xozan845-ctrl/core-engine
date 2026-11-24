use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    /// Cambia el estado del tenant y revoca las sesiones si deja de estar
    /// activo. `inactiva` representa el cierre operativo; la reactivación
    /// queda disponible para un administrador de plataforma.
    pub async fn cambiar_estado(
        &self,
        empresa_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: &str,
    ) -> Result<Value, ApiError> {
        let estado = estado.trim().to_lowercase();
        if !ESTADOS_EMPRESA.contains(&estado.as_str()) {
            return Err(ApiError::dominio(
                "ESTADO_EMPRESA_INVALIDO",
                "Usa activa, suspendida o inactiva.",
            ));
        }
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT estado, to_jsonb(e) AS datos
               FROM identity.empresas e
              WHERE e.id = $1::uuid
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };
        let actual: String = fila.try_get("estado").map_err(interno)?;
        let datos_actuales: Value = fila.try_get("datos").map_err(interno)?;
        if actual == estado {
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos_actuales));
        }
        if !transicion_estado_permitida(&actual, &estado) {
            return Err(ApiError::dominio(
                "TRANSICION_ESTADO_INVALIDA",
                format!("No se puede pasar de \"{actual}\" a \"{estado}\"."),
            ));
        }

        let fila = sqlx::query(
            "UPDATE identity.empresas AS e
                SET estado = $1
              WHERE e.id = $2::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(&estado)
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;

        if estado != "activa" {
            revocar_sesiones_tenant(&mut tx, empresa_id).await?;
        }

        auditoria::registrar(
            &mut tx,
            actor_usuario_id,
            empresa_id,
            None,
            "empresa.estado",
            "empresa",
            Some(empresa_id),
            json!({
                "anterior": actual,
                "nuevo": estado,
                "motivo": motivo,
            }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(Self::con_urls(&datos))
    }
}

/// `inactiva` es el cierre operativo; solo un administrador puede reactivar.
pub(super) fn transicion_estado_permitida(actual: &str, nuevo: &str) -> bool {
    matches!(
        (actual, nuevo),
        ("activa", "suspendida" | "inactiva")
            | ("suspendida", "activa" | "inactiva")
            | ("inactiva", "activa")
    )
}
