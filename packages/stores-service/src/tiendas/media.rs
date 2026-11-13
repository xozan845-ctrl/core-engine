//! Contrato común de media background (ADR-38/ADR-41) para tiendas.
//!
//! El binario vive en `storage-service`; stores solo guarda la referencia y
//! deriva el tipo y las URLs. La validación usa la metadata interna.

use super::slug::es_uuid;
use crate::clients::{ConsultaObjeto, ObjetosStorage};
use crate::errors::ApiError;
use serde::Serialize;
use serde_json::Value;

pub const BACKGROUND_IMAGENES: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];
pub const BACKGROUND_VIDEOS: [&str; 2] = ["video/mp4", "video/webm"];
pub const BACKGROUND_POSICIONES: [&str; 5] = ["center", "top", "bottom", "left", "right"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
            background_position: "center".into(),
            background_activo: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct BackgroundMovilCampos {
    pub background_movil_objeto_id: Option<String>,
    pub background_movil_media_type: Option<String>,
    pub background_movil_poster_objeto_id: Option<String>,
    pub background_movil_alt: String,
    pub background_movil_activo: bool,
}

fn texto_opcional(valor: Option<&Value>) -> Option<String> {
    valor.and_then(Value::as_str).map(str::to_string)
}

/// Completa los campos de fondo de una fila.
pub fn background_de(fila: &Value) -> BackgroundCampos {
    let Some(objeto) = fila.as_object() else {
        return BackgroundCampos::default();
    };
    BackgroundCampos {
        background_objeto_id: texto_opcional(objeto.get("background_objeto_id")),
        background_media_type: texto_opcional(objeto.get("background_media_type")),
        background_poster_objeto_id: texto_opcional(objeto.get("background_poster_objeto_id")),
        background_alt: objeto
            .get("background_alt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        background_overlay: objeto
            .get("background_overlay")
            .and_then(Value::as_i64)
            .unwrap_or(35),
        background_position: objeto
            .get("background_position")
            .and_then(Value::as_str)
            .unwrap_or("center")
            .to_string(),
        background_activo: objeto
            .get("background_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

pub fn background_movil_de(fila: &Value) -> BackgroundMovilCampos {
    let Some(objeto) = fila.as_object() else {
        return BackgroundMovilCampos::default();
    };
    BackgroundMovilCampos {
        background_movil_objeto_id: texto_opcional(objeto.get("background_movil_objeto_id")),
        background_movil_media_type: texto_opcional(objeto.get("background_movil_media_type")),
        background_movil_poster_objeto_id: texto_opcional(
            objeto.get("background_movil_poster_objeto_id"),
        ),
        background_movil_alt: objeto
            .get("background_movil_alt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        background_movil_activo: objeto
            .get("background_movil_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Entrada de fondo: cada campo distingue ausente de `null`.
#[derive(Debug, Clone, Default)]
pub struct BackgroundInput {
    pub background_objeto_id: Option<Value>,
    pub background_poster_objeto_id: Option<Value>,
    pub background_alt: Option<String>,
    pub background_overlay: Option<i64>,
    pub background_position: Option<String>,
    pub background_activo: Option<bool>,
    pub background_movil_objeto_id: Option<Value>,
    pub background_movil_poster_objeto_id: Option<Value>,
    pub background_movil_alt: Option<String>,
    pub background_movil_activo: Option<bool>,
}

impl BackgroundInput {
    pub fn desde_json(cuerpo: &Value) -> Self {
        let Some(objeto) = cuerpo.as_object() else {
            return Self::default();
        };
        let cadena = |clave: &str| {
            objeto
                .get(clave)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        Self {
            background_objeto_id: objeto.get("background_objeto_id").cloned(),
            background_poster_objeto_id: objeto.get("background_poster_objeto_id").cloned(),
            background_alt: cadena("background_alt"),
            background_overlay: objeto.get("background_overlay").and_then(Value::as_i64),
            background_position: cadena("background_position"),
            background_activo: objeto.get("background_activo").and_then(Value::as_bool),
            background_movil_objeto_id: objeto.get("background_movil_objeto_id").cloned(),
            background_movil_poster_objeto_id: objeto
                .get("background_movil_poster_objeto_id")
                .cloned(),
            background_movil_alt: cadena("background_movil_alt"),
            background_movil_activo: objeto
                .get("background_movil_activo")
                .and_then(Value::as_bool),
        }
    }
}

fn valor_opcional(valor: &Option<Value>, base: &Option<String>) -> Option<String> {
    match valor {
        None => base.clone(),
        Some(Value::Null) => None,
        Some(Value::String(texto)) => Some(texto.clone()),
        Some(_) => base.clone(),
    }
}

/// Mezcla parcial del fondo. `null` en el objeto limpia todo el bloque; un
/// objeto nuevo deja `background_media_type` a `None` hasta validar el binario.
pub fn normalizar_background(
    datos: &BackgroundInput,
    actual: &BackgroundCampos,
) -> BackgroundCampos {
    if matches!(datos.background_objeto_id, Some(Value::Null)) {
        return BackgroundCampos::default();
    }
    let objeto = valor_opcional(&datos.background_objeto_id, &actual.background_objeto_id);
    let mismo_objeto = objeto == actual.background_objeto_id;
    BackgroundCampos {
        background_media_type: if mismo_objeto {
            actual.background_media_type.clone()
        } else {
            None
        },
        background_objeto_id: objeto,
        background_poster_objeto_id: valor_opcional(
            &datos.background_poster_objeto_id,
            &actual.background_poster_objeto_id,
        ),
        background_alt: datos
            .background_alt
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.background_alt.clone()),
        background_overlay: datos
            .background_overlay
            .unwrap_or(actual.background_overlay),
        background_position: datos
            .background_position
            .clone()
            .unwrap_or_else(|| actual.background_position.clone()),
        background_activo: datos.background_activo.unwrap_or(actual.background_activo),
    }
}

pub fn normalizar_background_movil(
    datos: &BackgroundInput,
    actual: &BackgroundMovilCampos,
) -> BackgroundMovilCampos {
    if matches!(datos.background_movil_objeto_id, Some(Value::Null)) {
        return BackgroundMovilCampos::default();
    }
    let objeto = valor_opcional(
        &datos.background_movil_objeto_id,
        &actual.background_movil_objeto_id,
    );
    let mismo_objeto = objeto == actual.background_movil_objeto_id;
    BackgroundMovilCampos {
        background_movil_media_type: if mismo_objeto {
            actual.background_movil_media_type.clone()
        } else {
            None
        },
        background_movil_objeto_id: objeto,
        background_movil_poster_objeto_id: valor_opcional(
            &datos.background_movil_poster_objeto_id,
            &actual.background_movil_poster_objeto_id,
        ),
        background_movil_alt: datos
            .background_movil_alt
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.background_movil_alt.clone()),
        background_movil_activo: datos
            .background_movil_activo
            .unwrap_or(actual.background_movil_activo),
    }
}

fn invalido(mensaje: impl Into<String>) -> ApiError {
    ApiError::dominio("BACKGROUND_INVALIDO", mensaje)
}

fn uuid_valido(valor: &Option<String>) -> bool {
    valor.as_ref().map(|id| es_uuid(id)).unwrap_or(true)
}

/// Reglas de dominio que el DTO no puede garantizar por sí solo.
pub fn validar_background(
    datos: &BackgroundInput,
    meta: &BackgroundCampos,
) -> Result<(), ApiError> {
    if meta.background_objeto_id.is_none() {
        if datos.background_activo == Some(true) {
            return Err(invalido("No se puede activar un fondo sin objeto."));
        }
        if matches!(&datos.background_poster_objeto_id, Some(valor) if !valor.is_null()) {
            return Err(invalido("El poster requiere un objeto de fondo."));
        }
    }
    if !uuid_valido(&meta.background_objeto_id) {
        return Err(invalido("El fondo debe ser un objeto de storage válido."));
    }
    if !uuid_valido(&meta.background_poster_objeto_id) {
        return Err(invalido("El poster debe ser un objeto de storage válido."));
    }
    if meta.background_alt.chars().count() > 160 {
        return Err(invalido("El texto alternativo supera 160 caracteres."));
    }
    if !(0..=100).contains(&meta.background_overlay) {
        return Err(invalido("El overlay debe ser un entero entre 0 y 100."));
    }
    if !BACKGROUND_POSICIONES.contains(&meta.background_position.as_str()) {
        return Err(invalido("La posición del fondo no es válida."));
    }
    Ok(())
}

pub fn validar_background_movil(
    datos: &BackgroundInput,
    meta: &BackgroundMovilCampos,
) -> Result<(), ApiError> {
    if meta.background_movil_objeto_id.is_none() {
        if datos.background_movil_activo == Some(true) {
            return Err(invalido("No se puede activar un fondo móvil sin objeto."));
        }
        if matches!(&datos.background_movil_poster_objeto_id, Some(valor) if !valor.is_null()) {
            return Err(invalido(
                "El poster móvil requiere un objeto de fondo móvil.",
            ));
        }
    }
    if !uuid_valido(&meta.background_movil_objeto_id) {
        return Err(invalido(
            "El fondo móvil debe ser un objeto de storage válido.",
        ));
    }
    if !uuid_valido(&meta.background_movil_poster_objeto_id) {
        return Err(invalido(
            "El poster móvil debe ser un objeto de storage válido.",
        ));
    }
    if meta.background_movil_alt.chars().count() > 160 {
        return Err(invalido(
            "El texto alternativo móvil supera 160 caracteres.",
        ));
    }
    Ok(())
}

pub fn background_urls(campos: &BackgroundCampos) -> Value {
    serde_json::json!({
        "background_url": campos.background_objeto_id.as_ref().map(|id| format!("/api/v2/storage/public/objetos/{id}")),
        "background_poster_url": campos.background_poster_objeto_id.as_ref().map(|id| format!("/api/v2/storage/public/objetos/{id}")),
    })
}

pub fn background_movil_urls(campos: &BackgroundMovilCampos) -> Value {
    serde_json::json!({
        "background_movil_url": campos.background_movil_objeto_id.as_ref().map(|id| format!("/api/v2/storage/public/objetos/{id}")),
        "background_movil_poster_url": campos.background_movil_poster_objeto_id.as_ref().map(|id| format!("/api/v2/storage/public/objetos/{id}")),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsoMedia {
    Fondo,
    Poster,
    Imagen,
}

impl UsoMedia {
    fn permitidos(self) -> Vec<&'static str> {
        match self {
            UsoMedia::Fondo => BACKGROUND_IMAGENES
                .iter()
                .chain(BACKGROUND_VIDEOS.iter())
                .copied()
                .collect(),
            UsoMedia::Poster | UsoMedia::Imagen => BACKGROUND_IMAGENES.to_vec(),
        }
    }
}

/// Valida un objeto contra storage y devuelve el tipo derivado.
pub async fn validar_media(
    storage: &dyn ObjetosStorage,
    tenant_id: &str,
    objeto_id: &str,
    uso: UsoMedia,
) -> Result<String, ApiError> {
    let objeto = match storage.objeto(tenant_id, objeto_id).await {
        ConsultaObjeto::Encontrado(objeto) => objeto,
        ConsultaObjeto::NoEncontrado => {
            let recurso = match uso {
                UsoMedia::Poster => "Poster",
                UsoMedia::Imagen => "Imagen",
                UsoMedia::Fondo => "Fondo",
            };
            return Err(ApiError::no_encontrado(recurso, objeto_id));
        }
        ConsultaObjeto::NoDisponible => {
            return Err(ApiError::dominio(
                "BACKGROUND_NO_VERIFICABLE",
                "No se pudo verificar la media del fondo.",
            ));
        }
    };
    let content_type = objeto.content_type.to_lowercase();
    if objeto.tenant_id != tenant_id
        || objeto.estado != "activo"
        || !objeto.publico
        || !uso.permitidos().contains(&content_type.as_str())
    {
        let mensaje = match uso {
            UsoMedia::Poster => "El poster debe ser una imagen pública y activa del mismo tenant.",
            UsoMedia::Imagen => "La imagen debe ser una imagen pública y activa del mismo tenant.",
            UsoMedia::Fondo => {
                "El fondo debe ser una imagen o video público y activo del mismo tenant."
            }
        };
        return Err(invalido(mensaje));
    }
    Ok(if content_type.starts_with("video/") {
        "video".to_string()
    } else {
        "imagen".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct StorageFijo(ConsultaObjeto);

    #[async_trait]
    impl ObjetosStorage for StorageFijo {
        async fn objeto(&self, _tenant_id: &str, _objeto_id: &str) -> ConsultaObjeto {
            self.0.clone()
        }
    }

    fn entrada(valor: Value) -> BackgroundInput {
        BackgroundInput::desde_json(&valor)
    }

    #[test]
    fn debe_limpiar_el_bloque_con_null_y_conservar_el_tipo_si_es_el_mismo() {
        let base = BackgroundCampos {
            background_objeto_id: Some("abc".into()),
            background_media_type: Some("video".into()),
            background_overlay: 50,
            ..Default::default()
        };
        let mismo = normalizar_background(&entrada(json!({"background_alt": " x "})), &base);
        assert_eq!(mismo.background_media_type.as_deref(), Some("video"));
        assert_eq!(mismo.background_overlay, 50);
        assert_eq!(mismo.background_alt, "x");

        let nuevo = normalizar_background(&entrada(json!({"background_objeto_id": "otro"})), &base);
        assert_eq!(nuevo.background_media_type, None);

        let limpio = normalizar_background(&entrada(json!({"background_objeto_id": null})), &base);
        assert_eq!(limpio, BackgroundCampos::default());
    }

    #[test]
    fn debe_validar_las_reglas_del_contrato() {
        let base = BackgroundCampos::default();
        let activo_sin_objeto = entrada(json!({"background_activo": true}));
        assert_eq!(
            validar_background(
                &activo_sin_objeto,
                &normalizar_background(&activo_sin_objeto, &base)
            )
            .expect_err("activo")
            .codigo,
            "BACKGROUND_INVALIDO"
        );
        let poster_sin_fondo =
            entrada(json!({"background_poster_objeto_id": "130c07c6-7e10-4f91-9c13-e17ca6be6eea"}));
        assert!(validar_background(
            &poster_sin_fondo,
            &normalizar_background(&poster_sin_fondo, &base)
        )
        .is_err());

        let overlay = BackgroundCampos {
            background_overlay: 101,
            ..Default::default()
        };
        assert!(validar_background(&entrada(json!({})), &overlay).is_err());

        let movil_activo = entrada(json!({"background_movil_activo": true}));
        assert!(validar_background_movil(
            &movil_activo,
            &normalizar_background_movil(&movil_activo, &BackgroundMovilCampos::default())
        )
        .is_err());
    }

    #[tokio::test]
    async fn debe_derivar_el_tipo_y_traducir_los_errores_de_storage() {
        let encontrado = StorageFijo(ConsultaObjeto::Encontrado(crate::clients::ObjetoExterno {
            id: "o-1".into(),
            tenant_id: "t-1".into(),
            estado: "activo".into(),
            publico: true,
            content_type: "video/mp4".into(),
        }));
        assert_eq!(
            validar_media(&encontrado, "t-1", "o-1", UsoMedia::Fondo)
                .await
                .expect("media"),
            "video"
        );
        assert!(validar_media(&encontrado, "t-1", "o-1", UsoMedia::Poster)
            .await
            .is_err());

        let no_encontrado = StorageFijo(ConsultaObjeto::NoEncontrado);
        let error = validar_media(&no_encontrado, "t-1", "o-1", UsoMedia::Fondo)
            .await
            .expect_err("404");
        assert_eq!(error.codigo, "NO_ENCONTRADO");
        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);

        let caido = StorageFijo(ConsultaObjeto::NoDisponible);
        assert_eq!(
            validar_media(&caido, "t-1", "o-1", UsoMedia::Fondo)
                .await
                .expect_err("caido")
                .codigo,
            "BACKGROUND_NO_VERIFICABLE"
        );
    }

    #[test]
    fn debe_derivar_las_urls_publicas() {
        let campos = BackgroundCampos {
            background_objeto_id: Some("a".into()),
            background_poster_objeto_id: None,
            ..Default::default()
        };
        assert_eq!(
            background_urls(&campos)["background_url"],
            json!("/api/v2/storage/public/objetos/a")
        );
        assert_eq!(
            background_urls(&campos)["background_poster_url"],
            Value::Null
        );
    }
}
