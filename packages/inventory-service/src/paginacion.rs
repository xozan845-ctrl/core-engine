//! Paginación consistente de la API (misma semántica que `@core/shared`).

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paginacion {
    pub pagina: i64,
    pub limite: i64,
}

fn parsear_entero(valor: &str) -> Option<i64> {
    let texto = valor.trim();
    let (signo, resto) = match texto.strip_prefix('-') {
        Some(resto) => (-1i64, resto),
        None => (1i64, texto.strip_prefix('+').unwrap_or(texto)),
    };
    let digitos: String = resto
        .chars()
        .take_while(|caracter| caracter.is_ascii_digit())
        .collect();
    if digitos.is_empty() {
        return None;
    }
    digitos.parse::<i64>().ok().map(|numero| numero * signo)
}

/// `parseInt(x) || default`, con el tope de 100 del contrato.
pub fn parsear_paginacion(pagina: Option<&str>, limite: Option<&str>) -> Paginacion {
    let pagina = pagina
        .and_then(parsear_entero)
        .filter(|valor| *valor != 0)
        .unwrap_or(1)
        .max(1);
    let limite = limite
        .and_then(parsear_entero)
        .filter(|valor| *valor != 0)
        .unwrap_or(20)
        .clamp(1, 100);
    Paginacion { pagina, limite }
}

pub fn crear_pagina(items: Vec<Value>, total: i64, paginacion: Paginacion) -> Value {
    let paginas = ((total as f64) / (paginacion.limite as f64)).ceil() as i64;
    json!({
        "items": items,
        "total": total,
        "pagina": paginacion.pagina,
        "limite": paginacion.limite,
        "paginas": paginas.max(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_parsear_igual_que_el_contrato() {
        assert_eq!(
            parsear_paginacion(Some("2"), Some("50")),
            Paginacion {
                pagina: 2,
                limite: 50
            }
        );
        assert_eq!(
            parsear_paginacion(None, None),
            Paginacion {
                pagina: 1,
                limite: 20
            }
        );
        assert_eq!(
            parsear_paginacion(Some("abc"), Some("0")),
            Paginacion {
                pagina: 1,
                limite: 20
            }
        );
        assert_eq!(
            parsear_paginacion(Some("-3"), Some("500")),
            Paginacion {
                pagina: 1,
                limite: 100
            }
        );
    }

    #[test]
    fn debe_crear_la_pagina() {
        let pagina = crear_pagina(
            vec![json!({"id": 1})],
            41,
            Paginacion {
                pagina: 3,
                limite: 20,
            },
        );
        assert_eq!(pagina["total"], 41);
        assert_eq!(pagina["paginas"], 3);
        let vacia = crear_pagina(
            Vec::new(),
            0,
            Paginacion {
                pagina: 1,
                limite: 20,
            },
        );
        assert_eq!(vacia["paginas"], 1);
    }
}
