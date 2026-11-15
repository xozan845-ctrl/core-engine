//! Contrato compartido de tema y herencia de personalización (ADR-39/ADR-42).
//!
//! Réplica de `packages/shared/src/tienda/tema.ts`. Se opera sobre JSONB para
//! conservar exactamente los overrides guardados en `tema_override`.

use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const PLANTILLAS: [&str; 4] = ["clasica", "minimal", "boutique", "mercado"];
pub const FUENTES: [&str; 5] = ["system", "inter", "poppins", "montserrat", "playfair"];
pub const MODOS: [&str; 3] = ["claro", "oscuro", "auto"];
pub const ERROR_INVALIDO: &str = "PERSONALIZACION_INVALIDA";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tema {
    pub color_primario: String,
    pub color_secundario: String,
    pub color_acento: String,
    pub color_fondo: String,
    pub color_texto: String,
    pub fuente: String,
    pub radio_bordes: i64,
    pub modo: String,
}

impl Default for Tema {
    fn default() -> Self {
        Self {
            color_primario: "#1F2937".to_string(),
            color_secundario: "#F59E0B".to_string(),
            color_acento: "#10B981".to_string(),
            color_fondo: "#FFFFFF".to_string(),
            color_texto: "#111827".to_string(),
            fuente: "system".to_string(),
            radio_bordes: 12,
            modo: "claro".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origen {
    pub plantilla: String,
    pub tema: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalizacionResuelta {
    pub plantilla: String,
    pub tema: Tema,
    pub origen: Origen,
}

impl Default for HerenciaTema {
    fn default() -> Self {
        Self {
            plantilla: None,
            tema: json!({}),
        }
    }
}

/// Alias explícito para que el dominio no dependa de `Value` en la firma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerenciaTema {
    pub plantilla: Option<String>,
    pub tema: Value,
}

pub fn es_plantilla(valor: &str) -> bool {
    PLANTILLAS.contains(&valor)
}

pub fn es_fuente(valor: &str) -> bool {
    FUENTES.contains(&valor)
}

pub fn es_modo(valor: &str) -> bool {
    MODOS.contains(&valor)
}

pub fn plantilla_de(valor: Option<&Value>) -> Option<String> {
    valor
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| es_plantilla(valor))
        .map(str::to_string)
}

/// Solo conserva claves conocidas y valores válidos al leer de BD/petición.
pub fn tema_parcial_de(valor: &Value) -> Value {
    let Some(fila) = valor.as_object() else {
        return json!({});
    };
    let mut parcial = Map::new();
    for clave in [
        "color_primario",
        "color_secundario",
        "color_acento",
        "color_fondo",
        "color_texto",
    ] {
        if let Some(texto) = fila.get(clave).and_then(Value::as_str) {
            let texto = texto.trim();
            if es_color_hex(texto) {
                parcial.insert(clave.to_string(), json!(texto.to_uppercase()));
            }
        }
    }
    if let Some(fuente) = fila.get("fuente").and_then(Value::as_str) {
        if es_fuente(fuente) {
            parcial.insert("fuente".into(), json!(fuente));
        }
    }
    if let Some(modo) = fila.get("modo").and_then(Value::as_str) {
        if es_modo(modo) {
            parcial.insert("modo".into(), json!(modo));
        }
    }
    if let Some(radio) = fila.get("radio_bordes").and_then(Value::as_i64) {
        if (0..=32).contains(&radio) {
            parcial.insert("radio_bordes".into(), json!(radio));
        }
    }
    Value::Object(parcial)
}

/// Normaliza un update parcial: recorta y sube a mayúsculas los colores.
pub fn normalizar_tema_parcial(datos: &Value) -> Value {
    let Some(fila) = datos.as_object() else {
        return json!({});
    };
    let mut parcial = Map::new();
    for clave in [
        "color_primario",
        "color_secundario",
        "color_acento",
        "color_fondo",
        "color_texto",
    ] {
        if let Some(texto) = fila.get(clave).and_then(Value::as_str) {
            parcial.insert(clave.to_string(), json!(texto.trim().to_uppercase()));
        }
    }
    for clave in ["fuente", "radio_bordes", "modo"] {
        if let Some(valor) = fila.get(clave) {
            parcial.insert(clave.to_string(), valor.clone());
        }
    }
    Value::Object(parcial)
}

/// Mezcla capas de tema de menor a mayor prioridad sobre los defaults.
pub fn mezclar_tema(capas: &[&Value]) -> Tema {
    let mut resultado = Tema::default();
    for capa in capas {
        let Some(objeto) = capa.as_object() else {
            continue;
        };
        aplicar_capa(&mut resultado, objeto);
    }
    resultado
}

fn aplicar_capa(resultado: &mut Tema, objeto: &Map<String, Value>) {
    if let Some(valor) = objeto.get("color_primario").and_then(Value::as_str) {
        resultado.color_primario = valor.to_string();
    }
    if let Some(valor) = objeto.get("color_secundario").and_then(Value::as_str) {
        resultado.color_secundario = valor.to_string();
    }
    if let Some(valor) = objeto.get("color_acento").and_then(Value::as_str) {
        resultado.color_acento = valor.to_string();
    }
    if let Some(valor) = objeto.get("color_fondo").and_then(Value::as_str) {
        resultado.color_fondo = valor.to_string();
    }
    if let Some(valor) = objeto.get("color_texto").and_then(Value::as_str) {
        resultado.color_texto = valor.to_string();
    }
    if let Some(valor) = objeto.get("fuente").and_then(Value::as_str) {
        resultado.fuente = valor.to_string();
    }
    if let Some(valor) = objeto.get("radio_bordes").and_then(Value::as_i64) {
        resultado.radio_bordes = valor;
    }
    if let Some(valor) = objeto.get("modo").and_then(Value::as_str) {
        resultado.modo = valor.to_string();
    }
}

pub fn tema_de(valor: &Value) -> Tema {
    mezclar_tema(&[&tema_parcial_de(valor)])
}

pub fn validar_tema(tema: &Tema) -> Result<(), ApiError> {
    let colores = [
        ("color_primario", &tema.color_primario),
        ("color_secundario", &tema.color_secundario),
        ("color_acento", &tema.color_acento),
        ("color_fondo", &tema.color_fondo),
        ("color_texto", &tema.color_texto),
    ];
    for (campo, valor) in colores {
        if !es_color_hex(valor) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                format!("{campo} debe ser un color hexadecimal (#RGB, #RRGGBB o #RRGGBBAA)."),
            ));
        }
    }
    if !es_fuente(&tema.fuente) {
        return Err(ApiError::dominio(ERROR_INVALIDO, "La fuente no es válida."));
    }
    if !(0..=32).contains(&tema.radio_bordes) {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "El radio de bordes debe ser un entero entre 0 y 32.",
        ));
    }
    if !es_modo(&tema.modo) {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "El modo de color no es válido.",
        ));
    }
    Ok(())
}

