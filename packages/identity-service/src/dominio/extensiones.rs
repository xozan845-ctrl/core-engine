//! Extensiones configurables por empresa (ADR-0001/R-AR-5).
//!
//! Réplica de `extensiones/extensiones.service.ts`: versionado por código,
//! activación de una sola versión y valores validados contra JSON Schema.

use crate::auth::UsuarioContexto;
use crate::db::{iso_ms, Db};
use crate::dominio::slug::es_uuid;
use crate::errors::ApiError;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

const MAX_JSON_BYTES: usize = 64 * 1024;

pub struct Extensiones {
    db: Arc<Db>,
}

impl Extensiones {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.extensiones_empresa e
              WHERE e.empresa_id = $1::uuid
              ORDER BY e.recurso, e.codigo, e.version DESC",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let meta = normalizar_extension(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1 || ':' || $2))")
            .bind(empresa_id)
            .bind(texto(&meta, "codigo"))
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        let version: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1
               FROM identity.extensiones_empresa
              WHERE empresa_id = $1::uuid AND codigo = $2",
        )
        .bind(empresa_id)
        .bind(texto(&meta, "codigo"))
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        if meta.get("activo").and_then(Value::as_bool).unwrap_or(true) {
            sqlx::query(
                "UPDATE identity.extensiones_empresa
                    SET activo = false, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND codigo = $2 AND activo = true",
            )
            .bind(empresa_id)
            .bind(texto(&meta, "codigo"))
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        let fila = sqlx::query(
            "INSERT INTO identity.extensiones_empresa AS e
               (empresa_id, codigo, nombre, recurso, descripcion, esquema,
                formulario, reglas, version, activo, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb,
                     $9, $10, $11::uuid)
             RETURNING to_jsonb(e) AS datos",
        )
        .bind(empresa_id)
        .bind(texto(&meta, "codigo"))
        .bind(texto(&meta, "nombre"))
        .bind(texto(&meta, "recurso"))
        .bind(texto(&meta, "descripcion"))
        .bind(meta.get("esquema").cloned().unwrap_or_else(|| json!({})))
        .bind(meta.get("formulario").cloned().unwrap_or_else(|| json!({})))
        .bind(meta.get("reglas").cloned().unwrap_or_else(|| json!({})))
        .bind(version)
        .bind(meta.get("activo").and_then(Value::as_bool).unwrap_or(true))
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la extensión."))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn cambiar_estado(
        &self,
        empresa_id: &str,
        codigo: &str,
        activo: bool,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let codigo = normalizar_codigo(codigo)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.extensiones_empresa AS e
                SET activo = $3, actualizado_en = NOW()
              WHERE e.empresa_id = $1::uuid AND e.codigo = $2
                AND e.version = (
                  SELECT MAX(version) FROM identity.extensiones_empresa
                   WHERE empresa_id = $1::uuid AND codigo = $2
                )
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Extensión", &codigo))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn validar(
        &self,
        empresa_id: &str,
        codigo: &str,
        datos: &Value,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        let codigo = normalizar_codigo(codigo)?;
        let extension = self.extension_activa(empresa_id, &codigo).await?;
        if !datos.is_object() {
            return Ok(json!({
                "valido": false,
                "errores": [{"instancia": "", "mensaje": "Los datos deben ser un objeto JSON."}]
            }));
        }
        let esquema = extension
            .get("esquema")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let (valido, errores) = validar_json_schema(&esquema, datos)?;
        Ok(json!({ "valido": valido, "errores": errores }))
    }

    pub async fn guardar_valores(
        &self,
        empresa_id: &str,
        codigo: &str,
        recurso_id: &str,
        valores: &Value,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let recurso_uuid = validar_recurso_id(recurso_id)?;
        let codigo = normalizar_codigo(codigo)?;
        let extension = self.extension_activa(empresa_id, &codigo).await?;
        let esquema = extension
            .get("esquema")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let extension_id = texto(&extension, "id");
        let version = extension
            .get("version")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        validar_valores(&esquema, valores)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.extensiones_empresa_valores AS v
               (empresa_id, extension_id, recurso_id, valores, version_schema, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::jsonb, $5, $6::uuid)
             ON CONFLICT (empresa_id, extension_id, recurso_id) DO UPDATE
               SET valores = EXCLUDED.valores,
                   version_schema = EXCLUDED.version_schema,
                   actualizado_en = NOW()
             RETURNING to_jsonb(v) AS datos",
        )
        .bind(empresa_id)
        .bind(&extension_id)
        .bind(&recurso_uuid)
        .bind(valores)
        .bind(version)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::conflicto("No se pudieron guardar los valores de la extensión.")
        })?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn obtener_valores(
        &self,
        empresa_id: &str,
        codigo: &str,
        recurso_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Option<Value>, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        let recurso_uuid = validar_recurso_id(recurso_id)?;
        let codigo = normalizar_codigo(codigo)?;
        let extension = self.extension_activa(empresa_id, &codigo).await?;
        let extension_id = texto(&extension, "id");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(v) AS datos FROM identity.extensiones_empresa_valores v
              WHERE v.empresa_id = $1::uuid AND v.extension_id = $2::uuid
                AND v.recurso_id = $3::uuid",
        )
        .bind(empresa_id)
        .bind(&extension_id)
        .bind(&recurso_uuid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| {
            let datos: Value = fila.try_get("datos").map_err(interno)?;
            Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
        })
        .transpose()
    }

    pub async fn eliminar_valores(
        &self,
        empresa_id: &str,
        codigo: &str,
        recurso_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let recurso_uuid = validar_recurso_id(recurso_id)?;
        let codigo = normalizar_codigo(codigo)?;
        let extension = self.extension_activa(empresa_id, &codigo).await?;
        let extension_id = texto(&extension, "id");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "DELETE FROM identity.extensiones_empresa_valores
              WHERE empresa_id = $1::uuid AND extension_id = $2::uuid
                AND recurso_id = $3::uuid
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(&extension_id)
        .bind(&recurso_uuid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Valores de extensión", &recurso_uuid))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    async fn extension_activa(&self, empresa_id: &str, codigo: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.extensiones_empresa e
              WHERE e.empresa_id = $1::uuid AND e.codigo = $2 AND e.activo = true
              ORDER BY e.version DESC LIMIT 1",
        )
        .bind(empresa_id)
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Extensión", codigo))?;
        tx.commit().await.map_err(interno)?;
        fila.try_get("datos").map_err(interno)
    }

    async fn validar_acceso(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        escritura: bool,
    ) -> Result<(), ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::dominio(
                "EMPRESA_INVALIDA",
                "La empresa debe ser un UUID válido.",
            ));
        }
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No puedes operar extensiones de otra empresa.",
            ));
        }
        if escritura && actor.rol != "empresario" {
            return Err(ApiError::acceso_denegado(
                "Solo el empresario puede modificar extensiones.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let miembro = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal
              WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::acceso_denegado(
                "El usuario no es miembro activo de esta empresa.",
            ));
        }
        Ok(())
    }
}

