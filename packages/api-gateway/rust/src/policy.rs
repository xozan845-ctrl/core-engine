//! Políticas de acceso por ruta/rol en el borde (R-GW-1, R-GW-3).
//!
//! Deny-by-default: toda ruta `/api/v2/*` no declarada exige un token válido
//! pero no concede roles. Las rutas públicas declaran `roles: None`.

use regex::Regex;
use std::sync::OnceLock;

pub const SUPER_ADMIN: &str = "super_admin";
pub const ADMIN: &str = "admin";
pub const EMPRESARIO: &str = "empresario";
pub const VENDEDOR: &str = "vendedor";
pub const COMPRADOR: &str = "comprador";
pub const LOGISTICA: &str = "logistica";
pub const COORDINADOR: &str = "coordinador";
pub const SUPERVISOR: &str = "supervisor";
pub const OPERATIVO: &str = "operativo";

pub const ROLES: [&str; 9] = [
    SUPER_ADMIN,
    ADMIN,
    EMPRESARIO,
    VENDEDOR,
    COMPRADOR,
    LOGISTICA,
    COORDINADOR,
    SUPERVISOR,
    OPERATIVO,
];
pub const ROLES_LOGISTICA: [&str; 6] = [
    EMPRESARIO,
    ADMIN,
    LOGISTICA,
    COORDINADOR,
    SUPERVISOR,
    OPERATIVO,
];

#[derive(Debug)]
pub struct Politica {
    pub patron: Regex,
    pub metodos: Vec<&'static str>,
    /// `None` = pública; `Some(lista)` = roles permitidos.
    pub roles: Option<Vec<&'static str>>,
}

