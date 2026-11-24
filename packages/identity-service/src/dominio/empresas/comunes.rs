use super::prelude::*;
use super::Empresas;

impl Empresas {
    pub(super) fn con_urls(datos: &Value) -> Value {
        let mut salida = datos.as_object().cloned().unwrap_or_default();
        salida.remove("plantilla_override");
        salida.remove("tema_override");
        let background = background_de(datos);
        let con_urls = con_background_urls(background.clone());
        salida.insert(
            "background_objeto_id".into(),
            json!(con_urls.campos.background_objeto_id),
        );
        salida.insert(
            "background_media_type".into(),
            json!(con_urls.campos.background_media_type),
        );
        salida.insert(
            "background_poster_objeto_id".into(),
            json!(con_urls.campos.background_poster_objeto_id),
        );
        salida.insert(
            "background_alt".into(),
            json!(con_urls.campos.background_alt),
        );
        salida.insert(
            "background_overlay".into(),
            json!(con_urls.campos.background_overlay),
        );
        salida.insert(
            "background_position".into(),
            json!(con_urls.campos.background_position),
        );
        salida.insert(
            "background_activo".into(),
            json!(con_urls.campos.background_activo),
        );
        salida.insert("background_url".into(), json!(con_urls.background_url));
        salida.insert(
            "background_poster_url".into(),
            json!(con_urls.background_poster_url),
        );

        let overrides = HerenciaTema {
            plantilla: plantilla_de(datos.get("plantilla_override")),
            tema: tema_parcial_de(datos.get("tema_override").unwrap_or(&json!({}))),
        };
        let resuelta = resolver_personalizacion(
            &overrides,
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            "empresa",
        );
        salida.insert("plantilla".into(), json!(resuelta.plantilla));
        salida.insert(
            "tema".into(),
            serde_json::to_value(&resuelta.tema).unwrap_or_else(|_| json!({})),
        );
        salida.insert(
            "origen_personalizacion".into(),
            json!({
                "plantilla": resuelta.origen.plantilla,
                "tema": resuelta.origen.tema,
            }),
        );
        if let Some(logo) = datos.get("logo_objeto_id").and_then(Value::as_str) {
            salida.insert(
                "logo_url".into(),
                json!(format!("/api/v2/storage/public/objetos/{logo}")),
            );
        }
        for clave in ["creado_en", "actualizado_en"] {
            if let Some(valor) = datos.get(clave).and_then(Value::as_str) {
                if let Ok(fecha) = chrono::DateTime::parse_from_rfc3339(valor) {
                    salida.insert(
                        clave.into(),
                        json!(crate::db::iso_ms(fecha.with_timezone(&chrono::Utc))),
                    );
                }
            }
        }
        Value::Object(salida)
    }
}

/// Ejecuta una consulta que devuelve un `jsonb` agregado y recibe un único
/// parámetro `empresa_id`.
pub(super) async fn jsonb_por_empresa(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    empresa_id: &str,
) -> Result<Value, ApiError> {
    sqlx::query_scalar(sql)
        .bind(empresa_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(interno)
}

/// Suspender/cerrar corta el refresh de todos los usuarios del tenant; los
/// access tokens vigentes expiran en su TTL corto.
pub(super) async fn revocar_sesiones_tenant(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE identity.sesiones
            SET revocada_en = NOW()
          WHERE revocada_en IS NULL
            AND usuario_id IN (
                SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
            )",
    )
    .bind(empresa_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

pub(super) fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn es_unico(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

pub(super) fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

pub(super) trait IfEmpty {
    fn if_empty(self, respaldo: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, respaldo: &str) -> String {
        if self.is_empty() {
            respaldo.to_string()
        } else {
            self
        }
    }
}
