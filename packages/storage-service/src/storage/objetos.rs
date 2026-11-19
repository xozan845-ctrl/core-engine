//! Objetos, cuotas y URLs prefirmadas (ADR-24/ADR-28).

use crate::almacen::{AlmacenObjetos, ObjetoLeido};
use crate::auth::UsuarioContexto;
use crate::config::Config;
use crate::db::{error_db, jsonb, timestamp, timestamp_opcional, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::logs;
use crate::outbox::{Outbox, STORAGE_OBJETO_ELIMINADO, STORAGE_OBJETO_SUBIDO};
use crate::storage::buckets::{error_almacen, Buckets};
use crate::storage::pagination::{crear_pagina, parsear_paginacion, Pagina};
use base64::Engine;
use chrono::{Duration, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, QueryBuilder, Row, Transaction};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ObjetoStorage {
    pub id: String,
    pub tenant_id: String,
    pub bucket: String,
    pub objeto: String,
    pub nombre: String,
    pub content_type: String,
    pub carpeta: String,
    pub descripcion: String,
    pub etiquetas: BTreeMap<String, String>,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub estado: String,
    pub publico: bool,
    pub subido_por: Option<String>,
    pub creado_en: String,
    pub confirmado_en: Option<String>,
    pub eliminado_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SolicitudSubida {
    pub objeto_id: String,
    pub bucket: String,
    pub objeto: String,
    pub url_subida: String,
    pub metodo: String,
    pub headers: BTreeMap<String, String>,
    pub expira_en: String,
    pub max_bytes: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UrlDescarga {
    pub url_descarga: String,
    pub expira_en: String,
    pub nombre: String,
    pub content_type: String,
    pub tamano_bytes: i64,
}

pub struct ObjetoPublico {
    pub cuerpo: ObjetoLeido,
    pub content_type: String,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub nombre: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjetoPublicoMeta {
    pub content_type: String,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub nombre: String,
}

#[derive(Debug, Clone, Default)]
pub struct DatosSubida {
    pub nombre: String,
    pub content_type: String,
    pub tamano_bytes: i64,
    pub carpeta: Option<String>,
    pub descripcion: Option<String>,
    pub etiquetas: Option<BTreeMap<String, String>>,
    pub publico: bool,
}

pub struct Objetos {
    db: Arc<Db>,
    buckets: Arc<Buckets>,
    outbox: Arc<Outbox>,
    almacen: Arc<dyn AlmacenObjetos>,
    config: Arc<Config>,
}

impl Objetos {
    pub fn nuevo(
        db: Arc<Db>,
        buckets: Arc<Buckets>,
        outbox: Arc<Outbox>,
        almacen: Arc<dyn AlmacenObjetos>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            db,
            buckets,
            outbox,
            almacen,
            config,
        }
    }

    pub async fn solicitar_subida(
        &self,
        tenant_id: &str,
        usuario_id: Option<&str>,
        datos: DatosSubida,
    ) -> Result<SolicitudSubida, ApiError> {
        let nombre = sanear_nombre(&datos.nombre);
        let content_type = datos.content_type.trim().to_lowercase();
        if !self.config.content_types_permitidos.contains(&content_type) {
            return Err(ApiError::dominio(
                "CONTENT_TYPE_NO_PERMITIDO",
                format!("El tipo \"{content_type}\" no esta permitido."),
            )
            .con_detalles(json!({ "permitidos": self.config.content_types_permitidos })));
        }
        if datos.tamano_bytes <= 0 {
            return Err(ApiError::dominio(
                "TAMANO_INVALIDO",
                "El tamano debe ser un entero positivo en bytes.",
            ));
        }
        let max_bytes = self.maximo_para(&content_type);
        if datos.tamano_bytes > max_bytes {
            return Err(ApiError::dominio(
                "OBJETO_DEMASIADO_GRANDE",
                format!("El archivo excede el maximo de {max_bytes} bytes."),
            )
            .con_detalles(json!({ "max_bytes": max_bytes })));
        }

        let estado = self.buckets.asegurar(tenant_id).await?;
        self.buckets.verificar_cuota(&estado, datos.tamano_bytes)?;

        let id = Uuid::new_v4();
        let carpeta = sanear_carpeta(datos.carpeta.as_deref());
        let objeto = format!("{carpeta}/{id}/{nombre}");
        let etiquetas = datos.etiquetas.unwrap_or_default();
        let autor = usuario_id.filter(|valor| Uuid::parse_str(valor).is_ok());
        let descripcion = datos
            .descripcion
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect::<String>();

        sqlx::query(
            "INSERT INTO storage.objetos
               (id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
                descripcion, etiquetas, tamano_bytes, estado, subido_por, publico, creado_en)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, 'pendiente', $11, $12, NOW())",
        )
        .bind(id)
        .bind(Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(&estado.bucket)
        .bind(&objeto)
        .bind(&nombre)
        .bind(&content_type)
        .bind(&carpeta)
        .bind(&descripcion)
        .bind(sqlx::types::Json(&etiquetas))
        .bind(datos.tamano_bytes)
        .bind(autor.map(|valor| Uuid::parse_str(valor).expect("uuid validado")))
        .bind(datos.publico)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;

        let url_subida = self
            .almacen
            .url_subida(
                &estado.bucket,
                &objeto,
                &content_type,
                self.config.url_ttl_segundos,
            )
            .await
            .map_err(error_almacen)?;
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), content_type);
        Ok(SolicitudSubida {
            objeto_id: id.to_string(),
            bucket: estado.bucket,
            objeto,
            url_subida,
            metodo: "PUT".to_string(),
            headers,
            expira_en: self.expiracion(),
            max_bytes,
        })
    }

    /// Subida directa servicio→servicio (PDFs de comprobantes). Reutiliza la
    #[allow(clippy::too_many_arguments)]
    /// validación, cuota y outbox del flujo prefirmado.
    pub async fn subir_contenido(
        &self,
        tenant_id: &str,
        usuario_id: Option<&str>,
        nombre: &str,
        content_type: &str,
        bytes: Vec<u8>,
        publico: bool,
        descripcion: Option<String>,
    ) -> Result<ObjetoStorage, ApiError> {
        let tamano = i64::try_from(bytes.len()).map_err(|_| ApiError::interno())?;
        if tamano <= 0 {
            return Err(ApiError::dominio(
                "OBJETO_VACIO",
                "El contenido no puede estar vacío.",
            ));
        }
        let solicitud = self
            .solicitar_subida(
                tenant_id,
                usuario_id,
                DatosSubida {
                    nombre: nombre.to_string(),
                    content_type: content_type.to_string(),
                    tamano_bytes: tamano,
                    publico,
                    descripcion,
                    ..Default::default()
                },
            )
            .await?;
        if let Err(error) = self
            .almacen
            .subir(&solicitud.bucket, &solicitud.objeto, bytes, content_type)
            .await
        {
            let _ = sqlx::query("DELETE FROM storage.objetos WHERE id = $1")
                .bind(Uuid::parse_str(&solicitud.objeto_id).map_err(|_| ApiError::interno())?)
                .execute(self.db.pool())
                .await;
            return Err(error_almacen(error));
        }
        self.confirmar(tenant_id, &solicitud.objeto_id).await
    }

    pub async fn confirmar(&self, tenant_id: &str, id: &str) -> Result<ObjetoStorage, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado == "eliminado" {
            return Err(ApiError::conflicto(
                "El objeto fue eliminado y no puede confirmarse.",
            ));
        }
        if fila.estado == "activo" {
            return Ok(fila);
        }
        let info = self
            .almacen
            .estadisticas(&fila.bucket, &fila.objeto)
            .await
            .map_err(error_almacen)?
            .ok_or_else(|| ApiError::no_encontrado("Objeto en el almacenamiento", &fila.objeto))?;
        if info.tamano_bytes <= 0 {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "OBJETO_VACIO",
                "El objeto subido esta vacio.",
            ));
        }
        let content_type = info
            .content_type
            .clone()
            .unwrap_or_else(|| fila.content_type.clone())
            .trim()
            .to_lowercase();
        if !self.config.content_types_permitidos.contains(&content_type) {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "CONTENT_TYPE_NO_PERMITIDO",
                format!("El objeto subido tiene un tipo no permitido (\"{content_type}\")."),
            )
            .con_detalles(json!({ "permitidos": self.config.content_types_permitidos })));
        }
        let max_bytes = self.maximo_para(&content_type);
        if info.tamano_bytes > max_bytes {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "OBJETO_DEMASIADO_GRANDE",
                format!("El archivo excede el maximo de {max_bytes} bytes."),
            )
            .con_detalles(json!({ "max_bytes": max_bytes })));
        }

        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_id = Uuid::parse_str(id).map_err(|_| ApiError::interno())?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let estado = self
            .buckets
            .bloquear_estado_en_transaccion(&mut tx, tenant_id)
            .await?;
        self.buckets
            .verificar_cuota(&estado, (info.tamano_bytes - fila.tamano_bytes).max(0))?;
        let actualizado = sqlx::query(&format!(
            "UPDATE storage.objetos
                SET estado = 'activo', tamano_bytes = $3, content_type = $4, etag = $5,
                    confirmado_en = NOW()
              WHERE tenant_id = $1 AND id = $2 AND estado = 'pendiente'
              RETURNING {COLUMNAS}"
        ))
        .bind(tenant_uuid)
        .bind(objeto_id)
        .bind(info.tamano_bytes)
        .bind(&content_type)
        .bind(info.etag.clone())
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(actualizado) = actualizado else {
            tx.commit().await.map_err(error_db)?;
            return self.obtener(tenant_id, id).await;
        };
        self.buckets
            .sumar_uso_en_transaccion(&mut tx, tenant_id, info.tamano_bytes)
            .await?;
        self.emitir_subido(&mut tx, &actualizado).await?;
        tx.commit().await.map_err(error_db)?;
        serializar(&actualizado)
    }

    pub async fn listar(
        &self,
        tenant_id: &str,
        filtros: &FiltrosObjetos,
    ) -> Result<Pagina<ObjetoStorage>, ApiError> {
        let (pagina, limite) = parsear_paginacion(filtros.pagina, filtros.limite);
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;

        let mut count = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM storage.objetos WHERE tenant_id = ",
        );
        count.push_bind(tenant_uuid);
        aplicar_filtros(&mut count, filtros);
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut query = QueryBuilder::<Postgres>::new(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos WHERE tenant_id = "
        ));
        query.push_bind(tenant_uuid);
        aplicar_filtros(&mut query, filtros);
        query.push(" ORDER BY creado_en DESC LIMIT ");
        query.push_bind(limite);
        query.push(" OFFSET ");
        query.push_bind((pagina - 1) * limite);
        let filas = query
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;
        let items = filas
            .iter()
            .map(serializar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(items, total, pagina, limite))
    }

    pub async fn obtener(&self, tenant_id: &str, id: &str) -> Result<ObjetoStorage, ApiError> {
        self.obtener_fila(tenant_id, id).await
    }

    pub async fn url_descarga(&self, tenant_id: &str, id: &str) -> Result<UrlDescarga, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado == "eliminado" {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        let url_descarga = self
            .almacen
            .url_descarga(&fila.bucket, &fila.objeto, self.config.url_ttl_segundos)
            .await
            .map_err(error_almacen)?;
        Ok(UrlDescarga {
            url_descarga,
            expira_en: self.expiracion(),
            nombre: fila.nombre,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
        })
    }

    pub async fn leer_publico(
        &self,
        id: &str,
        rango: Option<(i64, i64)>,
    ) -> Result<ObjetoPublico, ApiError> {
        let fila = self.fila_publica(id).await?;
        let leido = match rango {
            Some((inicio, fin)) => {
                self.almacen
                    .leer_rango(&fila.bucket, &fila.objeto, inicio, fin)
                    .await
            }
            None => self.almacen.leer(&fila.bucket, &fila.objeto).await,
        }
        .map_err(error_almacen)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let etag = leido.etag.clone().or(fila.etag.clone());
        Ok(ObjetoPublico {
            cuerpo: leido,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag,
            nombre: fila.nombre,
        })
    }

    pub async fn obtener_publico_meta(&self, id: &str) -> Result<ObjetoPublicoMeta, ApiError> {
        let fila = self.fila_publica(id).await?;
        Ok(ObjetoPublicoMeta {
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag: fila.etag,
            nombre: fila.nombre,
        })
    }

    /// Lee el contenido de un objeto del tenant (servicio→servicio).
    pub async fn leer_contenido(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<ObjetoPublico, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado != "activo" {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        let leido = self
            .almacen
            .leer(&fila.bucket, &fila.objeto)
            .await
            .map_err(error_almacen)?
            .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let etag = leido.etag.clone().or(fila.etag.clone());
        Ok(ObjetoPublico {
            cuerpo: leido,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag,
            nombre: fila.nombre,
        })
    }

    pub async fn eliminar(
        &self,
        tenant_id: &str,
        id: &str,
        usuario: Option<&UsuarioContexto>,
    ) -> Result<(), ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if let Some(usuario) = usuario {
            if usuario.rol != "admin" {
                if let Some(autor) = &fila.subido_por {
                    if autor != &usuario.user_id {
                        return Err(ApiError::acceso_denegado(
                            "Solo el autor de la subida o un administrador pueden eliminarlo.",
                        ));
                    }
                }
            }
        }
        if fila.estado == "eliminado" {
            return Ok(());
        }
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_id = Uuid::parse_str(id).map_err(|_| ApiError::interno())?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let eliminado = sqlx::query(&format!(
            "UPDATE storage.objetos SET estado = 'eliminado', eliminado_en = NOW()
              WHERE tenant_id = $1 AND id = $2 AND estado <> 'eliminado'
              RETURNING {COLUMNAS}"
        ))
        .bind(tenant_uuid)
        .bind(objeto_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(eliminado) = eliminado else {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        };
        if fila.estado == "activo" {
            self.buckets
                .restar_uso_en_transaccion(&mut tx, tenant_id, fila.tamano_bytes)
                .await?;
        }
        self.emitir_eliminado(&mut tx, &eliminado).await?;
        tx.commit().await.map_err(error_db)?;
        if let Err(error) = self.almacen.eliminar(&fila.bucket, &fila.objeto).await {
            logs::error(
                "storage.objetos",
                crate::campos! {
                    "msg": "No se pudo borrar el objeto fisico; queda huerfano para reconciliacion",
                    "objeto_id": id,
                    "bucket": fila.bucket,
                    "err": error.to_string(),
                },
            );
        }
        Ok(())
    }

    /// Exporta metadatos del tenant y, para objetos pequeños, su contenido en
    /// base64. Los objetos grandes se referencian por id para no generar un
    /// archivo sin límite.
    pub async fn exportar_tenant(&self, tenant_id: &str) -> Result<Value, ApiError> {
        const LIMITE_BYTES: i64 = 5 * 1024 * 1024;
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objetos: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(o) ORDER BY o.creado_en), '[]'::jsonb)
               FROM storage.objetos o
              WHERE o.tenant_id = $1 AND o.estado <> 'eliminado'",
        )
        .bind(tenant_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let objetos = objetos.as_array().cloned().unwrap_or_default();
        let mut salida = Vec::with_capacity(objetos.len());
        for mut objeto in objetos {
            let id = objeto
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let tamano = objeto
                .get("tamano_bytes")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            if tamano > 0 && tamano <= LIMITE_BYTES {
                if let Ok(fila) = self.obtener_fila(tenant_id, &id).await {
                    if let Ok(Some(leido)) = self.almacen.leer(&fila.bucket, &fila.objeto).await {
                        let bytes = leido
                            .cuerpo
                            .collect()
                            .await
                            .map(|agregado| agregado.into_bytes())
                            .unwrap_or_default();
                        if let Some(mapa) = objeto.as_object_mut() {
                            mapa.insert(
                                "contenido_base64".to_string(),
                                Value::String(
                                    base64::engine::general_purpose::STANDARD.encode(bytes),
                                ),
                            );
                        }
                    }
                }
            } else if tamano > LIMITE_BYTES {
                if let Some(mapa) = objeto.as_object_mut() {
                    mapa.insert("omitido_por_tamano".to_string(), Value::Bool(true));
                }
            }
            salida.push(objeto);
        }
        let bucket: Value = sqlx::query_scalar(
            "SELECT COALESCE(to_jsonb(b), '{}'::jsonb)
               FROM storage.buckets b WHERE b.tenant_id = $1",
        )
        .bind(tenant_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(json!({
            "bucket": bucket,
            "objetos": salida,
            "limite_bytes_incluidos": LIMITE_BYTES,
        }))
    }

    /// Purga todos los objetos de un tenant: marca como eliminados, resta el
    /// uso del bucket, emite un evento por objeto y borra el fichero físico.
    /// Idempotente: los objetos ya eliminados se omiten.
    pub async fn purgar_tenant(&self, tenant_id: &str) -> Result<Value, ApiError> {
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos
              WHERE tenant_id = $1 AND estado <> 'eliminado'"
        ))
        .bind(tenant_uuid)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        if filas.is_empty() {
            return Ok(json!({ "objetos": 0, "bytes": 0, "eliminados": 0, "errores": 0 }));
        }
        let objetos: Vec<ObjetoStorage> = filas
            .iter()
            .map(serializar)
            .collect::<Result<Vec<_>, _>>()?;
        let bytes: i64 = objetos
            .iter()
            .filter(|objeto| objeto.estado == "activo")
            .map(|objeto| objeto.tamano_bytes)
            .sum();
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        for objeto in &objetos {
            let actualizado = sqlx::query(&format!(
                "UPDATE storage.objetos SET estado = 'eliminado', eliminado_en = NOW()
                  WHERE id = $1 AND estado <> 'eliminado'
                  RETURNING {COLUMNAS}"
            ))
            .bind(Uuid::parse_str(&objeto.id).map_err(|_| ApiError::interno())?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
            if let Some(fila) = actualizado {
                self.emitir_eliminado(&mut tx, &fila).await?;
            }
        }
        if bytes > 0 {
            self.buckets
                .restar_uso_en_transaccion(&mut tx, tenant_id, bytes)
                .await?;
        }
        tx.commit().await.map_err(error_db)?;

        let mut errores = 0usize;
        for objeto in &objetos {
            if let Err(error) = self.almacen.eliminar(&objeto.bucket, &objeto.objeto).await {
                errores += 1;
                logs::error(
                    "storage.objetos",
                    crate::campos! {
                        "msg": "No se pudo borrar el objeto fisico durante la purga",
                        "objeto_id": objeto.id,
                        "bucket": objeto.bucket,
                        "err": error.to_string(),
                    },
                );
            }
        }
        Ok(json!({
            "objetos": objetos.len(),
            "bytes": bytes,
            "eliminados": objetos.len().saturating_sub(errores),
            "errores": errores,
        }))
    }

    async fn fila_publica(&self, id: &str) -> Result<ObjetoStorage, ApiError> {
        let uuid = Uuid::parse_str(id).map_err(|_| ApiError::no_encontrado("Objeto", id))?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos
              WHERE id = $1 AND estado = 'activo' AND publico = true"
        ))
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let objeto = serializar(&fila)?;
        if !es_content_type_publico(&objeto.content_type) {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        Ok(objeto)
    }

    async fn obtener_fila(&self, tenant_id: &str, id: &str) -> Result<ObjetoStorage, ApiError> {
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_uuid = Uuid::parse_str(id).map_err(|_| ApiError::no_encontrado("Objeto", id))?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_uuid)
        .bind(objeto_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        serializar(&fila)
    }

    async fn emitir_subido(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        fila: &PgRow,
    ) -> Result<(), ApiError> {
        let objeto = serializar(fila)?;
        let mut data = Map::new();
        data.insert("objeto_id".into(), json!(objeto.id));
        data.insert("tenant_id".into(), json!(objeto.tenant_id));
        data.insert("bucket".into(), json!(objeto.bucket));
        data.insert("objeto".into(), json!(objeto.objeto));
        data.insert("nombre".into(), json!(objeto.nombre));
        data.insert("content_type".into(), json!(objeto.content_type));
        data.insert("tamano_bytes".into(), json!(objeto.tamano_bytes));
        if let Some(autor) = objeto.subido_por {
            data.insert("subido_por".into(), json!(autor));
        }
        self.outbox
            .insertar_en_transaccion(tx, STORAGE_OBJETO_SUBIDO, Value::Object(data))
            .await?;
        Ok(())
    }

    async fn emitir_eliminado(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        fila: &PgRow,
    ) -> Result<(), ApiError> {
        let objeto = serializar(fila)?;
        let data = json!({
            "objeto_id": objeto.id,
            "tenant_id": objeto.tenant_id,
            "bucket": objeto.bucket,
            "objeto": objeto.objeto,
            "nombre": objeto.nombre,
            "tamano_bytes": objeto.tamano_bytes,
        });
        self.outbox
            .insertar_en_transaccion(tx, STORAGE_OBJETO_ELIMINADO, data)
            .await?;
        Ok(())
    }

    fn maximo_para(&self, content_type: &str) -> i64 {
        self.config.maximo_para(content_type)
    }

    fn expiracion(&self) -> String {
        (Utc::now() + Duration::seconds(self.config.url_ttl_segundos as i64))
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FiltrosObjetos {
    pub q: Option<String>,
    pub carpeta: Option<String>,
    pub estado: Option<String>,
    pub pagina: Option<i64>,
    pub limite: Option<i64>,
}

const COLUMNAS: &str = "id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
  descripcion, etiquetas, tamano_bytes, etag, estado, publico, subido_por, creado_en,
  confirmado_en, eliminado_en";

fn aplicar_filtros(builder: &mut QueryBuilder<'_, Postgres>, filtros: &FiltrosObjetos) {
    if let Some(estado) = &filtros.estado {
        builder.push(" AND estado = ");
        builder.push_bind(estado.clone());
    } else {
        builder.push(" AND estado <> 'eliminado'");
    }
    if let Some(carpeta) = &filtros.carpeta {
        builder.push(" AND carpeta = ");
        builder.push_bind(carpeta.to_lowercase());
    }
    if let Some(q) = &filtros.q {
        builder.push(" AND LOWER(nombre_original) LIKE ");
        builder.push_bind(format!("%{}%", q.to_lowercase()));
    }
}

fn serializar(fila: &PgRow) -> Result<ObjetoStorage, ApiError> {
    let etiquetas = jsonb(fila, "etiquetas").map_err(error_db)?;
    let etiquetas: BTreeMap<String, String> = serde_json::from_value(etiquetas).unwrap_or_default();
    Ok(ObjetoStorage {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        tenant_id: uuid_texto(fila, "tenant_id").map_err(error_db)?,
        bucket: fila.try_get::<String, _>("bucket").map_err(error_db)?,
        objeto: fila.try_get::<String, _>("objeto").map_err(error_db)?,
        nombre: fila
            .try_get::<String, _>("nombre_original")
            .map_err(error_db)?,
        content_type: fila
            .try_get::<String, _>("content_type")
            .map_err(error_db)?,
        carpeta: fila.try_get::<String, _>("carpeta").map_err(error_db)?,
        descripcion: fila.try_get::<String, _>("descripcion").map_err(error_db)?,
        etiquetas,
        tamano_bytes: fila.try_get::<i64, _>("tamano_bytes").map_err(error_db)?,
        etag: fila
            .try_get::<Option<String>, _>("etag")
            .map_err(error_db)?,
        estado: fila.try_get::<String, _>("estado").map_err(error_db)?,
        publico: fila.try_get::<bool, _>("publico").map_err(error_db)?,
        subido_por: uuid_opcional(fila, "subido_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        confirmado_en: timestamp_opcional(fila, "confirmado_en").map_err(error_db)?,
        eliminado_en: timestamp_opcional(fila, "eliminado_en").map_err(error_db)?,
    })
}

fn es_content_type_publico(content_type: &str) -> bool {
    let tipo = content_type.trim().to_lowercase();
    if tipo == "image/svg+xml" {
        return false;
    }
    tipo.starts_with("image/") || tipo == "video/mp4" || tipo == "video/webm"
}

fn sanear_nombre(valor: &str) -> String {
    let base = valor.split(['/', '\\']).next_back().unwrap_or("");
    let limpio = base
        .chars()
        .map(|caracter| {
            if caracter.is_ascii_alphanumeric() || matches!(caracter, '.' | '_' | '-') {
                caracter
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_start_matches(['.', '_'])
        .chars()
        .take(180)
        .collect::<String>();
    if limpio.is_empty() {
        "archivo".to_string()
    } else {
        limpio
    }
}

fn sanear_carpeta(valor: Option<&str>) -> String {
    let limpio = valor
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .chars()
        .map(|caracter| {
            if caracter.is_ascii_alphanumeric() || matches!(caracter, '.' | '_' | '-') {
                caracter
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.'])
        .chars()
        .take(40)
        .collect::<String>();
    if limpio.is_empty() {
        "general".to_string()
    } else {
        limpio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_sanear_nombre_y_carpeta() {
        assert_eq!(sanear_nombre("../../foto final.JPG"), "foto_final.JPG");
        assert_eq!(sanear_nombre(""), "archivo");
        assert_eq!(sanear_carpeta(Some("../Productos 2026")), "productos-2026");
        assert_eq!(sanear_carpeta(None), "general");
    }

    #[test]
    fn debe_limitar_los_content_types_publicos() {
        assert!(es_content_type_publico("image/png"));
        assert!(es_content_type_publico("video/webm"));
        assert!(!es_content_type_publico("image/svg+xml"));
        assert!(!es_content_type_publico("application/pdf"));
    }
}
