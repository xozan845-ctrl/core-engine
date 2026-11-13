//! Clientes de los servicios internos que `stores-service` necesita.
//!
//! - `identity-service`: herencia visual (empresa y plataforma) con caché TTL y
//!   degradación a defaults si no responde (R-AR-7/R-RB-4).
//! - `storage-service`: metadata de un objeto para validar logo, fondos,
//!   pósteres e imagen SEO.
//! - `catalog-service`: item unificado antes de publicar una oferta.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Respuesta cruda de los endpoints de tema de identity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemaHerenciaCruda {
    pub plantilla_override: Value,
    pub tema_override: Value,
}

impl TemaHerenciaCruda {
    fn desde_json(valor: Value) -> Self {
        let mut mapa = match valor {
            Value::Object(mapa) => mapa,
            _ => Map::new(),
        };
        Self {
            plantilla_override: mapa.remove("plantilla_override").unwrap_or(Value::Null),
            tema_override: mapa.remove("tema_override").unwrap_or(Value::Null),
        }
    }
}

#[async_trait]
pub trait TemasIdentity: Send + Sync {
    /// Nunca falla: identity caído degrada a "sin herencia".
    async fn obtener_empresa(&self, empresa_id: &str) -> TemaHerenciaCruda;
    async fn obtener_plataforma(&self) -> TemaHerenciaCruda;
}

pub struct TemasIdentityHttp {
    url: String,
    clave: String,
    ttl: Duration,
    cache: Mutex<HashMap<String, (Instant, TemaHerenciaCruda)>>,
}

impl TemasIdentityHttp {
    pub fn nueva(url: String, clave: Option<String>, ttl_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            ttl: Duration::from_millis(ttl_ms.max(1)),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn obtener(&self, clave: &str, url: String) -> TemaHerenciaCruda {
        let ahora = Instant::now();
        if let Ok(cache) = self.cache.lock() {
            if let Some((expira, valor)) = cache.get(clave) {
                if *expira > ahora {
                    return valor.clone();
                }
            }
        }

        let mut valor = TemaHerenciaCruda::default();
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            .build();
        if let Ok(cliente) = cliente {
            if let Ok(respuesta) = cliente
                .get(&url)
                .header("x-internal-key", &self.clave)
                .send()
                .await
            {
                if respuesta.status().is_success() {
                    if let Ok(datos) = respuesta.json::<Value>().await {
                        valor = TemaHerenciaCruda::desde_json(datos);
                    }
                }
            }
        }

        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() > 500 {
                cache.clear();
            }
            cache.insert(clave.to_string(), (ahora + self.ttl, valor.clone()));
        }
        valor
    }
}

#[async_trait]
impl TemasIdentity for TemasIdentityHttp {
    async fn obtener_empresa(&self, empresa_id: &str) -> TemaHerenciaCruda {
        self.obtener(
            &format!("empresa:{empresa_id}"),
            format!(
                "{}/internal/empresas/{}/tema",
                self.url,
                urlencoding::encode(empresa_id)
            ),
        )
        .await
    }

    async fn obtener_plataforma(&self) -> TemaHerenciaCruda {
        self.obtener(
            "plataforma",
            format!("{}/internal/configuracion-sitio/tema", self.url),
        )
        .await
    }
}

/// Implementación para tests y estado sin infraestructura.
pub struct TemasSinHerencia;

