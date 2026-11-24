//! PDF de comprobantes fiscales.
//!
//! Genera un PDF 1.4 con texto Helvetica, sin dependencias externas. Incluye
//! emisor, cliente, líneas (base/IVA/total) y totales. Soporta varias páginas
//! para pedidos largos.

use crate::facturacion::Comprobante;
use serde_json::Value;

const LINEAS_POR_PAGINA: usize = 42;

pub fn comprobante_pdf(comprobante: &Comprobante) -> Vec<u8> {
    let lineas = texto_comprobante(comprobante);
    let paginas: Vec<&[String]> = lineas.chunks(LINEAS_POR_PAGINA).collect();
    let paginas: Vec<&[String]> = if paginas.is_empty() {
        vec![&[]]
    } else {
        paginas
    };

    let mut objetos: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut hijos = Vec::with_capacity(paginas.len());
    for (indice, pagina) in paginas.iter().enumerate() {
        let page_id = 4 + indice * 2;
        let content_id = page_id + 1;
        hijos.push(format!("{page_id} 0 R"));
        objetos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>"
        ));
        let contenido = contenido_pagina(pagina);
        objetos.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            contenido.len(),
            contenido
        ));
    }
    objetos[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        hijos.join(" "),
        paginas.len()
    );

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

fn texto_comprobante(comprobante: &Comprobante) -> Vec<String> {
    let datos = comprobante.datos_cliente.as_ref();
    let razon_social = texto(datos, "razon_social").unwrap_or_else(|| "-".to_string());
    let ruc_cliente = texto(datos, "ruc").unwrap_or_else(|| "-".to_string());
    let ruc_emisor = texto(datos, "emisor_ruc").unwrap_or_else(|| "-".to_string());
    let devolucion = texto(datos, "devolucion_id");
    let orden = texto(datos, "orden_id").unwrap_or_default();
    let items = datos
        .and_then(|valor| valor.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut lineas = vec![
        "Core Engine".to_string(),
        format!("RUC emisor: {ruc_emisor}"),
        format!("{} {}", comprobante.tipo, comprobante.numero),
        format!("Serie: {}", comprobante.serie),
        format!("Estado: {}", comprobante.estado),
        format!("Moneda: {}", comprobante.moneda),
        format!(
            "Fecha: {}",
            comprobante.emitido_en.as_deref().unwrap_or("-")
        ),
        String::new(),
        format!("Cliente: {razon_social}"),
        format!("RUC cliente: {ruc_cliente}"),
    ];
    if !orden.is_empty() {
        lineas.push(format!("Orden: {orden}"));
    }
    if comprobante.orden_id.is_some() && orden.is_empty() {
        lineas.push(format!(
            "Orden: {}",
            comprobante.orden_id.clone().unwrap_or_default()
        ));
    }
    if let Some(devolucion) = devolucion {
        lineas.push(format!("Devolucion: {devolucion}"));
    }
    lineas.push(String::new());
    lineas.push("SKU              CANT      BASE   TASA       IVA     TOTAL".to_string());
    lineas.push("------------------------------------------------------------".to_string());
    for item in items {
        let sku = item.get("sku").and_then(Value::as_str).unwrap_or("-");
        let sku: String = sku.chars().take(16).collect();
        let cantidad = item.get("cantidad").and_then(Value::as_i64).unwrap_or(0);
        let base = centavos(item.get("base_cents"));
        let iva = centavos(item.get("iva_cents"));
        let total = centavos(item.get("total_cents"));
        let tasa = item
            .get("tasa_impuesto_bps")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let tasa = if item.get("exento").and_then(Value::as_bool).unwrap_or(false) {
            "EXENTO".to_string()
        } else {
            format!("{:.2}%", tasa as f64 / 100.0)
        };
        lineas.push(format!(
            "{sku:<16} {cantidad:>4} {base:>10} {tasa:>6} {iva:>9} {total:>10}"
        ));
    }
    lineas.push("------------------------------------------------------------".to_string());
    lineas.push(format!(
        "Base gravada: {:.2}",
        comprobante.base_gravada_cents as f64 / 100.0
    ));
    lineas.push(format!("IVA: {:.2}", comprobante.iva_cents as f64 / 100.0));
    lineas.push(format!(
        "Exento: {:.2}",
        comprobante.exento_cents as f64 / 100.0
    ));
    if comprobante.retencion_cents != 0 {
        let codigo = datos
            .and_then(|valor| valor.get("retencion"))
            .and_then(|valor| valor.get("codigo"))
            .and_then(Value::as_str)
            .unwrap_or("IR");
        lineas.push(format!(
            "Retencion {} ({:.2}%): {:.2}",
            codigo,
            comprobante.retencion_tasa_bps as f64 / 100.0,
            comprobante.retencion_cents as f64 / 100.0
        ));
    }
    lineas.push(format!(
        "TOTAL: {:.2} {}",
        comprobante.total_cents as f64 / 100.0,
        comprobante.moneda
    ));
    lineas.push(String::new());
    lineas.push(format!(
        "Electronica: {}",
        comprobante
            .electronica_estado
            .as_deref()
            .unwrap_or("no aplica")
    ));
    lineas.push(format!(
        "Referencia: {}",
        comprobante.electronica_referencia.as_deref().unwrap_or("-")
    ));
    lineas.push("Documento generado por Core Engine".to_string());
    lineas
}

fn centavos(valor: Option<&Value>) -> String {
    let cents = valor.and_then(Value::as_i64).unwrap_or(0);
    format!("{:.2}", cents as f64 / 100.0)
}

fn texto(datos: Option<&Value>, clave: &str) -> Option<String> {
    datos
        .and_then(|valor| valor.get(clave))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty() && *valor != "null")
        .map(str::to_string)
}

