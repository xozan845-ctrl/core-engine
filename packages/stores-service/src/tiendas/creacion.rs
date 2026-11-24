use super::*;

impl Tiendas {
    #[allow(clippy::too_many_arguments)]
    pub async fn crear(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        propietario_id: &str,
        participacion: Option<i64>,
        nombre: &str,
        descripcion: Option<&str>,
        slug_solicitado: Option<&str>,
        vendedor_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let negocio_id = match negocio_id {
            Some(id) => Some(id.to_string()),
            None => sqlx::query_scalar(
                "SELECT id FROM identity.negocios
                  WHERE empresa_id = $1::uuid AND es_predeterminado LIMIT 1",
            )
            .bind(empresa_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(crate::db::interno)?
            .map(|id: Uuid| id.to_string()),
        };
        let nombre_final = nombre.trim();
        let longitud = nombre_final.chars().count();
        if !(2..=100).contains(&longitud) {
            return Err(ApiError::dominio(
                "NOMBRE_INVALIDO",
                "El nombre de la tienda debe tener entre 2 y 100 caracteres.",
            ));
        }
        let descripcion_final = descripcion.unwrap_or("").trim();
        let slug_explicito = match slug_solicitado {
            Some(valor) => Some(slug::validar_slug_solicitado(valor)?),
            None => None,
        };
        let max_intentos = if slug_explicito.is_some() { 1 } else { 3 };

        for intento in 1..=max_intentos {
            let slug = self
                .elegir_slug(nombre_final, slug_explicito.as_deref())
                .await?;
            let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
            let resultado = sqlx::query(&format!(
                "INSERT INTO stores.tiendas
                   (id, empresa_id, negocio_id, propietario_usuario_id, vendedor_id, participacion_porcentaje, nombre, descripcion, slug)
                 VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::int, $7, $8, $9)
                 RETURNING {CAMPOS_TIENDA}"
            ))
            .bind(Uuid::new_v4())
            .bind(empresa_id)
            .bind(negocio_id.as_deref())
            .bind(propietario_id)
            .bind(vendedor_id)
            .bind(participacion)
            .bind(nombre_final)
            .bind(descripcion_final)
            .bind(&slug)
            .fetch_optional(&mut *tx)
            .await;
            match resultado {
                Ok(fila) => {
                    tx.commit().await.map_err(crate::db::interno)?;
                    let Some(fila) = fila else {
                        return Err(ApiError::dominio(
                            "TIENDA_FALLIDA",
                            "No se pudo crear la tienda.",
                        ));
                    };
                    return self
                        .con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
                        .await;
                }
                Err(error) => {
                    let _ = tx.rollback().await;
                    if !es_violacion_unicidad(&error) {
                        return Err(ApiError::interno());
                    }
                    if slug_explicito.is_some() || intento == max_intentos {
                        let usado = slug_explicito.as_deref().unwrap_or(&slug);
                        return Err(ApiError::conflicto(format!(
                            "El slug \"{usado}\" ya está en uso."
                        )));
                    }
                }
            }
        }
        Err(ApiError::conflicto(
            "No se pudo asignar un slug disponible.",
        ))
    }

    async fn elegir_slug(&self, nombre: &str, explicito: Option<&str>) -> Result<String, ApiError> {
        if let Some(slug) = explicito {
            if !self.slug_libre(slug).await? {
                return Err(ApiError::conflicto(format!(
                    "El slug \"{slug}\" ya está en uso."
                )));
            }
            return Ok(slug.to_string());
        }
        let base = slug::slug_desde_nombre(nombre, "tienda");
        for intento in 1..=MAX_INTENTOS_SLUG {
            let candidato = if intento == 1 {
                base.clone()
            } else {
                slug::slug_con_sufijo(&base, intento)
            };
            if self.slug_libre(&candidato).await? {
                return Ok(candidato);
            }
        }
        let sufijo = format!(
            "-{}",
            base36(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duracion| duracion.as_millis())
                    .unwrap_or(0)
            )
        );
        let maximo = 80usize.saturating_sub(sufijo.len()).max(2);
        let mut recorte = base.chars().take(maximo).collect::<String>();
        while recorte.ends_with('-') {
            recorte.pop();
        }
        if recorte.is_empty() {
            recorte = "tienda".into();
        }
        Ok(format!("{recorte}{sufijo}"))
    }

    pub(super) async fn slug_libre(&self, slug: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query("SELECT 1 FROM stores.tienda_slugs WHERE slug=$1")
            .bind(slug)
            .fetch_optional(&mut *tx)
            .await
            .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        Ok(fila.is_none())
    }
}
