//! Slugs legibles para tiendas (ADR-26).
//!
//! Réplica de `@core/shared/slug.ts`: normalización determinista a `[a-z0-9-]`,
//! reservados, colisiones por sufijo y validación del slug solicitado.

use crate::errors::ApiError;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

pub const SLUG_MIN_LENGTH: usize = 2;
pub const SLUG_MAX_LENGTH: usize = 80;

/// Palabras que colisionarían con rutas, roles o recursos del API.
pub const SLUGS_RESERVADOS: [&str; 35] = [
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
];

fn es_slug_reservado(valor: &str) -> bool {
    SLUGS_RESERVADOS.contains(&valor)
}

/// Un UUID canónico (8-4-4-4-12 hex) se resuelve como clave interna.
pub fn es_uuid(valor: &str) -> bool {
    let grupos: Vec<&str> = valor.split('-').collect();
    if grupos.len() != 5 {
        return false;
    }
    let longitudes = [8, 4, 4, 4, 12];
    grupos.iter().zip(longitudes).all(|(grupo, longitud)| {
        grupo.len() == longitud && grupo.chars().all(|caracter| caracter.is_ascii_hexdigit())
    })
}

/// Normaliza un texto a slug: quita diacríticos, baja a minúsculas, convierte
/// cualquier secuencia no alfanumérica en un guion y recorta a 80 caracteres.
pub fn normalizar_slug(valor: &str) -> String {
    let sin_diacriticos: String = valor
        .nfd()
        .filter(|caracter| !is_combining_mark(*caracter))
        .collect();
    let mut salida = String::new();
    let mut guion_pendiente = false;
    for caracter in sin_diacriticos.chars() {
        let caracter = caracter.to_ascii_lowercase();
        if caracter.is_ascii_alphanumeric() {
            if guion_pendiente && !salida.is_empty() {
                salida.push('-');
            }
            guion_pendiente = false;
            salida.push(caracter);
        } else {
            guion_pendiente = true;
        }
    }
    let mut recortado = salida.chars().take(SLUG_MAX_LENGTH).collect::<String>();
    while recortado.ends_with('-') {
        recortado.pop();
    }
    recortado
}

fn patron_valido(valor: &str) -> bool {
    if valor.is_empty() || valor.starts_with('-') || valor.ends_with('-') {
        return false;
    }
    !valor.contains("--")
        && valor.chars().all(|caracter| {
            caracter.is_ascii_lowercase() || caracter.is_ascii_digit() || caracter == '-'
        })
}

/// Valida el formato ya normalizado y rechaza reservados y UUID.
pub fn es_slug_valido(valor: &str) -> bool {
    (SLUG_MIN_LENGTH..=SLUG_MAX_LENGTH).contains(&valor.len())
        && patron_valido(valor)
        && !es_slug_reservado(valor)
        && !es_uuid(valor)
}

/// Añade un sufijo numérico a un slug base sin superar la longitud máxima.
pub fn slug_con_sufijo(base: &str, numero: i64) -> String {
    let sufijo = format!("-{numero}");
    let maximo_base = SLUG_MIN_LENGTH.max(SLUG_MAX_LENGTH.saturating_sub(sufijo.len()));
    let mut recorte = base.chars().take(maximo_base).collect::<String>();
    while recorte.ends_with('-') {
        recorte.pop();
    }
    let recorte = if recorte.is_empty() {
        "entidad".to_string()
    } else {
        recorte
    };
    format!("{recorte}{sufijo}")
}

/// Deriva un slug base desde un nombre, con fallback y resolución de reservados.
pub fn slug_desde_nombre(nombre: &str, fallback: &str) -> String {
    let mut base = normalizar_slug(nombre);
    if base.is_empty() {
        base = normalizar_slug(fallback);
        if base.is_empty() {
            base = "entidad".to_string();
        }
    }
    if base.len() < SLUG_MIN_LENGTH {
        base = normalizar_slug(&format!("{base}-1"));
        if base.is_empty() {
            base = "entidad-1".to_string();
        }
    }
    if es_slug_reservado(&base) {
        base = slug_con_sufijo(&base, 1);
    }
    if es_uuid(&base) {
        base = format!("{base}-x");
    }
    let mut recortado = base.chars().take(SLUG_MAX_LENGTH).collect::<String>();
    while recortado.ends_with('-') {
        recortado.pop();
    }
    recortado
}

/// Normaliza y valida un slug enviado por el cliente. Un valor inválido es un
/// error de dominio (400), no una excepción de infraestructura.
pub fn validar_slug_solicitado(valor: &str) -> Result<String, ApiError> {
    let slug = normalizar_slug(valor);
    if slug.len() < SLUG_MIN_LENGTH {
        return Err(ApiError::dominio(
            "SLUG_INVALIDO",
            format!(
                "El slug debe tener entre {SLUG_MIN_LENGTH} y {SLUG_MAX_LENGTH} caracteres alfanuméricos o guiones."
            ),
        ));
    }
    if es_slug_reservado(&slug) {
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
    if !patron_valido(&slug) || slug.len() > SLUG_MAX_LENGTH {
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
        assert_eq!(
            normalizar_slug("Tienda de la Vendedora Piloto"),
            "tienda-de-la-vendedora-piloto"
        );
        assert_eq!(normalizar_slug("  Café & Ñandú  "), "cafe-nandu");
        assert_eq!(normalizar_slug("---"), "");
        assert_eq!(normalizar_slug(&"a".repeat(100)).len(), 80);
    }

    #[test]
    fn debe_detectar_uuids_y_reservados() {
        assert!(es_uuid("130c07c6-7e10-4f91-9c13-e17ca6be6eea"));
        assert!(!es_uuid("130c07c6-7e10-4f91-9c13"));
        assert!(!es_uuid("tienda-qa"));
        assert!(es_slug_valido("tienda-qa"));
        assert!(!es_slug_valido("tienda"));
        assert!(!es_slug_valido("Tienda-QA"));
        assert!(!es_slug_valido("a"));
        assert!(!es_slug_valido("tienda--qa"));
    }

    #[test]
    fn debe_derivar_slugs_con_fallback_sufijo_y_uuid() {
        assert_eq!(slug_desde_nombre("Mi Tienda", "tienda"), "mi-tienda");
        assert_eq!(slug_desde_nombre("!!!", "tienda"), "tienda-1");
        assert_eq!(slug_desde_nombre("a", "x"), "a-1");
        assert_eq!(
            slug_desde_nombre("130c07c6-7e10-4f91-9c13-e17ca6be6eea", "x"),
            "130c07c6-7e10-4f91-9c13-e17ca6be6eea-x"
        );
        assert_eq!(slug_con_sufijo(&"a".repeat(80), 12).len(), SLUG_MAX_LENGTH);
    }

    #[test]
    fn debe_validar_el_slug_solicitado_con_los_codigos_del_contrato() {
        assert_eq!(
            validar_slug_solicitado(" Mi Tienda ").expect("slug"),
            "mi-tienda"
        );
        assert_eq!(
            validar_slug_solicitado("a").expect_err("corto").codigo,
            "SLUG_INVALIDO"
        );
        assert_eq!(
            validar_slug_solicitado("tienda")
                .expect_err("reservado")
                .codigo,
            "SLUG_RESERVADO"
        );
        assert_eq!(
            validar_slug_solicitado("130c07c6-7e10-4f91-9c13-e17ca6be6eea")
                .expect_err("uuid")
                .codigo,
            "SLUG_INVALIDO"
        );
    }
}
