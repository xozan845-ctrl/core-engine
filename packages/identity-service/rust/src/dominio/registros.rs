//! Registros genéricos por parte (historia clínica, recetas, expedientes).
//!
//! Réplica de `registros/registros.service.ts`: la capacidad (`recetas`,
//! `historia_clinica` o `registros`) decide el acceso; el rubro no.

use crate::auth::UsuarioContexto;
use crate::db::{iso_ms, Db};
use crate::dominio::capacidades::Capacidades;
use crate::dominio::slug::es_uuid;
use crate::errors::ApiError;
use axum::http::StatusCode;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

const ROLES_ACCESO: [&str; 7] = [
    "super_admin",
    "admin",
    "empresario",
    "vendedor",
    "coordinador",
    "supervisor",
    "operativo",
];
const MAX_ADJUNTOS: usize = 10;
const MAX_DATOS_BYTES: usize = 64 * 1024;

pub struct Registros {
    db: Arc<Db>,
    capacidades: Arc<Capacidades>,
}

impl Registros {
    pub fn nueva(db: Arc<Db>, capacidades: Arc<Capacidades>) -> Self {
        Self { db, capacidades }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        tipo: Option<&str>,
        limite: Option<i64>,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_acceso(empresa_id, parte_id, actor).await?;
        if let Some(tipo) = tipo {
            validar_tipo(tipo)?;
        }
        let tope = limite.unwrap_or(50).clamp(1, 100);
        let mut sql = String::from(
            "SELECT to_jsonb(r) AS datos FROM identity.partes_registros r
              WHERE r.empresa_id = $1::uuid AND r.parte_id = $2::uuid AND r.eliminado_en IS NULL",
        );
        if tipo.is_some() {
            sql.push_str(" AND r.tipo = $3");
        }
        sql.push_str(&format!(
            " ORDER BY r.creado_en DESC LIMIT ${}::int",
            if tipo.is_some() { 4 } else { 3 }
        ));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql).bind(empresa_id).bind(parte_id);
        if let Some(tipo) = tipo {
            consulta = consulta.bind(tipo);
        }
        let filas = consulta
            .bind(tope)
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

