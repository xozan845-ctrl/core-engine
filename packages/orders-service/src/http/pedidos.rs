//! Rutas públicas de pedidos y cotizaciones.

use super::validacion::{campo, objeto, validar, Regla, ReglaCampo};
use super::{created, ok, query, query_numero, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::checkout::{ComandoCrear, LineaEntrada};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use std::collections::HashMap;

const CREAR_ORDEN: &[ReglaCampo] = &[
    campo("cliente_parte_id", false, &[Regla::Uuid]),
    campo(
        "tipo_orden",
        true,
        &[Regla::En(&crate::order_state::TIPOS_ORDEN)],
    ),
    campo("items", false, &[Regla::Arreglo]),
    campo("usar_carrito", false, &[Regla::Booleano]),
    campo("metodo_pago", false, &[Regla::En(&["simulado", "credito"])]),
    campo(
        "lista_precio_codigo",
        false,
        &[Regla::Patron(r"^[a-z][a-z0-9_-]{1,39}$")],
    ),
    campo(
        "segmento_codigo",
        false,
        &[Regla::Patron(r"^[a-z][a-z0-9_-]{0,39}$")],
    ),
];

const RECHAZAR_COTIZACION: &[ReglaCampo] = &[campo(
    "motivo",
    false,
    &[Regla::Texto, Regla::LongitudMax(500)],
)];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/orders", post(crear).get(mis_ordenes))
        .route("/api/v2/orders/admin/todas", get(todas))
        .route("/api/v2/orders/vendedor/mis-ventas", get(mis_ventas))
        .route("/api/v2/orders/tienda/{tienda_id}", get(por_tienda))
        .route("/api/v2/orders/empresa", get(por_empresa))
        .route("/api/v2/orders/{id}", get(detalle))
        .route("/api/v2/orders/{id}/timeline", get(timeline))
        .route(
            "/api/v2/orders/{id}/cotizacion/enviar",
            post(enviar_cotizacion),
        )
        .route(
            "/api/v2/orders/{id}/cotizacion/aceptar",
            post(aceptar_cotizacion),
        )
        .route(
            "/api/v2/orders/{id}/cotizacion/rechazar",
            post(rechazar_cotizacion),
        )
}

async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    validar(CREAR_ORDEN, &cuerpo)?;
    let mut comando: ComandoCrear = serde_json::from_value(cuerpo.clone())
        .map_err(|_| ApiError::validacion(json!([{"campo": "items", "restricciones": []}])))?;
    if let Some(items) = comando.items.as_ref() {
        if !items_validas(items) {
            return Err(ApiError::validacion(
                json!([{"campo": "items", "restricciones": []}]),
            ));
        }
    }
    // `usar_carrito` puede venir ausente: se normaliza a false.
    comando.usar_carrito = cuerpo
        .get("usar_carrito")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let orden = state
        .checkout()?
        .ejecutar(&comando, &usuario.user_id)
        .await?;
    created(orden)
}

async fn enviar_cotizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(
        &headers,
        &["super_admin", "admin", "empresario", "vendedor"],
        &["ordenes.leer"],
    )?;
    super::exigir_capacidades(&state, &usuario, &["cotizaciones"]).await?;
    let cotizacion = cotizacion_de(&state, &id).await?;
    let empresa = cotizacion.items.first().map(|i| i.empresa_id.as_str());
    let es_empresario = usuario.rol == "empresario"
        && empresa.is_some_and(|e| usuario.tenant_id.as_deref() == Some(e));
    if !usuario.es_admin_global() && cotizacion.vendedor_id != usuario.user_id && !es_empresario {
        return Err(ApiError::acceso_denegado(
            "Solo el vendedor o su empresa pueden enviar la cotizacion.",
        ));
    }
    if cotizacion.estado != "creada" {
        return Err(ApiError::dominio(
            "COTIZACION_ESTADO_INVALIDO",
            "Solo una cotizacion en borrador puede enviarse.",
        ));
    }
    let vista = state
        .checkout()?
        .transicionar(
            &id,
            "enviada",
            Some("cotizacion_enviada"),
            Some(&usuario.user_id),
            None,
        )
        .await?;
    ok(vista)
}

