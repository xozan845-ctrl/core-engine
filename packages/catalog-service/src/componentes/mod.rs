//! Composición de items (Fase D, ADR-0001).
//!
//! Réplica de `items/componentes.service.ts`. Un item compuesto (plato, kit,
//! combo) consume sus componentes al venderlo. El grafo es acíclico y cada
//! componente debe ser un producto de la empresa.
//!
//! `Componentes::nueva` recibe también `Items` porque la composición se apoya
//! en el catálogo unificado; hoy la validación se hace contra `catalog.items`
//! con las mismas consultas que el servicio TypeScript.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use std::sync::Arc;

pub struct Componentes {
    db: Arc<Db>,
    _items: Arc<Items>,
}

impl Componentes {
    pub fn nueva(db: Arc<Db>, items: Arc<Items>) -> Self {
        Self { db, _items: items }
    }

    pub async fn listar(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT c.id, c.empresa_id, c.item_id, c.componente_item_id,
                    i.codigo, i.nombre, i.stock, c.cantidad, c.creado_en, c.actualizado_en
               FROM catalog.item_componentes c
               JOIN catalog.items i ON i.id = c.componente_item_id
              WHERE c.empresa_id = $1 AND c.item_id = $2
              ORDER BY i.nombre",
        )
        .bind(empresa_id)
        .bind(item_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        item_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let componente_id =
            validar_componente(&texto_o_vacio(input.get("componente_item_id")), item_id)?;
        let cantidad = validar_cantidad(input.get("cantidad").unwrap_or(&Value::Null))?;
        let componente = self.verificar_item(empresa_id, &componente_id).await?;
        // El padre puede ser producto, servicio o alquiler (una reparación
        // consume repuestos); los componentes siempre deben ser productos.
        if componente.tipo != "producto" {
            return Err(ApiError::dominio(
                "COMPONENTE_INVALIDO",
                "Los componentes deben ser productos de la empresa.",
            ));
        }
        if self.hay_ciclo(empresa_id, item_id, &componente_id).await? {
            return Err(ApiError::conflicto("La composicion generaria un ciclo."));
        }

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(
                "WITH insertado AS (
                   INSERT INTO catalog.item_componentes
                     (empresa_id, item_id, componente_item_id, cantidad)
                   VALUES ($1, $2, $3, $4::bigint)
                   RETURNING id, empresa_id, item_id, componente_item_id, cantidad,
                             creado_en, actualizado_en
                 )
                 SELECT i.id, i.empresa_id, i.item_id, i.componente_item_id,
                        it.codigo, it.nombre, it.stock, i.cantidad, i.creado_en, i.actualizado_en
                   FROM insertado i JOIN catalog.items it ON it.id = i.componente_item_id",
            )
            .bind(empresa_id)
            .bind(item_id)
            .bind(&componente_id)
            .bind(cantidad)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::dominio(
                    "COMPONENTE_NO_CREADO",
                    "No se pudo guardar el componente.",
                ))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_unicidad(&error) {
                    Err(ApiError::conflicto(
                        "Ese producto ya es componente del item.",
                    ))
                } else {
                    Err(interno(error))
                }
            }
        }
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        item_id: &str,
        componente_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let cantidad = validar_cantidad(input.get("cantidad").unwrap_or(&Value::Null))?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.item_componentes
                SET cantidad = $1::bigint, actualizado_en = NOW()
              WHERE id = $2 AND empresa_id = $3 AND item_id = $4
              RETURNING id",
        )
        .bind(cantidad)
        .bind(componente_id)
        .bind(empresa_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if fila.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Componente", componente_id));
        }
        tx.commit().await.map_err(interno)?;
        self.buscar(empresa_id, item_id, componente_id).await
    }

    pub async fn eliminar(
        &self,
        empresa_id: &str,
        item_id: &str,
        componente_id: &str,
    ) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "DELETE FROM catalog.item_componentes
              WHERE id = $1 AND empresa_id = $2 AND item_id = $3
              RETURNING id",
        )
        .bind(componente_id)
        .bind(empresa_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if fila.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Componente", componente_id));
        }
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    /// ¿El item padre es alcanzable desde el componente? (evita ciclos).
    ///
    /// Misma CTE recursiva que el contrato: parte de los componentes de
    /// `componente_id`, expande el grafo de la empresa y busca `item_id`.
    pub async fn validar_ciclo(
        &self,
        empresa_id: &str,
        item_id: &str,
        componente_id: &str,
    ) -> Result<bool, ApiError> {
        self.hay_ciclo(empresa_id, item_id, componente_id).await
    }

    async fn hay_ciclo(
        &self,
        empresa_id: &str,
        item_id: &str,
        componente_id: &str,
    ) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "WITH RECURSIVE alcanzables AS (
               SELECT componente_item_id
                 FROM catalog.item_componentes
                WHERE item_id = $1 AND empresa_id = $3
                UNION
               SELECT c.componente_item_id
                 FROM catalog.item_componentes c
                 JOIN alcanzables a ON c.item_id = a.componente_item_id
                WHERE c.empresa_id = $3
             )
             SELECT 1 AS uno FROM alcanzables WHERE componente_item_id = $2 LIMIT 1",
        )
        .bind(componente_id)
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    async fn buscar(
        &self,
        empresa_id: &str,
        item_id: &str,
        componente_id: &str,
    ) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT c.id, c.empresa_id, c.item_id, c.componente_item_id,
                    i.codigo, i.nombre, i.stock, c.cantidad, c.creado_en, c.actualizado_en
               FROM catalog.item_componentes c
               JOIN catalog.items i ON i.id = c.componente_item_id
              WHERE c.id = $1 AND c.empresa_id = $2 AND c.item_id = $3",
        )
        .bind(componente_id)
        .bind(empresa_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Componente", componente_id)),
        }
    }

    async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemBasico, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, tipo, codigo, nombre, stock FROM catalog.items
              WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::no_encontrado("Item", item_id))?;
        let valor = fila_json(&fila)?;
        Ok(ItemBasico {
            tipo: valor
                .get("tipo")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }
}

