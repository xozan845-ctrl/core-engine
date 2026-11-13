//! Fondo de imagen/video compartido por plataforma, empresa y tienda
//! (ADR-38/ADR-43).
//!
//! Réplica de `media-background.ts`: el binario vive en `storage-service`;
//! identity solo persiste la referencia y deriva las URLs públicas.

use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const BACKGROUND_IMAGENES: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];
pub const BACKGROUND_VIDEOS: [&str; 2] = ["video/mp4", "video/webm"];
pub const BACKGROUND_POSICIONES: [&str; 5] = ["center", "top", "bottom", "left", "right"];
pub const ERROR_INVALIDO: &str = "BACKGROUND_INVALIDO";
pub const ERROR_NO_VERIFICABLE: &str = "BACKGROUND_NO_VERIFICABLE";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackgroundCampos {
    pub background_objeto_id: Option<String>,
    pub background_media_type: Option<String>,
    pub background_poster_objeto_id: Option<String>,
    pub background_alt: String,
    pub background_overlay: i64,
    pub background_position: String,
    pub background_activo: bool,
}

impl Default for BackgroundCampos {
    fn default() -> Self {
        Self {
            background_objeto_id: None,
            background_media_type: None,
            background_poster_objeto_id: None,
            background_alt: String::new(),
            background_overlay: 35,
            background_position: "center".to_string(),
            background_activo: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BackgroundConUrls {
    #[serde(flatten)]
    pub campos: BackgroundCampos,
    pub background_url: Option<String>,
    pub background_poster_url: Option<String>,
}

/// `None` = ausente; `Some(None)` = null explícito; `Some(Some(x))` = valor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BackgroundInput {
    pub background_objeto_id: Option<Option<String>>,
    pub background_poster_objeto_id: Option<Option<String>>,
    pub background_alt: Option<String>,
    pub background_overlay: Option<i64>,
    pub background_position: Option<String>,
    pub background_activo: Option<bool>,
}

impl BackgroundInput {
    pub fn desde(valor: &Value) -> Self {
        let leer_objeto = |clave: &str| match valor.get(clave) {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(texto)) => Some(Some(texto.trim().to_string())),
            Some(_) => None,
        };
        Self {
            background_objeto_id: leer_objeto("background_objeto_id"),
            background_poster_objeto_id: leer_objeto("background_poster_objeto_id"),
            background_alt: valor
                .get("background_alt")
                .and_then(Value::as_str)
                .map(str::to_string),
            background_overlay: valor.get("background_overlay").and_then(Value::as_i64),
            background_position: valor
                .get("background_position")
                .and_then(Value::as_str)
                .map(str::to_string),
            background_activo: valor.get("background_activo").and_then(Value::as_bool),
        }
    }

    pub fn con_prefijo(valor: &Value, prefijo: &str) -> Self {
        let Some(objeto) = valor.as_object() else {
            return Self::default();
        };
        let mut datos = serde_json::Map::new();
        for campo in [
            "objeto_id",
            "poster_objeto_id",
            "alt",
            "overlay",
            "position",
            "activo",
        ] {
            if let Some(valor) = objeto.get(&format!("{prefijo}_{campo}")) {
                datos.insert(campo.to_string(), valor.clone());
            }
        }
        Self::desde(&Value::Object(datos))
    }
}

pub fn background_de(valor: &Value) -> BackgroundCampos {
    leer_campos(valor, "background")
}

pub fn background_de_con_prefijo(valor: &Value, prefijo: &str) -> BackgroundCampos {
    leer_campos(valor, prefijo)
}

fn leer_campos(valor: &Value, prefijo: &str) -> BackgroundCampos {
    let Some(objeto) = valor.as_object() else {
        return BackgroundCampos::default();
    };
    let leer = |campo: &str| objeto.get(&format!("{prefijo}_{campo}"));
    BackgroundCampos {
        background_objeto_id: leer("objeto_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        background_media_type: leer("media_type")
            .and_then(Value::as_str)
            .map(str::to_string),
        background_poster_objeto_id: leer("poster_objeto_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        background_alt: leer("alt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        background_overlay: leer("overlay").and_then(Value::as_i64).unwrap_or(35),
        background_position: leer("position")
            .and_then(Value::as_str)
            .unwrap_or("center")
            .to_string(),
        background_activo: leer("activo").and_then(Value::as_bool).unwrap_or(false),
    }
}

pub fn normalizar_background(
    datos: &BackgroundInput,
    actual: Option<&BackgroundCampos>,
) -> BackgroundCampos {
    let base = actual.cloned().unwrap_or_default();
    // Solo `null` explícito limpia todo el bloque.
    if matches!(datos.background_objeto_id, Some(None)) {
        return BackgroundCampos::default();
    }
    let objeto_id = match &datos.background_objeto_id {
        Some(Some(valor)) => Some(valor.clone()),
        Some(None) => None,
        None => base.background_objeto_id.clone(),
    };
    let mismo_objeto = objeto_id == base.background_objeto_id;
    BackgroundCampos {
        background_objeto_id: objeto_id,
        background_media_type: if mismo_objeto {
            base.background_media_type
        } else {
            None
        },
        background_poster_objeto_id: match &datos.background_poster_objeto_id {
            Some(Some(valor)) => Some(valor.clone()),
            Some(None) => None,
            None => base.background_poster_objeto_id.clone(),
        },
        background_alt: datos
            .background_alt
            .as_deref()
            .map(str::trim)
            .unwrap_or(&base.background_alt)
            .to_string(),
        background_overlay: datos.background_overlay.unwrap_or(base.background_overlay),
        background_position: datos
            .background_position
            .clone()
            .unwrap_or(base.background_position),
        background_activo: datos.background_activo.unwrap_or(base.background_activo),
    }
}

pub fn validar_background(
    datos: &BackgroundInput,
    meta: &BackgroundCampos,
) -> Result<(), ApiError> {
    if meta.background_objeto_id.is_none() {
        if datos.background_activo == Some(true) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "No se puede activar un fondo sin objeto.",
            ));
        }
        if matches!(datos.background_poster_objeto_id, Some(Some(_))) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "El poster requiere un objeto de fondo.",
            ));
        }
    }
    if let Some(objeto) = &meta.background_objeto_id {
        if !crate::dominio::slug::es_uuid(objeto) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "El fondo debe ser un objeto de storage válido.",
            ));
        }
    }
    if let Some(poster) = &meta.background_poster_objeto_id {
        if !crate::dominio::slug::es_uuid(poster) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "El poster debe ser un objeto de storage válido.",
            ));
        }
    }
    if meta.background_alt.chars().count() > 160 {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "El texto alternativo supera 160 caracteres.",
        ));
    }
    if !(0..=100).contains(&meta.background_overlay) {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "El overlay debe ser un entero entre 0 y 100.",
        ));
    }
    if !BACKGROUND_POSICIONES.contains(&meta.background_position.as_str()) {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "La posición del fondo no es válida.",
        ));
    }
    Ok(())
}

