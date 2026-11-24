use super::*;

impl Tiendas {
    /// Cambia el slug actual; el anterior queda en el historial y redirige 301.
    pub async fn actualizar_slug(
        &self,
        actor_id: &str,
        identificador: &str,
        slug_solicitado: &str,
    ) -> Result<Value, ApiError> {
        let Some(resuelta) = self.resolver(identificador).await? else {
            return Err(ApiError::no_encontrado("Tienda", identificador));
        };
        let tienda = resuelta.tienda;
        if texto(&tienda, "propietario_usuario_id") != actor_id {
            return Err(ApiError::acceso_denegado(
                "Solo el propietario de la tienda puede cambiar su slug.",
            ));
        }
        let slug_nuevo = slug::validar_slug_solicitado(slug_solicitado)?;
        if slug_nuevo == texto(&tienda, "slug") {
            return Ok(tienda);
        }
        if !self.slug_libre(&slug_nuevo).await? {
            return Err(ApiError::conflicto(format!(
                "El slug \"{slug_nuevo}\" ya está en uso."
            )));
        }
        let id = texto(&tienda, "id");
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let resultado = sqlx::query(&format!(
            "UPDATE stores.tiendas SET slug=$1 WHERE id=$2::uuid RETURNING {CAMPOS_TIENDA}"
        ))
        .bind(&slug_nuevo)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await;
        match resultado {
            Ok(fila) => {
                tx.commit().await.map_err(crate::db::interno)?;
                let Some(fila) = fila else {
                    return Err(ApiError::no_encontrado("Tienda", &id));
                };
                self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
                    .await
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "El slug \"{slug_nuevo}\" ya está en uso."
                    )));
                }
                Err(ApiError::interno())
            }
        }
    }

    /// Asocia o quita el logo de la tienda.
    pub async fn actualizar_logo(
        &self,
        actor: &crate::auth::UsuarioContexto,
        identificador: &str,
        logo_objeto_id: Option<String>,
    ) -> Result<Value, ApiError> {
        let Some(resuelta) = self.resolver(identificador).await? else {
            return Err(ApiError::no_encontrado("Tienda", identificador));
        };
        let tienda = resuelta.tienda;
        verificar_acceso(
            actor,
            &tienda,
            "Solo el dueño de la tienda puede cambiar su logo.",
        )?;
        if let Some(objeto_id) = &logo_objeto_id {
            if !slug::es_uuid(objeto_id) {
                return Err(ApiError::dominio(
                    "LOGO_INVALIDO",
                    "El logo debe ser un objeto de storage válido.",
                ));
            }
            self.validar_logo(&texto(&tienda, "empresa_id"), objeto_id)
                .await?;
        }
        let id = texto(&tienda, "id");
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE stores.tiendas SET logo_objeto_id=$1::uuid, actualizado_en=NOW()
              WHERE id=$2::uuid RETURNING {CAMPOS_TIENDA}"
        ))
        .bind(&logo_objeto_id)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Tienda", &id));
        };
        self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
            .await
    }

    /// Asocia, cambia o limpia el fondo de la tienda (ADR-38).
    pub async fn actualizar_background(
        &self,
        actor: &crate::auth::UsuarioContexto,
        identificador: &str,
        datos: &BackgroundInput,
    ) -> Result<Value, ApiError> {
        let Some(resuelta) = self.resolver(identificador).await? else {
            return Err(ApiError::no_encontrado("Tienda", identificador));
        };
        let tienda = resuelta.tienda;
        verificar_acceso(
            actor,
            &tienda,
            "Solo el dueño de la tienda puede cambiar su fondo.",
        )?;
        let actual = background_de(&tienda);
        let mut meta = normalizar_background(datos, &actual);
        validar_background(datos, &meta)?;
        let actual_movil = background_movil_de(&tienda);
        let mut meta_movil = normalizar_background_movil(datos, &actual_movil);
        validar_background_movil(datos, &meta_movil)?;
        let empresa_id = texto(&tienda, "empresa_id");

        if let Some(objeto) = &meta.background_objeto_id {
            if Some(objeto) != actual.background_objeto_id.as_ref() {
                meta.background_media_type = Some(
                    validar_media(self.storage.as_ref(), &empresa_id, objeto, UsoMedia::Fondo)
                        .await?,
                );
            }
        }
        if let Some(objeto) = &meta.background_poster_objeto_id {
            if Some(objeto) != actual.background_poster_objeto_id.as_ref() {
                validar_media(self.storage.as_ref(), &empresa_id, objeto, UsoMedia::Poster).await?;
            }
        }
        if let Some(objeto) = &meta_movil.background_movil_objeto_id {
            if Some(objeto) != actual_movil.background_movil_objeto_id.as_ref() {
                meta_movil.background_movil_media_type = Some(
                    validar_media(self.storage.as_ref(), &empresa_id, objeto, UsoMedia::Fondo)
                        .await?,
                );
            }
        }
        if let Some(objeto) = &meta_movil.background_movil_poster_objeto_id {
            if Some(objeto) != actual_movil.background_movil_poster_objeto_id.as_ref() {
                validar_media(self.storage.as_ref(), &empresa_id, objeto, UsoMedia::Poster).await?;
            }
        }

        let id = texto(&tienda, "id");
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE stores.tiendas SET
               background_objeto_id=$1::uuid, background_media_type=$2,
               background_poster_objeto_id=$3::uuid, background_alt=$4,
               background_overlay=$5::smallint, background_position=$6, background_activo=$7,
               background_movil_objeto_id=$8::uuid, background_movil_media_type=$9,
               background_movil_poster_objeto_id=$10::uuid, background_movil_alt=$11,
               background_movil_activo=$12, actualizado_en=NOW()
             WHERE id=$13::uuid RETURNING {CAMPOS_TIENDA}"
        ))
        .bind(&meta.background_objeto_id)
        .bind(&meta.background_media_type)
        .bind(&meta.background_poster_objeto_id)
        .bind(&meta.background_alt)
        .bind(meta.background_overlay)
        .bind(&meta.background_position)
        .bind(meta.background_activo)
        .bind(&meta_movil.background_movil_objeto_id)
        .bind(&meta_movil.background_movil_media_type)
        .bind(&meta_movil.background_movil_poster_objeto_id)
        .bind(&meta_movil.background_movil_alt)
        .bind(meta_movil.background_movil_activo)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Tienda", &id));
        };
        self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
            .await
    }

    /// Actualiza la identidad visual de la tienda (ADR-39).
    pub async fn actualizar_personalizacion(
        &self,
        actor: &crate::auth::UsuarioContexto,
        identificador: &str,
        datos: &PersonalizacionInput,
    ) -> Result<Value, ApiError> {
        let Some(resuelta) = self.resolver(identificador).await? else {
            return Err(ApiError::no_encontrado("Tienda", identificador));
        };
        let tienda = resuelta.tienda;
        verificar_acceso(
            actor,
            &tienda,
            "Solo el dueño de la tienda puede personalizarla.",
        )?;
        let meta = normalizar_overrides(datos, &resuelta.overrides);
        validar_overrides(&meta)?;
        if let Some(objeto) = &meta.seo_imagen_objeto_id {
            if Some(objeto) != resuelta.overrides.seo_imagen_objeto_id.as_ref() {
                validar_media(
                    self.storage.as_ref(),
                    &texto(&tienda, "empresa_id"),
                    objeto,
                    UsoMedia::Imagen,
                )
                .await?;
            }
        }
        let tema = tema::tema_parcial_json(&meta.tema);
        let id = texto(&tienda, "id");
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE stores.tiendas SET
               plantilla_override=$1, tema_override=$2::jsonb, hero_titulo=$3,
               hero_subtitulo=$4, hero_cta_texto=$5, hero_cta_url=$6, seo_titulo=$7,
               seo_descripcion=$8, seo_imagen_objeto_id=$9::uuid,
               personalizacion_actualizada_en=NOW(), actualizado_en=NOW()
             WHERE id=$10::uuid RETURNING {CAMPOS_TIENDA}"
        ))
        .bind(&meta.plantilla)
        .bind(tema.to_string())
        .bind(&meta.hero_titulo)
        .bind(&meta.hero_subtitulo)
        .bind(&meta.hero_cta_texto)
        .bind(&meta.hero_cta_url)
        .bind(&meta.seo_titulo)
        .bind(&meta.seo_descripcion)
        .bind(&meta.seo_imagen_objeto_id)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Tienda", &id));
        };
        self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
            .await
    }

    /// Actualiza el contenido del storefront (ADR-40).
    pub async fn actualizar_contenido(
        &self,
        actor: &crate::auth::UsuarioContexto,
        identificador: &str,
        datos: &ContenidoInput,
    ) -> Result<Value, ApiError> {
        let Some(resuelta) = self.resolver(identificador).await? else {
            return Err(ApiError::no_encontrado("Tienda", identificador));
        };
        let tienda = resuelta.tienda;
        verificar_acceso(
            actor,
            &tienda,
            "Solo el dueño de la tienda puede cambiar su contenido.",
        )?;
        let meta = normalizar_contenido(datos, &contenido_de(&tienda));
        validar_contenido(&meta)?;
        let id = texto(&tienda, "id");
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE stores.tiendas SET
               secciones=$1::jsonb, sobre_nosotros=$2, politicas=$3::jsonb, redes=$4::jsonb,
               anuncio_texto=$5, anuncio_enlace=$6, anuncio_activo=$7,
               contenido_actualizado_en=NOW(), actualizado_en=NOW()
             WHERE id=$8::uuid RETURNING {CAMPOS_TIENDA}"
        ))
        .bind(serde_json::to_string(&meta.secciones).map_err(crate::db::interno)?)
        .bind(&meta.sobre_nosotros)
        .bind(serde_json::to_string(&meta.politicas).map_err(crate::db::interno)?)
        .bind(serde_json::to_string(&meta.redes).map_err(crate::db::interno)?)
        .bind(&meta.anuncio_texto)
        .bind(&meta.anuncio_enlace)
        .bind(meta.anuncio_activo)
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Tienda", &id));
        };
        self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
            .await
    }

    async fn validar_logo(&self, empresa_id: &str, objeto_id: &str) -> Result<(), ApiError> {
        let objeto = match self.storage.objeto(empresa_id, objeto_id).await {
            crate::clients::ConsultaObjeto::Encontrado(objeto) => objeto,
            crate::clients::ConsultaObjeto::NoEncontrado => {
                return Err(ApiError::no_encontrado("Logo", objeto_id));
            }
            crate::clients::ConsultaObjeto::NoDisponible => {
                return Err(ApiError::dominio(
                    "LOGO_NO_VERIFICABLE",
                    "No se pudo verificar el logo.",
                ));
            }
        };
        if objeto.tenant_id != empresa_id
            || objeto.estado != "activo"
            || !objeto.publico
            || !objeto.content_type.starts_with("image/")
        {
            return Err(ApiError::dominio(
                "LOGO_INVALIDO",
                "El logo debe ser una imagen pública y activa de la empresa.",
            ));
        }
        Ok(())
    }
}
