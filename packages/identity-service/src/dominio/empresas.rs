//! Empresas: alta, perfil, slug, pack y vendedores (ADR-25/ADR-26/ADR-42).
//!
//! Réplica de `empresas/empresas.service.ts`. Se opera con `to_jsonb` para
//! conservar el contrato completo de la fila sin duplicar 30 campos Rust.

use crate::crypto::{hmac_sha256_hex, sha256_hex};
use crate::db::Db;
use crate::dominio::auditoria;
use crate::dominio::media::{
    background_de, con_background_urls, normalizar_background, validar_background, BackgroundInput,
};
use crate::dominio::slug::{es_uuid, slug_con_sufijo, slug_desde_nombre, validar_slug_solicitado};
use crate::dominio::tema::{
    es_plantilla, mezclar_tema, normalizar_tema_parcial, plantilla_de, resolver_personalizacion,
    tema_parcial_de, validar_tema, validar_tema_input, HerenciaTema, PLANTILLAS,
};
use crate::dominio::usuarios::{correo_valido, CrearVendedor, Usuarios};
use crate::errors::ApiError;
use crate::purga::PurgaExterna;
use crate::storage::Storage;
use chrono::Utc;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

const MAX_INTENTOS_SLUG: u32 = 50;
const ESTADOS_EMPRESA: [&str; 3] = ["activa", "suspendida", "inactiva"];

pub struct Empresas {
    db: Arc<Db>,
    usuarios: Arc<Usuarios>,
    storage: Arc<Storage>,
    purga: Arc<PurgaExterna>,
    firma_clave: Arc<String>,
}

