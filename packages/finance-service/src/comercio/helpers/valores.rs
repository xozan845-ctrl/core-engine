use super::super::*;

pub fn con_defaults(mut objeto: Map<String, Value>, defaults: Value) -> Map<String, Value> {
    if let Some(defaults) = defaults.as_object() {
        for (clave, valor) in defaults {
            objeto.entry(clave.clone()).or_insert_with(|| valor.clone());
        }
    }
    objeto
}

pub fn total_lineas(lineas: &Value) -> i64 {
    let Some(lineas) = lineas.as_array() else {
        return 0;
    };
    lineas
        .iter()
        .map(|linea| {
            let precio = numero(linea.get("unitCostCents"));
            let cantidad = numero(linea.get("quantity"));
            (precio as f64 * cantidad as f64).round() as i64
        })
        .sum()
}

pub fn numero(valor: Option<&Value>) -> i64 {
    match valor {
        Some(Value::Number(numero)) => numero
            .as_i64()
            .or_else(|| numero.as_f64().map(|valor| valor.round() as i64))
            .unwrap_or(0),
        Some(Value::String(texto)) => texto.parse().unwrap_or(0),
        _ => 0,
    }
}

pub fn lineas_con_id(lineas: &[LineaCompra]) -> Value {
    Value::Array(
        lineas
            .iter()
            .map(|linea| {
                json!({
                    "id": Uuid::new_v4().to_string(),
                    "productId": linea.product_id,
                    "name": linea.name,
                    "sku": linea.sku.clone().unwrap_or_default(),
                    "quantity": linea.quantity,
                    "unitCostCents": linea.unit_cost_cents,
                    "salePriceCents": linea.sale_price_cents,
                })
            })
            .collect(),
    )
}

/// Dígito verificador EAN-13 (pesos 1 y 3 alternados desde la izquierda).
pub fn digito_ean13(base: &str) -> u32 {
    let suma: u32 = base
        .chars()
        .enumerate()
        .filter_map(|(indice, digito)| {
            digito
                .to_digit(10)
                .map(|valor| valor * if indice % 2 == 0 { 1 } else { 3 })
        })
        .sum();
    (10 - (suma % 10)) % 10
}

pub fn generar_codigo(simbologia: &str) -> String {
    if simbologia == "EAN13" {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let base: String = bytes
            .iter()
            .flat_map(|byte| [byte / 10 % 10, byte % 10])
            .take(12)
            .map(|digito| digito.to_string())
            .collect();
        let digito = digito_ean13(&base);
        return format!("{base}{digito}");
    }
    let millis = Utc::now().timestamp_millis() as u64;
    format!("C128-{:X}", millis)
}

pub fn hoy() -> String {
    iso_ms(Utc::now()).chars().take(10).collect()
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::dominio("ID_INVALIDO", "El id debe ser un UUID."))
}
