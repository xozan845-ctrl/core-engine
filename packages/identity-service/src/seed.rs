//! Seed del administrador inicial y del tenant de flota de plataforma.
//!
//! Réplica de `seed/seed.service.ts`. Es best-effort: un fallo se registra y no
//! impide arrancar el servicio.

use crate::campos;
use crate::config::Config;
use crate::db::Db;
use crate::dominio::usuarios::{CrearUsuario, Usuarios};
use crate::logs;

const TENANT_PLATAFORMA_DEFAULT: &str = "00000000-0000-4000-8000-000000000001";

pub async fn sembrar(usuarios: &Usuarios, db: &Db, config: &Config) {
    sembrar_admin(usuarios, config).await;
    sembrar_tenant_plataforma(db, config).await;
}

async fn sembrar_admin(usuarios: &Usuarios, config: &Config) {
    match usuarios.existe_admin().await {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            logs::warn(
                "seed",
                campos! { "msg": "No se pudo comprobar el administrador", "err": format!("{:?}", error) },
            );
            return;
        }
    }
    let resultado = usuarios
        .crear(CrearUsuario {
            nombre: "Administrador Core Engine".to_string(),
            correo: config.admin_email.clone(),
            contrasena: config.admin_password.clone(),
            rol: "super_admin".to_string(),
            tenant_id: None,
            personal_id: None,
        })
        .await;
    match resultado {
        Ok(admin) => logs::info(
            "seed",
            campos! {
                "msg": "Administrador inicial creado",
                "email": admin.correo,
                "consejo": "cambie la contrasena en produccion",
            },
        ),
        Err(error) => logs::error(
            "seed",
            campos! { "msg": "No se pudo crear el administrador inicial", "err": error.mensaje },
        ),
    }
}

async fn sembrar_tenant_plataforma(db: &Db, config: &Config) {
    let id = config
        .platform_logistics_tenant_id
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or(TENANT_PLATAFORMA_DEFAULT);
    let valido = uuid::Uuid::parse_str(id)
        .map(|uuid| uuid.get_version_num() == 4)
        .unwrap_or(false);
    if !valido {
        logs::warn(
            "seed",
            campos! { "msg": "PLATFORM_LOGISTICS_TENANT_ID no es un UUID v4; se omite", "id": id },
        );
        return;
    }
    let mut tx = match db.transaccion().await {
        Ok(tx) => tx,
        Err(error) => {
            logs::warn(
                "seed",
                campos! { "msg": "No se pudo abrir la transaccion del tenant", "err": error.to_string() },
            );
            return;
        }
    };
    let admin = match sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM identity.usuarios WHERE rol = 'super_admin' ORDER BY creado_en ASC LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(Some(admin)) => admin,
        Ok(None) => return,
        Err(error) => {
            logs::warn(
                "seed",
                campos! { "msg": "No se pudo leer el administrador del tenant", "err": error.to_string() },
            );
            return;
        }
    };
    let insertado = sqlx::query(
        "INSERT INTO identity.empresas
           (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por)
         VALUES ($1::uuid, 'Plataforma logística', 'plataforma-logistica', 'flota', 'pack_flota', 1, $2::uuid)
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(admin.to_string())
    .execute(&mut *tx)
    .await;
    if let Err(error) = insertado {
        logs::warn(
            "seed",
            campos! { "msg": "No se pudo asegurar el tenant de plataforma", "err": error.to_string() },
        );
        return;
    }
    let capacidades = sqlx::query(
        "INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
         SELECT $1::uuid, capacidad_codigo, config, activo
           FROM identity.pack_capacidades
          WHERE pack_codigo = 'pack_flota' AND pack_version = 1
         ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING",
    )
    .bind(id)
    .execute(&mut *tx)
    .await;
    if let Err(error) = capacidades {
        logs::warn(
            "seed",
            campos! { "msg": "No se pudieron sembrar las capacidades del tenant", "err": error.to_string() },
        );
        return;
    }
    if let Err(error) = tx.commit().await {
        logs::warn(
            "seed",
            campos! { "msg": "No se pudo confirmar el tenant de plataforma", "err": error.to_string() },
        );
    }
}

/// Helper de prueba: el tenant por defecto debe ser UUID v4.
pub fn tenant_por_defecto() -> &'static str {
    TENANT_PLATAFORMA_DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_tenant_por_defecto_es_uuid_v4() {
        let uuid = uuid::Uuid::parse_str(tenant_por_defecto()).expect("uuid");
        assert_eq!(uuid.get_version_num(), 4);
    }
}
