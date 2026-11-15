//! Personalización visual de tienda (ADR-39/ADR-42): hero, SEO y overrides.
//!
//! El contrato del tema vive en `tema.rs`; este archivo conserva los overrides
//! crudos de la tienda y la resolución efectiva.

use super::tema::{
    es_plantilla, mezclar_tema, normalizar_tema_parcial, plantilla_de, tema_parcial_de,
    validar_tema, TemaParcial,
};
use crate::errors::ApiError;
use serde_json::{Map, Value};

pub const LIMITE_HERO_TITULO: usize = 80;
pub const LIMITE_HERO_SUBTITULO: usize = 160;
pub const LIMITE_HERO_CTA_TEXTO: usize = 40;
pub const LIMITE_HERO_CTA_URL: usize = 300;
pub const LIMITE_SEO_TITULO: usize = 70;
pub const LIMITE_SEO_DESCRIPCION: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverridesPersonalizacion {
    pub plantilla: Option<String>,
    pub tema: TemaParcial,
    pub hero_titulo: String,
    pub hero_subtitulo: String,
    pub hero_cta_texto: String,
    pub hero_cta_url: String,
    pub seo_titulo: String,
    pub seo_descripcion: String,
    pub seo_imagen_objeto_id: Option<String>,
    pub personalizacion_actualizada_en: Option<String>,
}

fn texto(valor: Option<&Value>) -> String {
    valor
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn texto_opcional(valor: Option<&Value>) -> Option<String> {
    valor.and_then(Value::as_str).map(str::to_string)
}

/// Overrides crudos de la tienda: `null`/vacío significa heredar.
pub fn overrides_de(fila: &Value) -> OverridesPersonalizacion {
    let Some(objeto) = fila.as_object() else {
        return OverridesPersonalizacion::default();
    };
    OverridesPersonalizacion {
        plantilla: plantilla_de(objeto.get("plantilla_override").unwrap_or(&Value::Null)),
        tema: tema_parcial_de(objeto.get("tema_override").unwrap_or(&Value::Null)),
        hero_titulo: texto(objeto.get("hero_titulo")),
        hero_subtitulo: texto(objeto.get("hero_subtitulo")),
        hero_cta_texto: texto(objeto.get("hero_cta_texto")),
        hero_cta_url: texto(objeto.get("hero_cta_url")),
        seo_titulo: texto(objeto.get("seo_titulo")),
        seo_descripcion: texto(objeto.get("seo_descripcion")),
        seo_imagen_objeto_id: texto_opcional(objeto.get("seo_imagen_objeto_id")),
        personalizacion_actualizada_en: texto_opcional(
            objeto.get("personalizacion_actualizada_en"),
        ),
    }
}

/// Entrada de personalización: distingue ausente de `null`.
#[derive(Debug, Clone, Default)]
pub struct PersonalizacionInput {
    pub plantilla: Option<Value>,
    pub tema: Option<Value>,
    pub hero_titulo: Option<String>,
    pub hero_subtitulo: Option<String>,
    pub hero_cta_texto: Option<String>,
    pub hero_cta_url: Option<String>,
    pub seo_titulo: Option<String>,
    pub seo_descripcion: Option<String>,
    pub seo_imagen_objeto_id: Option<Value>,
}

impl PersonalizacionInput {
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
            plantilla: objeto.get("plantilla").cloned(),
            tema: objeto.get("tema").cloned(),
            hero_titulo: cadena("hero_titulo"),
            hero_subtitulo: cadena("hero_subtitulo"),
            hero_cta_texto: cadena("hero_cta_texto"),
            hero_cta_url: cadena("hero_cta_url"),
            seo_titulo: cadena("seo_titulo"),
            seo_descripcion: cadena("seo_descripcion"),
            seo_imagen_objeto_id: objeto.get("seo_imagen_objeto_id").cloned(),
        }
    }
}

fn fusionar_tema(base: &TemaParcial, datos: &TemaParcial) -> TemaParcial {
    TemaParcial {
        color_primario: datos
            .color_primario
            .clone()
            .or_else(|| base.color_primario.clone()),
        color_secundario: datos
            .color_secundario
            .clone()
            .or_else(|| base.color_secundario.clone()),
        color_acento: datos
            .color_acento
            .clone()
            .or_else(|| base.color_acento.clone()),
        color_fondo: datos
            .color_fondo
            .clone()
            .or_else(|| base.color_fondo.clone()),
        color_texto: datos
            .color_texto
            .clone()
            .or_else(|| base.color_texto.clone()),
        fuente: datos.fuente.clone().or_else(|| base.fuente.clone()),
        radio_bordes: datos.radio_bordes.or(base.radio_bordes),
        modo: datos.modo.clone().or_else(|| base.modo.clone()),
    }
}

