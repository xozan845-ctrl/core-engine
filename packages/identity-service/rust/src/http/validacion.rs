//! Validación de DTOs con el mismo shape de error que `class-validator` +
//! `DtoValidationPipe`: `VALIDACION` con `detalles[{campo, restricciones}]`.
//!
//! Las reglas replican los decoradores de los controladores Nest y el
//! `whitelist` del pipe (los campos desconocidos se ignoran).

use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Regla {
    Texto,
    Numero,
    Entero,
    Booleano,
    Arreglo,
    Objeto,
    Uuid,
    Fecha,
    Correo,
    MinLongitud(usize),
    MaxLongitud(usize),
    En(&'static [&'static str]),
    Min(f64),
    Max(f64),
    Longitud(usize, usize),
    Coincide(&'static str),
}

impl Regla {
    fn nombre(self) -> &'static str {
        match self {
            Regla::Texto => "isString",
            Regla::Numero => "isNumber",
            Regla::Entero => "isInt",
            Regla::Booleano => "isBoolean",
            Regla::Arreglo => "isArray",
            Regla::Objeto => "isObject",
            Regla::Uuid => "isUUID",
            Regla::Fecha => "isDateString",
            Regla::Correo => "isEmail",
            Regla::MinLongitud(_) => "minLength",
            Regla::MaxLongitud(_) => "maxLength",
            Regla::En(_) => "isIn",
            Regla::Min(_) => "min",
            Regla::Max(_) => "max",
            Regla::Longitud(_, _) => "isLength",
            Regla::Coincide(_) => "matches",
        }
    }

    fn cumple(self, valor: &Value) -> bool {
        match self {
            Regla::Texto => valor.is_string(),
            Regla::Numero => valor.as_f64().is_some_and(f64::is_finite),
            Regla::Entero => valor
                .as_f64()
                .is_some_and(|numero| numero.is_finite() && numero.fract() == 0.0),
            Regla::Booleano => valor.is_boolean(),
            Regla::Arreglo => valor.is_array(),
            Regla::Objeto => valor.is_object(),
            Regla::Uuid => valor
                .as_str()
                .is_some_and(|texto| Uuid::parse_str(texto).is_ok()),
            Regla::Fecha => valor.as_str().is_some_and(es_fecha_iso),
            Regla::Correo => valor.as_str().is_some_and(es_correo),
            Regla::MinLongitud(minimo) => valor
                .as_str()
                .is_some_and(|texto| texto.chars().count() >= minimo),
            Regla::MaxLongitud(maximo) => valor
                .as_str()
                .is_some_and(|texto| texto.chars().count() <= maximo),
            Regla::En(opciones) => valor
                .as_str()
                .is_some_and(|texto| opciones.contains(&texto)),
            Regla::Min(minimo) => valor.as_f64().is_some_and(|numero| numero >= minimo),
            Regla::Max(maximo) => valor.as_f64().is_some_and(|numero| numero <= maximo),
            Regla::Longitud(minimo, maximo) => valor.as_str().is_some_and(|texto| {
                let longitud = texto.chars().count();
                longitud >= minimo && longitud <= maximo
            }),
            Regla::Coincide(patron) => valor.as_str().is_some_and(|texto| coincide(texto, patron)),
        }
    }
}

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
        "codigo_barras" => !texto.trim().is_empty(),
        "sku" => {
            (2..=32).contains(&texto.len())
                && texto
                    .chars()
                    .all(|caracter| caracter.is_ascii_alphanumeric() || caracter == '-')
        }
        "monto" => {
            let partes: Vec<&str> = texto.split('.').collect();
            (partes.len() == 1 || partes.len() == 2)
                && !partes[0].is_empty()
                && partes[0].chars().all(|caracter| caracter.is_ascii_digit())
                && partes
                    .get(1)
                    .map(|decimales| {
                        (1..=2).contains(&decimales.len())
                            && decimales.chars().all(|caracter| caracter.is_ascii_digit())
                    })
                    .unwrap_or(true)
        }
        "serial" => {
            (1..=120).contains(&texto.chars().count())
                && texto.chars().all(|caracter| {
                    caracter.is_ascii_alphanumeric()
                        || caracter == '.'
                        || caracter == '_'
                        || caracter == '-'
                })
        }
        "imei" => texto.len() <= 20 && texto.chars().all(|caracter| caracter.is_ascii_digit()),
        "fecha_corta" => {
            let bytes = texto.as_bytes();
            bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes[..4].iter().all(u8::is_ascii_digit)
                && bytes[5..7].iter().all(u8::is_ascii_digit)
                && bytes[8..].iter().all(u8::is_ascii_digit)
        }
        "segmento" => {
            let mut caracteres = texto.chars();
            matches!(caracteres.next(), Some(primero) if primero.is_ascii_lowercase())
                && (1..=40).contains(&texto.chars().count())
                && texto.chars().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || caracter == '_'
                        || caracter == '-'
                })
        }
        "codigo_ubicacion" => {
            let mut caracteres = texto.chars();
            matches!(caracteres.next(), Some(primero) if primero.is_ascii_lowercase())
                && (2..=40).contains(&texto.chars().count())
                && texto.chars().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || caracter == '_'
                        || caracter == '-'
                })
        }
        _ => false,
    }
}

fn es_correo(texto: &str) -> bool {
    crate::dominio::usuarios::correo_valido(texto)
}

fn es_fecha_iso(texto: &str) -> bool {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};
    if DateTime::parse_from_rfc3339(texto).is_ok() {
        return true;
    }
    if NaiveDate::parse_from_str(texto, "%Y-%m-%d").is_ok() {
        return true;
    }
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ]
    .iter()
    .any(|formato| NaiveDateTime::parse_from_str(texto, formato).is_ok())
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

pub const fn campo(nombre: &'static str, requerido: bool, reglas: &'static [Regla]) -> ReglaCampo {
    ReglaCampo {
        campo: nombre,
        requerido,
        reglas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESQUEMA: &[ReglaCampo] = &[
        campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 10)]),
        campo("precio", true, &[Regla::Entero, Regla::Min(0.0)]),
        campo("activo", false, &[Regla::Booleano]),
    ];

    #[test]
    fn debe_reportar_campos_requeridos_e_invalidos() {
        let error = validar(ESQUEMA, &json!({})).expect_err("validacion");
        assert_eq!(error.cuerpo()["codigo"], "VALIDACION");
        assert_eq!(error.cuerpo()["detalles"][0]["campo"], "nombre");
        assert_eq!(
            error.cuerpo()["detalles"][0]["restricciones"],
            json!(["isString", "isLength"])
        );
        let error = validar(
            ESQUEMA,
            &json!({"nombre": "x", "precio": -1, "activo": "si"}),
        )
        .expect_err("validacion");
        let detalles = error.cuerpo()["detalles"].clone();
        assert_eq!(detalles[0]["restricciones"], json!(["isLength"]));
        assert_eq!(detalles[2]["restricciones"], json!(["isBoolean"]));
        assert!(validar(ESQUEMA, &json!({"nombre": "ok", "precio": 1})).is_ok());
    }
}
