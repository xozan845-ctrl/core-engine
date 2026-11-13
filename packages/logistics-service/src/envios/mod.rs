//! Dominio de logística: guías de despacho, asignación de flota y transiciones.
//!
//! Réplica de `envios.service.ts` con el mismo SQL, mensajes, códigos y
//! formato de montos. La guía se genera idempotentemente por `(order_id,
//! empresa_id)` y la flota se deriva de las capacidades activas de identity.

use crate::auth::Capacidades;
use crate::clients::{OrdenExterna, Orders};
use crate::db::{timestamp, uuid_opcional, uuid_texto, Db};
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
    pub proveedor_tipo: Option<String>,
    pub proveedor_tenant_id: Option<String>,
    pub parte_id: Option<String>,
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

    /// Prepara la guía de despacho cuando el pago fue procesado
    /// (`payment.procesado`). Idempotente por `(order_id, empresa_id)`.
    pub async fn preparar_envio(
        &self,
        order_id: &str,
        empresa_id: &str,
        monto_cents: i64,
        parte_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let empresa_uuid = parsear_uuid(empresa_id)?;

        let existente =
            sqlx::query("SELECT * FROM logistics.envios WHERE order_id = $1 AND empresa_id = $2")
                .bind(order_uuid)
                .bind(empresa_uuid)
                .fetch_optional(self.db.pool())
                .await
                .map_err(error_db)?;

        if let Some(fila) = existente {
            let envio = fila_envio(&fila)?;
            if let (Some(tipo), Some(tenant)) = (
                envio.proveedor_tipo.as_deref(),
                envio.proveedor_tenant_id.as_deref(),
            ) {
                self.registrar_asignacion(&envio.id, None, tipo, tenant, "automatica", None)
                    .await?;
            }
            return Ok(envio);
        }

        let guia = generar_guia();
        let proveedor = self.proveedor_de_empresa(empresa_id).await?;
        let parte_uuid = match parte_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };

        let fila = sqlx::query(
            "INSERT INTO logistics.envios
                (id, guia, order_id, empresa_id, parte_id, proveedor_tipo,
                 proveedor_tenant_id, monto_cents, estado)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'en_preparacion') RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(&guia)
        .bind(order_uuid)
        .bind(empresa_uuid)
        .bind(parte_uuid)
        .bind(proveedor.tipo.as_deref())
        .bind(
            proveedor
                .tenant_id
                .as_deref()
                .map(parsear_uuid)
                .transpose()?,
        )
        .bind(monto_cents)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::dominio("ENVIO_FALLIDO", "No se pudo preparar el envio."))?;

        let envio = fila_envio(&fila)?;
        if let (Some(tipo), Some(tenant)) = (
            envio.proveedor_tipo.as_deref(),
            envio.proveedor_tenant_id.as_deref(),
        ) {
            self.registrar_asignacion(&envio.id, None, tipo, tenant, "automatica", None)
                .await?;
        }
        logs::info(
            "envios",
            crate::campos! {
                "msg": "Guia de despacho generada",
                "order_id": order_id,
                "guia": envio.guia,
            },
        );
        Ok(envio)
    }

    /// Consulta la orden a orders y agrupa las líneas por empresa.
    pub async fn preparar_envios_de_orden(&self, order_id: &str) -> Result<Vec<Envio>, ApiError> {
        let orden = self.consultar_orden(order_id).await?;
        let Some(orden) = orden else {
            return Err(ApiError::dominio(
                "ORDEN_SIN_LINEAS",
                "La orden no tiene líneas para despachar.",
            ));
        };
        if orden.items.is_empty() {
            return Err(ApiError::dominio(
                "ORDEN_SIN_LINEAS",
                "La orden no tiene líneas para despachar.",
            ));
        }

        let mut montos: Vec<(String, i64)> = Vec::new();
        for item in &orden.items {
            if item.empresa_id.trim().is_empty() {
                return Err(ApiError::dominio(
                    "EMPRESA_REQUERIDA",
                    "Una línea no tiene empresa asignada.",
                ));
            }
            let subtotal = item.precio_unitario_cents.saturating_mul(item.cantidad);
            match montos.iter_mut().find(|(id, _)| id == &item.empresa_id) {
                Some((_, total)) => *total += subtotal,
                None => montos.push((item.empresa_id.clone(), subtotal)),
            }
        }

        let mut envios = Vec::with_capacity(montos.len());
        for (empresa, monto) in montos {
            envios.push(
                self.preparar_envio(order_id, &empresa, monto, orden.parte_id.as_deref())
                    .await?,
            );
        }
        Ok(envios)
    }

    /// Deriva la flota de las capacidades activas de la empresa.
    async fn proveedor_de_empresa(&self, empresa_id: &str) -> Result<Proveedor, ApiError> {
        let filas = self
            .capacidades
            .activas(empresa_id, &["logistica_rutas", "fulfillment_delivery"])
            .await
            .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
        let activa = |codigo: &str| {
            filas.iter().any(|fila| {
                fila.empresa_id == empresa_id && fila.activo && fila.capacidad_codigo == codigo
            })
        };
        let propia = activa("logistica_rutas");
        let delivery = activa("fulfillment_delivery");
        if propia && delivery {
            Ok(Proveedor {
                tipo: None,
                tenant_id: None,
            })
        } else if propia {
            Ok(Proveedor {
                tipo: Some("empresa".to_string()),
                tenant_id: Some(empresa_id.to_string()),
            })
        } else {
            Ok(Proveedor {
                tipo: Some("plataforma".to_string()),
                tenant_id: Some(self.tenant_plataforma.clone()),
            })
        }
    }

    pub async fn asignar_proveedor(
        &self,
        envio_id: &str,
        empresa_id: &str,
        tipo: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let envio_uuid = parsear_uuid(envio_id)?;
        let empresa_uuid = parsear_uuid(empresa_id)?;

        let envio = sqlx::query("SELECT * FROM logistics.envios WHERE id = $1 AND empresa_id = $2")
            .bind(envio_uuid)
            .bind(empresa_uuid)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?
            .ok_or_else(|| {
                ApiError::dominio(
                    "ENVIO_NO_ENCONTRADO",
                    "El envío no pertenece a la empresa activa.",
                )
            })?;
        let envio = fila_envio(&envio)?;
        if envio.proveedor_tipo.is_some() {
            return Err(ApiError::dominio(
                "ASIGNACION_FIJA",
                "Solo un envío híbrido permite elegir proveedor.",
            ));
        }

        let tenant = if tipo == "plataforma" {
            self.tenant_plataforma.clone()
        } else {
            empresa_id.to_string()
        };
        let fila = sqlx::query(
            "UPDATE logistics.envios SET proveedor_tipo = $1, proveedor_tenant_id = $2
             WHERE id = $3 AND empresa_id = $4 RETURNING *",
        )
        .bind(tipo)
        .bind(parsear_uuid(&tenant)?)
        .bind(envio_uuid)
        .bind(empresa_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::dominio("ENVIO_NO_ENCONTRADO", "No se pudo asignar el envío."))?;

        let actualizado = fila_envio(&fila)?;
        self.registrar_asignacion(
            envio_id,
            envio.proveedor_tipo.as_deref(),
            tipo,
            &tenant,
            "empresario",
            actor_usuario_id,
        )
        .await?;
        Ok(actualizado)
    }

    async fn registrar_asignacion(
        &self,
        envio_id: &str,
        anterior: Option<&str>,
        nuevo: &str,
        proveedor_tenant_id: &str,
        origen: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let sql = format!(
            "INSERT INTO logistics.asignaciones_envio
                (envio_id, proveedor_tipo_anterior, proveedor_tipo_nuevo,
                 proveedor_tenant_id, origen, actor_usuario_id)
             VALUES ($1,$2,$3,$4,$5,$6)
             {}",
            if origen == "automatica" {
                "ON CONFLICT DO NOTHING"
            } else {
                ""
            }
        );
        let actor = match actor_usuario_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };
        sqlx::query(&sql)
            .bind(parsear_uuid(envio_id)?)
            .bind(anterior)
            .bind(nuevo)
            .bind(parsear_uuid(proveedor_tenant_id)?)
            .bind(origen)
            .bind(actor)
            .execute(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(())
    }

    pub async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<Envio>, ApiError> {
        let filas = sqlx::query(
            "SELECT * FROM logistics.envios WHERE empresa_id = $1
             ORDER BY creado_en DESC LIMIT 200",
        )
        .bind(parsear_uuid(empresa_id)?)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(fila_envio).collect()
    }

    /// Avanza un envío de la flota y, si todos los envíos de la orden alcanzan
    /// el estado, propaga la transición a orders.
    pub async fn avanzar_estado_envio(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let fila = sqlx::query(
            "UPDATE logistics.envios SET estado = $1
             WHERE id = $2 AND proveedor_tenant_id = $3
             RETURNING *",
        )
        .bind(estado)
        .bind(parsear_uuid(envio_id)?)
        .bind(parsear_uuid(proveedor_tenant_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "ENVIO_NO_ENCONTRADO",
                "El envío no pertenece a la flota activa.",
            )
        })?;
        let envio = fila_envio(&fila)?;

        let aceptados = estados_aceptados(estado);
        let pendientes = sqlx::query(
            "SELECT COUNT(*)::int AS total FROM logistics.envios
             WHERE order_id = $1 AND NOT (estado = ANY($2::text[]))",
        )
        .bind(parsear_uuid(&envio.order_id)?)
        .bind(&aceptados)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let total = pendientes
            .as_ref()
            .and_then(|fila| fila.try_get::<i32, _>("total").ok())
            .unwrap_or(1);
        if total == 0 {
            self.avanzar_estado_orden(&envio.order_id, estado, motivo, None, actor_usuario_id)
                .await?;
        }
        Ok(envio)
    }

    pub async fn de_orden(&self, order_id: &str) -> Result<Option<Envio>, ApiError> {
        let fila = sqlx::query(
            "SELECT * FROM logistics.envios WHERE order_id = $1
             ORDER BY creado_en LIMIT 1",
        )
        .bind(parsear_uuid(order_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.as_ref().map(fila_envio).transpose()
    }

    pub async fn listar(&self) -> Result<Vec<EnvioConMonto>, ApiError> {
        let filas = sqlx::query("SELECT * FROM logistics.envios ORDER BY creado_en DESC LIMIT 200")
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;
        filas
            .iter()
            .map(|fila| fila_envio(fila).map(EnvioConMonto::from))
            .collect()
    }

    /// `PATCH /orders/:id/estado`: avanza el ciclo de vida en orders (Tabla 13).
    pub async fn avanzar_estado_orden(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        empresa_id: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        if let Some(empresa_id) = empresa_id {
            let orden = self
                .consultar_orden(order_id)
                .await?
                .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
            if !orden.items.iter().any(|item| item.empresa_id == empresa_id) {
                return Err(ApiError::acceso_denegado(
                    "La orden no pertenece a la empresa activa.",
                ));
            }
        }

        let orden = self
            .orders
            .avanzar(order_id, estado_nuevo, motivo, actor_usuario_id)
            .await?;
        logs::info(
            "envios",
            crate::campos! {
                "msg": "Estado de orden avanzado",
                "order_id": order_id,
                "estado": estado_nuevo,
            },
        );
        Ok(orden)
    }

    /// Consulta una orden; `None` si orders no responde o responde error.
    pub async fn consultar_orden(&self, order_id: &str) -> Result<Option<OrdenExterna>, ApiError> {
        Ok(self.orders.consultar(order_id).await.unwrap_or(None))
    }
}

/// Estados que cuentan como avance suficiente cuando el envío pasa a `enviada`.
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
        monto_cents: i64,
        parte_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        Envios::preparar_envio(self, order_id, empresa_id, monto_cents, parte_id).await
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
        proveedor_tipo: fila
            .try_get::<Option<String>, _>("proveedor_tipo")
            .map_err(error_db)?,
        proveedor_tenant_id: uuid_opcional(fila, "proveedor_tenant_id").map_err(error_db)?,
        parte_id: uuid_opcional(fila, "parte_id").map_err(error_db)?,
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
            proveedor_tipo: Some("empresa".into()),
            proveedor_tenant_id: Some("emp-1".into()),
            parte_id: None,
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
