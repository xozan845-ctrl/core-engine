//! Libro de operación de comercio (ADR-66 a ADR-72).
//!
//! Persiste los recursos que el frontend de empresa/vendedor administra
//! (bancos, créditos, gastos, caja, compras, clientes y códigos de barras) en
//! `finance.workspace_registros`, con aislamiento por tenant o por vendedor.

use crate::auth::{UsuarioContexto, ROLES_COMERCIO};
use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, es_violacion_unicidad, ApiError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

pub const RECURSOS: [&str; 16] = [
    "bancos-cuentas",
    "creditos",
    "gastos",
    "cuentas-por-pagar",
    "proveedores",
    "compras",
    "ordenes-compra",
    "caja-metodos-pago",
    "caja-movimientos",
    "caja-cierres",
    "caja-chica-fondo",
    "caja-chica-movimientos",
    "caja-transferencias",
    "configuracion-documentos",
    "clientes",
    "codigos-barras",
];

/// Recursos que solo existen en el workspace de empresa (ADR-66/ADR-72).
pub const RECURSOS_SOLO_EMPRESA: [&str; 10] = [
    "gastos",
    "proveedores",
    "compras",
    "ordenes-compra",
    "caja-metodos-pago",
    "caja-movimientos",
    "caja-cierres",
    "caja-chica-fondo",
    "caja-chica-movimientos",
    "caja-transferencias",
];

