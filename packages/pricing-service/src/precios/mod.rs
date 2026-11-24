//! Listas de precio y escalas por cantidad (Fase P1.1, ADR-0001).
//!
//! Réplica de `items/precios.service.ts`. Una lista agrupa precios por segmento
//! (mayorista, VIP, convenio) y puede ser la predeterminada de la empresa.
//! `lista_id NULL` es la escala base del item: es la que da contenido real a
//! `items.tipo_precio = 'escala'`.
//!
//! La resolución es una lectura: no reserva stock ni crea órdenes. Los
//! servicios que cotizan (orders, stores) pueden degradar al precio de la
//! oferta si el catálogo no responde.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;

const CODIGO: &str = r"^[a-z][a-z0-9_-]{1,39}$";
const SEGMENTO: &str = r"^[a-z][a-z0-9_-]{0,39}$";
const MAX_CANTIDAD: i64 = 1_000_000;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

const COLUMNAS_LISTA: &str = "id, empresa_id, codigo, nombre, descripcion, moneda,
  segmento_codigo, predeterminada, activo, creado_en, actualizado_en";
const COLUMNAS_PRECIO: &str = "id, empresa_id, lista_id, item_id, desde_cantidad,
  hasta_cantidad, precio_cents, creado_en, actualizado_en";

const MENSAJE_CONFLICTO_LISTA: &str = "La lista ya existe o el segmento ya tiene una lista activa.";
const MENSAJE_CONFLICTO_ESCALA: &str = "La escala se solapa con otra del mismo item.";

pub struct Precios {
    db: Arc<Db>,
}

mod consulta;
mod escalas;
mod listas;
mod precios_items;

impl Precios {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    // ── listas ─────────────────────────────────────────────────────────────
}

struct ItemPrecio {
    id: String,
    tipo_precio: String,
}

#[derive(Debug)]
struct RangoCantidad {
    desde: i64,
    hasta: Option<i64>,
    precio: i64,
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

fn error_conflicto(error: sqlx::Error, mensaje: &str) -> ApiError {
    if es_conflicto(&error) {
        ApiError::conflicto(mensaje)
    } else {
        interno(error)
    }
}

fn es_conflicto(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|base| base.code())
        .is_some_and(|codigo| codigo == "23P01" || codigo == "23505")
}

fn texto_de(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn entero_de(valor: &Value, clave: &str) -> Option<i64> {
    valor.get(clave).and_then(Value::as_i64)
}

fn booleano_de(valor: &Value, clave: &str) -> bool {
    valor.get(clave).and_then(Value::as_bool).unwrap_or(false)
}

fn texto(input: &Value, clave: &str) -> String {
    texto_o_vacio(input.get(clave))
}

fn texto_o_vacio(valor: Option<&Value>) -> String {
    match valor {
        None | Some(Value::Null) => String::new(),
        Some(valor) => texto_js(valor),
    }
}

/// `String(valor)` de JavaScript para los escalares que usa el contrato.
fn texto_js(valor: &Value) -> String {
    match valor {
        Value::Null => "null".to_string(),
        Value::Bool(valor) => valor.to_string(),
        Value::Number(valor) => valor.to_string(),
        Value::String(valor) => valor.clone(),
        Value::Array(valores) => valores.iter().map(texto_js).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn recortar(valor: String, maximo: usize) -> String {
    valor.chars().take(maximo).collect()
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        None | Some(Value::Null) => None,
        Some(Value::Bool(false)) => None,
        Some(Value::Number(numero)) if numero.as_f64() == Some(0.0) => None,
        Some(Value::String(texto)) if texto.is_empty() => None,
        Some(valor) => Some(texto_js(valor)),
    }
}

fn validar_rango_cantidad(
    desde: &Value,
    hasta: Option<&Value>,
    precio: &Value,
) -> Result<RangoCantidad, ApiError> {
    let desde = validar_cantidad(desde)?;
    let hasta = match hasta {
        None => None,
        Some(valor) => Some(validar_cantidad(valor)?),
    };
    if let Some(hasta) = hasta {
        if hasta < desde {
            return Err(ApiError::dominio(
                "ESCALA_INVALIDA",
                "La cantidad final no puede ser menor que la inicial.",
            ));
        }
    }
    Ok(RangoCantidad {
        desde,
        hasta,
        precio: validar_precio(precio)?,
    })
}

fn validar_cantidad(valor: &Value) -> Result<i64, ApiError> {
    let cantidad = numero_entero(valor).filter(|valor| *valor >= 1 && *valor <= MAX_CANTIDAD);
    cantidad.ok_or_else(|| {
        ApiError::dominio(
            "CANTIDAD_INVALIDA",
            format!("La cantidad debe ser un entero entre 1 y {MAX_CANTIDAD}."),
        )
    })
}

fn validar_precio(valor: &Value) -> Result<i64, ApiError> {
    let precio = numero_entero(valor).filter(|valor| *valor >= 0);
    let Some(precio) = precio else {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio debe ser un entero no negativo.",
        ));
    };
    Money::desde_centavos(precio)
        .map(|monto| monto.centavos)
        .map_err(|_| {
            ApiError::dominio(
                "PRECIO_INVALIDO",
                "El precio debe ser un entero no negativo.",
            )
        })
}

