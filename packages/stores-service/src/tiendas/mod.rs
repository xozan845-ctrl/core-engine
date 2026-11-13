//! Dominio de tiendas: tienda del vendedor, slugs, logo, fondos,
//! personalización y contenido del storefront.

pub mod contenido;
pub mod media;
pub mod personalizacion;
pub mod slug;
pub mod tema;

use crate::clients::{ObjetosStorage, TemasIdentity};
use crate::db::{fila_a_json, Db};
use crate::errors::ApiError;
use contenido::{
    contenido_de, contenido_json, normalizar_contenido, validar_contenido, ContenidoInput,
};
use media::{
    background_de, background_movil_de, background_movil_urls, background_urls,
    normalizar_background, normalizar_background_movil, validar_background,
    validar_background_movil, validar_media, BackgroundInput, UsoMedia,
};
use personalizacion::{
    con_seo_imagen_url, normalizar_overrides, overrides_de, personalizacion_json,
    validar_overrides, OverridesPersonalizacion, PersonalizacionInput,
};
use serde_json::{Map, Value};
use std::sync::Arc;
use tema::{plantilla_de, resolver_personalizacion, tema_parcial_de, TemaHerencia};
use uuid::Uuid;

pub const CAMPOS_TIENDA: &str = "id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, slug, logo_objeto_id, background_objeto_id, background_media_type, background_poster_objeto_id, background_alt, background_overlay, background_position, background_activo, background_movil_objeto_id, background_movil_media_type, background_movil_poster_objeto_id, background_movil_alt, background_movil_activo, plantilla_override, tema_override, hero_titulo, hero_subtitulo, hero_cta_texto, hero_cta_url, seo_titulo, seo_descripcion, seo_imagen_objeto_id, personalizacion_actualizada_en, secciones, sobre_nosotros, politicas, redes, anuncio_texto, anuncio_enlace, anuncio_activo, contenido_actualizado_en, creado_en, actualizado_en";

const MAX_INTENTOS_SLUG: i64 = 50;

#[derive(Debug, Clone)]
pub struct TiendaResuelta {
    pub tienda: Value,
    pub overrides: OverridesPersonalizacion,
    pub redirigir_a: Option<String>,
}

pub struct Tiendas {
    db: Arc<Db>,
    temas: Arc<dyn TemasIdentity>,
    storage: Arc<dyn ObjetosStorage>,
}

