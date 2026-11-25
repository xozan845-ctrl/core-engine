use super::consulta::{campo, validar_codigo};
use super::prelude::*;
use super::proveedores::*;
use super::*;
use serde_json::json;

impl Pagos {
    pub async fn listar_proveedores(&self) -> Result<Vec<ProveedorPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_proveedor).collect()
    }

    pub async fn crear_proveedor(&self, datos: &Value) -> Result<ProveedorPago, ApiError> {
        let codigo = texto(datos, "codigo").to_lowercase();
        validar_codigo_proveedor(&codigo)?;
        let nombre = texto(datos, "nombre");
        if nombre.trim().is_empty() {
            return Err(ApiError::dominio(
                "PROVEEDOR_INVALIDO",
                "El nombre del proveedor es obligatorio.",
            ));
        }
        let tipo = texto(datos, "tipo").to_lowercase();
        validar_tipo_proveedor(&tipo)?;
        let activo = datos
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(tipo == "simulado" || tipo == "manual");
        let config = configuracion_de(datos);
        validar_config_proveedor(&tipo, activo, &config)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.proveedores_pago (codigo, nombre, tipo, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             ON CONFLICT (codigo) DO NOTHING
             RETURNING codigo, nombre, tipo, activo, config",
        )
        .bind(&codigo)
        .bind(nombre.trim())
        .bind(&tipo)
        .bind(activo)
        .bind(config.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::conflicto("El proveedor de pago ya existe."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_proveedor(&fila)
    }

    pub async fn actualizar_proveedor(
        &self,
        codigo: &str,
        datos: &Value,
    ) -> Result<ProveedorPago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let actual = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1 FOR UPDATE",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(actual) = actual else {
            return Err(ApiError::no_encontrado("Proveedor de pago", codigo));
        };
        let actual = fila_proveedor(&actual)?;
        let tipo = texto(datos, "tipo").to_lowercase();
        let tipo = if tipo.is_empty() {
            actual.tipo.clone()
        } else {
            tipo
        };
        validar_tipo_proveedor(&tipo)?;
        let nombre = datos
            .get("nombre")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(&actual.nombre)
            .to_string();
        let activo = datos
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(actual.activo);
        let config = fusionar_config(&actual.config, &configuracion_de(datos));
        validar_config_proveedor(&tipo, activo, &config)?;
        let fila = sqlx::query(
            "UPDATE orders.proveedores_pago
                SET nombre = $2, tipo = $3, activo = $4, config = $5::jsonb,
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, tipo, activo, config",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(tipo)
        .bind(activo)
        .bind(config.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_proveedor(&fila)
    }

    pub async fn configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<ConfiguracionPagos>, ApiError> {
        if uuid::Uuid::parse_str(empresa_id).is_err() {
            return Ok(None);
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT empresa_id, negocio_id, proveedor_codigo, activo, config
               FROM orders.configuracion_pagos
              WHERE empresa_id = $1::uuid
                AND ($2::uuid IS NULL OR negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC
              LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_configuracion).transpose()
    }

    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        datos: &Value,
    ) -> Result<ConfiguracionPagos, ApiError> {
        if uuid::Uuid::parse_str(empresa_id).is_err() {
            return Err(ApiError::dominio(
                "EMPRESA_INVALIDA",
                "El id de empresa no es un UUID valido.",
            ));
        }
        let proveedor_codigo = texto(datos, "proveedor_codigo").to_lowercase();
        if proveedor_codigo.is_empty() {
            return Err(ApiError::dominio(
                "PROVEEDOR_REQUERIDO",
                "Selecciona un proveedor de pago.",
            ));
        }
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = configuracion_de(datos);
        validar_config_tenant(&config)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let proveedor = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1",
        )
        .bind(&proveedor_codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(proveedor) = proveedor else {
            return Err(ApiError::no_encontrado(
                "Proveedor de pago",
                &proveedor_codigo,
            ));
        };
        let proveedor = fila_proveedor(&proveedor)?;
        if activo && !proveedor.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "El proveedor de pago no esta activo.",
            ));
        }
        validar_config_proveedor(
            &proveedor.tipo,
            activo,
            &fusionar_config_tenant(&proveedor.config, &config),
        )?;
        let fila = if negocio_id.is_some() {
            sqlx::query(
                "INSERT INTO orders.configuracion_pagos
                   (empresa_id, negocio_id, proveedor_codigo, activo, config)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5::jsonb)
                 ON CONFLICT (empresa_id, negocio_id) WHERE negocio_id IS NOT NULL
                 DO UPDATE SET proveedor_codigo = EXCLUDED.proveedor_codigo,
                               activo = EXCLUDED.activo,
                               config = EXCLUDED.config,
                               actualizado_en = NOW()
                 RETURNING empresa_id, negocio_id, proveedor_codigo, activo, config",
            )
            .bind(empresa_id)
            .bind(negocio_id)
            .bind(&proveedor_codigo)
            .bind(activo)
            .bind(config.to_string())
            .fetch_one(&mut *tx)
            .await
        } else {
            sqlx::query(
                "INSERT INTO orders.configuracion_pagos
                   (empresa_id, negocio_id, proveedor_codigo, activo, config)
                 VALUES ($1::uuid, NULL, $2, $3, $4::jsonb)
                 ON CONFLICT (empresa_id) WHERE negocio_id IS NULL
                 DO UPDATE SET proveedor_codigo = EXCLUDED.proveedor_codigo,
                               activo = EXCLUDED.activo,
                               config = EXCLUDED.config,
                               actualizado_en = NOW()
                 RETURNING empresa_id, negocio_id, proveedor_codigo, activo, config",
            )
            .bind(empresa_id)
            .bind(&proveedor_codigo)
            .bind(activo)
            .bind(config.to_string())
            .fetch_one(&mut *tx)
            .await
        }
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_configuracion(&fila)
    }

    pub(super) async fn proveedor_para(
        &self,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<(String, Arc<dyn ProveedorPagos>), ApiError> {
        let Some(empresa_id) = empresa_id else {
            return Ok((
                self.proveedor_default.nombre(),
                Arc::clone(&self.proveedor_default),
            ));
        };
        let Some(config) = self.configuracion(empresa_id, negocio_id).await? else {
            return Ok((
                self.proveedor_default.nombre(),
                Arc::clone(&self.proveedor_default),
            ));
        };
        if !config.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "La configuracion de pagos de la empresa esta inactiva.",
            ));
        }
        let proveedor = self
            .proveedor_por_codigo(empresa_id, negocio_id, &config.proveedor_codigo)
            .await?;
        Ok((config.proveedor_codigo, proveedor))
    }

    pub(super) async fn proveedor_por_codigo(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        codigo: &str,
    ) -> Result<Arc<dyn ProveedorPagos>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Proveedor de pago", codigo));
        };
        let proveedor = fila_proveedor(&fila)?;
        if !proveedor.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "El proveedor de pago no esta activo.",
            ));
        }
        if proveedor.tipo == "simulado" {
            return Ok(Arc::new(ProveedorSimulado));
        }
        if proveedor.tipo == "manual" {
            return Ok(Arc::new(ProveedorManual));
        }
        let tenant_config = self
            .configuracion(empresa_id, negocio_id)
            .await?
            .filter(|config| config.proveedor_codigo == codigo)
            .map(|config| config.config)
            .unwrap_or_else(|| json!({}));
        let config = fusionar_config_tenant(&proveedor.config, &tenant_config);
        if proveedor.tipo == "tilopay" {
            return Ok(Arc::new(Tilopay::nuevo(&config)?));
        }
        if proveedor.tipo == "pagadito" {
            return Ok(Arc::new(Pagadito::nuevo(&config)?));
        }
        let Some(url) = config
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
        else {
            if codigo == self.proveedor_default.nombre() {
                return Ok(Arc::clone(&self.proveedor_default));
            }
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INCOMPLETO",
                "El proveedor HTTP requiere config.url.",
            ));
        };
        let clave = config
            .get("clave_env")
            .and_then(Value::as_str)
            .and_then(|variable| std::env::var(variable).ok());
        Ok(Arc::new(ProveedorHttp::nuevo(url.to_string(), clave)))
    }
}

