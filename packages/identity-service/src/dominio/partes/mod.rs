//! Partes universales: personas/organizaciones, contactos, direcciones y
//! relaciones (ADR-0001/R-AR-5).
//!
//! Réplica de `partes/partes.service.ts`: aislamiento por empresa, borrado
//! lógico y validación de metadata/roles de escritura.

mod comunes;
mod contactos;
mod direcciones;
mod gestion;
mod relaciones;

mod prelude {
    pub(crate) use crate::auth::UsuarioContexto;
    pub(crate) use crate::db::{iso_ms, Db};
    pub(crate) use crate::dominio::slug::es_uuid;
    pub(crate) use crate::errors::ApiError;
    pub(crate) use chrono::Utc;
    pub(crate) use serde_json::{json, Map, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
}
use self::prelude::*;

const ROLES_ESCRITURA: [&str; 5] = [
    "empresario",
    "vendedor",
    "coordinador",
    "supervisor",
    "operativo",
];
const TIPOS_PARTE: [&str; 2] = ["persona", "organizacion"];
const ESTADOS_PARTE: [&str; 3] = ["activo", "inactivo", "archivado"];
const TIPOS_CONTACTO: [&str; 5] = ["email", "telefono", "whatsapp", "web", "otro"];
const TIPOS_DIRECCION: [&str; 5] = ["fiscal", "comercial", "entrega", "operativa", "otro"];

pub struct Partes {
    db: Arc<Db>,
}

impl Partes {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use super::comunes::validar_metadata;
    use super::direcciones::normalizar_direccion;
    use super::gestion::normalizar_parte;
    use super::relaciones::normalizar_codigo;
    use super::*;
    use crate::auth::es_rol;
    use serde_json::json;

    #[test]
    fn debe_normalizar_y_validar_partes() {
        let parte =
            normalizar_parte(&json!({"tipo": "persona", "nombre": " Ana "})).expect("parte");
        assert_eq!(parte["nombre"], "Ana");
        assert_eq!(parte["estado"], "activo");
        assert!(normalizar_parte(&json!({"tipo": "robot", "nombre": "x"})).is_err());
        assert!(normalizar_parte(&json!({"tipo": "persona"})).is_err());
    }

    #[test]
    fn debe_validar_coordenadas_y_metadata() {
        assert!(normalizar_direccion(
            &json!({"tipo": "entrega", "linea_1": "Calle 1", "latitud": 12.1})
        )
        .is_ok());
        assert!(normalizar_direccion(
            &json!({"tipo": "entrega", "linea_1": "Calle 1", "latitud": 120})
        )
        .is_err());
        assert!(validar_metadata(Some(&json!({"a": 1}))).is_ok());
        assert!(validar_metadata(Some(&json!([]))).is_err());
    }

    #[test]
    fn debe_normalizar_codigos() {
        assert_eq!(
            normalizar_codigo("Proveedor", "TIPO").expect("codigo"),
            "proveedor"
        );
        assert!(normalizar_codigo("1malo", "TIPO").is_err());
    }

    #[test]
    fn debe_conservar_el_rol_de_escritura() {
        assert!(es_rol("empresario"));
        assert!(ROLES_ESCRITURA.contains(&"operativo"));
        assert!(!ROLES_ESCRITURA.contains(&"comprador"));
    }
}