/// Resuelve la herencia plataforma → empresa → overrides del nivel actual.
pub fn resolver_personalizacion(
    overrides: &HerenciaTema,
    empresa: &HerenciaTema,
    plataforma: &HerenciaTema,
    nivel: &str,
) -> PersonalizacionResuelta {
    let tema = mezclar_tema(&[&plataforma.tema, &empresa.tema, &overrides.tema]);
    let plantilla = overrides
        .plantilla
        .clone()
        .or_else(|| empresa.plantilla.clone())
        .or_else(|| plataforma.plantilla.clone())
        .unwrap_or_else(|| "clasica".to_string());
    let origen_plantilla = if overrides.plantilla.is_some() {
        nivel.to_string()
    } else if empresa.plantilla.is_some() {
        "empresa".to_string()
    } else if plataforma.plantilla.is_some() {
        "plataforma".to_string()
    } else {
        "defecto".to_string()
    };
    let vacio = |valor: &Value| valor.as_object().map(Map::is_empty).unwrap_or(true);
    let origen_tema = if !vacio(&overrides.tema) {
        nivel.to_string()
    } else if !vacio(&empresa.tema) {
        "empresa".to_string()
    } else if !vacio(&plataforma.tema) {
        "plataforma".to_string()
    } else {
        "defecto".to_string()
    };
    PersonalizacionResuelta {
        plantilla,
        tema,
        origen: Origen {
            plantilla: origen_plantilla,
            tema: origen_tema,
        },
    }
}