pub const METODOS_PAGO: [&str; 5] = ["efectivo", "tarjeta", "transferencia", "credito", "otro"];
pub const TIPOS_GASTO: [&str; 3] = ["inmediato", "programado", "recurrente"];
pub const ESTADOS_COMPRA: [&str; 3] = ["recibida", "pendiente", "anulada"];
pub const ORIGENES_COMPRA: [&str; 2] = ["productos", "directa"];
pub const TIPOS_COSTO: [&str; 4] = ["inventario", "servicio", "gasto", "otro"];
pub const SIMBOLOGIAS: [&str; 2] = ["EAN13", "CODE128"];
pub const TIPOS_CUENTA_BANCO: [&str; 3] = ["ahorro", "corriente", "wallet"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ambito {
    pub tenant_id: Option<String>,
    pub propietario_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CuentaCredito {
    pub id: String,
    pub empresa_id: String,
    pub parte_id: String,
    pub parte_nombre: String,
    pub limite_cents: i64,
    pub usado_cents: i64,
    pub disponible_cents: i64,
    pub activo: bool,
    pub actualizado_en: String,
}

/// Registro público: columnas de control + payload del recurso.
pub type Registro = Map<String, Value>;

pub struct Comercio {
    db: Arc<Db>,
}

// ── contenedores de petición ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LineaCompra {
    #[serde(default)]
    pub product_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    pub quantity: f64,
    pub unit_cost_cents: i64,
    #[serde(default)]
    pub sale_price_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaBanco {
    pub bank: String,
    pub alias: String,
    pub account_number: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Credito {
    pub customer: String,
    pub concept: String,
    #[serde(default)]
    pub date: Option<String>,
    pub due_date: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AbonoCredito {
    pub monto_cents: i64,
    #[serde(default)]
    pub metodo_pago: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaCreditoInput {
    pub limite_cents: i64,
    #[serde(default)]
    pub activo: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreditoInterno {
    pub empresa_id: String,
    pub parte_id: String,
    pub order_id: String,
    pub monto_cents: i64,
    pub concepto: String,
    #[serde(default)]
    pub vendedor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gasto {
    pub date: String,
    pub concept: String,
    pub category: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub includes_vat: Option<bool>,
    #[serde(default)]
    pub ir_retention_cents: Option<i64>,
    #[serde(default)]
    pub beneficiary: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub recurrence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaPorPagar {
    pub supplier: String,
    pub concept: String,
    pub due_date: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proveedor {
    pub name: String,
    #[serde(default)]
    pub ruc: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Compra {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub payment_method: String,
    pub status: String,
    pub origin: String,
    pub cost_type: String,
    pub lines: Vec<LineaCompra>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub retention_cents: Option<i64>,
    #[serde(default)]
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompraDirecta {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub concept: String,
    pub cost_type: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub retention_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompraProductoNuevo {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub product_name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sku: Option<String>,
    pub payment_method: String,
    pub unit_cost_cents: i64,
    pub sale_price_cents: i64,
    pub quantity: f64,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrdenCompra {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub lines: Vec<LineaCompra>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetodoPago {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoCaja {
    pub date: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub concept: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub original_amount_cents: Option<i64>,
    #[serde(default)]
    pub exchange_rate: Option<f64>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub observations: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CierreCaja {
    pub date: String,
    pub counted_balance_cents: i64,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FondoCajaChica {
    pub amount_cents: i64,
    pub responsible: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoCajaChica {
    pub date: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub concept: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransferenciaCaja {
    pub date: String,
    pub from_id: String,
    pub from_label: String,
    pub from_kind: String,
    pub to_id: String,
    pub to_label: String,
    pub to_kind: String,
    pub amount_cents: i64,
    pub concept: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguracionDocumentos {
    pub logo_size: String,
    pub header_color: String,
    pub match_table_color: bool,
    pub footer_message: String,
    pub show_ruc: bool,
    pub show_address: bool,
    pub show_phone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClienteComercio {
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub buyer_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportarClientes {
    pub clientes: Vec<ClienteComercio>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodigoBarras {
    pub product_name: String,
    pub sku: String,
    #[serde(default)]
    pub code: Option<String>,
    pub symbology: String,
    pub copies: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportacionClientes {
    pub imported: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewCierre {
    pub date: String,
    pub opening_balance_cents: i64,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub expected_balance_cents: i64,
    pub movements_count: i64,
    pub existing: Option<Registro>,
}

impl Comercio {
    pub fn nuevo(db: Arc<Db>) -> Self {
        Self { db }
    }

    // ── consultas y CRUD genérico ───────────────────────────────────────────

    pub async fn listar(
        &self,
        recurso: &str,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Vec<Registro>, ApiError> {
        let ambito = Self::ambito(recurso, usuario, empresa_id)?;
        let filas = sqlx::query(
            "SELECT id, datos, creado_en, actualizado_en
             FROM finance.workspace_registros
             WHERE recurso = $1 AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($3::uuid IS NULL OR propietario_id = $3::uuid)
             ORDER BY creado_en DESC, id",
        )
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(a_registro).collect())
    }

    pub async fn obtener(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let fila = self.buscar(recurso, id, usuario).await?;
        Ok(a_registro(&fila))
    }

    pub async fn crear(
        &self,
        recurso: &str,
        datos: Value,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        if ambito.tenant_id.is_none() && ambito.propietario_id.is_none() {
            return Err(ApiError::dominio(
                "TENANT_REQUERIDO",
                "El contexto no tiene empresa ni vendedor.",
            ));
        }
        let payload = Self::normalizar(recurso, datos)?;
        let fila = self.insertar(recurso, payload, &ambito).await?;
        Ok(a_registro(&fila))
    }

    pub async fn actualizar(
        &self,
        recurso: &str,
        id: &str,
        datos: Value,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || $4::jsonb, actualizado_en = NOW()
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($5::uuid IS NULL OR propietario_id = $5::uuid)
             RETURNING id, datos, creado_en, actualizado_en",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(sqlx::types::Json(&datos))
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.map(|fila| a_registro(&fila))
            .ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    pub async fn eliminar(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET eliminado_en = NOW(), actualizado_en = NOW()
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
             RETURNING id",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.map(|_| ())
            .ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    // ── bancos ─────────────────────────────────────────────────────────────

    pub async fn crear_banco(
        &self,
        dto: CuentaBanco,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.bank, "bank", 1)?;
        validar_texto(&dto.alias, "alias", 1)?;
        validar_texto(&dto.account_number, "accountNumber", 1)?;
        validar_in(&dto.tipo, "type", &TIPOS_CUENTA_BANCO)?;
        validar_rango(dto.balance_cents as f64, "balanceCents", 0.0, f64::MAX)?;
        self.crear(
            "bancos-cuentas",
            json!({
                "bank": dto.bank,
                "alias": dto.alias,
                "accountNumber": dto.account_number,
                "type": dto.tipo,
                "balanceCents": dto.balance_cents,
            }),
            usuario,
        )
        .await
    }

    // ── créditos ───────────────────────────────────────────────────────────

    pub async fn crear_credito(
        &self,
        dto: Credito,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.customer, "customer", 1)?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_fecha(&dto.due_date, "dueDate")?;
        if let Some(fecha) = &dto.date {
            validar_fecha(fecha, "date")?;
        }
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        self.crear(
            "creditos",
            json!({
                "customer": dto.customer,
                "concept": dto.concept,
                "date": dto.date.unwrap_or_else(hoy),
                "dueDate": dto.due_date,
                "amountCents": dto.amount_cents,
                "balanceCents": dto.amount_cents,
            }),
            usuario,
        )
        .await
    }

    pub async fn pagar_credito(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        self.actualizar(
            "creditos",
            id,
            json!({"balanceCents": 0, "paidAt": hoy()}),
            usuario,
        )
        .await
    }

    pub async fn abonar_credito(
        &self,
        id: &str,
        dto: AbonoCredito,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let credito = self.buscar("creditos", id, usuario).await?;
        let datos: Value = jsonb(&credito, "datos").map_err(error_db)?;
        let saldo = datos
            .get("balanceCents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if dto.monto_cents <= 0 {
            return Err(ApiError::dominio(
                "ABONO_INVALIDO",
                "El abono debe ser un entero positivo en centavos.",
            ));
        }
        if dto.monto_cents > saldo {
            return Err(ApiError::dominio(
                "ABONO_EXCEDE_SALDO",
                "El abono supera el saldo pendiente del credito.",
            ));
        }
        let mut abonos = datos
            .get("abonos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let nuevo_saldo = saldo - dto.monto_cents;
        abonos.push(json!({
            "montoCents": dto.monto_cents,
            "fecha": hoy(),
            "metodoPago": dto.metodo_pago.unwrap_or_else(|| "efectivo".to_string()),
        }));
        let mut cambios = json!({
            "balanceCents": nuevo_saldo,
            "abonos": abonos,
        });
        if nuevo_saldo == 0 {
            cambios["paidAt"] = json!(hoy());
        }
        self.actualizar("creditos", id, cambios, usuario).await
    }

    // ── cuentas de crédito por parte (venta a crédito) ──────────────────────

    pub async fn listar_cuentas_credito(
        &self,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Vec<CuentaCredito>, ApiError> {
        let tenant = Self::tenant_credito(usuario, empresa_id, false)?;
        let tenant = parsear_uuid(&tenant)?;
        let filas = sqlx::query(
            "SELECT c.id, c.empresa_id, c.parte_id, p.nombre AS parte_nombre,
                    c.limite_cents, c.activo, c.actualizado_en,
                    COALESCE((
                      SELECT SUM((w.datos ->> 'balanceCents')::int)
                        FROM finance.workspace_registros w
                       WHERE w.recurso = 'creditos' AND w.eliminado_en IS NULL
                         AND w.tenant_id = c.empresa_id AND w.parte_id = c.parte_id
                    ), 0)::int AS usado_cents
             FROM finance.credito_cuentas c
             JOIN identity.partes p ON p.id = c.parte_id
             WHERE c.empresa_id = $1
             ORDER BY p.nombre",
        )
        .bind(tenant)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas
            .iter()
            .map(|fila| {
                let limite = fila.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;
                let usado = fila.try_get::<i32, _>("usado_cents").unwrap_or(0) as i64;
                CuentaCredito {
                    id: uuid_texto(fila, "id").unwrap_or_default(),
                    empresa_id: uuid_texto(fila, "empresa_id").unwrap_or_default(),
                    parte_id: uuid_texto(fila, "parte_id").unwrap_or_default(),
                    parte_nombre: fila.try_get("parte_nombre").unwrap_or_default(),
                    limite_cents: limite,
                    usado_cents: usado,
                    disponible_cents: limite - usado,
                    activo: fila.try_get("activo").unwrap_or(false),
                    actualizado_en: timestamp(fila, "actualizado_en").unwrap_or_default(),
                }
            })
            .collect())
    }

    pub async fn guardar_cuenta_credito(
        &self,
        parte_id: &str,
        dto: CuentaCreditoInput,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<CuentaCredito, ApiError> {
        let tenant = Self::tenant_credito(usuario, empresa_id, true)?;
        let tenant = parsear_uuid(&tenant)?;
        validar_rango(dto.limite_cents as f64, "limiteCents", 0.0, f64::MAX)?;
        let parte_uuid = parsear_uuid(parte_id)?;
        let parte = sqlx::query(
            "SELECT nombre FROM identity.partes
             WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(parte_uuid)
        .bind(tenant)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(parte) = parte else {
            return Err(ApiError::no_encontrado("Parte", parte_id));
        };
        let nombre: String = parte.try_get("nombre").unwrap_or_default();

        let fila = sqlx::query(
            "INSERT INTO finance.credito_cuentas (empresa_id, parte_id, limite_cents, activo)
             VALUES ($1, $2, $3, COALESCE($4, true))
             ON CONFLICT (empresa_id, parte_id) DO UPDATE
               SET limite_cents = EXCLUDED.limite_cents,
                   activo = COALESCE($4, finance.credito_cuentas.activo),
                   actualizado_en = NOW()
             RETURNING id, empresa_id, parte_id, limite_cents, activo, actualizado_en",
        )
        .bind(tenant)
        .bind(parte_uuid)
        .bind(dto.limite_cents as i32)
        .bind(dto.activo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("CUENTA_CREDITO_FALLIDA", "No se pudo guardar la cuenta.")
        })?;

        let usado: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM((datos ->> 'balanceCents')::int), 0)::int AS usado
             FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND tenant_id = $1 AND parte_id = $2",
        )
        .bind(tenant)
        .bind(parte_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let limite = fila.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;
        let usado = usado.unwrap_or(0) as i64;
        Ok(CuentaCredito {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            empresa_id: uuid_texto(&fila, "empresa_id").map_err(error_db)?,
            parte_id: uuid_texto(&fila, "parte_id").map_err(error_db)?,
            parte_nombre: nombre,
            limite_cents: limite,
            usado_cents: usado,
            disponible_cents: limite - usado,
            activo: fila.try_get("activo").map_err(error_db)?,
            actualizado_en: timestamp(&fila, "actualizado_en").map_err(error_db)?,
        })
    }

    /// Registro interno (orders → finance) de una venta a crédito.
    pub async fn registrar_credito_orden(&self, input: CreditoInterno) -> Result<Value, ApiError> {
        if input.monto_cents <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "monto_cents", "restricciones": ["min:1"]}
            ])));
        }
        let empresa = parsear_uuid(&input.empresa_id)?;
        let parte = parsear_uuid(&input.parte_id)?;
        let cuenta = sqlx::query(
            "SELECT limite_cents, activo FROM finance.credito_cuentas
             WHERE empresa_id = $1 AND parte_id = $2",
        )
        .bind(empresa)
        .bind(parte)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(cuenta) = cuenta else {
            return Err(ApiError::conflicto(
                "La parte no tiene una cuenta de credito activa.",
            ));
        };
        let activo: bool = cuenta.try_get("activo").unwrap_or(false);
        if !activo {
            return Err(ApiError::conflicto(
                "La parte no tiene una cuenta de credito activa.",
            ));
        }
        let limite = cuenta.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;

        let existente = sqlx::query(
            "SELECT id, datos FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND datos ->> 'orderId' = $1
             LIMIT 1",
        )
        .bind(&input.order_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some(existente) = existente {
            let datos: Value = jsonb(&existente, "datos").map_err(error_db)?;
            return Ok(json!({
                "credito_id": uuid_texto(&existente, "id").map_err(error_db)?,
                "balance_cents": datos.get("balanceCents").and_then(Value::as_i64).unwrap_or(0),
                "limite_cents": limite,
            }));
        }

        let parte_fila = sqlx::query(
            "SELECT nombre FROM identity.partes
             WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(parte)
        .bind(empresa)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(parte_fila) = parte_fila else {
            return Err(ApiError::no_encontrado("Parte", &input.parte_id));
        };
        let nombre: String = parte_fila.try_get("nombre").unwrap_or_default();

        let usado: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM((datos ->> 'balanceCents')::int), 0)::int AS usado
             FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND tenant_id = $1 AND parte_id = $2",
        )
        .bind(empresa)
        .bind(parte)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let disponible = limite - usado.unwrap_or(0) as i64;
        if input.monto_cents > disponible {
            return Err(ApiError::conflicto(
                "El credito supera el limite disponible de la parte.",
            ));
        }

        let datos = json!({
            "customer": nombre,
            "concept": input.concepto,
            "date": hoy(),
            "dueDate": "",
            "amountCents": input.monto_cents,
            "balanceCents": input.monto_cents,
            "orderId": input.order_id,
            "sellerId": input.vendedor_id.unwrap_or_default(),
        });
        let fila = sqlx::query(
            "INSERT INTO finance.workspace_registros
               (tenant_id, propietario_id, recurso, datos, parte_id)
             VALUES ($1, NULL, 'creditos', $2::jsonb, $3)
             RETURNING id",
        )
        .bind(empresa)
        .bind(sqlx::types::Json(&datos))
        .bind(parte)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("CREDITO_NO_CREADO", "No se pudo registrar el credito.")
        })?;
        Ok(json!({
            "credito_id": uuid_texto(&fila, "id").map_err(error_db)?,
            "balance_cents": input.monto_cents,
            "limite_cents": limite,
        }))
    }

    pub async fn anular_credito_orden(&self, order_id: &str) -> Result<bool, ApiError> {
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET eliminado_en = NOW(), actualizado_en = NOW()
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND datos ->> 'orderId' = $1
             RETURNING id",
        )
        .bind(order_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.is_some())
    }

    // ── gastos y cuentas por pagar ─────────────────────────────────────────

    pub async fn crear_gasto(
        &self,
        dto: Gasto,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_texto(&dto.category, "category", 1)?;
        validar_in(&dto.tipo, "type", &TIPOS_GASTO)?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        self.crear("gastos", json!(dto), usuario).await
    }

    pub async fn crear_cuenta_por_pagar(
        &self,
        dto: CuentaPorPagar,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.supplier, "supplier", 1)?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_fecha(&dto.due_date, "dueDate")?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        self.crear("cuentas-por-pagar", json!(dto), usuario).await
    }

    // ── proveedores y compras ──────────────────────────────────────────────

    pub async fn crear_proveedor(
        &self,
        dto: Proveedor,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.name, "name", 1)?;
        if let Some(email) = &dto.email {
            validar_email(email, "email")?;
        }
        self.crear("proveedores", json!(dto), usuario).await
    }

    pub async fn crear_compra(
        &self,
        dto: Compra,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        validar_in(&dto.status, "status", &ESTADOS_COMPRA)?;
        validar_in(&dto.origin, "origin", &ORIGENES_COMPRA)?;
        validar_in(&dto.cost_type, "costType", &TIPOS_COSTO)?;
        if dto.lines.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lines", "restricciones": ["arrayMinSize"]}
            ])));
        }
        validar_lineas(&dto.lines)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": dto.status,
                "origin": dto.origin,
                "costType": dto.cost_type,
                "lines": lineas_con_id(&dto.lines),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": dto.retention_cents.unwrap_or(0),
                "userName": dto.user_name.unwrap_or_default(),
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_compra_directa(
        &self,
        dto: CompraDirecta,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.concept, "concept", 1)?;
        validar_in(&dto.cost_type, "costType", &TIPOS_COSTO)?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        let linea = LineaCompra {
            product_id: None,
            name: dto.concept,
            sku: Some(String::new()),
            quantity: 1.0,
            unit_cost_cents: dto.amount_cents,
            sale_price_cents: None,
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": "recibida",
                "origin": "directa",
                "costType": dto.cost_type,
                "lines": lineas_con_id(&[linea]),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": dto.retention_cents.unwrap_or(0),
                "userName": "",
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_compra_producto_nuevo(
        &self,
        dto: CompraProductoNuevo,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.product_name, "productName", 1)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        validar_rango(dto.unit_cost_cents as f64, "unitCostCents", 0.0, f64::MAX)?;
        validar_rango(dto.sale_price_cents as f64, "salePriceCents", 0.0, f64::MAX)?;
        validar_rango(dto.quantity, "quantity", 1.0, f64::MAX)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        let linea = LineaCompra {
            product_id: None,
            name: dto.product_name,
            sku: dto.sku,
            quantity: dto.quantity,
            unit_cost_cents: dto.unit_cost_cents,
            sale_price_cents: Some(dto.sale_price_cents),
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": "recibida",
                "origin": "productos",
                "costType": "inventario",
                "lines": lineas_con_id(&[linea]),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": 0,
                "userName": "",
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_orden_compra(
        &self,
        dto: OrdenCompra,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        if dto.lines.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lines", "restricciones": ["arrayMinSize"]}
            ])));
        }
        validar_lineas(&dto.lines)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => {
                self.siguiente_numero("ordenes-compra", "OC", usuario)
                    .await?
            }
        };
        self.crear(
            "ordenes-compra",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "lines": lineas_con_id(&dto.lines),
                "notes": dto.notes.unwrap_or_default(),
            }),
            usuario,
        )
        .await
    }

    pub async fn convertir_orden_compra(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let orden = self.obtener("ordenes-compra", id, usuario).await?;
        let estado = orden
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if estado != "pendiente" {
            return Err(ApiError::dominio(
                "ORDEN_NO_PENDIENTE",
                "La orden ya no está pendiente.",
            ));
        }
        let numero = self.siguiente_numero("compras", "C", usuario).await?;
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": hoy(),
                "supplierId": orden.get("supplierId").cloned().unwrap_or(Value::Null),
                "supplierName": orden.get("supplierName").cloned().unwrap_or_else(|| json!("")),
                "paymentMethod": "efectivo",
                "status": "pendiente",
                "origin": "productos",
                "costType": "inventario",
                "lines": orden.get("lines").cloned().unwrap_or_else(|| json!([])),
                "notes": format!(
                    "Convertida desde la orden {}.",
                    orden.get("number").and_then(Value::as_str).unwrap_or("")
                ),
                "retentionCents": 0,
                "userName": "",
            }),
            usuario,
        )
        .await?;
        self.actualizar(
            "ordenes-compra",
            id,
            json!({"status": "convertida"}),
            usuario,
        )
        .await
    }

    // ── configuración de documentos ────────────────────────────────────────

    pub async fn obtener_configuracion_documentos(
        &self,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let registros = self
            .listar("configuracion-documentos", usuario, None)
            .await?;
        if let Some(existente) = registros.into_iter().next() {
            return Ok(existente);
        }
        self.crear(
            "configuracion-documentos",
            json!({
                "logoSize": "mediano",
                "headerColor": "#0f172a",
                "matchTableColor": false,
                "footerMessage": "",
                "showRuc": true,
                "showAddress": true,
                "showPhone": true,
            }),
            usuario,
        )
        .await
    }

    pub async fn actualizar_configuracion_documentos(
        &self,
        dto: ConfiguracionDocumentos,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_in(
            &dto.logo_size,
            "logoSize",
            &["pequeno", "mediano", "grande"],
        )?;
        validar_texto(&dto.header_color, "headerColor", 1)?;
        let actual = self.obtener_configuracion_documentos(usuario).await?;
        let id = actual
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ApiError::interno)?
            .to_string();
        self.actualizar("configuracion-documentos", &id, json!(dto), usuario)
            .await
    }

    // ── caja ───────────────────────────────────────────────────────────────

    pub async fn eliminar_metodo_pago(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        let metodos = self.listar("caja-metodos-pago", usuario, None).await?;
        if metodos.len() <= 1 {
            return Err(ApiError::dominio(
                "ULTIMO_METODO",
                "Debe quedar al menos un método de pago.",
            ));
        }
        let movimientos = self.listar("caja-movimientos", usuario, None).await?;
        if movimientos
            .iter()
            .any(|movimiento| movimiento.get("paymentMethod").and_then(Value::as_str) == Some(id))
        {
            return Err(ApiError::dominio(
                "METODO_EN_USO",
                "No se puede eliminar un método de pago con movimientos registrados.",
            ));
        }
        self.eliminar("caja-metodos-pago", id, usuario).await
    }

    /// Calcula el corte del día sin persistirlo (pantalla de cierre).
    pub async fn preview_cierre(
        &self,
        usuario: &UsuarioContexto,
        fecha: Option<&str>,
    ) -> Result<PreviewCierre, ApiError> {
        let dia = fecha.map(str::to_string).unwrap_or_else(hoy);
        let movimientos: Vec<Registro> = self
            .listar("caja-movimientos", usuario, None)
            .await?
            .into_iter()
            .filter(|movimiento| {
                movimiento.get("status").and_then(Value::as_str) == Some("completado")
            })
            .collect();
        let anteriores: Vec<Registro> = movimientos
            .iter()
            .filter(|movimiento| {
                movimiento
                    .get("date")
                    .and_then(Value::as_str)
                    .map(|valor| valor < dia.as_str())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let del_dia: Vec<Registro> = movimientos
            .iter()
            .filter(|movimiento| {
                movimiento.get("date").and_then(Value::as_str) == Some(dia.as_str())
            })
            .cloned()
            .collect();
        let abrir = saldo(&anteriores);
        let ingresos = suma(
            &del_dia
                .iter()
                .filter(|movimiento| {
                    movimiento.get("type").and_then(Value::as_str) == Some("ingreso")
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let egresos = suma(
            &del_dia
                .iter()
                .filter(|movimiento| {
                    movimiento.get("type").and_then(Value::as_str) == Some("egreso")
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let existente = self
            .listar("caja-cierres", usuario, None)
            .await?
            .into_iter()
            .find(|cierre| cierre.get("date").and_then(Value::as_str) == Some(dia.as_str()));
        Ok(PreviewCierre {
            date: dia,
            opening_balance_cents: abrir,
            income_cents: ingresos,
            expense_cents: egresos,
            expected_balance_cents: abrir + ingresos - egresos,
            movements_count: del_dia.len() as i64,
            existing: existente,
        })
    }

    pub async fn crear_cierre(
        &self,
        dto: CierreCaja,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_rango(
            dto.counted_balance_cents as f64,
            "countedBalanceCents",
            0.0,
            f64::MAX,
        )?;
        let preview = self.preview_cierre(usuario, Some(&dto.date)).await?;
        if preview.existing.is_some() {
            return Err(ApiError::dominio(
                "CIERRE_EXISTENTE",
                format!("Ya existe un cierre para {}.", dto.date),
            ));
        }
        self.crear(
            "caja-cierres",
            json!({
                "date": preview.date,
                "openingBalanceCents": preview.opening_balance_cents,
                "incomeCents": preview.income_cents,
                "expenseCents": preview.expense_cents,
                "expectedBalanceCents": preview.expected_balance_cents,
                "countedBalanceCents": dto.counted_balance_cents,
                "differenceCents": dto.counted_balance_cents - preview.expected_balance_cents,
                "movementsCount": preview.movements_count,
                "notes": dto.notes.unwrap_or_default(),
                "closedAt": iso_ms(Utc::now()),
            }),
            usuario,
        )
        .await
    }

    pub async fn obtener_fondo_caja_chica(
        &self,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let registros = self.listar("caja-chica-fondo", usuario, None).await?;
        if let Some(existente) = registros.into_iter().next() {
            return Ok(existente);
        }
        self.crear(
            "caja-chica-fondo",
            json!({"amountCents": 0, "responsible": "", "updatedAt": iso_ms(Utc::now())}),
            usuario,
        )
        .await
    }

    pub async fn guardar_fondo_caja_chica(
        &self,
        dto: FondoCajaChica,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_rango(dto.amount_cents as f64, "amountCents", 0.0, f64::MAX)?;
        validar_texto(&dto.responsible, "responsible", 1)?;
        let actual = self.obtener_fondo_caja_chica(usuario).await?;
        let id = actual
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ApiError::interno)?
            .to_string();
        self.actualizar(
            "caja-chica-fondo",
            &id,
            json!({
                "amountCents": dto.amount_cents,
                "responsible": dto.responsible,
                "updatedAt": iso_ms(Utc::now()),
            }),
            usuario,
        )
        .await
    }

    /// Registra una transferencia interna y sus dos movimientos de caja.
    pub async fn crear_transferencia(
        &self,
        dto: TransferenciaCaja,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.from_id, "fromId", 1)?;
        validar_texto(&dto.from_label, "fromLabel", 1)?;
        validar_in(&dto.from_kind, "fromKind", &["cash", "bank"])?;
        validar_texto(&dto.to_id, "toId", 1)?;
        validar_texto(&dto.to_label, "toLabel", 1)?;
        validar_in(&dto.to_kind, "toKind", &["cash", "bank"])?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_texto(&dto.concept, "concept", 1)?;
        if dto.from_id == dto.to_id {
            return Err(ApiError::dominio(
                "TRANSFERENCIA_INVALIDA",
                "Las cuentas origen y destino deben ser distintas.",
            ));
        }
        let ambito = Self::ambito("caja-transferencias", usuario, None)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if dto.from_kind == "bank" {
            mover_banco(&mut tx, &dto.from_id, -dto.amount_cents, &ambito).await?;
        }
        if dto.to_kind == "bank" {
            mover_banco(&mut tx, &dto.to_id, dto.amount_cents, &ambito).await?;
        }
        let transferencia = insertar_en(
            &mut tx,
            "caja-transferencias",
            Self::normalizar(
                "caja-transferencias",
                json!({
                    "fromId": dto.from_id,
                    "fromLabel": dto.from_label,
                    "fromKind": dto.from_kind,
                    "toId": dto.to_id,
                    "toLabel": dto.to_label,
                    "toKind": dto.to_kind,
                    "amountCents": dto.amount_cents,
                    "concept": dto.concept,
                    "date": dto.date,
                    "notes": dto.notes.clone().unwrap_or_default(),
                    "status": "completado",
                }),
            )?,
            &ambito,
        )
        .await?;
        let transfer_id = uuid_texto(&transferencia, "id").map_err(error_db)?;
        let base = json!({
            "date": dto.date,
            "concept": dto.concept,
            "amountCents": dto.amount_cents,
            "paymentMethod": "transferencia",
            "category": "transferencia",
            "currency": "NIO",
            "originalAmountCents": dto.amount_cents,
            "exchangeRate": 1,
            "source": "transferencia",
            "status": "completado",
            "reference": transfer_id,
            "observations": dto.notes.clone().unwrap_or_default(),
            "transferId": transfer_id,
        });
        for tipo in ["egreso", "ingreso"] {
            let mut datos = base.clone();
            datos["type"] = json!(tipo);
            insertar_en(
                &mut tx,
                "caja-movimientos",
                Self::normalizar("caja-movimientos", datos)?,
                &ambito,
            )
            .await?;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(a_registro(&transferencia))
    }

    /// Anula la transferencia, sus movimientos ligados y revierte los bancos.
    pub async fn anular_transferencia(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito("caja-transferencias", usuario, None)?;
        let actual = self.buscar("caja-transferencias", id, usuario).await?;
        let datos: Value = jsonb(&actual, "datos").map_err(error_db)?;
        if datos.get("status").and_then(Value::as_str) == Some("anulado") {
            return Ok(a_registro(&actual));
        }
        let monto = numero(datos.get("amountCents"));
        let uuid = parsear_uuid(id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if datos.get("fromKind").and_then(Value::as_str) == Some("bank") {
            let from_id = datos
                .get("fromId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            mover_banco(&mut tx, from_id, monto, &ambito).await?;
        }
        if datos.get("toKind").and_then(Value::as_str) == Some("bank") {
            let to_id = datos
                .get("toId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            mover_banco(&mut tx, to_id, -monto, &ambito).await?;
        }
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || '{\"status\":\"anulado\"}'::jsonb, actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'caja-transferencias'
             RETURNING id, datos, creado_en, actualizado_en",
        )
        .bind(uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || '{\"status\":\"anulado\"}'::jsonb, actualizado_en = NOW()
             WHERE recurso = 'caja-movimientos' AND eliminado_en IS NULL
               AND datos->>'transferId' = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(a_registro(&fila))
    }

    // ── clientes y códigos de barras ───────────────────────────────────────

    pub async fn crear_cliente(
        &self,
        dto: ClienteComercio,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.name, "name", 1)?;
        if let Some(email) = &dto.email {
            validar_email(email, "email")?;
        }
        self.crear("clientes", json!(dto), usuario).await
    }

    pub async fn importar_clientes(
        &self,
        dto: ImportarClientes,
        usuario: &UsuarioContexto,
    ) -> Result<ImportacionClientes, ApiError> {
        if dto.clientes.len() > 500 {
            return Err(ApiError::dominio(
                "IMPORTACION_DEMASIADO_GRANDE",
                "El máximo es 500 clientes por importación.",
            ));
        }
        let existentes = self.listar("clientes", usuario, None).await?;
        let mut claves: HashSet<String> = existentes.iter().map(clave_cliente).collect();
        let mut imported = 0i64;
        let mut skipped = 0i64;
        let mut errors = Vec::new();
        for (indice, fila) in dto.clientes.into_iter().enumerate() {
            let nombre = fila.name.trim().to_string();
            if nombre.is_empty() {
                skipped += 1;
                errors.push(format!("Fila {}: nombre vacío.", indice + 1));
                continue;
            }
            let fila_clave = json!({"name": fila.name, "phone": fila.phone.clone()});
            let clave = clave_cliente(fila_clave.as_object().unwrap_or(&Map::new()));
            if claves.contains(&clave) {
                skipped += 1;
                continue;
            }
            let mut datos = json!(fila);
            datos["name"] = json!(nombre);
            datos["source"] = json!("import");
            match self.crear("clientes", datos, usuario).await {
                Ok(_) => {
                    claves.insert(clave);
                    imported += 1;
                }
                Err(error) => errors.push(format!("Fila {}: {}", indice + 1, error.mensaje)),
            }
        }
        Ok(ImportacionClientes {
            imported,
            skipped,
            errors,
        })
    }

    pub async fn crear_codigo_barras(
        &self,
        dto: CodigoBarras,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.product_name, "productName", 1)?;
        validar_texto(&dto.sku, "sku", 1)?;
        validar_in(&dto.symbology, "symbology", &SIMBOLOGIAS)?;
        validar_rango(dto.copies as f64, "copies", 1.0, 100.0)?;
        self.crear("codigos-barras", json!(dto), usuario).await
    }

    // ── helpers ────────────────────────────────────────────────────────────

    pub fn ambito(
        recurso: &str,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Ambito, ApiError> {
        if !ROLES_COMERCIO.contains(&usuario.rol.as_str()) {
            return Err(ApiError::dominio(
                "ACCESO_DENEGADO",
                "El rol no tiene acceso a la operación de comercio.",
            ));
        }
        if empresa_id.is_none()
            && usuario.rol == "vendedor"
            && RECURSOS_SOLO_EMPRESA.contains(&recurso)
        {
            return Err(ApiError::dominio(
                "ACCESO_DENEGADO",
                "Este recurso pertenece al workspace de empresa.",
            ));
        }
        if usuario.rol == "vendedor" {
            return Ok(Ambito {
                tenant_id: None,
                propietario_id: Some(usuario.user_id.clone()),
            });
        }
        if usuario.rol == "empresario" {
            let tenant = usuario.tenant_id.clone().ok_or_else(|| {
                ApiError::dominio(
                    "TENANT_REQUERIDO",
                    "El empresario no tiene una empresa activa.",
                )
            })?;
            return Ok(Ambito {
                tenant_id: Some(tenant),
                propietario_id: None,
            });
        }
        Ok(Ambito {
            tenant_id: empresa_id
                .map(str::to_string)
                .or_else(|| usuario.tenant_id.clone()),
            propietario_id: None,
        })
    }

    fn tenant_credito(
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
        escritura: bool,
    ) -> Result<String, ApiError> {
        if !ROLES_COMERCIO.contains(&usuario.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "El rol no tiene acceso a cuentas de credito.",
            ));
        }
        if escritura && !crate::auth::ROLES_CREDITO_ESCRITURA.contains(&usuario.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "Solo la empresa puede definir limites de credito.",
            ));
        }
        if usuario.es_admin_global() {
            return empresa_id
                .map(str::to_string)
                .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."));
        }
        let tenant = usuario.tenant().map(str::to_string)?;
        if let Some(empresa) = empresa_id {
            if empresa != tenant {
                return Err(ApiError::acceso_denegado("No puedes operar otra empresa."));
            }
        }
        Ok(tenant)
    }

    async fn buscar(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<sqlx::postgres::PgRow, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "SELECT id, datos, creado_en, actualizado_en
             FROM finance.workspace_registros
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    async fn insertar(
        &self,
        recurso: &str,
        datos: Value,
        ambito: &Ambito,
    ) -> Result<sqlx::postgres::PgRow, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = insertar_en(&mut tx, recurso, datos, ambito).await?;
        tx.commit().await.map_err(error_db)?;
        Ok(fila)
    }

    /// Construye el payload final del recurso: defaults y totales calculados.
    pub fn normalizar(recurso: &str, datos: Value) -> Result<Value, ApiError> {
        let ahora = iso_ms(Utc::now());
        let mut objeto = datos.as_object().cloned().unwrap_or_default();
        match recurso {
            "creditos" => {
                objeto.entry("date").or_insert_with(|| json!(hoy()));
                if !objeto.contains_key("balanceCents") {
                    let amount = objeto.get("amountCents").cloned().unwrap_or(json!(0));
                    objeto.insert("balanceCents".into(), amount);
                }
            }
            "gastos" => {
                let defaults = json!({
                    "status": "pendiente",
                    "includesVat": false,
                    "irRetentionCents": 0,
                    "beneficiary": "",
                    "notes": "",
                    "scheduledFor": "",
                    "recurrence": "",
                });
                objeto = con_defaults(objeto, defaults);
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "cuentas-por-pagar" => {
                objeto = con_defaults(objeto, json!({"status": "pendiente", "paidAt": ""}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "proveedores" => {
                objeto = con_defaults(
                    objeto,
                    json!({"ruc": "", "phone": "", "email": "", "address": ""}),
                );
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "compras" => {
                let lineas = objeto
                    .get("lines")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let subtotal = total_lineas(&Value::Array(lineas.clone()));
                objeto = con_defaults(objeto, json!({"status": "pendiente", "statusLabel": ""}));
                objeto.insert("lines".into(), Value::Array(lineas));
                objeto.insert("subtotalCents".into(), json!(subtotal));
                objeto = con_defaults(objeto, json!({"retentionCents": 0}));
                objeto.insert("totalCents".into(), json!(subtotal));
                objeto = con_defaults(objeto, json!({"notes": "", "userName": ""}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "ordenes-compra" => {
                let lineas = objeto.get("lines").cloned().unwrap_or_else(|| json!([]));
                objeto.insert("status".into(), json!("pendiente"));
                objeto.insert("totalCents".into(), json!(total_lineas(&lineas)));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-metodos-pago" => {
                objeto = con_defaults(objeto, json!({"icon": "payments", "active": true}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-movimientos" => {
                let amount = objeto.get("amountCents").cloned().unwrap_or(json!(0));
                objeto = con_defaults(objeto, json!({"currency": "NIO"}));
                objeto
                    .entry("originalAmountCents")
                    .or_insert_with(|| amount.clone());
                objeto.entry("exchangeRate").or_insert_with(|| json!(1));
                objeto = con_defaults(
                    objeto,
                    json!({
                        "category": "otro",
                        "source": "manual",
                        "reference": "",
                        "observations": "",
                    }),
                );
                objeto.insert("status".into(), json!("completado"));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-cierres" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-chica-fondo" => {
                objeto = con_defaults(objeto, json!({"amountCents": 0, "responsible": ""}));
                objeto.insert("updatedAt".into(), json!(ahora));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-chica-movimientos" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-transferencias" => {
                objeto = con_defaults(objeto, json!({"status": "completado"}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "configuracion-documentos" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "clientes" => {
                objeto = con_defaults(
                    objeto,
                    json!({"phone": "", "email": "", "company": "", "source": "manual"}),
                );
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "codigos-barras" => {
                let codigo = objeto
                    .get("code")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|valor| !valor.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        let simbologia = objeto
                            .get("symbology")
                            .and_then(Value::as_str)
                            .unwrap_or("CODE128");
                        generar_codigo(simbologia)
                    });
                objeto.insert("code".into(), json!(codigo));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            _ => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
        }
        Ok(Value::Object(objeto))
    }

    async fn siguiente_numero(
        &self,
        recurso: &str,
        prefijo: &str,
        usuario: &UsuarioContexto,
    ) -> Result<String, ApiError> {
        let registros = self.listar(recurso, usuario, None).await?;
        Ok(format!("{prefijo}-{:06}", registros.len() + 1))
    }
}

// ── utilidades libres ───────────────────────────────────────────────────────

fn a_registro(fila: &sqlx::postgres::PgRow) -> Registro {
    let datos: Value = jsonb(fila, "datos").unwrap_or_else(|_| json!({}));
    let mut objeto = datos.as_object().cloned().unwrap_or_default();
    objeto.insert(
        "id".into(),
        json!(uuid_texto(fila, "id").unwrap_or_default()),
    );
    objeto.insert(
        "createdAt".into(),
        json!(timestamp(fila, "creado_en").unwrap_or_default()),
    );
    objeto.insert(
        "updatedAt".into(),
        json!(timestamp(fila, "actualizado_en").unwrap_or_default()),
    );
    objeto
}

async fn insertar_en(
    tx: &mut Transaction<'_, Postgres>,
    recurso: &str,
    datos: Value,
    ambito: &Ambito,
) -> Result<sqlx::postgres::PgRow, ApiError> {
    sqlx::query(
        "INSERT INTO finance.workspace_registros (tenant_id, propietario_id, recurso, datos)
         VALUES ($1::uuid, $2::uuid, $3, $4::jsonb)
         RETURNING id, datos, creado_en, actualizado_en",
    )
    .bind(ambito.tenant_id.as_deref())
    .bind(ambito.propietario_id.as_deref())
    .bind(recurso)
    .bind(sqlx::types::Json(&datos))
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?
    .ok_or_else(|| ApiError::dominio("REGISTRO_NO_CREADO", "No se pudo crear el registro."))
}

async fn mover_banco(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    delta: i64,
    ambito: &Ambito,
) -> Result<(), ApiError> {
    if delta == 0 {
        return Ok(());
    }
    let uuid = parsear_uuid(id)?;
    let abs = delta.abs();
    let resultado = if delta < 0 {
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = jsonb_set(datos, '{balanceCents}',
                     to_jsonb((datos->>'balanceCents')::bigint - $3)),
                 actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
               AND (datos->>'balanceCents')::bigint >= $3
             RETURNING id",
        )
        .bind(uuid)
        .bind(ambito.tenant_id.as_deref())
        .bind(abs)
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?
    } else {
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = jsonb_set(datos, '{balanceCents}',
                     to_jsonb((datos->>'balanceCents')::bigint + $3)),
                 actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
             RETURNING id",
        )
        .bind(uuid)
        .bind(ambito.tenant_id.as_deref())
        .bind(abs)
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?
    };
    if resultado.is_some() {
        return Ok(());
    }
    let existe = sqlx::query(
        "SELECT 1 FROM finance.workspace_registros
         WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
           AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
           AND ($3::uuid IS NULL OR propietario_id = $3::uuid)",
    )
    .bind(uuid)
    .bind(ambito.tenant_id.as_deref())
    .bind(ambito.propietario_id.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if existe.is_none() {
        return Err(ApiError::no_encontrado("Cuenta bancaria", id));
    }
    Err(ApiError::dominio(
        "SALDO_INSUFICIENTE",
        "La cuenta no tiene saldo suficiente.",
    ))
}

fn con_defaults(mut objeto: Map<String, Value>, defaults: Value) -> Map<String, Value> {
    if let Some(defaults) = defaults.as_object() {
        for (clave, valor) in defaults {
            objeto.entry(clave.clone()).or_insert_with(|| valor.clone());
        }
    }
    objeto
}

pub fn total_lineas(lineas: &Value) -> i64 {
    let Some(lineas) = lineas.as_array() else {
        return 0;
    };
    lineas
        .iter()
        .map(|linea| {
            let precio = numero(linea.get("unitCostCents"));
            let cantidad = numero(linea.get("quantity"));
            (precio as f64 * cantidad as f64).round() as i64
        })
        .sum()
}

pub fn numero(valor: Option<&Value>) -> i64 {
    match valor {
        Some(Value::Number(numero)) => numero
            .as_i64()
            .or_else(|| numero.as_f64().map(|valor| valor.round() as i64))
            .unwrap_or(0),
        Some(Value::String(texto)) => texto.parse().unwrap_or(0),
        _ => 0,
    }
}

pub fn lineas_con_id(lineas: &[LineaCompra]) -> Value {
    Value::Array(
        lineas
            .iter()
            .map(|linea| {
                json!({
                    "id": Uuid::new_v4().to_string(),
                    "productId": linea.product_id,
                    "name": linea.name,
                    "sku": linea.sku.clone().unwrap_or_default(),
                    "quantity": linea.quantity,
                    "unitCostCents": linea.unit_cost_cents,
                    "salePriceCents": linea.sale_price_cents,
                })
            })
            .collect(),
    )
}

pub fn clave_cliente(datos: &Registro) -> String {
    let nombre = datos
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let telefono: String = datos
        .get("phone")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    format!("{nombre}|{telefono}")
}

/// Dígito verificador EAN-13 (pesos 1 y 3 alternados desde la izquierda).
pub fn digito_ean13(base: &str) -> u32 {
    let suma: u32 = base
        .chars()
        .enumerate()
        .filter_map(|(indice, digito)| {
            digito
                .to_digit(10)
                .map(|valor| valor * if indice % 2 == 0 { 1 } else { 3 })
        })
        .sum();
    (10 - (suma % 10)) % 10
}

pub fn generar_codigo(simbologia: &str) -> String {
    if simbologia == "EAN13" {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let base: String = bytes
            .iter()
            .flat_map(|byte| [byte / 10 % 10, byte % 10])
            .take(12)
            .map(|digito| digito.to_string())
            .collect();
        let digito = digito_ean13(&base);
        return format!("{base}{digito}");
    }
    let millis = Utc::now().timestamp_millis() as u64;
    format!("C128-{:X}", millis)
}

pub fn hoy() -> String {
    iso_ms(Utc::now()).chars().take(10).collect()
}

pub fn saldo(registros: &[Registro]) -> i64 {
    registros
        .iter()
        .map(|registro| {
            let monto = numero(registro.get("amountCents"));
            if registro.get("type").and_then(Value::as_str) == Some("ingreso") {
                monto
            } else {
                -monto
            }
        })
        .sum()
}

pub fn suma(registros: &[Registro]) -> i64 {
    registros
        .iter()
        .map(|registro| numero(registro.get("amountCents")))
        .sum()
}

fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::dominio("ID_INVALIDO", "El id debe ser un UUID."))
}

fn validar_texto(valor: &str, campo: &str, minimo: usize) -> Result<(), ApiError> {
    if valor.trim().len() < minimo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": [format!("minLength:{minimo}")]}
        ])));
    }
    Ok(())
}

fn validar_in(valor: &str, campo: &str, permitidos: &[&str]) -> Result<(), ApiError> {
    if !permitidos.contains(&valor) {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isIn"]}
        ])));
    }
    Ok(())
}

fn validar_rango(valor: f64, campo: &str, minimo: f64, maximo: f64) -> Result<(), ApiError> {
    if !valor.is_finite() || valor < minimo || valor > maximo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["min", "max"]}
        ])));
    }
    Ok(())
}

fn validar_fecha(valor: &str, campo: &str) -> Result<(), ApiError> {
    let ok =
        valor.len() >= 10 && chrono::NaiveDate::parse_from_str(&valor[..10], "%Y-%m-%d").is_ok();
    if !ok {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isDateString"]}
        ])));
    }
    Ok(())
}

