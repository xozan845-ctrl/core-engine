use super::comunes::*;
use super::prelude::*;
use super::*;
use serde_json::json;

impl Personal {
    pub async fn crear_rol(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearRol,
    ) -> Result<Value, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        let codigo = validar_codigo(&datos.codigo, CODIGO)?;
        let nombre = datos.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "NOMBRE_INVALIDO",
                "El nombre debe tener entre 2 y 120 caracteres.",
            ));
        }
        let permisos = validar_permisos(&datos.permisos)?;
        self.validar_catalogo_permisos(&permisos).await?;
        // `None` conserva el rol transversal de la empresa cuando el actor
        // administra toda la empresa; un rol de negocio acota su alcance.
        let negocio_id = match datos.negocio_id.as_deref() {
            Some(id) => self.negocios.resolver(empresa_id, Some(id)).await?,
            None if contexto.empresa_global => None,
            None => contexto.negocio_id.clone(),
        };
        Self::exigir_gestion(&contexto, negocio_id.as_deref())?;
        if !contexto.empresa_global {
            let permitidos = contexto.permisos_en(negocio_id.as_deref());
            for accion in acciones_de(&permisos) {
                if !permitidos.contains(&accion) {
                    return Err(ApiError::acceso_denegado(
                        "Un rol de negocio no puede otorgar permisos que no posee.",
                    ));
                }
            }
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.roles_empresa AS r
               (empresa_id, codigo, nombre, descripcion, permisos, negocio_id, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5::jsonb, $6::uuid, $7::uuid)
             RETURNING to_jsonb(r) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(nombre)
        .bind(datos.descripcion.as_deref().unwrap_or("").trim())
        .bind(&permisos)
        .bind(negocio_id.as_deref())
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_codigo)?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            negocio_id.as_deref(),
            "rol.creado",
            "rol_empresa",
            None,
            json!({ "codigo": codigo, "nombre": nombre, "permisos": permisos }),
        )
        .await?;
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
        let contexto = self.contexto(actor, empresa_id).await?;
        Self::exigir_lectura(&contexto)?;
        let empresa_global = contexto.empresa_global;
        let negocio_id = contexto.negocio_id.clone();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(r) AS datos FROM identity.roles_empresa r
              WHERE r.empresa_id = $1::uuid
                AND ($2::boolean
                     OR r.negocio_id IS NULL
                     OR ($3::uuid IS NOT NULL AND r.negocio_id = $3::uuid))
              ORDER BY r.nombre",
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
        let contexto = self.contexto(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let actual = sqlx::query(
            "SELECT negocio_id FROM identity.roles_empresa
              WHERE id = $1::uuid AND empresa_id = $2::uuid",
        )
        .bind(rol_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Rol", rol_id))?;
        let negocio_id: Option<Uuid> = actual.try_get("negocio_id").map_err(interno)?;
        Self::exigir_gestion(&contexto, negocio_id.map(|id| id.to_string()).as_deref())?;
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
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            negocio_id.map(|id| id.to_string()).as_deref(),
            "rol.actualizado",
            "rol_empresa",
            Some(rol_id),
            json!({ "activo": activo }),
        )
        .await?;
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
        let contexto = self.contexto(actor, empresa_id).await?;
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
        let objetivo = datos
            .negocio_id
            .clone()
            .or_else(|| contexto.negocio_id.clone());
        let negocio_id = self
            .negocios
            .resolver(empresa_id, objetivo.as_deref())
            .await?;
        Self::exigir_gestion(&contexto, negocio_id.as_deref())?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.puestos_empresa AS p
               (empresa_id, codigo, nombre, tipo, tienda_id, negocio_id, metadata, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::uuid,
                     COALESCE($7::jsonb, '{}'::jsonb), $8::uuid)
             RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(nombre)
        .bind(&tipo)
        .bind(datos.tienda_id.as_deref())
        .bind(negocio_id.as_deref())
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
        let contexto = self.contexto(actor, empresa_id).await?;
        Self::exigir_lectura(&contexto)?;
        let empresa_global = contexto.empresa_global;
        let negocio_id = contexto.negocio_id.clone();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(p) AS datos FROM identity.puestos_empresa p
              WHERE p.empresa_id = $1::uuid
                AND ($2::boolean
                     OR p.negocio_id IS NULL
                     OR ($3::uuid IS NOT NULL AND p.negocio_id = $3::uuid))
              ORDER BY p.nombre",
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
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub(super) async fn assert_rol(
        &self,
        empresa_id: &str,
        rol_id: &str,
        activo: bool,
    ) -> Result<Option<String>, ApiError> {
        let sql = format!(
            "SELECT id, negocio_id FROM identity.roles_empresa
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
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Rol", rol_id));
        };
        let negocio_id: Option<Uuid> = fila.try_get("negocio_id").map_err(interno)?;
        Ok(negocio_id.map(|id| id.to_string()))
    }

    pub(super) async fn assert_puesto(
        &self,
        empresa_id: &str,
        puesto_id: &str,
        tienda_id: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT tienda_id, negocio_id FROM identity.puestos_empresa
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
        let negocio_id: Option<Uuid> = fila.try_get("negocio_id").map_err(interno)?;
        Ok(negocio_id.map(|id| id.to_string()))
    }

    pub(super) async fn assert_tienda(
        &self,
        empresa_id: &str,
        tienda_id: &str,
    ) -> Result<(), ApiError> {
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

    async fn validar_catalogo_permisos(&self, permisos: &Value) -> Result<(), ApiError> {
        let acciones: Vec<String> = permisos
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
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let encontrados = sqlx::query_scalar::<_, String>(
            "SELECT codigo FROM identity.permisos
              WHERE codigo = ANY($1::text[]) AND activo = true",
        )
        .bind(&acciones)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let faltantes: Vec<&str> = acciones
            .iter()
            .map(String::as_str)
            .filter(|accion| !encontrados.iter().any(|codigo| codigo == accion))
            .collect();
        if !faltantes.is_empty() {
            return Err(ApiError::dominio(
                "PERMISO_INVALIDO",
                format!(
                    "Acciones no registradas en el catálogo: {}.",
                    faltantes.join(", ")
                ),
            ));
        }
        Ok(())
    }
}