async fn aceptar_cotizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    let cotizacion = cotizacion_de(&state, &id).await?;
    if cotizacion.comprador_id != usuario.user_id {
        return Err(ApiError::acceso_denegado(
            "Solo el comprador de la cotizacion puede aceptarla.",
        ));
    }
    if cotizacion.estado != "enviada" {
        return Err(ApiError::dominio(
            "COTIZACION_ESTADO_INVALIDO",
            "Solo una cotizacion enviada puede aceptarse.",
        ));
    }
    let items: Vec<LineaEntrada> = cotizacion
        .items
        .iter()
        .map(|item| LineaEntrada {
            oferta_id: item.oferta_id.clone(),
            cantidad: item.cantidad,
            fulfillment: None,
            fulfillment_config: None,
        })
        .collect();
    let comando = ComandoCrear {
        tipo_orden: "compra".to_string(),
        items: Some(items),
        ..Default::default()
    };
    let orden = state
        .checkout()?
        .ejecutar(&comando, &usuario.user_id)
        .await?;
    let aceptada = state
        .checkout()?
        .transicionar(
            &id,
            "aceptada",
            Some("cotizacion_aceptada"),
            Some(&usuario.user_id),
            None,
        )
        .await?;
    created(json!({ "cotizacion": aceptada, "orden": orden }))
}

async fn rechazar_cotizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    validar(RECHAZAR_COTIZACION, &cuerpo)?;
    let cotizacion = cotizacion_de(&state, &id).await?;
    if cotizacion.comprador_id != usuario.user_id {
        return Err(ApiError::acceso_denegado(
            "Solo el comprador de la cotizacion puede rechazarla.",
        ));
    }
    if cotizacion.estado != "enviada" {
        return Err(ApiError::dominio(
            "COTIZACION_ESTADO_INVALIDO",
            "Solo una cotizacion enviada puede rechazarse.",
        ));
    }
    let motivo = objeto(&cuerpo)
        .get("motivo")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or("cotizacion_rechazada")
        .to_string();
    let vista = state
        .checkout()?
        .transicionar(
            &id,
            "rechazada",
            Some(&motivo),
            Some(&usuario.user_id),
            None,
        )
        .await?;
    ok(vista)
}

async fn todas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    super::autorizar(&headers, &["super_admin", "admin"], &[])?;
    let ordenes = state
        .vistas()?
        .listar_todo(
            state.db()?,
            query(&params, "estado"),
            query_numero(&params, "limite"),
        )
        .await?;
    ok(ordenes)
}

async fn mis_ventas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["vendedor"], &["ordenes.leer"])?;
    let ordenes = state
        .vistas()?
        .listar_de_vendedor(state.db()?, &usuario.user_id, query(&params, "estado"))
        .await?;
    let pedidos = state.pedidos()?;
    let recortadas = ordenes
        .into_iter()
        .map(|orden| pedidos.para_vendedor(orden, &usuario.user_id))
        .collect::<Result<Vec<_>, _>>()?;
    ok(recortadas)
}

async fn por_tienda(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tienda_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(
        &headers,
        &["super_admin", "empresario", "vendedor", "admin"],
        &["ordenes.leer"],
    )?;
    let personal_empresa = usuario.es_empresario_o_rol_configurable();
    let empresa_id = if personal_empresa {
        Some(usuario.tenant().map_err(|_| {
            ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa.")
        })?)
    } else {
        None
    };
    let vendedor_id = if usuario.rol == "vendedor" || usuario.rol_empresa_id.is_some() {
        Some(usuario.user_id.as_str())
    } else {
        None
    };
    let ordenes = state
        .vistas()?
        .listar_de_tienda(
            state.db()?,
            &tienda_id,
            query(&params, "estado"),
            vendedor_id,
            empresa_id,
        )
        .await?;
    ok(proyectar(&state, ordenes, &usuario, empresa_id)?)
}

