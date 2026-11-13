//! Integración de storage contra PostgreSQL real con un almacén en memoria.
//! Valida cuotas, outbox, metadata y servido público sin depender de MinIO.

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use storage_service::almacen::{AlmacenError, AlmacenObjetos, ObjetoEnAlmacen, ObjetoLeido};
use storage_service::config::Config;
use storage_service::db::Db;
use storage_service::outbox::Outbox;
use storage_service::storage::buckets::Buckets;
use storage_service::storage::objetos::{DatosSubida, FiltrosObjetos, Objetos};
use tokio::sync::Mutex;
use uuid::Uuid;

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

#[derive(Default)]
struct AlmacenFake {
    objetos: Mutex<HashMap<(String, String), Vec<u8>>>,
    content_type: Mutex<HashMap<(String, String), String>>,
}

#[async_trait]
impl AlmacenObjetos for AlmacenFake {
    async fn asegurar_bucket(&self, _bucket: &str) -> Result<(), AlmacenError> {
        Ok(())
    }
    async fn existe_bucket(&self, _bucket: &str) -> Result<bool, AlmacenError> {
        Ok(true)
    }
    async fn url_subida(
        &self,
        bucket: &str,
        objeto: &str,
        _content_type: &str,
        _expira: u64,
    ) -> Result<String, AlmacenError> {
        Ok(format!("http://minio/{bucket}/{objeto}?upload=1"))
    }
    async fn url_descarga(
        &self,
        bucket: &str,
        objeto: &str,
        _expira: u64,
    ) -> Result<String, AlmacenError> {
        Ok(format!("http://minio/{bucket}/{objeto}?download=1"))
    }
    async fn estadisticas(
        &self,
        bucket: &str,
        objeto: &str,
    ) -> Result<Option<ObjetoEnAlmacen>, AlmacenError> {
        let clave = (bucket.to_string(), objeto.to_string());
        let objetos = self.objetos.lock().await;
        let Some(bytes) = objetos.get(&clave) else {
            return Ok(None);
        };
        let content_type = self.content_type.lock().await.get(&clave).cloned();
        Ok(Some(ObjetoEnAlmacen {
            tamano_bytes: bytes.len() as i64,
            content_type,
            etag: Some("etag-fake".to_string()),
        }))
    }
    async fn leer(&self, bucket: &str, objeto: &str) -> Result<Option<ObjetoLeido>, AlmacenError> {
        self.leer_rango(bucket, objeto, 0, i64::MAX).await
    }
    async fn leer_rango(
        &self,
        bucket: &str,
        objeto: &str,
        inicio: i64,
        fin: i64,
    ) -> Result<Option<ObjetoLeido>, AlmacenError> {
        let clave = (bucket.to_string(), objeto.to_string());
        let objetos = self.objetos.lock().await;
        let Some(bytes) = objetos.get(&clave) else {
            return Ok(None);
        };
        let inicio = inicio.max(0) as usize;
        let fin = (fin as usize).min(bytes.len().saturating_sub(1));
        let trozo = if inicio <= fin {
            bytes[inicio..=fin].to_vec()
        } else {
            Vec::new()
        };
        let content_type = self.content_type.lock().await.get(&clave).cloned();
        Ok(Some(ObjetoLeido {
            cuerpo: ByteStream::from(trozo),
            tamano_bytes: bytes.len() as i64,
            content_type,
            etag: Some("etag-fake".to_string()),
        }))
    }
    async fn eliminar(&self, bucket: &str, objeto: &str) -> Result<(), AlmacenError> {
        self.objetos
            .lock()
            .await
            .remove(&(bucket.to_string(), objeto.to_string()));
        Ok(())
    }
}

async fn subir(almacen: &AlmacenFake, bucket: &str, objeto: &str, bytes: &[u8], ct: &str) {
    almacen
        .objetos
        .lock()
        .await
        .insert((bucket.to_string(), objeto.to_string()), bytes.to_vec());
    almacen
        .content_type
        .lock()
        .await
        .insert((bucket.to_string(), objeto.to_string()), ct.to_string());
}

