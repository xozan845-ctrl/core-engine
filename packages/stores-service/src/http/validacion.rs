//! Validación de DTOs con el mismo shape de error que `class-validator` +
//! `DtoValidationPipe`: `VALIDACION` con `detalles[{campo, restricciones}]`.
//!
//! Las reglas replican los decoradores declarados en los controladores Nest y
//! el `whitelist` del pipe (los campos desconocidos se ignoran).

use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Regla {
    Texto,
    En(&'static [&'static str]),
    Entero,
    Min(f64),
    Max(f64),
    Booleano,
    Arreglo,
    Objeto,
    Uuid,
    Longitud(usize, usize),
    Coincide(&'static str),
}

impl Regla {
    fn nombre(self) -> &'static str {
        match self {
            Regla::Texto => "isString",
            Regla::En(_) => "isIn",
            Regla::Entero => "isInt",
            Regla::Min(_) => "min",
            Regla::Max(_) => "max",
            Regla::Booleano => "isBoolean",
            Regla::Arreglo => "isArray",
            Regla::Objeto => "isObject",
            Regla::Uuid => "isUuid",
            Regla::Longitud(_, _) => "isLength",
            Regla::Coincide(_) => "matches",
        }
    }

    fn cumple(self, valor: &Value) -> bool {
        match self {
            Regla::Texto => valor.is_string(),
            Regla::En(opciones) => valor
                .as_str()
                .is_some_and(|texto| opciones.contains(&texto)),
            Regla::Entero => valor
                .as_f64()
                .is_some_and(|numero| numero.is_finite() && numero.fract() == 0.0),
            Regla::Min(minimo) => valor.as_f64().is_some_and(|numero| numero >= minimo),
            Regla::Max(maximo) => valor.as_f64().is_some_and(|numero| numero <= maximo),
            Regla::Booleano => valor.is_boolean(),
            Regla::Arreglo => valor.is_array(),
            Regla::Objeto => valor.is_object(),
            Regla::Uuid => valor
                .as_str()
                .is_some_and(|texto| Uuid::parse_str(texto).is_ok()),
            Regla::Longitud(minimo, maximo) => valor.as_str().is_some_and(|texto| {
                let longitud = texto.chars().count();
                longitud >= minimo && longitud <= maximo
            }),
            Regla::Coincide(patron) => valor.as_str().is_some_and(|texto| coincide(texto, patron)),
        }
    }
}

/// Patrón mínimo usado por los DTO (`^[A-Za-z]{3}$` y color hex).
fn coincide(texto: &str, patron: &str) -> bool {
    match patron {
        "moneda_iso" => {
            texto.len() == 3 && texto.chars().all(|caracter| caracter.is_ascii_alphabetic())
        }
        "color_hex" => {
            let resto = match texto.strip_prefix('#') {
                Some(resto) => resto,
                None => return false,
            };
            matches!(resto.len(), 3 | 6 | 8)
                && resto.chars().all(|caracter| caracter.is_ascii_hexdigit())
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReglaCampo {
    pub campo: &'static str,
    pub requerido: bool,
    pub reglas: &'static [Regla],
}

pub fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    let vacio = Map::new();
    let objeto = cuerpo.as_object().unwrap_or(&vacio);
    let mut detalles = Vec::new();

    for campo in esquema {
        let valor = objeto.get(campo.campo);
        let ausente_o_nulo = matches!(valor, None | Some(Value::Null));
        if ausente_o_nulo {
            if campo.requerido {
                let restricciones = campo
                    .reglas
                    .iter()
                    .map(|regla| regla.nombre())
                    .collect::<Vec<_>>();
                detalles.push(json!({
                    "campo": campo.campo,
                    "restricciones": restricciones,
                }));
            }
            continue;
        }
        let valor = valor.expect("valor presente");
        let fallidas = campo
            .reglas
            .iter()
            .filter(|regla| !regla.cumple(valor))
            .map(|regla| regla.nombre())
            .collect::<Vec<_>>();
        if !fallidas.is_empty() {
            detalles.push(json!({
                "campo": campo.campo,
                "restricciones": fallidas,
            }));
        }
    }

    if detalles.is_empty() {
        Ok(())
    } else {
        Err(ApiError::validacion(Value::Array(detalles)))
    }
}

const fn campo(nombre: &'static str, requerido: bool, reglas: &'static [Regla]) -> ReglaCampo {
    ReglaCampo {
        campo: nombre,
        requerido,
        reglas,
    }
}

pub const PLANTILLAS: &[&str] = &["clasica", "minimal", "boutique", "mercado"];
pub const POSICIONES_FONDO: &[&str] = &["center", "top", "bottom", "left", "right"];
pub const TIPOS_ITEM: &[&str] = &["producto", "servicio", "alquiler"];

pub const CREAR_TIENDA: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 100)]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("slug", false, &[Regla::Texto, Regla::Longitud(2, 80)]),
];

pub const ACTUALIZAR_LOGO: &[ReglaCampo] = &[campo("logo_objeto_id", false, &[Regla::Uuid])];

