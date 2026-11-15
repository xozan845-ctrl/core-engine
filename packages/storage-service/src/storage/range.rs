//! Interpretación de `Range` (RFC 7233), mismo contrato que `range.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangoBytes {
    pub inicio: i64,
    pub fin: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoRango {
    Completo,
    Parcial(RangoBytes),
    Invalido,
}

pub fn interpretar_rango(cabecera: Option<&str>, total_bytes: i64) -> ResultadoRango {
    let total = if total_bytes > 0 { total_bytes } else { 0 };
    let Some(cabecera) = cabecera else {
        return ResultadoRango::Completo;
    };
    let cabecera = cabecera.trim();
    if cabecera.is_empty() {
        return ResultadoRango::Completo;
    }
    let Some(resto) = cabecera.strip_prefix("bytes=") else {
        return ResultadoRango::Invalido;
    };
    let Some((inicio_texto, fin_texto)) = resto.split_once('-') else {
        return ResultadoRango::Invalido;
    };
    if inicio_texto.is_empty() && fin_texto.is_empty() {
        return ResultadoRango::Invalido;
    }
    if total == 0 {
        return ResultadoRango::Invalido;
    }
    if inicio_texto.is_empty() {
        let Ok(sufijo) = fin_texto.parse::<i64>() else {
            return ResultadoRango::Invalido;
        };
        if sufijo <= 0 {
            return ResultadoRango::Invalido;
        }
        return ResultadoRango::Parcial(RangoBytes {
            inicio: (total - sufijo).max(0),
            fin: total - 1,
        });
    }
    let Ok(inicio) = inicio_texto.parse::<i64>() else {
        return ResultadoRango::Invalido;
    };
    if inicio < 0 || inicio >= total {
        return ResultadoRango::Invalido;
    }
    if fin_texto.is_empty() {
        return ResultadoRango::Parcial(RangoBytes {
            inicio,
            fin: total - 1,
        });
    }
    let Ok(fin) = fin_texto.parse::<i64>() else {
        return ResultadoRango::Invalido;
    };
    if fin < inicio {
        return ResultadoRango::Invalido;
    }
    ResultadoRango::Parcial(RangoBytes {
        inicio,
        fin: fin.min(total - 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_devolver_completo_sin_cabecera() {
        assert_eq!(interpretar_rango(None, 10), ResultadoRango::Completo);
        assert_eq!(interpretar_rango(Some(" "), 10), ResultadoRango::Completo);
    }

    #[test]
    fn debe_interpretar_rangos_abiertos_y_sufijos() {
        assert_eq!(
            interpretar_rango(Some("bytes=2-5"), 10),
            ResultadoRango::Parcial(RangoBytes { inicio: 2, fin: 5 })
        );
        assert_eq!(
            interpretar_rango(Some("bytes=7-"), 10),
            ResultadoRango::Parcial(RangoBytes { inicio: 7, fin: 9 })
        );
        assert_eq!(
            interpretar_rango(Some("bytes=-3"), 10),
            ResultadoRango::Parcial(RangoBytes { inicio: 7, fin: 9 })
        );
        assert_eq!(
            interpretar_rango(Some("bytes=7-99"), 10),
            ResultadoRango::Parcial(RangoBytes { inicio: 7, fin: 9 })
        );
    }

    #[test]
    fn debe_rechazar_rangos_invalidos() {
        for cabecera in [
            "bytes=10-",
            "bytes=5-2",
            "bytes=-0",
            "items=1-2",
            "bytes=a-b",
        ] {
            assert_eq!(
                interpretar_rango(Some(cabecera), 10),
                ResultadoRango::Invalido,
                "{cabecera}"
            );
        }
        assert_eq!(
            interpretar_rango(Some("bytes=0-0"), 0),
            ResultadoRango::Invalido
        );
    }
}
