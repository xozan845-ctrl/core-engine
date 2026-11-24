use super::super::*;

pub fn validar_texto(valor: &str, campo: &str, minimo: usize) -> Result<(), ApiError> {
    if valor.trim().len() < minimo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": [format!("minLength:{minimo}")]}
        ])));
    }
    Ok(())
}

pub fn validar_in(valor: &str, campo: &str, permitidos: &[&str]) -> Result<(), ApiError> {
    if !permitidos.contains(&valor) {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isIn"]}
        ])));
    }
    Ok(())
}

pub fn validar_rango(valor: f64, campo: &str, minimo: f64, maximo: f64) -> Result<(), ApiError> {
    if !valor.is_finite() || valor < minimo || valor > maximo {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["min", "max"]}
        ])));
    }
    Ok(())
}

pub fn validar_fecha(valor: &str, campo: &str) -> Result<(), ApiError> {
    let ok =
        valor.len() >= 10 && chrono::NaiveDate::parse_from_str(&valor[..10], "%Y-%m-%d").is_ok();
    if !ok {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isDateString"]}
        ])));
    }
    Ok(())
}

pub fn validar_email(valor: &str, campo: &str) -> Result<(), ApiError> {
    let (usuario, dominio) = valor.split_once('@').unwrap_or_default();
    if usuario.is_empty() || !dominio.contains('.') || dominio.starts_with('.') {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isEmail"]}
        ])));
    }
    Ok(())
}

pub fn validar_lineas(lineas: &[LineaCompra]) -> Result<(), ApiError> {
    for (indice, linea) in lineas.iter().enumerate() {
        if linea.name.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.name"), "restricciones": ["minLength:1"]}
            ])));
        }
        if !linea.quantity.is_finite() || linea.quantity < 1.0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.quantity"), "restricciones": ["min:1"]}
            ])));
        }
        if linea.unit_cost_cents < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": format!("lines.{indice}.unitCostCents"), "restricciones": ["min:0"]}
            ])));
        }
    }
    Ok(())
}
