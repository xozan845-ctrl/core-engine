//! Dispersión diaria: cierre del día, ledger de saldos y pago al negocio y al
//! vendedor (ADR-0053).
//!
//! La plataforma cobra en su cuenta. `orders-service` registra el fee de
//! pasarela como costo de la plataforma y `commissions-service` reparte el
//! neto. El cierre diario consolida las comisiones acreditadas en
//! liquidaciones por beneficiario y crea un payout por cada uno. El payout se
//! envía al proveedor configurado (Tilopay o Pagadito) y solo al confirmarse
//! se marcan las liquidaciones como pagadas.

use crate::db::{centavos_i32, error_db, parsear_uuid, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::liquidaciones::{LiquidacionesPort, Periodo};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

pub(super) const PROVEEDORES_DISPERSION: [&str; 2] = ["tilopay", "pagadito"];

mod modelos;
mod proveedor;

pub use modelos::*;
pub use proveedor::{DispersionHttp, DispersionNoDisponible, ProveedorDispersion};

pub struct Dispersion {
    db: Arc<Db>,
    liquidaciones: Arc<dyn LiquidacionesPort>,
}

impl Dispersion {
    pub fn nueva(db: Arc<Db>, liquidaciones: Arc<dyn LiquidacionesPort>) -> Self {
        Self { db, liquidaciones }
    }

    pub async fn configuracion(&self) -> Result<ConfiguracionDispersion, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "SELECT proveedor_codigo, activo, hora_local, minimo_cents, moneda, retencion_bps
               FROM commissions.configuracion_dispersion WHERE id = 1",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        ConfiguracionDispersion::desde_fila(&fila).map_err(error_db)
    }

    pub async fn guardar_configuracion(
        &self,
        datos: &Value,
    ) -> Result<ConfiguracionDispersion, ApiError> {
        let actual = self.configuracion().await?;
        let proveedor = texto(datos, "proveedor_codigo")
            .unwrap_or(actual.proveedor_codigo)
            .to_lowercase();
        if !PROVEEDORES_DISPERSION.contains(&proveedor.as_str()) {
            return Err(ApiError::validacion(json!([
                {"campo": "proveedor_codigo", "restricciones": ["isIn"]}
            ])));
        }
        let activo = datos
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(actual.activo);
        let hora = texto(datos, "hora_local").unwrap_or(actual.hora_local);
        if !hora_valida(&hora) {
            return Err(ApiError::validacion(json!([
                {"campo": "hora_local", "restricciones": ["matches"]}
            ])));
        }
        let minimo = datos
            .get("minimo_cents")
            .and_then(Value::as_i64)
            .unwrap_or(actual.minimo_cents);
        if minimo < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "minimo_cents", "restricciones": ["min:0"]}
            ])));
        }
        let moneda = texto(datos, "moneda")
            .unwrap_or(actual.moneda)
            .to_uppercase();
        if !moneda_valida(&moneda) {
            return Err(ApiError::validacion(json!([
                {"campo": "moneda", "restricciones": ["matches"]}
            ])));
        }
        let retencion = datos
            .get("retencion_bps")
            .and_then(Value::as_i64)
            .unwrap_or(actual.retencion_bps);
        if !(0..=10_000).contains(&retencion) {
            return Err(ApiError::validacion(json!([
                {"campo": "retencion_bps", "restricciones": ["min:0", "max:10000"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE commissions.configuracion_dispersion
                SET proveedor_codigo = $1, activo = $2, hora_local = $3,
                    minimo_cents = $4, moneda = $5, retencion_bps = $6,
                    actualizado_en = NOW()
              WHERE id = 1
              RETURNING proveedor_codigo, activo, hora_local, minimo_cents, moneda, retencion_bps",
        )
        .bind(&proveedor)
        .bind(activo)
        .bind(&hora)
        .bind(centavos_i32(minimo)?)
        .bind(&moneda)
        .bind(i32::try_from(retencion).unwrap_or(10_000))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        ConfiguracionDispersion::desde_fila(&fila).map_err(error_db)
    }

    pub async fn listar_cuentas(&self) -> Result<Vec<CuentaPago>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, beneficiario_tipo, beneficiario_id, proveedor,
                    referencia_externa, moneda, activo
               FROM commissions.cuentas_pago ORDER BY beneficiario_tipo, creado_en",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(CuentaPago::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }

    pub async fn guardar_cuenta(&self, datos: &Value) -> Result<CuentaPago, ApiError> {
        let tipo = texto(datos, "beneficiario_tipo").unwrap_or_default();
        if !["empresa", "negocio", "vendedor"].contains(&tipo.as_str()) {
            return Err(ApiError::validacion(json!([
                {"campo": "beneficiario_tipo", "restricciones": ["isIn"]}
            ])));
        }
        let beneficiario = texto(datos, "beneficiario_id").unwrap_or_default();
        let beneficiario_uuid = parsear_uuid(&beneficiario)?;
        let proveedor = texto(datos, "proveedor").unwrap_or_default().to_lowercase();
        if !PROVEEDORES_DISPERSION.contains(&proveedor.as_str()) {
            return Err(ApiError::validacion(json!([
                {"campo": "proveedor", "restricciones": ["isIn"]}
            ])));
        }
        let referencia = texto(datos, "referencia_externa").unwrap_or_default();
        if referencia.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "referencia_externa", "restricciones": ["isNotEmpty"]}
            ])));
        }
        let moneda = texto(datos, "moneda")
            .unwrap_or_else(|| "NIO".to_string())
            .to_uppercase();
        if !moneda_valida(&moneda) {
            return Err(ApiError::validacion(json!([
                {"campo": "moneda", "restricciones": ["matches"]}
            ])));
        }
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let detalle = datos
            .get("detalle")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO commissions.cuentas_pago
               (beneficiario_tipo, beneficiario_id, proveedor, referencia_externa,
                moneda, detalle, activo)
             VALUES ($1, $2::uuid, $3, $4, $5, $6::jsonb, $7)
             ON CONFLICT (beneficiario_tipo, beneficiario_id, proveedor)
             DO UPDATE SET referencia_externa = EXCLUDED.referencia_externa,
                           moneda = EXCLUDED.moneda,
                           detalle = EXCLUDED.detalle,
                           activo = EXCLUDED.activo,
                           actualizado_en = NOW()
             RETURNING id, beneficiario_tipo, beneficiario_id, proveedor,
                       referencia_externa, moneda, activo",
        )
        .bind(&tipo)
        .bind(beneficiario_uuid)
        .bind(&proveedor)
        .bind(&referencia)
        .bind(&moneda)
        .bind(detalle.to_string())
        .bind(activo)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        CuentaPago::desde_fila(&fila).map_err(error_db)
    }

    pub async fn listar_payouts(
        &self,
        fecha: Option<NaiveDate>,
        estado: Option<&str>,
        beneficiario: Option<(&str, &str)>,
    ) -> Result<Vec<Payout>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let (beneficiario_tipo, beneficiario_id) = match beneficiario {
            Some((tipo, id)) => (Some(tipo), Some(parsear_uuid(id)?.to_string())),
            None => (None, None),
        };
        let filas = sqlx::query(
            "SELECT id, fecha, beneficiario_tipo, beneficiario_id, empresa_id, negocio_id,
                    monto_cents, retencion_cents, monto_neto_cents, moneda, proveedor,
                    estado, referencia_externa, error, creado_en
               FROM commissions.payouts
              WHERE ($1::date IS NULL OR fecha = $1::date)
                AND ($2::text IS NULL OR estado = $2::text)
                AND ($3::text IS NULL OR beneficiario_tipo = $3::text)
                AND ($4::uuid IS NULL OR beneficiario_id = $4::uuid)
              ORDER BY fecha DESC, creado_en DESC LIMIT 500",
        )
        .bind(fecha)
        .bind(estado)
        .bind(beneficiario_tipo)
        .bind(beneficiario_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(Payout::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }

    /// Cierra el día y crea los payouts pendientes. Es idempotente: el cierre
    /// de liquidaciones no duplica y cada payout tiene unique por beneficiario
    /// y fecha.
    /// Payouts de una empresa/negocio para el portal del empresario.
    pub async fn listar_payouts_empresa(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Vec<Payout>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, fecha, beneficiario_tipo, beneficiario_id, empresa_id, negocio_id,
                    monto_cents, retencion_cents, monto_neto_cents, moneda, proveedor,
                    estado, referencia_externa, error, creado_en
               FROM commissions.payouts
              WHERE empresa_id = $1::uuid
                AND ($2::uuid IS NULL OR negocio_id = $2::uuid)
              ORDER BY fecha DESC, creado_en DESC LIMIT 200",
        )
        .bind(parsear_uuid(empresa_id)?)
        .bind(negocio_id.map(parsear_uuid).transpose()?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(Payout::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }

    /// Payouts de un vendedor para su estado de cuenta.
    pub async fn listar_payouts_vendedor(
        &self,
        vendedor_id: &str,
    ) -> Result<Vec<Payout>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, fecha, beneficiario_tipo, beneficiario_id, empresa_id, negocio_id,
                    monto_cents, retencion_cents, monto_neto_cents, moneda, proveedor,
                    estado, referencia_externa, error, creado_en
               FROM commissions.payouts
              WHERE beneficiario_tipo = 'vendedor' AND beneficiario_id = $1::uuid
              ORDER BY fecha DESC, creado_en DESC LIMIT 200",
        )
        .bind(parsear_uuid(vendedor_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(Payout::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }

    pub async fn cerrar_dia(&self, fecha: NaiveDate) -> Result<ResumenDia, ApiError> {
        let liquidaciones = self
            .liquidaciones
            .cerrar_periodo(Periodo {
                inicio: fecha.format("%Y-%m-%d").to_string(),
                fin: fecha.format("%Y-%m-%d").to_string(),
            })
            .await?;
        let config = self.configuracion().await?;
        let grupos = self.grupos_pendientes(fecha).await?;
        let mut resumen = ResumenDia {
            liquidaciones_cerradas: liquidaciones.len() as u64,
            ..ResumenDia::default()
        };
        for grupo in grupos {
            if grupo.monto_cents <= 0 || grupo.monto_cents < config.minimo_cents {
                continue;
            }
            if self
                .crear_payout(&grupo, fecha, &config, &mut resumen)
                .await?
            {
                resumen.payouts_creados += 1;
            }
        }
        Ok(resumen)
    }

    async fn grupos_pendientes(&self, fecha: NaiveDate) -> Result<Vec<GrupoPago>, ApiError> {
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT tipo, beneficiario_id, empresa_id, negocio_id, liquidacion_id, monto_cents
               FROM (
                     SELECT 'vendedor'::text AS tipo, l.vendedor_id AS beneficiario_id,
                            NULL::uuid AS empresa_id, NULL::uuid AS negocio_id,
                            l.id AS liquidacion_id, l.monto_cents
                       FROM commissions.liquidaciones l
                      WHERE l.estado = 'aprobada' AND l.periodo_fin <= $1::date
                        AND NOT EXISTS (
                              SELECT 1 FROM commissions.payout_items pi
                               WHERE pi.liquidacion_tipo = 'vendedor'
                                 AND pi.liquidacion_id = l.id
                        )
                     UNION ALL
                     SELECT CASE WHEN l.negocio_id IS NULL THEN 'empresa' ELSE 'negocio' END,
                            CASE WHEN l.negocio_id IS NULL THEN l.empresa_id ELSE l.negocio_id END,
                            l.empresa_id, l.negocio_id, l.id, l.monto_cents
                       FROM commissions.liquidaciones_empresas l
                      WHERE l.estado = 'aprobada' AND l.periodo_fin <= $1::date
                        AND NOT EXISTS (
                              SELECT 1 FROM commissions.payout_items pi
                               WHERE pi.liquidacion_tipo = 'empresa'
                                 AND pi.liquidacion_id = l.id
                        )
               ) pendientes
              ORDER BY tipo, beneficiario_id, liquidacion_id",
        )
        .bind(fecha)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        let mut grupos: Vec<GrupoPago> = Vec::new();
        for fila in filas {
            let tipo: String = fila.try_get("tipo").map_err(error_db)?;
            let beneficiario = uuid_texto(&fila, "beneficiario_id").map_err(error_db)?;
            let empresa = uuid_opcional(&fila, "empresa_id").map_err(error_db)?;
            let negocio = uuid_opcional(&fila, "negocio_id").map_err(error_db)?;
            let liquidacion = uuid_texto(&fila, "liquidacion_id").map_err(error_db)?;
            let monto = i64::from(fila.try_get::<i32, _>("monto_cents").map_err(error_db)?);
            if let Some(grupo) = grupos
                .iter_mut()
                .find(|grupo| grupo.tipo == tipo && grupo.beneficiario_id == beneficiario)
            {
                grupo.monto_cents += monto;
                grupo.items.push(PayoutItem {
                    liquidacion_tipo: if tipo == "vendedor" {
                        "vendedor".to_string()
                    } else {
                        "empresa".to_string()
                    },
                    liquidacion_id: liquidacion,
                    monto_cents: monto,
                });
            } else {
                let liquidacion_tipo = if tipo == "vendedor" {
                    "vendedor".to_string()
                } else {
                    "empresa".to_string()
                };
                grupos.push(GrupoPago {
                    tipo,
                    beneficiario_id: beneficiario,
                    empresa_id: empresa,
                    negocio_id: negocio,
                    monto_cents: monto,
                    items: vec![PayoutItem {
                        liquidacion_tipo,
                        liquidacion_id: liquidacion,
                        monto_cents: monto,
                    }],
                });
            }
        }
        Ok(grupos)
    }

    async fn crear_payout(
        &self,
        grupo: &GrupoPago,
        fecha: NaiveDate,
        config: &ConfiguracionDispersion,
        _resumen: &mut ResumenDia,
    ) -> Result<bool, ApiError> {
        let retencion = if config.retencion_bps > 0 {
            (grupo.monto_cents.saturating_mul(config.retencion_bps) + 5_000) / 10_000
        } else {
            0
        };
        let neto = (grupo.monto_cents - retencion).max(0);
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO commissions.payouts
               (fecha, beneficiario_tipo, beneficiario_id, empresa_id, negocio_id,
                monto_cents, retencion_cents, monto_neto_cents, moneda, proveedor, estado)
             VALUES ($1::date, $2, $3::uuid, $4::uuid, $5::uuid, $6, $7, $8, $9, $10, 'pendiente')
             ON CONFLICT (beneficiario_tipo, beneficiario_id, fecha) DO NOTHING
             RETURNING id",
        )
        .bind(fecha)
        .bind(&grupo.tipo)
        .bind(parsear_uuid(&grupo.beneficiario_id)?)
        .bind(grupo.empresa_id.as_deref().map(parsear_uuid).transpose()?)
        .bind(grupo.negocio_id.as_deref().map(parsear_uuid).transpose()?)
        .bind(centavos_i32(grupo.monto_cents)?)
        .bind(centavos_i32(retencion)?)
        .bind(centavos_i32(neto)?)
        .bind(&config.moneda)
        .bind(&config.proveedor_codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            tx.commit().await.map_err(error_db)?;
            return Ok(false);
        };
        let payout_id: uuid::Uuid = fila.try_get("id").map_err(error_db)?;
        for item in &grupo.items {
            sqlx::query(
                "INSERT INTO commissions.payout_items
                   (payout_id, liquidacion_tipo, liquidacion_id, monto_cents)
                 VALUES ($1::uuid, $2, $3::uuid, $4)
                 ON CONFLICT (payout_id, liquidacion_tipo, liquidacion_id) DO NOTHING",
            )
            .bind(payout_id)
            .bind(&item.liquidacion_tipo)
            .bind(parsear_uuid(&item.liquidacion_id)?)
            .bind(centavos_i32(item.monto_cents)?)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(true)
    }

    pub async fn enviar_pendientes(&self) -> Result<ResumenDia, ApiError> {
        let config = self.configuracion().await?;
        if !config.activo {
            return Ok(ResumenDia::default());
        }
        let proveedor = DispersionHttp::nuevo(&config.proveedor_codigo)?;
        let mut payouts = self
            .listar_payouts(None, Some("pendiente"), None)
            .await?;
        payouts.extend(self.listar_payouts(None, Some("fallido"), None).await?);
        let mut resumen = ResumenDia::default();
        for payout in payouts {
            match self.enviar_con(&payout.id, &proveedor).await {
                Ok(_) => resumen.payouts_enviados += 1,
                Err(error) => {
                    resumen.payouts_fallidos += 1;
                    crate::logs::warn(
                        "commissions.dispersion",
                        crate::campos! {
                            "msg": "payout fallido",
                            "payout_id": payout.id,
                            "codigo": error.codigo,
                            "err": error.mensaje,
                        },
                    );
                }
            }
        }
        Ok(resumen)
    }

    pub async fn enviar(&self, payout_id: &str) -> Result<Payout, ApiError> {
        let config = self.configuracion().await?;
        let proveedor = DispersionHttp::nuevo(&config.proveedor_codigo)?;
        self.enviar_con(payout_id, &proveedor).await
    }

    async fn enviar_con(
        &self,
        payout_id: &str,
        proveedor: &dyn ProveedorDispersion,
    ) -> Result<Payout, ApiError> {
        let payout = self
            .obtener_payout(payout_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Payout", payout_id))?;
        if !matches!(payout.estado.as_str(), "pendiente" | "fallido") {
            return Err(ApiError::conflicto(format!(
                "El payout está en estado {}.",
                payout.estado
            )));
        }
        let cuenta = self
            .cuenta_para(
                &payout.beneficiario_tipo,
                &payout.beneficiario_id,
                &payout.proveedor,
            )
            .await?
            .ok_or_else(|| {
                ApiError::dominio(
                    "CUENTA_PAGO_NO_CONFIGURADA",
                    "El beneficiario no tiene cuenta de pago activa para este proveedor.",
                )
            })?;
        match proveedor.enviar(&payout, &cuenta).await {
            Ok(referencia) => {
                let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
                let fila = sqlx::query(
                    "UPDATE commissions.payouts
                        SET estado = 'enviado', referencia_externa = $2, error = '',
                            actualizado_en = NOW()
                      WHERE id = $1::uuid
                      RETURNING id, fecha, beneficiario_tipo, beneficiario_id, empresa_id,
                                negocio_id, monto_cents, retencion_cents, monto_neto_cents,
                                moneda, proveedor, estado, referencia_externa, error, creado_en",
                )
                .bind(parsear_uuid(payout_id)?)
                .bind(referencia)
                .fetch_one(&mut *tx)
                .await
                .map_err(error_db)?;
                tx.commit().await.map_err(error_db)?;
                Payout::desde_fila(&fila).map_err(error_db)
            }
            Err(error) => {
                let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
                sqlx::query(
                    "UPDATE commissions.payouts
                        SET estado = 'fallido', error = $2, actualizado_en = NOW()
                      WHERE id = $1::uuid",
                )
                .bind(parsear_uuid(payout_id)?)
                .bind(&error.mensaje)
                .execute(&mut *tx)
                .await
                .map_err(error_db)?;
                tx.commit().await.map_err(error_db)?;
                Err(error)
            }
        }
    }

    /// Confirma un payout enviado. Marca las liquidaciones incluidas como
    /// pagadas. Puede usarlo el webhook del proveedor o un admin.
    pub async fn confirmar(
        &self,
        payout_id: &str,
        referencia: Option<&str>,
    ) -> Result<Payout, ApiError> {
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE commissions.payouts
                SET estado = 'confirmado',
                    referencia_externa = COALESCE(NULLIF($2, ''), referencia_externa),
                    error = '', actualizado_en = NOW()
              WHERE id = $1::uuid AND estado IN ('pendiente', 'enviado')
              RETURNING id, fecha, beneficiario_tipo, beneficiario_id, empresa_id,
                        negocio_id, monto_cents, retencion_cents, monto_neto_cents,
                        moneda, proveedor, estado, referencia_externa, error, creado_en",
        )
        .bind(parsear_uuid(payout_id)?)
        .bind(referencia.unwrap_or_default())
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::conflicto("El payout no existe o no está pendiente/enviado."))?;
        let payout = Payout::desde_fila(&fila).map_err(error_db)?;
        let items = sqlx::query(
            "SELECT liquidacion_tipo, liquidacion_id
               FROM commissions.payout_items WHERE payout_id = $1::uuid",
        )
        .bind(parsear_uuid(payout_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        for item in items {
            let tipo: String = item.try_get("liquidacion_tipo").map_err(error_db)?;
            let id: uuid::Uuid = item.try_get("liquidacion_id").map_err(error_db)?;
            let sql = if tipo == "vendedor" {
                "UPDATE commissions.liquidaciones SET estado = 'pagada' WHERE id = $1"
            } else {
                "UPDATE commissions.liquidaciones_empresas SET estado = 'pagada' WHERE id = $1"
            };
            sqlx::query(sql)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(error_db)?;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(payout)
    }

    pub async fn marcar_fallido(&self, payout_id: &str, motivo: &str) -> Result<Payout, ApiError> {
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE commissions.payouts
                SET estado = 'fallido', error = $2, actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING id, fecha, beneficiario_tipo, beneficiario_id, empresa_id,
                        negocio_id, monto_cents, retencion_cents, monto_neto_cents,
                        moneda, proveedor, estado, referencia_externa, error, creado_en",
        )
        .bind(parsear_uuid(payout_id)?)
        .bind(motivo)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Payout::desde_fila(&fila).map_err(error_db)
    }

    async fn obtener_payout(&self, payout_id: &str) -> Result<Option<Payout>, ApiError> {
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let fila = sqlx::query(
            "SELECT id, fecha, beneficiario_tipo, beneficiario_id, empresa_id, negocio_id,
                    monto_cents, retencion_cents, monto_neto_cents, moneda, proveedor,
                    estado, referencia_externa, error, creado_en
               FROM commissions.payouts WHERE id = $1::uuid",
        )
        .bind(parsear_uuid(payout_id)?)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref()
            .map(Payout::desde_fila)
            .transpose()
            .map_err(error_db)
    }

    async fn cuenta_para(
        &self,
        tipo: &str,
        beneficiario_id: &str,
        proveedor: &str,
    ) -> Result<Option<CuentaPago>, ApiError> {
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        let fila = sqlx::query(
            "SELECT id, beneficiario_tipo, beneficiario_id, proveedor,
                    referencia_externa, moneda, activo
               FROM commissions.cuentas_pago
              WHERE beneficiario_tipo = $1 AND beneficiario_id = $2::uuid
                AND proveedor = $3 AND activo = true",
        )
        .bind(tipo)
        .bind(parsear_uuid(beneficiario_id)?)
        .bind(proveedor)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref()
            .map(CuentaPago::desde_fila)
            .transpose()
            .map_err(error_db)
    }
}

struct GrupoPago {
    tipo: String,
    beneficiario_id: String,
    empresa_id: Option<String>,
    negocio_id: Option<String>,
    monto_cents: i64,
    items: Vec<PayoutItem>,
}

fn texto(datos: &Value, clave: &str) -> Option<String> {
    datos
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
}

fn hora_valida(valor: &str) -> bool {
    if valor.len() != 5 {
        return false;
    }
    let Some((hora, minuto)) = valor.split_once(':') else {
        return false;
    };
    matches!(hora.parse::<u32>(), Ok(0..=23)) && matches!(minuto.parse::<u32>(), Ok(0..=59))
}

fn moneda_valida(valor: &str) -> bool {
    valor.len() == 3 && valor.chars().all(|caracter| caracter.is_ascii_uppercase())
}

#[allow(dead_code)]
fn _tipos(_tx: &mut Transaction<'_, Postgres>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_hora_y_moneda() {
        assert!(hora_valida("00:10"));
        assert!(hora_valida("23:59"));
        assert!(!hora_valida("24:00"));
        assert!(!hora_valida("0:10"));
        assert!(moneda_valida("NIO"));
        assert!(!moneda_valida("nio"));
    }

    #[test]
    fn debe_calcular_retencion_sobre_el_monto() {
        let monto = 10_000_i64;
        let retencion = monto.saturating_mul(150) / 10_000;
        assert_eq!(retencion, 150);
        assert_eq!(monto - retencion, 9_850);
    }
}
