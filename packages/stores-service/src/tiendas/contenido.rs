//! Contenido del storefront de tienda (ADR-40).
//!
//! Igual que el tema, el backend guarda datos estructurados y validados; el
//! frontend decide cómo renderizar.

use super::personalizacion::url_destino_valida;
use crate::errors::ApiError;
use serde::Serialize;
use serde_json::{json, Value};

pub const TIPOS_SECCION: [&str; 8] = [
    "hero",
    "destacados",
    "categorias",
    "ofertas",
    "sobre_nosotros",
    "politicas",
    "contacto",
    "redes",
];
pub const REDES_SOCIALES: [&str; 7] = [
    "instagram",
    "facebook",
    "tiktok",
    "x",
    "whatsapp",
    "youtube",
    "otra",
];

pub const LIMITES_SECCIONES: usize = 12;
pub const LIMITE_SOBRE_NOSOTROS: usize = 2000;
pub const LIMITE_POLITICA: usize = 1000;
pub const LIMITE_REDES: usize = 6;
pub const LIMITE_ANUNCIO_TEXTO: usize = 120;
pub const LIMITE_ANUNCIO_ENLACE: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seccion {
    pub tipo: String,
    pub visible: bool,
    pub orden: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedSocial {
    pub red: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Politicas {
    pub envio: String,
    pub devolucion: String,
    pub privacidad: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ContenidoCampos {
    pub secciones: Vec<Seccion>,
    pub sobre_nosotros: String,
    pub politicas: Politicas,
    pub redes: Vec<RedSocial>,
    pub anuncio_texto: String,
    pub anuncio_enlace: String,
    pub anuncio_activo: bool,
    pub contenido_actualizado_en: Option<String>,
}

fn texto(valor: Option<&Value>) -> String {
    valor
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn leer_secciones(valor: &Value) -> Vec<Seccion> {
    let Some(items) = valor.as_array() else {
        return Vec::new();
    };
    let mut secciones = items
        .iter()
        .filter_map(Value::as_object)
        .filter(|item| {
            item.get("tipo")
                .and_then(Value::as_str)
                .is_some_and(|tipo| TIPOS_SECCION.contains(&tipo))
        })
        .map(|item| Seccion {
            tipo: item
                .get("tipo")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            visible: item.get("visible").and_then(Value::as_bool).unwrap_or(true),
            orden: item.get("orden").and_then(Value::as_i64).unwrap_or(0),
        })
        .collect::<Vec<_>>();
    secciones.sort_by_key(|seccion| seccion.orden);
    secciones
}

fn leer_politicas(valor: &Value) -> Politicas {
    let Some(objeto) = valor.as_object() else {
        return Politicas::default();
    };
    Politicas {
        envio: texto(objeto.get("envio")),
        devolucion: texto(objeto.get("devolucion")),
        privacidad: texto(objeto.get("privacidad")),
    }
}

fn leer_redes(valor: &Value) -> Vec<RedSocial> {
    let Some(items) = valor.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_object)
        .filter(|item| {
            item.get("red")
                .and_then(Value::as_str)
                .is_some_and(|red| REDES_SOCIALES.contains(&red))
        })
        .map(|item| RedSocial {
            red: item
                .get("red")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            url: texto(item.get("url")),
        })
        .filter(|red| !red.url.is_empty())
        .collect()
}

/// Completa una fila parcial con los valores por defecto.
pub fn contenido_de(fila: &Value) -> ContenidoCampos {
    let Some(objeto) = fila.as_object() else {
        return ContenidoCampos::default();
    };
    ContenidoCampos {
        secciones: leer_secciones(objeto.get("secciones").unwrap_or(&Value::Null)),
        sobre_nosotros: texto(objeto.get("sobre_nosotros")),
        politicas: leer_politicas(objeto.get("politicas").unwrap_or(&Value::Null)),
        redes: leer_redes(objeto.get("redes").unwrap_or(&Value::Null)),
        anuncio_texto: texto(objeto.get("anuncio_texto")),
        anuncio_enlace: texto(objeto.get("anuncio_enlace")),
        anuncio_activo: objeto
            .get("anuncio_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        contenido_actualizado_en: objeto
            .get("contenido_actualizado_en")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// Entrada de contenido: cada campo distingue ausente de `null`.
#[derive(Debug, Clone, Default)]
pub struct ContenidoInput {
    pub secciones: Option<Value>,
    pub sobre_nosotros: Option<String>,
    pub politicas: Option<Value>,
    pub redes: Option<Value>,
    pub anuncio_texto: Option<String>,
    pub anuncio_enlace: Option<String>,
    pub anuncio_activo: Option<bool>,
}

impl ContenidoInput {
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
            secciones: objeto.get("secciones").cloned(),
            sobre_nosotros: cadena("sobre_nosotros"),
            politicas: objeto.get("politicas").cloned(),
            redes: objeto.get("redes").cloned(),
            anuncio_texto: cadena("anuncio_texto"),
            anuncio_enlace: cadena("anuncio_enlace"),
            anuncio_activo: objeto.get("anuncio_activo").and_then(Value::as_bool),
        }
    }
}

fn normalizar_secciones(valor: &Value) -> Vec<Seccion> {
    let Some(items) = valor.as_array() else {
        return Vec::new();
    };
    let mut secciones = items
        .iter()
        .filter_map(Value::as_object)
        .map(|item| Seccion {
            tipo: item
                .get("tipo")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            visible: item
                .get("visible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            orden: item.get("orden").and_then(Value::as_i64).unwrap_or(0),
        })
        .collect::<Vec<_>>();
    secciones.sort_by_key(|seccion| seccion.orden);
    secciones
}

fn normalizar_redes(valor: &Value) -> Vec<RedSocial> {
    let Some(items) = valor.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_object)
        .map(|item| RedSocial {
            red: item
                .get("red")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            url: item
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
        })
        .collect()
}

/// Mezcla parcial: un campo ausente conserva el valor actual.
pub fn normalizar_contenido(datos: &ContenidoInput, actual: &ContenidoCampos) -> ContenidoCampos {
    let politicas = match &datos.politicas {
        Some(valor) => {
            let objeto = valor.as_object();
            let leer = |clave: &str, base: &str| {
                objeto
                    .and_then(|mapa| mapa.get(clave))
                    .and_then(Value::as_str)
                    .map(|texto| texto.trim().to_string())
                    .unwrap_or_else(|| base.to_string())
            };
            Politicas {
                envio: leer("envio", &actual.politicas.envio),
                devolucion: leer("devolucion", &actual.politicas.devolucion),
                privacidad: leer("privacidad", &actual.politicas.privacidad),
            }
        }
        None => actual.politicas.clone(),
    };
    ContenidoCampos {
        secciones: datos
            .secciones
            .as_ref()
            .map(normalizar_secciones)
            .unwrap_or_else(|| actual.secciones.clone()),
        sobre_nosotros: datos
            .sobre_nosotros
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.sobre_nosotros.clone()),
        politicas,
        redes: datos
            .redes
            .as_ref()
            .map(normalizar_redes)
            .unwrap_or_else(|| actual.redes.clone()),
        anuncio_texto: datos
            .anuncio_texto
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.anuncio_texto.clone()),
        anuncio_enlace: datos
            .anuncio_enlace
            .as_ref()
            .map(|valor| valor.trim().to_string())
            .unwrap_or_else(|| actual.anuncio_enlace.clone()),
        anuncio_activo: datos.anuncio_activo.unwrap_or(actual.anuncio_activo),
        contenido_actualizado_en: actual.contenido_actualizado_en.clone(),
    }
}

fn invalido(mensaje: impl Into<String>) -> ApiError {
    ApiError::dominio("CONTENIDO_INVALIDO", mensaje)
}

/// Reglas de dominio que el DTO no puede garantizar por sí solo.
pub fn validar_contenido(meta: &ContenidoCampos) -> Result<(), ApiError> {
    if meta.secciones.len() > LIMITES_SECCIONES {
        return Err(invalido(format!(
            "No se pueden configurar más de {LIMITES_SECCIONES} secciones."
        )));
    }
    let mut tipos = std::collections::HashSet::new();
    let mut ordenes = std::collections::HashSet::new();
    for seccion in &meta.secciones {
        if !TIPOS_SECCION.contains(&seccion.tipo.as_str()) {
            return Err(invalido(format!(
                "La sección \"{}\" no es válida.",
                seccion.tipo
            )));
        }
        if !(0..LIMITES_SECCIONES as i64).contains(&seccion.orden) {
            return Err(invalido(format!(
                "El orden de una sección debe ser un entero entre 0 y {}.",
                LIMITES_SECCIONES - 1
            )));
        }
        if !tipos.insert(seccion.tipo.clone()) {
            return Err(invalido(format!(
                "La sección \"{}\" está repetida.",
                seccion.tipo
            )));
        }
        if !ordenes.insert(seccion.orden) {
            return Err(invalido(format!(
                "El orden {} está repetido.",
                seccion.orden
            )));
        }
    }

    if meta.sobre_nosotros.chars().count() > LIMITE_SOBRE_NOSOTROS {
        return Err(invalido(format!(
            "sobre_nosotros supera {LIMITE_SOBRE_NOSOTROS} caracteres."
        )));
    }
    for (campo, valor) in [
        ("envio", &meta.politicas.envio),
        ("devolucion", &meta.politicas.devolucion),
        ("privacidad", &meta.politicas.privacidad),
    ] {
        if valor.chars().count() > LIMITE_POLITICA {
            return Err(invalido(format!(
                "La política de {campo} supera {LIMITE_POLITICA} caracteres."
            )));
        }
    }

    if meta.redes.len() > LIMITE_REDES {
        return Err(invalido(format!(
            "No se pueden configurar más de {LIMITE_REDES} redes."
        )));
    }
    let mut redes = std::collections::HashSet::new();
    for red in &meta.redes {
        if !REDES_SOCIALES.contains(&red.red.as_str()) {
            return Err(invalido(format!("La red \"{}\" no es válida.", red.red)));
        }
        if !redes.insert(red.red.clone()) {
            return Err(invalido(format!("La red \"{}\" está repetida.", red.red)));
        }
        if red.url.is_empty() || !url_destino_valida(&red.url) || !red.url.starts_with("https://") {
            return Err(invalido("Cada red debe tener una URL HTTPS válida."));
        }
    }

    if meta.anuncio_texto.chars().count() > LIMITE_ANUNCIO_TEXTO {
        return Err(invalido(format!(
            "El anuncio supera {LIMITE_ANUNCIO_TEXTO} caracteres."
        )));
    }
    if meta.anuncio_enlace.chars().count() > LIMITE_ANUNCIO_ENLACE {
        return Err(invalido(format!(
            "El enlace del anuncio supera {LIMITE_ANUNCIO_ENLACE} caracteres."
        )));
    }
    if !url_destino_valida(&meta.anuncio_enlace) {
        return Err(invalido(
            "El enlace del anuncio debe ser una ruta relativa o una URL HTTPS.",
        ));
    }
    Ok(())
}

/// Objeto JSON en el orden del contrato original.
pub fn contenido_json(campos: &ContenidoCampos) -> Value {
    json!({
        "secciones": campos.secciones,
        "sobre_nosotros": campos.sobre_nosotros,
        "politicas": campos.politicas,
        "redes": campos.redes,
        "anuncio_texto": campos.anuncio_texto,
        "anuncio_enlace": campos.anuncio_enlace,
        "anuncio_activo": campos.anuncio_activo,
        "contenido_actualizado_en": campos.contenido_actualizado_en,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contenido(datos: Value) -> ContenidoInput {
        let objeto = datos.as_object().cloned().unwrap_or_default();
        ContenidoInput {
            secciones: objeto.get("secciones").cloned(),
            sobre_nosotros: objeto
                .get("sobre_nosotros")
                .and_then(Value::as_str)
                .map(str::to_string),
            politicas: objeto.get("politicas").cloned(),
            redes: objeto.get("redes").cloned(),
            anuncio_texto: objeto
                .get("anuncio_texto")
                .and_then(Value::as_str)
                .map(str::to_string),
            anuncio_enlace: objeto
                .get("anuncio_enlace")
                .and_then(Value::as_str)
                .map(str::to_string),
            anuncio_activo: objeto.get("anuncio_activo").and_then(Value::as_bool),
        }
    }

    #[test]
    fn debe_leer_y_ordenar_secciones_y_redes() {
        let campos = contenido_de(&json!({
            "secciones": [
                {"tipo": "hero", "visible": true, "orden": 2},
                {"tipo": "ofertas", "visible": false, "orden": 1},
                {"tipo": "invalida", "orden": 0}
            ],
            "redes": [
                {"red": "instagram", "url": " https://instagram.com/x "},
                {"red": "invalida", "url": "https://x.com"}
            ],
            "politicas": {"envio": " 24h "},
            "anuncio_activo": true
        }));
        assert_eq!(campos.secciones.len(), 2);
        assert_eq!(campos.secciones[0].tipo, "ofertas");
        assert_eq!(campos.secciones[1].tipo, "hero");
        assert_eq!(campos.redes.len(), 1);
        assert_eq!(campos.redes[0].url, "https://instagram.com/x");
        assert_eq!(campos.politicas.envio, "24h");
        assert!(campos.anuncio_activo);
    }

    #[test]
    fn debe_conservar_campos_ausentes_y_limpiar_con_null() {
        let base = contenido_de(&json!({
            "sobre_nosotros": "hola",
            "secciones": [{"tipo": "hero", "visible": true, "orden": 0}]
        }));
        let mezcla = normalizar_contenido(
            &contenido(json!({"anuncio_texto": " promo ", "secciones": null})),
            &base,
        );
        assert_eq!(mezcla.sobre_nosotros, "hola");
        assert_eq!(mezcla.anuncio_texto, "promo");
        assert!(mezcla.secciones.is_empty());
    }

    #[test]
    fn debe_validar_limites_y_duplicados() {
        let base = ContenidoCampos::default();
        assert!(validar_contenido(&base).is_ok());

        let repetidas = ContenidoCampos {
            secciones: vec![
                Seccion {
                    tipo: "hero".into(),
                    visible: true,
                    orden: 0,
                },
                Seccion {
                    tipo: "hero".into(),
                    visible: true,
                    orden: 1,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            validar_contenido(&repetidas).expect_err("repetida").codigo,
            "CONTENIDO_INVALIDO"
        );

        let orden_repetido = ContenidoCampos {
            secciones: vec![
                Seccion {
                    tipo: "hero".into(),
                    visible: true,
                    orden: 1,
                },
                Seccion {
                    tipo: "ofertas".into(),
                    visible: true,
                    orden: 1,
                },
            ],
            ..Default::default()
        };
        assert!(validar_contenido(&orden_repetido).is_err());

        let redes = ContenidoCampos {
            redes: vec![RedSocial {
                red: "instagram".into(),
                url: "http://insegura.com".into(),
            }],
            ..Default::default()
        };
        assert!(validar_contenido(&redes).is_err());

        let enlace = ContenidoCampos {
            anuncio_enlace: "javascript:alert(1)".into(),
            ..Default::default()
        };
        assert!(validar_contenido(&enlace).is_err());
    }
}
