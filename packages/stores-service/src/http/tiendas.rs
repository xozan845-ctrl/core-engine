//! Handlers HTTP de tiendas.

use super::validacion::{self, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{ROLES_EMPRESARIO, ROLES_SOLO_VENDEDOR, ROLES_TIENDA_GESTION, ROLES_VENDEDOR};
use crate::errors::ApiError;
use crate::state::AppState;
use crate::tiendas::contenido::ContenidoInput;
use crate::tiendas::media::BackgroundInput;
use crate::tiendas::personalizacion::PersonalizacionInput;
use crate::tiendas::tema::generar_tema_css;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn cadena(cuerpo: &Value, clave: &str) -> Option<String> {
    cuerpo
        .get(clave)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

/// `POST /api/v2/vendedores/tienda`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_VENDEDOR, &[], &[]).await?;
    validar(validacion::CREAR_TIENDA, &cuerpo)?;
    let tenant = usuario
        .tenant_id
        .as_deref()
        .ok_or_else(|| ApiError::no_encontrado("Empresa", "activa"))?;
    let es_empresario = usuario.rol == "empresario";
    let participacion = if es_empresario {
        None
    } else {
        usuario.participacion_porcentaje
    };
    if !es_empresario && participacion.is_none() {
        return Err(ApiError::no_encontrado("Participacion", "del vendedor"));
    }
    let vendedor_id = if es_empresario {
        None
    } else {
        Some(usuario.user_id.as_str())
    };
    let tienda = state
        .tiendas()?
        .crear(
            tenant,
            &usuario.user_id,
            participacion,
            &cadena(&cuerpo, "nombre").unwrap_or_default(),
            cadena(&cuerpo, "descripcion").as_deref(),
            cadena(&cuerpo, "slug").as_deref(),
            vendedor_id,
        )
        .await?;
    created(tienda)
}

/// `GET /api/v2/vendedores/me/tienda`.
pub async fn mi_tienda(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SOLO_VENDEDOR, &[], &[]).await?;
    let tienda = state
        .tiendas()?
        .de_vendedor(&usuario.user_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tienda", "del vendedor"))?;
    ok(tienda)
}

/// `GET /api/v2/empresarios/me/tiendas`.
pub async fn tiendas_de_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_EMPRESARIO, &[], &[]).await?;
    let tenant = usuario
        .tenant_id
        .as_deref()
        .ok_or_else(|| ApiError::no_encontrado("Empresa", "activa"))?;
    ok(Value::Array(state.tiendas()?.de_empresa(tenant).await?))
}

/// `GET /api/v2/tiendas/:id/tema.css` (público).
pub async fn tema_css(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let tienda = state
        .tiendas()?
        .encontrar_por_id(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tienda", &id))?;
    let tema = crate::tiendas::tema::tema_de(tienda.get("tema").unwrap_or(&Value::Null));
    let css = generar_tema_css(&tema);
    Ok((
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=60"),
        ],
        css,
    )
        .into_response())
}

/// `PATCH /api/v2/tiendas/:id/logo`.
pub async fn actualizar_logo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_TIENDA_GESTION, &[], &[]).await?;
    validar(validacion::ACTUALIZAR_LOGO, &cuerpo)?;
    let logo = cuerpo
        .get("logo_objeto_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    ok(state
        .tiendas()?
        .actualizar_logo(&usuario, &id, logo)
        .await?)
}

/// `PATCH /api/v2/tiendas/:id/background`.
pub async fn actualizar_background(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_TIENDA_GESTION, &[], &[]).await?;
    validar(validacion::ACTUALIZAR_BACKGROUND, &cuerpo)?;
    let datos = BackgroundInput::desde_json(&cuerpo);
    ok(state
        .tiendas()?
        .actualizar_background(&usuario, &id, &datos)
        .await?)
}

/// `PATCH /api/v2/tiendas/:id/personalizacion`.
pub async fn actualizar_personalizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_TIENDA_GESTION, &[], &[]).await?;
    validar(validacion::ACTUALIZAR_PERSONALIZACION, &cuerpo)?;
    let datos = PersonalizacionInput::desde_json(&cuerpo);
    ok(state
        .tiendas()?
        .actualizar_personalizacion(&usuario, &id, &datos)
        .await?)
}

/// `PATCH /api/v2/tiendas/:id/contenido`.
pub async fn actualizar_contenido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_TIENDA_GESTION, &[], &[]).await?;
    validar(validacion::ACTUALIZAR_CONTENIDO, &cuerpo)?;
    let datos = ContenidoInput::desde_json(&cuerpo);
    ok(state
        .tiendas()?
        .actualizar_contenido(&usuario, &id, &datos)
        .await?)
}

/// `PATCH /api/v2/tiendas/:id/slug`.
pub async fn actualizar_slug(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_VENDEDOR, &[], &[]).await?;
    validar(validacion::ACTUALIZAR_SLUG, &cuerpo)?;
    ok(state
        .tiendas()?
        .actualizar_slug(
            &usuario.user_id,
            &id,
            &cadena(&cuerpo, "slug").unwrap_or_default(),
        )
        .await?)
}