    pub async fn obtener(
        &self,
        empresa_id: &str,
        parte_id: &str,
        registro_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, parte_id, actor).await?;
        self.buscar(empresa_id, parte_id, registro_id).await
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, parte_id, actor).await?;
        let tipo = validar_tipo(campo(datos, "tipo"))?;
        self.exigir_capacidad(empresa_id, &tipo).await?;
        let titulo = titulo_de(datos)?;
        let datos_json = validar_datos(datos.get("datos"))?;
        let adjuntos = validar_adjuntos(datos.get("adjuntos"))?;
        let cita_id = validar_uuid_opcional(datos.get("cita_id"), "cita_id")?;
        let item_id = validar_uuid_opcional(datos.get("item_id"), "item_id")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.partes_registros AS r
               (empresa_id, parte_id, tipo, titulo, datos, adjuntos, cita_id, item_id, autor_id)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5::jsonb, $6::jsonb, $7::uuid, $8::uuid, $9::uuid)
             RETURNING to_jsonb(r) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(&tipo)
        .bind(titulo)
        .bind(datos_json)
        .bind(adjuntos)
        .bind(cita_id)
        .bind(item_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio("REGISTRO_NO_CREADO", "No se pudo guardar el registro.")
        })?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        registro_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, parte_id, actor).await?;
        let actual = self.buscar(empresa_id, parte_id, registro_id).await?;
        let tipo = match datos.get("tipo") {
            Some(_) => validar_tipo(campo(datos, "tipo"))?,
            None => texto(&actual, "tipo"),
        };
        if tipo != texto(&actual, "tipo") {
            self.exigir_capacidad(empresa_id, &tipo).await?;
        }
        let titulo = match datos.get("titulo") {
            Some(_) => titulo_de(datos)?,
            None => texto(&actual, "titulo"),
        };
        let datos_json = match datos.get("datos") {
            Some(_) => validar_datos(datos.get("datos"))?,
            None => actual.get("datos").cloned().unwrap_or_else(|| json!({})),
        };
        let adjuntos = match datos.get("adjuntos") {
            Some(_) => validar_adjuntos(datos.get("adjuntos"))?,
            None => actual.get("adjuntos").cloned().unwrap_or_else(|| json!([])),
        };
        let cita_id = match datos.get("cita_id") {
            Some(_) => validar_uuid_opcional(datos.get("cita_id"), "cita_id")?,
            None => actual
                .get("cita_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        let item_id = match datos.get("item_id") {
            Some(_) => validar_uuid_opcional(datos.get("item_id"), "item_id")?,
            None => actual
                .get("item_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes_registros AS r
                SET tipo = $1, titulo = $2, datos = $3::jsonb, adjuntos = $4::jsonb,
                    cita_id = $5::uuid, item_id = $6::uuid, actualizado_en = NOW()
              WHERE r.id = $7::uuid AND r.empresa_id = $8::uuid
                AND r.parte_id = $9::uuid AND r.eliminado_en IS NULL
              RETURNING to_jsonb(r) AS datos",
        )
        .bind(&tipo)
        .bind(titulo)
        .bind(datos_json)
        .bind(adjuntos)
        .bind(cita_id)
        .bind(item_id)
        .bind(registro_id)
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Registro", registro_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn eliminar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        registro_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, parte_id, actor).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes_registros
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND parte_id = $3::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(registro_id)
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Registro", registro_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    async fn buscar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        registro_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(registro_id) {
            return Err(ApiError::no_encontrado("Registro", registro_id));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(r) AS datos FROM identity.partes_registros r
              WHERE r.id = $1::uuid AND r.empresa_id = $2::uuid
                AND r.parte_id = $3::uuid AND r.eliminado_en IS NULL",
        )
        .bind(registro_id)
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Registro", registro_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    async fn validar_acceso(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        if !es_uuid(empresa_id) || !es_uuid(parte_id) {
            return Err(ApiError::dominio(
                "PARTE_INVALIDA",
                "La empresa o la parte no son validas.",
            ));
        }
        if !actor.es_admin_global() {
            if !ROLES_ACCESO.contains(&actor.rol.as_str()) {
                return Err(ApiError::acceso_denegado(
                    "El rol no tiene acceso a registros de partes.",
                ));
            }
            if actor.tenant_id.as_deref() != Some(empresa_id) {
                return Err(ApiError::acceso_denegado(
                    "Solo puedes operar registros de tu empresa.",
                ));
            }
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let parte = sqlx::query(
            "SELECT 1 FROM identity.partes
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(parte_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if parte.is_none() {
            return Err(ApiError::no_encontrado("Parte", parte_id));
        }
        Ok(())
    }

    async fn exigir_capacidad(&self, empresa_id: &str, tipo: &str) -> Result<(), ApiError> {
        let requerida = match tipo {
            "receta" => "recetas",
            "historia_clinica" => "historia_clinica",
            _ => "registros",
        };
        let filas = self
            .capacidades
            .activas_de_empresas(&[empresa_id.to_string()], &[requerida.to_string()])
            .await?;
        let activa = filas.iter().any(|fila| {
            fila.get("empresa_id").and_then(Value::as_str) == Some(empresa_id)
                && fila.get("activo").and_then(Value::as_bool) == Some(true)
        });
        if !activa {
            return Err(ApiError::nueva(
                "CAPACIDAD_NO_ACTIVA",
                format!("La empresa no tiene activa la capacidad {requerida}."),
                StatusCode::FORBIDDEN,
            )
            .con_detalles(json!({
                "empresa_id": empresa_id,
                "capacidades": [requerida],
            })));
        }
        Ok(())
    }
}

fn validar_tipo(valor: &str) -> Result<String, ApiError> {
    let tipo = valor.trim().to_lowercase();
    let mut chars = tipo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&tipo.len())
        && tipo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            "REGISTRO_TIPO_INVALIDO",
            "El tipo de registro no es valido.",
        ));
    }
    Ok(tipo)
}

fn titulo_de(datos: &Value) -> Result<String, ApiError> {
    match datos.get("titulo") {
        None | Some(Value::Null) => Ok(String::new()),
        Some(valor) => validar_titulo_estricto(valor),
    }
}

