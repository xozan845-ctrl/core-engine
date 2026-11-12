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

pub fn restar(minuendo: i64, sustraendo: i64) -> Result<i64, ApiError> {
    validar_centavos(minuendo - sustraendo)
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
    format!("{}.{:02}", centavos / 100, centavos % 100)
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
    fn debe_rechazar_una_tasa_fuera_de_rango() {
        for tasa in [0.0, 1.0, -0.1, 1.1, f64::NAN] {
            let error = comision(100, tasa).expect_err("tasa invalida");
            assert_eq!(error.codigo, "TASA_INVALIDA");
        }
    }

    #[test]
    fn debe_redondear_la_comision_como_javascript() {
        assert_eq!(comision(10_000, 0.12).expect("comision"), 1_200);
        assert_eq!(comision(1, 0.5).expect("comision"), 1);
        assert_eq!(comision(3, 0.5).expect("comision"), 2);
        assert_eq!(comision(115_000, 0.12).expect("comision"), 13_800);
    }

    #[test]
    fn debe_formatear_centavos_con_dos_decimales() {
        assert_eq!(string(0), "0.00");
        assert_eq!(string(5), "0.05");
        assert_eq!(string(115_000), "1150.00");
        assert_eq!(string(1_234_567), "12345.67");
    }

    #[test]
    fn debe_cubrir_el_rango_de_participaciones_y_ventas() {
        // Property test acotado: para cualquier venta positiva y participación
        // entera, la suma de las partes es el neto y ninguna es negativa.
        for venta in [1_i64, 2, 99, 100, 10_000, 111_111, 1_000_000] {
            for participacion in 0..=100 {
                for tasa in [0.05_f64, 0.12, 0.25, 0.5, 0.9] {
                    let comision_cents = comision(venta, tasa).expect("comision");
                    let neto = restar(venta, comision_cents).expect("neto");
                    let vendedor = if participacion == 100 {
                        neto
                    } else if participacion == 0 {
                        0
                    } else {
                        comision(neto, participacion as f64 / 100.0).expect("reparto")
                    };
                    let empresario = restar(neto, vendedor).expect("empresario");
                    assert_eq!(vendedor + empresario, neto);
                    assert!(vendedor >= 0 && empresario >= 0);
                }
            }
        }
    }
}
