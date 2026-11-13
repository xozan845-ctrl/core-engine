//! Correo transaccional (ADR-0003): `log` en dev/test y SMTP en stage/prod.
//!
//! Réplica del contrato `EmailProvider` de `@core/shared/email` usando
//! `lettre` para el transporte SMTP. Los logs nunca incluyen el cuerpo ni el
//! correo completo: solo el destinatario enmascarado y el asunto.

use crate::campos;
use crate::config::{ConfigError, EmailConfig, EmailMode};
use crate::logs;
use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

#[async_trait]
pub trait EmailProvider: Send + Sync {
    async fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String>;
}

/// Enmascara el correo para logs: conserva la primera letra y el dominio.
pub fn mask_correo(correo: &str) -> String {
    let Some(arroba) = correo.find('@') else {
        return "***".to_string();
    };
    if arroba == 0 || arroba == correo.len() - 1 {
        return "***".to_string();
    }
    let primera = correo.chars().next().unwrap_or('*');
    format!("{primera}***@{}", &correo[arroba + 1..])
}

/// Modo log: no envía correo. Deja una traza sin cuerpo ni token para que
/// dev/test puedan validar el flujo sin SMTP ni datos sensibles.
pub struct LogEmailProvider;

#[async_trait]
impl EmailProvider for LogEmailProvider {
    async fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
        logs::info(
            "email.log",
            campos! {
                "msg": "email_simulado",
                "to": mask_correo(&mensaje.to),
                "subject": mensaje.subject,
            },
        );
        Ok(())
    }
}

/// SMTP genérico (Supabase Auth SMTP, relay corporativo, etc.).
pub struct SmtpEmailProvider {
    transporte: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpEmailProvider {
    fn nuevo(config: &EmailConfig) -> Result<Self, ConfigError> {
        let host = config
            .smtp_host
            .as_deref()
            .ok_or_else(|| ConfigError::Email("SMTP_HOST es obligatorio.".to_string()))?;
        let from = config
            .from
            .as_deref()
            .ok_or_else(|| ConfigError::Email("EMAIL_FROM es obligatorio.".to_string()))?
            .parse::<Mailbox>()
            .map_err(|error| ConfigError::Email(format!("EMAIL_FROM invalido: {error}")))?;

        let tls = TlsParameters::new(host.to_string())
            .map_err(|error| ConfigError::Email(format!("Configuracion TLS invalida: {error}")))?;
        let tls = if config.es_seguro() {
            Tls::Wrapper(tls)
        } else {
            Tls::Opportunistic(tls)
        };
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(config.smtp_port)
            .tls(tls);
        if let (Some(usuario), Some(password)) = (&config.smtp_user, &config.smtp_password) {
            builder = builder.credentials(Credentials::new(usuario.clone(), password.clone()));
        }

        Ok(Self {
            transporte: builder.build(),
            from,
        })
    }
}

#[async_trait]
impl EmailProvider for SmtpEmailProvider {
    async fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
        let destino = mensaje
            .to
            .parse::<Mailbox>()
            .map_err(|error| format!("Destinatario invalido: {error}"))?;
        let constructor = Message::builder()
            .from(self.from.clone())
            .to(destino)
            .subject(mensaje.subject.clone());
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
        self.transporte
            .send(correo)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Fábrica única: `log` por defecto; `smtp` exige configuración completa y
/// falla al arrancar si falta (R-DS-1: los secretos vienen del entorno).
pub fn crear_proveedor(config: &EmailConfig) -> Result<Arc<dyn EmailProvider>, ConfigError> {
    config.validar()?;
    match config.modo {
        EmailMode::Log => Ok(Arc::new(LogEmailProvider)),
        EmailMode::Smtp => Ok(Arc::new(SmtpEmailProvider::nuevo(config)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_enmascarar_el_correo_sin_exponer_el_local_completo() {
        assert_eq!(mask_correo("ana@core.test"), "a***@core.test");
        assert_eq!(mask_correo("ab@x"), "a***@x");
        assert_eq!(mask_correo("sin-arroba"), "***");
        assert_eq!(mask_correo("@core.test"), "***");
        assert_eq!(mask_correo("ana@"), "***");
    }

    #[tokio::test]
    async fn debe_usar_log_por_defecto_y_no_fallar() {
        let config = EmailConfig {
            modo: EmailMode::Log,
            from: None,
            smtp_host: None,
            smtp_port: 587,
            smtp_user: None,
            smtp_password: None,
        };
        let proveedor = crear_proveedor(&config).expect("log");
        proveedor
            .enviar(&EmailMessage {
                to: "ana@core.test".to_string(),
                subject: "Hola".to_string(),
                text: "token-secreto-123".to_string(),
                html: None,
            })
            .await
            .expect("envio log");
    }

    #[test]
    fn debe_crear_el_proveedor_smtp_con_tls() {
        let config = EmailConfig {
            modo: EmailMode::Smtp,
            from: Some("no-reply@test".to_string()),
            smtp_host: Some("smtp.test".to_string()),
            smtp_port: 465,
            smtp_user: None,
            smtp_password: None,
        };
        let proveedor = crear_proveedor(&config).expect("smtp");
        assert!(Arc::strong_count(&proveedor) >= 1);
    }

    #[tokio::test]
    async fn debe_rechazar_un_destinatario_invalido_sin_conectar() {
        let config = EmailConfig {
            modo: EmailMode::Smtp,
            from: Some("no-reply@test".to_string()),
            smtp_host: Some("smtp.test".to_string()),
            smtp_port: 2525,
            smtp_user: None,
            smtp_password: None,
        };
        let proveedor = SmtpEmailProvider::nuevo(&config).expect("smtp");
        let error = proveedor
            .enviar(&EmailMessage {
                to: "sin-arroba".to_string(),
                subject: "Hola".to_string(),
                text: "cuerpo".to_string(),
                html: Some("<b>cuerpo</b>".to_string()),
            })
            .await
            .expect_err("destinatario invalido");
        assert!(error.starts_with("Destinatario invalido:"));
    }

    #[test]
    fn debe_rechazar_un_remitente_invalido() {
        let config = EmailConfig {
            modo: EmailMode::Smtp,
            from: Some("sin-arroba".to_string()),
            smtp_host: Some("smtp.test".to_string()),
            smtp_port: 587,
            smtp_user: None,
            smtp_password: None,
        };
        assert!(matches!(
            crear_proveedor(&config),
            Err(ConfigError::Email(_))
        ));
    }
}
