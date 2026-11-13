//! Contrato compartido del tema de tienda (ADR-39/ADR-42).
//!
//! Réplica de `@core/shared` (`tienda/tema.ts`): plantillas, fuentes, modos,
//! defaults, normalización, herencia y generación del CSS.

use crate::errors::ApiError;
use serde::Serialize;
use serde_json::{Map, Value};

pub const PLANTILLAS: [&str; 4] = ["clasica", "minimal", "boutique", "mercado"];
pub const FUENTES: [&str; 5] = ["system", "inter", "poppins", "montserrat", "playfair"];
pub const MODOS: [&str; 3] = ["claro", "oscuro", "auto"];

pub type Origen = String;

pub const ORIGEN_TIENDA: &str = "tienda";
pub const ORIGEN_EMPRESA: &str = "empresa";
pub const ORIGEN_PLATAFORMA: &str = "plataforma";
pub const ORIGEN_DEFECTO: &str = "defecto";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

pub fn tema_por_defecto() -> Tema {
    Tema {
        color_primario: "#1F2937".into(),
        color_secundario: "#F59E0B".into(),
        color_acento: "#10B981".into(),
        color_fondo: "#FFFFFF".into(),
        color_texto: "#111827".into(),
        fuente: "system".into(),
        radio_bordes: 12,
        modo: "claro".into(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemaParcial {
    pub color_primario: Option<String>,
    pub color_secundario: Option<String>,
    pub color_acento: Option<String>,
    pub color_fondo: Option<String>,
    pub color_texto: Option<String>,
    pub fuente: Option<String>,
    pub radio_bordes: Option<i64>,
    pub modo: Option<String>,
}

impl TemaParcial {
    pub fn esta_vacio(&self) -> bool {
        *self == Self::default()
    }
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

/// Devuelve la plantilla válida o `None` si no hay override.
pub fn plantilla_de(valor: &Value) -> Option<String> {
    valor
        .as_str()
        .filter(|texto| es_plantilla(texto))
        .map(str::to_string)
}

fn color_hex(valor: &str) -> bool {
    let resto = match valor.strip_prefix('#') {
        Some(resto) => resto,
        None => return false,
    };
    matches!(resto.len(), 3 | 6 | 8) && resto.chars().all(|caracter| caracter.is_ascii_hexdigit())
}

/// Normaliza un tema parcial leído de BD o de una petición. Solo conserva
/// claves conocidas y valores con el tipo correcto; el resto se ignora.
pub fn tema_parcial_de(valor: &Value) -> TemaParcial {
    let Some(objeto) = valor.as_object() else {
        return TemaParcial::default();
    };
    let color = |clave: &str| {
        objeto
            .get(clave)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|texto| color_hex(texto))
            .map(|texto| texto.to_uppercase())
    };
    TemaParcial {
        color_primario: color("color_primario"),
        color_secundario: color("color_secundario"),
        color_acento: color("color_acento"),
        color_fondo: color("color_fondo"),
        color_texto: color("color_texto"),
        fuente: objeto
            .get("fuente")
            .and_then(Value::as_str)
            .filter(|texto| es_fuente(texto))
            .map(str::to_string),
        radio_bordes: objeto
            .get("radio_bordes")
            .and_then(Value::as_i64)
            .filter(|numero| (0..=32).contains(numero)),
        modo: objeto
            .get("modo")
            .and_then(Value::as_str)
            .filter(|texto| es_modo(texto))
            .map(str::to_string),
    }
}

/// Normaliza un update parcial: recorta/mayúsculas colores sin rellenar defaults.
pub fn normalizar_tema_parcial(objeto: &Map<String, Value>) -> TemaParcial {
    let color = |clave: &str| {
        objeto
            .get(clave)
            .and_then(Value::as_str)
            .map(|texto| texto.trim().to_uppercase())
    };
    TemaParcial {
        color_primario: color("color_primario"),
        color_secundario: color("color_secundario"),
        color_acento: color("color_acento"),
        color_fondo: color("color_fondo"),
        color_texto: color("color_texto"),
        fuente: objeto
            .get("fuente")
            .and_then(Value::as_str)
            .map(str::to_string),
        radio_bordes: objeto.get("radio_bordes").and_then(Value::as_i64),
        modo: objeto
            .get("modo")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// Mezcla capas de tema de menor a mayor prioridad sobre los defaults.
pub fn mezclar_tema(capas: &[&TemaParcial]) -> Tema {
    let mut resultado = tema_por_defecto();
    for capa in capas {
        if let Some(valor) = &capa.color_primario {
            resultado.color_primario = valor.clone();
        }
        if let Some(valor) = &capa.color_secundario {
            resultado.color_secundario = valor.clone();
        }
        if let Some(valor) = &capa.color_acento {
            resultado.color_acento = valor.clone();
        }
        if let Some(valor) = &capa.color_fondo {
            resultado.color_fondo = valor.clone();
        }
        if let Some(valor) = &capa.color_texto {
            resultado.color_texto = valor.clone();
        }
        if let Some(valor) = &capa.fuente {
            resultado.fuente = valor.clone();
        }
        if let Some(valor) = capa.radio_bordes {
            resultado.radio_bordes = valor;
        }
        if let Some(valor) = &capa.modo {
            resultado.modo = valor.clone();
        }
    }
    resultado
}

/// Serializa solo las claves presentes de un tema parcial (como el TS).
pub fn tema_parcial_json(parcial: &TemaParcial) -> Value {
    let mut objeto = Map::new();
    if let Some(valor) = &parcial.color_primario {
        objeto.insert("color_primario".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &parcial.color_secundario {
        objeto.insert("color_secundario".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &parcial.color_acento {
        objeto.insert("color_acento".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &parcial.color_fondo {
        objeto.insert("color_fondo".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &parcial.color_texto {
        objeto.insert("color_texto".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = &parcial.fuente {
        objeto.insert("fuente".into(), Value::String(valor.clone()));
    }
    if let Some(valor) = parcial.radio_bordes {
        objeto.insert("radio_bordes".into(), Value::from(valor));
    }
    if let Some(valor) = &parcial.modo {
        objeto.insert("modo".into(), Value::String(valor.clone()));
    }
    Value::Object(objeto)
}

/// Tema completo a partir de un valor leído de BD.
pub fn tema_de(valor: &Value) -> Tema {
    let parcial = tema_parcial_de(valor);
    mezclar_tema(&[&parcial])
}

/// Mezcla parcial de un update sobre el tema actual (no sobre los defaults).
pub fn normalizar_tema(datos: Option<&TemaParcial>, actual: Option<&Tema>) -> Tema {
    let base = actual.cloned().unwrap_or_else(tema_por_defecto);
    let Some(datos) = datos else {
        return base;
    };
    Tema {
        color_primario: datos.color_primario.clone().unwrap_or(base.color_primario),
        color_secundario: datos
            .color_secundario
            .clone()
            .unwrap_or(base.color_secundario),
        color_acento: datos.color_acento.clone().unwrap_or(base.color_acento),
        color_fondo: datos.color_fondo.clone().unwrap_or(base.color_fondo),
        color_texto: datos.color_texto.clone().unwrap_or(base.color_texto),
        fuente: datos.fuente.clone().unwrap_or(base.fuente),
        radio_bordes: datos.radio_bordes.unwrap_or(base.radio_bordes),
        modo: datos.modo.clone().unwrap_or(base.modo),
    }
}

/// Reglas de dominio que el DTO no puede garantizar por sí solo.
pub fn validar_tema(tema: &Tema) -> Result<(), ApiError> {
    let colores = [
        ("color_primario", &tema.color_primario),
        ("color_secundario", &tema.color_secundario),
        ("color_acento", &tema.color_acento),
        ("color_fondo", &tema.color_fondo),
        ("color_texto", &tema.color_texto),
    ];
    for (campo, valor) in colores {
        if !color_hex(valor) {
            return Err(ApiError::dominio(
                "PERSONALIZACION_INVALIDA",
                format!("{campo} debe ser un color hexadecimal (#RGB, #RRGGBB o #RRGGBBAA)."),
            ));
        }
    }
    if !es_fuente(&tema.fuente) {
        return Err(ApiError::dominio(
            "PERSONALIZACION_INVALIDA",
            "La fuente no es válida.",
        ));
    }
    if !(0..=32).contains(&tema.radio_bordes) {
        return Err(ApiError::dominio(
            "PERSONALIZACION_INVALIDA",
            "El radio de bordes debe ser un entero entre 0 y 32.",
        ));
    }
    if !es_modo(&tema.modo) {
        return Err(ApiError::dominio(
            "PERSONALIZACION_INVALIDA",
            "El modo de color no es válido.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemaHerencia {
    pub plantilla: Option<String>,
    pub tema: TemaParcial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalizacionResuelta {
    pub plantilla: String,
    pub tema: Tema,
    pub origen_plantilla: Origen,
    pub origen_tema: Origen,
}

/// Resuelve la herencia: plataforma → empresa → overrides del nivel superior.
pub fn resolver_personalizacion(
    overrides: &TemaHerencia,
    empresa: &TemaHerencia,
    plataforma: &TemaHerencia,
) -> PersonalizacionResuelta {
    let tema = mezclar_tema(&[&plataforma.tema, &empresa.tema, &overrides.tema]);
    let plantilla = overrides
        .plantilla
        .clone()
        .or_else(|| empresa.plantilla.clone())
        .or_else(|| plataforma.plantilla.clone())
        .unwrap_or_else(|| "clasica".to_string());

    let origen_plantilla = if overrides.plantilla.is_some() {
        ORIGEN_TIENDA
    } else if empresa.plantilla.is_some() {
        ORIGEN_EMPRESA
    } else if plataforma.plantilla.is_some() {
        ORIGEN_PLATAFORMA
    } else {
        ORIGEN_DEFECTO
    };

    let origen_tema = if !overrides.tema.esta_vacio() {
        ORIGEN_TIENDA
    } else if !empresa.tema.esta_vacio() {
        ORIGEN_EMPRESA
    } else if !plataforma.tema.esta_vacio() {
        ORIGEN_PLATAFORMA
    } else {
        ORIGEN_DEFECTO
    };

    PersonalizacionResuelta {
        plantilla,
        tema,
        origen_plantilla: origen_plantilla.to_string(),
        origen_tema: origen_tema.to_string(),
    }
}

fn fuente_css(fuente: &str) -> &'static str {
    match fuente {
        "inter" => "Inter, system-ui, sans-serif",
        "poppins" => "Poppins, system-ui, sans-serif",
        "montserrat" => "Montserrat, system-ui, sans-serif",
        "playfair" => "\"Playfair Display\", Georgia, serif",
        _ => "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif",
    }
}

/// Genera variables CSS a partir del tema resuelto.
pub fn generar_tema_css(tema: &Tema) -> String {
    let modo = match tema.modo.as_str() {
        "auto" => "light dark",
        "oscuro" => "dark",
        _ => "light",
    };
    let variables = [
        ("--tienda-color-primario", tema.color_primario.clone()),
        ("--tienda-color-secundario", tema.color_secundario.clone()),
        ("--tienda-color-acento", tema.color_acento.clone()),
        ("--tienda-color-fondo", tema.color_fondo.clone()),
        ("--tienda-color-texto", tema.color_texto.clone()),
        ("--tienda-fuente", fuente_css(&tema.fuente).to_string()),
        ("--tienda-radio-bordes", format!("{}px", tema.radio_bordes)),
        ("--tienda-modo", tema.modo.clone()),
        ("color-scheme", modo.to_string()),
    ];
    let cuerpo = variables
        .iter()
        .map(|(nombre, valor)| format!("  {nombre}: {valor};"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(":root {{\n{cuerpo}\n}}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_filtrar_y_normalizar_un_tema_parcial() {
        let parcial = tema_parcial_de(&json!({
            "color_primario": " #abc ",
            "fuente": "poppins",
            "radio_bordes": 8,
            "modo": "oscuro",
            "extra": 1
        }));
        assert_eq!(parcial.color_primario.as_deref(), Some("#ABC"));
        assert_eq!(parcial.fuente.as_deref(), Some("poppins"));
        assert_eq!(parcial.radio_bordes, Some(8));
        assert_eq!(parcial.modo.as_deref(), Some("oscuro"));
        assert_eq!(
            tema_parcial_de(&json!({"color_primario": "rojo", "radio_bordes": 40})),
            TemaParcial::default()
        );
    }

    #[test]
    fn debe_mezclar_capas_con_prioridad_sobre_los_defaults() {
        let tema = mezclar_tema(&[
            &TemaParcial {
                color_primario: Some("#111111".into()),
                ..Default::default()
            },
            &TemaParcial {
                color_primario: Some("#222222".into()),
                fuente: Some("inter".into()),
                ..Default::default()
            },
            &TemaParcial {
                color_secundario: Some("#333333".into()),
                ..Default::default()
            },
        ]);
        assert_eq!(tema.color_primario, "#222222");
        assert_eq!(tema.fuente, "inter");
        assert_eq!(tema.color_secundario, "#333333");
        assert_eq!(tema.color_fondo, tema_por_defecto().color_fondo);
    }

    #[test]
    fn debe_normalizar_un_update_parcial_sin_rellenar_defaults() {
        let mut mapa = Map::new();
        mapa.insert("color_primario".into(), json!(" #abc "));
        let parcial = normalizar_tema_parcial(&mapa);
        assert_eq!(parcial.color_primario.as_deref(), Some("#ABC"));
        assert_eq!(parcial.color_fondo, None);
    }

    #[test]
    fn debe_validar_el_tema_completo() {
        assert!(validar_tema(&tema_por_defecto()).is_ok());
        let mut invalido = tema_por_defecto();
        invalido.color_primario = "rojo".into();
        assert_eq!(
            validar_tema(&invalido).expect_err("color").codigo,
            "PERSONALIZACION_INVALIDA"
        );
        let mut radio = tema_por_defecto();
        radio.radio_bordes = 33;
        assert!(validar_tema(&radio).is_err());
        let mut fuente = tema_por_defecto();
        fuente.fuente = "x".into();
        assert!(validar_tema(&fuente).is_err());
        let mut modo = tema_por_defecto();
        modo.modo = "x".into();
        assert!(validar_tema(&modo).is_err());
    }

    #[test]
    fn debe_resolver_la_herencia_y_el_origen_de_cada_capa() {
        let plataforma = TemaHerencia {
            plantilla: Some("minimal".into()),
            tema: tema_parcial_de(&json!({"color_primario": "#111111"})),
        };
        let empresa = TemaHerencia {
            plantilla: Some("boutique".into()),
            tema: tema_parcial_de(&json!({"color_secundario": "#222222"})),
        };
        let overrides = TemaHerencia {
            plantilla: None,
            tema: tema_parcial_de(&json!({"color_acento": "#333333"})),
        };
        let resuelta = resolver_personalizacion(&overrides, &empresa, &plataforma);
        assert_eq!(resuelta.plantilla, "boutique");
        assert_eq!(resuelta.tema.color_primario, "#111111");
        assert_eq!(resuelta.tema.color_secundario, "#222222");
        assert_eq!(resuelta.tema.color_acento, "#333333");
        assert_eq!(resuelta.origen_plantilla, "empresa");
        assert_eq!(resuelta.origen_tema, "tienda");

        let vacio = TemaHerencia::default();
        let solo_plataforma = resolver_personalizacion(&vacio, &vacio, &plataforma);
        assert_eq!(solo_plataforma.origen_plantilla, "plataforma");
        assert_eq!(solo_plataforma.origen_tema, "plataforma");

        let sin_nada = resolver_personalizacion(&vacio, &vacio, &vacio);
        assert_eq!(sin_nada.plantilla, "clasica");
        assert_eq!(sin_nada.origen_plantilla, "defecto");
        assert_eq!(sin_nada.origen_tema, "defecto");
    }

    #[test]
    fn debe_generar_css_y_resolver_helpers_de_enums() {
        let mut tema = tema_por_defecto();
        tema.modo = "auto".into();
        assert!(generar_tema_css(&tema).contains("color-scheme: light dark;"));
        tema.modo = "oscuro".into();
        assert!(generar_tema_css(&tema).contains("color-scheme: dark;"));
        assert_eq!(plantilla_de(&json!("minimal")).as_deref(), Some("minimal"));
        assert_eq!(plantilla_de(&json!("x")), None);
        assert!(es_fuente("inter"));
        assert!(!es_modo("sepia"));
        assert!(es_plantilla("clasica"));
        assert_eq!(
            tema_de(&json!({"color_primario": "#abc"})).color_primario,
            "#ABC"
        );
        assert!(generar_tema_css(&tema_por_defecto()).ends_with("}\n"));
    }
}