pub fn con_background_urls(campos: BackgroundCampos) -> BackgroundConUrls {
    let background_url = campos
        .background_objeto_id
        .as_ref()
        .map(|id| format!("/api/v2/storage/public/objetos/{id}"));
    let background_poster_url = campos
        .background_poster_objeto_id
        .as_ref()
        .map(|id| format!("/api/v2/storage/public/objetos/{id}"));
    BackgroundConUrls {
        campos,
        background_url,
        background_poster_url,
    }
}

/// Objeto de respuesta para bloques prefijados (`panel_ai_background_*`).
pub fn background_a_json(campos: &BackgroundCampos, prefijo: Option<&str>) -> Value {
    let clave = |nombre: &str| match prefijo {
        Some(prefijo) => format!("{prefijo}_{nombre}"),
        None => nombre.to_string(),
    };
    json!({
        clave("objeto_id"): campos.background_objeto_id,
        clave("media_type"): campos.background_media_type,
        clave("poster_objeto_id"): campos.background_poster_objeto_id,
        clave("alt"): campos.background_alt,
        clave("overlay"): campos.background_overlay,
        clave("position"): campos.background_position,
        clave("activo"): campos.background_activo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_completar_los_defaults_de_una_fila_vacia() {
        let campos = background_de(&json!({}));
        assert_eq!(campos.background_overlay, 35);
        assert_eq!(campos.background_position, "center");
        assert!(!campos.background_activo);
        assert!(campos.background_objeto_id.is_none());
    }

    #[test]
    fn debe_limpiar_el_bloque_con_null_explicito() {
        let actual = BackgroundCampos {
            background_objeto_id: Some("11111111-1111-4111-8111-111111111111".into()),
            background_media_type: Some("imagen".into()),
            ..Default::default()
        };
        let datos = BackgroundInput {
            background_objeto_id: Some(None),
            ..Default::default()
        };
        let meta = normalizar_background(&datos, Some(&actual));
        assert!(meta.background_objeto_id.is_none());
        assert!(meta.background_media_type.is_none());
    }

    #[test]
    fn debe_invalidar_un_fondo_activo_sin_objeto() {
        let datos = BackgroundInput {
            background_activo: Some(true),
            ..Default::default()
        };
        let meta = normalizar_background(&datos, None);
        assert_eq!(
            validar_background(&datos, &meta)
                .expect_err("invalido")
                .codigo,
            ERROR_INVALIDO
        );
    }

    #[test]
    fn debe_derivar_las_urls_publicas() {
        let campos = BackgroundCampos {
            background_objeto_id: Some("abc".into()),
            background_poster_objeto_id: Some("def".into()),
            ..Default::default()
        };
        let con_urls = con_background_urls(campos);
        assert_eq!(
            con_urls.background_url.as_deref(),
            Some("/api/v2/storage/public/objetos/abc")
        );
        assert_eq!(
            con_urls.background_poster_url.as_deref(),
            Some("/api/v2/storage/public/objetos/def")
        );
    }

    #[test]
    fn debe_leer_un_bloque_prefijado() {
        let campos = background_de_con_prefijo(
            &json!({"panel_ai_background_objeto_id": "abc", "panel_ai_background_overlay": 10}),
            "panel_ai_background",
        );
        assert_eq!(campos.background_objeto_id.as_deref(), Some("abc"));
        assert_eq!(campos.background_overlay, 10);
    }
}