fn p(patron: &str, metodos: &[&'static str], roles: Option<Vec<&'static str>>) -> Politica {
    Politica {
        patron: Regex::new(patron).expect("regex de politica"),
        metodos: metodos.to_vec(),
        roles,
    }
}

pub fn politicas() -> &'static [Politica] {
    static POLITICAS: OnceLock<Vec<Politica>> = OnceLock::new();
    POLITICAS.get_or_init(|| {
        let todos = ROLES.to_vec();
        vec![
            p(r"^/api/v2/auth/registro$", &["POST"], None),
            p(r"^/api/v2/auth/login$", &["POST"], None),
            p(r"^/api/v2/auth/refresh$", &["POST"], None),
            p(r"^/api/v2/auth/token$", &["POST"], None),
            p(r"^/api/v2/auth/restablecer-contrasena$", &["POST"], None),
            p(r"^/api/v2/auth/restablecer-contrasena/confirmar$", &["POST"], None),
            p(r"^/api/v2/packs(/|$)", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/packs$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/packs/(modelos-operacion|verticales)(/|$)", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/packs/(modelos-operacion|verticales)(/|$)", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/configuracion/sitio$", &["GET"], None),
            p(r"^/api/v2/configuracion/sitio$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/catalog/items(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/(configuracion-inventario|escanear|combos|promociones|inventario)(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/slug$", &["PATCH"], Some(vec![VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/logo$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/background$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/personalizacion$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/contenido$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/tiendas/[^/]+/tema\.css$", &["GET"], None),
            p(r"^/api/v2/tiendas/[^/]+$", &["GET"], None),
            p(r"^/api/v2/usuarios/[^/]+/avatar$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/usuarios/[^/]+$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/usuarios(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/auth/crear-usuario$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas$", &["POST"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/partes(/|$)", &["GET", "POST", "PATCH", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/empresas/[^/]+/extensiones(/|$)", &["GET", "POST", "PATCH", "PUT", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/empresas/[^/]+/logistica$", &["PATCH"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/perfil$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/tipo$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/vendedores$", &["POST", "GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/invitaciones/vendedores(/|$)", &["POST", "DELETE"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/(roles|puestos|asignaciones-personal|invitaciones/personal)(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/incorporaciones/[^/]+$", &["GET"], None),
            p(r"^/api/v2/incorporaciones/[^/]+/aceptar$", &["POST"], None),
            p(r"^/api/v2/empresas/[^/]+/slug$", &["PATCH"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/auth/me$", &["GET"], None),
            p(r"^/api/v2/auth/logout$", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/auth/(cambiar-contrasena|vincular-personal)$", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/vendedores/(me/ventas|me/liquidaciones)$", &["GET"], Some(vec![VENDEDOR])),
            p(r"^/api/v2/empresarios/me/liquidaciones$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/ventas$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/tiendas$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/envios(/|$)", &["GET", "POST"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/envios/[^/]+/estado$", &["PATCH"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/scheduling(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/vendedores/", &["GET", "POST", "PATCH", "PUT", "DELETE"], Some(vec![VENDEDOR, EMPRESARIO])),
            p(r"^/api/v2/orders$", &["POST"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/orders$", &["GET"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/orders/[^/]+/estado$", &["PATCH", "POST"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA])),
            p(r"^/api/v2/orders/[^/]+/cotizacion/enviar$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/orders/[^/]+/cotizacion/(aceptar|rechazar)$", &["POST"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/orders/admin/todas$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/orders/vendedor/mis-ventas$", &["GET"], Some(vec![VENDEDOR])),
            p(r"^/api/v2/orders/vendedor/comisiones$", &["GET"], Some(vec![SUPER_ADMIN, VENDEDOR, ADMIN])),
            p(r"^/api/v2/orders/tienda/[^/]+$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, VENDEDOR, ADMIN])),
            p(r"^/api/v2/orders/empresa$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN])),
            p(r"^/api/v2/orders/[^/]+/timeline$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, COMPRADOR, VENDEDOR, ADMIN])),
            p(r"^/api/v2/orders/[^/]+$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, COMPRADOR, VENDEDOR, ADMIN])),
            p(r"^/api/v2/carrito", &["GET", "POST", "PATCH", "DELETE"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/storage/public/objetos/[^/]+$", &["GET", "HEAD"], None),
            p(r"^/api/v2/storage/bucket$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/storage/", &["*"], Some(todos.clone())),
            p(r"^/api/v2/api-keys(/|$)", &["GET", "POST", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/admin/inventario", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/admin/", &["*"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/comercio/", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/finanzas/", &["*"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/field/", &["*"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/inteligencia/me/", &["GET"], Some(vec![VENDEDOR, ADMIN])),
            p(r"^/api/v2/inteligencia/", &["GET"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/inteligencia/ventas/registrar$", &["POST"], Some(vec![VENDEDOR, EMPRESARIO, SUPER_ADMIN, ADMIN])),
            // Deny-by-default.
            p(r"^/api/v2/", &["*"], Some(Vec::new())),
        ]
    })
}

/// Primera política que coincide con ruta+método; `None` si no aplica.
pub fn encontrar_politica(ruta: &str, metodo: &str) -> Option<&'static Politica> {
    politicas().iter().find(|politica| {
        politica.patron.is_match(ruta)
            && (politica.metodos.contains(&metodo.to_uppercase().as_str())
                || politica.metodos.contains(&"*"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_marcar_publicas_y_denegar_por_defecto() {
        assert!(encontrar_politica("/api/v2/auth/login", "POST")
            .expect("login")
            .roles
            .is_none());
        assert_eq!(
            encontrar_politica("/api/v2/ruta-inexistente", "GET")
                .expect("deny")
                .roles
                .as_ref()
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn debe_respetar_el_metodo() {
        // `GET /auth/login` no está declarada: cae en deny-by-default (sin roles).
        let por_defecto = encontrar_politica("/api/v2/auth/login", "GET").expect("deny");
        assert_eq!(por_defecto.roles.as_ref().map(Vec::len), Some(0));
        assert!(encontrar_politica("/api/v2/orders", "POST").is_some());
    }

    #[test]
    fn debe_asignar_los_roles_esperados() {
        let politica = encontrar_politica("/api/v2/finanzas/resumen", "GET").expect("finanzas");
        assert_eq!(politica.roles, Some(vec![SUPER_ADMIN, ADMIN]));
        let media = encontrar_politica("/api/v2/storage/public/objetos/abc", "GET").expect("media");
        assert!(media.roles.is_none());
    }
}
