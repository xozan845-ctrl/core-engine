//! Dinero en centavos con la misma semántica que `@core/shared/money.ts`.
//!
//! Los montos nunca se manipulan en punto flotante salvo el redondeo explícito
//! de una tasa o participación, idéntico a `Math.round` de JavaScript.

use crate::errors::ApiError;

/// `Number.MAX_SAFE_INTEGER`, límite que valida `Money`.
pub const MAX_SAFE_CENTS: i64 = 9_007_199_254_740_991;

pub fn validar_centavos(centavos: i64) -> Result<i64, ApiError> {
    if !(0..=MAX_SAFE_CENTS).contains(&centavos) {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "El monto debe ser un entero no negativo en centavos.",
        ));
    }
    Ok(centavos)
}

pub fn desde_centavos(centavos: i64) -> Result<i64, ApiError> {
    validar_centavos(centavos)
}

pub fn sumar(a: i64, b: i64) -> Result<i64, ApiError> {
    validar_centavos(a + b)
}

pub fn restar(minuendo: i64, sustraendo: i64) -> Result<i64, ApiError> {
    validar_centavos(minuendo - sustraendo)
}

/// `Money.parsear`: acepta `"1150"`, `"1150.00"`, `"1150,50"` y números.
pub fn parsear(valor: &str) -> Result<i64, ApiError> {
    let texto = valor.trim().replace(',', ".").replace(' ', "");
    let partes: Vec<&str> = texto.split('.').collect();
    let formato_ok = match partes.as_slice() {
        [enteros] => !enteros.is_empty() && enteros.chars().all(|c| c.is_ascii_digit()),
        [enteros, decimales] => {
            !enteros.is_empty()
                && enteros.chars().all(|c| c.is_ascii_digit())
                && (1..=2).contains(&decimales.len())
                && decimales.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    };
    if !formato_ok {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            format!("Formato de monto invalido: \"{valor}\". Use numero con hasta 2 decimales."),
        ));
    }
    let enteros: i64 = partes[0].parse().map_err(|_| {
        ApiError::dominio(
            "MONTO_INVALIDO",
            format!("Formato de monto invalido: \"{valor}\". Use numero con hasta 2 decimales."),
        )
    })?;
    let decimales: i64 = partes
        .get(1)
        .map(|decimales| format!("{decimales:0<2}").parse().unwrap_or(0))
        .unwrap_or(0);
    validar_centavos(enteros.saturating_mul(100).saturating_add(decimales))
}

pub fn parsear_opcional(valor: Option<&serde_json::Value>) -> Result<i64, ApiError> {
    match valor {
        None | Some(serde_json::Value::Null) => Ok(0),
        Some(serde_json::Value::Number(numero)) => {
            if let Some(entero) = numero.as_i64() {
                validar_centavos(entero)
            } else {
                parsear(&numero.to_string())
            }
        }
        Some(serde_json::Value::String(texto)) => parsear(texto),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: use numero con hasta 2 decimales.",
        )),
    }
}

pub fn multiplicar_por(centavos: i64, ratio: f64) -> Result<i64, ApiError> {
    validar_centavos((centavos as f64 * ratio).round() as i64)
}

/// `Money.comision`: exige `0 < tasa < 1` y redondea como `Math.round`.
pub fn comision(centavos: i64, tasa: f64) -> Result<i64, ApiError> {
    if !tasa.is_finite() || tasa <= 0.0 || tasa >= 1.0 {
        return Err(ApiError::dominio(
            "TASA_INVALIDA",
            format!("La tasa debe estar entre 0 y 1 (recibida: {tasa})."),
        ));
    }
    validar_centavos((centavos as f64 * tasa).round() as i64)
}

/// Serialización `"1150.00"` (dos decimales, sin separador de miles).
pub fn string(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, centavos.rem_euclid(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_rechazar_centavos_negativos() {
        let error = validar_centavos(-1).expect_err("negativo");
        assert_eq!(error.codigo, "MONTO_INVALIDO");
    }

    #[test]
    fn debe_parsear_enteros_decimales_y_coma() {
        assert_eq!(parsear("1150").expect("monto"), 115_000);
        assert_eq!(parsear("1150.00").expect("monto"), 115_000);
        assert_eq!(parsear("1150,5").expect("monto"), 115_050);
        assert_eq!(parsear("0.07").expect("monto"), 7);
    }

    #[test]
    fn debe_rechazar_formatos_de_monto_invalidos() {
        for valor in ["", "abc", "1.234", "-5", "1,2,3"] {
            let error = parsear(valor).expect_err("invalido");
            assert_eq!(error.codigo, "MONTO_INVALIDO");
        }
    }

    #[test]
    fn debe_formatear_con_dos_decimales() {
        assert_eq!(string(115_000), "1150.00");
        assert_eq!(string(7), "0.07");
        assert_eq!(string(0), "0.00");
    }

    #[test]
    fn debe_redondear_como_javascript() {
        assert_eq!(comision(100, 0.12).expect("comision"), 12);
        assert_eq!(comision(999, 0.12).expect("comision"), 120);
        assert_eq!(multiplicar_por(100, 0.125).expect("iva"), 13);
    }

    #[test]
    fn debe_rechazar_una_tasa_fuera_de_rango() {
        for tasa in [0.0, 1.0, -0.1, 1.1, f64::NAN] {
            let error = comision(100, tasa).expect_err("tasa invalida");
            assert_eq!(error.codigo, "TASA_INVALIDA");
        }
    }
}