/// `Number(valor)` + `Number.isInteger`, sin punto flotante en la salida.
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

fn validar_codigo(valor: &str, campo: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    if !coincide(&codigo, CODIGO) {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            format!(
                "El {campo} debe empezar con letra y usar minusculas, numeros, guion o guion bajo."
            ),
        ));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    let longitud = nombre.chars().count();
    if !(2..=120).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 120 caracteres.",
        ));
    }
    Ok(nombre.to_string())
}

fn validar_moneda(valor: &str) -> Result<String, ApiError> {
    let moneda = valor.trim().to_uppercase();
    let valida = moneda.len() == 3 && moneda.chars().all(|caracter| caracter.is_ascii_uppercase());
    if !valida {
        return Err(ApiError::dominio(
            "MONEDA_INVALIDA",
            "La moneda debe ser un codigo ISO de 3 letras.",
        ));
    }
    Ok(moneda)
}

fn validar_segmento(valor: &str) -> Result<String, ApiError> {
    let segmento = valor.trim().to_lowercase();
    if !segmento.is_empty() && !coincide(&segmento, SEGMENTO) {
        return Err(ApiError::dominio(
            "SEGMENTO_INVALIDO",
            "El segmento debe empezar con letra y usar minusculas, numeros, guion o guion bajo.",
        ));
    }
    Ok(segmento)
}

fn validar_uuid(valor: &str, campo: &str) -> Result<String, ApiError> {
    if !es_uuid(valor) {
        return Err(ApiError::dominio(
            "ID_INVALIDO",
            format!("El {campo} debe ser un UUID."),
        ));
    }
    Ok(valor.to_string())
}

/// Regex del contrato: 8-4-4-4-12 hexadecimal con guiones.
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

/// Implementación acotada de `^[a-z][a-z0-9_-]{n,m}$` sin dependencias regex.
fn coincide(valor: &str, patron: &str) -> bool {
    if patron == CODIGO {
        let mut caracteres = valor.chars();
        matches!(caracteres.next(), Some(caracter) if caracter.is_ascii_lowercase()) && {
            let resto: Vec<char> = caracteres.collect();
            (1..=39).contains(&resto.len())
                && resto.iter().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || *caracter == '_'
                        || *caracter == '-'
                })
        }
    } else if patron == SEGMENTO {
        if valor.is_empty() {
            return true;
        }
        let mut caracteres = valor.chars();
        matches!(caracteres.next(), Some(caracter) if caracter.is_ascii_lowercase()) && {
            let resto: Vec<char> = caracteres.collect();
            resto.len() <= 39
                && resto.iter().all(|caracter| {
                    caracter.is_ascii_lowercase()
                        || caracter.is_ascii_digit()
                        || *caracter == '_'
                        || *caracter == '-'
                })
        }
    } else {
        false
    }
}

fn multiplicar(precio: i64, cantidad: i64) -> Result<i64, ApiError> {
    let total = precio
        .checked_mul(cantidad)
        .filter(|total| *total <= MAX_SAFE_INTEGER)
        .ok_or_else(|| {
            ApiError::dominio(
                "TOTAL_INVALIDO",
                "El total del precio excede el rango permitido.",
            )
        })?;
    Ok(total)
}

