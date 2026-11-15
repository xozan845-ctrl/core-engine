//! Montos en centavos (R-DI-1): nunca punto flotante.
//!
//! Réplica de `@core/shared/money.ts` para catálogo, precios, tarifas y stock.

use crate::errors::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money {
    pub centavos: i64,
}

impl Money {
    pub fn desde_centavos(centavos: i64) -> Result<Self, ApiError> {
        if centavos < 0 {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            ));
        }
        Ok(Self { centavos })
    }

    /// Convierte un monto decimal ("1150.00", 1150 o "1150,50") a centavos.
    pub fn parsear(valor: &str) -> Result<Self, ApiError> {
        let texto = valor
            .trim()
            .replace(',', ".")
            .replace(char::is_whitespace, "");
        let partes: Vec<&str> = texto.split('.').collect();
        let valido = (partes.len() == 1 || partes.len() == 2)
            && !partes[0].is_empty()
            && partes[0].chars().all(|caracter| caracter.is_ascii_digit())
            && partes
                .get(1)
                .map(|decimales| {
                    (1..=2).contains(&decimales.len())
                        && decimales.chars().all(|caracter| caracter.is_ascii_digit())
                })
                .unwrap_or(true);
        if !valido {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                format!(
                    "Formato de monto invalido: \"{valor}\". Use numero con hasta 2 decimales."
                ),
            ));
        }
        let enteros: i64 = partes[0].parse().map_err(|_| {
            ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            )
        })?;
        let decimales = partes
            .get(1)
            .map(|valor| format!("{valor:0<2}"))
            .unwrap_or_else(|| "00".to_string());
        let decimales: i64 = decimales.parse().map_err(|_| {
            ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            )
        })?;
        Self::desde_centavos(enteros.saturating_mul(100).saturating_add(decimales))
    }

    pub fn sumar(&self, otro: Money) -> Result<Self, ApiError> {
        Self::desde_centavos(self.centavos.saturating_add(otro.centavos))
    }

    pub fn restar(&self, otro: Money) -> Result<Self, ApiError> {
        Self::desde_centavos(self.centavos.saturating_sub(otro.centavos))
    }

    pub fn multiplicar_por(&self, ratio: f64) -> Result<Self, ApiError> {
        Self::desde_centavos((self.centavos as f64 * ratio).round() as i64)
    }

    pub fn aplicar_margen(&self, margen: i64) -> Result<Self, ApiError> {
        if !(0..=90).contains(&margen) {
            return Err(ApiError::dominio(
                "MARGEN_INVALIDO",
                "El margen debe estar entre 0 y 90 % (RN-01).",
            ));
        }
        let centavos =
            (self.centavos as f64 + (self.centavos as f64 * margen as f64) / 100.0).round() as i64;
        Self::desde_centavos(centavos)
    }

    /// Serialización "1150.00".
    pub fn string(&self) -> String {
        format!("{}.{:02}", self.centavos / 100, self.centavos % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_parsear_y_serializar_montos() {
        assert_eq!(Money::parsear("1150.00").expect("monto").centavos, 115_000);
        assert_eq!(Money::parsear("1150").expect("monto").centavos, 115_000);
        assert_eq!(Money::parsear("1150,5").expect("monto").centavos, 115_050);
        assert_eq!(Money::parsear("0.01").expect("monto").string(), "0.01");
        assert_eq!(
            Money::parsear("1150.00").expect("monto").string(),
            "1150.00"
        );
        assert!(Money::parsear("1.234").is_err());
        assert!(Money::parsear("-1").is_err());
    }

    #[test]
    fn debe_aplicar_margen_con_redondeo() {
        let base = Money::desde_centavos(115_000).expect("base");
        assert_eq!(base.aplicar_margen(15).expect("margen").centavos, 132_250);
        assert_eq!(base.aplicar_margen(90).expect("margen").centavos, 218_500);
        assert!(base.aplicar_margen(91).is_err());
    }
}