fn fila_proveedor(fila: &sqlx::postgres::PgRow) -> Result<ProveedorPago, ApiError> {
    Ok(ProveedorPago {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        tipo: fila.try_get("tipo").map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

fn fila_configuracion(fila: &sqlx::postgres::PgRow) -> Result<ConfiguracionPagos, ApiError> {
    Ok(ConfiguracionPagos {
        empresa_id: uuid_texto(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        negocio_id: uuid_opcional(fila, "negocio_id").map_err(crate::event_store::error_db)?,
        proveedor_codigo: fila
            .try_get("proveedor_codigo")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

fn texto(datos: &Value, clave: &str) -> String {
    campo(datos, clave).to_string()
}

fn fusionar_config(base: &Value, extra: &Value) -> Value {
    let mut salida = base.as_object().cloned().unwrap_or_default();
    if let Some(extra) = extra.as_object() {
        for (clave, valor) in extra {
            salida.insert(clave.clone(), valor.clone());
        }
    }
    Value::Object(salida)
}

/// Claves que solo puede definir la plataforma (admin), nunca el tenant:
/// permitir que el tenant las sobrescriba convierte la configuración de pagos
/// en un SSRF con exfiltración de variables de entorno.
const CONFIG_RESERVADA: [&str; 14] = [
    "url",
    "base_url",
    "clave_env",
    "apiuser_env",
    "password_env",
    "key_env",
    "uid_env",
    "wsk_env",
    "webhook_secreto_env",
    "webhook_url",
    "callback_url",
    "return_url",
    "ambiente",
    "liquidacion_unidad",
];

/// Fusiona la config del proveedor con la del tenant, ignorando las claves
/// reservadas en la parte del tenant.
pub(super) fn fusionar_config_tenant(base: &Value, extra: &Value) -> Value {
    let mut salida = base.as_object().cloned().unwrap_or_default();
    if let Some(extra) = extra.as_object() {
        for (clave, valor) in extra {
            if !CONFIG_RESERVADA.contains(&clave.as_str()) {
                salida.insert(clave.clone(), valor.clone());
            }
        }
    }
    Value::Object(salida)
}

pub(super) fn validar_config_tenant(config: &Value) -> Result<(), ApiError> {
    if let Some(objeto) = config.as_object() {
        for clave in CONFIG_RESERVADA {
            if objeto.contains_key(clave) {
                return Err(ApiError::validacion(json!([
                    {"campo": format!("config.{clave}"), "restricciones": ["isReserved"]}
                ])));
            }
        }
    }
    Ok(())
}

fn configuracion_de(datos: &Value) -> Value {
    datos
        .get("config")
        .filter(|valor| valor.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn validar_tipo_proveedor(tipo: &str) -> Result<(), ApiError> {
    if ["simulado", "http", "manual", "tilopay", "pagadito"].contains(&tipo) {
        Ok(())
    } else {
        Err(ApiError::validacion(json!([
            {"campo": "tipo", "restricciones": ["isIn"]}
        ])))
    }
}

fn validar_codigo_proveedor(codigo: &str) -> Result<(), ApiError> {
    validar_codigo(codigo).map(|_| ())
}

fn validar_config_proveedor(tipo: &str, activo: bool, config: &Value) -> Result<(), ApiError> {
    if !config.is_object() {
        return Err(ApiError::validacion(json!([
            {"campo": "config", "restricciones": ["isObject"]}
        ])));
    }
    if tipo == "http" && activo {
        let url = config.get("url").and_then(Value::as_str).unwrap_or("");
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INCOMPLETO",
                "Un proveedor HTTP activo requiere config.url http(s).",
            ));
        }
    }
    for clave in ["clave_env", "webhook_secreto_env"] {
        if let Some(valor) = config.get(clave) {
            if !valor.is_string() {
                return Err(ApiError::validacion(json!([
                    {"campo": clave, "restricciones": ["isString"]}
                ])));
            }
        }
    }
    Ok(())
}
