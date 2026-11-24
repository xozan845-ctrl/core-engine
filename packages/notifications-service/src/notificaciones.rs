//! Orquesta una notificación: valida el pedido, resuelve comprador/vendedor por
//! las APIs internas, construye las plantillas y envía por el `EmailProvider`.
//!
//! No marca idempotencia: eso lo hace el consumidor tras completar el envío.

use crate::campos;
use crate::clientes::{es_uuid, Clientes};
use crate::email::EmailProvider;
use crate::errors::ErrorNotificacion;
use crate::logs;
use crate::mensajeria::{Sms, WhatsApp};
use crate::plantillas::{construir_mensajes, render, variables};
use crate::plantillas_db::Plantillas;
use crate::tipos::{ContextoPedido, Destinatario, OrdenExterna};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

#[async_trait]
pub trait NotificacionesPort: Send + Sync {
    async fn procesar(&self, evento: &Value) -> Result<(), ErrorNotificacion>;
}

pub struct Notificaciones {
    clientes: Arc<dyn Clientes>,
    email: Arc<dyn EmailProvider>,
    plantillas: Option<Arc<Plantillas>>,
    whatsapp: Option<Arc<WhatsApp>>,
    sms: Option<Arc<Sms>>,
    canales: Vec<String>,
}

impl Notificaciones {
    pub fn nueva(clientes: Arc<dyn Clientes>, email: Arc<dyn EmailProvider>) -> Self {
        Self {
            clientes,
            email,
            plantillas: None,
            whatsapp: None,
            sms: None,
            canales: vec!["email".to_string()],
        }
    }

    pub fn con_plantillas(mut self, plantillas: Arc<Plantillas>) -> Self {
        self.plantillas = Some(plantillas);
        self
    }

    pub fn con_whatsapp(mut self, whatsapp: Arc<WhatsApp>) -> Self {
        self.whatsapp = Some(whatsapp);
        self
    }

    pub fn con_sms(mut self, sms: Arc<Sms>) -> Self {
        self.sms = Some(sms);
        self
    }

    pub fn con_canales(mut self, canales: Vec<String>) -> Self {
        if !canales.is_empty() {
            self.canales = canales;
        }
        self
    }

    async fn texto_canal(
        &self,
        tipo: &str,
        empresa_id: Option<&str>,
        canal: &str,
        vars: &std::collections::BTreeMap<String, String>,
        fallback: &str,
    ) -> Result<String, ErrorNotificacion> {
        if let Some(plantillas) = self.plantillas.as_ref() {
            if let Some(plantilla) = plantillas.obtener(empresa_id, tipo, canal).await? {
                return Ok(render(&plantilla.cuerpo, vars));
            }
        }
        Ok(fallback.to_string())
    }
}

#[async_trait]
impl NotificacionesPort for Notificaciones {
    async fn procesar(&self, evento: &Value) -> Result<(), ErrorNotificacion> {
        let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
        let order_id = evento
            .get("data")
            .and_then(|data| data.get("order_id"))
            .and_then(Value::as_str);
        let Some(order_id) = order_id.filter(|valor| es_uuid(valor)) else {
            logs::warn(
                "notifications.pedidos",
                campos! { "msg": "evento_sin_orden", "tipo": tipo },
            );
            return Ok(());
        };

        let Some(orden) = self.clientes.obtener_orden(order_id).await? else {
            // Sin proyección no hay a quién notificar; no se bloquea la cola.
            logs::warn(
                "notifications.pedidos",
                campos! { "msg": "orden_no_disponible", "order_id": order_id, "tipo": tipo },
            );
            return Ok(());
        };

        let vendedor_id = orden.vendedor_id.clone();
        let (comprador, vendedor) =
            tokio::join!(self.clientes.obtener_usuario(&orden.comprador_id), async {
                match vendedor_id.as_deref() {
                    Some(id) => self.clientes.obtener_usuario(id).await,
                    None => Ok(None),
                }
            },);
        let contexto = ContextoPedido {
            orden,
            comprador: comprador?,
            vendedor: vendedor?,
        };
        let mensajes = construir_mensajes(evento, &contexto);
        for mensaje in &mensajes {
            let empresa_id = empresa_para_mensaje(evento, &contexto, &mensaje.destinatario);
            let vars = variables(evento, &contexto, mensaje.destinatario);
            let mut correo = mensaje.mensaje.clone();
            if let Some(plantillas) = self.plantillas.as_ref() {
                if let Some(plantilla) = plantillas
                    .obtener(empresa_id.as_deref(), tipo, "email")
                    .await?
                {
                    if !plantilla.asunto.trim().is_empty() {
                        correo.subject = render(&plantilla.asunto, &vars);
                    }
                    correo.text = render(&plantilla.cuerpo, &vars);
                    correo.html = None;
                }
            }
            if self.canales.iter().any(|canal| canal == "email") {
                self.email
                    .enviar(&correo)
                    .await
                    .map_err(ErrorNotificacion::Correo)?;
            }
            let Some(telefono) = mensaje
                .telefono
                .as_deref()
                .filter(|valor| !valor.is_empty())
            else {
                continue;
            };
            if self.canales.iter().any(|canal| canal == "whatsapp") {
                if let Some(whatsapp) = self.whatsapp.as_ref() {
                    let texto = self
                        .texto_canal(tipo, empresa_id.as_deref(), "whatsapp", &vars, &correo.text)
                        .await?;
                    if let Err(error) = whatsapp.enviar(telefono, &texto).await {
                        logs::error(
                            "notifications.whatsapp",
                            campos! { "msg": "envio fallido", "order_id": order_id, "err": error },
                        );
                    }
                }
            }
            if self.canales.iter().any(|canal| canal == "sms") {
                if let Some(sms) = self.sms.as_ref() {
                    let texto = self
                        .texto_canal(tipo, empresa_id.as_deref(), "sms", &vars, &correo.text)
                        .await?;
                    if let Err(error) = sms.enviar(telefono, &texto).await {
                        logs::error(
                            "notifications.sms",
                            campos! { "msg": "envio fallido", "order_id": order_id, "err": error },
                        );
                    }
                }
            }
        }
        let destinatarios = mensajes
            .iter()
            .map(|mensaje| mensaje.destinatario.como_str())
            .collect::<Vec<_>>();
        logs::info(
            "notifications.pedidos",
            campos! {
                "msg": "notificacion_enviada",
                "tipo": tipo,
                "order_id": order_id,
                "destinatarios": destinatarios,
            },
        );
        Ok(())
    }
}

