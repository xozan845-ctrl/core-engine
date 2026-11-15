//! Dinero en centavos con la misma semántica que `@core/shared/money.ts`.
//!
//! Los montos nunca se manipulan en punto flotante salvo el redondeo explícito
//! de una tasa o un margen, idéntico a `Math.round` de JavaScript.

use crate::errors::ApiError;

pub const MAX_SAFE_CENTS: i64 = 9_007_199_254_740_991;
pub const MARGEN_MINIMO: i64 = 0;
pub const MARGEN_MAXIMO: i64 = 90;

pub fn validar_centavos(centavos: i64) -> Result<i64, ApiError> {
    if !(0..=MAX_SAFE_CENTS).contains(&centavos) {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "El monto debe ser un entero no negativo en centavos.",
        ));
    }
    Ok(centavos)
}

pub fn sumar(a: i64, b: i64) -> i64 {
    a + b
}

/// Convierte `"1150.00"`, `1150` o `"1150,50"` a centavos enteros.
pub fn parsear(valor: &str) -> Result<i64, ApiError> {
    let texto = valor.trim().replace(',', ".").replace(' ', "");
    if !es_monto_valido(&texto) {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            format!("Formato de monto invalido: \"{valor}\". Use numero con hasta 2 decimales."),
        ));
    }
    let partes: Vec<&str> = texto.split('.').collect();
    let enteros: i64 = partes[0].parse().map_err(|_| {
        ApiError::dominio(
            "MONTO_INVALIDO",
            format!("Formato de monto invalido: \"{valor}\"."),
        )
    })?;
    let decimales = partes.get(1).copied().unwrap_or("").to_string();
    let decimales = format!("{decimales:0<2}");
    let decimales: i64 = decimales.parse().map_err(|_| {
        ApiError::dominio(
            "MONTO_INVALIDO",
            format!("Formato de monto invalido: \"{valor}\"."),
        )
    })?;
    validar_centavos(enteros * 100 + decimales)
}

fn es_monto_valido(texto: &str) -> bool {
    let mut partes = texto.split('.');
    let Some(enteros) = partes.next() else {
        return false;
    };
    if enteros.is_empty() || !enteros.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match partes.next() {
        None => true,
        Some(decimales) => {
            !decimales.is_empty()
                && decimales.len() <= 2
                && decimales.chars().all(|c| c.is_ascii_digit())
                && partes.next().is_none()
        }
    }
}

/// RN-01: precio final = base + base × margen / 100, redondeando como JS.
pub fn aplicar_margen(centavos: i64, margen: i64) -> Result<i64, ApiError> {
    if !(MARGEN_MINIMO..=MARGEN_MAXIMO).contains(&margen) {
        return Err(ApiError::dominio(
            "MARGEN_INVALIDO",
            "El margen debe estar entre 0 y 90 % (RN-01).",
        ));
    }
    validar_centavos(centavos + ((centavos as f64 * margen as f64) / 100.0).round() as i64)
}

pub fn multiplicar(centavos: i64, cantidad: i64) -> i64 {
    centavos * cantidad
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
    fn debe_parsear_montos_decimales_y_enteros() {
        assert_eq!(parsear("1150.00").expect("monto"), 115_000);
        assert_eq!(parsear("1150,5").expect("monto"), 115_050);
        assert_eq!(parsear(" 12 ").expect("monto"), 1_200);
        assert_eq!(parsear("0.05").expect("monto"), 5);
    }

    #[test]
    fn debe_rechazar_formatos_invalidos() {
        for valor in ["", "abc", "-1", "1.234", "1.2.3", ".5"] {
            assert_eq!(
                parsear(valor).expect_err("invalido").codigo,
                "MONTO_INVALIDO"
            );
        }
    }

    #[test]
    fn debe_aplicar_el_margen_como_javascript() {
        assert_eq!(aplicar_margen(10_000, 15).expect("margen"), 11_500);
        assert_eq!(aplicar_margen(3, 50).expect("margen"), 5);
        assert_eq!(aplicar_margen(1, 50).expect("margen"), 2);
        assert_eq!(aplicar_margen(10_000, 0).expect("margen"), 10_000);
    }

    #[test]
    fn debe_rechazar_un_margen_fuera_de_rango() {
        assert_eq!(
            aplicar_margen(100, 91).expect_err("margen").codigo,
            "MARGEN_INVALIDO"
        );
    }

    #[test]
    fn debe_calcular_y_formatear_la_comision() {
        assert_eq!(comision(10_000, 0.12).expect("comision"), 1_200);
        assert_eq!(comision(1, 0.5).expect("comision"), 1);
        assert_eq!(string(0), "0.00");
        assert_eq!(string(5), "0.05");
        assert_eq!(string(115_000), "1150.00");
    }
}
