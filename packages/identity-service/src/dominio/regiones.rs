//! Regiones tributarias por tenant (ADR-0049).
//!
//! La región es una dimensión de configuración, no una capacidad booleana:
//!
//! - `finance.jurisdicciones` es la fuente de verdad de las reglas fiscales
//!   (tasas, moneda, periodicidad, documento fiscal).
//! - `identity.pack_regiones` declara qué regiones admite cada pack.
//! - `identity.empresas.region_tributaria` guarda la elección del dueño.
//!
//! Este módulo es de solo lectura sobre el catálogo fiscal: las altas de
//! regiones siguen siendo responsabilidad de finance. La lectura directa del
//! catálogo evita una llamada HTTP durante el onboarding (mismo Postgres,
//! R-DB-2) y mantiene el arranque fail-closed si la región no existe.
//!
//! Añadir un país = insertar su fila fiscal, declararla en el pack y (si
//! aplica) configurar su adaptador electrónico. No se toca este módulo.

use crate::auth::UsuarioContexto;
use crate::db::Db;
use crate::dominio::auditoria;
use crate::errors::ApiError;
use chrono::NaiveDate;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

const FEATURE_PREFIJO_FISCAL: &str = "fiscal.";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RegionTributaria {
    pub codigo: String,
    pub nombre: String,
    pub pais_iso: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdivision: Option<String>,
    /// Símbolo histórico que usan los comprobantes (p. ej. `C$`).
    pub moneda: String,
    /// Código ISO 4217 que usan identity/orders (p. ej. `NIO`).
    pub moneda_iso: String,
    pub simbolo_moneda: String,
    pub locale: String,
    pub zona_horaria: String,
    pub tasa_iva: f64,
    pub tasa_ir: f64,
    pub periodicidad_declaracion: String,
    pub documento_fiscal: Value,
    pub perfil: Value,
    pub leyes: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vigente_desde: Option<String>,
    pub predeterminada: bool,
}

