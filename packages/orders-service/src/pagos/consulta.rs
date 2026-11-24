use super::comunes::*;
use super::prelude::*;
use super::*;
use serde_json::json;

impl Pagos {
    /// Consulta el estado real de un pago ya cargado. Lo usa el job de
    /// conciliación para no volver a buscar la fila por referencia.
    pub async fn consultar_pago(
        &self,
        pago: &Pago,
        datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        let proveedor = self.proveedor_de_pago(pago).await?;
        proveedor.consultar(referencia_operativa(pago), datos).await
    }

    /// Pagos pendientes con proveedor externo que ya superaron la ventana de
    /// gracia. El job los consulta y decide; nunca confirma por el webhook.
    /// Solo mira los últimos 30 días para acotar links abandonados.
    pub async fn pendientes_de_conciliacion(
        &self,
        minutos_gracia: u64,
        limite: i64,
    ) -> Result<Vec<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT p.id, p.order_id, p.empresa_id, p.metodo_codigo, p.estado, p.monto_cents,
                    p.moneda, p.referencia, p.referencia_proveedor, p.proveedor,
                    p.monto_reembolsado_cents, p.motivo_rechazo, p.confirmado_por,
                    p.conciliado_en, p.confirmado_en, p.creado_en
               FROM orders.pagos p
              WHERE p.estado = 'pendiente'
                AND p.referencia <> ''
                AND p.proveedor NOT IN ('manual')
                AND (
                      EXISTS (
                            SELECT 1
                              FROM orders.proveedores_pago pr
                             WHERE pr.codigo = p.proveedor
                               AND pr.tipo IN ('tilopay', 'pagadito')
                      )
                      OR (
                            p.proveedor = $3
                            AND NOT EXISTS (
                                  SELECT 1
                                    FROM orders.proveedores_pago pr2
                                   WHERE pr2.codigo = p.proveedor
                            )
                      )
                )
                AND p.creado_en < NOW() - ($1::int * INTERVAL '1 minute')
                AND p.creado_en > NOW() - INTERVAL '30 days'
              ORDER BY p.creado_en
              LIMIT $2",
        )
        .bind(minutos_gracia as i32)
        .bind(limite)
        .bind(self.proveedor_default.nombre())
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_pago).collect()
    }

    pub async fn listar_metodos(&self) -> Result<Vec<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_metodo).collect()
    }

    pub async fn listar_por_orden(&self, order_id: &str) -> Result<Vec<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en,
                    confirmado_en, creado_en
               FROM orders.pagos
              WHERE order_id = $1::uuid
              ORDER BY creado_en DESC",
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_pago).collect()
    }

    pub async fn obtener(&self, pago_id: &str) -> Result<Option<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en,
                    confirmado_en, creado_en
               FROM orders.pagos WHERE id = $1::uuid",
        )
        .bind(pago_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_pago).transpose()
    }

    /// Método activo por código; `None` si no existe o está inactivo.
    pub async fn metodo_activo(&self, codigo: &str) -> Result<Option<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago
              WHERE codigo = $1 AND activo = true",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_metodo).transpose()
    }

    pub async fn crear_metodo(&self, datos: &Value) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(campo(datos, "codigo"))?;
        let nombre = validar_nombre(campo(datos, "nombre"))?;
        let requiere_confirmacion = datos
            .get("requiere_confirmacion")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = config_de(datos)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.metodos_pago
               (codigo, nombre, requiere_confirmacion, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(&nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el metodo de pago."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }

    pub async fn actualizar_metodo(
        &self,
        codigo: &str,
        datos: &Value,
    ) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(codigo)?;
        let nombre = match datos.get("nombre") {
            Some(valor) => Some(validar_nombre(valor.as_str().unwrap_or_default())?),
            None => None,
        };
        let requiere_confirmacion = datos.get("requiere_confirmacion").and_then(Value::as_bool);
        let activo = datos.get("activo").and_then(Value::as_bool);
        let config = if datos.get("config").is_some() {
            Some(config_de(datos)?)
        } else {
            None
        };
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.metodos_pago
                SET nombre = COALESCE($2, nombre),
                    requiere_confirmacion = COALESCE($3, requiere_confirmacion),
                    activo = COALESCE($4, activo),
                    config = COALESCE($5::jsonb, config),
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Metodo de pago", &codigo))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }
}

fn fila_metodo(fila: &sqlx::postgres::PgRow) -> Result<MetodoPago, ApiError> {
    Ok(MetodoPago {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        requiere_confirmacion: fila
            .try_get("requiere_confirmacion")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

pub(super) fn campo<'a>(datos: &'a Value, clave: &str) -> &'a str {
    datos.get(clave).and_then(Value::as_str).unwrap_or("")
}

pub(super) fn validar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "codigo", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

pub(super) fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    if !(2..=120).contains(&nombre.chars().count()) {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["isString"]}
        ])));
    }
    Ok(nombre.to_string())
}

pub(super) fn config_de(datos: &Value) -> Result<Value, ApiError> {
    let config = datos.get("config").cloned().unwrap_or_else(|| json!({}));
    if !config.is_object() {
        return Err(ApiError::validacion(json!([
            {"campo": "config", "restricciones": ["isObject"]}
        ])));
    }
    Ok(config)
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if let Some(codigo) = error.as_database_error().and_then(|db| db.code()) {
        if codigo == "23505" {
            return ApiError::conflicto("El metodo de pago ya existe.");
        }
    }
    crate::event_store::error_db(error)
}
