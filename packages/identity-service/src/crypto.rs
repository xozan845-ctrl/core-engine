//! Cifrado de secretos en reposo (TOTP) con AES-256-GCM.
//!
//! Formato: `v1:<nonce_base64>:<ciphertext_base64>`. Los valores sin prefijo
//! `v1:` se tratan como texto legacy para permitir migraciones suaves.

use crate::errors::ApiError;
use base64::Engine;
use rand::Rng;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::digest::{digest, SHA256};
use ring::hmac;

const PREFIJO: &str = "v1:";

/// SHA-256 en hexadecimal minúsculas.
pub fn sha256_hex(datos: &[u8]) -> String {
    let resumen = digest(&SHA256, datos);
    resumen
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// HMAC-SHA256 en hexadecimal minúsculas (firma de exportaciones).
pub fn hmac_sha256_hex(clave: &[u8], datos: &[u8]) -> String {
    let clave = hmac::Key::new(hmac::HMAC_SHA256, clave);
    hmac::sign(&clave, datos)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub struct CifradoSecreto {
    clave: LessSafeKey,
}

impl CifradoSecreto {
    pub fn nueva(clave: [u8; 32]) -> Result<Self, String> {
        let clave = UnboundKey::new(&AES_256_GCM, &clave)
            .map_err(|_| "clave AES-256-GCM invalida".to_string())?;
        Ok(Self {
            clave: LessSafeKey::new(clave),
        })
    }

    pub fn cifrar(&self, texto: &str) -> Result<String, ApiError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut salida = texto.as_bytes().to_vec();
        self.clave
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut salida)
            .map_err(|_| ApiError::interno())?;
        let motor = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        Ok(format!(
            "{PREFIJO}{}:{}",
            motor.encode(nonce_bytes),
            motor.encode(salida)
        ))
    }

    pub fn descifrar(&self, valor: &str) -> Result<String, ApiError> {
        let Some(resto) = valor.strip_prefix(PREFIJO) else {
            return Ok(valor.to_string());
        };
        let mut partes = resto.splitn(2, ':');
        let nonce_b64 = partes.next().unwrap_or_default();
        let cipher_b64 = partes.next().unwrap_or_default();
        let motor = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let nonce_bytes = motor.decode(nonce_b64).map_err(|_| ApiError::interno())?;
        if nonce_bytes.len() != NONCE_LEN {
            return Err(ApiError::interno());
        }
        let mut nonce_array = [0u8; NONCE_LEN];
        nonce_array.copy_from_slice(&nonce_bytes);
        let mut cipher = motor.decode(cipher_b64).map_err(|_| ApiError::interno())?;
        let nonce = Nonce::assume_unique_for_key(nonce_array);
        let claro = self
            .clave
            .open_in_place(nonce, Aad::empty(), &mut cipher)
            .map_err(|_| ApiError::interno())?;
        String::from_utf8(claro.to_vec()).map_err(|_| ApiError::interno())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_cifrar_y_descifrar() {
        let cifrado = CifradoSecreto::nueva([7u8; 32]).expect("clave");
        let valor = cifrado.cifrar("JBSWY3DPEHPK3PXP").expect("cifrar");
        assert!(valor.starts_with("v1:"));
        assert!(!valor.contains("JBSWY3DPEHPK3PXP"));
        assert_eq!(
            cifrado.descifrar(&valor).expect("descifrar"),
            "JBSWY3DPEHPK3PXP"
        );
    }

    #[test]
    fn debe_rechazar_una_clave_distinta() {
        let uno = CifradoSecreto::nueva([1u8; 32]).expect("uno");
        let dos = CifradoSecreto::nueva([2u8; 32]).expect("dos");
        let valor = uno.cifrar("secreto").expect("cifrar");
        assert!(dos.descifrar(&valor).is_err());
    }

    #[test]
    fn debe_firmar_de_forma_determinista() {
        let firma = hmac_sha256_hex(b"clave", b"contenido");
        assert_eq!(firma, hmac_sha256_hex(b"clave", b"contenido"));
        assert_ne!(firma, hmac_sha256_hex(b"otra", b"contenido"));
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }

    #[test]
    fn debe_aceptar_texto_legacy() {
        let cifrado = CifradoSecreto::nueva([3u8; 32]).expect("clave");
        assert_eq!(cifrado.descifrar("legacy").expect("legacy"), "legacy");
    }
}
