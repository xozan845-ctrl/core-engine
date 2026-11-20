//! Onboarding idempotente de un empresario autenticado (ADR-0047).
//!
//! Empaqueta el alta que ya hace `POST /empresas` y devuelve el estado, el
//! negocio principal, las capacidades y un access token con el tenant fresco.
//! No crea usuarios ni admins: el actor autenticado es el empresario.

use super::empresas::{capacidades_extra, CREAR, PERFIL};
use super::validacion::validar;
use super::Cuerpo;
use crate::auth::{crear_access_token, UsuarioContexto, UsuarioToken};
use crate::config::Config;
use crate::crypto::sha256_hex;
use crate::dominio::empresas::CrearEmpresa;
use crate::dominio::onboarding::{Inicio, Job};
use crate::dominio::slug::{slug_desde_nombre, validar_slug_solicitado};
use crate::dominio::usuarios::Usuario;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};
use uuid::Uuid;

const CLAVE_MIN: usize = 8;
const CLAVE_MAX: usize = 255;

/// POST /api/v2/onboarding
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["empresario"], &[])?;
    let clave = idempotency_key(&headers)?;

    if let Some(admin) = dto.get("admin").filter(|valor| !valor.is_null()) {
        let correo = admin.get("correo").and_then(Value::as_str).unwrap_or("");
        if !correo.trim().is_empty() {
            return Err(ApiError::dominio(
                "ADMIN_NO_SOPORTADO",
                "El onboarding usa al empresario autenticado. Invita al admin con el flujo de personal.",
            ));
        }
    }
    if dto.get("datos_demo").and_then(Value::as_bool) == Some(true) {
        return Err(ApiError::dominio(
            "DATOS_DEMO_NO_SOPORTADO",
            "Los datos demo todavía no están disponibles para onboarding.",
        ));
    }

    let empresa_dto = normalizar_empresa(&dto)?;
    validar(CREAR, &empresa_dto)?;
    validar(PERFIL, &empresa_dto)?;
    let capacidades = capacidades_extra(&empresa_dto)?;
    let nombre = texto(&empresa_dto, "nombre");
    let vertical = texto(&empresa_dto, "vertical_id");
    let pack = texto(&empresa_dto, "pack_codigo");
    let pack_version = empresa_dto
        .get("pack_version")
        .and_then(Value::as_i64)
        .and_then(|valor| i32::try_from(valor).ok());
    let slug = slug_del_onboarding(&empresa_dto, &dto, &nombre, &clave)?;
    let hash = hash_canonico(&dto)?;

    let servicios = state.servicios()?;
    let inicio = servicios
        .onboarding
        .iniciar(&actor.user_id, &clave, &hash, &slug)
        .await?;
    let (job, replayed) = match inicio {
        Inicio::Replay(job) => (job, true),
        Inicio::Nuevo(job) | Inicio::Reclamado(job) => (job, false),
    };
    if replayed {
        return respuesta(&state, &actor, &job, true, StatusCode::CREATED).await;
    }

    // El plan se exige antes de crear; un replay de un job completado no vuelve
    // a pasar por caja (su empresa ya existe).
    if let Err(error) = servicios.suscripciones.exigir_empresa_extra(&actor).await {
        let _ = servicios.onboarding.fallar(&job.id, &error.mensaje).await;
        return Err(error);
    }

    let empresa = match servicios
        .empresas
        .crear(CrearEmpresa {
            usuario_id: actor.user_id.clone(),
            nombre,
            slug: Some(job.slug.clone()),
            perfil: empresa_dto,
            vertical_id: Some(vertical),
            pack_codigo: Some(pack),
            pack_version,
            capacidades_extra: capacidades,
        })
        .await
    {
        Ok(empresa) => empresa,
        Err(error) => {
            // Si el commit anterior se perdió pero la empresa ya existe con el
            // mismo slug del job, se adopta en lugar de duplicar.
            if error.codigo == "CONFLICTO" {
                if let Some(empresa_id) = servicios
                    .onboarding
                    .empresa_id_por_slug(&actor.user_id, &job.slug)
                    .await?
                {
                    match servicios.empresas.obtener(&empresa_id).await? {
                        Some(empresa) => empresa,
                        None => {
                            let _ = servicios.onboarding.fallar(&job.id, &error.mensaje).await;
                            return Err(error);
                        }
                    }
                } else {
                    let _ = servicios.onboarding.fallar(&job.id, &error.mensaje).await;
                    return Err(error);
                }
            } else {
                let _ = servicios.onboarding.fallar(&job.id, &error.mensaje).await;
                return Err(error);
            }
        }
    };
    let empresa_id = empresa
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(ApiError::interno)?
        .to_string();
    let negocio_id = match servicios.onboarding.negocio_principal(&empresa_id).await {
        Ok(negocio_id) => negocio_id,
        Err(error) => {
            let _ = servicios.onboarding.fallar(&job.id, &error.mensaje).await;
            return Err(error.con_detalles(json!({
                "onboarding_id": job.id,
                "empresa_id": empresa_id,
            })));
        }
    };
    let capacidades = servicios.onboarding.capacidades(&empresa_id).await?;
    let resultado = json!({
        "empresa": empresa,
        "negocio_id": negocio_id,
        "capacidades": capacidades,
    });
    let job = match servicios
        .onboarding
        .completar(&job.id, &empresa_id, &negocio_id, resultado)
        .await
    {
        Ok(job) => job,
        Err(error) => {
            // La empresa ya existe: el job queda en proceso y el reintento la
            // adopta por slug. Se informa el id para que el frontend consulte.
            return Err(error.con_detalles(json!({
                "onboarding_id": job.id,
                "empresa_id": empresa_id,
            })));
        }
    };
    respuesta(&state, &actor, &job, false, StatusCode::CREATED).await
}

