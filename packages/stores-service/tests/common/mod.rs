#![allow(dead_code)]

//! Utilidades compartidas por las pruebas de integración.
//!
//! Provisionan su propio esquema `stores` (y el mínimo de `identity`) para
//! correr en CI contra una PostgreSQL efímera.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use stores_service::auth::{CapacidadActiva, Capacidades};
use stores_service::clients::{
    Catalogo, ConsultaObjeto, ItemExterno, ObjetoExterno, ObjetosStorage, TemaHerenciaCruda,
    TemasIdentity,
};
use stores_service::db::Db;

pub const EMPRESA: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
pub const USUARIO: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
pub const OTRO_USUARIO: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
pub const ITEM: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

pub fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

pub async fn provisionar(url: &str) {
    let pool = PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(
        r#"
        DROP SCHEMA IF EXISTS stores CASCADE;
        CREATE SCHEMA stores;

        CREATE TABLE stores.tiendas (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          vendedor_id uuid,
          nombre text NOT NULL,
          descripcion text NOT NULL DEFAULT '',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          empresa_id uuid,
          propietario_usuario_id uuid,
          participacion_porcentaje integer,
          slug text NOT NULL,
          logo_objeto_id uuid,
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          background_objeto_id uuid,
          background_media_type text,
          background_poster_objeto_id uuid,
          background_alt text NOT NULL DEFAULT '',
          background_overlay smallint NOT NULL DEFAULT 35,
          background_position text NOT NULL DEFAULT 'center',
          background_activo boolean NOT NULL DEFAULT false,
          plantilla text NOT NULL DEFAULT 'clasica',
          tema jsonb NOT NULL DEFAULT '{}'::jsonb,
          hero_titulo text NOT NULL DEFAULT '',
          hero_subtitulo text NOT NULL DEFAULT '',
          hero_cta_texto text NOT NULL DEFAULT '',
          hero_cta_url text NOT NULL DEFAULT '',
          seo_titulo text NOT NULL DEFAULT '',
          seo_descripcion text NOT NULL DEFAULT '',
          seo_imagen_objeto_id uuid,
          personalizacion_actualizada_en timestamptz,
          secciones jsonb NOT NULL DEFAULT '[]'::jsonb,
          sobre_nosotros text NOT NULL DEFAULT '',
          politicas jsonb NOT NULL DEFAULT '{}'::jsonb,
          redes jsonb NOT NULL DEFAULT '[]'::jsonb,
          anuncio_texto text NOT NULL DEFAULT '',
          anuncio_enlace text NOT NULL DEFAULT '',
          anuncio_activo boolean NOT NULL DEFAULT false,
          contenido_actualizado_en timestamptz,
          background_movil_objeto_id uuid,
          background_movil_media_type text,
          background_movil_poster_objeto_id uuid,
          background_movil_alt text NOT NULL DEFAULT '',
          background_movil_activo boolean NOT NULL DEFAULT false,
          plantilla_override text,
          tema_override jsonb NOT NULL DEFAULT '{}'::jsonb,
          CONSTRAINT tiendas_slug_unico UNIQUE (slug),
          CONSTRAINT tiendas_vendedor_unico UNIQUE (vendedor_id)
        );

        CREATE TABLE stores.tienda_slugs (
          slug text PRIMARY KEY,
          tienda_id uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          reemplazado_en timestamptz
        );
        CREATE UNIQUE INDEX idx_tienda_slugs_actual
          ON stores.tienda_slugs (tienda_id) WHERE reemplazado_en IS NULL;

        CREATE FUNCTION stores.fn_tienda_slug_historial()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF TG_OP = 'INSERT' THEN
            INSERT INTO stores.tienda_slugs (tienda_id, slug) VALUES (NEW.id, NEW.slug);
          ELSIF NEW.slug IS DISTINCT FROM OLD.slug THEN
            UPDATE stores.tienda_slugs SET reemplazado_en = NOW()
             WHERE tienda_id = NEW.id AND reemplazado_en IS NULL;
            INSERT INTO stores.tienda_slugs (tienda_id, slug) VALUES (NEW.id, NEW.slug);
          END IF;
          RETURN NEW;
        END;
        $$;
        CREATE TRIGGER trg_tienda_slug_historial
          AFTER INSERT OR UPDATE OF slug ON stores.tiendas
          FOR EACH ROW EXECUTE FUNCTION stores.fn_tienda_slug_historial();

        CREATE TABLE stores.ofertas (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          tienda_id uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
          sku text NOT NULL,
          producto_nombre text NOT NULL,
          margen integer NOT NULL,
          precio_base_cents integer NOT NULL,
          precio_venta_cents integer NOT NULL,
          stock integer DEFAULT 0,
          estado text NOT NULL DEFAULT 'activa',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          atributos jsonb NOT NULL DEFAULT '{}'::jsonb,
          imagen_objeto_id uuid,
          item_type text NOT NULL DEFAULT 'producto',
          item_id uuid NOT NULL,
          moneda text NOT NULL DEFAULT 'NIO',
          CONSTRAINT ofertas_tienda_item UNIQUE (tienda_id, item_type, item_id)
        );

        CREATE TABLE stores.historico_precios (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          oferta_id uuid NOT NULL REFERENCES stores.ofertas(id) ON DELETE CASCADE,
          precio_anterior_cents integer,
          precio_nuevo_cents integer NOT NULL,
          vigente_desde timestamptz NOT NULL DEFAULT NOW()
        );

        CREATE TABLE stores.eventos_procesados (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          procesado_en timestamptz NOT NULL DEFAULT NOW()
        );

        DROP SCHEMA IF EXISTS identity CASCADE;
        CREATE SCHEMA identity;
        CREATE TABLE identity.asignaciones_personal (
          usuario_id uuid NOT NULL,
          tienda_id uuid NOT NULL,
          estado text NOT NULL DEFAULT 'activa'
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("esquema stores");
    pool.close().await;
}

pub async fn db() -> Arc<Db> {
    Arc::new(
        Db::conectar(&url().expect("TEST_DATABASE_URL"))
            .await
            .expect("db"),
    )
}

#[derive(Default)]
pub struct TemasFijos {
    pub empresa: TemaHerenciaCruda,
    pub plataforma: TemaHerenciaCruda,
}

#[async_trait]
impl TemasIdentity for TemasFijos {
    async fn obtener_empresa(&self, _empresa_id: &str) -> TemaHerenciaCruda {
        self.empresa.clone()
    }
    async fn obtener_plataforma(&self) -> TemaHerenciaCruda {
        self.plataforma.clone()
    }
}

pub fn temas(
    empresa_plantilla: Option<&str>,
    empresa_tema: Value,
    plataforma_plantilla: Option<&str>,
    plataforma_tema: Value,
) -> Arc<dyn TemasIdentity> {
    Arc::new(TemasFijos {
        empresa: TemaHerenciaCruda {
            plantilla_override: empresa_plantilla
                .map(|valor| json!(valor))
                .unwrap_or(Value::Null),
            tema_override: empresa_tema,
        },
        plataforma: TemaHerenciaCruda {
            plantilla_override: plataforma_plantilla
                .map(|valor| json!(valor))
                .unwrap_or(Value::Null),
            tema_override: plataforma_tema,
        },
    })
}

pub struct StorageFijo(pub HashMap<String, ObjetoExterno>);

#[async_trait]
impl ObjetosStorage for StorageFijo {
    async fn objeto(&self, _tenant_id: &str, objeto_id: &str) -> ConsultaObjeto {
        match self.0.get(objeto_id) {
            Some(objeto) => ConsultaObjeto::Encontrado(objeto.clone()),
            None => ConsultaObjeto::NoEncontrado,
        }
    }
}

pub fn storage_vacio() -> Arc<dyn ObjetosStorage> {
    Arc::new(StorageFijo(HashMap::new()))
}

pub fn objeto(tenant: &str, content_type: &str) -> ObjetoExterno {
    ObjetoExterno {
        id: uuid_objeto(),
        tenant_id: tenant.to_string(),
        estado: "activo".to_string(),
        publico: true,
        content_type: content_type.to_string(),
    }
}

pub fn uuid_objeto() -> String {
    "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee".to_string()
}

pub struct CatalogoFijo(pub Option<ItemExterno>);

#[async_trait]
impl Catalogo for CatalogoFijo {
    async fn item(&self, _item_id: &str) -> Option<ItemExterno> {
        self.0.clone()
    }
}

pub fn catalogo(precio: &str, stock: Option<i64>, track: bool) -> Arc<dyn Catalogo> {
    Arc::new(CatalogoFijo(Some(ItemExterno {
        item_id: ITEM.to_string(),
        empresa_id: EMPRESA.to_string(),
        tipo: "producto".to_string(),
        codigo: "SKU-1".to_string(),
        nombre: "Producto QA".to_string(),
        precio: precio.to_string(),
        stock,
        track_inventory: track,
        duration_minutes: None,
        atributos: json!({"talla": "M"}),
        estado: "activo".to_string(),
    })))
}

pub fn catalogo_ausente() -> Arc<dyn Catalogo> {
    Arc::new(CatalogoFijo(None))
}

pub struct CapacidadesFijas(pub Vec<CapacidadActiva>);

#[async_trait]
impl Capacidades for CapacidadesFijas {
    async fn activas(
        &self,
        _empresa_id: &str,
        _requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        Ok(self.0.clone())
    }
}

pub fn capacidades_ok() -> Arc<dyn Capacidades> {
    Arc::new(CapacidadesFijas(vec![CapacidadActiva {
        empresa_id: EMPRESA.to_string(),
        capacidad_codigo: "catalogo_items".to_string(),
        activo: true,
    }]))
}
