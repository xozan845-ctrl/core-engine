//! Paginación consistente de la API (equivalente a `pagination.ts`).
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Pagina<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub pagina: i64,
    pub limite: i64,
    pub paginas: i64,
}

pub fn parsear_paginacion(pagina: Option<i64>, limite: Option<i64>) -> (i64, i64) {
    let pagina = pagina.unwrap_or(1).max(1);
    let limite = limite.unwrap_or(20).clamp(1, 100);
    (pagina, limite)
}

pub fn crear_pagina<T>(items: Vec<T>, total: i64, pagina: i64, limite: i64) -> Pagina<T> {
    let paginas = if total <= 0 {
        1
    } else {
        (total + limite - 1) / limite
    };
    Pagina {
        items,
        total,
        pagina,
        limite,
        paginas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_acotar_paginacion() {
        assert_eq!(parsear_paginacion(None, None), (1, 20));
        assert_eq!(parsear_paginacion(Some(0), Some(250)), (1, 100));
        assert_eq!(parsear_paginacion(Some(3), Some(0)), (3, 1));
    }

    #[test]
    fn debe_calcular_paginas_minimo_una() {
        assert_eq!(crear_pagina(vec!["a"], 5, 1, 2).paginas, 3);
        assert_eq!(crear_pagina(Vec::<String>::new(), 0, 2, 20).paginas, 1);
    }
}
