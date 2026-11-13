//! Montos en centavos (R-C-5): enteros, nunca punto flotante.
//!
//! Réplica de `Money.string()` de `@core/shared`.

/// Serializa centavos como `"1150.00"` (dos decimales, sin separador de miles).
pub fn centavos_a_texto(centavos: i64) -> String {
    let signo = if centavos < 0 { "-" } else { "" };
    let absoluto = centavos.unsigned_abs();
    format!("{signo}{}.{:02}", absoluto / 100, absoluto % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_serializar_centavos_con_dos_decimales() {
        assert_eq!(centavos_a_texto(115_000), "1150.00");
        assert_eq!(centavos_a_texto(12_345), "123.45");
        assert_eq!(centavos_a_texto(5), "0.05");
        assert_eq!(centavos_a_texto(0), "0.00");
    }
}