fn contenido_pagina(lineas: &[String]) -> String {
    let mut contenido = String::from("BT /F1 10 Tf 50 760 Td 13 TL\n");
    for linea in lineas {
        contenido.push_str(&format!("({}) Tj T*\n", escapar(linea)));
    }
    contenido.push_str("ET");
    contenido
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
    use serde_json::json;

    fn comprobante() -> Comprobante {
        Comprobante {
            id: "c-1".into(),
            serie: "F001".into(),
            numero: "F001-00000001".into(),
            tipo: "FACTURA".into(),
            orden_id: Some("o-1".into()),
            empresa_id: None,
            parte_id: None,
            negocio_id: None,
            base_gravada_cents: 100_000,
            iva_cents: 15_000,
            exento_cents: 0,
            retencion_cents: 2_000,
            retencion_tasa_bps: 200,
            total_cents: 115_000,
            moneda: "NIO".into(),
            estado: "EMITIDO".into(),
            datos_cliente: Some(json!({
                "razon_social": "Cliente S.A.",
                "ruc": "RUC-1",
                "emisor_ruc": "RUC-EMISOR",
                "items": [{"sku": "SKU-1", "cantidad": 2, "base_cents": 100_000, "iva_cents": 15_000, "total_cents": 115_000, "tasa_impuesto_bps": 1500, "exento": false}],
                "retencion": {"codigo": "IR", "tasa_bps": 200, "monto_cents": 2_000}
            })),
            emitido_en: Some("2026-01-01T00:00:00.000Z".into()),
            creado_en: "2026-01-01T00:00:00.000Z".into(),
            electronica_estado: Some("aceptada".into()),
            electronica_referencia: Some("sim-1".into()),
            electronica_enviada_en: None,
        }
    }

    #[test]
    fn debe_generar_un_pdf_con_cabecera_y_xref() {
        let pdf = comprobante_pdf(&comprobante());
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(5).any(|ventana| ventana == b"xref\n"));
        assert!(pdf.windows(5).any(|ventana| ventana == b"%%EOF"));
    }

    #[test]
    fn debe_incluir_emisor_cliente_y_lineas() {
        let pdf = comprobante_pdf(&comprobante());
        let texto = String::from_utf8_lossy(&pdf);
        assert!(texto.contains("RUC-EMISOR"));
        assert!(texto.contains("Cliente S.A."));
        assert!(texto.contains("SKU-1"));
        assert!(texto.contains("15.00%"));
        assert!(texto.contains("Retencion IR"));
        assert!(texto.contains("TOTAL: 1150.00"));
    }
}
