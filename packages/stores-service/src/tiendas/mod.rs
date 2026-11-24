//! Dominio de tiendas: tienda del vendedor, slugs, logo, fondos,
//! personalización y contenido del storefront.

pub mod contenido;
pub mod media;
pub mod personalizacion;
pub mod slug;
pub mod tema;

use crate::clients::{ObjetosStorage, TemasIdentity};
use crate::db::{fila_a_json, Db};
use crate::errors::ApiError;
use contenido::{
    contenido_de, contenido_json, normalizar_contenido, validar_contenido, ContenidoInput,
};
use media::{
    background_de, background_movil_de, background_movil_urls, background_urls,
    normalizar_background, normalizar_background_movil, validar_background,
    validar_background_movil, validar_media, BackgroundInput, UsoMedia,
};
use personalizacion::{
    con_seo_imagen_url, normalizar_overrides, overrides_de, personalizacion_json,
    validar_overrides, OverridesPersonalizacion, PersonalizacionInput,
};
use serde_json::{Map, Value};
use std::sync::Arc;
use tema::{plantilla_de, resolver_personalizacion, tema_parcial_de, TemaHerencia};
use uuid::Uuid;

pub const CAMPOS_TIENDA: &str = "id, empresa_id, negocio_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, slug, logo_objeto_id, background_objeto_id, background_media_type, background_poster_objeto_id, background_alt, background_overlay, background_position, background_activo, background_movil_objeto_id, background_movil_media_type, background_movil_poster_objeto_id, background_movil_alt, background_movil_activo, plantilla_override, tema_override, hero_titulo, hero_subtitulo, hero_cta_texto, hero_cta_url, seo_titulo, seo_descripcion, seo_imagen_objeto_id, personalizacion_actualizada_en, secciones, sobre_nosotros, politicas, redes, anuncio_texto, anuncio_enlace, anuncio_activo, contenido_actualizado_en, creado_en, actualizado_en";

const MAX_INTENTOS_SLUG: i64 = 50;

#[derive(Debug, Clone)]
pub struct TiendaResuelta {
    pub tienda: Value,
    pub overrides: OverridesPersonalizacion,
    pub redirigir_a: Option<String>,
}

pub struct Tiendas {
    db: Arc<Db>,
    temas: Arc<dyn TemasIdentity>,
    storage: Arc<dyn ObjetosStorage>,
}

fn texto(fila: &Value, clave: &str) -> String {
    fila.get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_opcional(fila: &Value, clave: &str) -> Option<String> {
    fila.get(clave).and_then(Value::as_str).map(str::to_string)
}

fn entero(fila: &Value, clave: &str) -> Option<i64> {
    fila.get(clave).and_then(Value::as_i64)
}

fn booleano(fila: &Value, clave: &str) -> bool {
    fila.get(clave).and_then(Value::as_bool).unwrap_or(false)
}

fn insertar(objeto: &mut Map<String, Value>, clave: &str, valor: Value) {
    objeto.insert(clave.to_string(), valor);
}

fn opt(valor: Option<String>) -> Value {
    valor.map(Value::String).unwrap_or(Value::Null)
}

/// Base36 de milisegundos, como `Date.now().toString(36)`.
pub fn base36(mut numero: u128) -> String {
    const DIGITOS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if numero == 0 {
        return "0".into();
    }
    let mut salida = Vec::new();
    while numero > 0 {
        salida.push(DIGITOS[(numero % 36) as usize]);
        numero /= 36;
    }
    salida.reverse();
    String::from_utf8(salida).expect("base36 ascii")
}

mod asignacion;
mod comunes;
mod consultas;
mod creacion;
mod edicion;

impl Tiendas {
    pub fn nueva(
        db: Arc<Db>,
        temas: Arc<dyn TemasIdentity>,
        storage: Arc<dyn ObjetosStorage>,
    ) -> Self {
        Self { db, temas, storage }
    }
}

pub fn verificar_acceso(
    actor: &crate::auth::UsuarioContexto,
    tienda: &Value,
    mensaje: &str,
) -> Result<(), ApiError> {
    let es_global = matches!(actor.rol.as_str(), "super_admin" | "admin");
    let es_empresario = actor.rol == "empresario"
        && actor
            .tenant_id
            .as_deref()
            .is_some_and(|tenant| tenant == texto(tienda, "empresa_id"));
    let es_vendedor = actor.rol == "vendedor"
        && (texto(tienda, "propietario_usuario_id") == actor.user_id
            || texto(tienda, "vendedor_id") == actor.user_id);
    if es_global || es_empresario || es_vendedor {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado(mensaje))
    }
}

pub fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

/// Resumen serializable usado por las pruebas y por la respuesta interna.
pub fn resumen(tienda: &Value) -> Value {
    serde_json::json!({
        "id": tienda.get("id"),
        "slug": tienda.get("slug"),
        "nombre": tienda.get("nombre"),
        "empresa_id": tienda.get("empresa_id"),
        "vendedor_id": tienda.get("vendedor_id"),
        "origen_personalizacion": tienda.get("origen_personalizacion"),
    })
}

mod tests {
    use super::*;
    use serde_json::json;

    fn actor(rol: &str, user: &str, tenant: Option<&str>) -> crate::auth::UsuarioContexto {
        crate::auth::UsuarioContexto {
            user_id: user.into(),
            rol: rol.into(),
            tenant_id: tenant.map(str::to_string),
            ..Default::default()
        }
    }

    fn tienda() -> Value {
        json!({
            "empresa_id": "e-1",
            "propietario_usuario_id": "u-1",
            "vendedor_id": "u-2",
        })
    }

    #[test]
    fn debe_autorizar_globales_empresario_del_tenant_y_vendedor_asignado() {
        assert!(verificar_acceso(&actor("admin", "x", None), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("empresario", "x", Some("e-1")), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("vendedor", "u-1", None), &tienda(), "no").is_ok());
        assert!(verificar_acceso(&actor("vendedor", "u-2", None), &tienda(), "no").is_ok());
        let error = verificar_acceso(&actor("empresario", "x", Some("e-2")), &tienda(), "no")
            .expect_err("403");
        assert_eq!(error.codigo, "ACCESO_DENEGADO");
        assert!(verificar_acceso(&actor("vendedor", "u-3", None), &tienda(), "no").is_err());
    }

    #[test]
    fn debe_calcular_base36_como_javascript() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1_700_000_000_000), "loyw3v28");
    }

    #[test]
    fn debe_detectar_la_violacion_de_unicidad() {
        let normal = sqlx::Error::RowNotFound;
        assert!(!es_violacion_unicidad(&normal));
    }
}
