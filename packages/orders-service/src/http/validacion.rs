//! Validación de DTOs con el mismo shape de error que `class-validator` +
//! `DtoValidationPipe`: `VALIDACION` con `detalles[{campo, restricciones}]`.
//!
//! Las reglas replican los decoradores declarados en los DTOs y el `whitelist`
//! del pipe (los campos desconocidos se ignoran).

use crate::errors::ApiError;
use chrono::NaiveDate;
use regex::Regex;
use serde_json::{json, Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Regla {
    Texto,
    NoVacio,
    En(&'static [&'static str]),
    Entero,
    Min(i64),
    Max(i64),
    Booleano,
    Arreglo,
    Objeto,
    Uuid,
    Patron(&'static str),
    Fecha,
    /// `@Length(min, max)`.
    Longitud(usize, usize),
    /// `@MaxLength(max)`.
    LongitudMax(usize),
}

impl Regla {
    fn nombre(self) -> &'static str {
        match self {
            Regla::Texto => "isString",
            Regla::NoVacio => "isNotEmpty",
            Regla::En(_) => "isIn",
            Regla::Entero => "isInt",
            Regla::Min(_) => "min",
            Regla::Max(_) => "max",
            Regla::Booleano => "isBoolean",
            Regla::Arreglo => "isArray",
            Regla::Objeto => "isObject",
            Regla::Uuid => "isUuid",
            Regla::Patron(_) => "matches",
            Regla::Fecha => "matches",
            Regla::Longitud(_, _) => "isLength",
            Regla::LongitudMax(_) => "maxLength",
        }
    }

    fn cumple(self, valor: &Value) -> bool {
        match self {
            Regla::Texto => valor.is_string(),
            Regla::NoVacio => valor.as_str().is_some_and(|texto| !texto.trim().is_empty()),
            Regla::En(opciones) => valor
                .as_str()
                .is_some_and(|texto| opciones.contains(&texto)),
            Regla::Entero => valor
                .as_f64()
                .is_some_and(|numero| numero.is_finite() && numero.fract() == 0.0),
            Regla::Min(minimo) => valor.as_f64().is_some_and(|numero| numero >= minimo as f64),
            Regla::Max(maximo) => valor.as_f64().is_some_and(|numero| numero <= maximo as f64),
            Regla::Booleano => valor.is_boolean(),
            Regla::Arreglo => valor.is_array(),
            Regla::Objeto => valor.is_object(),
            Regla::Uuid => valor
                .as_str()
                .is_some_and(|texto| Uuid::parse_str(texto).is_ok()),
            Regla::Patron(patron) => valor
                .as_str()
                .is_some_and(|texto| Regex::new(patron).is_ok_and(|regex| regex.is_match(texto))),
            Regla::Fecha => valor
                .as_str()
                .is_some_and(|texto| NaiveDate::parse_from_str(texto, "%Y-%m-%d").is_ok()),
            Regla::Longitud(minimo, maximo) => valor.as_str().is_some_and(|texto| {
                let largo = texto.chars().count();
                (minimo..=maximo).contains(&largo)
            }),
            Regla::LongitudMax(maximo) => valor
                .as_str()
                .is_some_and(|texto| texto.chars().count() <= maximo),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReglaCampo {
    pub campo: &'static str,
    pub requerido: bool,
    pub reglas: &'static [Regla],
}

pub const fn campo(campo: &'static str, requerido: bool, reglas: &'static [Regla]) -> ReglaCampo {
    ReglaCampo {
        campo,
        requerido,
        reglas,
    }
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

/// Valida y devuelve el objeto. Un cuerpo que no es objeto se trata como vacío
/// (mismo comportamiento que `class-validator` con un array/escalar).
pub fn objeto(cuerpo: &Value) -> Map<String, Value> {
    cuerpo.as_object().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESQUEMA: &[ReglaCampo] = &[
        campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 5)]),
        campo(
            "cantidad",
            true,
            &[Regla::Entero, Regla::Min(1), Regla::Max(99)],
        ),
        campo("tipo", false, &[Regla::En(&["a", "b"])]),
        campo("fecha", false, &[Regla::Fecha]),
    ];

    #[test]
    fn debe_aceptar_un_cuerpo_valido() {
        assert!(validar(
            ESQUEMA,
            &json!({"nombre": "ok", "cantidad": 3, "tipo": "a", "fecha": "2026-01-01"})
        )
        .is_ok());
    }

    #[test]
    fn debe_acumular_las_restricciones_fallidas() {
        let error = validar(
            ESQUEMA,
            &json!({"nombre": "x", "cantidad": 1.5, "tipo": "z", "fecha": "2026-13-01"}),
        )
        .expect_err("invalido");
        assert_eq!(error.codigo, "VALIDACION");
        let detalles = error.detalles.expect("detalles");
        assert_eq!(detalles.as_array().expect("array").len(), 4);
        assert_eq!(detalles[0]["campo"], "nombre");
        assert_eq!(detalles[0]["restricciones"][0], "isLength");
    }

    #[test]
    fn debe_marcar_los_campos_requeridos_ausentes() {
        let error = validar(ESQUEMA, &json!({})).expect_err("invalido");
        let detalles = error.detalles.expect("detalles");
        assert_eq!(detalles[0]["campo"], "nombre");
        assert_eq!(detalles[1]["campo"], "cantidad");
    }

    #[test]
    fn debe_tolerar_un_cuerpo_que_no_es_objeto() {
        let error = validar(ESQUEMA, &json!([])).expect_err("invalido");
        assert_eq!(error.codigo, "VALIDACION");
    }
}
