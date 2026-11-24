use super::comunes::*;
use super::prelude::*;
use super::*;
use serde_json::json;

impl Personal {
    pub async fn asignar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: AsignarPersonal,
    ) -> Result<Value, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        self.assert_miembro(empresa_id, &datos.usuario_id).await?;
        let rol_negocio = self
            .assert_rol(empresa_id, &datos.rol_empresa_id, true)
            .await?;
        if !contexto.empresa_global {
            if datos.usuario_id == actor.user_id {
                return Err(ApiError::acceso_denegado(
                    "Un rol de negocio no puede asignarse permisos a si mismo.",
                ));
            }
            let codigo = self.codigo_rol(empresa_id, &datos.rol_empresa_id).await?;
            if codigo == "empresario" {
                return Err(ApiError::acceso_denegado(
                    "Solo el empresario puede asignar el rol empresario.",
                ));
            }
            let permitidos = contexto.permisos_en(
                datos
                    .negocio_id
                    .as_deref()
                    .or(contexto.negocio_id.as_deref()),
            );
            for permiso in self.permisos_rol(empresa_id, &datos.rol_empresa_id).await? {
                if !permitidos.contains(&permiso) {
                    return Err(ApiError::acceso_denegado(
                        "Un rol de negocio no puede asignar permisos que no posee.",
                    ));
                }
            }
        }
        if datos.tienda_id.is_none() && datos.puesto_id.is_none() {
            return Err(ApiError::dominio(
                "ALCANCE_REQUERIDO",
                "La asignación debe incluir tienda o puesto.",
            ));
        }
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
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.asignaciones_personal AS a
               (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id,
                negocio_id, metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::uuid,
                     COALESCE($7::jsonb, '{}'::jsonb), $8::uuid)
             RETURNING to_jsonb(a) AS datos",
        )
        .bind(empresa_id)
        .bind(&datos.usuario_id)
        .bind(&datos.rol_empresa_id)
        .bind(datos.tienda_id.as_deref())
        .bind(datos.puesto_id.as_deref())
        .bind(negocio_id.as_deref())
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
        let contexto = self.contexto(actor, empresa_id).await?;
        Self::exigir_lectura(&contexto)?;
        let empresa_global = contexto.empresa_global;
        let negocio_id = contexto.negocio_id.clone();
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
                AND ($2::boolean
                     OR a.negocio_id IS NULL
                     OR ($3::uuid IS NOT NULL AND a.negocio_id = $3::uuid))
              ORDER BY a.creado_en DESC",
        )
        .bind(empresa_id)
        .bind(empresa_global)
        .bind(negocio_id)
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

    pub async fn transferir(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: TransferirPersonal,
    ) -> Result<Value, ApiError> {
        let contexto = self.contexto(actor, empresa_id).await?;
        if !contexto.empresa_global {
            return Err(ApiError::acceso_denegado(
                "Solo un empresario transversal puede transferir personal entre negocios.",
            ));
        }
        let rol_negocio = self
            .assert_rol(empresa_id, &datos.rol_empresa_id, true)
            .await?;
        let destino = self
            .negocios
            .resolver(empresa_id, Some(&datos.destino_id))
            .await?;
        let origen = self
            .negocios
            .resolver(empresa_id, Some(&datos.origen_id))
            .await?;
        validar_alcance_negocio(destino.as_deref(), rol_negocio.as_deref(), "rol")?;
        let puesto_negocio = if let Some(puesto_id) = datos.puesto_id.as_deref() {
            self.assert_puesto(empresa_id, puesto_id, datos.tienda_id.as_deref())
                .await?
        } else {
            None
        };
        validar_alcance_negocio(destino.as_deref(), puesto_negocio.as_deref(), "puesto")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.asignaciones_personal AS a
                SET negocio_id = $1::uuid, rol_empresa_id = $2::uuid,
                    tienda_id = $3::uuid, puesto_id = $4::uuid
              WHERE a.empresa_id = $5::uuid AND a.usuario_id = $6::uuid
                AND a.negocio_id = $7::uuid AND a.estado = 'activa'
              RETURNING to_jsonb(a) AS datos",
        )
        .bind(destino.as_deref())
        .bind(&datos.rol_empresa_id)
        .bind(datos.tienda_id.as_deref())
        .bind(datos.puesto_id.as_deref())
        .bind(empresa_id)
        .bind(&datos.usuario_id)
        .bind(origen.as_deref())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::no_encontrado(
                "Asignacion",
                &format!("{}:{}", datos.usuario_id, datos.origen_id),
            )
        })?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            destino.as_deref(),
            "personal.transferido",
            "asignacion_personal",
            Some(&datos.usuario_id),
            json!({ "origen_id": datos.origen_id, "destino_id": datos.destino_id }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        fila.try_get("datos").map_err(interno)
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

    async fn codigo_rol(&self, empresa_id: &str, rol_id: &str) -> Result<String, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let codigo: Option<String> = sqlx::query_scalar(
            "SELECT codigo FROM identity.roles_empresa
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND activo = true",
        )
        .bind(rol_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        codigo.ok_or_else(|| ApiError::no_encontrado("Rol", rol_id))
    }

    async fn permisos_rol(&self, empresa_id: &str, rol_id: &str) -> Result<Vec<String>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let permisos: Option<Value> = sqlx::query_scalar(
            "SELECT permisos FROM identity.roles_empresa
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND activo = true",
        )
        .bind(rol_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        permisos
            .map(|valor| acciones_de(&valor))
            .ok_or_else(|| ApiError::no_encontrado("Rol", rol_id))
    }
}