#[tokio::test]
async fn debe_gestionar_buckets_objetos_cuotas_y_publicos() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let pool = PgPool::connect(&url).await.expect("postgres");
    sqlx::raw_sql(include_str!("../../../infra/db/init/09_storage.sql"))
        .execute(&pool)
        .await
        .expect("migracion storage");
    sqlx::raw_sql(include_str!(
        "../../../infra/db/init/09a_storage_publico.sql"
    ))
    .execute(&pool)
    .await
    .expect("migracion storage publico");
    pool.close().await;

    let config = Arc::new(Config::from_pairs([("SERVICIO", "storage-service")]).expect("config"));
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let almacen = Arc::new(AlmacenFake::default());
    let buckets = Arc::new(Buckets::nueva(
        Arc::clone(&db),
        almacen.clone(),
        Arc::clone(&config),
    ));
    let outbox = Arc::new(Outbox::nueva("storage.outbox"));
    let objetos = Objetos::nuevo(
        Arc::clone(&db),
        Arc::clone(&buckets),
        outbox,
        almacen.clone(),
        Arc::clone(&config),
    );
    let tenant = Uuid::new_v4().to_string();
    let tenant_uuid = Uuid::parse_str(&tenant).expect("uuid");

    let estado = buckets.asegurar(&tenant).await.expect("bucket");
    assert_eq!(estado.cuota_bytes, config.cuota_default_bytes);
    assert_eq!(estado.objetos, 0);

    let solicitud = objetos
        .solicitar_subida(
            &tenant,
            None,
            DatosSubida {
                nombre: "foto.jpg".into(),
                content_type: "image/jpeg".into(),
                tamano_bytes: 4,
                publico: true,
                ..Default::default()
            },
        )
        .await
        .expect("solicitud");
    assert!(solicitud.url_subida.contains("upload=1"));

    subir(
        &almacen,
        &solicitud.bucket,
        &solicitud.objeto,
        b"data",
        "image/jpeg",
    )
    .await;
    let activo = objetos
        .confirmar(&tenant, &solicitud.objeto_id)
        .await
        .expect("confirmar");
    assert_eq!(activo.estado, "activo");
    assert_eq!(activo.tamano_bytes, 4);

    let bucket = buckets.estado(&tenant).await.expect("estado");
    assert_eq!(bucket.usado_bytes, 4);
    assert_eq!(bucket.objetos, 1);

    let pagina = objetos
        .listar(
            &tenant,
            &FiltrosObjetos {
                q: Some("foto".into()),
                ..Default::default()
            },
        )
        .await
        .expect("listar");
    assert_eq!(pagina.total, 1);

    let descarga = objetos
        .url_descarga(&tenant, &solicitud.objeto_id)
        .await
        .expect("descarga");
    assert!(descarga.url_descarga.contains("download=1"));

    let meta = objetos
        .obtener_publico_meta(&solicitud.objeto_id)
        .await
        .expect("meta");
    assert_eq!(meta.content_type, "image/jpeg");
    let publico = objetos
        .leer_publico(&solicitud.objeto_id, Some((0, 1)))
        .await
        .expect("publico");
    assert_eq!(publico.cuerpo.tamano_bytes, 4);

    let recalibrado = buckets.recalcular_uso(&tenant).await.expect("recalcular");
    assert_eq!(recalibrado.usado_bytes, 4);

    let outbox_pendientes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM storage.outbox WHERE payload->'data'->>'tenant_id' = $1",
    )
    .bind(&tenant)
    .fetch_one(db.pool())
    .await
    .expect("outbox");
    assert_eq!(outbox_pendientes, 1);

    objetos
        .eliminar(&tenant, &solicitud.objeto_id, None)
        .await
        .expect("eliminar");
    let estado_final = buckets.estado(&tenant).await.expect("estado final");
    assert_eq!(estado_final.usado_bytes, 0);
    assert!(objetos
        .obtener_publico_meta(&solicitud.objeto_id)
        .await
        .is_err());

    assert!(buckets.actualizar_cuota(&tenant, 0).await.is_err());
    buckets
        .cambiar_estado(&tenant, "suspendido")
        .await
        .expect("suspender");
    let bloqueado = objetos
        .solicitar_subida(
            &tenant,
            None,
            DatosSubida {
                nombre: "otro.jpg".into(),
                content_type: "image/jpeg".into(),
                tamano_bytes: 1,
                ..Default::default()
            },
        )
        .await;
    assert!(bloqueado.is_err());

    let _ = sqlx::query("DELETE FROM storage.outbox WHERE payload->'data'->>'tenant_id' = $1")
        .bind(&tenant)
        .execute(db.pool())
        .await;
    let _ = sqlx::query("DELETE FROM storage.objetos WHERE tenant_id = $1")
        .bind(tenant_uuid)
        .execute(db.pool())
        .await;
    let _ = sqlx::query("DELETE FROM storage.buckets WHERE tenant_id = $1")
        .bind(tenant_uuid)
        .execute(db.pool())
        .await;
}