/// GET /api/v2/onboarding/:id
pub async fn estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["empresario"], &[])?;
    if Uuid::parse_str(&id).is_err() {
        return Err(ApiError::dominio(
            "ONBOARDING_INVALIDO",
            "El id de onboarding debe ser un UUID.",
        ));
    }
    let servicios = state.servicios()?;
    let job = servicios.onboarding.obtener(&actor.user_id, &id).await?;
    if job.estado == "completado" {
        return respuesta(&state, &actor, &job, false, StatusCode::OK).await;
    }
    Ok(Json(json!({
        "onboarding_id": job.id,
        "estado": job.estado,
        "paso": job.paso,
        "empresa_id": job.empresa_id,
        "negocio_id": job.negocio_id,
        "slug": job.slug,
        "error": job.error,
    }))
    .into_response())
}

fn normalizar_empresa(dto: &Value) -> Result<Value, ApiError> {
    let empresa = dto.get("empresa").cloned().unwrap_or(Value::Null);
    let mut objeto = empresa.as_object().cloned().ok_or_else(|| {
        ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "El cuerpo debe incluir el objeto empresa.",
        )
    })?;
    let vertical = dto
        .get("vertical")
        .or_else(|| dto.get("vertical_id"))
        .and_then(Value::as_str)
        .or_else(|| objeto.get("vertical_id").and_then(Value::as_str));
    let pack = dto
        .get("pack")
        .or_else(|| dto.get("pack_codigo"))
        .and_then(Value::as_str)
        .or_else(|| objeto.get("pack_codigo").and_then(Value::as_str));
    let vertical = vertical
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("VERTICAL_REQUERIDA", "Selecciona una vertical."))?;
    let pack = pack
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("PACK_REQUERIDO", "Selecciona un pack."))?;
    objeto.insert("vertical_id".into(), Value::String(vertical));
    objeto.insert("pack_codigo".into(), Value::String(pack));
    if let Some(version) = dto.get("pack_version") {
        objeto.insert("pack_version".into(), version.clone());
    }
    let mut capacidades = capacidades_extra(&Value::Object(objeto.clone()))?;
    for capacidad in capacidades_extra(dto)? {
        if !capacidades.contains(&capacidad) {
            capacidades.push(capacidad);
        }
    }
    objeto.insert("capacidades_extra".into(), json!(capacidades));
    Ok(Value::Object(objeto))
}

fn slug_del_onboarding(
    empresa: &Value,
    dto: &Value,
    nombre: &str,
    clave: &str,
) -> Result<String, ApiError> {
    let solicitado = empresa
        .get("slug")
        .and_then(Value::as_str)
        .or_else(|| dto.get("dominio_deseado").and_then(Value::as_str))
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    if let Some(slug) = solicitado {
        if slug.contains('.') || slug.contains('/') {
            return Err(ApiError::dominio(
                "DOMINIO_NO_SOPORTADO",
                "Los dominios personalizados no están implementados; envía un slug (por ejemplo, mi-salon).",
            ));
        }
        return validar_slug_solicitado(slug);
    }
    let sufijo = sha256_hex(clave.as_bytes());
    Ok(slug_desde_nombre(nombre, &sufijo[..8]))
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let clave = headers
        .get("idempotency-key")
        .and_then(|valor| valor.to_str().ok())
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::dominio(
                "IDEMPOTENCIA_REQUERIDA",
                "El encabezado Idempotency-Key es obligatorio para el onboarding.",
            )
        })?;
    if !(CLAVE_MIN..=CLAVE_MAX).contains(&clave.chars().count()) {
        return Err(ApiError::dominio(
            "IDEMPOTENCIA_INVALIDA",
            format!("Idempotency-Key debe tener entre {CLAVE_MIN} y {CLAVE_MAX} caracteres."),
        ));
    }
    Ok(clave.to_string())
}

fn hash_canonico(dto: &Value) -> Result<String, ApiError> {
    let canonico = canonizar(dto);
    let bytes = serde_json::to_vec(&canonico).map_err(|_| ApiError::interno())?;
    Ok(sha256_hex(&bytes))
}