async fn por_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(
        &headers,
        &["super_admin", "empresario", "admin"],
        &["ordenes.leer"],
    )?;
    let mut objetivo = query(&params, "empresaId").map(ToString::to_string);
    if usuario.es_empresario_o_rol_configurable() {
        objetivo = Some(
            usuario
                .tenant()
                .map_err(|_| {
                    ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa.")
                })?
                .to_string(),
        );
    }
    let Some(objetivo) = objetivo else {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "Indique la empresa a consultar.",
        ));
    };
    let ordenes = state
        .vistas()?
        .listar_de_empresa(
            state.db()?,
            &objetivo,
            query(&params, "estado"),
            query(&params, "tiendaId"),
            query_numero(&params, "limite"),
        )
        .await?;
    ok(proyectar(&state, ordenes, &usuario, Some(&objetivo))?)
}

async fn mis_ordenes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    let ordenes = state
        .vistas()?
        .listar_de_cliente(state.db()?, &usuario.user_id, query(&params, "estado"))
        .await?;
    ok(ordenes)
}

async fn detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(
        &headers,
        &[
            "super_admin",
            "empresario",
            "comprador",
            "vendedor",
            "admin",
        ],
        &["ordenes.leer"],
    )?;
    let orden = state
        .vistas()?
        .encontrar(state.db()?, &id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &id))?;
    ok(state.pedidos()?.detalle_para(orden, &usuario)?)
}

async fn timeline(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(
        &headers,
        &[
            "super_admin",
            "empresario",
            "comprador",
            "vendedor",
            "admin",
        ],
        &["ordenes.leer"],
    )?;
    let orden = state
        .vistas()?
        .encontrar(state.db()?, &id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &id))?;
    let visible = state.pedidos()?.detalle_para(orden, &usuario)?;
    let eventos = state
        .vistas()?
        .obtener_timeline(state.db()?, &visible.id)
        .await?;
    ok(state.pedidos()?.timeline_visible(eventos, &visible.items))
}

fn proyectar(
    state: &AppState,
    ordenes: Vec<crate::views::OrderView>,
    usuario: &UsuarioContexto,
    empresa_id: Option<&str>,
) -> Result<Vec<crate::views::OrderView>, ApiError> {
    if usuario.es_empresario_o_rol_configurable() {
        if let Some(empresa) = empresa_id {
            let pedidos = state.pedidos()?;
            return ordenes
                .into_iter()
                .map(|orden| pedidos.para_empresa(orden, empresa))
                .collect();
        }
    }
    if (usuario.rol == "vendedor" || usuario.rol_empresa_id.is_some())
        && !usuario.user_id.is_empty()
    {
        let pedidos = state.pedidos()?;
        return ordenes
            .into_iter()
            .map(|orden| pedidos.para_vendedor(orden, &usuario.user_id))
            .collect();
    }
    Ok(ordenes)
}

async fn cotizacion_de(state: &AppState, id: &str) -> Result<crate::views::OrderView, ApiError> {
    let cotizacion = state
        .vistas()?
        .encontrar(state.db()?, id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Cotizacion", id))?;
    if cotizacion.tipo_orden != "cotizacion" {
        return Err(ApiError::no_encontrado("Cotizacion", id));
    }
    Ok(cotizacion)
}

fn items_validas(items: &[LineaEntrada]) -> bool {
    items.iter().all(|item| {
        !item.oferta_id.trim().is_empty()
            && (1..=99).contains(&item.cantidad)
            && item
                .fulfillment
                .as_deref()
                .map(|valor| crate::order_state::FULFILLMENTS.contains(&valor))
                .unwrap_or(true)
            && item
                .fulfillment_config
                .as_ref()
                .map(|config| config.is_null() || config.is_object())
                .unwrap_or(true)
    })
}
