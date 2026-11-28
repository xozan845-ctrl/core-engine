//! TOTP RFC 6238 (SHA-1, 6 dígitos, 30 s) para MFA opcional.

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub const PERIODO_SEGUNDOS: u64 = 30;
const DIGITOS: u32 = 6;

pub fn generar_secreto() -> String {
    let mut bytes = [0u8; 20];
    rand::rng().fill_bytes(&mut bytes);
    BASE32_NOPAD.encode(&bytes)
}

pub fn uri(secreto: &str, cuenta: &str) -> String {
    format!(
        "otpauth://totp/Core%20Engine:{cuenta}?secret={secreto}&issuer=Core%20Engine&algorithm=SHA1&digits=6&period=30"
    )
}

pub fn codigo(secreto: &str, timestamp: u64) -> Option<String> {
    let clave = BASE32_NOPAD
        .decode(secreto.trim().to_uppercase().as_bytes())
        .ok()?;
    let contador = timestamp / PERIODO_SEGUNDOS;
    let mut mac = HmacSha1::new_from_slice(&clave).ok()?;
    mac.update(&contador.to_be_bytes());
    let resultado = mac.finalize().into_bytes();
    let offset = (resultado[19] & 0x0f) as usize;
    let binario = ((resultado[offset] as u32 & 0x7f) << 24)
        | ((resultado[offset + 1] as u32) << 16)
        | ((resultado[offset + 2] as u32) << 8)
        | (resultado[offset + 3] as u32);
    Some(format!("{:06}", binario % 10u32.pow(DIGITOS)))
}

pub fn verificar(secreto: &str, codigo: &str, timestamp: u64) -> bool {
    let codigo = codigo.trim();
    if codigo.len() != DIGITOS as usize || !codigo.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    [-1i64, 0, 1].iter().any(|delta| {
        let ajustado = timestamp as i64 + delta * PERIODO_SEGUNDOS as i64;
        ajustado >= 0
            && codigo_para(secreto, ajustado as u64).is_some_and(|esperado| esperado == codigo)
    })
}

fn codigo_para(secreto: &str, timestamp: u64) -> Option<String> {
    codigo(secreto, timestamp)
}

pub fn codigo_recuperacion() -> String {
    format!("{:08X}", rand::rng().next_u32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_generar_un_secreto_base32_de_20_bytes() {
        let secreto = generar_secreto();
        assert_eq!(secreto.len(), 32);
        assert!(BASE32_NOPAD.decode(secreto.as_bytes()).is_ok());
    }

    #[test]
    fn debe_coincidir_con_el_vector_rfc_6238() {
        // RFC 6238, SHA-1, T=59 s, secreto "12345678901234567890".
        let secreto = BASE32_NOPAD.encode(b"12345678901234567890");
        assert_eq!(codigo(&secreto, 59).as_deref(), Some("287082"));
        assert!(verificar(&secreto, "287082", 59));
        assert!(verificar(&secreto, "287082", 61));
        assert!(!verificar(&secreto, "000000", 59));
    }

    #[test]
    fn debe_generar_codigos_de_recuperacion_distintos() {
        let uno = codigo_recuperacion();
        let dos = codigo_recuperacion();
        assert_eq!(uno.len(), 8);
        assert_eq!(dos.len(), 8);
        assert_ne!(uno, dos);
    }
}