struct ItemBasico {
    tipo: String,
}

fn filas_json(filas: &[PgRow]) -> Result<Value, ApiError> {
    let valores = filas
        .iter()
        .map(fila_a_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(interno)?;
    Ok(Value::Array(valores))
}

fn fila_json(fila: &PgRow) -> Result<Value, ApiError> {
    fila_a_json(fila).map_err(interno)
}

fn es_unicidad(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|base| base.code())
        .is_some_and(|codigo| codigo == "23505")
}

fn texto_o_vacio(valor: Option<&Value>) -> String {
    let Some(valor) = valor else {
        return String::new();
    };
    match valor {
        Value::Null => String::new(),
        Value::String(texto) => texto.clone(),
        Value::Number(numero) => numero.to_string(),
        Value::Bool(valor) => valor.to_string(),
        _ => String::new(),
    }
}

fn validar_componente(componente_id: &str, item_id: &str) -> Result<String, ApiError> {
    if !es_uuid(componente_id) {
        return Err(ApiError::dominio(
            "COMPONENTE_INVALIDO",
            "El componente debe ser un UUID.",
        ));
    }
    if componente_id == item_id {
        return Err(ApiError::dominio(
            "COMPONENTE_INVALIDO",
            "Un item no puede componerse de si mismo.",
        ));
    }
    Ok(componente_id.to_string())
}

fn validar_cantidad(valor: &Value) -> Result<i64, ApiError> {
    let cantidad = numero_entero(valor).filter(|valor| *valor >= 1);
    cantidad.ok_or_else(|| {
        ApiError::dominio(
            "COMPONENTE_CANTIDAD_INVALIDA",
            "La cantidad debe ser un entero positivo.",
        )
    })
}

fn numero_entero(valor: &Value) -> Option<i64> {
    let numero = match valor {
        Value::Number(numero) => numero.as_f64()?,
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                0.0
            } else {
                texto.parse::<f64>().ok()?
            }
        }
        Value::Bool(valor) => {
            if *valor {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        _ => return None,
    };
    if !numero.is_finite() || numero.fract() != 0.0 {
        return None;
    }
    if numero < i64::MIN as f64 || numero > i64::MAX as f64 {
        return None;
    }
    Some(numero as i64)
}

fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(indice, byte)| {
        if matches!(indice, 8 | 13 | 18 | 23) {
            *byte == b'-'
        } else {
            byte.is_ascii_hexdigit()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    const ITEM: &str = "11111111-1111-4111-8111-111111111111";
    const COMPONENTE: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn debe_rechazar_auto_referencia_y_uuids_invalidos() {
        assert_eq!(
            validar_componente(ITEM, ITEM).expect_err("auto").codigo,
            "COMPONENTE_INVALIDO"
        );
        assert_eq!(
            validar_componente(ITEM, ITEM).expect_err("auto").mensaje,
            "Un item no puede componerse de si mismo."
        );
        assert_eq!(
            validar_componente("no-uuid", ITEM)
                .expect_err("uuid")
                .codigo,
            "COMPONENTE_INVALIDO"
        );
        assert_eq!(
            validar_componente(COMPONENTE, ITEM).expect("componente"),
            COMPONENTE
        );
    }

    #[test]
    fn debe_validar_la_cantidad_como_entero_positivo() {
        assert_eq!(validar_cantidad(&json!(2)).expect("cantidad"), 2);
        assert_eq!(validar_cantidad(&json!("3")).expect("cantidad"), 3);
        assert_eq!(
            validar_cantidad(&json!(0)).expect_err("cero").codigo,
            "COMPONENTE_CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_cantidad(&json!(1.5)).expect_err("decimal").codigo,
            "COMPONENTE_CANTIDAD_INVALIDA"
        );
    }

    /// El mismo recorrido que la CTE: desde el componente hacia sus hijos; si
    /// el padre aparece en el camino, agregarlo cerraría un ciclo.
    fn alcanza(aristas: &[(&str, &str)], desde: &str, objetivo: &str) -> bool {
        let mut pendientes = VecDeque::from([desde.to_string()]);
        let mut vistos = HashSet::new();
        while let Some(actual) = pendientes.pop_front() {
            if !vistos.insert(actual.clone()) {
                continue;
            }
            for (padre, hijo) in aristas {
                if *padre == actual {
                    if *hijo == objetivo {
                        return true;
                    }
                    pendientes.push_back((*hijo).to_string());
                }
            }
        }
        false
    }

    #[test]
    fn debe_detectar_ciclos_en_el_grafo_de_componentes() {
        let aristas = [("kit", "salsa"), ("salsa", "tomate"), ("tomate", "kit")];
        assert!(alcanza(&aristas, "salsa", "kit"));
        assert!(alcanza(&aristas, "tomate", "kit"));
        assert!(!alcanza(&aristas, "kit", "otro"));
        assert!(!alcanza(&[], "kit", "salsa"));
    }
}
