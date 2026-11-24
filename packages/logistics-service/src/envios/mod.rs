//! Dominio de logística: guías de despacho, asignación de flota y transiciones.
//!
//! Réplica de `envios.service.ts` con el mismo SQL, mensajes, códigos y
//! formato de montos. La guía se genera idempotentemente por `(order_id,
//! empresa_id)` y la flota se deriva de las capacidades activas de identity.

use crate::auth::Capacidades;
use crate::clients::{OrdenExterna, Orders};
use crate::db::{timestamp, timestamp_opcional, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::logs;
use crate::money::centavos_a_texto;
use crate::outbox::Outbox;
use async_trait::async_trait;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub const ESTADOS_ENVIO: [&str; 3] = ["enviada", "entregada", "cancelada"];
pub const ESTADOS_ORDEN: [&str; 5] = [
    "en_preparacion",
    "enviada",
    "entregada",
    "cancelada",
    "devuelta",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Envio {
    pub id: String,
    pub guia: String,
    pub order_id: String,
    pub monto_cents: i64,
    pub estado: String,
    pub creado_en: String,
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub proveedor_tipo: Option<String>,
    pub proveedor_tenant_id: Option<String>,
    pub parte_id: Option<String>,
    pub pod_receptor: Option<String>,
    pub pod_notas: Option<String>,
    pub pod_foto_objeto_id: Option<String>,
    pub pod_firma_objeto_id: Option<String>,
    pub pod_capturado_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvioConMonto {
    #[serde(flatten)]
    pub envio: Envio,
    pub monto: String,
}

impl From<Envio> for EnvioConMonto {
    fn from(envio: Envio) -> Self {
        Self {
            monto: centavos_a_texto(envio.monto_cents),
            envio,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Proveedor {
    tipo: Option<String>,
    tenant_id: Option<String>,
}

pub struct Envios {
    db: Arc<Db>,
    orders: Arc<dyn Orders>,
    capacidades: Arc<dyn Capacidades>,
    tenant_plataforma: String,
    outbox: Outbox,
}

mod preparacion;
mod seguimiento;

impl Envios {
    pub fn nueva(
        db: Arc<Db>,
        orders: Arc<dyn Orders>,
        capacidades: Arc<dyn Capacidades>,
        tenant_plataforma: impl Into<String>,
        outbox_tabla: impl Into<String>,
    ) -> Self {
        Self {
            db,
            orders,
            capacidades,
            tenant_plataforma: tenant_plataforma.into(),
            outbox: Outbox::nueva(outbox_tabla),
        }
    }

    pub fn outbox(&self) -> &Outbox {
        &self.outbox
    }
}

pub fn estados_aceptados(estado: &str) -> Vec<String> {
    if estado == "enviada" {
        vec!["enviada".to_string(), "entregada".to_string()]
    } else {
        vec![estado.to_string()]
    }
}

#[async_trait]
pub trait EnviosPort: Send + Sync {
    async fn preparar_envio(
        &self,
        order_id: &str,
        empresa_id: &str,
        negocio_id: Option<&str>,
        monto_cents: i64,
        parte_id: Option<&str>,
    ) -> Result<Envio, ApiError>;

    async fn preparar_envios_de_orden(&self, order_id: &str) -> Result<Vec<Envio>, ApiError>;

    async fn asignar_proveedor(
        &self,
        envio_id: &str,
        empresa_id: &str,
        tipo: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError>;

    async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<Envio>, ApiError>;

    async fn avanzar_estado_envio(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError>;

    async fn registrar_pod(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        receptor: &str,
        foto_objeto_id: Option<&str>,
        firma_objeto_id: Option<&str>,
        notas: Option<&str>,
    ) -> Result<Envio, ApiError>;

    async fn de_orden(&self, order_id: &str) -> Result<Option<Envio>, ApiError>;

    async fn listar(&self) -> Result<Vec<EnvioConMonto>, ApiError>;

    async fn avanzar_estado_orden(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        empresa_id: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<OrdenExterna, ApiError>;

    async fn consultar_orden(&self, order_id: &str) -> Result<Option<OrdenExterna>, ApiError>;
}

#[async_trait]

impl EnviosPort for Envios {
    async fn preparar_envio(
        &self,
        order_id: &str,
        empresa_id: &str,
        negocio_id: Option<&str>,
        monto_cents: i64,
        parte_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Envios::preparar_envio(
            self,
            order_id,
            empresa_id,
            negocio_id,
            monto_cents,
            parte_id,
        )
        .await
    }

    async fn preparar_envios_de_orden(&self, order_id: &str) -> Result<Vec<Envio>, ApiError> {
        Envios::preparar_envios_de_orden(self, order_id).await
    }

    async fn asignar_proveedor(
        &self,
        envio_id: &str,
        empresa_id: &str,
        tipo: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Envios::asignar_proveedor(self, envio_id, empresa_id, tipo, actor_usuario_id).await
    }

    async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<Envio>, ApiError> {
        Envios::de_empresa(self, empresa_id).await
    }

    async fn avanzar_estado_envio(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Envios::avanzar_estado_envio(
            self,
            envio_id,
            proveedor_tenant_id,
            estado,
            motivo,
            actor_usuario_id,
        )
        .await
    }

    async fn registrar_pod(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        receptor: &str,
        foto_objeto_id: Option<&str>,
        firma_objeto_id: Option<&str>,
        notas: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Envios::registrar_pod(
            self,
            envio_id,
            proveedor_tenant_id,
            receptor,
            foto_objeto_id,
            firma_objeto_id,
            notas,
        )
        .await
    }

    async fn de_orden(&self, order_id: &str) -> Result<Option<Envio>, ApiError> {
        Envios::de_orden(self, order_id).await
    }

    async fn listar(&self) -> Result<Vec<EnvioConMonto>, ApiError> {
        Envios::listar(self).await
    }

    async fn avanzar_estado_orden(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        empresa_id: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        Envios::avanzar_estado_orden(
            self,
            order_id,
            estado_nuevo,
            motivo,
            empresa_id,
            actor_usuario_id,
        )
        .await
    }

    async fn consultar_orden(&self, order_id: &str) -> Result<Option<OrdenExterna>, ApiError> {
        Envios::consultar_orden(self, order_id).await
    }
}

/// Implementación para tests y arranque sin PostgreSQL.
pub struct EnviosNoDisponible;

#[async_trait]
impl EnviosPort for EnviosNoDisponible {
    async fn preparar_envio(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: i64,
        _: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn preparar_envios_de_orden(&self, _: &str) -> Result<Vec<Envio>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn asignar_proveedor(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn de_empresa(&self, _: &str) -> Result<Vec<Envio>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn avanzar_estado_envio(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn registrar_pod(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn de_orden(&self, _: &str) -> Result<Option<Envio>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn listar(&self) -> Result<Vec<EnvioConMonto>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn avanzar_estado_orden(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn consultar_orden(&self, _: &str) -> Result<Option<OrdenExterna>, ApiError> {
        Ok(None)
    }
}

fn fila_envio(fila: &sqlx::postgres::PgRow) -> Result<Envio, ApiError> {
    Ok(Envio {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        guia: fila.try_get::<String, _>("guia").map_err(error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(error_db)?,
        monto_cents: fila
            .try_get::<i32, _>("monto_cents")
            .map(i64::from)
            .map_err(error_db)?,
        estado: fila.try_get::<String, _>("estado").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        empresa_id: uuid_opcional(fila, "empresa_id")
            .map_err(error_db)?
            .unwrap_or_default(),
        negocio_id: uuid_opcional(fila, "negocio_id").map_err(error_db)?,
        proveedor_tipo: fila
            .try_get::<Option<String>, _>("proveedor_tipo")
            .map_err(error_db)?,
        proveedor_tenant_id: uuid_opcional(fila, "proveedor_tenant_id").map_err(error_db)?,
        parte_id: uuid_opcional(fila, "parte_id").map_err(error_db)?,
        pod_receptor: fila
            .try_get::<Option<String>, _>("pod_receptor")
            .map_err(error_db)?,
        pod_notas: fila
            .try_get::<Option<String>, _>("pod_notas")
            .map_err(error_db)?,
        pod_foto_objeto_id: uuid_opcional(fila, "pod_foto_objeto_id").map_err(error_db)?,
        pod_firma_objeto_id: uuid_opcional(fila, "pod_firma_objeto_id").map_err(error_db)?,
        pod_capturado_en: timestamp_opcional(fila, "pod_capturado_en").map_err(error_db)?,
    })
}

pub fn error_db(error: sqlx::Error) -> ApiError {
    logs::error(
        "envios",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::interno())
}

/// Guía `BGH-<base36(millis)>-<6 uuid>` (mismo formato que el TypeScript).
fn generar_guia() -> String {
    let millis = Utc::now().timestamp_millis().max(0) as u128;
    let uuid = Uuid::new_v4().to_string();
    format!("BGH-{}-{}", base36(millis), uuid[..6].to_uppercase())
}

fn base36(mut valor: u128) -> String {
    const DIGITOS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if valor == 0 {
        return "0".to_string();
    }
    let mut salida = Vec::new();
    while valor > 0 {
        salida.push(DIGITOS[(valor % 36) as usize]);
        valor /= 36;
    }
    salida.reverse();
    String::from_utf8(salida).expect("base36 ascii")
}

pub fn validar_estado_orden(estado: &str) -> Result<(), ApiError> {
    if ESTADOS_ORDEN.contains(&estado) {
        Ok(())
    } else {
        Err(ApiError::dominio(
            "ESTADO_INVALIDO",
            "Estado objetivo invalido para logistica.",
        ))
    }
}

pub fn validar_estado_envio(estado: &str) -> Result<(), ApiError> {
    if ESTADOS_ENVIO.contains(&estado) {
        Ok(())
    } else {
        Err(ApiError::validacion(json!([
            {"campo": "estado", "restricciones": ["isIn"]}
        ])))
    }
}

/// Mismo shape que el `EnvioResponseDto` de la API.
pub fn envio_json(envio: &Envio) -> Value {
    serde_json::to_value(envio).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_generar_la_guia_con_el_formato_del_contrato() {
        let guia = generar_guia();
        let partes: Vec<&str> = guia.split('-').collect();
        assert_eq!(partes.len(), 3);
        assert_eq!(partes[0], "BGH");
        assert!(!partes[1].is_empty());
        assert_eq!(partes[2].len(), 6);
    }

    #[test]
    fn debe_codificar_en_base36_como_javascript() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "Z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1_760_000_000_000), "MGJ6K3CW");
    }

    #[test]
    fn debe_considerar_enviada_o_entregada_para_despacho() {
        assert_eq!(estados_aceptados("enviada"), vec!["enviada", "entregada"]);
        assert_eq!(estados_aceptados("entregada"), vec!["entregada"]);
        assert_eq!(estados_aceptados("cancelada"), vec!["cancelada"]);
    }

    #[test]
    fn debe_validar_los_estados_objetivo() {
        assert!(validar_estado_orden("devuelta").is_ok());
        assert_eq!(
            validar_estado_orden("creada").expect_err("invalido").codigo,
            "ESTADO_INVALIDO"
        );
        assert!(validar_estado_envio("cancelada").is_ok());
        assert_eq!(
            validar_estado_envio("creada").expect_err("invalido").codigo,
            "VALIDACION"
        );
    }

    #[test]
    fn debe_serializar_el_envio_con_el_mismo_shape() {
        let envio = Envio {
            id: "e-1".into(),
            guia: "BGH-ABC-123456".into(),
            order_id: "o-1".into(),
            monto_cents: 115_000,
            estado: "en_preparacion".into(),
            creado_en: "2026-09-01T00:00:00.000Z".into(),
            empresa_id: "emp-1".into(),
            negocio_id: None,
            proveedor_tipo: Some("empresa".into()),
            proveedor_tenant_id: Some("emp-1".into()),
            parte_id: None,
            pod_receptor: None,
            pod_notas: None,
            pod_foto_objeto_id: None,
            pod_firma_objeto_id: None,
            pod_capturado_en: None,
        };
        let valor = envio_json(&envio);
        assert_eq!(valor["guia"], "BGH-ABC-123456");
        assert_eq!(valor["parte_id"], Value::Null);
        assert_eq!(valor["monto_cents"], 115_000);
        let con_monto = EnvioConMonto::from(envio);
        assert_eq!(con_monto.monto, "1150.00");
    }

    #[test]
    fn debe_mapear_el_uuid_invalido_a_500_como_el_driver() {
        assert_eq!(
            parsear_uuid("no-es-uuid").expect_err("500").status,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
