//! Partes universales: personas/organizaciones, contactos, direcciones y
//! relaciones (ADR-0001/R-AR-5).
//!
//! Réplica de `partes/partes.service.ts`: aislamiento por empresa, borrado
//! lógico y validación de metadata/roles de escritura.

use crate::auth::UsuarioContexto;
use crate::db::{iso_ms, Db};
use crate::dominio::slug::es_uuid;
use crate::errors::ApiError;
use chrono::Utc;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;

const ROLES_ESCRITURA: [&str; 5] = [
    "empresario",
    "vendedor",
    "coordinador",
    "supervisor",
    "operativo",
];
const TIPOS_PARTE: [&str; 2] = ["persona", "organizacion"];
const ESTADOS_PARTE: [&str; 3] = ["activo", "inactivo", "archivado"];
const TIPOS_CONTACTO: [&str; 5] = ["email", "telefono", "whatsapp", "web", "otro"];
const TIPOS_DIRECCION: [&str; 5] = ["fiscal", "comercial", "entrega", "operativa", "otro"];

pub struct Partes {
    db: Arc<Db>,
}

impl Partes {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        q: Option<&str>,
        tipo: Option<&str>,
        limite: Option<i64>,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        let mut condiciones = vec![
            "p.empresa_id = $1::uuid".to_string(),
            "p.eliminado_en IS NULL".to_string(),
        ];
        let mut parametros: Vec<String> = vec![empresa_id.to_string()];
        if let Some(tipo) = tipo {
            if !TIPOS_PARTE.contains(&tipo) {
                return Err(ApiError::dominio(
                    "TIPO_PARTE_INVALIDO",
                    "El tipo de parte no es válido.",
                ));
            }
            parametros.push(tipo.to_string());
            condiciones.push(format!("p.tipo = ${}", parametros.len()));
        }
        if let Some(q) = q.map(str::trim).filter(|valor| !valor.is_empty()) {
            parametros.push(format!("%{q}%"));
            condiciones.push(format!(
                "(p.nombre ILIKE ${0} OR p.nombre_comercial ILIKE ${0} OR p.identificador_valor ILIKE ${0})",
                parametros.len()
            ));
        }
        let limite = limite.unwrap_or(50).clamp(1, 100);
        parametros.push(limite.to_string());
        let sql = format!(
            "SELECT to_jsonb(p) AS datos FROM identity.partes p
              WHERE {}
              ORDER BY p.actualizado_en DESC, p.id DESC
              LIMIT ${}::int",
            condiciones.join(" AND "),
            parametros.len()
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &parametros {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &["creado_en", "actualizado_en", "eliminado_en"],
                ))
            })
            .collect()
    }

    pub async fn obtener(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        self.obtener_parte(empresa_id, parte_id).await
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let meta = normalizar_parte(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.partes AS p
               (empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
                identificador_valor, estado, metadata, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::uuid)
             RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "nombre"))
        .bind(texto(&meta, "nombre_comercial"))
        .bind(texto(&meta, "identificador_tipo"))
        .bind(texto(&meta, "identificador_valor"))
        .bind(texto(&meta, "estado"))
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_identificador)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::conflicto("No se pudo crear la parte."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_parte_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes AS p
                SET tipo = COALESCE($3, tipo),
                    nombre = COALESCE($4, nombre),
                    nombre_comercial = COALESCE($5, nombre_comercial),
                    identificador_tipo = COALESCE($6, identificador_tipo),
                    identificador_valor = COALESCE($7, identificador_valor),
                    estado = COALESCE($8, estado),
                    metadata = COALESCE($9::jsonb, metadata),
                    actualizado_en = NOW()
              WHERE p.empresa_id = $1::uuid AND p.id = $2::uuid AND p.eliminado_en IS NULL
              RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("nombre").and_then(Value::as_str))
        .bind(meta.get("nombre_comercial").and_then(Value::as_str))
        .bind(meta.get("identificador_tipo").and_then(Value::as_str))
        .bind(meta.get("identificador_valor").and_then(Value::as_str))
        .bind(meta.get("estado").and_then(Value::as_str))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_identificador)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn eliminar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes
                SET estado = 'archivado', eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        sqlx::query(
            "UPDATE identity.medios_contacto SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.direcciones_partes SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid
                AND (parte_origen_id = $2::uuid OR parte_destino_id = $2::uuid) AND activo = true",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    /// Validación interna (servicio→servicio): ¿la parte vive en esa empresa?
    pub async fn pertenece_a_empresa(
        &self,
        empresa_id: &str,
        parte_id: &str,
    ) -> Result<bool, ApiError> {
        if !es_uuid(empresa_id) || !es_uuid(parte_id) {
            return Ok(false);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM identity.partes
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(parte_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn listar_contactos(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(c) AS datos FROM identity.medios_contacto c
              WHERE c.empresa_id = $1::uuid AND c.parte_id = $2::uuid AND c.eliminado_en IS NULL
              ORDER BY c.es_principal DESC, c.tipo, c.valor",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &[
                        "verificado_en",
                        "creado_en",
                        "actualizado_en",
                        "eliminado_en",
                    ],
                ))
            })
            .collect()
    }

    pub async fn crear_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_contacto(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.medios_contacto AS c
               (empresa_id, parte_id, tipo, valor, etiqueta, es_principal, metadata)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7::jsonb)
             RETURNING to_jsonb(c) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "valor"))
        .bind(texto(&meta, "etiqueta"))
        .bind(
            meta.get("es_principal")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_contacto)?;
        tx.commit().await.map_err(interno)?;
        let fila =
            fila.ok_or_else(|| ApiError::conflicto("No se pudo crear el medio de contacto."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &[
                "verificado_en",
                "creado_en",
                "actualizado_en",
                "eliminado_en",
            ],
        ))
    }

    pub async fn actualizar_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        contacto_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(contacto_id, "CONTACTO_INVALIDO")?;
        let meta = normalizar_contacto_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.medios_contacto AS c
                SET tipo = COALESCE($4, tipo), valor = COALESCE($5, valor),
                    etiqueta = COALESCE($6, etiqueta), es_principal = COALESCE($7, es_principal),
                    metadata = COALESCE($8::jsonb, metadata), actualizado_en = NOW()
              WHERE c.empresa_id = $1::uuid AND c.parte_id = $2::uuid
                AND c.id = $3::uuid AND c.eliminado_en IS NULL
              RETURNING to_jsonb(c) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(contacto_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("valor").and_then(Value::as_str))
        .bind(meta.get("etiqueta").and_then(Value::as_str))
        .bind(meta.get("es_principal").and_then(Value::as_bool))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_contacto)?
        .ok_or_else(|| ApiError::no_encontrado("Medio de contacto", contacto_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &[
                "verificado_en",
                "creado_en",
                "actualizado_en",
                "eliminado_en",
            ],
        ))
    }

    pub async fn eliminar_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        contacto_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(contacto_id, "CONTACTO_INVALIDO")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.medios_contacto
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid
                AND id = $3::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(contacto_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Medio de contacto", contacto_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    pub async fn listar_direcciones(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(d) AS datos FROM identity.direcciones_partes d
              WHERE d.empresa_id = $1::uuid AND d.parte_id = $2::uuid AND d.eliminado_en IS NULL
              ORDER BY d.es_principal DESC, d.tipo, d.id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &["creado_en", "actualizado_en", "eliminado_en"],
                ))
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn crear_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_direccion(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.direcciones_partes AS d
               (empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2, ciudad,
                departamento, pais, codigo_postal, referencias, latitud, longitud,
                es_principal, metadata)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                     $12, $13, $14, $15::jsonb)
             RETURNING to_jsonb(d) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "etiqueta"))
        .bind(texto(&meta, "linea_1"))
        .bind(texto(&meta, "linea_2"))
        .bind(texto(&meta, "ciudad"))
        .bind(texto(&meta, "departamento"))
        .bind(texto(&meta, "pais"))
        .bind(texto(&meta, "codigo_postal"))
        .bind(texto(&meta, "referencias"))
        .bind(meta.get("latitud").and_then(Value::as_f64))
        .bind(meta.get("longitud").and_then(Value::as_f64))
        .bind(
            meta.get("es_principal")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::conflicto("No se pudo crear la dirección."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn actualizar_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        direccion_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(direccion_id, "DIRECCION_INVALIDA")?;
        let meta = normalizar_direccion_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.direcciones_partes AS d
                SET tipo = COALESCE($4, tipo), etiqueta = COALESCE($5, etiqueta),
                    linea_1 = COALESCE($6, linea_1), linea_2 = COALESCE($7, linea_2),
                    ciudad = COALESCE($8, ciudad), departamento = COALESCE($9, departamento),
                    pais = COALESCE($10, pais), codigo_postal = COALESCE($11, codigo_postal),
                    referencias = COALESCE($12, referencias), latitud = COALESCE($13, latitud),
                    longitud = COALESCE($14, longitud), es_principal = COALESCE($15, es_principal),
                    metadata = COALESCE($16::jsonb, metadata), actualizado_en = NOW()
              WHERE d.empresa_id = $1::uuid AND d.parte_id = $2::uuid
                AND d.id = $3::uuid AND d.eliminado_en IS NULL
              RETURNING to_jsonb(d) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(direccion_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("etiqueta").and_then(Value::as_str))
        .bind(meta.get("linea_1").and_then(Value::as_str))
        .bind(meta.get("linea_2").and_then(Value::as_str))
        .bind(meta.get("ciudad").and_then(Value::as_str))
        .bind(meta.get("departamento").and_then(Value::as_str))
        .bind(meta.get("pais").and_then(Value::as_str))
        .bind(meta.get("codigo_postal").and_then(Value::as_str))
        .bind(meta.get("referencias").and_then(Value::as_str))
        .bind(meta.get("latitud").and_then(Value::as_f64))
        .bind(meta.get("longitud").and_then(Value::as_f64))
        .bind(meta.get("es_principal").and_then(Value::as_bool))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Dirección", direccion_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn eliminar_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        direccion_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(direccion_id, "DIRECCION_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.direcciones_partes
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid
                AND id = $3::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(direccion_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Dirección", direccion_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    pub async fn listar_relaciones(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(r) || jsonb_build_object(
                      'relacionada_id', p.id,
                      'relacionada_nombre', p.nombre,
                      'relacionada_tipo', p.tipo
                    ) AS datos
               FROM identity.relaciones_partes r
               JOIN identity.partes p
                 ON p.id = CASE WHEN r.parte_origen_id = $2::uuid
                                THEN r.parte_destino_id ELSE r.parte_origen_id END
              WHERE r.empresa_id = $1::uuid
                AND (r.parte_origen_id = $2::uuid OR r.parte_destino_id = $2::uuid)
                AND r.activo = true AND p.eliminado_en IS NULL
              ORDER BY r.tipo, r.creado_en DESC",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let mut datos: Value = fila.try_get("datos").map_err(interno)?;
                if let Some(objeto) = datos.as_object_mut() {
                    let relacionada = json!({
                        "id": objeto.remove("relacionada_id"),
                        "nombre": objeto.remove("relacionada_nombre"),
                        "tipo": objeto.remove("relacionada_tipo"),
                    });
                    objeto.insert("parte_relacionada".into(), relacionada);
                }
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub async fn crear_relacion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        destino_id: &str,
        tipo: &str,
        metadata: Option<&Value>,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(destino_id, "PARTE_DESTINO_INVALIDA")?;
        let tipo = normalizar_codigo(tipo, "TIPO_RELACION_INVALIDO")?;
        validar_metadata(metadata)?;
        if parte_id == destino_id {
            return Err(ApiError::dominio(
                "RELACION_INVALIDA",
                "Una parte no puede relacionarse consigo misma.",
            ));
        }
        self.obtener_parte(empresa_id, destino_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.relaciones_partes AS r
               (empresa_id, parte_origen_id, parte_destino_id, tipo, metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, COALESCE($5::jsonb, '{}'::jsonb), $6::uuid)
             RETURNING to_jsonb(r) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(destino_id)
        .bind(&tipo)
        .bind(metadata.cloned())
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto("La relación ya existe.")
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la relación."))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn eliminar_relacion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        relacion_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(relacion_id, "RELACION_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid
                AND (parte_origen_id = $3::uuid OR parte_destino_id = $3::uuid) AND activo = true
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(relacion_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Relación", relacion_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    async fn obtener_parte(&self, empresa_id: &str, parte_id: &str) -> Result<Value, ApiError> {
        validar_uuid(parte_id, "PARTE_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(p) AS datos FROM identity.partes p
              WHERE p.empresa_id = $1::uuid AND p.id = $2::uuid AND p.eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    async fn validar_parte_acceso(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        escritura: bool,
    ) -> Result<(), ApiError> {
        self.validar_acceso(empresa_id, actor, escritura).await?;
        self.obtener_parte(empresa_id, parte_id).await?;
        Ok(())
    }

    async fn validar_acceso(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        escritura: bool,
    ) -> Result<(), ApiError> {
        validar_uuid(empresa_id, "EMPRESA_INVALIDA")?;
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No puedes operar partes de otra empresa.",
            ));
        }
        if escritura && !ROLES_ESCRITURA.contains(&actor.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "El rol no puede modificar los datos universales.",
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

fn normalizar_parte(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_parte_parcial(datos)?;
    let tipo = parcial.get("tipo").and_then(Value::as_str);
    let nombre = parcial.get("nombre").and_then(Value::as_str);
    if tipo.is_none() || nombre.is_none() {
        return Err(ApiError::dominio(
            "PARTE_INVALIDA",
            "El tipo y nombre de la parte son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    salida.insert(
        "nombre_comercial".into(),
        json!(texto(&parcial, "nombre_comercial")),
    );
    salida.insert(
        "identificador_tipo".into(),
        json!(texto(&parcial, "identificador_tipo")),
    );
    salida.insert(
        "identificador_valor".into(),
        json!(texto(&parcial, "identificador_valor")),
    );
    salida.insert(
        "estado".into(),
        parcial
            .get("estado")
            .cloned()
            .unwrap_or_else(|| json!("activo")),
    );
    salida.insert(
        "metadata".into(),
        parcial
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Ok(Value::Object(salida))
}

fn normalizar_parte_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "PARTE_INVALIDA",
            "La parte debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_PARTE.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_PARTE_INVALIDO",
                "El tipo de parte no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    if let Some(estado) = objeto.get("estado") {
        let estado = estado.as_str().unwrap_or_default();
        if !ESTADOS_PARTE.contains(&estado) {
            return Err(ApiError::dominio(
                "ESTADO_PARTE_INVALIDO",
                "El estado de parte no es válido.",
            ));
        }
        salida.insert("estado".into(), json!(estado));
    }
    if let Some(nombre) = objeto.get("nombre") {
        let nombre = nombre.as_str().unwrap_or("").trim();
        if nombre.is_empty() || nombre.chars().count() > 240 {
            return Err(ApiError::dominio(
                "NOMBRE_PARTE_INVALIDO",
                "El nombre debe tener entre 1 y 240 caracteres.",
            ));
        }
        salida.insert("nombre".into(), json!(nombre));
    }
    for (clave, maximo) in [
        ("nombre_comercial", 240),
        ("identificador_tipo", 40),
        ("identificador_valor", 120),
    ] {
        if let Some(valor) = objeto.get(clave) {
            salida.insert(clave.into(), json!(texto_max(valor, maximo)?));
        }
    }
    if let Some(metadata) = objeto.get("metadata") {
        validar_metadata(Some(metadata))?;
        salida.insert("metadata".into(), metadata.clone());
    }
    Ok(Value::Object(salida))
}

fn normalizar_contacto(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_contacto_parcial(datos)?;
    if parcial.get("tipo").is_none() || parcial.get("valor").is_none() {
        return Err(ApiError::dominio(
            "CONTACTO_INVALIDO",
            "El tipo y valor del contacto son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    salida.insert("etiqueta".into(), json!(texto(&parcial, "etiqueta")));
    salida.insert(
        "es_principal".into(),
        parcial
            .get("es_principal")
            .cloned()
            .unwrap_or_else(|| json!(false)),
    );
    salida.insert(
        "metadata".into(),
        parcial
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Ok(Value::Object(salida))
}

fn normalizar_contacto_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "CONTACTO_INVALIDO",
            "El contacto debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_CONTACTO.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_CONTACTO_INVALIDO",
                "El tipo de contacto no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    if let Some(valor) = objeto.get("valor") {
        salida.insert("valor".into(), json!(texto_max(valor, 240)?));
    }
    if let Some(etiqueta) = objeto.get("etiqueta") {
        salida.insert("etiqueta".into(), json!(texto_max(etiqueta, 120)?));
    }
    if let Some(es_principal) = objeto.get("es_principal") {
        salida.insert("es_principal".into(), es_principal.clone());
    }
    if let Some(metadata) = objeto.get("metadata") {
        validar_metadata(Some(metadata))?;
        salida.insert("metadata".into(), metadata.clone());
    }
    Ok(Value::Object(salida))
}

fn normalizar_direccion(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_direccion_parcial(datos)?;
    if parcial.get("tipo").is_none() || parcial.get("linea_1").is_none() {
        return Err(ApiError::dominio(
            "DIRECCION_INVALIDA",
            "El tipo y la línea principal son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    for clave in [
        "etiqueta",
        "linea_2",
        "ciudad",
        "departamento",
        "pais",
        "codigo_postal",
        "referencias",
    ] {
        if !salida.contains_key(clave) {
            salida.insert(clave.into(), json!(""));
        }
    }
    if !salida.contains_key("latitud") {
        salida.insert("latitud".into(), Value::Null);
    }
    if !salida.contains_key("longitud") {
        salida.insert("longitud".into(), Value::Null);
    }
    if !salida.contains_key("es_principal") {
        salida.insert("es_principal".into(), json!(false));
    }
    if !salida.contains_key("metadata") {
        salida.insert("metadata".into(), json!({}));
    }
    Ok(Value::Object(salida))
}

fn normalizar_direccion_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "DIRECCION_INVALIDA",
            "La dirección debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_DIRECCION.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_DIRECCION_INVALIDO",
                "El tipo de dirección no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    for (clave, maximo) in [
        ("etiqueta", 120),
        ("linea_1", 240),
        ("linea_2", 240),
        ("ciudad", 120),
        ("departamento", 120),
        ("pais", 120),
        ("codigo_postal", 32),
        ("referencias", 500),
    ] {
        if let Some(valor) = objeto.get(clave) {
            salida.insert(clave.into(), json!(texto_max(valor, maximo)?));
        }
    }
    for (clave, minimo, maximo) in [("latitud", -90.0, 90.0), ("longitud", -180.0, 180.0)] {
        if let Some(valor) = objeto.get(clave) {
            if valor.is_null() {
                salida.insert(clave.into(), Value::Null);
            } else if let Some(numero) = valor.as_f64() {
                if numero < minimo || numero > maximo {
                    return Err(ApiError::dominio(
                        "COORDENADA_INVALIDA",
                        "La coordenada está fuera de rango.",
                    ));
                }
                salida.insert(clave.into(), json!(numero));
            } else {
                return Err(ApiError::dominio(
                    "COORDENADA_INVALIDA",
                    "La coordenada debe ser numérica.",
                ));
            }
        }
    }
    if let Some(es_principal) = objeto.get("es_principal") {
        salida.insert("es_principal".into(), es_principal.clone());
    }
    if let Some(metadata) = objeto.get("metadata") {
        validar_metadata(Some(metadata))?;
        salida.insert("metadata".into(), metadata.clone());
    }
    Ok(Value::Object(salida))
}

fn validar_metadata(metadata: Option<&Value>) -> Result<(), ApiError> {
    if let Some(metadata) = metadata {
        if !metadata.is_object() {
            return Err(ApiError::dominio(
                "METADATA_INVALIDA",
                "La metadata debe ser un objeto JSON.",
            ));
        }
        if serde_json::to_vec(metadata)
            .map(|bytes| bytes.len())
            .unwrap_or(0)
            > 64 * 1024
        {
            return Err(ApiError::dominio(
                "METADATA_INVALIDA",
                "La metadata supera 64 KiB.",
            ));
        }
    }
    Ok(())
}

fn validar_uuid(valor: &str, codigo: &str) -> Result<(), ApiError> {
    if !es_uuid(valor) {
        return Err(ApiError::dominio(
            codigo,
            "El identificador debe ser un UUID válido.",
        ));
    }
    Ok(())
}

fn normalizar_codigo(valor: &str, error: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            error,
            "El código solo admite minúsculas, números y guion bajo (2-40).",
        ));
    }
    Ok(codigo)
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_max(valor: &Value, maximo: usize) -> Result<String, ApiError> {
    let texto = valor.as_str().unwrap_or("").trim();
    if texto.chars().count() > maximo {
        return Err(ApiError::dominio(
            "VALOR_INVALIDO",
            format!("El valor no puede superar {maximo} caracteres."),
        ));
    }
    Ok(texto.to_string())
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

fn conflicto_identificador(error: sqlx::Error) -> ApiError {
    if es_unico(&error) {
        return ApiError::conflicto("Ya existe una parte con ese identificador.");
    }
    interno(error)
}

fn conflicto_contacto(error: sqlx::Error) -> ApiError {
    if es_unico(&error) {
        return ApiError::conflicto("Ya existe ese medio de contacto para la parte.");
    }
    interno(error)
}

fn es_unico(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
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
    use crate::auth::es_rol;

    #[test]
    fn debe_normalizar_y_validar_partes() {
        let parte =
            normalizar_parte(&json!({"tipo": "persona", "nombre": " Ana "})).expect("parte");
        assert_eq!(parte["nombre"], "Ana");
        assert_eq!(parte["estado"], "activo");
        assert!(normalizar_parte(&json!({"tipo": "robot", "nombre": "x"})).is_err());
        assert!(normalizar_parte(&json!({"tipo": "persona"})).is_err());
    }

    #[test]
    fn debe_validar_coordenadas_y_metadata() {
        assert!(normalizar_direccion(
            &json!({"tipo": "entrega", "linea_1": "Calle 1", "latitud": 12.1})
        )
        .is_ok());
        assert!(normalizar_direccion(
            &json!({"tipo": "entrega", "linea_1": "Calle 1", "latitud": 120})
        )
        .is_err());
        assert!(validar_metadata(Some(&json!({"a": 1}))).is_ok());
        assert!(validar_metadata(Some(&json!([]))).is_err());
    }

    #[test]
    fn debe_normalizar_codigos() {
        assert_eq!(
            normalizar_codigo("Proveedor", "TIPO").expect("codigo"),
            "proveedor"
        );
        assert!(normalizar_codigo("1malo", "TIPO").is_err());
    }

    #[test]
    fn debe_conservar_el_rol_de_escritura() {
        assert!(es_rol("empresario"));
        assert!(ROLES_ESCRITURA.contains(&"operativo"));
        assert!(!ROLES_ESCRITURA.contains(&"comprador"));
    }
}