fn normalizar_extension(datos: &Value) -> Result<Value, ApiError> {
    let codigo = normalizar_codigo(campo(datos, "codigo"))?;
    let recurso = normalizar_codigo(campo(datos, "recurso"))?;
    let nombre = campo(datos, "nombre").trim();
    if !(2..=120).contains(&nombre.chars().count()) {
        return Err(ApiError::dominio(
            "NOMBRE_EXTENSION_INVALIDO",
            "El nombre debe tener entre 2 y 120 caracteres.",
        ));
    }
    let descripcion = campo(datos, "descripcion").trim();
    if descripcion.chars().count() > 1000 {
        return Err(ApiError::dominio(
            "DESCRIPCION_EXTENSION_INVALIDA",
            "La descripción no puede superar 1000 caracteres.",
        ));
    }
    let esquema = objeto(datos.get("esquema"), "ESQUEMA_EXTENSION_INVALIDO")?.ok_or_else(|| {
        ApiError::dominio(
            "ESQUEMA_EXTENSION_INVALIDO",
            "El esquema JSON es obligatorio.",
        )
    })?;
    compilar_esquema(&esquema)?;
    let formulario = objeto(datos.get("formulario"), "FORMULARIO_EXTENSION_INVALIDO")?
        .unwrap_or_else(|| json!({}));
    let reglas =
        objeto(datos.get("reglas"), "REGLAS_EXTENSION_INVALIDAS")?.unwrap_or_else(|| json!({}));
    Ok(json!({
        "codigo": codigo,
        "nombre": nombre,
        "recurso": recurso,
        "descripcion": descripcion,
        "esquema": esquema,
        "formulario": formulario,
        "reglas": reglas,
        "activo": datos.get("activo").and_then(Value::as_bool).unwrap_or(true),
    }))
}

fn objeto(valor: Option<&Value>, codigo: &str) -> Result<Option<Value>, ApiError> {
    let Some(valor) = valor else {
        return Ok(None);
    };
    if !valor.is_object() {
        return Err(ApiError::dominio(
            codigo,
            "El valor debe ser un objeto JSON.",
        ));
    }
    if serde_json::to_vec(valor)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
        > MAX_JSON_BYTES
    {
        return Err(ApiError::dominio(
            codigo,
            format!("El objeto no puede superar {MAX_JSON_BYTES} bytes."),
        ));
    }
    if contiene_ref(valor) {
        return Err(ApiError::dominio(
            codigo,
            "Las referencias externas ($ref) no están permitidas.",
        ));
    }
    Ok(Some(valor.clone()))
}

fn contiene_ref(valor: &Value) -> bool {
    match valor {
        Value::Array(items) => items.iter().any(contiene_ref),
        Value::Object(objeto) => objeto
            .iter()
            .any(|(clave, item)| clave == "$ref" || contiene_ref(item)),
        _ => false,
    }
}