/// Proyección pública de la tienda (R-DS-3 / R-C-3).
#[derive(Debug, Serialize)]
pub struct TiendaPublica {
    pub id: String,
    pub nombre: String,
    pub slug: String,
    pub descripcion: String,
    pub logo_objeto_id: Option<String>,
    pub logo_url: Option<String>,
    pub background_objeto_id: Option<String>,
    pub background_media_type: Option<String>,
    pub background_poster_objeto_id: Option<String>,
    pub background_alt: String,
    pub background_overlay: i64,
    pub background_position: String,
    pub background_activo: bool,
    pub background_url: Option<String>,
    pub background_poster_url: Option<String>,
    pub background_movil_objeto_id: Option<String>,
    pub background_movil_media_type: Option<String>,
    pub background_movil_poster_objeto_id: Option<String>,
    pub background_movil_alt: String,
    pub background_movil_activo: bool,
    pub plantilla: String,
    pub tema: Value,
    pub hero_titulo: String,
    pub hero_subtitulo: String,
    pub hero_cta_texto: String,
    pub hero_cta_url: String,
    pub seo_titulo: String,
    pub seo_descripcion: String,
    pub seo_imagen_objeto_id: Option<String>,
    pub seo_imagen_url: Option<String>,
    pub secciones: Value,
    pub sobre_nosotros: String,
    pub politicas: Value,
    pub redes: Value,
    pub anuncio_texto: String,
    pub anuncio_enlace: String,
    pub anuncio_activo: bool,
    pub personalizacion_actualizada_en: Option<String>,
    pub contenido_actualizado_en: Option<String>,
    pub origen_personalizacion: Value,
    pub creado_en: String,
    pub actualizado_en: String,
}

fn cadena_valor(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn opcional(valor: &Value, clave: &str) -> Option<String> {
    valor.get(clave).and_then(Value::as_str).map(str::to_string)
}

/// Lista blanca explícita: un campo nuevo no se publica por accidente.
pub fn a_tienda_publica(tienda: &Value) -> TiendaPublica {
    TiendaPublica {
        id: cadena_valor(tienda, "id"),
        nombre: cadena_valor(tienda, "nombre"),
        slug: cadena_valor(tienda, "slug"),
        descripcion: cadena_valor(tienda, "descripcion"),
        logo_objeto_id: opcional(tienda, "logo_objeto_id"),
        logo_url: opcional(tienda, "logo_url"),
        background_objeto_id: opcional(tienda, "background_objeto_id"),
        background_media_type: opcional(tienda, "background_media_type"),
        background_poster_objeto_id: opcional(tienda, "background_poster_objeto_id"),
        background_alt: cadena_valor(tienda, "background_alt"),
        background_overlay: tienda
            .get("background_overlay")
            .and_then(Value::as_i64)
            .unwrap_or(35),
        background_position: cadena_valor(tienda, "background_position"),
        background_activo: tienda
            .get("background_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        background_url: opcional(tienda, "background_url"),
        background_poster_url: opcional(tienda, "background_poster_url"),
        background_movil_objeto_id: opcional(tienda, "background_movil_objeto_id"),
        background_movil_media_type: opcional(tienda, "background_movil_media_type"),
        background_movil_poster_objeto_id: opcional(tienda, "background_movil_poster_objeto_id"),
        background_movil_alt: cadena_valor(tienda, "background_movil_alt"),
        background_movil_activo: tienda
            .get("background_movil_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        plantilla: cadena_valor(tienda, "plantilla"),
        tema: tienda.get("tema").cloned().unwrap_or(Value::Null),
        hero_titulo: cadena_valor(tienda, "hero_titulo"),
        hero_subtitulo: cadena_valor(tienda, "hero_subtitulo"),
        hero_cta_texto: cadena_valor(tienda, "hero_cta_texto"),
        hero_cta_url: cadena_valor(tienda, "hero_cta_url"),
        seo_titulo: cadena_valor(tienda, "seo_titulo"),
        seo_descripcion: cadena_valor(tienda, "seo_descripcion"),
        seo_imagen_objeto_id: opcional(tienda, "seo_imagen_objeto_id"),
        seo_imagen_url: opcional(tienda, "seo_imagen_url"),
        secciones: tienda
            .get("secciones")
            .cloned()
            .unwrap_or_else(|| json!([])),
        sobre_nosotros: cadena_valor(tienda, "sobre_nosotros"),
        politicas: tienda
            .get("politicas")
            .cloned()
            .unwrap_or_else(|| json!({"envio": "", "devolucion": "", "privacidad": ""})),
        redes: tienda.get("redes").cloned().unwrap_or_else(|| json!([])),
        anuncio_texto: cadena_valor(tienda, "anuncio_texto"),
        anuncio_enlace: cadena_valor(tienda, "anuncio_enlace"),
        anuncio_activo: tienda
            .get("anuncio_activo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        personalizacion_actualizada_en: opcional(tienda, "personalizacion_actualizada_en"),
        contenido_actualizado_en: opcional(tienda, "contenido_actualizado_en"),
        origen_personalizacion: tienda
            .get("origen_personalizacion")
            .cloned()
            .unwrap_or(Value::Null),
        creado_en: cadena_valor(tienda, "creado_en"),
        actualizado_en: cadena_valor(tienda, "actualizado_en"),
    }
}

/// `GET /api/v2/tiendas/:id` (público, con 301 para slugs anteriores).
pub async fn tienda_publica(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let resuelta = state
        .tiendas()?
        .resolver(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tienda", &id))?;
    let ofertas = state
        .ofertas()?
        .de_tienda(
            resuelta
                .tienda
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .await?;
    let cuerpo = json!({
        "tienda": a_tienda_publica(&resuelta.tienda),
        "ofertas": ofertas
            .iter()
            .map(crate::ofertas::a_oferta_publica)
            .map(|oferta| serde_json::to_value(oferta).unwrap_or(Value::Null))
            .collect::<Vec<_>>(),
    });
    match resuelta.redirigir_a {
        Some(destino) => Ok((
            StatusCode::MOVED_PERMANENTLY,
            [(header::LOCATION, destino)],
            Json(cuerpo),
        )
            .into_response()),
        None => Ok(Json(cuerpo).into_response()),
    }
}