#[async_trait]
impl TemasIdentity for TemasSinHerencia {
    async fn obtener_empresa(&self, _empresa_id: &str) -> TemaHerenciaCruda {
        TemaHerenciaCruda::default()
    }
    async fn obtener_plataforma(&self) -> TemaHerenciaCruda {
        TemaHerenciaCruda::default()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ObjetoExterno {
    pub id: String,
    pub tenant_id: String,
    pub estado: String,
    pub publico: bool,
    pub content_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsultaObjeto {
    Encontrado(ObjetoExterno),
    NoEncontrado,
    NoDisponible,
}

#[async_trait]
pub trait ObjetosStorage: Send + Sync {
    async fn objeto(&self, tenant_id: &str, objeto_id: &str) -> ConsultaObjeto;
}

pub struct ObjetosStorageHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl ObjetosStorageHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl ObjetosStorage for ObjetosStorageHttp {
    async fn objeto(&self, tenant_id: &str, objeto_id: &str) -> ConsultaObjeto {
        let cliente = match reqwest::Client::builder().timeout(self.timeout).build() {
            Ok(cliente) => cliente,
            Err(_) => return ConsultaObjeto::NoDisponible,
        };
        let url = format!(
            "{}/internal/storage/objetos/{}?tenant_id={}",
            self.url,
            urlencoding::encode(objeto_id),
            urlencoding::encode(tenant_id)
        );
        let respuesta = match cliente
            .get(url)
            .header("x-internal-key", &self.clave)
            .send()
            .await
        {
            Ok(respuesta) => respuesta,
            Err(_) => return ConsultaObjeto::NoDisponible,
        };
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return ConsultaObjeto::NoEncontrado;
        }
        if !respuesta.status().is_success() {
            return ConsultaObjeto::NoDisponible;
        }
        match respuesta.json::<ObjetoExterno>().await {
            Ok(objeto) => ConsultaObjeto::Encontrado(objeto),
            Err(_) => ConsultaObjeto::NoDisponible,
        }
    }
}

/// Implementación para tests sin storage.
pub struct StorageSinConexion;

#[async_trait]
impl ObjetosStorage for StorageSinConexion {
    async fn objeto(&self, _tenant_id: &str, _objeto_id: &str) -> ConsultaObjeto {
        ConsultaObjeto::NoDisponible
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ItemExterno {
    pub item_id: String,
    pub empresa_id: String,
    pub tipo: String,
    pub codigo: String,
    pub nombre: String,
    pub precio: String,
    pub stock: Option<i64>,
    pub track_inventory: bool,
    #[serde(default)]
    pub duration_minutes: Option<i64>,
    #[serde(default)]
    pub atributos: Value,
    pub estado: String,
}

#[async_trait]
pub trait Catalogo: Send + Sync {
    /// `None` ante cualquier fallo, igual que el `fetch` original.
    async fn item(&self, item_id: &str) -> Option<ItemExterno>;
}

pub struct CatalogoHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl CatalogoHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl Catalogo for CatalogoHttp {
    async fn item(&self, item_id: &str) -> Option<ItemExterno> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .ok()?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/items/{}",
                self.url,
                urlencoding::encode(item_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<ItemExterno>().await.ok()
    }
}

/// Implementación para tests sin catálogo.
pub struct CatalogoSinConexion;

#[async_trait]
impl Catalogo for CatalogoSinConexion {
    async fn item(&self, _item_id: &str) -> Option<ItemExterno> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_leer_los_overrides_crudos_de_identity() {
        let crudo = TemaHerenciaCruda::desde_json(json!({
            "plantilla_override": "minimal",
            "tema_override": {"color_primario": "#111111"},
            "extra": 1
        }));
        assert_eq!(crudo.plantilla_override, json!("minimal"));
        assert_eq!(crudo.tema_override, json!({"color_primario": "#111111"}));
    }

    #[test]
    fn debe_degradar_a_vacio_cuando_identity_no_devuelve_objeto() {
        let crudo = TemaHerenciaCruda::desde_json(json!(null));
        assert_eq!(crudo, TemaHerenciaCruda::default());
    }

    #[tokio::test]
    async fn debe_degradar_a_sin_herencia_sin_servicios() {
        let temas = TemasSinHerencia;
        assert_eq!(
            temas.obtener_empresa("t-1").await,
            TemaHerenciaCruda::default()
        );
        assert_eq!(
            temas.obtener_plataforma().await,
            TemaHerenciaCruda::default()
        );
    }
}
