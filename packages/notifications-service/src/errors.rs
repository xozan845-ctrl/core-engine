//! Errores internos del consumidor y del envío de correo.
//!
//! El servicio no expone API pública de negocio; estos errores viajan al log y
//! a la cabecera `x-error` de la DLQ con el mismo mensaje observable que el
//! TypeScript.

#[derive(Debug, thiserror::Error)]
pub enum ErrorNotificacion {
    #[error("{0}")]
    Cliente(String),
    #[error("{0}")]
    Correo(String),
    #[error("{0}")]
    Evento(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl ErrorNotificacion {
    pub fn mensaje(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_conservar_el_mensaje_observable() {
        let error = ErrorNotificacion::Cliente("No se pudo consultar identity.".to_string());
        assert_eq!(error.mensaje(), "No se pudo consultar identity.");
        let error = ErrorNotificacion::Correo("SMTP caído".to_string());
        assert_eq!(error.to_string(), "SMTP caído");
    }
}
