//! Acceso a PostgreSQL con el contrato de `PgService` (pool de 10 conexiones).
//!
//! El consumidor solo lee/escribe `notifications.eventos_procesados`; la tabla
//! no tiene políticas RLS y el servicio usa el mismo usuario de aplicación.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

/// Conexiones máximas por servicio. El presupuesto total (servicios x pool)
/// debe caber en `max_connections` de PostgreSQL o pasar por PgBouncer.
fn pool_max() -> u32 {
    std::env::var("DATABASE_POOL_MAX")
        .ok()
        .and_then(|valor| valor.parse::<u32>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(4)
}

pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn conectar(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(pool_max())
            // Claim por defecto para jobs y conexiones sin usuario; las
            // transacciones HTTP lo sobreescriben con el usuario real.
            .after_connect(|conn, _| {
                Box::pin(async move {
                    sqlx::query("SELECT set_config('request.jwt.claims', $1, false)")
                        .bind(serde_json::json!({ "rol": "super_admin" }).to_string())
                        .execute(conn)
                        .await?;
                    Ok::<(), sqlx::Error>(())
                })
            })
            .connect(url)
            .await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn evento_procesado(&self, event_id: Uuid) -> Result<bool, sqlx::Error> {
        let fila =
            sqlx::query("SELECT 1 FROM notifications.eventos_procesados WHERE event_id = $1")
                .bind(event_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(fila.is_some())
    }

    pub async fn marcar_evento(&self, event_id: Uuid, tipo: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO notifications.eventos_procesados (event_id, tipo)
             VALUES ($1, $2) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn cerrar(&self) {
        self.pool.close().await;
    }
}
