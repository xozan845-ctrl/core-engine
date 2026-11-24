//! Empresas: alta, perfil, slug, pack y vendedores (ADR-25/ADR-26/ADR-42).
//!
//! Réplica de `empresas/empresas.service.ts`. Se opera con `to_jsonb` para
//! conservar el contrato completo de la fila sin duplicar 30 campos Rust.

pub struct Empresas {
    db: Arc<Db>,
    usuarios: Arc<Usuarios>,
    storage: Arc<Storage>,
    purga: Arc<PurgaExterna>,
    regiones: Arc<Regiones>,
    firma_clave: Arc<String>,
}

#[derive(Debug, Clone)]
pub struct DescargaExportacion {
    pub empresa_id: String,
    pub formato: String,
    pub sha256: String,
    pub firma: String,
    pub contenido: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CrearEmpresa {
    pub usuario_id: String,
    pub nombre: String,
    pub slug: Option<String>,
    pub perfil: Value,
    pub vertical_id: Option<String>,
    pub pack_codigo: Option<String>,
    pub pack_version: Option<i32>,
    /// Región tributaria elegida. Si es `None`, se usa la predeterminada del
    /// pack (fail-closed: sin región el alta falla).
    pub region_tributaria: Option<String>,
    pub capacidades_extra: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrearVendedorEmpresa {
    pub nombre: String,
    pub correo: String,
    pub contrasena: String,
    pub participacion_porcentaje: i32,
}

impl Empresas {
    pub fn nueva(
        db: Arc<Db>,
        usuarios: Arc<Usuarios>,
        storage: Arc<Storage>,
        purga: Arc<PurgaExterna>,
        regiones: Arc<Regiones>,
        firma_clave: String,
    ) -> Self {
        Self {
            db,
            usuarios,
            storage,
            purga,
            regiones,
            firma_clave: Arc::new(firma_clave),
        }
    }
}

mod comunes;
mod creacion;
mod eliminacion;
mod estado;
mod exportacion;
mod lectura;
mod packs;
mod perfil;
mod vendedores;

mod prelude {
    pub(crate) use crate::crypto::{hmac_sha256_hex, sha256_hex};
    pub(crate) use crate::db::Db;
    pub(crate) use crate::dominio::auditoria;
    pub(crate) use crate::dominio::media::{
        background_de, con_background_urls, normalizar_background, validar_background,
        BackgroundInput,
    };
    pub(crate) use crate::dominio::regiones::{resolver_en, RegionTributaria, Regiones};
    pub(crate) use crate::dominio::slug::{
        es_uuid, slug_con_sufijo, slug_desde_nombre, validar_slug_solicitado,
    };
    pub(crate) use crate::dominio::tema::{
        es_plantilla, mezclar_tema, normalizar_tema_parcial, plantilla_de,
        resolver_personalizacion, tema_parcial_de, validar_tema, validar_tema_input, HerenciaTema,
        PLANTILLAS,
    };
    pub(crate) use crate::dominio::usuarios::{correo_valido, CrearVendedor, Usuarios};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use crate::purga::PurgaExterna;
    pub(crate) use crate::storage::Storage;
    pub(crate) use chrono::Utc;
    pub(crate) use serde_json::{json, Map, Value};
    pub(crate) use sqlx::{Postgres, Row, Transaction};
    pub(crate) use std::sync::Arc;
    pub(crate) use uuid::Uuid;
}
use self::prelude::*;

const ESTADOS_EMPRESA: [&str; 3] = ["activa", "suspendida", "inactiva"];
const MAX_INTENTOS_SLUG: u32 = 50;

#[cfg(test)]
mod tests {
    use super::creacion::es_codigo_segmento;
    use super::estado::transicion_estado_permitida;
    use super::*;
    use crate::dominio::slug::{es_slug_valido, normalizar_slug};

    #[test]
    fn debe_validar_la_vertical_y_el_slug() {
        assert!(es_codigo_segmento("retail"));
        assert!(es_codigo_segmento("servicios_domicilio"));
        assert!(!es_codigo_segmento("1retail"));
        assert!(!es_codigo_segmento("Retail"));
    }

    #[test]
    fn debe_normalizar_el_slug_desde_el_nombre() {
        assert_eq!(slug_desde_nombre("Mi Empresa", "empresa"), "mi-empresa");
        assert!(es_slug_valido(&normalizar_slug("Café Ñandú")));
    }

    #[test]
    fn debe_validar_las_transiciones_de_estado_del_tenant() {
        assert!(transicion_estado_permitida("activa", "suspendida"));
        assert!(transicion_estado_permitida("activa", "inactiva"));
        assert!(transicion_estado_permitida("suspendida", "activa"));
        assert!(transicion_estado_permitida("suspendida", "inactiva"));
        assert!(transicion_estado_permitida("inactiva", "activa"));
        assert!(!transicion_estado_permitida("inactiva", "suspendida"));
        assert!(!transicion_estado_permitida("suspendida", "suspendida"));
    }

    #[test]
    fn debe_limpiar_la_respuesta_con_el_tema_resuelto() {
        let datos = json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "nombre": "Empresa",
            "slug": "empresa",
            "logo_objeto_id": "22222222-2222-4222-8222-222222222222",
            "plantilla_override": "minimal",
            "tema_override": {"color_primario": "#112233"},
            "creado_en": "2026-11-13T00:00:00+00:00"
        });
        let salida = Empresas::con_urls(&datos);
        assert_eq!(salida["plantilla"], "minimal");
        assert_eq!(salida["tema"]["color_primario"], "#112233");
        assert_eq!(salida["origen_personalizacion"]["plantilla"], "empresa");
        assert!(salida.get("plantilla_override").is_none());
        assert_eq!(
            salida["logo_url"],
            "/api/v2/storage/public/objetos/22222222-2222-4222-8222-222222222222"
        );
        assert_eq!(salida["creado_en"], "2026-11-13T00:00:00.000Z");
    }
}