fn compilar_esquema(esquema: &Value) -> Result<(), ApiError> {
    jsonschema::validator_for(esquema)
        .map(|_| ())
        .map_err(|error| {
            ApiError::dominio(
                "ESQUEMA_EXTENSION_INVALIDO",
                "El esquema JSON no es válido.",
            )
            .con_detalles(json!({ "mensaje": error.to_string() }))
        })
}

fn validar_valores(esquema: &Value, valores: &Value) -> Result<(), ApiError> {
    if !valores.is_object() {
        return Err(ApiError::dominio(
            "VALORES_EXTENSION_INVALIDOS",
            "Los valores deben ser un objeto JSON.",
        ));
    }
    if serde_json::to_vec(valores)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
        > MAX_JSON_BYTES
    {
        return Err(ApiError::dominio(
            "VALORES_EXTENSION_INVALIDOS",
            format!("Los valores no pueden superar {MAX_JSON_BYTES} bytes."),
        ));
    }
    let (valido, errores) = validar_json_schema(esquema, valores)?;
    if !valido {
        return Err(ApiError::dominio(
            "VALORES_EXTENSION_INVALIDOS",
            "Los valores no cumplen el JSON Schema de la extensión.",
        )
        .con_detalles(json!({ "errores": errores })));
    }
    Ok(())
}

fn validar_json_schema(esquema: &Value, datos: &Value) -> Result<(bool, Value), ApiError> {
    let validador = jsonschema::validator_for(esquema).map_err(|error| {
        ApiError::dominio(
            "ESQUEMA_EXTENSION_INVALIDO",
            "El esquema JSON no es válido.",
        )
        .con_detalles(json!({ "mensaje": error.to_string() }))
    })?;
    let errores: Vec<Value> = validador
        .iter_errors(datos)
        .map(|error| {
            json!({
                "instancia": error.instance_path().to_string(),
                "parametro": {},
                "mensaje": error.to_string(),
            })
        })
        .collect();
    Ok((errores.is_empty(), Value::Array(errores)))
}

fn normalizar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_EXTENSION_INVALIDO",
            "El código solo admite minúsculas, números y guion bajo (2-40).",
        ));
    }
    Ok(codigo)
}

fn validar_recurso_id(valor: &str) -> Result<String, ApiError> {
    if !es_uuid(valor.trim()) {
        return Err(ApiError::dominio(
            "RECURSO_INVALIDO",
            "El recurso debe ser un UUID válido.",
        ));
    }
    Ok(valor.trim().to_string())
}

fn campo<'a>(valor: &'a Value, clave: &str) -> &'a str {
    valor.get(clave).and_then(Value::as_str).unwrap_or("")
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn normalizar_fechas(mut datos: Value, claves: &[&str]) -> Value {
    if let Some(objeto) = datos.as_object_mut() {
        for clave in claves {
            if let Some(valor) = objeto.get(*clave).and_then(Value::as_str) {
                if let Ok(fecha) = chrono::DateTime::parse_from_rfc3339(valor) {
                    objeto.insert(
                        (*clave).to_string(),
                        json!(iso_ms(fecha.with_timezone(&Utc))),
                    );
                }
            }
        }
    }
    datos
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_esquemas_y_rechazar_refs() {
        let schema = json!({
            "type": "object",
            "properties": {"edad": {"type": "integer", "minimum": 0}},
            "required": ["edad"]
        });
        compilar_esquema(&schema).expect("schema");
        assert!(!contiene_ref(&schema));
        assert!(contiene_ref(&json!({"$ref": "https://x"})));
        let meta = normalizar_extension(&json!({
            "codigo": "ficha_cliente",
            "nombre": "Ficha",
            "recurso": "cliente",
            "esquema": schema
        }))
        .expect("extension");
        assert_eq!(meta["activo"], true);
    }

    #[test]
    fn debe_validar_valores_contra_el_esquema() {
        let schema = json!({"type": "object", "required": ["nombre"]});
        let (valido, errores) =
            validar_json_schema(&schema, &json!({"nombre": "Ana"})).expect("ok");
        assert!(valido);
        assert!(errores.as_array().expect("errores").is_empty());
        let (valido, errores) = validar_json_schema(&schema, &json!({})).expect("invalido");
        assert!(!valido);
        assert!(!errores.as_array().expect("errores").is_empty());
        assert!(validar_valores(&schema, &json!([])).is_err());
    }

    #[test]
    fn debe_normalizar_codigos_y_recursos() {
        assert_eq!(
            normalizar_codigo("Ficha_Cliente").expect("codigo"),
            "ficha_cliente"
        );
        assert!(normalizar_codigo("1malo").is_err());
        assert!(validar_recurso_id("11111111-1111-4111-8111-111111111111").is_ok());
        assert!(validar_recurso_id("x").is_err());
    }
}
