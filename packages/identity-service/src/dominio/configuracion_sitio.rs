//! Configuración visible del sitio (branding de plataforma, ADR-38/ADR-42).
//!
//! Réplica de `configuracion-sitio/configuracion-sitio.service.ts`: el fondo y
//! el panel de IA se guardan como referencias a storage; el tema efectivo se
//! resuelve desde los overrides y las URLs se derivan en cada respuesta.

use crate::db::Db;
use crate::dominio::media::{
    background_de, background_de_con_prefijo, con_background_urls, normalizar_background,
    validar_background, BackgroundCampos, BackgroundInput,
};
use crate::dominio::slug::es_uuid;
use crate::dominio::tema::{
    es_plantilla, mezclar_tema, normalizar_tema_parcial, resolver_personalizacion, tema_parcial_de,
    validar_tema, validar_tema_input, HerenciaTema, PLANTILLAS,
};
use crate::errors::ApiError;
use crate::storage::Storage;
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;

pub struct ConfiguracionSitio {
    db: Arc<Db>,
    storage: Arc<Storage>,
    platform_tenant: Option<String>,
}

#[derive(Debug, Clone)]
struct FilaConfig {
    background: BackgroundCampos,
    panel: BackgroundCampos,
    plantilla_override: Option<String>,
    tema_override: Value,
    actualizado_en: Option<String>,
}

impl ConfiguracionSitio {
    pub fn nueva(db: Arc<Db>, storage: Arc<Storage>, platform_tenant: Option<String>) -> Self {
        Self {
            db,
            storage,
            platform_tenant,
        }
    }

    pub async fn obtener(&self) -> Result<Value, ApiError> {
        let fila = self.cargar().await?;
        Ok(Self::respuesta(&fila))
    }

