//! Servicios de la empresa (R-PS-02).
//!
//! Réplica de `servicios.service.ts`: es un recurso propio del catálogo, no un
//! producto sin stock. Tiene duración, precio/costo y baja lógica. Desde la
//! Fase 3 vive en `catalog.items` con `tipo='servicio'`.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use crate::money::Money;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use crate::productos::Productos;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashSet;
use std::sync::Arc;

const MAX_DURACION: i64 = 1440;

const COLUMNAS_SERVICIO: &str = "i.id, i.empresa_id, i.codigo, i.nombre, i.descripcion,
  i.categoria, i.marca, i.codigo_barras, i.precio_cents, i.costo_cents, i.unidad,
  i.duration_minutes AS duracion_minutos, i.atributos, i.estado, i.creado_en,
  i.actualizado_en";

pub struct Servicios {
    db: Arc<Db>,
    productos: Arc<Productos>,
    items: Option<Arc<Items>>,
}

impl Servicios {
    /// Constructor pedido por el dominio. `productos` se usa únicamente para
    /// `validar_empresa_catalogo` (ADR-27); si el llamador ya tiene una
    /// instancia compartida use [`Servicios::con_productos`].
    pub fn nueva(db: Arc<Db>, items: Option<Arc<Items>>) -> Self {
        let productos = Arc::new(Productos::nueva(db.clone(), None, items.clone()));
        Self {
            db,
            productos,
            items,
        }
    }

    pub fn con_productos(
        db: Arc<Db>,
        productos: Arc<Productos>,
        items: Option<Arc<Items>>,
    ) -> Self {
        Self {
            db,
            productos,
            items,
        }
    }