#[derive(Debug, Clone)]
pub struct DescargaExportacion {
    pub empresa_id: String,
    pub formato: String,
    pub sha256: String,
    pub firma: String,
    pub contenido: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CrearEmpresa {
    pub usuario_id: String,
    pub nombre: String,
    pub slug: Option<String>,
    pub perfil: Value,
    pub vertical_id: Option<String>,
    pub pack_codigo: Option<String>,
    pub pack_version: Option<i32>,
    pub capacidades_extra: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrearVendedorEmpresa {
    pub nombre: String,
    pub correo: String,
    pub contrasena: String,
    pub participacion_porcentaje: i32,
}

impl Empresas {
    pub fn nueva(
        db: Arc<Db>,
        usuarios: Arc<Usuarios>,
        storage: Arc<Storage>,
        purga: Arc<PurgaExterna>,
        firma_clave: String,
    ) -> Self {
        Self {
            db,
            usuarios,
            storage,
            purga,
            firma_clave: Arc::new(firma_clave),
        }
    }

    pub async fn crear(&self, input: CrearEmpresa) -> Result<Value, ApiError> {
        let nombre = input.nombre.trim().to_string();
        let vertical = (input.vertical_id.unwrap_or_default())
            .trim()
            .to_lowercase();
        if !es_codigo_segmento(&vertical) {
            return Err(ApiError::dominio(
                "VERTICAL_REQUERIDA",
                "Selecciona una vertical para la empresa.",
            ));
        }
        let Some(pack_codigo) = input
            .pack_codigo
            .map(|valor| valor.trim().to_lowercase())
            .filter(|valor| !valor.is_empty())
        else {
            return Err(ApiError::dominio(
                "PACK_REQUERIDO",
                "Selecciona un pack para la empresa.",
            ));
        };
        let mut perfil_datos = input.perfil.clone();
        if !perfil_datos.is_object() {
            perfil_datos = json!({});
        }
        if let Some(objeto) = perfil_datos.as_object_mut() {
            objeto.insert("logo_objeto_id".into(), Value::Null);
            objeto.insert("background_objeto_id".into(), Value::Null);
        }
        let meta = self.normalizar_perfil(&perfil_datos, &json!({}));
        self.validar_perfil(&meta)?;
        let ruc = texto(&meta, "ruc");
        self.validar_ruc_unico(&ruc, None).await?;

        let slug_explicito = match input.slug.as_deref() {
            Some(valor) => Some(validar_slug_solicitado(valor)?),
            None => None,
        };
        let max_intentos = if slug_explicito.is_some() { 1 } else { 3 };

        for intento in 1..=max_intentos {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let pack =
                resolver_pack(&mut tx, &pack_codigo, Some(&vertical), input.pack_version).await?;
            let slug = elegir_slug(&mut tx, &nombre, slug_explicito.as_deref()).await?;
            let plantilla_override = meta.get("plantilla").and_then(Value::as_str);
            let tema_override = meta.get("tema").cloned().unwrap_or_else(|| json!({}));
            let insertado = sqlx::query(
                "INSERT INTO identity.empresas AS e
                   (nombre, slug, vertical_id, pack_codigo, pack_version, creado_por,
                    ruc, razon_social, nombre_comercial, descripcion, telefono,
                    correo_contacto, sitio_web, direccion, ciudad, departamento, pais,
                    moneda, zona_horaria, sector, representante_legal, horario_atencion,
                    plantilla_override, tema_override, logo_objeto_id)
                 VALUES ($1,$2,$3,$4,$5,$6::uuid,$7,$8,$9,$10,$11,$12,$13,$14,
                         $15,$16,$17,$18,$19,$20,$21,$22,$23,$24,NULL)
                 RETURNING to_jsonb(e) AS datos",
            )
            .bind(&nombre)
            .bind(&slug)
            .bind(&pack.vertical_codigo)
            .bind(&pack.codigo)
            .bind(pack.version)
            .bind(&input.usuario_id)
            .bind(texto(&meta, "ruc"))
            .bind(texto(&meta, "razon_social"))
            .bind(texto(&meta, "nombre_comercial"))
            .bind(texto(&meta, "descripcion"))
            .bind(texto(&meta, "telefono"))
            .bind(texto(&meta, "correo_contacto"))
            .bind(texto(&meta, "sitio_web"))
            .bind(texto(&meta, "direccion"))
            .bind(texto(&meta, "ciudad"))
            .bind(texto(&meta, "departamento"))
            .bind(texto(&meta, "pais"))
            .bind(texto(&meta, "moneda"))
            .bind(texto(&meta, "zona_horaria"))
            .bind(texto(&meta, "sector"))
            .bind(texto(&meta, "representante_legal"))
            .bind(texto(&meta, "horario_atencion"))
            .bind(plantilla_override)
            .bind(tema_override)
            .fetch_optional(&mut *tx)
            .await;

            let fila = match insertado {
                Ok(fila) => fila,
                Err(error) => {
                    if es_unico(&error) {
                        if slug_explicito.is_some() || intento == max_intentos {
                            return Err(ApiError::conflicto(format!(
                                "El slug \"{}\" ya está en uso.",
                                slug_explicito.as_deref().unwrap_or(&nombre)
                            )));
                        }
                        continue;
                    }
                    return Err(interno(error));
                }
            };
            let Some(fila) = fila else {
                return Err(ApiError::interno());
            };
            let datos: Value = fila.try_get("datos").map_err(interno)?;
            let empresa_id = texto(&datos, "id");
            sqlx::query("SELECT set_config('request.jwt.claims', $1, true)")
                .bind(
                    json!({
                        "sub": input.usuario_id,
                        "rol": "empresario",
                        "tenant_id": empresa_id,
                    })
                    .to_string(),
                )
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.negocios
                   (empresa_id, codigo, nombre, vertical_id, pack_codigo,
                    pack_version, es_predeterminado, creado_por)
                 VALUES ($1::uuid, 'principal', $2, $3, $4, $5, true, $6::uuid)
                 ON CONFLICT (empresa_id, codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(if nombre.chars().count() > 120 {
                nombre.chars().take(120).collect::<String>()
            } else {
                nombre.clone()
            })
            .bind(&pack.vertical_codigo)
            .bind(&pack.codigo)
            .bind(pack.version)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            auditoria::registrar(
                &mut tx,
                &input.usuario_id,
                &empresa_id,
                None,
                "empresa.creada",
                "empresa",
                Some(&empresa_id),
                json!({ "nombre": nombre, "slug": slug }),
            )
            .await?;
            sqlx::query(
                "INSERT INTO identity.roles_empresa
                   (empresa_id, negocio_id, codigo, nombre, permisos, creado_por)
                 SELECT $1::uuid, n.id, p.codigo, p.nombre, p.permisos, $2::uuid
                   FROM identity.negocios n
                   JOIN identity.plantillas_roles p ON p.vertical_id = n.vertical_id
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO inventory.ubicaciones
                   (empresa_id, negocio_id, codigo, nombre, tipo, predeterminada)
                 SELECT $1::uuid, n.id,
                        'principal_' || substr(replace(n.id::text, '-', ''), 1, 8),
                        'Principal', 'bodega', true
                   FROM identity.negocios n
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.roles_empresa
                   (empresa_id, codigo, nombre, permisos, creado_por)
                 VALUES ($1::uuid, 'empresario', 'Empresario', '{}'::jsonb, $2::uuid)
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.asignaciones_personal
                   (empresa_id, usuario_id, rol_empresa_id, estado, creado_por)
                 SELECT $1::uuid, $2::uuid, r.id, 'activa', $2::uuid
                   FROM identity.roles_empresa r
                  WHERE r.empresa_id = $1::uuid AND r.codigo = 'empresario'",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.empresa_capacidades
                   (empresa_id, capacidad_codigo, config, activo)
                 SELECT $1::uuid, capacidad_codigo, config, activo
                   FROM identity.pack_capacidades
                  WHERE pack_codigo = $2 AND pack_version = $3
                 ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&pack.codigo)
            .bind(pack.version)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            for capacidad in &input.capacidades_extra {
                let codigo = capacidad.trim().to_lowercase();
                if codigo.is_empty() {
                    continue;
                }
                let disponible = sqlx::query(
                    "SELECT 1 FROM identity.vertical_capacidades vc
                       JOIN identity.capacidades c ON c.codigo = vc.capacidad_codigo
                      WHERE vc.vertical_codigo = $1 AND vc.capacidad_codigo = $2
                        AND c.activo = true",
                )
                .bind(&pack.vertical_codigo)
                .bind(&codigo)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
                if disponible.is_none() {
                    return Err(ApiError::dominio(
                        "CAPACIDAD_NO_DISPONIBLE",
                        format!("La capacidad \"{codigo}\" no esta disponible para la vertical."),
                    ));
                }
                sqlx::query(
                    "INSERT INTO identity.empresa_capacidades AS ec
                       (empresa_id, capacidad_codigo, config, activo)
                     VALUES ($1::uuid, $2, '{}'::jsonb, true)
                     ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
                       SET activo = true, actualizado_en = NOW()",
                )
                .bind(&empresa_id)
                .bind(&codigo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            sqlx::query(
                "INSERT INTO identity.negocio_capacidades
                   (negocio_id, capacidad_codigo, config, activo)
                 SELECT n.id, ec.capacidad_codigo, ec.config, ec.activo
                   FROM identity.negocios n
                   JOIN identity.empresa_capacidades ec ON ec.empresa_id = n.empresa_id
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT (negocio_id, capacidad_codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "UPDATE identity.usuarios SET tenant_id = $1::uuid, rol = 'empresario'
                  WHERE id = $2::uuid",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos));
        }
        Err(ApiError::conflicto(
            "No se pudo asignar un slug disponible.",
        ))
    }

    /// Resuelve por UUID o slug (actual o histórico) y autoriza la membresía.
    pub async fn resolver_para(
        &self,
        usuario_id: &str,
        identificador: &str,
        es_global: bool,
        rol_miembro: Option<&str>,
    ) -> Result<(Value, Option<String>), ApiError> {
        let (datos, es_actual) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        if !es_global {
            self.autorizar_membresia(&empresa_id, usuario_id, rol_miembro)
                .await?;
        }
        let respuesta = Self::con_urls(&datos);
        let redireccion = if es_actual {
            None
        } else {
            Some(format!("/api/v2/empresas/{}", texto(&datos, "slug")))
        };
        Ok((respuesta, redireccion))
    }

    pub async fn actualizar_pack(
        &self,
        empresa_id: &str,
        pack_codigo: &str,
        version: Option<i32>,
    ) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e
              WHERE e.id = $1::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let vertical = datos.get("vertical_id").and_then(Value::as_str);
        let pack = resolver_pack(&mut tx, pack_codigo, vertical, version).await?;
        if texto(&datos, "pack_codigo") == pack.codigo
            && datos.get("pack_version").and_then(Value::as_i64) == Some(i64::from(pack.version))
        {
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos));
        }
        let antes: i32 = sqlx::query_scalar(
            "SELECT COUNT(*)::int FROM identity.empresa_capacidades WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let capacidades = sqlx::query(
            "SELECT capacidad_codigo, config, activo
               FROM identity.pack_capacidades
              WHERE pack_codigo = $1 AND pack_version = $2",
        )
        .bind(&pack.codigo)
        .bind(pack.version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let codigos: Vec<String> = capacidades
            .iter()
            .map(|fila| fila.try_get("capacidad_codigo").map_err(interno))
            .collect::<Result<_, _>>()?;
        sqlx::query(
            "DELETE FROM identity.empresa_capacidades
              WHERE empresa_id = $1::uuid AND NOT (capacidad_codigo = ANY($2::text[]))",
        )
        .bind(empresa_id)
        .bind(&codigos)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for capacidad in &capacidades {
            sqlx::query(
                "INSERT INTO identity.empresa_capacidades AS ec
                   (empresa_id, capacidad_codigo, config, activo)
                 VALUES ($1::uuid, $2, $3::jsonb, $4)
                 ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
                   SET config = EXCLUDED.config,
                       activo = EXCLUDED.activo,
                       actualizado_en = NOW()",
            )
            .bind(empresa_id)
            .bind(
                capacidad
                    .try_get::<String, _>("capacidad_codigo")
                    .map_err(interno)?,
            )
            .bind(capacidad.try_get::<Value, _>("config").map_err(interno)?)
            .bind(capacidad.try_get::<bool, _>("activo").map_err(interno)?)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        let actualizada = sqlx::query(
            "UPDATE identity.empresas AS e
                SET pack_codigo = $2, pack_version = $3
              WHERE e.id = $1::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(empresa_id)
        .bind(&pack.codigo)
        .bind(pack.version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let datos_nuevos: Value = actualizada.try_get("datos").map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.pack_migraciones
               (empresa_id, vertical_origen, pack_codigo, pack_version,
                capacidades_antes, capacidades_despues, estado, detalles)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, 'aplicado', $7::jsonb)
             ON CONFLICT (empresa_id) DO UPDATE
               SET pack_codigo = EXCLUDED.pack_codigo,
                   pack_version = EXCLUDED.pack_version,
                   capacidades_antes = EXCLUDED.capacidades_antes,
                   capacidades_despues = EXCLUDED.capacidades_despues,
                   estado = EXCLUDED.estado,
                   detalles = EXCLUDED.detalles,
                   migrado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(vertical)
        .bind(&pack.codigo)
        .bind(pack.version)
        .bind(antes)
        .bind(capacidades.len() as i32)
        .bind(json!({
            "origen": "pack_upgrade",
            "pack_anterior": datos.get("pack_codigo"),
            "version_anterior": datos.get("pack_version"),
            "pack_nuevo": pack.codigo,
            "version_nueva": pack.version,
        }))
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(Self::con_urls(&datos_nuevos))
    }

    pub async fn actualizar_slug(
        &self,
        usuario_id: &str,
        identificador: &str,
        slug_solicitado: &str,
    ) -> Result<Value, ApiError> {
        let (datos, _) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        self.autorizar_membresia(&empresa_id, usuario_id, Some("empresario"))
            .await?;
        let slug = validar_slug_solicitado(slug_solicitado)?;
        if slug == texto(&datos, "slug") {
            return Ok(Self::con_urls(&datos));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ocupado = sqlx::query("SELECT 1 FROM identity.empresa_slugs WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        if ocupado.is_some() {
            return Err(ApiError::conflicto(format!(
                "El slug \"{slug}\" ya está en uso."
            )));
        }
        let fila = sqlx::query(
            "UPDATE identity.empresas AS e SET slug = $1
              WHERE e.id = $2::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(&slug)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto(format!("El slug \"{slug}\" ya está en uso."))
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", &empresa_id))?;
        tx.commit().await.map_err(interno)?;
        let nuevos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(Self::con_urls(&nuevos))
    }

    /// Cambia el estado del tenant y revoca las sesiones si deja de estar
    /// activo. `inactiva` representa el cierre operativo; la reactivación
    /// queda disponible para un administrador de plataforma.
    pub async fn cambiar_estado(
        &self,
        empresa_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: &str,
    ) -> Result<Value, ApiError> {
        let estado = estado.trim().to_lowercase();
        if !ESTADOS_EMPRESA.contains(&estado.as_str()) {
            return Err(ApiError::dominio(
                "ESTADO_EMPRESA_INVALIDO",
                "Usa activa, suspendida o inactiva.",
            ));
        }
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT estado, to_jsonb(e) AS datos
               FROM identity.empresas e
              WHERE e.id = $1::uuid
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };
        let actual: String = fila.try_get("estado").map_err(interno)?;
        let datos_actuales: Value = fila.try_get("datos").map_err(interno)?;
        if actual == estado {
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos_actuales));
        }
        if !transicion_estado_permitida(&actual, &estado) {
            return Err(ApiError::dominio(
                "TRANSICION_ESTADO_INVALIDA",
                format!("No se puede pasar de \"{actual}\" a \"{estado}\"."),
            ));
        }

        let fila = sqlx::query(
            "UPDATE identity.empresas AS e
                SET estado = $1
              WHERE e.id = $2::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(&estado)
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;

        if estado != "activa" {
            revocar_sesiones_tenant(&mut tx, empresa_id).await?;
        }

        auditoria::registrar(
            &mut tx,
            actor_usuario_id,
            empresa_id,
            None,
            "empresa.estado",
            "empresa",
            Some(empresa_id),
            json!({
                "anterior": actual,
                "nuevo": estado,
                "motivo": motivo,
            }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(Self::con_urls(&datos))
    }

    /// Exporta los datos que posee identity para el tenant. Los datos de otros
    /// servicios se listan como pendientes en `alcance` para no fingir una
    /// exportación completa.
    pub async fn exportar(&self, empresa_id: &str, actor_id: &str) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let empresa: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e WHERE e.id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(empresa) = empresa else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };

        let asignaciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.creado_en), '[]'::jsonb)
               FROM identity.asignaciones_personal a WHERE a.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let usuarios = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                        'id', u.id, 'nombre', u.nombre, 'correo', u.correo,
                        'rol', u.rol, 'tenant_id', u.tenant_id,
                        'participacion_porcentaje', u.participacion_porcentaje,
                        'creado_en', u.creado_en
                    ) ORDER BY u.creado_en), '[]'::jsonb)
               FROM identity.usuarios u WHERE u.tenant_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let negocios = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(n) ORDER BY n.creado_en), '[]'::jsonb)
               FROM identity.negocios n WHERE n.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let partes = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.creado_en), '[]'::jsonb)
               FROM identity.partes p WHERE p.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let contactos = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.creado_en), '[]'::jsonb)
               FROM identity.medios_contacto c WHERE c.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let direcciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(d) ORDER BY d.creado_en), '[]'::jsonb)
               FROM identity.direcciones_partes d WHERE d.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let relaciones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.creado_en), '[]'::jsonb)
               FROM identity.relaciones_partes r WHERE r.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let capacidades = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.capacidad_codigo), '[]'::jsonb)
               FROM identity.empresa_capacidades c WHERE c.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let extensiones = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY v.creado_en), '[]'::jsonb)
               FROM identity.extensiones_empresa_valores v WHERE v.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let slugs = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY s.creado_en), '[]'::jsonb)
               FROM identity.empresa_slugs s WHERE s.empresa_id = $1::uuid",
            empresa_id,
        )
        .await?;
        let auditoria = jsonb_por_empresa(
            &mut tx,
            "SELECT COALESCE(jsonb_agg(datos ORDER BY creado_en DESC), '[]'::jsonb)
               FROM (
                   SELECT to_jsonb(a) AS datos, a.creado_en
                     FROM identity.auditoria a
                    WHERE a.empresa_id = $1::uuid
                    ORDER BY a.creado_en DESC
                    LIMIT 1000
               ) reciente",
            empresa_id,
        )
        .await?;

        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.export",
            "empresa",
            Some(empresa_id),
            json!({ "formato": "core-engine.tenant-export.v1" }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;

        Ok(json!({
            "formato": "core-engine.tenant-export.v1",
            "generado_en": crate::db::iso_ms(Utc::now()),
            "empresa": Self::con_urls(&empresa),
            "asignaciones_personal": asignaciones,
            "usuarios": usuarios,
            "negocios": negocios,
            "partes": partes,
            "contactos": contactos,
            "direcciones": direcciones,
            "relaciones": relaciones,
            "capacidades": capacidades,
            "extensiones_valores": extensiones,
            "slugs": slugs,
            "auditoria": auditoria,
            "alcance": {
                "incluye": ["identity"],
                "pendiente": [
                    "orders", "catalog", "inventory", "pricing",
                    "finance", "logistics", "analytics"
                ]
            }
        }))
    }

    /// Cierra el tenant y registra una solicitud de eliminación con retención.
    pub async fn solicitar_eliminacion(
        &self,
        empresa_id: &str,
        motivo: Option<&str>,
        retencion_dias: i32,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !(0..=3650).contains(&retencion_dias) {
            return Err(ApiError::dominio(
                "RETENCION_INVALIDA",
                "La retención debe estar entre 0 y 3650 días.",
            ));
        }
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let estado: Option<String> = sqlx::query_scalar(
            "SELECT estado FROM identity.empresas WHERE id = $1::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(estado) = estado else {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        };
        let pendiente: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado = 'pendiente' FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if pendiente.is_some() {
            return Err(ApiError::conflicto(
                "Ya existe una solicitud de eliminación pendiente para esta empresa.",
            ));
        }
        if estado != "inactiva" {
            sqlx::query("UPDATE identity.empresas SET estado = 'inactiva' WHERE id = $1::uuid")
                .bind(empresa_id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            revocar_sesiones_tenant(&mut tx, empresa_id).await?;
            auditoria::registrar(
                &mut tx,
                actor_id,
                empresa_id,
                None,
                "empresa.estado",
                "empresa",
                Some(empresa_id),
                json!({
                    "anterior": estado,
                    "nuevo": "inactiva",
                    "motivo": "eliminacion_solicitada",
                }),
            )
            .await?;
        }
        let fila = sqlx::query(
            "INSERT INTO identity.solicitudes_eliminacion_tenant AS s
               (empresa_id, motivo, retencion_hasta, solicitado_por)
             VALUES ($1::uuid, $2, CURRENT_DATE + $3::int, $4::uuid)
             RETURNING to_jsonb(s) AS datos,
                       (retencion_hasta <= CURRENT_DATE) AS purgable",
        )
        .bind(empresa_id)
        .bind(motivo.unwrap_or(""))
        .bind(retencion_dias)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let purgable: bool = fila.try_get("purgable").map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_solicitada",
            "empresa",
            Some(empresa_id),
            json!({ "retencion_dias": retencion_dias, "purgable": purgable }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "solicitud": datos, "purgable": purgable }))
    }

    pub async fn consultar_eliminacion(&self, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(empresa_id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(s) AS datos,
                    (s.estado = 'pendiente' AND s.retencion_hasta <= CURRENT_DATE) AS purgable
               FROM identity.solicitudes_eliminacion_tenant s
              WHERE s.empresa_id = $1::uuid
              ORDER BY s.creado_en DESC
              LIMIT 1",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                let purgable: bool = fila.try_get("purgable").map_err(interno)?;
                Ok(Some(json!({ "solicitud": datos, "purgable": purgable })))
            }
            None => Ok(None),
        }
    }

    pub async fn cancelar_eliminacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, (retencion_hasta <= CURRENT_DATE) AS vencida
               FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado IN ('pendiente', 'purgada')
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        };
        let vencida: bool = fila.try_get("vencida").map_err(interno)?;
        if vencida {
            return Err(ApiError::conflicto(
                "La retención ya venció; la solicitud no se puede cancelar.",
            ));
        }
        let fila = sqlx::query(
            "UPDATE identity.solicitudes_eliminacion_tenant AS s
                SET estado = 'cancelada', actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND estado = 'pendiente'
              RETURNING to_jsonb(s) AS datos",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_cancelada",
            "empresa",
            Some(empresa_id),
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "solicitud": datos, "purgable": false }))
    }

    /// Anonimiza los datos de identidad del tenant una vez vencida la
    /// retención. Conserva las filas ancla (`empresas`, `usuarios`, `partes`)
    /// para no romper referencias de otros servicios.
    pub async fn purgar_eliminacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, (retencion_hasta <= CURRENT_DATE) AS vencida
               FROM identity.solicitudes_eliminacion_tenant
              WHERE empresa_id = $1::uuid AND estado IN ('pendiente', 'purgada')
              FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado(
                "Solicitud de eliminación",
                empresa_id,
            ));
        };
        let solicitud_id: Uuid = fila.try_get("id").map_err(interno)?;
        let vencida: bool = fila.try_get("vencida").map_err(interno)?;
        if !vencida {
            return Err(ApiError::conflicto(
                "La retención aún no vence; no se puede purgar la identidad.",
            ));
        }

        let negocio_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM identity.negocios WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let usuario_ids: Vec<String> =
            sqlx::query_scalar("SELECT id::text FROM identity.usuarios WHERE tenant_id = $1::uuid")
                .bind(empresa_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;

        sqlx::query(
            "UPDATE identity.empresas
                SET nombre = 'Empresa eliminada',
                    slug = 'eliminada-' || replace(id::text, '-', ''),
                    estado = 'inactiva',
                    ruc = '', razon_social = '', nombre_comercial = '',
                    descripcion = '', telefono = '', correo_contacto = '',
                    sitio_web = '', direccion = '', ciudad = '',
                    departamento = '', pais = '', moneda = 'NIO',
                    zona_horaria = 'UTC', sector = '',
                    representante_legal = '', horario_atencion = '',
                    logo_objeto_id = NULL,
                    background_objeto_id = NULL,
                    background_media_type = NULL,
                    background_poster_objeto_id = NULL,
                    background_alt = '', background_overlay = 0,
                    background_position = 'center', background_activo = false,
                    plantilla_override = NULL, tema_override = '{}'::jsonb,
                    actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        sqlx::query(
            "UPDATE identity.usuarios
                SET nombre = 'Usuario eliminado',
                    correo = 'eliminado+' || replace(id::text, '-', '')
                             || '@core-engine.invalid',
                    contrasena_hash = '!eliminado',
                    personal_id = NULL,
                    participacion_porcentaje = NULL,
                    avatar_objeto_id = NULL,
                    actualizado_en = NOW()
              WHERE tenant_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        sqlx::query(
            "DELETE FROM identity.sesiones
              WHERE usuario_id IN (
                  SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
              )",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "DELETE FROM identity.password_reset_tokens
              WHERE usuario_id IN (
                  SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
              )",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for tabla in [
            "identity.api_keys",
            "identity.asignaciones_personal",
            "identity.invitaciones_personal",
            "identity.empresa_slugs",
            "identity.exportacion_jobs",
        ] {
            sqlx::query(&format!("DELETE FROM {tabla} WHERE empresa_id = $1::uuid"))
                .bind(empresa_id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
        }

        sqlx::query(
            "UPDATE identity.partes
                SET nombre = 'Parte eliminada', nombre_comercial = '',
                    identificador_valor = '', metadata = '{}'::jsonb,
                    estado = 'archivado', eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.medios_contacto
                SET valor = '', etiqueta = '', metadata = '{}'::jsonb,
                    verificado_en = NULL, eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.direcciones_partes
                SET etiqueta = '', linea_1 = '', linea_2 = '', ciudad = '',
                    departamento = '', pais = '', codigo_postal = '',
                    referencias = '', latitud = NULL, longitud = NULL,
                    metadata = '{}'::jsonb,
                    eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.relaciones_partes
                SET metadata = '{}'::jsonb, activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.partes_registros
                SET titulo = 'Registro eliminado', datos = '{}'::jsonb,
                    adjuntos = '[]'::jsonb,
                    eliminado_en = COALESCE(eliminado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.extensiones_empresa_valores
                SET valores = '{}'::jsonb, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.negocios
                SET nombre = 'Negocio eliminado', estado = 'inactivo',
                    archivado_en = COALESCE(archivado_en, NOW()),
                    actualizado_en = NOW()
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        let fila = sqlx::query(
            "UPDATE identity.solicitudes_eliminacion_tenant AS s
                SET estado = 'purgada', purgado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING to_jsonb(s) AS datos",
        )
        .bind(solicitud_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        // El rastro previo se conserva sin PII; el evento de purga se inserta
        // después y queda como constancia de la operación.
        sqlx::query(
            "UPDATE identity.auditoria
                SET actor_usuario_id = NULL, request_id = NULL, datos = '{}'::jsonb
              WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.eliminacion_purgada",
            "empresa",
            Some(empresa_id),
            json!({ "solicitud_id": solicitud_id.to_string() }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        let servicios = self
            .purga
            .ejecutar(empresa_id, &negocio_ids, &usuario_ids)
            .await;
        Ok(json!({
            "solicitud": datos,
            "purgable": false,
            "servicios": servicios,
        }))
    }

    /// Crea un job de exportación asíncrona (identidad + servicios).
    pub async fn iniciar_exportacion(
        &self,
        empresa_id: &str,
        actor_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let existe: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM identity.empresas WHERE id = $1::uuid")
                .bind(empresa_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.exportacion_jobs (empresa_id, solicitado_por)
             VALUES ($1::uuid, $2::uuid)
             RETURNING id, estado, formato, creado_en",
        )
        .bind(empresa_id)
        .bind(actor_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)?;
        let job_id: Uuid = fila.try_get("id").map_err(interno)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            actor_id,
            empresa_id,
            None,
            "empresa.export_solicitada",
            "empresa",
            Some(empresa_id),
            json!({ "job_id": job_id.to_string() }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({
            "job_id": job_id.to_string(),
            "estado": fila.try_get::<String, _>("estado").map_err(interno)?,
            "formato": fila.try_get::<String, _>("formato").map_err(interno)?,
            "creado_en": crate::db::iso_ms(fila.try_get::<chrono::DateTime<Utc>, _>("creado_en").map_err(interno)?),
        }))
    }

    pub async fn estado_exportacion(
        &self,
        empresa_id: &str,
        job_id: &str,
    ) -> Result<Value, ApiError> {
        if !es_uuid(empresa_id) || !es_uuid(job_id) {
            return Err(ApiError::no_encontrado("Exportación", job_id));
        }
        let datos: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(j) - 'contenido' AS datos
               FROM identity.exportacion_jobs j
              WHERE j.id = $1::uuid AND j.empresa_id = $2::uuid",
        )
        .bind(job_id)
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        datos.ok_or_else(|| ApiError::no_encontrado("Exportación", job_id))
    }

    /// Devuelve el archivo firmado de un job completado.
    pub async fn descargar_exportacion(
        &self,
        job_id: &str,
    ) -> Result<DescargaExportacion, ApiError> {
        if !es_uuid(job_id) {
            return Err(ApiError::no_encontrado("Exportación", job_id));
        }
        let fila = sqlx::query(
            "SELECT empresa_id, formato, contenido, sha256, firma, estado
               FROM identity.exportacion_jobs WHERE id = $1::uuid",
        )
        .bind(job_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Exportación", job_id))?;
        let estado: String = fila.try_get("estado").map_err(interno)?;
        if estado != "completado" {
            return Err(ApiError::conflicto(
                "La exportación aún no está completada.",
            ));
        }
        let contenido: Option<Vec<u8>> = fila.try_get("contenido").map_err(interno)?;
        let contenido = contenido.ok_or_else(ApiError::interno)?;
        Ok(DescargaExportacion {
            empresa_id: fila
                .try_get::<Uuid, _>("empresa_id")
                .map_err(interno)?
                .to_string(),
            formato: fila.try_get("formato").map_err(interno)?,
            sha256: fila.try_get("sha256").map_err(interno)?,
            firma: fila.try_get("firma").map_err(interno)?,
            contenido,
        })
    }

    /// Reanuda jobs que quedaron pendientes o en proceso tras un reinicio.
    pub async fn reanudar_exportaciones(&self) -> Result<Vec<String>, ApiError> {
        sqlx::query(
            "UPDATE identity.exportacion_jobs
                SET estado = 'pendiente', iniciado_en = NULL
              WHERE estado = 'en_proceso'",
        )
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        sqlx::query_scalar(
            "SELECT id::text FROM identity.exportacion_jobs
              WHERE estado = 'pendiente' ORDER BY creado_en LIMIT 50",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)
    }

    /// Ejecuta el job en segundo plano. Es idempotente: un job completado no
    /// se vuelve a generar.
    pub async fn ejecutar_exportacion(&self, job_id: &str) -> Result<(), ApiError> {
        if !es_uuid(job_id) {
            return Ok(());
        }
        let fila = match sqlx::query(
            "SELECT empresa_id, solicitado_por, estado
               FROM identity.exportacion_jobs WHERE id = $1::uuid",
        )
        .bind(job_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        {
            Some(fila) => fila,
            None => return Ok(()),
        };
        let estado: String = fila.try_get("estado").map_err(interno)?;
        if estado == "completado" || estado == "en_proceso" {
            return Ok(());
        }
        let empresa_id: Uuid = fila.try_get("empresa_id").map_err(interno)?;
        let actor_id: Option<Uuid> = fila.try_get("solicitado_por").map_err(interno)?;
        let actor_id = actor_id
            .map(|valor| valor.to_string())
            .unwrap_or_else(|| "sistema".to_string());
        let actualizado = sqlx::query(
            "UPDATE identity.exportacion_jobs
                SET estado = 'en_proceso', iniciado_en = NOW(), error = NULL
              WHERE id = $1::uuid AND estado IN ('pendiente', 'fallido')",
        )
        .bind(job_id)
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        if actualizado.rows_affected() == 0 {
            return Ok(());
        }
        let empresa = empresa_id.to_string();
        let negocio_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM identity.negocios WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(interno)?;
        let usuario_ids: Vec<String> =
            sqlx::query_scalar("SELECT id::text FROM identity.usuarios WHERE tenant_id = $1::uuid")
                .bind(empresa_id)
                .fetch_all(self.db.pool())
                .await
                .map_err(interno)?;

        let resultado: Result<(), ApiError> = async {
            let identidad = self.exportar(&empresa, &actor_id).await?;
            let servicios = self
                .purga
                .exportar(&empresa, &negocio_ids, &usuario_ids)
                .await;
            let cuerpo = json!({
                "formato": "core-engine.tenant-export.v2",
                "generado_en": crate::db::iso_ms(Utc::now()),
                "empresa_id": empresa,
                "identidad": identidad,
                "servicios": servicios,
            });
            let bytes = serde_json::to_vec(&cuerpo).map_err(|_| ApiError::interno())?;
            let sha = sha256_hex(&bytes);
            let firma = hmac_sha256_hex(self.firma_clave.as_bytes(), &bytes);
            let servicios_estado: Vec<Value> = cuerpo
                .get("servicios")
                .and_then(Value::as_array)
                .map(|valores| {
                    valores
                        .iter()
                        .map(|valor| {
                            json!({
                                "servicio": valor.get("servicio"),
                                "estado": valor.get("estado"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            sqlx::query(
                "UPDATE identity.exportacion_jobs
                    SET estado = 'completado', contenido = $2, tamano_bytes = $3,
                        sha256 = $4, firma = $5, servicios = $6::jsonb,
                        completado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(job_id)
            .bind(&bytes)
            .bind(bytes.len() as i64)
            .bind(&sha)
            .bind(&firma)
            .bind(sqlx::types::Json(&servicios_estado))
            .execute(self.db.pool())
            .await
            .map_err(interno)?;
            Ok(())
        }
        .await;
        if let Err(error) = resultado {
            let _ = sqlx::query(
                "UPDATE identity.exportacion_jobs
                    SET estado = 'fallido', error = $2, completado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(job_id)
            .bind(&error.mensaje)
            .execute(self.db.pool())
            .await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn actualizar_perfil(
        &self,
        usuario_id: &str,
        identificador: &str,
        cambios: &Value,
        es_global: bool,
    ) -> Result<Value, ApiError> {
        let (datos, _) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        if !es_global {
            self.autorizar_membresia(&empresa_id, usuario_id, Some("empresario"))
                .await?;
        }
        let meta = self.normalizar_perfil(cambios, &datos);
        self.validar_perfil(&meta)?;
        validar_background(&BackgroundInput::desde(cambios), &background_de(&meta))?;
        let ruc = texto(&meta, "ruc");
        if ruc != texto(&datos, "ruc") {
            self.validar_ruc_unico(&ruc, Some(&empresa_id)).await?;
        }
        let logo = meta.get("logo_objeto_id").and_then(Value::as_str);
        if logo != datos.get("logo_objeto_id").and_then(Value::as_str) {
            if let Some(logo) = logo {
                self.storage.validar_logo(&empresa_id, logo).await?;
            }
        }
        let mut media = meta
            .get("background_media_type")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(objeto) = meta.get("background_objeto_id").and_then(Value::as_str) {
            if Some(objeto) != datos.get("background_objeto_id").and_then(Value::as_str) {
                media = Some(
                    self.storage
                        .validar_media(&empresa_id, objeto, "fondo")
                        .await?,
                );
            }
        } else {
            media = None;
        }
        if let Some(poster) = meta
            .get("background_poster_objeto_id")
            .and_then(Value::as_str)
        {
            if Some(poster)
                != datos
                    .get("background_poster_objeto_id")
                    .and_then(Value::as_str)
            {
                self.storage
                    .validar_media(&empresa_id, poster, "poster")
                    .await?;
            }
        }
        let plantilla = meta
            .get("plantilla")
            .and_then(Value::as_str)
            .map(str::to_string);
        let tema = meta.get("tema").cloned().unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.empresas AS e SET
               ruc=$2, razon_social=$3, nombre_comercial=$4, descripcion=$5,
               telefono=$6, correo_contacto=$7, sitio_web=$8, direccion=$9,
               ciudad=$10, departamento=$11, pais=$12, moneda=$13, zona_horaria=$14,
               sector=$15, representante_legal=$16, horario_atencion=$17,
               logo_objeto_id=$18::uuid, background_objeto_id=$19::uuid,
               background_media_type=$20, background_poster_objeto_id=$21::uuid,
               background_alt=$22, background_overlay=$23, background_position=$24,
               background_activo=$25, plantilla_override=$26, tema_override=$27::jsonb,
               actualizado_en=NOW()
             WHERE e.id = $1::uuid
             RETURNING to_jsonb(e) AS datos",
        )
        .bind(&empresa_id)
        .bind(texto(&meta, "ruc"))
        .bind(texto(&meta, "razon_social"))
        .bind(texto(&meta, "nombre_comercial"))
        .bind(texto(&meta, "descripcion"))
        .bind(texto(&meta, "telefono"))
        .bind(texto(&meta, "correo_contacto"))
        .bind(texto(&meta, "sitio_web"))
        .bind(texto(&meta, "direccion"))
        .bind(texto(&meta, "ciudad"))
        .bind(texto(&meta, "departamento"))
        .bind(texto(&meta, "pais"))
        .bind(texto(&meta, "moneda"))
        .bind(texto(&meta, "zona_horaria"))
        .bind(texto(&meta, "sector"))
        .bind(texto(&meta, "representante_legal"))
        .bind(texto(&meta, "horario_atencion"))
        .bind(logo)
        .bind(meta.get("background_objeto_id").and_then(Value::as_str))
        .bind(media)
        .bind(
            meta.get("background_poster_objeto_id")
                .and_then(Value::as_str),
        )
        .bind(texto(&meta, "background_alt"))
        .bind(
            meta.get("background_overlay")
                .and_then(Value::as_i64)
                .unwrap_or(35),
        )
        .bind(texto(&meta, "background_position"))
        .bind(
            meta.get("background_activo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(plantilla)
        .bind(tema)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto("El RUC ya está registrado en otra empresa.")
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", &empresa_id))?;
        tx.commit().await.map_err(interno)?;
        let nuevos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(Self::con_urls(&nuevos))
    }

    pub async fn crear_vendedor(
        &self,
        actor_id: &str,
        empresa_id: &str,
        datos: CrearVendedorEmpresa,
    ) -> Result<Value, ApiError> {
        self.autorizar_membresia(empresa_id, actor_id, Some("empresario"))
            .await?;
        let usuario = self
            .usuarios
            .crear_vendedor_en_empresa(CrearVendedor {
                nombre: datos.nombre,
                correo: datos.correo,
                contrasena: datos.contrasena,
                empresa_id: empresa_id.to_string(),
                participacion_porcentaje: datos.participacion_porcentaje,
            })
            .await?;
        serde_json::to_value(usuario).map_err(|_| ApiError::interno())
    }

    async fn buscar_por_identificador(
        &self,
        identificador: &str,
    ) -> Result<Option<(Value, bool)>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = if es_uuid(identificador) {
            sqlx::query(
                "SELECT to_jsonb(e) AS datos, true AS es_actual
                   FROM identity.empresas e WHERE e.id = $1::uuid",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?
        } else {
            sqlx::query(
                "SELECT to_jsonb(e) AS datos, (s.reemplazado_en IS NULL) AS es_actual
                   FROM identity.empresa_slugs s
                   JOIN identity.empresas e ON e.id = s.empresa_id
                  WHERE s.slug = $1",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?
        };
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let es_actual: bool = fila.try_get("es_actual").map_err(interno)?;
        Ok(Some((datos, es_actual)))
    }

    async fn autorizar_membresia(
        &self,
        empresa_id: &str,
        usuario_id: &str,
        rol: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match rol {
            Some(rol) => sqlx::query(
                "SELECT 1 FROM identity.asignaciones_personal a
                   JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
                  WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                    AND r.codigo = $3 AND a.estado = 'activa'",
            )
            .bind(empresa_id)
            .bind(usuario_id)
            .bind(rol)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT 1 FROM identity.asignaciones_personal
                  WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'",
            )
            .bind(empresa_id)
            .bind(usuario_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
        };
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        Ok(())
    }

    fn normalizar_perfil(&self, datos: &Value, actual: &Value) -> Value {
        let texto = |clave: &str, respaldo: &str| {
            datos
                .get(clave)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_else(|| {
                    actual
                        .get(clave)
                        .and_then(Value::as_str)
                        .unwrap_or(respaldo)
                })
                .to_string()
        };
        let mut perfil = Map::new();
        perfil.insert("ruc".into(), json!(texto("ruc", "")));
        perfil.insert("razon_social".into(), json!(texto("razon_social", "")));
        perfil.insert(
            "nombre_comercial".into(),
            json!(texto("nombre_comercial", "")),
        );
        perfil.insert("descripcion".into(), json!(texto("descripcion", "")));
        perfil.insert("telefono".into(), json!(texto("telefono", "")));
        perfil.insert(
            "correo_contacto".into(),
            json!(texto("correo_contacto", "")),
        );
        perfil.insert("sitio_web".into(), json!(texto("sitio_web", "")));
        perfil.insert("direccion".into(), json!(texto("direccion", "")));
        perfil.insert("ciudad".into(), json!(texto("ciudad", "")));
        perfil.insert("departamento".into(), json!(texto("departamento", "")));
        perfil.insert(
            "pais".into(),
            json!(texto("pais", "Nicaragua").if_empty("Nicaragua")),
        );
        perfil.insert(
            "moneda".into(),
            json!(texto("moneda", "NIO").if_empty("NIO").to_uppercase()),
        );
        perfil.insert(
            "zona_horaria".into(),
            json!(texto("zona_horaria", "America/Managua").if_empty("America/Managua")),
        );
        perfil.insert("sector".into(), json!(texto("sector", "")));
        perfil.insert(
            "representante_legal".into(),
            json!(texto("representante_legal", "")),
        );
        perfil.insert(
            "horario_atencion".into(),
            json!(texto("horario_atencion", "")),
        );

        let logo = match datos.get("logo_objeto_id") {
            None => actual
                .get("logo_objeto_id")
                .and_then(Value::as_str)
                .map(|valor| json!(valor))
                .unwrap_or(Value::Null),
            Some(Value::Null) => Value::Null,
            Some(Value::String(valor)) => json!(valor.trim()),
            Some(_) => Value::Null,
        };
        perfil.insert("logo_objeto_id".into(), logo);

        let plantilla_actual = actual
            .get("plantilla_override")
            .and_then(Value::as_str)
            .filter(|valor| es_plantilla(valor))
            .map(|valor| json!(valor))
            .unwrap_or(Value::Null);
        let plantilla = match datos.get("plantilla") {
            None => plantilla_actual,
            Some(Value::Null) => Value::Null,
            Some(Value::String(valor)) if es_plantilla(valor) => json!(valor),
            Some(_) => plantilla_actual,
        };
        perfil.insert("plantilla".into(), plantilla);

        let actual_tema = actual
            .get("tema_override")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let tema = match datos.get("tema") {
            None => actual_tema,
            Some(Value::Null) => json!({}),
            Some(valor @ Value::Object(_)) => {
                let mut mezcla = actual_tema.as_object().cloned().unwrap_or_default();
                if let Some(parcial) = normalizar_tema_parcial(valor).as_object().cloned() {
                    for (clave, valor) in parcial {
                        mezcla.insert(clave, valor);
                    }
                }
                Value::Object(mezcla)
            }
            Some(_) => actual_tema,
        };
        perfil.insert("tema".into(), tema);

        let background =
            normalizar_background(&BackgroundInput::desde(datos), Some(&background_de(actual)));
        if let Some(objeto) = serde_json::to_value(background)
            .ok()
            .and_then(|valor| valor.as_object().cloned())
        {
            for (clave, valor) in objeto {
                perfil.insert(clave, valor);
            }
        }
        Value::Object(perfil)
    }

    fn validar_perfil(&self, meta: &Value) -> Result<(), ApiError> {
        let limites = [
            ("ruc", 30),
            ("razon_social", 160),
            ("nombre_comercial", 160),
            ("descripcion", 1000),
            ("telefono", 40),
            ("sitio_web", 200),
            ("direccion", 300),
            ("ciudad", 80),
            ("departamento", 80),
            ("pais", 80),
            ("zona_horaria", 64),
            ("sector", 80),
            ("representante_legal", 160),
            ("horario_atencion", 120),
        ];
        for (campo, maximo) in limites {
            if texto(meta, campo).chars().count() > maximo {
                return Err(ApiError::dominio(
                    "PERFIL_INVALIDO",
                    format!("{campo} supera {maximo} caracteres."),
                ));
            }
        }
        let moneda = texto(meta, "moneda");
        if moneda.len() != 3 || !moneda.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "La moneda debe ser un código ISO de 3 letras.",
            ));
        }
        let correo = texto(meta, "correo_contacto");
        if !correo.is_empty() && !correo_valido(&correo) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "El correo de contacto no es válido.",
            ));
        }
        if let Some(logo) = meta.get("logo_objeto_id").and_then(Value::as_str) {
            if !es_uuid(logo) {
                return Err(ApiError::dominio(
                    "PERFIL_INVALIDO",
                    "El logo debe ser un objeto de storage válido.",
                ));
            }
        }
        let plantilla = meta.get("plantilla").and_then(Value::as_str);
        if plantilla.is_some_and(|valor| !PLANTILLAS.contains(&valor)) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "La plantilla no es válida.",
            ));
        }
        validar_tema_input(meta.get("tema").unwrap_or(&json!({})))?;
        validar_tema(&mezclar_tema(&[meta.get("tema").unwrap_or(&json!({}))]))
    }

    async fn validar_ruc_unico(&self, ruc: &str, excluir_id: Option<&str>) -> Result<(), ApiError> {
        if ruc.is_empty() {
            return Ok(());
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match excluir_id {
            Some(id) => sqlx::query(
                "SELECT 1 FROM identity.empresas
                  WHERE upper(ruc) = upper($1) AND id <> $2::uuid LIMIT 1",
            )
            .bind(ruc)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => {
                sqlx::query("SELECT 1 FROM identity.empresas WHERE upper(ruc) = upper($1) LIMIT 1")
                    .bind(ruc)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(interno)?
            }
        };
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(
                "El RUC ya está registrado en otra empresa.",
            ));
        }
        Ok(())
    }

    fn con_urls(datos: &Value) -> Value {
        let mut salida = datos.as_object().cloned().unwrap_or_default();
        salida.remove("plantilla_override");
        salida.remove("tema_override");
        let background = background_de(datos);
        let con_urls = con_background_urls(background.clone());
        salida.insert(
            "background_objeto_id".into(),
            json!(con_urls.campos.background_objeto_id),
        );
        salida.insert(
            "background_media_type".into(),
            json!(con_urls.campos.background_media_type),
        );
        salida.insert(
            "background_poster_objeto_id".into(),
            json!(con_urls.campos.background_poster_objeto_id),
        );
        salida.insert(
            "background_alt".into(),
            json!(con_urls.campos.background_alt),
        );
        salida.insert(
            "background_overlay".into(),
            json!(con_urls.campos.background_overlay),
        );
        salida.insert(
            "background_position".into(),
            json!(con_urls.campos.background_position),
        );
        salida.insert(
            "background_activo".into(),
            json!(con_urls.campos.background_activo),
        );
        salida.insert("background_url".into(), json!(con_urls.background_url));
        salida.insert(
            "background_poster_url".into(),
            json!(con_urls.background_poster_url),
        );

        let overrides = HerenciaTema {
            plantilla: plantilla_de(datos.get("plantilla_override")),
            tema: tema_parcial_de(datos.get("tema_override").unwrap_or(&json!({}))),
        };
        let resuelta = resolver_personalizacion(
            &overrides,
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            &HerenciaTema {
                plantilla: None,
                tema: json!({}),
            },
            "empresa",
        );
        salida.insert("plantilla".into(), json!(resuelta.plantilla));
        salida.insert(
            "tema".into(),
            serde_json::to_value(&resuelta.tema).unwrap_or_else(|_| json!({})),
        );
        salida.insert(
            "origen_personalizacion".into(),
            json!({
                "plantilla": resuelta.origen.plantilla,
                "tema": resuelta.origen.tema,
            }),
        );
        if let Some(logo) = datos.get("logo_objeto_id").and_then(Value::as_str) {
            salida.insert(
                "logo_url".into(),
                json!(format!("/api/v2/storage/public/objetos/{logo}")),
            );
        }
        for clave in ["creado_en", "actualizado_en"] {
            if let Some(valor) = datos.get(clave).and_then(Value::as_str) {
                if let Ok(fecha) = chrono::DateTime::parse_from_rfc3339(valor) {
                    salida.insert(
                        clave.into(),
                        json!(crate::db::iso_ms(fecha.with_timezone(&chrono::Utc))),
                    );
                }
            }
        }
        Value::Object(salida)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackResuelto {
    codigo: String,
    version: i32,
    vertical_codigo: String,
}

async fn resolver_pack(
    tx: &mut Transaction<'_, Postgres>,
    codigo: &str,
    vertical: Option<&str>,
    version: Option<i32>,
) -> Result<PackResuelto, ApiError> {
    let fila = match version {
        Some(version) => sqlx::query(
            "SELECT codigo, version, vertical_codigo FROM identity.packs
              WHERE codigo = $1 AND version = $2 AND activo = true",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?,
        None => sqlx::query(
            "SELECT codigo, version, vertical_codigo FROM identity.packs
              WHERE codigo = $1 AND activo = true
              ORDER BY version DESC LIMIT 1",
        )
        .bind(codigo)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?,
    };
    let Some(fila) = fila else {
        return Err(ApiError::dominio(
            "PACK_INVALIDO",
            format!("El pack \"{codigo}\" no existe o esta inactivo."),
        ));
    };
    let pack = PackResuelto {
        codigo: fila.try_get("codigo").map_err(interno)?,
        version: fila.try_get::<i32, _>("version").map_err(interno)?,
        vertical_codigo: fila.try_get("vertical_codigo").map_err(interno)?,
    };
    if let Some(vertical) = vertical {
        if vertical != pack.vertical_codigo {
            return Err(ApiError::dominio(
                "PACK_VERTICAL_INCOMPATIBLE",
                "El pack no corresponde a la vertical solicitada.",
            ));
        }
    }
    Ok(pack)
}

async fn elegir_slug(
    tx: &mut Transaction<'_, Postgres>,
    nombre: &str,
    explicito: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(explicito) = explicito {
        if !slug_libre(tx, explicito).await? {
            return Err(ApiError::conflicto(format!(
                "El slug \"{explicito}\" ya está en uso."
            )));
        }
        return Ok(explicito.to_string());
    }
    let base = slug_desde_nombre(nombre, "empresa");
    for intento in 1..=MAX_INTENTOS_SLUG {
        let candidato = if intento == 1 {
            base.clone()
        } else {
            slug_con_sufijo(&base, intento)
        };
        if slug_libre(tx, &candidato).await? {
            return Ok(candidato);
        }
    }
    let sufijo = format!("-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let maximo = crate::dominio::slug::SLUG_MAX_LENGTH.saturating_sub(sufijo.len());
    let recorte = base
        .chars()
        .take(maximo)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();
    Ok(format!(
        "{}{sufijo}",
        if recorte.is_empty() {
            "empresa"
        } else {
            &recorte
        }
    ))
}

async fn slug_libre(tx: &mut Transaction<'_, Postgres>, slug: &str) -> Result<bool, ApiError> {
    let fila = sqlx::query("SELECT 1 FROM identity.empresa_slugs WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
    Ok(fila.is_none())
}

fn es_codigo_segmento(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Ejecuta una consulta que devuelve un `jsonb` agregado y recibe un único
/// parámetro `empresa_id`.
async fn jsonb_por_empresa(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    empresa_id: &str,
) -> Result<Value, ApiError> {
    sqlx::query_scalar(sql)
        .bind(empresa_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(interno)
}

/// Suspender/cerrar corta el refresh de todos los usuarios del tenant; los
/// access tokens vigentes expiran en su TTL corto.
async fn revocar_sesiones_tenant(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE identity.sesiones
            SET revocada_en = NOW()
          WHERE revocada_en IS NULL
            AND usuario_id IN (
                SELECT id FROM identity.usuarios WHERE tenant_id = $1::uuid
            )",
    )
    .bind(empresa_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

/// `inactiva` es el cierre operativo; solo un administrador puede reactivar.
fn transicion_estado_permitida(actual: &str, nuevo: &str) -> bool {
    matches!(
        (actual, nuevo),
        ("activa", "suspendida" | "inactiva")
            | ("suspendida", "activa" | "inactiva")
            | ("inactiva", "activa")
    )
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
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

trait IfEmpty {
    fn if_empty(self, respaldo: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, respaldo: &str) -> String {
        if self.is_empty() {
            respaldo.to_string()
        } else {
            self
        }
    }
}

impl Empresas {
    /// Lectura interna por id (sin autorización de miembro).
    pub async fn obtener(&self, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(empresa_id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e WHERE e.id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| {
            let datos: Value = fila.try_get("datos").map_err(interno)?;
            Ok(Self::con_urls(&datos))
        })
        .transpose()
    }

    /// Overrides crudos de tema para que stores resuelva la herencia (ADR-42).
    pub async fn tema_overrides(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT plantilla_override, tema_override FROM identity.empresas WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(json!({ "plantilla_override": Value::Null, "tema_override": {} }));
        };
        let plantilla: Option<String> = fila.try_get("plantilla_override").map_err(interno)?;
        let tema: Value = fila.try_get("tema_override").map_err(interno)?;
        Ok(json!({
            "plantilla_override": plantilla_de(plantilla.as_ref().map(|valor| json!(valor)).as_ref()),
            "tema_override": tema_parcial_de(&tema),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominio::slug::{es_slug_valido, normalizar_slug};

    #[test]
    fn debe_validar_la_vertical_y_el_slug() {
        assert!(es_codigo_segmento("retail"));
        assert!(es_codigo_segmento("servicios_domicilio"));
        assert!(!es_codigo_segmento("1retail"));
        assert!(!es_codigo_segmento("Retail"));
    }

    #[test]
    fn debe_normalizar_el_slug_desde_el_nombre() {
        assert_eq!(slug_desde_nombre("Mi Empresa", "empresa"), "mi-empresa");
        assert!(es_slug_valido(&normalizar_slug("Café Ñandú")));
    }

    #[test]
    fn debe_validar_las_transiciones_de_estado_del_tenant() {
        assert!(transicion_estado_permitida("activa", "suspendida"));
        assert!(transicion_estado_permitida("activa", "inactiva"));
        assert!(transicion_estado_permitida("suspendida", "activa"));
        assert!(transicion_estado_permitida("suspendida", "inactiva"));
        assert!(transicion_estado_permitida("inactiva", "activa"));
        assert!(!transicion_estado_permitida("inactiva", "suspendida"));
        assert!(!transicion_estado_permitida("suspendida", "suspendida"));
    }

    #[test]
    fn debe_limpiar_la_respuesta_con_el_tema_resuelto() {
        let datos = json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "nombre": "Empresa",
            "slug": "empresa",
            "logo_objeto_id": "22222222-2222-4222-8222-222222222222",
            "plantilla_override": "minimal",
            "tema_override": {"color_primario": "#112233"},
            "creado_en": "2026-11-13T00:00:00+00:00"
        });
        let salida = Empresas::con_urls(&datos);
        assert_eq!(salida["plantilla"], "minimal");
        assert_eq!(salida["tema"]["color_primario"], "#112233");
        assert_eq!(salida["origen_personalizacion"]["plantilla"], "empresa");
        assert!(salida.get("plantilla_override").is_none());
        assert_eq!(
            salida["logo_url"],
            "/api/v2/storage/public/objetos/22222222-2222-4222-8222-222222222222"
        );
        assert_eq!(salida["creado_en"], "2026-11-13T00:00:00.000Z");
    }
}
