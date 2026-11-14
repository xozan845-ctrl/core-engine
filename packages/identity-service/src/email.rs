//! Correo transaccional (ADR-0003): modo `log` en desarrollo/test y SMTP en
//! staging/producción.
//!
//! Réplica de `packages/shared/src/email/email.provider.ts`. El dominio
//! (`password_reset.rs`) depende del puerto `Email`, nunca de lettre.

use crate::config::Config;
use lettre::message::{header::ContentType, Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

pub trait Email: Send + Sync {
    fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String>;
}

pub struct EmailLog;

impl Email for EmailLog {
    fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
        crate::logs::info(
            "email.log",
            crate::campos! {
                "msg": "email_simulado",
                "to": mask_email(&mensaje.to),
                "subject": mensaje.subject,
            },
        );
        Ok(())
    }
}

pub struct EmailSmtp {
    from: String,
    host: String,
    port: u16,
    user: Option<String>,
    password: Option<String>,
}

impl EmailSmtp {
    pub fn nueva(config: &Config) -> Result<Self, String> {
        let host = config
            .smtp_host
            .clone()
            .ok_or_else(|| "SMTP_HOST es obligatoria con EMAIL_MODE=smtp.".to_string())?;
        let from = config
            .email_from
            .clone()
            .ok_or_else(|| "EMAIL_FROM es obligatoria con EMAIL_MODE=smtp.".to_string())?;
        Ok(Self {
            from,
            host,
            port: config.smtp_port,
            user: config.smtp_user.clone(),
            password: config.smtp_password.clone(),
        })
    }

    fn transporte(&self) -> SmtpTransport {
        let constructor = if self.port == 465 {
            SmtpTransport::relay(&self.host).expect("host SMTP valido")
        } else {
            SmtpTransport::builder_dangerous(&self.host)
        };
        let constructor = constructor.port(self.port);
        match (&self.user, &self.password) {
            (Some(usuario), Some(clave)) => constructor
                .credentials(Credentials::new(usuario.clone(), clave.clone()))
                .build(),
            _ => constructor.build(),
        }
    }
}

impl Email for EmailSmtp {
    fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
        let from: Mailbox = self
            .from
            .parse()
            .map_err(|_| "EMAIL_FROM no es una direccion valida.".to_string())?;
        let to: Mailbox = mensaje
            .to
            .parse()
            .map_err(|_| "El destinatario no es una direccion valida.".to_string())?;
        let constructor = Message::builder()
            .from(from)
            .to(to)
            .subject(&mensaje.subject);
        let correo = match &mensaje.html {
            Some(html) => constructor
                .multipart(MultiPart::alternative_plain_html(
                    mensaje.text.clone(),
                    html.clone(),
                ))
                .map_err(|error| error.to_string())?,
            None => constructor
                .header(ContentType::TEXT_PLAIN)
                .body(mensaje.text.clone())
                .map_err(|error| error.to_string())?,
        };
        self.transporte()
            .send(&correo)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Fábrica única. `log` por defecto; `smtp` exige configuración completa
/// (validada también en `Config::validar_arranque`).
pub fn crear_email(config: &Config) -> Result<Box<dyn Email>, String> {
    match config.email_mode.as_str() {
        "log" => Ok(Box::new(EmailLog)),
        "smtp" => Ok(Box::new(EmailSmtp::nueva(config)?)),
        otro => Err(format!("EMAIL_MODE invalido: \"{otro}\". Usa log o smtp.")),
    }
}

/// Enmascara el correo para logs: conserva la primera letra y el dominio.
pub fn mask_email(correo: &str) -> String {
    match correo.split_once('@') {
        Some((usuario, dominio)) if !usuario.is_empty() && !dominio.is_empty() => {
            format!("{}***@{dominio}", &usuario[..1])
        }
        _ => "***".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct EmailFake {
        enviados: Arc<Mutex<Vec<EmailMessage>>>,
    }

    impl Email for EmailFake {
        fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
            self.enviados.lock().expect("fake").push(mensaje.clone());
            Ok(())
        }
    }

    #[test]
    fn debe_enmascarar_el_correo_sin_filtrar_el_usuario() {
        assert_eq!(mask_email("ana@tienda.test"), "a***@tienda.test");
        assert_eq!(mask_email("sin-arroba"), "***");
        assert_eq!(mask_email("@tienda.test"), "***");
    }

    #[test]
    fn el_modo_log_no_falla_y_el_puerto_por_defecto_es_587() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(config.email_mode, "log");
        assert_eq!(config.smtp_port, 587);
        assert!(crear_email(&config).is_ok());
    }

    #[test]
    fn el_fake_registra_el_mensaje_para_las_pruebas_de_dominio() {
        let fake = EmailFake::default();
        fake.enviar(&EmailMessage {
            to: "ana@tienda.test".into(),
            subject: "Hola".into(),
            text: "texto".into(),
            html: None,
        })
        .expect("envio");
        assert_eq!(fake.enviados.lock().expect("fake").len(), 1);
    }
}