pub const ACTUALIZAR_BACKGROUND: &[ReglaCampo] = &[
    campo("background_objeto_id", false, &[Regla::Uuid]),
    campo("background_poster_objeto_id", false, &[Regla::Uuid]),
    campo(
        "background_alt",
        false,
        &[Regla::Texto, Regla::Longitud(0, 160)],
    ),
    campo(
        "background_overlay",
        false,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
    campo("background_position", false, &[Regla::En(POSICIONES_FONDO)]),
    campo("background_activo", false, &[Regla::Booleano]),
    campo("background_movil_objeto_id", false, &[Regla::Uuid]),
    campo("background_movil_poster_objeto_id", false, &[Regla::Uuid]),
    campo(
        "background_movil_alt",
        false,
        &[Regla::Texto, Regla::Longitud(0, 160)],
    ),
    campo("background_movil_activo", false, &[Regla::Booleano]),
];

pub const ACTUALIZAR_PERSONALIZACION: &[ReglaCampo] = &[
    campo("plantilla", false, &[Regla::En(PLANTILLAS)]),
    campo("tema", false, &[Regla::Objeto]),
    campo(
        "hero_titulo",
        false,
        &[Regla::Texto, Regla::Longitud(0, 80)],
    ),
    campo(
        "hero_subtitulo",
        false,
        &[Regla::Texto, Regla::Longitud(0, 160)],
    ),
    campo(
        "hero_cta_texto",
        false,
        &[Regla::Texto, Regla::Longitud(0, 40)],
    ),
    campo(
        "hero_cta_url",
        false,
        &[Regla::Texto, Regla::Longitud(0, 300)],
    ),
    campo("seo_titulo", false, &[Regla::Texto, Regla::Longitud(0, 70)]),
    campo(
        "seo_descripcion",
        false,
        &[Regla::Texto, Regla::Longitud(0, 300)],
    ),
    campo("seo_imagen_objeto_id", false, &[Regla::Uuid]),
];

pub const ACTUALIZAR_CONTENIDO: &[ReglaCampo] = &[
    campo("secciones", false, &[Regla::Arreglo]),
    campo(
        "sobre_nosotros",
        false,
        &[Regla::Texto, Regla::Longitud(0, 2000)],
    ),
    campo("politicas", false, &[Regla::Objeto]),
    campo("redes", false, &[Regla::Arreglo]),
    campo(
        "anuncio_texto",
        false,
        &[Regla::Texto, Regla::Longitud(0, 120)],
    ),
    campo(
        "anuncio_enlace",
        false,
        &[Regla::Texto, Regla::Longitud(0, 300)],
    ),
    campo("anuncio_activo", false, &[Regla::Booleano]),
];

pub const ACTUALIZAR_SLUG: &[ReglaCampo] =
    &[campo("slug", true, &[Regla::Texto, Regla::Longitud(2, 80)])];

pub const PUBLICAR_ITEM: &[ReglaCampo] = &[
    campo("tienda_id", false, &[Regla::Texto]),
    campo("item_type", false, &[Regla::En(TIPOS_ITEM)]),
    campo("item_id", true, &[Regla::Texto]),
    campo(
        "margen",
        true,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(90.0)],
    ),
    campo("moneda", false, &[Regla::Coincide("moneda_iso")]),
];

pub const CAMBIAR_MARGEN: &[ReglaCampo] = &[campo(
    "margen",
    true,
    &[Regla::Entero, Regla::Min(0.0), Regla::Max(90.0)],
)];

pub const ASIGNAR_VENDEDOR: &[ReglaCampo] = &[
    campo("usuario_id", true, &[Regla::Uuid]),
    campo(
        "participacion",
        true,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
];

pub const DESASIGNAR_VENDEDOR: &[ReglaCampo] = &[campo("usuario_id", true, &[Regla::Uuid])];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_reportar_los_campos_requeridos_con_sus_restricciones() {
        let error = validar(CREAR_TIENDA, &json!({})).expect_err("validacion");
        assert_eq!(error.cuerpo()["codigo"], "VALIDACION");
        assert_eq!(error.cuerpo()["detalles"][0]["campo"], "nombre");
        assert_eq!(
            error.cuerpo()["detalles"][0]["restricciones"],
            json!(["isString", "isLength"])
        );
    }

    #[test]
    fn debe_ignorar_nulos_opcionales_y_campos_desconocidos() {
        assert!(validar(
            ACTUALIZAR_LOGO,
            &json!({"logo_objeto_id": null, "otro": true})
        )
        .is_ok());
        assert!(validar(
            CREAR_TIENDA,
            &json!({"nombre": "Tienda", "descripcion": "x"})
        )
        .is_ok());
    }

    #[test]
    fn debe_validar_longitudes_enums_uuids_y_booleanos() {
        let error = validar(
            ACTUALIZAR_BACKGROUND,
            &json!({
                "background_objeto_id": "no-uuid",
                "background_overlay": 101,
                "background_position": "arriba",
                "background_activo": "si"
            }),
        )
        .expect_err("validacion");
        let detalles = error.cuerpo()["detalles"].clone();
        let campos = detalles
            .as_array()
            .expect("detalles")
            .iter()
            .map(|detalle| detalle["campo"].as_str().expect("campo").to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            campos,
            vec![
                "background_objeto_id",
                "background_overlay",
                "background_position",
                "background_activo"
            ]
        );
    }

    #[test]
    fn debe_validar_moneda_iso_y_margen() {
        assert!(validar(
            PUBLICAR_ITEM,
            &json!({"item_id": "i-1", "margen": 15, "moneda": "NIO"})
        )
        .is_ok());
        assert!(validar(
            PUBLICAR_ITEM,
            &json!({"item_id": "i-1", "margen": 91, "moneda": "N"})
        )
        .is_err());
        assert!(validar(PUBLICAR_ITEM, &json!({"item_id": "i-1"})).is_err());
    }

    #[test]
    fn debe_validar_participacion_y_uuid_interno() {
        assert!(validar(
            ASIGNAR_VENDEDOR,
            &json!({
                "usuario_id": "130c07c6-7e10-4f91-9c13-e17ca6be6eea",
                "participacion": 40
            })
        )
        .is_ok());
        assert!(validar(
            ASIGNAR_VENDEDOR,
            &json!({"usuario_id": "x", "participacion": 101})
        )
        .is_err());
    }
}