    pub async fn listar(&self, filtros: &Value) -> Result<Value, ApiError> {
        let empresa_id = exigir_empresa(filtros.get("empresa_id"))?;
        let paginacion = paginacion_de(filtros);
        let mut params: Vec<String> = vec![empresa_id];
        let mut condiciones: Vec<String> = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.tipo = 'servicio'".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        if let Some(q) = filtros.get("q").filter(|valor| valor_verdadero(valor)) {
            params.push(format!("%{}%", texto_js(q).to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0}
                  OR LOWER(i.marca) LIKE ${0} OR LOWER(i.codigo_barras) LIKE ${0})",
                params.len()
            ));
        }
        if let Some(estado) = filtros.get("estado").filter(|valor| valor_verdadero(valor)) {
            params.push(texto_js(estado));
            condiciones.push(format!("i.estado = ${}", params.len()));
        }
        if let Some(categoria) = filtros
            .get("categoria")
            .filter(|valor| valor_verdadero(valor))
        {
            params.push(texto_js(categoria));
            condiciones.push(format!("LOWER(i.categoria) = LOWER(${})", params.len()));
        }
        let where_sql = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM catalog.items i {where_sql}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let sql = format!(
            "SELECT {COLUMNAS_SERVICIO}
               FROM catalog.items i {where_sql}
              ORDER BY i.creado_en DESC
              LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let items = filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno).and_then(serializar))
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn encontrar_por_id(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_SERVICIO}
               FROM catalog.items i
              WHERE i.id = $1::uuid AND i.empresa_id = $2::uuid AND i.tipo = 'servicio'
                AND i.eliminado_en IS NULL"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(serializar(fila_a_json(&fila).map_err(interno)?)?)),
            None => Ok(None),
        }
    }

    /// Escaneo: código de barras o código interno, solo servicios activos.
    pub async fn encontrar_por_codigo(
        &self,
        codigo: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        let valor = codigo.trim();
        if valor.is_empty() {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_SERVICIO}
               FROM catalog.items i
              WHERE i.empresa_id = $1::uuid AND i.tipo = 'servicio' AND i.eliminado_en IS NULL
                AND i.estado = 'activo'
                AND (i.codigo_barras = $2 OR upper(i.codigo) = upper($2))
              LIMIT 1"
        ))
        .bind(empresa_id)
        .bind(valor)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(serializar(fila_a_json(&fila).map_err(interno)?)?)),
            None => Ok(None),
        }
    }

    pub async fn crear(&self, datos: &Value, empresa_id: &str) -> Result<Value, ApiError> {
        self.productos.validar_empresa_catalogo(empresa_id).await?;
        let codigo = normalizar_codigo(&texto(datos, "codigo"))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = validar_precio(datos.get("precio"))?;
        let costo = match datos.get("costo") {
            Some(Value::Null) | None => validar_precio(Some(&json!("0")))?,
            Some(valor) => validar_precio(Some(valor))?,
        };
        let duracion = validar_duracion(datos.get("duracion_minutos"))?;
        let atributos = atributos_de(datos);
        if let Some(items) = &self.items {
            items
                .validar_atributos(
                    empresa_id,
                    "servicio",
                    &categoria_de(datos),
                    atributos.clone(),
                )
                .await?;
        }
        let codigo_barras = texto(datos, "codigo_barras").trim().to_string();
        self.validar_codigo_unico(empresa_id, &codigo).await?;
        self.validar_barcode_unico(empresa_id, &codigo_barras, None)
            .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO catalog.items
               (empresa_id, tipo, codigo, nombre, descripcion, categoria, marca, codigo_barras,
                precio_cents, costo_cents, unidad, duration_minutes, atributos, estado,
                track_inventory)
             VALUES ($1::uuid, 'servicio', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13,
                     false)
             RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                       codigo_barras, precio_cents, costo_cents, unidad,
                       duration_minutes AS duracion_minutos, atributos, estado, creado_en,
                       actualizado_en",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(texto(datos, "descripcion").trim().to_string())
        .bind(texto(datos, "categoria").trim().to_string())
        .bind(texto(datos, "marca").trim().to_string())
        .bind(&codigo_barras)
        .bind(precio.centavos)
        .bind(costo.centavos)
        .bind(defecto(texto(datos, "unidad"), "Hora (h)"))
        .bind(duracion)
        .bind(atributos.to_string())
        .bind(estado_de(datos))
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "SERVICIO_NO_CREADO",
                "No se pudo crear el servicio.",
            ));
        };
        serializar(fila_a_json(&fila).map_err(interno)?)
    }

    pub async fn actualizar(
        &self,
        id: &str,
        cambios: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        let actual = self
            .encontrar_por_id(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Servicio", id))?;
        let precio = match cambios.get("precio") {
            None => Money::parsear(&texto(&actual, "precio"))?,
            Some(valor) => validar_precio(Some(valor))?,
        };
        let costo = match cambios.get("costo") {
            None => Money::parsear(&texto(&actual, "costo"))?,
            Some(valor) => validar_precio(Some(valor))?,
        };
        let duracion = match cambios.get("duracion_minutos") {
            None => actual
                .get("duracion_minutos")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            Some(valor) => validar_duracion(Some(valor))?,
        };
        let nombre = match cambios.get("nombre") {
            None => texto(&actual, "nombre"),
            Some(valor) => validar_nombre(&texto_js(valor))?,
        };
        let codigo_barras = match cambios.get("codigo_barras") {
            None => texto(&actual, "codigo_barras"),
            Some(valor) => texto_js(valor).trim().to_string(),
        };
        let atributos = match cambios.get("atributos") {
            None => actual
                .get("atributos")
                .cloned()
                .unwrap_or_else(|| json!({})),
            Some(valor) => valor.clone(),
        };
        if cambios.get("atributos").is_some() {
            let categoria = cambios
                .get("categoria")
                .filter(|valor| !valor.is_null())
                .map(texto_js)
                .unwrap_or_else(|| texto(&actual, "categoria"));
            if let Some(items) = &self.items {
                items
                    .validar_atributos(empresa_id, "servicio", &categoria, atributos.clone())
                    .await?;
            }
        }
        if codigo_barras != texto(&actual, "codigo_barras") {
            self.validar_barcode_unico(empresa_id, &codigo_barras, Some(id))
                .await?;
        }
        let descripcion = match cambios.get("descripcion") {
            None => texto(&actual, "descripcion"),
            Some(valor) => texto_js(valor).trim().to_string(),
        };
        let categoria = match cambios.get("categoria") {
            None => texto(&actual, "categoria"),
            Some(valor) => texto_js(valor).trim().to_string(),
        };
        let marca = match cambios.get("marca") {
            None => texto(&actual, "marca"),
            Some(valor) => texto_js(valor).trim().to_string(),
        };
        let unidad = match cambios.get("unidad") {
            None => texto(&actual, "unidad"),
            Some(valor) => {
                let unidad = texto_js(valor).trim().to_string();
                if unidad.is_empty() {
                    texto(&actual, "unidad")
                } else {
                    unidad
                }
            }
        };
        let estado = match cambios.get("estado") {
            None => texto(&actual, "estado"),
            Some(valor) => {
                if texto_js(valor) == "inactivo" {
                    "inactivo".to_string()
                } else {
                    "activo".to_string()
                }
            }
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.items
                SET nombre = $3, descripcion = $4, categoria = $5, marca = $6,
                    codigo_barras = $7, precio_cents = $8, costo_cents = $9, unidad = $10,
                    duration_minutes = $11, atributos = $12::jsonb, estado = $13,
                    actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND tipo = 'servicio'
                AND eliminado_en IS NULL
              RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                        codigo_barras, precio_cents, costo_cents, unidad,
                        duration_minutes AS duracion_minutos, atributos, estado, creado_en,
                        actualizado_en",
        )
        .bind(id)
        .bind(empresa_id)
        .bind(&nombre)
        .bind(&descripcion)
        .bind(&categoria)
        .bind(&marca)
        .bind(&codigo_barras)
        .bind(precio.centavos)
        .bind(costo.centavos)
        .bind(&unidad)
        .bind(duracion)
        .bind(atributos.to_string())
        .bind(&estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Servicio", id));
        };
        serializar(fila_a_json(&fila).map_err(interno)?)
    }

    /// Baja lógica: conserva el registro y lo marca inactivo (R-DB-5).
    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND tipo = 'servicio'
                AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Servicio", id));
        }
        Ok(())
    }

    async fn validar_codigo_unico(&self, empresa_id: &str, codigo: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM catalog.items
              WHERE empresa_id = $1::uuid AND tipo = 'servicio' AND upper(codigo) = $2
                AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "Ya existe un servicio con el código {codigo}."
            )));
        }
        Ok(())
    }

    async fn validar_barcode_unico(
        &self,
        empresa_id: &str,
        codigo_barras: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if codigo_barras.is_empty() {
            return Ok(());
        }
        let mut sql = "SELECT 1 FROM catalog.items
                        WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL"
            .to_string();
        if excluir_id.is_some() {
            sql.push_str(" AND id <> $3::uuid");
        }
        let mut consulta = sqlx::query(&sql).bind(empresa_id).bind(codigo_barras);
        if let Some(excluir) = excluir_id {
            consulta = consulta.bind(excluir);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "El código de barras {codigo_barras} ya está registrado."
            )));
        }
        Ok(())
    }
}