    pub async fn actualizar(&self, datos: &Value, usuario_id: &str) -> Result<Value, ApiError> {
        let actual = self.cargar().await?;

        let entrada = BackgroundInput::desde(datos);
        let mut principal = normalizar_background(&entrada, Some(&actual.background));
        validar_background(&entrada, &principal)?;

        let entrada_panel = BackgroundInput::con_prefijo(datos, "panel_ai_background");
        let mut panel = normalizar_background(&entrada_panel, Some(&actual.panel));
        validar_background(&entrada_panel, &panel)?;

        if let Some(objeto) = principal.background_objeto_id.clone() {
            if Some(&objeto) != actual.background.background_objeto_id.as_ref() {
                let tenant = self.tenant()?;
                principal.background_media_type =
                    Some(self.storage.validar_media(tenant, &objeto, "fondo").await?);
            }
        } else {
            principal.background_media_type = None;
        }
        if let Some(poster) = principal.background_poster_objeto_id.clone() {
            if Some(&poster) != actual.background.background_poster_objeto_id.as_ref() {
                let tenant = self.tenant()?;
                self.storage
                    .validar_media(tenant, &poster, "poster")
                    .await?;
            }
        }
        if let Some(objeto) = panel.background_objeto_id.clone() {
            if Some(&objeto) != actual.panel.background_objeto_id.as_ref() {
                let tenant = self.tenant()?;
                panel.background_media_type =
                    Some(self.storage.validar_media(tenant, &objeto, "fondo").await?);
            }
        } else {
            panel.background_media_type = None;
        }
        if let Some(poster) = panel.background_poster_objeto_id.clone() {
            if Some(&poster) != actual.panel.background_poster_objeto_id.as_ref() {
                let tenant = self.tenant()?;
                self.storage
                    .validar_media(tenant, &poster, "poster")
                    .await?;
            }
        }

        let plantilla = match datos.get("plantilla") {
            None => actual.plantilla_override.clone(),
            Some(Value::Null) => None,
            Some(Value::String(valor)) if es_plantilla(valor) => Some(valor.clone()),
            Some(_) => {
                return Err(ApiError::dominio(
                    "CONFIGURACION_INVALIDA",
                    "La plantilla no es válida.",
                ))
            }
        };
        let tema = match datos.get("tema") {
            None => actual.tema_override.clone(),
            Some(Value::Null) => json!({}),
            Some(Value::Object(_)) => {
                validar_tema_input(datos.get("tema").expect("tema"))?;
                let mut mezcla = actual
                    .tema_override
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                if let Some(parcial) = normalizar_tema_parcial(datos.get("tema").expect("tema"))
                    .as_object()
                    .cloned()
                {
                    for (clave, valor) in parcial {
                        mezcla.insert(clave, valor);
                    }
                }
                Value::Object(mezcla)
            }
            Some(_) => {
                return Err(ApiError::dominio(
                    "CONFIGURACION_INVALIDA",
                    "El tema no es válido.",
                ))
            }
        };
        validar_tema(&mezclar_tema(&[&tema]))?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.configuracion_plataforma
               (id, background_objeto_id, background_media_type, background_poster_objeto_id,
                background_alt, background_overlay, background_position, background_activo,
                panel_ai_background_objeto_id, panel_ai_background_media_type,
                panel_ai_background_poster_objeto_id, panel_ai_background_alt,
                panel_ai_background_overlay, panel_ai_background_position,
                panel_ai_background_activo,
                plantilla_override, tema_override, actualizado_por, actualizado_en)
             VALUES (1, $1::uuid, $2, $3::uuid, $4, $5, $6, $7,
                     $8::uuid, $9, $10::uuid, $11, $12, $13, $14,
                     $15, $16::jsonb, $17::uuid, NOW())
             ON CONFLICT (id) DO UPDATE SET
               background_objeto_id = EXCLUDED.background_objeto_id,
               background_media_type = EXCLUDED.background_media_type,
               background_poster_objeto_id = EXCLUDED.background_poster_objeto_id,
               background_alt = EXCLUDED.background_alt,
               background_overlay = EXCLUDED.background_overlay,
               background_position = EXCLUDED.background_position,
               background_activo = EXCLUDED.background_activo,
               panel_ai_background_objeto_id = EXCLUDED.panel_ai_background_objeto_id,
               panel_ai_background_media_type = EXCLUDED.panel_ai_background_media_type,
               panel_ai_background_poster_objeto_id = EXCLUDED.panel_ai_background_poster_objeto_id,
               panel_ai_background_alt = EXCLUDED.panel_ai_background_alt,
               panel_ai_background_overlay = EXCLUDED.panel_ai_background_overlay,
               panel_ai_background_position = EXCLUDED.panel_ai_background_position,
               panel_ai_background_activo = EXCLUDED.panel_ai_background_activo,
               plantilla_override = EXCLUDED.plantilla_override,
               tema_override = EXCLUDED.tema_override,
               actualizado_por = EXCLUDED.actualizado_por,
               actualizado_en = NOW()
             RETURNING background_objeto_id, background_media_type,
               background_poster_objeto_id, background_alt, background_overlay,
               background_position, background_activo,
               panel_ai_background_objeto_id, panel_ai_background_media_type,
               panel_ai_background_poster_objeto_id, panel_ai_background_alt,
               panel_ai_background_overlay, panel_ai_background_position,
               panel_ai_background_activo,
               plantilla_override, tema_override, actualizado_en",
        )
        .bind(&principal.background_objeto_id)
        .bind(&principal.background_media_type)
        .bind(&principal.background_poster_objeto_id)
        .bind(&principal.background_alt)
        .bind(principal.background_overlay)
        .bind(&principal.background_position)
        .bind(principal.background_activo)
        .bind(&panel.background_objeto_id)
        .bind(&panel.background_media_type)
        .bind(&panel.background_poster_objeto_id)
        .bind(&panel.background_alt)
        .bind(panel.background_overlay)
        .bind(&panel.background_position)
        .bind(panel.background_activo)
        .bind(&plantilla)
        .bind(&tema)
        .bind(usuario_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio(
                "CONFIGURACION_NO_ACTUALIZADA",
                "No se pudo actualizar la configuración del sitio.",
            )
        })?;
        tx.commit().await.map_err(interno)?;
        Ok(Self::respuesta(&self.fila_de(&fila)?))
    }

    async fn cargar(&self) -> Result<FilaConfig, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT background_objeto_id, background_media_type,
                    background_poster_objeto_id, background_alt, background_overlay,
                    background_position, background_activo,
                    panel_ai_background_objeto_id, panel_ai_background_media_type,
                    panel_ai_background_poster_objeto_id, panel_ai_background_alt,
                    panel_ai_background_overlay, panel_ai_background_position,
                    panel_ai_background_activo,
                    plantilla_override, tema_override, actualizado_en
               FROM identity.configuracion_plataforma WHERE id = 1",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => self.fila_de(&fila),
            None => Ok(FilaConfig {
                background: BackgroundCampos::default(),
                panel: BackgroundCampos::default(),
                plantilla_override: None,
                tema_override: json!({}),
                actualizado_en: None,
            }),
        }
    }

    fn fila_de(&self, fila: &sqlx::postgres::PgRow) -> Result<FilaConfig, ApiError> {
        let valor = json!({
            "background_objeto_id": fila.try_get::<Option<uuid::Uuid>, _>("background_objeto_id").map_err(interno)?.map(|valor| valor.to_string()),
            "background_media_type": fila.try_get::<Option<String>, _>("background_media_type").map_err(interno)?,
            "background_poster_objeto_id": fila.try_get::<Option<uuid::Uuid>, _>("background_poster_objeto_id").map_err(interno)?.map(|valor| valor.to_string()),
            "background_alt": fila.try_get::<String, _>("background_alt").map_err(interno)?,
            "background_overlay": fila.try_get::<i16, _>("background_overlay").map_err(interno)?,
            "background_position": fila.try_get::<String, _>("background_position").map_err(interno)?,
            "background_activo": fila.try_get::<bool, _>("background_activo").map_err(interno)?,
            "panel_ai_background_objeto_id": fila.try_get::<Option<uuid::Uuid>, _>("panel_ai_background_objeto_id").map_err(interno)?.map(|valor| valor.to_string()),
            "panel_ai_background_media_type": fila.try_get::<Option<String>, _>("panel_ai_background_media_type").map_err(interno)?,
            "panel_ai_background_poster_objeto_id": fila.try_get::<Option<uuid::Uuid>, _>("panel_ai_background_poster_objeto_id").map_err(interno)?.map(|valor| valor.to_string()),
            "panel_ai_background_alt": fila.try_get::<String, _>("panel_ai_background_alt").map_err(interno)?,
            "panel_ai_background_overlay": fila.try_get::<i16, _>("panel_ai_background_overlay").map_err(interno)?,
            "panel_ai_background_position": fila.try_get::<String, _>("panel_ai_background_position").map_err(interno)?,
            "panel_ai_background_activo": fila.try_get::<bool, _>("panel_ai_background_activo").map_err(interno)?,
        });
        let actualizado_en: Option<DateTime<Utc>> =
            fila.try_get("actualizado_en").map_err(interno)?;
        Ok(FilaConfig {
            background: background_de(&valor),
            panel: background_de_con_prefijo(&valor, "panel_ai_background"),
            plantilla_override: fila.try_get("plantilla_override").map_err(interno)?,
            tema_override: fila.try_get("tema_override").map_err(interno)?,
            actualizado_en: actualizado_en.map(crate::db::iso_ms),
        })
    }

    fn respuesta(fila: &FilaConfig) -> Value {
        let principal = con_background_urls(fila.background.clone());
        let panel = con_background_urls(fila.panel.clone());
        let overrides = HerenciaTema {
            plantilla: fila
                .plantilla_override
                .as_deref()
                .filter(|valor| es_plantilla(valor))
                .map(str::to_string),
            tema: tema_parcial_de(&fila.tema_override),
        };
        let resuelta = resolver_personalizacion(
            &overrides,
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            "plataforma",
        );
        let mut salida = Map::new();
        salida.insert(
            "background_objeto_id".into(),
            json!(principal.campos.background_objeto_id),
        );
        salida.insert(
            "background_media_type".into(),
            json!(principal.campos.background_media_type),
        );
        salida.insert("background_url".into(), json!(principal.background_url));
        salida.insert(
            "background_poster_objeto_id".into(),
            json!(principal.campos.background_poster_objeto_id),
        );
        salida.insert(
            "background_poster_url".into(),
            json!(principal.background_poster_url),
        );
        salida.insert(
            "background_alt".into(),
            json!(principal.campos.background_alt),
        );
        salida.insert(
            "background_overlay".into(),
            json!(principal.campos.background_overlay),
        );
        salida.insert(
            "background_position".into(),
            json!(principal.campos.background_position),
        );
        salida.insert(
            "background_activo".into(),
            json!(principal.campos.background_activo),
        );
        salida.insert(
            "panel_ai_background_objeto_id".into(),
            json!(panel.campos.background_objeto_id),
        );
        salida.insert(
            "panel_ai_background_media_type".into(),
            json!(panel.campos.background_media_type),
        );
        salida.insert(
            "panel_ai_background_url".into(),
            json!(panel.background_url),
        );
        salida.insert(
            "panel_ai_background_poster_objeto_id".into(),
            json!(panel.campos.background_poster_objeto_id),
        );
        salida.insert(
            "panel_ai_background_poster_url".into(),
            json!(panel.background_poster_url),
        );
        salida.insert(
            "panel_ai_background_alt".into(),
            json!(panel.campos.background_alt),
        );
        salida.insert(
            "panel_ai_background_overlay".into(),
            json!(panel.campos.background_overlay),
        );
        salida.insert(
            "panel_ai_background_position".into(),
            json!(panel.campos.background_position),
        );
        salida.insert(
            "panel_ai_background_activo".into(),
            json!(panel.campos.background_activo),
        );
        salida.insert("plantilla".into(), json!(resuelta.plantilla));
        salida.insert(
            "tema".into(),
            serde_json::to_value(&resuelta.tema).unwrap_or_else(|_| json!({})),
        );
        salida.insert(
            "origen_personalizacion".into(),
            json!({
                "plantilla": resuelta.origen.plantilla,
                "tema": resuelta.origen.tema,
            }),
        );
        salida.insert("actualizado_en".into(), json!(fila.actualizado_en));
        Value::Object(salida)
    }

    fn tenant(&self) -> Result<&str, ApiError> {
        match self.platform_tenant.as_deref() {
            Some(tenant) if es_uuid(tenant) => Ok(tenant),
            _ => Err(ApiError::dominio(
                "PLATFORM_TENANT_ID_NO_CONFIGURADO",
                "La plataforma no tiene configurado el tenant de sus assets.",
            )),
        }
    }
}

