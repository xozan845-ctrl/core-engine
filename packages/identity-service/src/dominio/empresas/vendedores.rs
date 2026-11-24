use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    pub async fn crear_vendedor(
        &self,
        actor_id: &str,
        empresa_id: &str,
        datos: CrearVendedorEmpresa,
    ) -> Result<Value, ApiError> {
        self.autorizar_membresia(empresa_id, actor_id, Some("empresario"))
            .await?;
        let usuario = self
            .usuarios
            .crear_vendedor_en_empresa(CrearVendedor {
                nombre: datos.nombre,
                correo: datos.correo,
                contrasena: datos.contrasena,
                empresa_id: empresa_id.to_string(),
                participacion_porcentaje: datos.participacion_porcentaje,
            })
            .await?;
        serde_json::to_value(usuario).map_err(|_| ApiError::interno())
    }

    pub(super) async fn buscar_por_identificador(
        &self,
        identificador: &str,
    ) -> Result<Option<(Value, bool)>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = if es_uuid(identificador) {
            sqlx::query(
                "SELECT to_jsonb(e) AS datos, true AS es_actual
                   FROM identity.empresas e WHERE e.id = $1::uuid",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?
        } else {
            sqlx::query(
                "SELECT to_jsonb(e) AS datos, (s.reemplazado_en IS NULL) AS es_actual
                   FROM identity.empresa_slugs s
                   JOIN identity.empresas e ON e.id = s.empresa_id
                  WHERE s.slug = $1",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?
        };
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let es_actual: bool = fila.try_get("es_actual").map_err(interno)?;
        Ok(Some((datos, es_actual)))
    }

    pub(super) async fn autorizar_membresia(
        &self,
        empresa_id: &str,
        usuario_id: &str,
        rol: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match rol {
            Some(rol) => sqlx::query(
                "SELECT 1 FROM identity.asignaciones_personal a
                   JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
                  WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                    AND r.codigo = $3 AND a.estado = 'activa'",
            )
            .bind(empresa_id)
            .bind(usuario_id)
            .bind(rol)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT 1 FROM identity.asignaciones_personal
                  WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'",
            )
            .bind(empresa_id)
            .bind(usuario_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
        };
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        Ok(())
    }
}