fn texto(fila: &Value, clave: &str) -> String {
    fila.get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_opcional(fila: &Value, clave: &str) -> Option<String> {
    fila.get(clave).and_then(Value::as_str).map(str::to_string)
}

fn entero(fila: &Value, clave: &str) -> Option<i64> {
    fila.get(clave).and_then(Value::as_i64)
}

fn booleano(fila: &Value, clave: &str) -> bool {
    fila.get(clave).and_then(Value::as_bool).unwrap_or(false)
}

fn insertar(objeto: &mut Map<String, Value>, clave: &str, valor: Value) {
    objeto.insert(clave.to_string(), valor);
}

fn opt(valor: Option<String>) -> Value {
    valor.map(Value::String).unwrap_or(Value::Null)
}

/// Base36 de milisegundos, como `Date.now().toString(36)`.
pub fn base36(mut numero: u128) -> String {
    const DIGITOS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if numero == 0 {
        return "0".into();
    }
    let mut salida = Vec::new();
    while numero > 0 {
        salida.push(DIGITOS[(numero % 36) as usize]);
        numero /= 36;
    }
    salida.reverse();
    String::from_utf8(salida).expect("base36 ascii")
}

impl Tiendas {
    pub fn nueva(
        db: Arc<Db>,
        temas: Arc<dyn TemasIdentity>,
        storage: Arc<dyn ObjetosStorage>,
    ) -> Self {
        Self { db, temas, storage }
    }

    pub async fn de_vendedor(&self, vendedor_id: &str) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA}
             FROM stores.tiendas
             WHERE vendedor_id = $1::uuid OR EXISTS (
               SELECT 1 FROM identity.asignaciones_personal ap
               WHERE ap.usuario_id = $1::uuid AND ap.tienda_id = stores.tiendas.id AND ap.estado = 'activa'
             )
             ORDER BY CASE WHEN vendedor_id = $1::uuid THEN 0 ELSE 1 END, creado_en
             LIMIT 1"
        ))
        .bind(vendedor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        match fila {
            Some(fila) => Ok(Some(
                self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
                    .await?,
            )),
            None => Ok(None),
        }
    }

    pub async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA}
             FROM stores.tiendas WHERE empresa_id = $1::uuid
             ORDER BY creado_en DESC LIMIT 200"
        ))
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(crate::db::interno)?;
            salida.push(self.con_urls(&valor).await?);
        }
        Ok(salida)
    }

    /// Resuelve por UUID o slug; conserva el slug anterior como redirección.
    pub async fn resolver(&self, identificador: &str) -> Result<Option<TiendaResuelta>, ApiError> {
        let Some(fila) = self.buscar_por_identificador(identificador).await? else {
            return Ok(None);
        };
        let es_actual = booleano(&fila, "es_actual");
        let overrides = overrides_de(&fila);
        Ok(Some(TiendaResuelta {
            tienda: self.con_urls(&fila).await?,
            overrides,
            redirigir_a: if es_actual {
                None
            } else {
                Some(format!("/api/v2/tiendas/{}", texto(&fila, "slug")))
            },
        }))
    }

    pub async fn encontrar_por_id(&self, id: &str) -> Result<Option<Value>, ApiError> {
        Ok(self.resolver(id).await?.map(|resuelta| resuelta.tienda))
    }

    /// Lote interno por ids (identity no debe leer `stores.tiendas`).
    pub async fn por_ids(&self, ids: &[String]) -> Result<Vec<Value>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids
            .iter()
            .map(|id| Uuid::parse_str(id.trim()).map_err(crate::db::interno))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA} FROM stores.tiendas WHERE id = ANY($1::uuid[])"
        ))
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(crate::db::interno)?;
            salida.push(self.con_urls(&valor).await?);
        }
        Ok(salida)
    }

    /// Asignación atómica de vendedor (solo si la tienda sigue libre).
    pub async fn asignar_vendedor(
        &self,
        tienda_id: &str,
        empresa_id: &str,
        usuario_id: &str,
        participacion: i64,
    ) -> Result<(), ApiError> {
        if !(0..=100).contains(&participacion) {
            return Err(ApiError::dominio(
                "PARTICIPACION_INVALIDA",
                "La participación debe estar entre 0 y 100.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(
            "UPDATE stores.tiendas
                SET vendedor_id = $1::uuid, propietario_usuario_id = $1::uuid,
                    participacion_porcentaje = $2::int, actualizado_en = NOW()
              WHERE id = $3::uuid AND empresa_id = $4::uuid AND vendedor_id IS NULL
            RETURNING id",
        )
        .bind(usuario_id)
        .bind(participacion)
        .bind(tienda_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        if fila.is_none() {
            return Err(ApiError::conflicto_codigo(
                "La tienda ya tiene un vendedor asignado.",
                "TIENDA_YA_ASIGNADA",
                serde_json::json!({ "tienda_id": tienda_id }),
            ));
        }
        Ok(())
    }

    /// Compensación: libera la tienda si la transacción del llamador falla.
    pub async fn desasignar_vendedor(
        &self,
        tienda_id: &str,
        usuario_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        sqlx::query(
            "UPDATE stores.tiendas
                SET vendedor_id = NULL, participacion_porcentaje = NULL, actualizado_en = NOW()
              WHERE id = $1::uuid AND vendedor_id = $2::uuid",
        )
        .bind(tienda_id)
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn crear(
        &self,
        empresa_id: &str,
        propietario_id: &str,
        participacion: Option<i64>,
        nombre: &str,
        descripcion: Option<&str>,
        slug_solicitado: Option<&str>,
        vendedor_id: Option<&str>,
    ) -> Result<Value, ApiError> {
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
                   (id, empresa_id, propietario_usuario_id, vendedor_id, participacion_porcentaje, nombre, descripcion, slug)
                 VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5::int, $6, $7, $8)
                 RETURNING {CAMPOS_TIENDA}"
            ))
            .bind(Uuid::new_v4())
            .bind(empresa_id)
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

    async fn con_urls(&self, fila: &Value) -> Result<Value, ApiError> {
        let overrides = overrides_de(fila);
        let empresa_cruda = self.temas.obtener_empresa(&texto(fila, "empresa_id")).await;
        let plataforma_cruda = self.temas.obtener_plataforma().await;
        let empresa = TemaHerencia {
            plantilla: plantilla_de(&empresa_cruda.plantilla_override),
            tema: tema_parcial_de(&empresa_cruda.tema_override),
        };
        let plataforma = TemaHerencia {
            plantilla: plantilla_de(&plataforma_cruda.plantilla_override),
            tema: tema_parcial_de(&plataforma_cruda.tema_override),
        };
        let overrides_tema = TemaHerencia {
            plantilla: overrides.plantilla.clone(),
            tema: overrides.tema.clone(),
        };
        let resuelta = resolver_personalizacion(&overrides_tema, &empresa, &plataforma);
        let background = background_de(fila);
        let movil = background_movil_de(fila);
        let contenido = contenido_de(fila);

        // El orden de claves replica el spread de objetos de `conUrls`: se
        // parte de la fila (columnas en el orden del SELECT), se reemplazan los
        // campos normalizados en su sitio y las claves nuevas se añaden al
        // final en el mismo orden que TypeScript.
        let mut objeto = fila.as_object().cloned().unwrap_or_default();
        objeto.remove("plantilla_override");
        objeto.remove("tema_override");
        objeto.remove("es_actual");

        insertar(
            &mut objeto,
            "background_objeto_id",
            opt(background.background_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_media_type",
            opt(background.background_media_type.clone()),
        );
        insertar(
            &mut objeto,
            "background_poster_objeto_id",
            opt(background.background_poster_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_alt",
            Value::String(background.background_alt.clone()),
        );
        insertar(
            &mut objeto,
            "background_overlay",
            Value::from(background.background_overlay),
        );
        insertar(
            &mut objeto,
            "background_position",
            Value::String(background.background_position.clone()),
        );
        insertar(
            &mut objeto,
            "background_activo",
            Value::Bool(background.background_activo),
        );
        insertar(
            &mut objeto,
            "background_movil_objeto_id",
            opt(movil.background_movil_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_media_type",
            opt(movil.background_movil_media_type.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_poster_objeto_id",
            opt(movil.background_movil_poster_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_alt",
            Value::String(movil.background_movil_alt.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_activo",
            Value::Bool(movil.background_movil_activo),
        );
        for (clave, valor) in personalizacion_json(&overrides, &resuelta) {
            if clave == "plantilla" || clave == "tema" {
                continue;
            }
            insertar(&mut objeto, &clave, valor);
        }
        if let Some(campos) = contenido_json(&contenido).as_object().cloned() {
            for (clave, valor) in campos {
                insertar(&mut objeto, &clave, valor);
            }
        }
        if texto_opcional(fila, "logo_objeto_id").is_some() {
            insertar(
                &mut objeto,
                "logo_url",
                Value::String(format!(
                    "/api/v2/storage/public/objetos/{}",
                    texto(fila, "logo_objeto_id")
                )),
            );
        }
        for (clave, valor) in background_urls(&background)
            .as_object()
            .cloned()
            .unwrap_or_default()
        {
            insertar(&mut objeto, &clave, valor);
        }
        for (clave, valor) in background_movil_urls(&movil)
            .as_object()
            .cloned()
            .unwrap_or_default()
        {
            insertar(&mut objeto, &clave, valor);
        }
        insertar(
            &mut objeto,
            "plantilla",
            Value::String(resuelta.plantilla.clone()),
        );
        insertar(
            &mut objeto,
            "tema",
            serde_json::to_value(&resuelta.tema).map_err(crate::db::interno)?,
        );
        insertar(
            &mut objeto,
            "seo_imagen_url",
            opt(con_seo_imagen_url(&overrides.seo_imagen_objeto_id)),
        );
        insertar(
            &mut objeto,
            "origen_personalizacion",
            serde_json::json!({
                "plantilla": resuelta.origen_plantilla,
                "tema": resuelta.origen_tema,
            }),
        );
        Ok(Value::Object(objeto))
    }

    async fn buscar_por_identificador(
        &self,
        identificador: &str,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = if slug::es_uuid(identificador) {
            sqlx::query("SELECT t.*, true AS es_actual FROM stores.tiendas t WHERE t.id = $1::uuid")
                .bind(identificador)
                .fetch_optional(&mut *tx)
                .await
        } else {
            sqlx::query(
                "SELECT t.*, (s.reemplazado_en IS NULL) AS es_actual
                   FROM stores.tienda_slugs s
                   JOIN stores.tiendas t ON t.id = s.tienda_id
                  WHERE s.slug = $1",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
        }
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        fila.map(|fila| fila_a_json(&fila).map_err(crate::db::interno))
            .transpose()
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

    async fn slug_libre(&self, slug: &str) -> Result<bool, ApiError> {
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

/// Acceso a logo/fondo/personalización/contenido.
pub fn verificar_acceso(
    actor: &crate::auth::UsuarioContexto,
    tienda: &Value,
    mensaje: &str,
) -> Result<(), ApiError> {
    let es_global = matches!(actor.rol.as_str(), "super_admin" | "admin");
    let es_empresario = actor.rol == "empresario"
        && actor
            .tenant_id
            .as_deref()
            .is_some_and(|tenant| tenant == texto(tienda, "empresa_id"));
    let es_vendedor = actor.rol == "vendedor"
        && (texto(tienda, "propietario_usuario_id") == actor.user_id
            || texto(tienda, "vendedor_id") == actor.user_id);
    if es_global || es_empresario || es_vendedor {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado(mensaje))
    }
}

pub fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

/// Resumen serializable usado por las pruebas y por la respuesta interna.
pub fn resumen(tienda: &Value) -> Value {
    serde_json::json!({
        "id": tienda.get("id"),
        "slug": tienda.get("slug"),
        "nombre": tienda.get("nombre"),
        "empresa_id": tienda.get("empresa_id"),
        "vendedor_id": tienda.get("vendedor_id"),
        "origen_personalizacion": tienda.get("origen_personalizacion"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn actor(rol: &str, user: &str, tenant: Option<&str>) -> crate::auth::UsuarioContexto {
        crate::auth::UsuarioContexto {
            user_id: user.into(),
            rol: rol.into(),
            tenant_id: tenant.map(str::to_string),
            ..Default::default()
        }
    }

    fn tienda() -> Value {
        json!({
            "empresa_id": "e-1",
            "propietario_usuario_id": "u-1",
            "vendedor_id": "u-2",
        })
    }

    #[test]
    fn debe_autorizar_globales_empresario_del_tenant_y_vendedor_asignado() {
        assert!(verificar_acceso(&actor("admin", "x", None), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("empresario", "x", Some("e-1")), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("vendedor", "u-1", None), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("vendedor", "u-2", None), &tienda(), "no").is_ok());
        let error = verificar_acceso(&actor("empresario", "x", Some("e-2")), &tienda(), "no")
            .expect_err("403");
        assert_eq!(error.codigo, "ACCESO_DENEGADO");
        assert!(verificar_acceso(&actor("vendedor", "u-3", None), &tienda(), "no").is_err());
    }

    #[test]
    fn debe_calcular_base36_como_javascript() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1_700_000_000_000), "loyw3v28");
    }

    #[test]
    fn debe_detectar_la_violacion_de_unicidad() {
        let normal = sqlx::Error::RowNotFound;
        assert!(!es_violacion_unicidad(&normal));
    }
}