/// Consulta secuencial reutilizable por tests: equivalente al `Promise.all`
/// del TypeScript pero sin exponer la concurrencia interna.
pub async fn resolver_contexto(
    clientes: &dyn Clientes,
    orden: OrdenExterna,
) -> Result<ContextoPedido, ErrorNotificacion> {
    let comprador = clientes.obtener_usuario(&orden.comprador_id).await?;
    let vendedor = match orden.vendedor_id.as_deref() {
        Some(id) => clientes.obtener_usuario(id).await?,
        None => None,
    };
    Ok(ContextoPedido {
        orden,
        comprador,
        vendedor,
    })
}

/// Resuelve el tenant dueño de cada mensaje sin asociar una orden multi-tenant
/// completa a una sola plantilla empresarial. El comprador usa plantilla de
/// empresa solo cuando todas las líneas pertenecen al mismo tenant; el mensaje
/// del vendedor usa la empresa de sus propias líneas.
fn empresa_para_mensaje(
    evento: &Value,
    contexto: &ContextoPedido,
    destinatario: &Destinatario,
) -> Option<String> {
    if let Some(empresa) = evento
        .get("data")
        .and_then(|data| data.get("empresa_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
    {
        return Some(empresa.to_string());
    }
    let empresas: Vec<String> = contexto
        .orden
        .items
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item.empresa_id.clone())
        .fold(Vec::new(), |mut salida, empresa| {
            if !salida.contains(&empresa) {
                salida.push(empresa);
            }
            salida
        });
    match destinatario {
        Destinatario::Comprador if empresas.len() == 1 => empresas.into_iter().next(),
        Destinatario::Comprador => None,
        Destinatario::Vendedor => {
            let vendedor = contexto.orden.vendedor_id.as_deref()?;
            contexto
                .orden
                .items
                .as_deref()?
                .iter()
                .find(|item| item.vendedor_id.as_deref() == Some(vendedor))
                .and_then(|item| item.empresa_id.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::UsuarioExterno;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Mutex;

    const ORDEN_ID: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    const COMPRADOR_ID: &str = "11111111-1111-1111-1111-111111111111";
    const VENDEDOR_ID: &str = "22222222-2222-2222-2222-222222222222";

    fn orden() -> OrdenExterna {
        OrdenExterna {
            id: ORDEN_ID.to_string(),
            comprador_id: COMPRADOR_ID.to_string(),
            vendedor_id: Some(VENDEDOR_ID.to_string()),
            estado: "pagada".to_string(),
            total_cents: 115000,
            moneda: "NIO".to_string(),
            items: None,
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

    struct ClientesFalsos {
        orden: Option<OrdenExterna>,
        usuarios: bool,
        orden_consultada: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Clientes for ClientesFalsos {
        async fn obtener_orden(
            &self,
            order_id: &str,
        ) -> Result<Option<OrdenExterna>, ErrorNotificacion> {
            self.orden_consultada
                .lock()
                .expect("lock")
                .push(order_id.to_string());
            Ok(self.orden.clone())
        }

        async fn obtener_usuario(
            &self,
            usuario_id: &str,
        ) -> Result<Option<UsuarioExterno>, ErrorNotificacion> {
            if !self.usuarios {
                return Ok(None);
            }
            Ok(Some(UsuarioExterno {
                id: usuario_id.to_string(),
                nombre: if usuario_id == COMPRADOR_ID {
                    "Ana"
                } else {
                    "Luis"
                }
                .to_string(),
                correo: if usuario_id == COMPRADOR_ID {
                    "ana@core.test"
                } else {
                    "luis@core.test"
                }
                .to_string(),
                telefono: None,
            }))
        }
    }

    #[derive(Default)]
    struct EmailFalso {
        enviados: Mutex<Vec<String>>,
        falla: bool,
    }

    #[async_trait]
    impl EmailProvider for EmailFalso {
        async fn enviar(&self, mensaje: &crate::email::EmailMessage) -> Result<(), String> {
            if self.falla {
                return Err("SMTP caído".to_string());
            }
            self.enviados.lock().expect("lock").push(mensaje.to.clone());
            Ok(())
        }
    }

    fn crear(
        orden_disponible: bool,
        usuarios: bool,
        falla_email: bool,
    ) -> (Notificaciones, Arc<EmailFalso>) {
        let clientes = Arc::new(ClientesFalsos {
            orden: orden_disponible.then(orden),
            usuarios,
            orden_consultada: Mutex::new(Vec::new()),
        });
        let email = Arc::new(EmailFalso {
            enviados: Mutex::new(Vec::new()),
            falla: falla_email,
        });
        (
            Notificaciones::nueva(clientes, Arc::clone(&email) as Arc<dyn EmailProvider>),
            email,
        )
    }

    #[tokio::test]
    async fn resuelve_la_orden_y_envia_un_correo_por_destinatario() {
        let (servicio, email) = crear(true, true, false);
        servicio
            .procesar(&evento(
                "payment.procesado",
                json!({ "order_id": ORDEN_ID }),
            ))
            .await
            .expect("procesar");
        assert_eq!(
            *email.enviados.lock().expect("lock"),
            vec!["ana@core.test".to_string(), "luis@core.test".to_string()]
        );
    }

    #[tokio::test]
    async fn ignora_eventos_sin_order_id_o_con_id_invalido() {
        let (servicio, email) = crear(true, true, false);
        servicio
            .procesar(&evento("payment.procesado", json!({})))
            .await
            .expect("sin id");
        servicio
            .procesar(&evento(
                "payment.procesado",
                json!({ "order_id": "../../x" }),
            ))
            .await
            .expect("id invalido");
        assert!(email.enviados.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn no_envia_nada_si_la_orden_no_esta_disponible() {
        let (servicio, email) = crear(false, true, false);
        servicio
            .procesar(&evento(
                "payment.procesado",
                json!({ "order_id": ORDEN_ID }),
            ))
            .await
            .expect("procesar");
        assert!(email.enviados.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn propaga_el_fallo_de_correo_para_que_el_consumidor_reintente() {
        let (servicio, _email) = crear(true, true, true);
        let error = servicio
            .procesar(&evento(
                "payment.procesado",
                json!({ "order_id": ORDEN_ID }),
            ))
            .await
            .expect_err("fallo");
        assert_eq!(error.mensaje(), "SMTP caído");
    }

    #[tokio::test]
    async fn no_envia_a_usuarios_sin_correo() {
        let (servicio, email) = crear(true, false, false);
        servicio
            .procesar(&evento(
                "payment.procesado",
                json!({ "order_id": ORDEN_ID }),
            ))
            .await
            .expect("procesar");
        assert!(email.enviados.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn resuelve_el_contexto_con_ambos_usuarios() {
        let clientes = ClientesFalsos {
            orden: Some(orden()),
            usuarios: true,
            orden_consultada: Mutex::new(Vec::new()),
        };
        let contexto = resolver_contexto(&clientes, orden())
            .await
            .expect("contexto");
        assert_eq!(contexto.comprador.expect("comprador").nombre, "Ana");
        assert_eq!(contexto.vendedor.expect("vendedor").nombre, "Luis");
    }

    #[tokio::test]
    async fn venta_directa_notifica_al_comprador_sin_exigir_vendedor() {
        let mut orden_directa = orden();
        orden_directa.vendedor_id = None;
        let clientes = Arc::new(ClientesFalsos {
            orden: Some(orden_directa),
            usuarios: true,
            orden_consultada: Mutex::new(Vec::new()),
        });
        let email = Arc::new(EmailFalso::default());
        let notificaciones =
            Notificaciones::nueva(clientes, Arc::clone(&email) as Arc<dyn EmailProvider>);
        notificaciones
            .procesar(&evento(
                "payment.procesado",
                json!({"order_id": ORDEN_ID, "monto_cents": 115000}),
            ))
            .await
            .expect("notificar venta directa");
        assert_eq!(email.enviados.lock().expect("enviados").len(), 1);
    }
}