pub fn plantillas_disponibles() -> &'static [&'static str] {
    &PLANTILLAS
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

impl ConfiguracionSitio {
    /// Overrides crudos de tema de la plataforma (ADR-42).
    pub async fn tema_overrides(&self) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT plantilla_override, tema_override
               FROM identity.configuracion_plataforma WHERE id = 1",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(json!({ "plantilla_override": Value::Null, "tema_override": {} }));
        };
        let plantilla: Option<String> = fila.try_get("plantilla_override").map_err(interno)?;
        let tema: Value = fila.try_get("tema_override").map_err(interno)?;
        Ok(json!({
            "plantilla_override": plantilla
                .as_deref()
                .filter(|valor| es_plantilla(valor)),
            "tema_override": tema_parcial_de(&tema),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_exponer_las_plantillas_del_contrato() {
        assert_eq!(plantillas_disponibles().len(), 4);
        assert!(PLANTILLAS.contains(&"boutique"));
    }

    #[test]
    fn debe_construir_una_respuesta_con_defaults_sin_fila() {
        let fila = FilaConfig {
            background: BackgroundCampos::default(),
            panel: BackgroundCampos::default(),
            plantilla_override: None,
            tema_override: json!({}),
            actualizado_en: None,
        };
        let respuesta = ConfiguracionSitio::respuesta(&fila);
        assert_eq!(respuesta["background_overlay"], 35);
        assert_eq!(respuesta["plantilla"], "clasica");
        assert_eq!(respuesta["tema"]["modo"], "claro");
        assert_eq!(respuesta["origen_personalizacion"]["tema"], "defecto");
    }
}