/// Mezcla parcial de overrides. `plantilla: null` y `tema: null` limpian el
/// override y devuelven el control a la herencia.
pub fn normalizar_overrides(
    datos: &PersonalizacionInput,
    actual: &OverridesPersonalizacion,
) -> OverridesPersonalizacion {
    let plantilla = match &datos.plantilla {
        None => actual.plantilla.clone(),
        Some(Value::Null) => None,
        Some(Value::String(valor)) if es_plantilla(valor) => Some(valor.clone()),
        Some(_) => actual.plantilla.clone(),
    };
    let tema = match &datos.tema {
        None => actual.tema.clone(),
        Some(Value::Null) => TemaParcial::default(),
        Some(Value::Object(objeto)) => {
            fusionar_tema(&actual.tema, &normalizar_tema_parcial(objeto))
        }
        Some(_) => actual.tema.clone(),
    };
    OverridesPersonalizacion {
        plantilla,
        tema,
        hero_titulo: datos
            .hero_titulo
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.hero_titulo.clone()),
        hero_subtitulo: datos
            .hero_subtitulo
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.hero_subtitulo.clone()),
        hero_cta_texto: datos
            .hero_cta_texto
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.hero_cta_texto.clone()),
        hero_cta_url: datos
            .hero_cta_url
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.hero_cta_url.clone()),
        seo_titulo: datos
            .seo_titulo
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.seo_titulo.clone()),
        seo_descripcion: datos
            .seo_descripcion
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.seo_descripcion.clone()),
        seo_imagen_objeto_id: match &datos.seo_imagen_objeto_id {
            None => actual.seo_imagen_objeto_id.clone(),
            Some(Value::Null) => None,
            Some(Value::String(valor)) => Some(valor.clone()),
            Some(_) => actual.seo_imagen_objeto_id.clone(),
        },
        personalizacion_actualizada_en: actual.personalizacion_actualizada_en.clone(),
    }
}

/// Solo rutas relativas seguras o HTTPS; nunca `javascript:`, `data:` ni `//host`.
pub fn url_destino_valida(valor: &str) -> bool {
    if valor.is_empty() {
        return true;
    }
    if valor.starts_with('/') && !valor.starts_with("//") {
        return true;
    }
    reqwest::Url::parse(valor)
        .map(|url| url.scheme() == "https")
        .unwrap_or(false)
}

fn invalido(mensaje: impl Into<String>) -> ApiError {
    ApiError::dominio("PERSONALIZACION_INVALIDA", mensaje)
}

/// Reglas de dominio que el DTO no puede garantizar por sí solo.
pub fn validar_overrides(meta: &OverridesPersonalizacion) -> Result<(), ApiError> {
    let plantilla = meta.plantilla.clone().unwrap_or_else(|| "clasica".into());
    if !es_plantilla(&plantilla) {
        return Err(invalido("La plantilla no es válida."));
    }
    validar_tema(&mezclar_tema(&[&meta.tema]))?;
    for (campo, valor, maximo) in [
        ("hero_titulo", &meta.hero_titulo, LIMITE_HERO_TITULO),
        (
            "hero_subtitulo",
            &meta.hero_subtitulo,
            LIMITE_HERO_SUBTITULO,
        ),
        (
            "hero_cta_texto",
            &meta.hero_cta_texto,
            LIMITE_HERO_CTA_TEXTO,
        ),
        ("hero_cta_url", &meta.hero_cta_url, LIMITE_HERO_CTA_URL),
        ("seo_titulo", &meta.seo_titulo, LIMITE_SEO_TITULO),
        (
            "seo_descripcion",
            &meta.seo_descripcion,
            LIMITE_SEO_DESCRIPCION,
        ),
    ] {
        if valor.chars().count() > maximo {
            return Err(invalido(format!("{campo} supera {maximo} caracteres.")));
        }
    }
    if !url_destino_valida(&meta.hero_cta_url) {
        return Err(invalido(
            "hero_cta_url debe ser una ruta relativa o una URL HTTPS.",
        ));
    }
    if let Some(objeto) = &meta.seo_imagen_objeto_id {
        if !super::slug::es_uuid(objeto) {
            return Err(invalido(
                "La imagen SEO debe ser un objeto de storage válido.",
            ));
        }
    }
    Ok(())
}

/// Deriva la URL pública; nunca se persiste ni se acepta del cliente.
pub fn con_seo_imagen_url(objeto_id: &Option<String>) -> Option<String> {
    objeto_id
        .as_ref()
        .map(|id| format!("/api/v2/storage/public/objetos/{id}"))
}