// ── helpers puros ─────────────────────────────────────────────────────────

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_js(valor: &Value) -> String {
    match valor {
        Value::String(texto) => texto.clone(),
        Value::Number(numero) => numero.to_string(),
        Value::Bool(valor) => valor.to_string(),
        Value::Null => "null".to_string(),
        otro => otro.to_string(),
    }
}

fn valor_verdadero(valor: &Value) -> bool {
    match valor {
        Value::Null => false,
        Value::Bool(valor) => *valor,
        Value::Number(numero) => numero.as_f64().map(|valor| valor != 0.0).unwrap_or(true),
        Value::String(texto) => !texto.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(indice, byte)| {
            if matches!(indice, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn paginacion_de(filtros: &Value) -> crate::paginacion::Paginacion {
    let pagina = filtros.get("pagina").map(texto_js);
    let limite = filtros.get("limite").map(texto_js);
    parsear_paginacion(pagina.as_deref(), limite.as_deref())
}

fn exigir_empresa(valor: Option<&Value>) -> Result<String, ApiError> {
    let empresa = valor.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if empresa.is_empty() {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "Los servicios se consultan por empresa.",
        ));
    }
    Ok(empresa.to_string())
}

fn normalizar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = (2..=32).contains(&codigo.len())
        && codigo.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            "El código del servicio solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim().to_string();
    let longitud = nombre.chars().count();
    if !(2..=200).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre del servicio debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(nombre)
}

fn validar_precio(valor: Option<&Value>) -> Result<Money, ApiError> {
    match valor {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        Some(Value::Bool(valor)) => Money::parsear(if *valor { "true" } else { "false" }),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn validar_duracion(valor: Option<&Value>) -> Result<i64, ApiError> {
    let duracion = valor.and_then(|valor| match valor {
        Value::Number(numero) => numero.as_i64().or_else(|| {
            numero
                .as_f64()
                .filter(|valor| valor.is_finite() && valor.fract() == 0.0)
                .map(|valor| valor as i64)
        }),
        _ => None,
    });
    match duracion {
        Some(duracion) if (1..=MAX_DURACION).contains(&duracion) => Ok(duracion),
        _ => Err(ApiError::dominio(
            "DURACION_INVALIDA",
            format!("La duración debe ser un entero entre 1 y {MAX_DURACION} minutos."),
        )),
    }
}

fn categoria_de(datos: &Value) -> String {
    match datos.get("categoria") {
        Some(Value::String(categoria)) => categoria.clone(),
        _ => "general".to_string(),
    }
}

fn atributos_de(datos: &Value) -> Value {
    datos
        .get("atributos")
        .filter(|valor| !valor.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn estado_de(datos: &Value) -> String {
    if texto(datos, "estado") == "inactivo" {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

fn defecto(valor: String, por_defecto: &str) -> String {
    let valor = valor.trim().to_string();
    if valor.is_empty() {
        por_defecto.to_string()
    } else {
        valor
    }
}

fn serializar(fila: Value) -> Result<Value, ApiError> {
    let precio = Money::desde_centavos(
        fila.get("precio_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    )?;
    let costo =
        Money::desde_centavos(fila.get("costo_cents").and_then(Value::as_i64).unwrap_or(0))?;
    let mut objeto = Map::new();
    objeto.insert("id".into(), Value::String(texto(&fila, "id")));
    objeto.insert(
        "empresa_id".into(),
        Value::String(texto(&fila, "empresa_id")),
    );
    objeto.insert("codigo".into(), Value::String(texto(&fila, "codigo")));
    objeto.insert("nombre".into(), Value::String(texto(&fila, "nombre")));
    objeto.insert(
        "descripcion".into(),
        Value::String(texto(&fila, "descripcion")),
    );
    objeto.insert("categoria".into(), Value::String(texto(&fila, "categoria")));
    objeto.insert("marca".into(), Value::String(texto(&fila, "marca")));
    objeto.insert(
        "codigo_barras".into(),
        Value::String(texto(&fila, "codigo_barras")),
    );
    objeto.insert("precio".into(), Value::String(precio.string()));
    objeto.insert("costo".into(), Value::String(costo.string()));
    objeto.insert("unidad".into(), Value::String(texto(&fila, "unidad")));
    objeto.insert(
        "duracion_minutos".into(),
        json!(fila
            .get("duracion_minutos")
            .and_then(Value::as_i64)
            .unwrap_or(0)),
    );
    objeto.insert(
        "atributos".into(),
        fila.get("atributos")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    objeto.insert(
        "estado".into(),
        Value::String(
            if texto(&fila, "estado") == "inactivo" {
                "inactivo"
            } else {
                "activo"
            }
            .to_string(),
        ),
    );
    objeto.insert("creado_en".into(), Value::String(texto(&fila, "creado_en")));
    objeto.insert(
        "actualizado_en".into(),
        Value::String(texto(&fila, "actualizado_en")),
    );
    Ok(Value::Object(objeto))
}

/// Deduplicación estable usada por las consultas de escaneo.
#[allow(dead_code)]
fn unicos(valores: &[String]) -> Vec<String> {
    let mut vistos = HashSet::new();
    valores
        .iter()
        .filter(|valor| vistos.insert(valor.to_string()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fila_servicio() -> Value {
        json!({
            "id": "11111111-2222-4333-8444-555555555555",
            "empresa_id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
            "codigo": "SERV-001",
            "nombre": "Instalación",
            "descripcion": "Instalación en sitio",
            "categoria": "servicios",
            "marca": "Core",
            "codigo_barras": "7501234567890",
            "precio_cents": 150000,
            "costo_cents": 50000,
            "unidad": "Hora (h)",
            "duracion_minutos": 60,
            "estado": "activo",
            "atributos": null,
            "creado_en": "2026-10-30T10:00:00.000Z",
            "actualizado_en": "2026-10-30T10:00:00.000Z",
        })
    }

    #[test]
    fn debe_normalizar_y_validar_el_codigo() {
        assert_eq!(normalizar_codigo(" serv-001 ").expect("codigo"), "SERV-001");
        assert_eq!(
            normalizar_codigo("A").expect_err("corto").codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            normalizar_codigo("SERV@1").expect_err("caracter").codigo,
            "CODIGO_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_nombre_duracion_y_montos() {
        assert_eq!(
            validar_nombre(" Instalación ").expect("nombre"),
            "Instalación"
        );
        assert_eq!(
            validar_nombre(" ").expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
        assert_eq!(validar_duracion(Some(&json!(60))).expect("duracion"), 60);
        assert_eq!(
            validar_duracion(Some(&json!(0))).expect_err("cero").codigo,
            "DURACION_INVALIDA"
        );
        assert_eq!(
            validar_duracion(Some(&json!(1441)))
                .expect_err("maximo")
                .codigo,
            "DURACION_INVALIDA"
        );
        assert_eq!(
            validar_precio(Some(&json!("-1")))
                .expect_err("negativo")
                .codigo,
            "MONTO_INVALIDO"
        );
    }

    #[test]
    fn debe_serializar_el_servicio_con_montos_y_defaults() {
        let servicio = serializar(fila_servicio()).expect("servicio");
        assert_eq!(servicio["precio"], "1500.00");
        assert_eq!(servicio["costo"], "500.00");
        assert_eq!(servicio["duracion_minutos"], 60);
        assert_eq!(servicio["estado"], "activo");
        assert_eq!(servicio["atributos"], json!({}));
    }

    #[test]
    fn debe_exigir_empresa_al_listar() {
        assert_eq!(
            exigir_empresa(None).expect_err("empresa").codigo,
            "EMPRESA_REQUERIDA"
        );
        assert_eq!(
            exigir_empresa(Some(&json!(" a-1 "))).expect("empresa"),
            "a-1"
        );
    }

    #[test]
    fn debe_deduplicar_valores_de_escaneo() {
        let valores = vec!["A".to_string(), "A".to_string(), "B".to_string()];
        assert_eq!(unicos(&valores), vec!["A".to_string(), "B".to_string()]);
    }
}