impl RegionTributaria {
    /// Reconstruye la región desde el `jsonb` que devuelve la consulta de
    /// packs. Mantiene un único mapeo cuando la región viaja anidada.
    pub fn desde_valor(valor: &Value) -> Self {
        let texto = |clave: &str| {
            valor
                .get(clave)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        Self {
            codigo: texto("codigo"),
            nombre: texto("nombre"),
            pais_iso: texto("pais_iso"),
            subdivision: valor
                .get("subdivision")
                .and_then(Value::as_str)
                .map(str::to_string),
            moneda: texto("moneda"),
            moneda_iso: texto("moneda_iso"),
            simbolo_moneda: texto("simbolo_moneda"),
            locale: texto("locale"),
            zona_horaria: texto("zona_horaria"),
            tasa_iva: valor.get("tasa_iva").and_then(Value::as_f64).unwrap_or(0.0),
            tasa_ir: valor.get("tasa_ir").and_then(Value::as_f64).unwrap_or(0.0),
            periodicidad_declaracion: texto("periodicidad_declaracion"),
            documento_fiscal: valor
                .get("documento_fiscal")
                .cloned()
                .unwrap_or_else(|| json!({})),
            perfil: valor.get("perfil").cloned().unwrap_or_else(|| json!({})),
            leyes: valor.get("leyes").cloned().unwrap_or_else(|| json!({})),
            vigente_desde: valor
                .get("vigente_desde")
                .and_then(Value::as_str)
                .map(str::to_string),
            predeterminada: valor
                .get("predeterminada")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RegionEmpresa {
    pub empresa_id: String,
    pub region: RegionTributaria,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desde: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmpresaRegion {
    pub empresa_id: String,
    pub pack_codigo: Option<String>,
    pub pack_version: Option<i32>,
    pub region_codigo: Option<String>,
    pub desde: Option<String>,
}

pub struct Regiones {
    db: Arc<Db>,
}

impl Regiones {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Regiones disponibles para un pack (o todas las activas si no se indica).
    pub async fn listar(
        &self,
        pack_codigo: Option<&str>,
        pack_version: Option<i32>,
    ) -> Result<Vec<RegionTributaria>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        if let Some(codigo) = pack_codigo {
            let version = pack_version.ok_or_else(|| {
                ApiError::dominio(
                    "PACK_VERSION_REQUERIDA",
                    "Indica la versión del pack para listar sus regiones.",
                )
            })?;
            asegurar_pack(&mut tx, codigo, version).await?;
            let regiones = regiones_de_pack(&mut tx, codigo, version).await?;
            tx.commit().await.map_err(interno)?;
            return Ok(regiones);
        }
        let regiones = todas_las_regiones(&mut tx).await?;
        tx.commit().await.map_err(interno)?;
        Ok(regiones)
    }

    /// Región efectiva de una empresa. Si la empresa es legacy y no tiene
    /// región, se resuelve la predeterminada de su pack.
    pub async fn de_empresa(&self, empresa_id: &str) -> Result<Option<RegionEmpresa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let region = region_de_empresa_en(&mut tx, empresa_id).await?;
        tx.commit().await.map_err(interno)?;
        Ok(region)
    }

    /// Valida y guarda una nueva región para la empresa. No toca documentos ya
    /// emitidos: solo cambia la jurisdicción para operaciones futuras.
    pub async fn asignar(
        &self,
        empresa_id: &str,
        region_codigo: &str,
        motivo: &str,
        actor: &UsuarioContexto,
    ) -> Result<RegionEmpresa, ApiError> {
        validar_acceso(self.db.as_ref(), empresa_id, actor).await?;
        let codigo = normalizar_codigo(region_codigo)?;
        let motivo = motivo.trim();
        if motivo.chars().count() > 300 {
            return Err(ApiError::validacion(json!([
                {"campo": "motivo", "restricciones": ["maxLength:300"]}
            ])));
        }

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let empresa = empresa_region(&mut tx, empresa_id).await?;
        let (Some(pack_codigo), Some(pack_version)) =
            (empresa.pack_codigo.as_deref(), empresa.pack_version)
        else {
            return Err(ApiError::dominio(
                "PACK_REQUERIDO",
                "La empresa no tiene pack; no se puede asignar una región tributaria.",
            ));
        };
        let nueva = resolver_en(&mut tx, pack_codigo, pack_version, Some(&codigo)).await?;

        if empresa.region_codigo.as_deref() == Some(nueva.codigo.as_str()) {
            tx.commit().await.map_err(interno)?;
            return Ok(RegionEmpresa {
                empresa_id: empresa.empresa_id,
                region: nueva,
                desde: empresa.desde,
            });
        }

        // La moneda y la zona horaria acompañan a la región fiscal. `pais` se
        // alinea al nombre de la región para que perfil y facturación no
        // diverjan.
        sqlx::query(
            "UPDATE identity.empresas
                SET region_tributaria = $2,
                    region_tributaria_desde = CURRENT_DATE,
                    pais = $3,
                    moneda = $4,
                    zona_horaria = $5
              WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .bind(&nueva.codigo)
        .bind(&nueva.nombre)
        .bind(&nueva.moneda_iso)
        .bind(&nueva.zona_horaria)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        // El override de empresa (negocio_id NULL) se realinea con la nueva
        // región y conserva sus flags de facturación electrónica. Los
        // overrides por negocio se respetan: son una decisión explícita.
        // R-MS-16/ADR-0051: finance es dueño de su configuración fiscal.
        sqlx::query_scalar::<_, i32>("SELECT finance.actualizar_region_fiscal($1::uuid, $2)")
            .bind(empresa_id)
            .bind(&nueva.codigo)
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?;

        sqlx::query(
            "INSERT INTO identity.empresa_region_historial
               (empresa_id, region_anterior, region_nueva, motivo, actor_id)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid)",
        )
        .bind(empresa_id)
        .bind(empresa.region_codigo.as_deref())
        .bind(&nueva.codigo)
        .bind(motivo)
        .bind(&actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            None,
            "empresa.region_tributaria_cambiada",
            "empresa",
            Some(empresa_id),
            json!({
                "region_anterior": empresa.region_codigo,
                "region_nueva": nueva.codigo,
                "motivo": motivo,
            }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;

        Ok(RegionEmpresa {
            empresa_id: empresa.empresa_id,
            region: nueva,
            desde: Some(chrono::Utc::now().date_naive().to_string()),
        })
    }

    /// Features fiscales que la región añade al manifiesto. Solo se exponen si
    /// la empresa tiene activa la capacidad `facturacion`: una región no
    /// enciende por sí sola módulos que el pack no incluye.
    pub fn features_fiscales(region: &RegionTributaria, facturacion_activa: bool) -> Vec<String> {
        if !facturacion_activa {
            return Vec::new();
        }
        region
            .perfil
            .get("features")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|feature| feature.starts_with(FEATURE_PREFIJO_FISCAL))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Resuelve la región efectiva dentro de una transacción existente.
///
/// - Si `solicitada` viene, debe estar declarada y activa para el pack.
/// - Si no viene, se elige la predeterminada del pack o, en su defecto, la
///   primera región activa declarada.
///
/// Falla cerrado: un pack sin regiones no puede crear empresas.
pub async fn resolver_en(
    tx: &mut Transaction<'_, Postgres>,
    pack_codigo: &str,
    pack_version: i32,
    solicitada: Option<&str>,
) -> Result<RegionTributaria, ApiError> {
    asegurar_pack(tx, pack_codigo, pack_version).await?;
    let opciones = regiones_de_pack(tx, pack_codigo, pack_version).await?;
    if opciones.is_empty() {
        return Err(ApiError::dominio(
            "REGION_NO_DISPONIBLE",
            "El pack no declara regiones tributarias activas.",
        ));
    }
    match solicitada {
        Some(codigo) => {
            let codigo = normalizar_codigo(codigo)?;
            opciones
                .into_iter()
                .find(|region| region.codigo == codigo)
                .ok_or_else(|| {
                    ApiError::dominio(
                        "REGION_NO_DISPONIBLE",
                        format!("La región \"{codigo}\" no está disponible para el pack."),
                    )
                })
        }
        None => Ok(opciones
            .iter()
            .find(|region| region.predeterminada)
            .cloned()
            .unwrap_or_else(|| opciones[0].clone())),
    }
}

/// Resuelve la región efectiva de una empresa dentro de una transacción
/// existente. Útil para manifiestos que ya abrieron su propia transacción.
pub async fn region_de_empresa_en(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
) -> Result<Option<RegionEmpresa>, ApiError> {
    let empresa = empresa_region(tx, empresa_id).await?;
    let region = match empresa.region_codigo.as_deref() {
        Some(codigo) => region_por_codigo(tx, codigo).await?,
        None => match (empresa.pack_codigo.as_deref(), empresa.pack_version) {
            (Some(pack), Some(version)) => resolver_en(tx, pack, version, None).await.ok(),
            _ => None,
        },
    };
    Ok(region.map(|region| RegionEmpresa {
        empresa_id: empresa.empresa_id.clone(),
        region,
        desde: empresa.desde.clone(),
    }))
}

/// Datos mínimos de la empresa para resolver su región.
pub async fn empresa_region(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
) -> Result<EmpresaRegion, ApiError> {
    let fila = sqlx::query(
        "SELECT id::text AS empresa_id, pack_codigo, pack_version,
                region_tributaria,
                region_tributaria_desde::text AS desde
           FROM identity.empresas
          WHERE id = $1::uuid",
    )
    .bind(empresa_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(interno)?
    .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
    Ok(EmpresaRegion {
        empresa_id: fila.try_get("empresa_id").map_err(interno)?,
        pack_codigo: fila.try_get("pack_codigo").map_err(interno)?,
        pack_version: fila.try_get("pack_version").map_err(interno)?,
        region_codigo: fila.try_get("region_tributaria").map_err(interno)?,
        desde: fila.try_get("desde").map_err(interno)?,
    })
}

/// Privado: valida que el actor sea admin global o empresario activo del tenant.
async fn validar_acceso(
    db: &Db,
    empresa_id: &str,
    actor: &UsuarioContexto,
) -> Result<(), ApiError> {
    if actor.es_admin_global() {
        return Ok(());
    }
    if actor.rol != "empresario" || actor.tenant_id.as_deref() != Some(empresa_id) {
        return Err(ApiError::acceso_denegado(
            "Solo el empresario de la empresa puede cambiar su región tributaria.",
        ));
    }
    let mut tx = db.transaccion().await.map_err(interno)?;
    let miembro = sqlx::query(
        "SELECT 1
           FROM identity.asignaciones_personal a
           JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
          WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
            AND r.codigo = 'empresario' AND a.estado = 'activa'",
    )
    .bind(empresa_id)
    .bind(&actor.user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    if miembro.is_none() {
        return Err(ApiError::acceso_denegado(
            "El usuario no es empresario activo de esta empresa.",
        ));
    }
    Ok(())
}

async fn asegurar_pack(
    tx: &mut Transaction<'_, Postgres>,
    pack_codigo: &str,
    pack_version: i32,
) -> Result<(), ApiError> {
    let existe = sqlx::query(
        "SELECT 1 FROM identity.packs
          WHERE codigo = $1 AND version = $2 AND activo = true",
    )
    .bind(pack_codigo)
    .bind(pack_version)
    .fetch_optional(&mut **tx)
    .await
    .map_err(interno)?;
    if existe.is_none() {
        return Err(ApiError::no_encontrado(
            "Pack",
            &format!("{pack_codigo}@{pack_version}"),
        ));
    }
    Ok(())
}

async fn regiones_de_pack(
    tx: &mut Transaction<'_, Postgres>,
    pack_codigo: &str,
    pack_version: i32,
) -> Result<Vec<RegionTributaria>, ApiError> {
    let filas = sqlx::query(&format!(
        "{SELECT_REGION_PACK}
          WHERE j.activa = true
            AND pr.activo = true
            AND pr.pack_codigo = $1
            AND pr.pack_version = $2
          ORDER BY pr.predeterminada DESC, j.codigo_pais"
    ))
    .bind(pack_codigo)
    .bind(pack_version)
    .fetch_all(&mut **tx)
    .await
    .map_err(interno)?;
    filas.iter().map(region_de_fila).collect()
}

async fn todas_las_regiones(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<RegionTributaria>, ApiError> {
    let filas = sqlx::query(&format!(
        "{SELECT_REGION_SIMPLE}
          WHERE j.activa = true
          ORDER BY j.codigo_pais"
    ))
    .fetch_all(&mut **tx)
    .await
    .map_err(interno)?;
    filas.iter().map(region_de_fila).collect()
}

async fn region_por_codigo(
    tx: &mut Transaction<'_, Postgres>,
    codigo: &str,
) -> Result<Option<RegionTributaria>, ApiError> {
    let fila = sqlx::query(&format!(
        "{SELECT_REGION_SIMPLE}
          WHERE j.codigo_pais = $1"
    ))
    .bind(codigo)
    .fetch_optional(&mut **tx)
    .await
    .map_err(interno)?;
    fila.as_ref().map(region_de_fila).transpose()
}

const SELECT_REGION_PACK: &str = "SELECT j.codigo_pais, j.nombre, j.pais_iso, j.subdivision,
        j.moneda, j.moneda_iso, j.simbolo_moneda, j.locale, j.zona_horaria,
        j.tasa_iva, j.tasa_ir, j.periodicidad_declaracion, j.documento_fiscal,
        j.perfil, j.leyes, j.vigente_desde, j.activa,
        COALESCE(pr.predeterminada, false) AS predeterminada
   FROM finance.jurisdicciones j
   LEFT JOIN identity.pack_regiones pr
     ON pr.region_codigo = j.codigo_pais
    AND pr.pack_codigo = $1
    AND pr.pack_version = $2";

const SELECT_REGION_SIMPLE: &str = "SELECT j.codigo_pais, j.nombre, j.pais_iso, j.subdivision,
        j.moneda, j.moneda_iso, j.simbolo_moneda, j.locale, j.zona_horaria,
        j.tasa_iva, j.tasa_ir, j.periodicidad_declaracion, j.documento_fiscal,
        j.perfil, j.leyes, j.vigente_desde, j.activa,
        false AS predeterminada
   FROM finance.jurisdicciones j";

pub fn region_de_fila(fila: &sqlx::postgres::PgRow) -> Result<RegionTributaria, ApiError> {
    let vigente_desde: Option<NaiveDate> = fila.try_get("vigente_desde").map_err(interno)?;
    Ok(RegionTributaria {
        codigo: fila.try_get("codigo_pais").map_err(interno)?,
        nombre: fila.try_get("nombre").map_err(interno)?,
        pais_iso: fila
            .try_get::<Option<String>, _>("pais_iso")
            .map_err(interno)?
            .unwrap_or_default(),
        subdivision: fila.try_get("subdivision").map_err(interno)?,
        moneda: fila.try_get("moneda").map_err(interno)?,
        moneda_iso: fila
            .try_get::<Option<String>, _>("moneda_iso")
            .map_err(interno)?
            .unwrap_or_default(),
        simbolo_moneda: fila.try_get("simbolo_moneda").map_err(interno)?,
        locale: fila.try_get("locale").map_err(interno)?,
        zona_horaria: fila.try_get("zona_horaria").map_err(interno)?,
        tasa_iva: fila.try_get::<f64, _>("tasa_iva").unwrap_or(0.0),
        tasa_ir: fila.try_get::<f64, _>("tasa_ir").unwrap_or(0.0),
        periodicidad_declaracion: fila.try_get("periodicidad_declaracion").map_err(interno)?,
        documento_fiscal: fila.try_get("documento_fiscal").map_err(interno)?,
        perfil: fila.try_get("perfil").map_err(interno)?,
        leyes: fila.try_get("leyes").map_err(interno)?,
        vigente_desde: vigente_desde.map(|fecha| fecha.to_string()),
        predeterminada: fila.try_get("predeterminada").map_err(interno)?,
    })
}

fn normalizar_codigo(codigo: &str) -> Result<String, ApiError> {
    let codigo = codigo.trim().to_uppercase();
    if codigo.is_empty() || codigo.chars().count() > 16 {
        return Err(ApiError::validacion(json!([
            {"campo": "region_tributaria", "restricciones": ["minLength:1", "maxLength:16"]}
        ])));
    }
    if !codigo.chars().all(|caracter| {
        caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
    }) {
        return Err(ApiError::validacion(json!([
            {"campo": "region_tributaria", "restricciones": ["pattern:^[A-Z0-9-]{1,16}$"]}
        ])));
    }
    Ok(codigo)
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.regiones",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_y_validar_codigos_de_region() {
        assert_eq!(normalizar_codigo(" ni ").expect("ni"), "NI");
        assert_eq!(normalizar_codigo("us-ca").expect("us-ca"), "US-CA");
        assert!(normalizar_codigo("").is_err());
        assert!(normalizar_codigo("NI/RN").is_err());
        assert!(normalizar_codigo(&"X".repeat(17)).is_err());
    }

    #[test]
    fn debe_exponer_solo_features_fiscales_y_solo_con_facturacion() {
        let region = RegionTributaria {
            codigo: "NI".into(),
            nombre: "Nicaragua".into(),
            pais_iso: "NI".into(),
            subdivision: None,
            moneda: "C$".into(),
            moneda_iso: "NIO".into(),
            simbolo_moneda: "C$".into(),
            locale: "es-NI".into(),
            zona_horaria: "America/Managua".into(),
            tasa_iva: 0.15,
            tasa_ir: 0.30,
            periodicidad_declaracion: "MENSUAL".into(),
            documento_fiscal: json!({"tipo": "RUC"}),
            perfil: json!({"features": [
                "fiscal.iva",
                "fiscal.facturacion_electronica",
                "presentacion.es-NI"
            ]}),
            leyes: json!({}),
            vigente_desde: Some("2026-01-01".into()),
            predeterminada: true,
        };
        assert!(Regiones::features_fiscales(&region, false).is_empty());
        assert_eq!(
            Regiones::features_fiscales(&region, true),
            vec![
                "fiscal.iva".to_string(),
                "fiscal.facturacion_electronica".to_string()
            ]
        );
    }
}
