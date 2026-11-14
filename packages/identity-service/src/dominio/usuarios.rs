//! Usuarios, credenciales y perfiles (RF-01).
//!
//! Réplica de `usuarios/usuarios.service.ts`: el hash nunca sale del servicio
//! (mínima exposición de datos) y los roles/tenant se copian al JWT.

#[cfg(test)]
use crate::auth::ROLES;
use crate::auth::{es_rol, hash_contrasena, verificar_contrasena, UsuarioContexto};
use crate::db::{iso_ms, Db};
use crate::errors::ApiError;
use crate::storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;

const COLUMNAS: &str = "u.id, u.nombre, u.correo, u.rol, u.tenant_id, u.personal_id,
       u.participacion_porcentaje, u.avatar_objeto_id, u.creado_en,
       (SELECT a.rol_empresa_id FROM identity.asignaciones_personal a
         WHERE a.usuario_id = u.id AND a.empresa_id = u.tenant_id AND a.estado = 'activa'
         LIMIT 1) AS rol_empresa_id";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Usuario {
    pub id: String,
    pub nombre: String,
    pub correo: String,
    pub rol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participacion_porcentaje: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol_empresa_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_objeto_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsuarioConHash {
    pub usuario: Usuario,
    pub contrasena_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrearUsuario {
    pub nombre: String,
    pub correo: String,
    pub contrasena: String,
    pub rol: String,
    pub tenant_id: Option<String>,
    pub personal_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FiltroUsuarios {
    pub rol: Option<String>,
    pub tenant_id: Option<String>,
}

pub struct Usuarios {
    db: Arc<Db>,
    storage: Arc<Storage>,
}

impl Usuarios {
    pub fn nueva(db: Arc<Db>, storage: Arc<Storage>) -> Self {
        Self { db, storage }
    }

    pub async fn encontrar_por_id(&self, id: &str) -> Result<Option<Usuario>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM identity.usuarios u WHERE u.id = $1::uuid"
        ))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| sanear(&fila)).transpose().map_err(interno)
    }

    pub async fn encontrar_por_correo(
        &self,
        correo: &str,
    ) -> Result<Option<UsuarioConHash>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}, u.contrasena_hash
               FROM identity.usuarios u
              WHERE LOWER(u.correo) = LOWER($1)"
        ))
        .bind(correo.trim())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| {
            let contrasena_hash: String = fila.try_get("contrasena_hash")?;
            Ok(UsuarioConHash {
                usuario: sanear(&fila)?,
                contrasena_hash,
            })
        })
        .transpose()
        .map_err(interno)
    }

    pub async fn crear(&self, datos: CrearUsuario) -> Result<Usuario, ApiError> {
        let correo = datos.correo.trim().to_lowercase();
        if !correo_valido(&correo) {
            return Err(ApiError::dominio(
                "CORREO_INVALIDO",
                "El correo no tiene un formato valido.",
            ));
        }
        if datos.contrasena.chars().count() < 8 {
            return Err(ApiError::dominio(
                "CONTRASENA_DEBIL",
                "La contrasena debe tener al menos 8 caracteres.",
            ));
        }
        if !es_rol(&datos.rol) {
            return Err(ApiError::dominio(
                "ROL_INVALIDO",
                "El rol indicado no es valido.",
            ));
        }
        if self.encontrar_por_correo(&correo).await?.is_some() {
            return Err(ApiError::conflicto("Ya existe una cuenta con ese correo."));
        }
        let hash = hash_contrasena(&datos.contrasena)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.usuarios
               (id, nombre, correo, contrasena_hash, rol, tenant_id, personal_id)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5::uuid, $6::uuid)
             RETURNING id",
        )
        .bind(datos.nombre.trim())
        .bind(&correo)
        .bind(hash)
        .bind(&datos.rol)
        .bind(datos.tenant_id.as_deref())
        .bind(datos.personal_id.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if es_violacion_unicidad(&error) {
                ApiError::conflicto("Ya existe una cuenta con ese correo.")
            } else {
                interno(error)
            }
        })?;
        let id: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        self.encontrar_por_id(&id.to_string())
            .await?
            .ok_or_else(|| ApiError::dominio("REGISTRO_FALLIDO", "No se pudo crear el usuario."))
    }

    pub async fn verificar_credenciales(
        &self,
        correo: &str,
        contrasena: &str,
    ) -> Result<Usuario, ApiError> {
        let Some(fila) = self.encontrar_por_correo(correo).await? else {
            return Err(credenciales_invalidas());
        };
        if !verificar_contrasena(contrasena, &fila.contrasena_hash) {
            return Err(credenciales_invalidas());
        }
        Ok(fila.usuario)
    }

    pub async fn existe_admin(&self) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM identity.usuarios WHERE rol IN ('super_admin', 'admin') LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn contar_vendedores(&self) -> Result<i64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM identity.usuarios WHERE rol = 'vendedor'",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(total)
    }

    pub async fn listar_vendedores(&self) -> Result<Vec<Usuario>, ApiError> {
        self.listar(FiltroUsuarios {
            rol: Some("vendedor".to_string()),
            tenant_id: None,
        })
        .await
    }

    pub async fn listar(&self, filtro: FiltroUsuarios) -> Result<Vec<Usuario>, ApiError> {
        let mut condiciones = Vec::new();
        let mut parametros: Vec<String> = Vec::new();
        if let Some(rol) = filtro.rol {
            parametros.push(rol);
            condiciones.push(format!("u.rol = ${}", parametros.len()));
        }
        if let Some(tenant) = filtro.tenant_id {
            parametros.push(tenant);
            condiciones.push(format!("u.tenant_id = ${}::uuid", parametros.len()));
        }
        let where_clause = if condiciones.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", condiciones.join(" AND "))
        };
        let sql = format!(
            "SELECT {COLUMNAS} FROM identity.usuarios u{where_clause}
             ORDER BY u.creado_en DESC LIMIT 500"
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
            .map(sanear)
            .collect::<Result<_, _>>()
            .map_err(interno)
    }

    pub async fn cambiar_contrasena(
        &self,
        id: &str,
        actual: &str,
        nueva: &str,
    ) -> Result<(), ApiError> {
        let Some(usuario) = self.encontrar_por_id(id).await? else {
            return Err(ApiError::no_encontrado("Usuario", id));
        };
        let Some(fila) = self.encontrar_por_correo(&usuario.correo).await? else {
            return Err(ApiError::no_encontrado("Usuario", id));
        };
        if !verificar_contrasena(actual, &fila.contrasena_hash) {
            return Err(ApiError::no_autorizado(
                "La contrasena actual es incorrecta.",
            ));
        }
        if nueva.chars().count() < 8 {
            return Err(ApiError::dominio(
                "CONTRASENA_DEBIL",
                "La nueva contrasena debe tener al menos 8 caracteres.",
            ));
        }
        let hash = hash_contrasena(nueva)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query("UPDATE identity.usuarios SET contrasena_hash = $1 WHERE id = $2::uuid")
            .bind(hash)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn vincular_personal(&self, id: &str, personal_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query("UPDATE identity.usuarios SET personal_id = $1::uuid WHERE id = $2::uuid")
            .bind(personal_id)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    /// Asocia un avatar público ya confirmado en storage. El vendedor puede
    /// editar el suyo; el empresario, los de su tenant; admin/super_admin, todos.
    pub async fn actualizar_avatar(
        &self,
        id: &str,
        avatar_objeto_id: Option<&str>,
        actor: &UsuarioContexto,
    ) -> Result<Usuario, ApiError> {
        let objetivo = self
            .encontrar_por_id(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Usuario", id))?;
        if !puede_cambiar_avatar(actor, id, objetivo.tenant_id.as_deref()) {
            return Err(ApiError::acceso_denegado(
                "No puedes cambiar el avatar de este usuario.",
            ));
        }
        if let Some(objeto_id) = avatar_objeto_id {
            if !es_uuid(objeto_id) {
                return Err(ApiError::dominio(
                    "AVATAR_INVALIDO",
                    "El avatar debe ser un objeto de storage válido.",
                ));
            }
            let tenant = objetivo
                .tenant_id
                .as_deref()
                .or(actor.tenant_id.as_deref())
                .ok_or_else(|| {
                    ApiError::dominio(
                        "TENANT_REQUERIDO",
                        "El usuario no tiene tenant para el avatar.",
                    )
                })?;
            self.storage.validar_avatar(tenant, objeto_id).await?;
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE identity.usuarios
                SET avatar_objeto_id = $1::uuid, actualizado_en = NOW()
              WHERE id = $2::uuid",
        )
        .bind(avatar_objeto_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        self.encontrar_por_id(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Usuario", id))
    }

    pub async fn actualizar_perfil(
        &self,
        id: &str,
        nombre: Option<&str>,
        correo: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut asignaciones = Vec::new();
        let mut parametros: Vec<String> = Vec::new();
        if let Some(nombre) = nombre {
            parametros.push(nombre.trim().to_string());
            asignaciones.push(format!("nombre = ${}", parametros.len()));
        }
        if let Some(correo) = correo {
            let correo = correo.trim().to_lowercase();
            if !correo_valido(&correo) {
                return Err(ApiError::dominio(
                    "CORREO_INVALIDO",
                    "El correo no tiene un formato valido.",
                ));
            }
            parametros.push(correo);
            asignaciones.push(format!("correo = ${}", parametros.len()));
        }
        if asignaciones.is_empty() {
            return Ok(());
        }
        parametros.push(id.to_string());
        let sql = format!(
            "UPDATE identity.usuarios SET {} WHERE id = ${}::uuid",
            asignaciones.join(", "),
            parametros.len()
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &parametros {
            consulta = consulta.bind(parametro);
        }
        consulta.execute(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}

fn sanear(fila: &PgRow) -> Result<Usuario, sqlx::Error> {
    let id: uuid::Uuid = fila.try_get("id")?;
    let tenant_id: Option<uuid::Uuid> = fila.try_get("tenant_id")?;
    let personal_id: Option<uuid::Uuid> = fila.try_get("personal_id")?;
    let avatar_objeto_id: Option<uuid::Uuid> = fila.try_get("avatar_objeto_id")?;
    let rol_empresa_id: Option<uuid::Uuid> = fila.try_get("rol_empresa_id")?;
    let participacion: Option<i32> = fila.try_get("participacion_porcentaje")?;
    let creado_en: DateTime<Utc> = fila.try_get("creado_en")?;
    let avatar_texto = avatar_objeto_id.map(|valor| valor.to_string());
    Ok(Usuario {
        id: id.to_string(),
        nombre: fila.try_get("nombre")?,
        correo: fila.try_get("correo")?,
        rol: fila.try_get("rol")?,
        tenant_id: tenant_id.map(|valor| valor.to_string()),
        personal_id: personal_id.map(|valor| valor.to_string()),
        participacion_porcentaje: participacion.map(i64::from),
        rol_empresa_id: rol_empresa_id.map(|valor| valor.to_string()),
        avatar_url: avatar_texto
            .as_ref()
            .map(|id| format!("/api/v2/storage/public/objetos/{id}")),
        avatar_objeto_id: avatar_texto,
        creado_en: iso_ms(creado_en),
    })
}

/// Contrato de `actualizarAvatar`: global, el propio usuario o el empresario
/// del mismo tenant.
pub fn puede_cambiar_avatar(
    actor: &UsuarioContexto,
    objetivo_id: &str,
    tenant_objetivo: Option<&str>,
) -> bool {
    actor.es_admin_global()
        || actor.user_id == objetivo_id
        || (actor.rol == "empresario"
            && actor.tenant_id.is_some()
            && actor.tenant_id.as_deref() == tenant_objetivo)
}

pub(crate) fn correo_valido(correo: &str) -> bool {
    let Some((usuario, dominio)) = correo.split_once('@') else {
        return false;
    };
    !usuario.is_empty()
        && !dominio.is_empty()
        && !dominio.contains('@')
        && !correo.contains(char::is_whitespace)
        && !dominio.starts_with('.')
        && !dominio.ends_with('.')
        && dominio
            .split('.')
            .all(|etiqueta| !etiqueta.is_empty() && etiqueta.len() <= 63)
}

fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

fn credenciales_invalidas() -> ApiError {
    ApiError::no_autorizado("Correo o contrasena incorrectos.")
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

/// Roles admitidos por el alta administrativa (`crear-usuario`).
pub fn roles_creables() -> [&'static str; 7] {
    [
        "empresario",
        "comprador",
        "admin",
        "logistica",
        "coordinador",
        "supervisor",
        "operativo",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_correos_con_el_mismo_criterio() {
        assert!(correo_valido("ana@tienda.test"));
        assert!(correo_valido("a.b+c@sub.dominio.com"));
        assert!(correo_valido("ana@localhost"));
        assert!(!correo_valido("sin-arroba"));
        assert!(!correo_valido("@dominio.com"));
        assert!(!correo_valido("ana@"));
        assert!(!correo_valido("ana@@dominio.com"));
        assert!(!correo_valido("ana @dominio.com"));
        assert!(!correo_valido("ana@dominio..com"));
    }

    #[test]
    fn debe_reconocer_los_roles_del_contrato() {
        assert_eq!(ROLES.len(), 9);
        assert!(ROLES.contains(&"logistica"));
        assert!(!ROLES.contains(&"personal"));
    }

    #[test]
    fn debe_listar_los_roles_creables_por_admin() {
        let roles = roles_creables();
        assert!(roles.contains(&"admin"));
        assert!(roles.contains(&"operativo"));
        assert!(roles.contains(&"comprador"));
    }
}

/// Alta de vendedor dentro de una empresa (usada por empresas y personal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrearVendedor {
    pub nombre: String,
    pub correo: String,
    pub contrasena: String,
    pub empresa_id: String,
    pub participacion_porcentaje: i32,
}

impl Usuarios {
    pub async fn crear_vendedor_en_empresa(
        &self,
        datos: CrearVendedor,
    ) -> Result<Usuario, ApiError> {
        let correo = datos.correo.trim().to_lowercase();
        if !correo_valido(&correo) {
            return Err(ApiError::dominio(
                "CORREO_INVALIDO",
                "El correo no tiene un formato valido.",
            ));
        }
        if datos.contrasena.chars().count() < 8 {
            return Err(ApiError::dominio(
                "CONTRASENA_DEBIL",
                "La contrasena debe tener al menos 8 caracteres.",
            ));
        }
        if !(0..=100).contains(&datos.participacion_porcentaje) {
            return Err(ApiError::dominio(
                "PARTICIPACION_INVALIDA",
                "La participacion debe estar entre 0 y 100.",
            ));
        }
        let hash = hash_contrasena(&datos.contrasena)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existente =
            sqlx::query("SELECT 1 FROM identity.usuarios WHERE LOWER(correo) = LOWER($1)")
                .bind(&correo)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
        if existente.is_some() {
            return Err(ApiError::conflicto("Ya existe una cuenta con ese correo."));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.usuarios
               (nombre, correo, contrasena_hash, rol, tenant_id, participacion_porcentaje)
             VALUES ($1, $2, $3, 'vendedor', $4::uuid, $5)
             RETURNING id",
        )
        .bind(datos.nombre.trim())
        .bind(&correo)
        .bind(hash)
        .bind(&datos.empresa_id)
        .bind(datos.participacion_porcentaje)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let id: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.roles_empresa
               (empresa_id, codigo, nombre, permisos, creado_por)
             VALUES ($1::uuid, 'vendedor', 'Vendedor', '{}'::jsonb, $2::uuid)
             ON CONFLICT (empresa_id, codigo) DO NOTHING",
        )
        .bind(&datos.empresa_id)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.asignaciones_personal
               (empresa_id, usuario_id, rol_empresa_id, participacion_porcentaje, estado, creado_por)
             SELECT $1::uuid, $2::uuid, r.id, $3, 'activa', $2::uuid
               FROM identity.roles_empresa r
              WHERE r.empresa_id = $1::uuid AND r.codigo = 'vendedor'",
        )
        .bind(&datos.empresa_id)
        .bind(id.to_string())
        .bind(datos.participacion_porcentaje)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        self.encontrar_por_id(&id.to_string())
            .await?
            .ok_or_else(ApiError::interno)
    }
}