fn canonizar(valor: &Value) -> Value {
    match valor {
        Value::Object(objeto) => {
            let mut claves: Vec<&String> = objeto.keys().collect();
            claves.sort();
            let mut salida = Map::new();
            for clave in claves {
                salida.insert(clave.clone(), canonizar(&objeto[clave]));
            }
            Value::Object(salida)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonizar).collect()),
        otro => otro.clone(),
    }
}

async fn respuesta(
    state: &AppState,
    actor: &UsuarioContexto,
    job: &Job,
    replayed: bool,
    status: StatusCode,
) -> Result<Response, ApiError> {
    let Some(empresa_id) = job.empresa_id.as_deref() else {
        return Err(ApiError::interno());
    };
    let servicios = state.servicios()?;
    let empresa = servicios
        .empresas
        .obtener(empresa_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
    let negocio_id = match job.negocio_id.clone() {
        Some(negocio_id) => negocio_id,
        None => servicios.onboarding.negocio_principal(empresa_id).await?,
    };
    let capacidades = servicios.onboarding.capacidades(empresa_id).await?;
    let usuario = servicios
        .usuarios
        .encontrar_por_id(&actor.user_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Usuario", &actor.user_id))?;
    let (access_token, expires_in) = token_fresco(&usuario, &state.config)?;
    let data = json!({
        "onboarding_id": job.id,
        "estado": job.estado,
        "empresa_id": empresa_id,
        "negocio_id": negocio_id,
        "slug": job.slug,
        "empresa": empresa,
        "capacidades": capacidades,
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": expires_in,
        "storage": { "bucket": Value::Null, "estado": "lazy" },
    });
    let mut response = (status, Json(data)).into_response();
    if replayed {
        response
            .headers_mut()
            .insert("idempotency-replayed", HeaderValue::from_static("true"));
    }
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("/api/v2/onboarding/{}", job.id))
            .map_err(|_| ApiError::interno())?,
    );
    Ok(response)
}

fn token_fresco(usuario: &Usuario, config: &Config) -> Result<(String, u64), ApiError> {
    let ttl = config.access_ttl_segundos();
    let token = UsuarioToken {
        id: usuario.id.clone(),
        email: usuario.correo.clone(),
        rol: usuario.rol.clone(),
        nombre: usuario.nombre.clone(),
        tenant_id: usuario.tenant_id.clone(),
        personal_id: usuario.personal_id.clone(),
        participacion_porcentaje: usuario.participacion_porcentaje,
        rol_empresa_id: usuario.rol_empresa_id.clone(),
        negocio_id: usuario.negocio_id.clone(),
        permisos: usuario.permisos.clone(),
    };
    let access = crear_access_token(&token, &config.jwt_secret, ttl)?;
    Ok((access, ttl))
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_canonizar_el_cuerpo_para_la_clave_de_idempotencia() {
        let a = json!({ "b": 1, "a": { "d": 2, "c": 3 } });
        let b = json!({ "a": { "c": 3, "d": 2 }, "b": 1 });
        assert_eq!(hash_canonico(&a).unwrap(), hash_canonico(&b).unwrap());
        assert_ne!(
            hash_canonico(&a).unwrap(),
            hash_canonico(&json!({ "a": 1 })).unwrap()
        );
    }

    #[test]
    fn debe_exigir_y_validar_la_clave_de_idempotencia() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            idempotency_key(&headers).unwrap_err().codigo,
            "IDEMPOTENCIA_REQUERIDA"
        );
        headers.insert("idempotency-key", "corta".parse().unwrap());
        assert_eq!(
            idempotency_key(&headers).unwrap_err().codigo,
            "IDEMPOTENCIA_INVALIDA"
        );
        headers.insert("idempotency-key", "clave-suficiente".parse().unwrap());
        assert_eq!(idempotency_key(&headers).unwrap(), "clave-suficiente");
    }

    #[test]
    fn debe_derivar_un_slug_estable_para_la_misma_clave() {
        let dto = json!({ "empresa": { "nombre": "Mi Salón" } });
        let empresa = normalizar_empresa(&json!({
            "vertical": "belleza",
            "pack": "pack_belleza",
            "empresa": { "nombre": "Mi Salón" }
        }))
        .expect("empresa");
        let slug_a = slug_del_onboarding(&empresa, &dto, "Mi Salón", "clave-uno").unwrap();
        let slug_b = slug_del_onboarding(&empresa, &dto, "Mi Salón", "clave-uno").unwrap();
        assert_eq!(slug_a, slug_b);
        assert!(!slug_a.contains("clave"));
    }

    #[test]
    fn debe_rechazar_un_dominio_personalizado() {
        let empresa = json!({ "nombre": "Mi Salón" });
        let dto = json!({ "dominio_deseado": "mi-salon.com" });
        assert_eq!(
            slug_del_onboarding(&empresa, &dto, "Mi Salón", "clave-uno")
                .unwrap_err()
                .codigo,
            "DOMINIO_NO_SOPORTADO"
        );
    }
}