/// Objeto JSON con los campos de personalización efectivos.
pub fn personalizacion_json(
    overrides: &OverridesPersonalizacion,
    resuelta: &super::tema::PersonalizacionResuelta,
) -> Map<String, Value> {
    let mut objeto = Map::new();
    objeto.insert(
        "plantilla".into(),
        Value::String(resuelta.plantilla.clone()),
    );
    objeto.insert(
        "tema".into(),
        serde_json::to_value(&resuelta.tema).unwrap_or(Value::Null),
    );
    objeto.insert(
        "hero_titulo".into(),
        Value::String(overrides.hero_titulo.clone()),
    );
    objeto.insert(
        "hero_subtitulo".into(),
        Value::String(overrides.hero_subtitulo.clone()),
    );
    objeto.insert(
        "hero_cta_texto".into(),
        Value::String(overrides.hero_cta_texto.clone()),
    );
    objeto.insert(
        "hero_cta_url".into(),
        Value::String(overrides.hero_cta_url.clone()),
    );
    objeto.insert(
        "seo_titulo".into(),
        Value::String(overrides.seo_titulo.clone()),
    );
    objeto.insert(
        "seo_descripcion".into(),
        Value::String(overrides.seo_descripcion.clone()),
    );
    objeto.insert(
        "seo_imagen_objeto_id".into(),
        overrides
            .seo_imagen_objeto_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    objeto.insert(
        "personalizacion_actualizada_en".into(),
        overrides
            .personalizacion_actualizada_en
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    objeto
}

/// Objeto JSON con los overrides crudos (para la respuesta interna).
pub fn overrides_json(overrides: &OverridesPersonalizacion) -> Value {
    let mut tema = Map::new();
    if let Some(valor) = &overrides.tema.color_primario {
        tema.insert("color_primario".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &overrides.tema.color_secundario {
        tema.insert("color_secundario".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &overrides.tema.color_acento {
        tema.insert("color_acento".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &overrides.tema.color_fondo {
        tema.insert("color_fondo".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &overrides.tema.color_texto {
        tema.insert("color_texto".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &overrides.tema.fuente {
        tema.insert("fuente".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = overrides.tema.radio_bordes {
        tema.insert("radio_bordes".into(), Value::from(valor));
    }
    if let Some(valor) = &overrides.tema.modo {
        tema.insert("modo".into(), Value::String(valor.clone()));
    }
    serde_json::json!({
        "plantilla": overrides.plantilla,
        "tema": tema,
        "hero_titulo": overrides.hero_titulo,
        "hero_subtitulo": overrides.hero_subtitulo,
        "hero_cta_texto": overrides.hero_cta_texto,
        "hero_cta_url": overrides.hero_cta_url,
        "seo_titulo": overrides.seo_titulo,
        "seo_descripcion": overrides.seo_descripcion,
        "seo_imagen_objeto_id": overrides.seo_imagen_objeto_id,
        "personalizacion_actualizada_en": overrides.personalizacion_actualizada_en,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiendas::tema::resolver_personalizacion;
    use serde_json::json;

    #[test]
    fn debe_normalizar_overrides_y_limpiar_con_null() {
        let base = OverridesPersonalizacion {
            plantilla: Some("minimal".into()),
            tema: tema_parcial_de(&json!({"color_primario": "#111111"})),
            hero_titulo: "Hola".into(),
            ..Default::default()
        };
        let datos = PersonalizacionInput::desde_json(&json!({
            "plantilla": null,
            "tema": {"color_acento": "#abc"},
            "hero_titulo": "  Mundo ",
            "seo_imagen_objeto_id": "130c07c6-7e10-4f91-9c13-e17ca6be6eea"
        }));
        let meta = normalizar_overrides(&datos, &base);
        assert_eq!(meta.plantilla, None);
        assert_eq!(meta.tema.color_primario.as_deref(), Some("#111111"));
        assert_eq!(meta.tema.color_acento.as_deref(), Some("#ABC"));
        assert_eq!(meta.hero_titulo, "Mundo");
        assert!(meta.seo_imagen_objeto_id.is_some());
    }

    #[test]
    fn debe_validar_urls_y_limites() {
        assert!(url_destino_valida(""));
        assert!(url_destino_valida("/tienda"));
        assert!(!url_destino_valida("//evil.com"));
        assert!(url_destino_valida("https://example.com/x"));
        assert!(!url_destino_valida("javascript:alert(1)"));

        let largo = OverridesPersonalizacion {
            hero_titulo: "x".repeat(LIMITE_HERO_TITULO + 1),
            ..Default::default()
        };
        assert!(validar_overrides(&largo).is_err());
        let url = OverridesPersonalizacion {
            hero_cta_url: "javascript:alert(1)".into(),
            ..Default::default()
        };
        assert!(validar_overrides(&url).is_err());
        let uuid = OverridesPersonalizacion {
            seo_imagen_objeto_id: Some("no-es-uuid".into()),
            ..Default::default()
        };
        assert!(validar_overrides(&uuid).is_err());
    }

    #[test]
    fn debe_derivar_la_url_publica_del_seo() {
        assert_eq!(con_seo_imagen_url(&None), None);
        assert_eq!(
            con_seo_imagen_url(&Some("abc".into())).as_deref(),
            Some("/api/v2/storage/public/objetos/abc")
        );
    }

    #[test]
    fn debe_serializar_los_campos_en_el_orden_del_contrato() {
        let meta = OverridesPersonalizacion::default();
        let resuelta = resolver_personalizacion(
            &Default::default(),
            &Default::default(),
            &Default::default(),
        );
        let objeto = personalizacion_json(&meta, &resuelta);
        let claves = objeto.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            claves,
            vec![
                "plantilla",
                "tema",
                "hero_titulo",
                "hero_subtitulo",
                "hero_cta_texto",
                "hero_cta_url",
                "seo_titulo",
                "seo_descripcion",
                "seo_imagen_objeto_id",
                "personalizacion_actualizada_en",
            ]
        );
    }
}