pub fn es_color_hex(valor: &str) -> bool {
    let Some(resto) = valor.strip_prefix('#') else {
        return false;
    };
    matches!(resto.len(), 3 | 6 | 8) && resto.chars().all(|caracter| caracter.is_ascii_hexdigit())
}

/// Valida un `tema` de entrada (DTO) antes de mezclarlo con los overrides.
pub fn validar_tema_input(datos: &Value) -> Result<(), ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            ERROR_INVALIDO,
            "El tema debe ser un objeto.",
        ));
    };
    for clave in [
        "color_primario",
        "color_secundario",
        "color_acento",
        "color_fondo",
        "color_texto",
    ] {
        if let Some(valor) = objeto.get(clave) {
            let texto = valor.as_str().ok_or_else(|| {
                ApiError::dominio(
                    ERROR_INVALIDO,
                    format!("{clave} debe ser un color hexadecimal."),
                )
            })?;
            if !es_color_hex(texto) {
                return Err(ApiError::dominio(
                    ERROR_INVALIDO,
                    format!("{clave} debe ser un color hexadecimal (#RGB, #RRGGBB o #RRGGBBAA)."),
                ));
            }
        }
    }
    if let Some(fuente) = objeto.get("fuente") {
        if !fuente.as_str().is_some_and(es_fuente) {
            return Err(ApiError::dominio(ERROR_INVALIDO, "La fuente no es válida."));
        }
    }
    if let Some(radio) = objeto.get("radio_bordes") {
        if !radio
            .as_i64()
            .is_some_and(|valor| (0..=32).contains(&valor))
        {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "El radio de bordes debe ser un entero entre 0 y 32.",
            ));
        }
    }
    if let Some(modo) = objeto.get("modo") {
        if !modo.as_str().is_some_and(es_modo) {
            return Err(ApiError::dominio(
                ERROR_INVALIDO,
                "El modo de color no es válido.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_colores_y_descartar_claves_desconocidas() {
        let parcial = tema_parcial_de(&json!({
            "color_primario": " #abc ",
            "fuente": "inter",
            "radio_bordes": 40,
            "modo": "raro",
            "otra": "x"
        }));
        assert_eq!(parcial["color_primario"], "#ABC");
        assert_eq!(parcial["fuente"], "inter");
        assert!(parcial.get("radio_bordes").is_none());
        assert!(parcial.get("otra").is_none());
    }

    #[test]
    fn debe_mezclar_plataforma_empresa_y_overrides() {
        let plataforma = json!({"color_primario": "#111111", "fuente": "inter"});
        let empresa = json!({"color_primario": "#222222"});
        let overrides = json!({"color_primario": "#333333", "modo": "oscuro"});
        let tema = mezclar_tema(&[&plataforma, &empresa, &overrides]);
        assert_eq!(tema.color_primario, "#333333");
        assert_eq!(tema.fuente, "inter");
        assert_eq!(tema.modo, "oscuro");
        assert_eq!(tema.radio_bordes, 12);
    }

    #[test]
    fn debe_resolver_origenes_de_plantilla_y_tema() {
        let overrides = HerenciaTema {
            plantilla: None,
            tema: json!({}),
        };
        let empresa = HerenciaTema {
            plantilla: Some("minimal".into()),
            tema: json!({"color_fondo": "#000000"}),
        };
        let plataforma = HerenciaTema {
            plantilla: Some("clasica".into()),
            tema: json!({"color_texto": "#FFFFFF"}),
        };
        let resuelta = resolver_personalizacion(&overrides, &empresa, &plataforma, "tienda");
        assert_eq!(resuelta.plantilla, "minimal");
        assert_eq!(resuelta.origen.plantilla, "empresa");
        assert_eq!(resuelta.origen.tema, "empresa");
        assert_eq!(resuelta.tema.color_fondo, "#000000");
        assert_eq!(resuelta.tema.color_texto, "#FFFFFF");
    }

    #[test]
    fn debe_rechazar_un_tema_invalido() {
        let tema = Tema {
            color_primario: "rojo".into(),
            ..Default::default()
        };
        assert_eq!(
            validar_tema(&tema).expect_err("color").codigo,
            ERROR_INVALIDO
        );
        let tema = Tema {
            color_primario: "#FFF".into(),
            radio_bordes: 33,
            ..Default::default()
        };
        assert!(validar_tema(&tema).is_err());
    }
}
