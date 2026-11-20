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
            p(r"^/api/v2/auth/verificar-correo$", &["POST"], None),
            p(r"^/api/v2/auth/reenviar-verificacion$", &["POST"], None),
            p(r"^/api/v2/pagos/webhook$", &["POST"], None),
            p(r"^/api/v2/auth/mfa(/|$)", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/auth/sesiones(/|$)", &["GET", "DELETE"], Some(todos.clone())),
            p(r"^/api/v2/packs(/|$)", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/packs$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/packs/(modelos-operacion|verticales)(/|$)", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/packs/(modelos-operacion|verticales)(/|$)", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/configuracion/sitio$", &["GET"], None),
            p(r"^/api/v2/configuracion/sitio$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/catalog/items(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/(configuracion-inventario|escanear|combos|inventario)(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/(listas-precio|promociones)(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/series(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/items/[^/]+/(precio|escalas|tarifas|cotizacion)(/|$)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
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
            p(r"^/api/v2/admin/proveedores-pago(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/configuracion-pagos$", &["GET", "PUT"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/estado$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/exportacion$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/eliminacion/purgar$", &["POST"], Some(vec![SUPER_ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/eliminacion$", &["GET", "POST", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/tipo$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/vendedores$", &["POST", "GET"], Some(vec![EMPRESARIO])),
            // La capacidad fina por negocio se valida en identity-service.
            p(r"^/api/v2/empresas/[^/]+/negocios/[^/]+/capacidades(/|$)", &["GET", "PATCH"], Some(todos.clone())),
            p(r"^/api/v2/empresas/[^/]+/negocios(/|$)", &["GET", "POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/negocios/[^/]+/plantillas$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/negocios/[^/]+/estado$", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/negocios/plantillas$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/personal/[^/]+/transferir$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/empresas/[^/]+/invitaciones/vendedores(/|$)", &["POST", "DELETE"], Some(vec![EMPRESARIO])),
            // La autorización fina de roles, puestos e invitaciones vive en
            // identity-service: un rol de negocio puede administrar su negocio
            // aunque su rol global no sea `empresario`.
            p(r"^/api/v2/empresas/[^/]+/(roles|puestos|asignaciones-personal|invitaciones/personal)(/|$)", &["GET", "POST", "PATCH"], Some(todos.clone())),
            p(r"^/api/v2/incorporaciones/[^/]+$", &["GET"], None),
            p(r"^/api/v2/incorporaciones/[^/]+/aceptar$", &["POST"], None),
            p(r"^/api/v2/tienda/items/[^/]+/resenas$", &["GET"], None),
            p(r"^/api/v2/comprador/(resenas|favoritos)(/|$)", &["GET", "POST", "PATCH", "DELETE"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/admin/resenas/", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresas/[^/]+/slug$", &["PATCH"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresas/", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/auth/me$", &["GET"], None),
            p(r"^/api/v2/auth/logout$", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/auth/negocio-activo$", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/auth/negocios$", &["GET"], Some(todos.clone())),
            p(r"^/api/v2/auth/(cambiar-contrasena|vincular-personal)$", &["POST"], Some(todos.clone())),
            p(r"^/api/v2/vendedores/(me/ventas|me/liquidaciones)$", &["GET"], Some(vec![VENDEDOR])),
            p(r"^/api/v2/empresarios/me/liquidaciones$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/reportes/negocios$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/analytics/plataforma$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/analytics(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/admin/notifications/webhooks(/|$)", &["GET", "POST", "PATCH", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/admin/notifications(/|$)", &["GET", "PUT", "POST", "PATCH", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/empresarios/me/ventas$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/tiendas$", &["GET"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/empresarios/me/envios(/|$)", &["GET", "POST"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/envios/[^/]+/estado$", &["PATCH"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/envios/[^/]+/prueba-entrega$", &["POST"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
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
            p(r"^/api/v2/orders/empresa/resumen$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN])),
            p(r"^/api/v2/orders/empresa$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN])),
            p(r"^/api/v2/orders/[^/]+/timeline$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, COMPRADOR, VENDEDOR, ADMIN])),
            p(r"^/api/v2/orders/[^/]+$", &["GET"], Some(vec![SUPER_ADMIN, EMPRESARIO, COMPRADOR, VENDEDOR, ADMIN])),
            p(r"^/api/v2/carrito", &["GET", "POST", "PATCH", "DELETE"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/storage/public/objetos/[^/]+$", &["GET", "HEAD"], None),
            p(r"^/api/v2/storage/bucket$", &["POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/storage/", &["*"], Some(todos.clone())),
            p(r"^/api/v2/api-keys(/|$)", &["GET", "POST", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/onboarding(/|$)", &["GET", "POST"], Some(vec![EMPRESARIO])),
            p(r"^/api/v2/plataforma(/|$)", &["GET", "POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/admin/plataforma(/|$)", &["GET", "POST"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/admin/(inventario|ubicaciones)", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/admin/", &["*"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/comercio/", &["*"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/finanzas/", &["*"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/field/", &["*"], Some(vec![SUPER_ADMIN, EMPRESARIO, ADMIN, LOGISTICA, COORDINADOR, SUPERVISOR, OPERATIVO])),
            p(r"^/api/v2/inteligencia/me/", &["GET"], Some(vec![VENDEDOR, ADMIN])),
            p(r"^/api/v2/inteligencia/", &["GET"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/inteligencia/ventas/registrar$", &["POST"], Some(vec![VENDEDOR, EMPRESARIO, SUPER_ADMIN, ADMIN])),
            // Superficies que ya existían en los servicios pero no tenían
            // política de borde.
            p(r"^/api/v2/empresas/[^/]+/configuracion-pagos$", &["GET", "PUT"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/fidelizacion/mis-", &["GET", "POST"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/fidelizacion(/|$)", &["GET", "PUT", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/contratos(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/suscripciones(/|$)", &["GET", "POST"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/mesas(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/comandas(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO, VENDEDOR])),
            p(r"^/api/v2/pedidos/[^/]+/entregas-digitales$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, COMPRADOR])),
            p(r"^/api/v2/entregas-digitales/[^/]+$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, COMPRADOR])),
            p(r"^/api/v2/orders/[^/]+/devoluciones$", &["GET", "POST"], Some(vec![COMPRADOR, EMPRESARIO, ADMIN])),
            p(r"^/api/v2/admin/devoluciones(/|$)", &["GET", "POST", "PATCH"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/catalog/cupones(/|$)", &["GET", "POST", "PATCH", "DELETE"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/comprador/(resenas|favoritos)(/|$)", &["GET", "POST", "PATCH", "DELETE"], Some(vec![COMPRADOR])),
            p(r"^/api/v2/tienda/items/[^/]+/resenas$", &["GET"], None),
            p(r"^/api/v2/admin/resenas(/|$)", &["PATCH"], Some(vec![SUPER_ADMIN, ADMIN])),
            p(r"^/api/v2/exportaciones/[^/]+/descarga$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
            p(r"^/api/v2/negocios/plantillas$", &["GET"], Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO])),
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
        assert!(encontrar_politica("/api/v2/pagos/webhook", "POST")
            .expect("webhook")
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
    fn debe_denegar_por_defecto_y_asignar_roles() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/api/core-engine.openapi.json"))
                .expect("openapi publico");
        let mut sin_politica = Vec::new();
        for (ruta, operaciones) in spec["paths"].as_object().expect("paths") {
            for metodo in operaciones.as_object().expect("metodos").keys() {
                let politica = encontrar_politica(ruta, metodo.to_uppercase().as_str())
                    .expect("politica");
                if matches!(&politica.roles, Some(roles) if roles.is_empty()) {
                    sin_politica.push(format!("{} {}", metodo.to_uppercase(), ruta));
                }
            }
        }
        assert!(
            sin_politica.is_empty(),
            "operaciones publicas sin politica de rol:\n{}",
            sin_politica.join("\n")
        );
    }

    #[test]
    fn debe_asignar_los_roles_esperados() {
        let politica = encontrar_politica("/api/v2/finanzas/resumen", "GET").expect("finanzas");
        assert_eq!(politica.roles, Some(vec![SUPER_ADMIN, ADMIN]));
        let media = encontrar_politica("/api/v2/storage/public/objetos/abc", "GET").expect("media");
        assert!(media.roles.is_none());
        let series = encontrar_politica("/api/v2/catalog/series/abc", "PATCH").expect("series");
        assert_eq!(series.roles, Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO]));
        let negocios =
            encontrar_politica("/api/v2/empresas/e-1/negocios", "POST").expect("negocios");
        assert_eq!(negocios.roles, Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO]));
        let estado = encontrar_politica("/api/v2/empresas/e-1/estado", "PATCH").expect("estado");
        assert_eq!(estado.roles, Some(vec![SUPER_ADMIN, ADMIN]));
        let purgar =
            encontrar_politica("/api/v2/empresas/e-1/eliminacion/purgar", "POST").expect("purgar");
        assert_eq!(purgar.roles, Some(vec![SUPER_ADMIN]));
        let capacidades =
            encontrar_politica("/api/v2/empresas/e-1/negocios/n-1/capacidades", "PATCH")
                .expect("capacidades negocio");
        assert_eq!(capacidades.roles.as_ref().map(Vec::len), Some(ROLES.len()));
        let plan =
            encontrar_politica("/api/v2/plataforma/suscripciones", "POST").expect("plataforma");
        assert_eq!(plan.roles, Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO]));
        let admin_plan = encontrar_politica("/api/v2/admin/plataforma/suscripciones", "GET")
            .expect("admin plataforma");
        assert_eq!(admin_plan.roles, Some(vec![SUPER_ADMIN, ADMIN]));
        let webhooks =
            encontrar_politica("/api/v2/admin/notifications/webhooks", "POST").expect("webhooks");
        assert_eq!(webhooks.roles, Some(vec![SUPER_ADMIN, ADMIN, EMPRESARIO]));
        let plantillas = encontrar_politica("/api/v2/admin/notifications/plantillas", "PUT")
            .expect("plantillas");
        assert_eq!(plantillas.roles, Some(vec![SUPER_ADMIN, ADMIN]));
        let negocio_activo =
            encontrar_politica("/api/v2/auth/negocio-activo", "POST").expect("negocio-activo");
        assert_eq!(
            negocio_activo.roles.as_ref().map(Vec::len),
            Some(ROLES.len())
        );
    }
}
