//! Slugs legibles para empresas y tiendas (ADR-26).
//!
//! Réplica de `packages/shared/src/slug.ts`: normalización determinista a
//! `[a-z0-9-]`, reservados y rechazo de UUID.

use crate::errors::ApiError;
use unicode_normalization::UnicodeNormalization;

pub const SLUG_MIN_LENGTH: usize = 2;
pub const SLUG_MAX_LENGTH: usize = 80;

pub const SLUGS_RESERVADOS: [&str; 37] = [
    "admin",
    "administrador",
    "api",
    "app",
    "auth",
    "carrito",
    "catalog",
    "docs",
    "empresa",
    "empresas",
    "empresario",
    "empresarios",
    "envios",
    "health",
    "internal",
    "login",
    "logout",
    "me",
    "metrics",
    "mis",
    "nuevo",
    "nueva",
    "ofertas",
    "orders",
    "pagos",
    "productos",
    "public",
    "ready",
    "registro",
    "reportes",
    "shop",
    "static",
    "tienda",
    "tiendas",
    "todas",
    "todos",
    "vendedor",
];

pub fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

pub fn normalizar_slug(valor: &str) -> String {
    let sin_diacriticos: String = valor
        .nfd()
        .filter(|caracter| !unicode_normalization::char::is_combining_mark(*caracter))
        .collect();
    let mut salida = String::new();
    for caracter in sin_diacriticos.to_lowercase().chars() {
        if caracter.is_ascii_alphanumeric() {
            salida.push(caracter);
        } else if !salida.is_empty() && !salida.ends_with('-') {
            salida.push('-');
        }
    }
    let recortado = salida.chars().take(SLUG_MAX_LENGTH).collect::<String>();
    recortado.trim_end_matches('-').to_string()
}

pub fn es_slug_valido(valor: &str) -> bool {
    let longitud = valor.chars().count();
    (SLUG_MIN_LENGTH..=SLUG_MAX_LENGTH).contains(&longitud)
        && es_formato_slug(valor)
        && !SLUGS_RESERVADOS.contains(&valor)
        && !es_uuid(valor)
}

fn es_formato_slug(valor: &str) -> bool {
    if valor.is_empty() || valor.starts_with('-') || valor.ends_with('-') {
        return false;
    }
    if valor.contains("--") {
        return false;
    }
    valor.chars().all(|caracter| {
        caracter.is_ascii_lowercase() || caracter.is_ascii_digit() || caracter == '-'
    })
}

pub fn slug_desde_nombre(nombre: &str, fallback: &str) -> String {
    let mut base = normalizar_slug(nombre);
    if base.is_empty() {
        base = normalizar_slug(fallback);
        if base.is_empty() {
            base = "entidad".to_string();
        }
    }
    if base.chars().count() < SLUG_MIN_LENGTH {
        let candidato = normalizar_slug(&format!("{base}-1"));
        base = if candidato.is_empty() {
            "entidad-1".to_string()
        } else {
            candidato
        };
    }
    if SLUGS_RESERVADOS.contains(&base.as_str()) {
        base = slug_con_sufijo(&base, 1);
    }
    if es_uuid(&base) {
        base = format!("{base}-x");
    }
    base.chars()
        .take(SLUG_MAX_LENGTH)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

pub fn slug_con_sufijo(base: &str, numero: u32) -> String {
    let sufijo = format!("-{numero}");
    let maximo_base = SLUG_MAX_LENGTH
        .saturating_sub(sufijo.len())
        .max(SLUG_MIN_LENGTH);
    let recorte = base
        .chars()
        .take(maximo_base)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();
    format!(
        "{}{sufijo}",
        if recorte.is_empty() {
            "entidad"
        } else {
            &recorte
        }
    )
}

pub fn validar_slug_solicitado(valor: &str) -> Result<String, ApiError> {
    let slug = normalizar_slug(valor);
    if slug.chars().count() < SLUG_MIN_LENGTH {
        return Err(ApiError::dominio(
            "SLUG_INVALIDO",
            format!(
                "El slug debe tener entre {SLUG_MIN_LENGTH} y {SLUG_MAX_LENGTH} caracteres alfanumericos o guiones."
            ),
        ));
    }
    if SLUGS_RESERVADOS.contains(&slug.as_str()) {
        return Err(ApiError::dominio(
            "SLUG_RESERVADO",
            format!("El slug \"{slug}\" está reservado por la plataforma."),
        ));
    }
    if es_uuid(&slug) {
        return Err(ApiError::dominio(
            "SLUG_INVALIDO",
            "El slug no puede tener formato de UUID.",
        ));
    }
    if !es_formato_slug(&slug) {
        return Err(ApiError::dominio(
            "SLUG_INVALIDO",
            "El slug solo admite letras minúsculas, números y guiones.",
        ));
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_acentos_espacios_y_simbolos() {
        assert_eq!(normalizar_slug("Café  Ñandú & Co."), "cafe-nandu-co");
        assert_eq!(normalizar_slug("  Hola---Mundo  "), "hola-mundo");
        assert_eq!(normalizar_slug("¡¡!!"), "");
    }

    #[test]
    fn debe_rechazar_reservados_y_uuids() {
        assert!(!es_slug_valido("admin"));
        assert!(!es_slug_valido("11111111-1111-4111-8111-111111111111"));
        assert!(es_slug_valido("mi-tienda"));
        assert_eq!(
            validar_slug_solicitado("Admin")
                .expect_err("reservado")
                .codigo,
            "SLUG_RESERVADO"
        );
    }

    #[test]
    fn debe_derivar_un_slug_con_fallback() {
        assert_eq!(slug_desde_nombre("Mi Tienda", "entidad"), "mi-tienda");
        assert_eq!(slug_desde_nombre("", "Mi Negocio"), "mi-negocio");
        assert_eq!(slug_desde_nombre("a", "entidad"), "a-1");
        assert_eq!(slug_desde_nombre("admin", "entidad"), "admin-1");
    }

    #[test]
    fn debe_recortar_sin_dejar_guion_final() {
        let base = "a".repeat(79);
        let slug = slug_con_sufijo(&base, 10);
        assert!(slug.ends_with("-10"));
        assert!(slug.chars().count() <= SLUG_MAX_LENGTH);
        assert!(!slug.contains("--"));
    }
}
