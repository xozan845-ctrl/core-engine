//! Ofertas: relación vendedor-item con precio final (Tabla 11).
//!
//! RN-01: precio venta = base × (1 + margen); RN-02: no se publica con stock
//! menor a 1. El catálogo maestro se consulta a `catalog-service` (REST).

use crate::clients::Catalogo;
use crate::db::{fila_a_json, Db};
use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub mod publico;
pub use publico::{a_oferta_publica, OfertaPublica};

pub const MARGEN_MINIMO: i64 = 0;
pub const MARGEN_MAXIMO: i64 = 90;
const URL_IMAGEN_PUBLICA: &str = "/api/v2/storage/public/objetos";

const COLUMNAS_OFERTA: &str = "o.id, o.tienda_id, t.vendedor_id, t.empresa_id,
  t.participacion_porcentaje, o.item_type, o.item_id, o.sku,
  o.producto_nombre, o.atributos, o.imagen_objeto_id, o.margen,
  o.precio_base_cents, o.precio_venta_cents, o.stock, o.estado, o.moneda,
  o.creado_en";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    pub centavos: i64,
}

impl Money {
    pub fn desde_centavos(centavos: i64) -> Result<Self, ApiError> {
        if centavos < 0 {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            ));
        }
        Ok(Self { centavos })
    }

    /// Convierte un monto decimal ("1150.00", 1150 o "1150,50") a centavos.
    pub fn parsear(valor: &str) -> Result<Self, ApiError> {
        let texto = valor
            .trim()
            .replace(',', ".")
            .replace(char::is_whitespace, "");
        let partes: Vec<&str> = texto.split('.').collect();
        let valido = (partes.len() == 1 || partes.len() == 2)
            && !partes[0].is_empty()
            && partes[0].chars().all(|caracter| caracter.is_ascii_digit())
            && partes
                .get(1)
                .map(|decimales| {
                    (1..=2).contains(&decimales.len())
                        && decimales.chars().all(|caracter| caracter.is_ascii_digit())
                })
                .unwrap_or(true);
        if !valido {
            return Err(ApiError::dominio(
                "MONTO_INVALIDO",
                format!(
                    "Formato de monto invalido: \"{valor}\". Use numero con hasta 2 decimales."
                ),
            ));
        }
        let enteros: i64 = partes[0].parse().map_err(|_| {
            ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            )
        })?;
        let decimales = partes
            .get(1)
            .map(|valor| format!("{valor:0<2}"))
            .unwrap_or_else(|| "00".to_string());
        let decimales: i64 = decimales.parse().map_err(|_| {
            ApiError::dominio(
                "MONTO_INVALIDO",
                "El monto debe ser un entero no negativo en centavos.",
            )
        })?;
        Self::desde_centavos(enteros.saturating_mul(100).saturating_add(decimales))
    }

    /// RN-01: precio final = base × (1 + margen).
    pub fn aplicar_margen(&self, margen: i64) -> Result<Self, ApiError> {
        if !(MARGEN_MINIMO..=MARGEN_MAXIMO).contains(&margen) {
            return Err(ApiError::dominio(
                "MARGEN_INVALIDO",
                "El margen debe estar entre 0 y 90 % (RN-01).",
            ));
        }
        let centavos =
            (self.centavos as f64 + (self.centavos as f64 * margen as f64) / 100.0).round() as i64;
        Self::desde_centavos(centavos)
    }

    /// Serialización "1150.00".
    pub fn string(&self) -> String {
        format!("{}.{:02}", self.centavos / 100, self.centavos % 100)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paginacion {
    pub pagina: i64,
    pub limite: i64,
}

fn parsear_entero(valor: &str) -> Option<i64> {
    let texto = valor.trim();
    let (signo, resto) = match texto.strip_prefix('-') {
        Some(resto) => (-1i64, resto),
        None => (1i64, texto.strip_prefix('+').unwrap_or(texto)),
    };
    let digitos: String = resto
        .chars()
        .take_while(|caracter| caracter.is_ascii_digit())
        .collect();
    if digitos.is_empty() {
        return None;
    }
    digitos.parse::<i64>().ok().map(|numero| numero * signo)
}

/// Réplica de `parsearPaginacion` (incluido `parseInt(x) || default`).
pub fn parsear_paginacion(pagina: Option<&str>, limite: Option<&str>) -> Paginacion {
    let pagina = pagina
        .and_then(parsear_entero)
        .filter(|valor| *valor != 0)
        .unwrap_or(1)
        .max(1);
    let limite = limite
        .and_then(parsear_entero)
        .filter(|valor| *valor != 0)
        .unwrap_or(20)
        .clamp(1, 100);
    Paginacion { pagina, limite }
}

pub fn crear_pagina(items: Vec<Value>, total: i64, paginacion: Paginacion) -> Value {
    let paginas = ((total as f64) / (paginacion.limite as f64)).ceil() as i64;
    json!({
        "items": items,
        "total": total,
        "pagina": paginacion.pagina,
        "limite": paginacion.limite,
        "paginas": paginas.max(1),
    })
}

fn texto(fila: &Value, clave: &str) -> String {
    fila.get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_opcional(fila: &Value, clave: &str) -> Option<String> {
    fila.get(clave).and_then(Value::as_str).map(str::to_string)
}

fn entero(fila: &Value, clave: &str) -> i64 {
    fila.get(clave).and_then(Value::as_i64).unwrap_or(0)
}

/// Serializa una fila con el orden del contrato `Oferta`.
pub fn serializar(fila: &Value) -> Result<Value, ApiError> {
    let mut objeto = Map::new();
    objeto.insert("id".into(), Value::String(texto(fila, "id")));
    objeto.insert("tienda_id".into(), Value::String(texto(fila, "tienda_id")));
    if fila.get("vendedor_id").is_some() {
        objeto.insert(
            "vendedor_id".into(),
            fila.get("vendedor_id").cloned().unwrap_or(Value::Null),
        );
    }
    if fila.get("empresa_id").is_some() {
        objeto.insert(
            "empresa_id".into(),
            fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        );
    }
    if fila.get("participacion_porcentaje").is_some() {
        objeto.insert(
            "participacion_porcentaje".into(),
            fila.get("participacion_porcentaje")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    objeto.insert("item_type".into(), Value::String(texto(fila, "item_type")));
    objeto.insert("item_id".into(), Value::String(texto(fila, "item_id")));
    objeto.insert("sku".into(), Value::String(texto(fila, "sku")));
    objeto.insert(
        "producto_nombre".into(),
        Value::String(texto(fila, "producto_nombre")),
    );
    objeto.insert(
        "atributos".into(),
        fila.get("atributos").cloned().unwrap_or_else(|| json!({})),
    );
    let imagen = texto_opcional(fila, "imagen_objeto_id");
    objeto.insert(
        "imagen_objeto_id".into(),
        imagen.clone().map(Value::String).unwrap_or(Value::Null),
    );
    objeto.insert(
        "imagen_url".into(),
        imagen
            .map(|id| Value::String(format!("{URL_IMAGEN_PUBLICA}/{id}")))
            .unwrap_or(Value::Null),
    );
    objeto.insert("margen".into(), Value::from(entero(fila, "margen")));
    objeto.insert(
        "precio_base".into(),
        Value::String(Money::desde_centavos(entero(fila, "precio_base_cents"))?.string()),
    );
    objeto.insert(
        "precio_venta".into(),
        Value::String(Money::desde_centavos(entero(fila, "precio_venta_cents"))?.string()),
    );
    objeto.insert(
        "stock".into(),
        fila.get("stock").cloned().unwrap_or(Value::Null),
    );
    objeto.insert("estado".into(), Value::String(texto(fila, "estado")));
    objeto.insert(
        "moneda".into(),
        Value::String(texto_opcional(fila, "moneda").unwrap_or_else(|| "NIO".to_string())),
    );
    objeto.insert("creado_en".into(), Value::String(texto(fila, "creado_en")));
    Ok(Value::Object(objeto))
}

pub struct Ofertas {
    db: Arc<Db>,
    catalogo: Arc<dyn Catalogo>,
}

impl Ofertas {
    pub fn nueva(db: Arc<Db>, catalogo: Arc<dyn Catalogo>) -> Self {
        Self { db, catalogo }
    }

    pub async fn encontrar_por_id(&self, id: &str) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_OFERTA}
               FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE o.id = $1::uuid"
        ))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        fila.map(|fila| {
            let valor = fila_a_json(&fila).map_err(crate::db::interno)?;
            serializar(&valor)
        })
        .transpose()
    }

    pub async fn por_ids(&self, ids: &[String]) -> Result<Vec<Value>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids
            .iter()
            .map(|id| Uuid::parse_str(id.trim()).map_err(crate::db::interno))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_OFERTA}
               FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE o.id = ANY($1::uuid[])"
        ))
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(crate::db::interno)?;
                serializar(&valor)
            })
            .collect()
    }

    pub async fn de_vendedor(
        &self,
        vendedor_id: &str,
        pagina: Option<&str>,
        limite: Option<&str>,
    ) -> Result<Value, ApiError> {
        let paginacion = parsear_paginacion(pagina, limite);
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let total: i32 = sqlx::query(
            "SELECT COUNT(*)::int AS n FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE t.vendedor_id = $1::uuid OR EXISTS (
                SELECT 1 FROM identity.asignaciones_personal ap
                WHERE ap.usuario_id = $1::uuid AND ap.tienda_id = t.id AND ap.estado = 'activa'
              )",
        )
        .bind(vendedor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db::interno)?
        .get("n");
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_OFERTA}
               FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE t.vendedor_id = $1::uuid OR EXISTS (
                SELECT 1 FROM identity.asignaciones_personal ap
                WHERE ap.usuario_id = $1::uuid AND ap.tienda_id = t.id AND ap.estado = 'activa'
              )
              ORDER BY o.creado_en DESC LIMIT $2 OFFSET $3"
        ))
        .bind(vendedor_id)
        .bind(paginacion.limite)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let items = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(crate::db::interno)?;
                serializar(&valor)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn de_tienda(&self, tienda_id: &str) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_OFERTA}
               FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE o.tienda_id = $1::uuid AND o.estado = 'activa'"
        ))
        .bind(tienda_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(crate::db::interno)?;
                serializar(&valor)
            })
            .collect()
    }

    /// Publica cualquier item del catálogo unificado.
    pub async fn publicar_item(
        &self,
        vendedor_id: &str,
        tienda_id: &str,
        item_id: &str,
        margen: i64,
        moneda_valor: &str,
    ) -> Result<Value, ApiError> {
        let moneda = moneda_valor.trim().to_uppercase();
        if moneda.len() != 3 || !moneda.chars().all(|caracter| caracter.is_ascii_uppercase()) {
            return Err(ApiError::dominio(
                "MONEDA_INVALIDA",
                "La moneda debe ser un codigo ISO de 3 letras.",
            ));
        }
        if !(MARGEN_MINIMO..=MARGEN_MAXIMO).contains(&margen) {
            return Err(ApiError::dominio(
                "MARGEN_INVALIDO",
                format!(
                    "El margen debe estar entre {MARGEN_MINIMO} y {MARGEN_MAXIMO} por ciento (RN-01)."
                ),
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let tienda = sqlx::query(
            "SELECT id, empresa_id FROM stores.tiendas
              WHERE id = $1::uuid AND (propietario_usuario_id = $2::uuid OR EXISTS (
                SELECT 1 FROM identity.asignaciones_personal ap
                WHERE ap.usuario_id = $2::uuid AND ap.tienda_id = stores.tiendas.id AND ap.estado = 'activa'
              ))",
        )
        .bind(tienda_id)
        .bind(vendedor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let Some(tienda) = tienda else {
            return Err(ApiError::dominio(
                "TIENDA_REQUERIDA",
                "Crea primero tu tienda de vendedor.",
            ));
        };
        let tienda_id: Uuid = tienda.get("id");
        let empresa_id: Uuid = tienda.get("empresa_id");
        let tienda_id = tienda_id.to_string();
        let empresa_id = empresa_id.to_string();

        let Some(item) = self.catalogo.item(item_id).await else {
            return Err(ApiError::no_encontrado("Item", item_id));
        };
        if item.empresa_id != empresa_id {
            return Err(ApiError::dominio(
                "ITEM_OTRA_EMPRESA",
                "La tienda solo puede publicar items de su empresa.",
            ));
        }
        if item.estado != "activo" {
            return Err(ApiError::conflicto(
                "No se puede publicar un item inactivo.",
            ));
        }
        if item.track_inventory && item.stock.unwrap_or(0) < 1 {
            return Err(ApiError::conflicto(
                "No se puede publicar un item sin stock disponible.",
            ));
        }

        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let existente = sqlx::query(
            "SELECT id FROM stores.ofertas WHERE tienda_id = $1::uuid AND item_type = $2 AND item_id = $3::uuid",
        )
        .bind(&tienda_id)
        .bind(&item.tipo)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        if existente.is_some() {
            let _ = tx.rollback().await;
            return Err(ApiError::conflicto(
                "Este item ya esta publicado en tu tienda.",
            ));
        }

        let precio_base = Money::parsear(&item.precio)?;
        let precio_venta = precio_base.aplicar_margen(margen)?;
        let id = Uuid::new_v4();
        let atributos = if item.atributos.is_null() {
            json!({})
        } else {
            item.atributos.clone()
        };
        let fila = sqlx::query(
            "INSERT INTO stores.ofertas
               (id, tienda_id, item_type, item_id, sku,
                producto_nombre, atributos, imagen_objeto_id, margen, precio_base_cents,
                precio_venta_cents, stock, estado, moneda)
             VALUES ($1, $2::uuid, $3, $4::uuid, $5, $6, $7::jsonb, NULL, $8::int, $9::int, $10::int, $11::int, 'activa', $12)
             RETURNING id",
        )
        .bind(id)
        .bind(&tienda_id)
        .bind(&item.tipo)
        .bind(item_id)
        .bind(&item.codigo)
        .bind(&item.nombre)
        .bind(atributos.to_string())
        .bind(margen)
        .bind(precio_base.centavos)
        .bind(precio_venta.centavos)
        .bind(if item.track_inventory {
            item.stock
        } else {
            None
        })
        .bind(&moneda)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            let _ = tx.rollback().await;
            return Err(ApiError::dominio(
                "OFERTA_FALLIDA",
                "No se pudo publicar el item.",
            ));
        };
        let oferta_id: Uuid = fila.get("id");
        sqlx::query(
            "INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
             VALUES ($1::uuid, NULL, $2::int, NOW())",
        )
        .bind(oferta_id)
        .bind(precio_venta.centavos)
        .execute(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;

        self.encontrar_por_id(&oferta_id.to_string())
            .await?
            .ok_or_else(|| ApiError::dominio("OFERTA_FALLIDA", "No se pudo leer la oferta creada."))
    }

    /// Cambio de margen del vendedor: afecta solo a compras nuevas.
    pub async fn cambiar_margen(
        &self,
        oferta_id: &str,
        vendedor_id: &str,
        margen: i64,
    ) -> Result<Value, ApiError> {
        if !(MARGEN_MINIMO..=MARGEN_MAXIMO).contains(&margen) {
            return Err(ApiError::dominio(
                "MARGEN_INVALIDO",
                format!("El margen debe estar entre {MARGEN_MINIMO} y {MARGEN_MAXIMO} % (RN-01)."),
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(
            "SELECT o.*, t.vendedor_id FROM stores.ofertas o
               JOIN stores.tiendas t ON t.id = o.tienda_id
              WHERE o.id = $1::uuid AND (t.vendedor_id = $2::uuid OR EXISTS (
                SELECT 1 FROM identity.asignaciones_personal ap
                WHERE ap.usuario_id = $2::uuid AND ap.tienda_id = t.id AND ap.estado = 'activa'
              ))",
        )
        .bind(oferta_id)
        .bind(vendedor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        let Some(fila) = fila else {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Oferta", oferta_id));
        };
        let oferta = fila_a_json(&fila).map_err(crate::db::interno)?;
        let base = Money::desde_centavos(entero(&oferta, "precio_base_cents"))?;
        let nuevo = base.aplicar_margen(margen)?;
        let actualizado = sqlx::query(
            "UPDATE stores.ofertas SET margen = $2::int, precio_venta_cents = $3::int WHERE id = $1::uuid
             RETURNING id",
        )
        .bind(oferta_id)
        .bind(margen)
        .bind(nuevo.centavos)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        if actualizado.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Oferta", oferta_id));
        }
        sqlx::query(
            "INSERT INTO stores.historico_precios (oferta_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
             VALUES ($1::uuid, $2::int, $3::int, NOW())",
        )
        .bind(oferta_id)
        .bind(entero(&oferta, "precio_venta_cents"))
        .bind(nuevo.centavos)
        .execute(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;

        let mut actualizada = oferta;
        if let Some(objeto) = actualizada.as_object_mut() {
            objeto.insert("margen".into(), Value::from(margen));
            objeto.insert("precio_venta_cents".into(), Value::from(nuevo.centavos));
        }
        serializar(&actualizada)
    }

    /// Ofertas activas de un producto/variante (guard de integridad de catalog).
    pub async fn contar_activas_por_producto(&self, producto_id: &str) -> Result<i64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(
            "SELECT COUNT(*)::int AS n FROM stores.ofertas
              WHERE item_id = $1::uuid AND estado = 'activa'",
        )
        .bind(producto_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        Ok(fila.get::<i32, _>("n") as i64)
    }

    /// Sincroniza el stock visible de las ofertas de UNA empresa (ADR-27).
    pub async fn sincronizar_stock_de_oferta(
        &self,
        empresa_id: &str,
        sku: &str,
        stock_nuevo: i64,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        sqlx::query(
            "UPDATE stores.ofertas o
                SET stock = $3::int, estado = CASE WHEN $3 > 0 THEN 'activa' ELSE 'agotada' END
                FROM stores.tiendas t
               WHERE t.id = o.tienda_id
                 AND t.empresa_id = $1::uuid
                 AND o.item_type = 'producto'
                 AND LOWER(o.sku) = LOWER($2)",
        )
        .bind(empresa_id)
        .bind(sku)
        .bind(stock_nuevo.max(0))
        .execute(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_parsear_y_serializar_montos_en_centavos() {
        assert_eq!(Money::parsear("1150.00").expect("monto").centavos, 115_000);
        assert_eq!(Money::parsear("1150").expect("monto").centavos, 115_000);
        assert_eq!(Money::parsear("1150,5").expect("monto").centavos, 115_050);
        assert_eq!(Money::parsear(" 0.01 ").expect("monto").centavos, 1);
        assert_eq!(
            Money::parsear("1150.00").expect("monto").string(),
            "1150.00"
        );
        assert_eq!(Money::parsear("0.01").expect("monto").string(), "0.01");
        assert_eq!(
            Money::parsear("1.234").expect_err("invalido").codigo,
            "MONTO_INVALIDO"
        );
        assert_eq!(
            Money::parsear("-1").expect_err("negativo").codigo,
            "MONTO_INVALIDO"
        );
    }

    #[test]
    fn debe_aplicar_el_margen_con_redondeo_como_javascript() {
        let base = Money::desde_centavos(115_000).expect("base");
        assert_eq!(base.aplicar_margen(15).expect("margen").centavos, 132_250);
        assert_eq!(base.aplicar_margen(0).expect("margen").centavos, 115_000);
        assert_eq!(base.aplicar_margen(90).expect("margen").centavos, 218_500);
        assert_eq!(
            base.aplicar_margen(91).expect_err("margen").codigo,
            "MARGEN_INVALIDO"
        );
        assert_eq!(
            Money::desde_centavos(1)
                .expect("uno")
                .aplicar_margen(15)
                .expect("margen")
                .centavos,
            1
        );
        assert_eq!(
            Money::desde_centavos(3)
                .expect("tres")
                .aplicar_margen(50)
                .expect("margen")
                .centavos,
            5
        );
    }

    #[test]
    fn debe_parsear_la_paginacion_como_el_servicio_original() {
        let paginacion = parsear_paginacion(Some("2"), Some("50"));
        assert_eq!(paginacion.pagina, 2);
        assert_eq!(paginacion.limite, 50);
        let defecto = parsear_paginacion(None, None);
        assert_eq!((defecto.pagina, defecto.limite), (1, 20));
        let invalida = parsear_paginacion(Some("abc"), Some("0"));
        assert_eq!((invalida.pagina, invalida.limite), (1, 20));
        let tope = parsear_paginacion(Some("-3"), Some("500"));
        assert_eq!((tope.pagina, tope.limite), (1, 100));
        let decimal = parsear_paginacion(Some("3.9"), Some("20.5"));
        assert_eq!((decimal.pagina, decimal.limite), (3, 20));
    }

    #[test]
    fn debe_crear_la_pagina_con_el_shape_del_contrato() {
        let pagina = crear_pagina(
            vec![json!({"id": "o-1"})],
            41,
            Paginacion {
                pagina: 3,
                limite: 20,
            },
        );
        assert_eq!(pagina["items"][0]["id"], "o-1");
        assert_eq!(pagina["total"], 41);
        assert_eq!(pagina["paginas"], 3);
        let vacia = crear_pagina(
            Vec::new(),
            0,
            Paginacion {
                pagina: 1,
                limite: 20,
            },
        );
        assert_eq!(vacia["paginas"], 1);
    }

    #[test]
    fn debe_serializar_la_oferta_con_los_tipos_del_contrato() {
        let fila = json!({
            "id": "o-1",
            "tienda_id": "t-1",
            "vendedor_id": "u-1",
            "empresa_id": "e-1",
            "participacion_porcentaje": 10,
            "item_type": "producto",
            "item_id": "i-1",
            "sku": "SKU-1",
            "producto_nombre": "Cafe",
            "atributos": {"tamano": "1kg"},
            "imagen_objeto_id": null,
            "margen": 15,
            "precio_base_cents": 115000,
            "precio_venta_cents": 132250,
            "stock": 5,
            "estado": "activa",
            "moneda": "NIO",
            "creado_en": "2026-11-13T00:00:00.000Z"
        });
        let oferta = serializar(&fila).expect("oferta");
        assert_eq!(oferta["precio_base"], "1150.00");
        assert_eq!(oferta["precio_venta"], "1322.50");
        assert_eq!(oferta["imagen_url"], Value::Null);
        assert_eq!(oferta["moneda"], "NIO");
        assert_eq!(oferta["atributos"]["tamano"], "1kg");
        let publica = a_oferta_publica(&oferta);
        let claves = serde_json::to_value(publica).expect("publica");
        assert!(claves.get("margen").is_none());
        assert!(claves.get("empresa_id").is_none());
        assert_eq!(claves["precio_venta"], "1322.50");
    }

    #[test]
    fn debe_omitir_los_campos_ausentes_de_la_fila_del_update() {
        let fila = json!({
            "id": "o-1",
            "tienda_id": "t-1",
            "vendedor_id": "u-1",
            "item_type": "producto",
            "item_id": "i-1",
            "sku": "SKU-1",
            "producto_nombre": "Cafe",
            "margen": 15,
            "precio_base_cents": 100,
            "precio_venta_cents": 115,
            "stock": 1,
            "estado": "activa",
            "creado_en": "2026-11-13T00:00:00.000Z"
        });
        let oferta = serializar(&fila).expect("oferta");
        assert!(oferta.get("empresa_id").is_none());
        assert!(oferta.get("participacion_porcentaje").is_none());
        assert_eq!(oferta["moneda"], "NIO");
    }
}