fn resumen_lista(lista: &Value) -> Value {
    json!({
        "id": lista.get("id").cloned().unwrap_or(Value::Null),
        "codigo": lista.get("codigo").cloned().unwrap_or(Value::Null),
        "nombre": lista.get("nombre").cloned().unwrap_or(Value::Null),
        "segmento_codigo": lista.get("segmento_codigo").cloned().unwrap_or(Value::Null),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_codigos_nombres_monedas_y_segmentos() {
        assert_eq!(
            validar_codigo(" Mayorista ", "codigo").expect("codigo"),
            "mayorista"
        );
        assert_eq!(
            validar_codigo("Mayorista!", "codigo")
                .expect_err("simbolo")
                .codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(
            validar_codigo("a", "codigo").expect_err("corto").codigo,
            "CODIGO_INVALIDO"
        );
        assert_eq!(validar_nombre(" VIP ").expect("nombre"), "VIP");
        assert_eq!(
            validar_nombre("x").expect_err("corto").codigo,
            "NOMBRE_INVALIDO"
        );
        assert_eq!(validar_moneda("usd").expect("moneda"), "USD");
        assert_eq!(
            validar_moneda("").expect_err("vacia").codigo,
            "MONEDA_INVALIDA"
        );
        assert_eq!(
            validar_moneda("peso").expect_err("moneda").codigo,
            "MONEDA_INVALIDA"
        );
        assert_eq!(validar_segmento("").expect("segmento"), "");
        assert_eq!(
            validar_segmento(" Mayorista ").expect("segmento"),
            "mayorista"
        );
        assert_eq!(
            validar_segmento("V I P").expect_err("segmento").codigo,
            "SEGMENTO_INVALIDO"
        );
    }

    #[test]
    fn debe_validar_uuid_con_el_regex_del_contrato() {
        assert!(es_uuid("11111111-1111-4111-8111-111111111111"));
        assert!(es_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"));
        assert!(!es_uuid("11111111111141118111111111111111"));
        assert!(!es_uuid("11111111-1111-4111-8111-11111111111z"));
        assert!(!es_uuid(""));
    }

    #[test]
    fn debe_validar_cantidades_y_el_rango_de_escala() {
        assert_eq!(validar_cantidad(&json!(1)).expect("cantidad"), 1);
        assert_eq!(validar_cantidad(&json!("12")).expect("cantidad"), 12);
        assert_eq!(
            validar_cantidad(&json!(1_000_000)).expect("cantidad"),
            1_000_000
        );
        assert_eq!(
            validar_cantidad(&json!(0)).expect_err("cero").codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_cantidad(&json!(1.5)).expect_err("decimal").codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_cantidad(&json!(1_000_001))
                .expect_err("maximo")
                .codigo,
            "CANTIDAD_INVALIDA"
        );
        assert_eq!(
            validar_rango_cantidad(&json!(10), Some(&json!(5)), &json!(100))
                .expect_err("rango")
                .codigo,
            "ESCALA_INVALIDA"
        );
        assert_eq!(
            validar_rango_cantidad(&json!(1), None, &json!(-1))
                .expect_err("precio")
                .codigo,
            "PRECIO_INVALIDO"
        );
    }

    #[test]
    fn debe_multiplicar_sin_salir_del_rango_seguro() {
        assert_eq!(multiplicar(1_000, 3).expect("total"), 3_000);
        assert_eq!(
            multiplicar(MAX_SAFE_INTEGER, 2)
                .expect_err("desborde")
                .codigo,
            "TOTAL_INVALIDO"
        );
    }

    #[test]
    fn debe_resumir_la_lista_con_las_cuatro_columnas_del_contrato() {
        let resumen = resumen_lista(&json!({
            "id": "33333333-3333-4333-8333-333333333333",
            "codigo": "mayorista",
            "nombre": "Mayorista",
            "segmento_codigo": "mayorista",
            "moneda": "NIO",
        }));
        assert_eq!(
            resumen,
            json!({
                "id": "33333333-3333-4333-8333-333333333333",
                "codigo": "mayorista",
                "nombre": "Mayorista",
                "segmento_codigo": "mayorista",
            })
        );
    }
}
