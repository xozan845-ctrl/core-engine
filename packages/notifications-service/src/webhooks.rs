//! Webhooks salientes por tenant (ADR-0045).
//!
//! El consumidor de la cola `notifications.webhooks` recibe todos los eventos
//! del exchange, busca las suscripciones activas del tenant y registra una
//! entrega idempotente por `(webhook, event_id)`. Las entregas fallidas se
//! reintentan con backoff exponencial desde un worker periódico.

use crate::campos;
use crate::db::Db;
use crate::errors::ErrorNotificacion;
use crate::logs::{self, ContextoCorrelacion};
use crate::rabbit::Handler;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub const MAX_INTENTOS: i32 = 5;
const MAX_ENTREGAS_REINTENTO: i64 = 20;
const MAX_EVENTOS: usize = 50;

pub struct Webhooks {
    db: Arc<Db>,
    cliente: reqwest::Client,
    https_obligatorio: bool,
}

impl Webhooks {
    pub fn nueva(db: Arc<Db>, timeout_ms: u64, https_obligatorio: bool) -> Self {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms.max(1)))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            db,
            cliente,
            https_obligatorio,
        }
    }

    pub async fn listar(&self, empresa_id: &str) -> Result<Vec<Value>, ErrorNotificacion> {
        let filas = sqlx::query(
            "SELECT to_jsonb(w) - 'secreto' AS datos
               FROM notifications.webhooks w
              WHERE w.empresa_id = $1::uuid
              ORDER BY w.creado_en DESC",
        )
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(Into::into))
            .collect()
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        url: &str,
        descripcion: &str,
        eventos: Vec<String>,
        creado_por: &str,
    ) -> Result<Value, ErrorNotificacion> {
        self.validar_url(url)?;
        let eventos = normalizar_eventos(eventos)?;
        let secreto = nuevo_secreto();
        let fila = sqlx::query(
            "INSERT INTO notifications.webhooks
               (empresa_id, url, descripcion, eventos, secreto, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6::uuid)
             RETURNING to_jsonb(webhooks) AS datos",
        )
        .bind(empresa_id)
        .bind(url.trim())
        .bind(descripcion.trim())
        .bind(&eventos)
        .bind(&secreto)
        .bind(creado_por)
        .fetch_one(self.db.pool())
        .await?;
        let mut datos: Value = fila.try_get("datos")?;
        if let Some(objeto) = datos.as_object_mut() {
            objeto.insert("secreto".into(), Value::String(secreto));
        }
        Ok(datos)
    }

    pub async fn actualizar(
        &self,
        id: &str,
        empresa_id: &str,
        url: Option<&str>,
        descripcion: Option<&str>,
        eventos: Option<Vec<String>>,
        activo: Option<bool>,
    ) -> Result<Value, ErrorNotificacion> {
        if let Some(url) = url {
            self.validar_url(url)?;
        }
        let eventos = eventos.map(normalizar_eventos).transpose()?;
        let fila = sqlx::query(
            "UPDATE notifications.webhooks w
                SET url = COALESCE($3, w.url),
                    descripcion = COALESCE($4, w.descripcion),
                    eventos = COALESCE($5::text[], w.eventos),
                    activo = COALESCE($6, w.activo),
                    actualizado_en = NOW()
              WHERE w.id = $1::uuid AND w.empresa_id = $2::uuid
             RETURNING to_jsonb(w) - 'secreto' AS datos",
        )
        .bind(id)
        .bind(empresa_id)
        .bind(url.map(str::trim))
        .bind(descripcion.map(str::trim))
        .bind(eventos)
        .bind(activo)
        .fetch_optional(self.db.pool())
        .await?
        .ok_or_else(|| ErrorNotificacion::Evento("Webhook no encontrado.".to_string()))?;
        fila.try_get("datos").map_err(Into::into)
    }

    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ErrorNotificacion> {
        let resultado = sqlx::query(
            "DELETE FROM notifications.webhooks
              WHERE id = $1::uuid AND empresa_id = $2::uuid",
        )
        .bind(id)
        .bind(empresa_id)
        .execute(self.db.pool())
        .await?;
        if resultado.rows_affected() == 0 {
            return Err(ErrorNotificacion::Evento(
                "Webhook no encontrado.".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn rotar_secreto(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Value, ErrorNotificacion> {
        let secreto = nuevo_secreto();
        let fila = sqlx::query(
            "UPDATE notifications.webhooks
                SET secreto = $3, actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
             RETURNING id, secreto",
        )
        .bind(id)
        .bind(empresa_id)
        .bind(&secreto)
        .fetch_optional(self.db.pool())
        .await?
        .ok_or_else(|| ErrorNotificacion::Evento("Webhook no encontrado.".to_string()))?;
        Ok(json!({
            "id": fila.try_get::<Uuid, _>("id").map(|id| id.to_string())?,
            "secreto": secreto,
        }))
    }

    pub async fn entregas(
        &self,
        webhook_id: &str,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ErrorNotificacion> {
        let filas = sqlx::query(
            "SELECT to_jsonb(e) - 'payload' AS datos
               FROM notifications.webhook_entregas e
               JOIN notifications.webhooks w ON w.id = e.webhook_id
              WHERE e.webhook_id = $1::uuid AND w.empresa_id = $2::uuid
              ORDER BY e.creado_en DESC
              LIMIT 100",
        )
        .bind(webhook_id)
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(Into::into))
            .collect()
    }

    /// Exportación de tenant sin secretos ni payloads completos.
    pub async fn exportar(&self, empresa_id: &str) -> Result<Value, ErrorNotificacion> {
        let webhooks = self.listar(empresa_id).await?;
        let fila = sqlx::query(
            "SELECT COALESCE(jsonb_agg(to_jsonb(e) - 'payload' ORDER BY e.creado_en DESC), '[]'::jsonb) AS datos
               FROM notifications.webhook_entregas e
               JOIN notifications.webhooks w ON w.id = e.webhook_id
              WHERE w.empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(self.db.pool())
        .await?;
        let entregas: Value = fila.try_get("datos")?;
        Ok(json!({ "webhooks": webhooks, "entregas": entregas }))
    }

    /// Purga webhooks y entregas del tenant (cascade).
    pub async fn purgar(&self, empresa_id: &str) -> Result<u64, ErrorNotificacion> {
        let resultado =
            sqlx::query("DELETE FROM notifications.webhooks WHERE empresa_id = $1::uuid")
                .bind(empresa_id)
                .execute(self.db.pool())
                .await?;
        Ok(resultado.rows_affected())
    }

    /// Registra una entrega por webhook suscrito y la intenta de inmediato.
    pub async fn despachar(&self, evento: &Value) -> Result<(), ErrorNotificacion> {
        let Some(event_id) = evento
            .get("event_id")
            .and_then(Value::as_str)
            .and_then(|valor| Uuid::parse_str(valor).ok())
        else {
            logs::warn(
                "notifications.webhooks",
                campos! { "msg": "evento_sin_event_id" },
            );
            return Ok(());
        };
        let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
        if tipo.is_empty() {
            logs::warn(
                "notifications.webhooks",
                campos! { "msg": "evento_sin_tipo", "event_id": event_id.to_string() },
            );
            return Ok(());
        }
        let Some(empresa_id) = empresa_de(evento) else {
            logs::warn(
                "notifications.webhooks",
                campos! { "msg": "evento_sin_empresa", "event_id": event_id.to_string(), "tipo": tipo },
            );
            return Ok(());
        };
        let filas = sqlx::query(
            "SELECT id FROM notifications.webhooks
              WHERE empresa_id = $1::uuid AND activo
                AND (cardinality(eventos) = 0 OR $2 = ANY(eventos))",
        )
        .bind(&empresa_id)
        .bind(tipo)
        .fetch_all(self.db.pool())
        .await?;
        for fila in filas {
            let webhook_id: Uuid = fila.try_get("id")?;
            let entrega: Option<Uuid> = sqlx::query_scalar(
                "INSERT INTO notifications.webhook_entregas
                   (webhook_id, event_id, tipo, payload)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (webhook_id, event_id) DO NOTHING
                 RETURNING id",
            )
            .bind(webhook_id)
            .bind(event_id)
            .bind(tipo)
            .bind(evento)
            .fetch_optional(self.db.pool())
            .await?;
            if let Some(entrega) = entrega {
                self.intentar(&entrega.to_string()).await?;
            }
        }
        Ok(())
    }

    /// Intenta una entrega concreta y actualiza su estado.
    pub async fn intentar(&self, entrega_id: &str) -> Result<(), ErrorNotificacion> {
        let fila = sqlx::query(
            "SELECT e.payload, e.intentos, w.url, w.secreto
               FROM notifications.webhook_entregas e
               JOIN notifications.webhooks w ON w.id = e.webhook_id
              WHERE e.id = $1::uuid AND e.estado <> 'entregado'",
        )
        .bind(entrega_id)
        .fetch_optional(self.db.pool())
        .await?;
        let Some(fila) = fila else {
            return Ok(());
        };
        let payload: Value = fila.try_get("payload")?;
        let intentos: i32 = fila.try_get("intentos")?;
        let url: String = fila.try_get("url")?;
        let secreto: String = fila.try_get("secreto")?;
        let cuerpo = serde_json::to_vec(&payload).map_err(|error| {
            ErrorNotificacion::Evento(format!("No se pudo serializar el evento: {error}"))
        })?;
        let evento = payload.get("tipo").and_then(Value::as_str).unwrap_or("");
        let firma = firmar(&secreto, &cuerpo);
        let respuesta = self
            .cliente
            .post(&url)
            .header("x-core-event", evento)
            .header("x-core-delivery", entrega_id)
            .header("x-core-signature", firma)
            .header("content-type", "application/json")
            .body(cuerpo)
            .send()
            .await;
        let (estado, status, error) = match respuesta {
            Ok(respuesta) => {
                let status = respuesta.status().as_u16();
                if respuesta.status().is_success() {
                    ("entregado", Some(status), String::new())
                } else {
                    ("fallido", Some(status), format!("http {status}"))
                }
            }
            Err(error) => ("fallido", None, error.to_string()),
        };
        let siguiente = if estado == "entregado" || intentos + 1 >= MAX_INTENTOS {
            None
        } else {
            Some(backoff_segundos(intentos + 1))
        };
        sqlx::query(
            "UPDATE notifications.webhook_entregas
                SET estado = $2,
                    intentos = intentos + 1,
                    ultimo_error = $3,
                    respuesta_status = $4,
                    entregado_en = CASE WHEN $2 = 'entregado' THEN NOW() ELSE NULL END,
                    proximo_intento = CASE
                      WHEN $5::bigint IS NULL THEN NULL
                      ELSE NOW() + make_interval(secs => $5::double precision)
                    END
              WHERE id = $1::uuid",
        )
        .bind(entrega_id)
        .bind(estado)
        .bind(&error)
        .bind(status.map(i32::from))
        .bind(siguiente)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Reintenta una entrega concreta verificando el tenant del webhook.
    pub async fn reintentar_entrega(
        &self,
        entrega_id: &str,
        empresa_id: &str,
    ) -> Result<(), ErrorNotificacion> {
        let existe: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM notifications.webhook_entregas e
                  JOIN notifications.webhooks w ON w.id = e.webhook_id
                 WHERE e.id = $1::uuid AND w.empresa_id = $2::uuid
            )",
        )
        .bind(entrega_id)
        .bind(empresa_id)
        .fetch_one(self.db.pool())
        .await?;
        if !existe {
            return Err(ErrorNotificacion::Evento(
                "Entrega no encontrada.".to_string(),
            ));
        }
        self.intentar(entrega_id).await
    }

    /// Reintenta entregas fallidas vencidas (worker periódico).
    pub async fn reintentar_pendientes(&self) -> Result<usize, ErrorNotificacion> {
        let filas = sqlx::query(
            "SELECT id FROM notifications.webhook_entregas
              WHERE estado = 'fallido'
                AND intentos < $1
                AND (proximo_intento IS NULL OR proximo_intento <= NOW())
              ORDER BY proximo_intento
              LIMIT $2",
        )
        .bind(MAX_INTENTOS)
        .bind(MAX_ENTREGAS_REINTENTO)
        .fetch_all(self.db.pool())
        .await?;
        let mut intentos = 0usize;
        for fila in filas {
            let id: Uuid = fila.try_get("id")?;
            self.intentar(&id.to_string()).await?;
            intentos += 1;
        }
        Ok(intentos)
    }

    fn validar_url(&self, url: &str) -> Result<(), ErrorNotificacion> {
        validar_url(url, self.https_obligatorio)
    }
}

fn validar_url(url: &str, https_obligatorio: bool) -> Result<(), ErrorNotificacion> {
    let url = url.trim();
    if !(8..=500).contains(&url.chars().count()) {
        return Err(ErrorNotificacion::Evento(
            "La URL del webhook debe tener entre 8 y 500 caracteres.".to_string(),
        ));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| ErrorNotificacion::Evento("La URL del webhook no es válida.".to_string()))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(ErrorNotificacion::Evento(
            "La URL del webhook debe usar http o https.".to_string(),
        ));
    }
    if https_obligatorio && parsed.scheme() != "https" {
        return Err(ErrorNotificacion::Evento(
            "La URL del webhook debe usar https.".to_string(),
        ));
    }
    if parsed.host_str().is_none() || !parsed.username().is_empty() {
        return Err(ErrorNotificacion::Evento(
            "La URL del webhook no es válida.".to_string(),
        ));
    }
    Ok(())
}

fn normalizar_eventos(eventos: Vec<String>) -> Result<Vec<String>, ErrorNotificacion> {
    let mut normalizados = Vec::new();
    for evento in eventos {
        let evento = evento.trim().to_lowercase();
        if evento.is_empty() {
            continue;
        }
        if !(3..=80).contains(&evento.chars().count())
            || !evento.chars().all(|caracter| {
                caracter.is_ascii_alphanumeric() || matches!(caracter, '_' | '-' | '.')
            })
        {
            return Err(ErrorNotificacion::Evento(format!(
                "El evento \"{evento}\" no es válido."
            )));
        }
        if !normalizados.contains(&evento) {
            normalizados.push(evento);
        }
    }
    if normalizados.len() > MAX_EVENTOS {
        return Err(ErrorNotificacion::Evento(format!(
            "No se pueden suscribir más de {MAX_EVENTOS} eventos por webhook."
        )));
    }
    Ok(normalizados)
}

fn empresa_de(evento: &Value) -> Option<String> {
    for origen in [evento.get("data"), Some(evento)] {
        if let Some(empresa) = origen
            .and_then(|valor| valor.get("empresa_id"))
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
        {
            return Some(empresa.to_string());
        }
    }
    None
}

fn firmar(secreto: &str, cuerpo: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secreto.as_bytes()).expect("HMAC acepta cualquier clave");
    mac.update(cuerpo);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

fn nuevo_secreto() -> String {
    format!(
        "whsec_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn backoff_segundos(intentos: i32) -> i64 {
    let base = 30i64.saturating_mul(1i64 << intentos.clamp(0, 6));
    base.min(3600)
}

pub fn handler_webhooks(webhooks: Arc<Webhooks>) -> Handler {
    Arc::new(move |evento| {
        let webhooks = Arc::clone(&webhooks);
        Box::pin(async move {
            let request_id = evento
                .get("event_id")
                .and_then(Value::as_str)
                .map(|id| format!("notifications-webhook-{id}"))
                .unwrap_or_else(|| "notifications-webhook-desconocido".to_string());
            logs::ejecutar_con_contexto(contexto(request_id), async move {
                match webhooks.despachar(&evento).await {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        logs::error(
                            "notifications.webhooks",
                            campos! { "msg": "despacho fallido", "err": error.to_string() },
                        );
                        Err(error.to_string())
                    }
                }
            })
            .await
        })
    })
}

fn contexto(request_id: String) -> ContextoCorrelacion {
    ContextoCorrelacion {
        request_id,
        servicio: "notifications-webhooks".to_string(),
        usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
        rol: Some("super_admin".to_string()),
        tenant_id: None,
        negocio_id: None,
    }
}

/// Firma de referencia para clientes y pruebas.
pub fn firma_de(secreto: &str, cuerpo: &[u8]) -> String {
    firmar(secreto, cuerpo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_firma_es_estable_y_depende_del_cuerpo() {
        let firma = firma_de("whsec_prueba", br#"{"a":1}"#);
        assert!(firma.starts_with("sha256="));
        assert_eq!(firma, firma_de("whsec_prueba", br#"{"a":1}"#));
        assert_ne!(firma, firma_de("whsec_prueba", br#"{"a":2}"#));
        assert_ne!(firma, firma_de("otro", br#"{"a":1}"#));
    }

    #[test]
    fn el_backoff_crece_y_se_capa() {
        assert_eq!(backoff_segundos(1), 60);
        assert_eq!(backoff_segundos(2), 120);
        assert_eq!(backoff_segundos(10), 1920);
        assert_eq!(backoff_segundos(20), 1920);
    }

    #[test]
    fn normaliza_eventos_y_rechaza_invalidos() {
        let eventos = normalizar_eventos(vec![
            " Order.Created ".into(),
            "order.created".into(),
            "".into(),
            "payment.procesado".into(),
        ])
        .expect("eventos");
        assert_eq!(eventos, vec!["order.created", "payment.procesado"]);
        assert!(normalizar_eventos(vec!["no vale".into()]).is_err());
    }

    #[test]
    fn valida_esquema_host_y_https() {
        assert!(validar_url("https://erp.example.com/hook", true).is_ok());
        assert!(validar_url("http://erp.example.com/hook", true).is_err());
        assert!(validar_url("http://127.0.0.1:8080/hook", false).is_ok());
        assert!(validar_url("ftp://erp.example.com/hook", false).is_err());
        assert!(validar_url("https://user:pass@erp.example.com", false).is_err());
        assert!(validar_url("corto", false).is_err());
    }

    #[test]
    fn encuentra_la_empresa_en_data_o_en_la_raiz() {
        assert_eq!(
            empresa_de(&json!({ "data": { "empresa_id": "e-1" } })),
            Some("e-1".to_string())
        );
        assert_eq!(
            empresa_de(&json!({ "empresa_id": "e-2" })),
            Some("e-2".to_string())
        );
        assert_eq!(empresa_de(&json!({ "data": {} })), None);
    }
}
