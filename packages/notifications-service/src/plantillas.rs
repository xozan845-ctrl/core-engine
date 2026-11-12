//! Plantillas de correo del ciclo del pedido (ADR-0003).
//!
//! Son funciones puras: reciben el evento y el contexto resuelto por las APIs
//! internas, y devuelven los mensajes a enviar. No acceden a BD ni al bus.

use crate::email::EmailMessage;
use crate::tipos::{ContextoPedido, Destinatario, MensajeNotificacion, UsuarioExterno};
use serde_json::Value;

const EVENTO_PAYMENT_PROCESADO: &str = "payment.procesado";
const EVENTO_SHIPMENT_STARTED: &str = "shipment.started";
const EVENTO_ORDER_STATUS_UPDATED: &str = "order.status.updated";
const EVENTO_ORDER_COMPLETADO: &str = "order.completado";
const EVENTO_DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";
const EVENTO_STOCK_FALLIDO: &str = "stock.fallido";

fn dinero(cents: i64, moneda: &str) -> String {
    let simbolo = if moneda == "USD" { "US$" } else { "C$" };
    format!("{simbolo} {:.2}", cents as f64 / 100.0)
}

fn escapar_html(valor: &str) -> String {
    valor
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn pedido_corto(id: &str) -> String {
    id.chars().take(8).collect()
}

fn cuerpo(parrafos: &[String]) -> (String, String) {
    let mut lineas: Vec<String> = parrafos.to_vec();
    lineas.push(String::new());
    lineas.push("Core Engine".to_string());
    let texto = lineas.join("\n");
    let mut html = parrafos
        .iter()
        .map(|parrafo| format!("<p>{}</p>", escapar_html(parrafo)))
        .collect::<Vec<_>>()
        .join("");
    html.push_str("<p>Core Engine</p>");
    (texto, html)
}

fn mensaje(
    destinatario: Destinatario,
    usuario: Option<&UsuarioExterno>,
    asunto: &str,
    contenido: &(String, String),
) -> Vec<MensajeNotificacion> {
    let Some(usuario) = usuario else {
        return Vec::new();
    };
    if usuario.correo.is_empty() {
        return Vec::new();
    }
    vec![MensajeNotificacion {
        destinatario,
        mensaje: EmailMessage {
            to: usuario.correo.clone(),
            subject: asunto.to_string(),
            text: contenido.0.clone(),
            html: Some(format!("<h1>{}</h1>{}", escapar_html(asunto), contenido.1)),
        },
    }]
}

fn total_orden(contexto: &ContextoPedido) -> String {
    dinero(contexto.orden.total_cents, &contexto.orden.moneda)
}

fn estado_legible(estado: &str) -> &str {
    match estado {
        "creada" => "creado",
        "pagada" => "pagado",
        "en_preparacion" => "en preparación",
        "enviada" => "enviado",
        "en_camino" => "en camino",
        "entregada" => "entregado",
        "cancelada" => "cancelado",
        "devuelta" => "devuelto",
        "devolucion_solicitada" => "con devolución solicitada",
        otro => otro,
    }
}

fn asunto_estado(estado: &str, corto: &str) -> String {
    if matches!(estado, "enviada" | "en_camino" | "en_transito") {
        return format!("Tu pedido está en camino · #{corto}");
    }
    if estado == "entregada" {
        return format!("Tu pedido fue entregado · #{corto}");
    }
    if estado == "cancelada" {
        return format!("Tu pedido fue cancelado · #{corto}");
    }
    if matches!(estado, "devuelta" | "devolucion_solicitada") {
        return format!("Devolución registrada · #{corto}");
    }
    format!("Actualización de tu pedido · #{corto}")
}

fn texto_data<'a>(data: Option<&'a Value>, clave: &str) -> Option<&'a str> {
    data.and_then(|valor| valor.get(clave))
        .and_then(Value::as_str)
}