fn validar_titulo(valor: Option<&Value>) -> String {
    valor
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .chars()
        .take(200)
        .collect()
}

fn validar_titulo_estricto(valor: &Value) -> Result<String, ApiError> {
    let titulo = valor.as_str().unwrap_or("").trim();
    if titulo.chars().count() > 200 {
        return Err(ApiError::dominio(
            "REGISTRO_TITULO_INVALIDO",
            "El titulo admite hasta 200 caracteres.",
        ));
    }
    Ok(titulo.to_string())
}

fn validar_datos(valor: Option<&Value>) -> Result<Value, ApiError> {
    let valor = valor.cloned().unwrap_or_else(|| json!({}));
    if !valor.is_object() {
        return Err(ApiError::dominio(
            "REGISTRO_DATOS_INVALIDOS",
            "Los datos deben ser un objeto JSON.",
        ));
    }
    if serde_json::to_vec(&valor)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
        > MAX_DATOS_BYTES
    {
        return Err(ApiError::dominio(
            "REGISTRO_DATOS_INVALIDOS",
            "Los datos superan el limite de 64 KB.",
        ));
    }
    Ok(valor)
}

fn validar_adjuntos(valor: Option<&Value>) -> Result<Value, ApiError> {
    let valor = valor.cloned().unwrap_or_else(|| json!([]));
    let Some(lista) = valor.as_array() else {
        return Err(ApiError::dominio(
            "REGISTRO_ADJUNTOS_INVALIDOS",
            format!("Adjuntos: maximo {MAX_ADJUNTOS} objetos."),
        ));
    };
    if lista.len() > MAX_ADJUNTOS {
        return Err(ApiError::dominio(
            "REGISTRO_ADJUNTOS_INVALIDOS",
            format!("Adjuntos: maximo {MAX_ADJUNTOS} objetos."),
        ));
    }
    for adjunto in lista {
        let Some(adjunto) = adjunto.as_str() else {
            return Err(ApiError::dominio(
                "REGISTRO_ADJUNTOS_INVALIDOS",
                "Cada adjunto debe ser un UUID de storage.",
            ));
        };
        if !es_uuid(adjunto) {
            return Err(ApiError::dominio(
                "REGISTRO_ADJUNTOS_INVALIDOS",
                "Cada adjunto debe ser un UUID de storage.",
            ));
        }
    }
    Ok(valor)
}

fn validar_uuid_opcional(valor: Option<&Value>, campo: &str) -> Result<Option<String>, ApiError> {
    match valor {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(valor)) if valor.is_empty() => Ok(None),
        Some(Value::String(valor)) if es_uuid(valor) => Ok(Some(valor.clone())),
        Some(_) => Err(ApiError::dominio(
            "REGISTRO_REFERENCIA_INVALIDA",
            format!("{campo} debe ser un UUID."),
        )),
    }
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
    fn debe_validar_tipos_titulos_datos_y_adjuntos() {
        assert_eq!(
            validar_tipo("Historia_Clinica").expect("tipo"),
            "historia_clinica"
        );
        assert!(validar_tipo("1malo").is_err());
        assert_eq!(validar_titulo(Some(&json!("  Nota  "))), "Nota");
        assert!(validar_titulo_estricto(&json!("x".repeat(201))).is_err());
        assert!(validar_datos(Some(&json!({"ok": true}))).is_ok());
        assert!(validar_datos(Some(&json!([]))).is_err());
        assert!(validar_adjuntos(Some(&json!(["11111111-1111-4111-8111-111111111111"]))).is_ok());
        assert!(validar_adjuntos(Some(&json!(["no-uuid"]))).is_err());
    }

    #[test]
    fn debe_validar_referencias_opcionales() {
        assert_eq!(validar_uuid_opcional(None, "cita_id").expect("none"), None);
        assert_eq!(
            validar_uuid_opcional(
                Some(&json!("11111111-1111-4111-8111-111111111111")),
                "cita_id"
            )
            .expect("uuid"),
            Some("11111111-1111-4111-8111-111111111111".to_string())
        );
        assert!(validar_uuid_opcional(Some(&json!("x")), "cita_id").is_err());
    }
}
