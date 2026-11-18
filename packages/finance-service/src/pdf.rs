//! PDF mínimo para comprobantes fiscales.
//!
//! No depende de una librería externa: genera un PDF 1.4 de una página con
//! texto Helvetica. El objetivo es un documento descargable y auditable; un
//! diseño fiscal avanzado puede sustituirse por un renderizador dedicado.

use crate::facturacion::Comprobante;

pub fn comprobante_pdf(comprobante: &Comprobante) -> Vec<u8> {
    let lineas = vec![
        "Core Engine".to_string(),
        format!("{} {}", comprobante.tipo, comprobante.numero),
        format!("Serie: {}", comprobante.serie),
        format!("Estado: {}", comprobante.estado),
        format!("Moneda: {}", comprobante.moneda),
        format!(
            "Base gravada: {:.2}",
            comprobante.base_gravada_cents as f64 / 100.0
        ),
        format!("IVA: {:.2}", comprobante.iva_cents as f64 / 100.0),
        format!("Exento: {:.2}", comprobante.exento_cents as f64 / 100.0),
        format!("Total: {:.2}", comprobante.total_cents as f64 / 100.0),
        format!(
            "Electronica: {}",
            comprobante
                .electronica_estado
                .as_deref()
                .unwrap_or("no aplica")
        ),
        format!(
            "Referencia: {}",
            comprobante.electronica_referencia.as_deref().unwrap_or("-")
        ),
        "Documento generado por Core Engine".to_string(),
    ];
    let mut contenido = String::from("BT /F1 11 Tf 50 760 Td 16 TL\n");
    for linea in lineas {
        contenido.push_str(&format!("({}) Tj T*\n", escapar(&linea)));
    }
    contenido.push_str("ET");

    let objetos = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            contenido.len(),
            contenido
        ),
    ];

    let mut salida = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objetos.len());
    for (indice, objeto) in objetos.iter().enumerate() {
        offsets.push(salida.len());
        salida.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", indice + 1, objeto).as_bytes());
    }
    let xref = salida.len();
    salida.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1).as_bytes(),
    );
    for offset in offsets {
        salida.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    salida.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objetos.len() + 1
        )
        .as_bytes(),
    );
    salida
}

fn escapar(texto: &str) -> String {
    texto
        .chars()
        .map(|caracter| match caracter {
            '(' => "\\(".to_string(),
            ')' => "\\)".to_string(),
            '\\' => "\\\\".to_string(),
            caracter if caracter.is_ascii() => caracter.to_string(),
            _ => "?".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_generar_un_pdf_con_cabecera_y_xref() {
        let comprobante = Comprobante {
            id: "c-1".into(),
            serie: "F001".into(),
            numero: "F001-00000001".into(),
            tipo: "FACTURA".into(),
            orden_id: None,
            empresa_id: None,
            parte_id: None,
            negocio_id: None,
            base_gravada_cents: 100_000,
            iva_cents: 15_000,
            exento_cents: 0,
            total_cents: 115_000,
            moneda: "NIO".into(),
            estado: "EMITIDO".into(),
            datos_cliente: None,
            emitido_en: None,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
            electronica_estado: Some("aceptada".into()),
            electronica_referencia: Some("sim-1".into()),
            electronica_enviada_en: None,
        };
        let pdf = comprobante_pdf(&comprobante);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(5).any(|ventana| ventana == b"xref\n"));
        assert!(pdf.windows(5).any(|ventana| ventana == b"%%EOF"));
    }
}