/// Construye los mensajes del evento. Los eventos sin plantilla (p. ej.
/// `order.created`) no generan correo.
pub fn construir_mensajes(evento: &Value, contexto: &ContextoPedido) -> Vec<MensajeNotificacion> {
    let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
    let data = evento.get("data");
    let orden = &contexto.orden;
    let corto = pedido_corto(&orden.id);
    let comprador = contexto.comprador.as_ref();
    let vendedor = contexto.vendedor.as_ref();

    match tipo {
        EVENTO_PAYMENT_PROCESADO => {
            let total = total_orden(contexto);
            let compra = cuerpo(&[
                format!("Recibimos el pago de tu pedido por {total}."),
                "El vendedor ya fue notificado para prepararlo.".to_string(),
            ]);
            let venta = cuerpo(&[
                format!("Tienes un pedido pagado por {total}."),
                "Prepara el envío desde tu panel de vendedor.".to_string(),
            ]);
            let mut mensajes = mensaje(
                Destinatario::Comprador,
                comprador,
                &format!("Pago confirmado · #{corto}"),
                &compra,
            );
            mensajes.extend(mensaje(
                Destinatario::Vendedor,
                vendedor,
                &format!("Nuevo pedido pagado · #{corto}"),
                &venta,
            ));
            mensajes
        }

        EVENTO_SHIPMENT_STARTED => {
            let guia = texto_data(data, "guia");
            let detalle = match guia {
                Some(guia) if !guia.is_empty() => format!("Guía de despacho: {guia}."),
                _ => "Estamos preparando el envío.".to_string(),
            };
            let compra = cuerpo(&[
                detalle,
                format!("Total del pedido: {}.", total_orden(contexto)),
            ]);
            mensaje(
                Destinatario::Comprador,
                comprador,
                &format!("Tu pedido está en preparación · #{corto}"),
                &compra,
            )
        }

        EVENTO_ORDER_STATUS_UPDATED => {
            let estado = texto_data(data, "estado").unwrap_or(&orden.estado);
            let motivo = match texto_data(data, "motivo") {
                Some(motivo) if !motivo.is_empty() => format!(" Motivo: {motivo}."),
                _ => String::new(),
            };
            let asunto = asunto_estado(estado, &corto);
            let compra = cuerpo(&[
                format!(
                    "Tu pedido #{corto} pasó a estado \"{}\".{motivo}",
                    estado_legible(estado)
                ),
                format!("Total: {}.", total_orden(contexto)),
            ]);
            let mut mensajes = mensaje(Destinatario::Comprador, comprador, &asunto, &compra);
            if matches!(estado, "cancelada" | "devuelta") {
                let venta = cuerpo(&[format!(
                    "El pedido #{corto} pasó a estado \"{}\".{motivo}",
                    estado_legible(estado)
                )]);
                mensajes.extend(mensaje(
                    Destinatario::Vendedor,
                    vendedor,
                    &format!("Pedido {} · #{corto}", estado_legible(estado)),
                    &venta,
                ));
            }
            mensajes
        }

        EVENTO_ORDER_COMPLETADO => {
            let compra = cuerpo(&[
                "Tu pedido fue entregado. ¡Gracias por comprar!".to_string(),
                format!("Total: {}.", total_orden(contexto)),
            ]);
            mensaje(
                Destinatario::Comprador,
                comprador,
                &format!("Pedido entregado · #{corto}"),
                &compra,
            )
        }

        EVENTO_DEVOLUCION_SOLICITADA => {
            let detalle = match texto_data(data, "motivo") {
                Some(motivo) if !motivo.is_empty() => format!(" Motivo: {motivo}."),
                _ => String::new(),
            };
            let compra = cuerpo(&[format!("Registramos la devolución de tu pedido.{detalle}")]);
            mensaje(
                Destinatario::Comprador,
                comprador,
                &format!("Devolución solicitada · #{corto}"),
                &compra,
            )
        }

        EVENTO_STOCK_FALLIDO => {
            let compra = cuerpo(&[
                "No pudimos reservar todos los productos de tu pedido.".to_string(),
                "El pedido será cancelado y el pago revertido si corresponde.".to_string(),
            ]);
            mensaje(
                Destinatario::Comprador,
                comprador,
                &format!("No pudimos reservar el stock · #{corto}"),
                &compra,
            )
        }

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::{OrdenExterna, UsuarioExterno};
    use serde_json::json;

    fn orden() -> OrdenExterna {
        OrdenExterna {
            id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            comprador_id: "11111111-1111-1111-1111-111111111111".to_string(),
            vendedor_id: "22222222-2222-2222-2222-222222222222".to_string(),
            estado: "pagada".to_string(),
            total_cents: 115000,
            moneda: "NIO".to_string(),
            items: None,
        }
    }

    fn comprador() -> UsuarioExterno {
        UsuarioExterno {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            nombre: "Ana".to_string(),
            correo: "ana@core.test".to_string(),
        }
    }

    fn vendedor() -> UsuarioExterno {
        UsuarioExterno {
            id: "22222222-2222-2222-2222-222222222222".to_string(),
            nombre: "Luis".to_string(),
            correo: "luis@core.test".to_string(),
        }
    }

    fn contexto() -> ContextoPedido {
        ContextoPedido {
            orden: orden(),
            comprador: Some(comprador()),
            vendedor: Some(vendedor()),
        }
    }

    fn evento(tipo: &str, data: Value) -> Value {
        json!({
            "event_id": "33333333-3333-3333-3333-333333333333",
            "tipo": tipo,
            "ocurrido_en": "2026-11-11T00:00:00.000Z",
            "data": data,
        })
    }

    #[test]
    fn el_pago_confirmado_avisa_al_comprador_y_al_vendedor() {
        let mensajes = construir_mensajes(
            &evento(EVENTO_PAYMENT_PROCESADO, json!({ "order_id": orden().id })),
            &contexto(),
        );
        let pares: Vec<(&str, &str)> = mensajes
            .iter()
            .map(|m| (m.destinatario.como_str(), m.mensaje.to.as_str()))
            .collect();
        assert_eq!(
            pares,
            vec![
                ("comprador", "ana@core.test"),
                ("vendedor", "luis@core.test")
            ]
        );
        assert_eq!(mensajes[0].mensaje.subject, "Pago confirmado · #aaaaaaaa");
        assert!(mensajes[0].mensaje.text.contains("C$ 1150.00"));
    }

    #[test]
    fn el_envio_iniciado_incluye_la_guia_y_solo_va_al_comprador() {
        let mensajes = construir_mensajes(
            &evento(
                EVENTO_SHIPMENT_STARTED,
                json!({ "order_id": orden().id, "guia": "GUIA-123" }),
            ),
            &contexto(),
        );
        assert_eq!(mensajes.len(), 1);
        assert!(mensajes[0].mensaje.text.contains("GUIA-123"));
        assert_eq!(
            mensajes[0].mensaje.subject,
            "Tu pedido está en preparación · #aaaaaaaa"
        );
    }

    #[test]
    fn mapea_el_estado_a_su_asunto() {
        let casos = [
            ("enviada", "Tu pedido está en camino · #aaaaaaaa", 1),
            ("entregada", "Tu pedido fue entregado · #aaaaaaaa", 1),
            ("cancelada", "Tu pedido fue cancelado · #aaaaaaaa", 2),
            ("devuelta", "Devolución registrada · #aaaaaaaa", 2),
            ("pagada", "Actualización de tu pedido · #aaaaaaaa", 1),
        ];
        for (estado, asunto, cantidad) in casos {
            let mensajes = construir_mensajes(
                &evento(
                    EVENTO_ORDER_STATUS_UPDATED,
                    json!({ "order_id": orden().id, "estado": estado }),
                ),
                &contexto(),
            );
            assert_eq!(mensajes.len(), cantidad, "estado {estado}");
            assert_eq!(mensajes[0].mensaje.subject, asunto);
        }
    }

    #[test]
    fn escapa_el_motivo_en_el_html_y_lo_incluye_en_el_texto() {
        let mensajes = construir_mensajes(
            &evento(
                EVENTO_ORDER_STATUS_UPDATED,
                json!({
                    "order_id": orden().id,
                    "estado": "cancelada",
                    "motivo": "<script>alert(1)</script>",
                }),
            ),
            &contexto(),
        );
        assert!(mensajes[0]
            .mensaje
            .text
            .contains("<script>alert(1)</script>"));
        let html = mensajes[0].mensaje.html.as_deref().expect("html");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn notifica_la_entrega_la_devolucion_y_el_stock_fallido() {
        let entrega = construir_mensajes(
            &evento(EVENTO_ORDER_COMPLETADO, json!({ "order_id": orden().id })),
            &contexto(),
        );
        let devolucion = construir_mensajes(
            &evento(
                EVENTO_DEVOLUCION_SOLICITADA,
                json!({ "order_id": orden().id, "motivo": "dañado" }),
            ),
            &contexto(),
        );
        let stock = construir_mensajes(
            &evento(EVENTO_STOCK_FALLIDO, json!({ "order_id": orden().id })),
            &contexto(),
        );
        assert_eq!(entrega[0].mensaje.subject, "Pedido entregado · #aaaaaaaa");
        assert!(devolucion[0].mensaje.text.contains("dañado"));
        assert_eq!(
            stock[0].mensaje.subject,
            "No pudimos reservar el stock · #aaaaaaaa"
        );
    }

    #[test]
    fn no_genera_correo_sin_destinatario_ni_para_eventos_sin_plantilla() {
        let sin_destinatarios = ContextoPedido {
            orden: orden(),
            comprador: None,
            vendedor: None,
        };
        assert!(construir_mensajes(
            &evento(EVENTO_PAYMENT_PROCESADO, json!({ "order_id": orden().id })),
            &sin_destinatarios,
        )
        .is_empty());
        assert!(construir_mensajes(
            &evento("order.created", json!({ "order_id": orden().id })),
            &contexto(),
        )
        .is_empty());
    }

    #[test]
    fn usa_usd_cuando_la_orden_esta_en_dolares() {
        let mut contexto = contexto();
        contexto.orden.moneda = "USD".to_string();
        let mensajes = construir_mensajes(
            &evento(EVENTO_PAYMENT_PROCESADO, json!({ "order_id": orden().id })),
            &contexto,
        );
        assert!(mensajes[0].mensaje.text.contains("US$ 1150.00"));
    }
}