fn validar_email(valor: &str, campo: &str) -> Result<(), ApiError> {
    let (usuario, dominio) = valor.split_once('@').unwrap_or_default();
    if usuario.is_empty() || !dominio.contains('.') || dominio.starts_with('.') {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isEmail"]}
        ])));
    }
    Ok(())
}

fn validar_lineas(lineas: &[LineaCompra]) -> Result<(), ApiError> {
    for (indice, linea) in lineas.iter().enumerate() {
        if linea.name.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.name"), "restricciones": ["minLength:1"]}
            ])));
        }
        if !linea.quantity.is_finite() || linea.quantity < 1.0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.quantity"), "restricciones": ["min:1"]}
            ])));
        }
        if linea.unit_cost_cents < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.unitCostCents"), "restricciones": ["min:0"]}
            ])));
        }
    }
    Ok(())
}

/// Marca una violación de unicidad de mesa/comanda como conflicto (helper
/// compartido con el resto del servicio).
pub fn conflicto_si_unicidad(error: sqlx::Error, mensaje: &str) -> ApiError {
    if es_violacion_unicidad(&error) {
        ApiError::conflicto(mensaje)
    } else {
        error_db(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json as j;

    fn usuario(rol: &str, tenant: Option<&str>) -> UsuarioContexto {
        UsuarioContexto {
            user_id: "u-1".into(),
            rol: rol.into(),
            tenant_id: tenant.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn debe_resolver_el_ambito_de_cada_rol() {
        let vendedor = Comercio::ambito("clientes", &usuario("vendedor", None), None).unwrap();
        assert_eq!(vendedor.propietario_id.as_deref(), Some("u-1"));
        assert!(Comercio::ambito("gastos", &usuario("vendedor", None), None).is_err());
        let empresario =
            Comercio::ambito("gastos", &usuario("empresario", Some("t-1")), None).unwrap();
        assert_eq!(empresario.tenant_id.as_deref(), Some("t-1"));
        let admin = Comercio::ambito("gastos", &usuario("admin", None), Some("t-9")).unwrap();
        assert_eq!(admin.tenant_id.as_deref(), Some("t-9"));
        assert!(Comercio::ambito("clientes", &usuario("comprador", None), None).is_err());
    }

    #[test]
    fn debe_aplicar_defaults_de_recursos() {
        let normalizado =
            Comercio::normalizar("gastos", j!({"concept": "luz"})).expect("normalizado");
        assert_eq!(normalizado["status"], "pendiente");
        assert_eq!(normalizado["includesVat"], false);
        assert!(normalizado["createdAt"].is_string());

        let credito =
            Comercio::normalizar("creditos", j!({"amountCents": 1000, "customer": "Ana"}))
                .expect("credito");
        assert_eq!(credito["balanceCents"], 1000);
        assert_eq!(credito["date"].as_str().unwrap().len(), 10);
    }

    #[test]
    fn debe_calcular_los_totales_de_compras() {
        let compra = Comercio::normalizar(
            "compras",
            j!({"lines": [
                {"quantity": 2, "unitCostCents": 500},
                {"quantity": 3, "unitCostCents": 1000}
            ]}),
        )
        .expect("compra");
        assert_eq!(compra["subtotalCents"], 4000);
        assert_eq!(compra["totalCents"], 4000);
        assert_eq!(compra["status"], "pendiente");
    }

    #[test]
    fn debe_generar_el_codigo_de_barras_si_falta() {
        let ean = Comercio::normalizar(
            "codigos-barras",
            j!({"productName": "x", "sku": "1", "symbology": "EAN13", "copies": 1}),
        )
        .expect("ean");
        let codigo = ean["code"].as_str().unwrap();
        assert_eq!(codigo.len(), 13);
        assert_eq!(
            codigo.chars().last().unwrap().to_digit(10).unwrap(),
            digito_ean13(&codigo[..12])
        );
        let code128 = Comercio::normalizar(
            "codigos-barras",
            j!({"productName": "x", "sku": "1", "symbology": "CODE128", "copies": 1}),
        )
        .expect("code128");
        assert!(code128["code"].as_str().unwrap().starts_with("C128-"));
    }

    #[test]
    fn debe_calcular_el_digito_ean13() {
        assert_eq!(digito_ean13("400638133393"), 1);
        assert_eq!(digito_ean13("590123412345"), 7);
    }

    #[test]
    fn debe_normalizar_la_clave_de_cliente() {
        let valor = j!({"name": " Ana  Pérez ", "phone": "(505) 8888-1111"});
        let clave = clave_cliente(valor.as_object().expect("objeto"));
        assert_eq!(clave, "ana  pérez|50588881111");
    }

    #[test]
    fn debe_calcular_saldo_de_movimientos() {
        let registros = vec![
            j!({"type": "ingreso", "amountCents": 1000})
                .as_object()
                .cloned()
                .unwrap(),
            j!({"type": "egreso", "amountCents": 400})
                .as_object()
                .cloned()
                .unwrap(),
        ];
        assert_eq!(saldo(&registros), 600);
        assert_eq!(suma(&registros), 1400);
    }

    #[test]
    fn debe_validar_las_lineas_de_compra() {
        let linea = LineaCompra {
            name: "Producto".into(),
            quantity: 1.0,
            unit_cost_cents: 100,
            ..Default::default()
        };
        validar_lineas(&[linea]).expect("linea valida");
        let invalida = LineaCompra {
            name: "".into(),
            quantity: 1.0,
            unit_cost_cents: 100,
            ..Default::default()
        };
        assert!(validar_lineas(&[invalida]).is_err());
    }
}
