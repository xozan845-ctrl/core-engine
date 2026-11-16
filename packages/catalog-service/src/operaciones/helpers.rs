use super::*;

// ── funciones puras (probadas sin base de datos) ─────────────────────────

pub(super) fn configuracion_por_defecto(empresa_id: &str) -> Value {
    json!({
        "empresa_id": empresa_id,
        "metodo_costeo": "promedio",
        "permitir_stock_negativo": false,
        "alerta_stock_bajo": true,
        "actualizar_costo_en_compra": true,
        "unidad_defecto": UNIDAD_DEFECTO,
        "actualizado_en": "1970-01-01T00:00:00.000Z",
    })
}

pub(super) fn serializar_configuracion(fila: &Value) -> Result<Value, ApiError> {
    Ok(json!({
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "metodo_costeo": fila.get("metodo_costeo").cloned().unwrap_or(Value::Null),
        "permitir_stock_negativo": fila
            .get("permitir_stock_negativo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "alerta_stock_bajo": fila
            .get("alerta_stock_bajo")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "actualizar_costo_en_compra": fila
            .get("actualizar_costo_en_compra")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "unidad_defecto": fila
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .unwrap_or(UNIDAD_DEFECTO),
        "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
    }))
}

/// Construye el `datos` que consume `Productos::crear`, conservando el orden y
/// los campos opcionales de `ImportarProductoFila`.
pub(super) fn datos_producto_importado(fila: &Value, empresa_id: &str) -> Value {
    let mut datos = Map::new();
    datos.insert("empresa_id".into(), Value::String(empresa_id.to_string()));
    for clave in [
        "sku",
        "nombre",
        "descripcion",
        "categoria",
        "precio_base",
        "stock",
        "costo",
        "marca",
        "codigo_barras",
        "unidad",
        "stock_minimo",
        "stock_maximo",
        "es_materia_prima",
        "ubicacion",
    ] {
        if let Some(valor) = fila.get(clave) {
            if !valor.is_null() {
                datos.insert(clave.to_string(), valor.clone());
            }
        }
    }
    Value::Object(datos)
}

pub(super) fn serializar_promocion(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "item_id": fila.get("item_id").cloned().unwrap_or(Value::Null),
        "sku": fila.get("sku").and_then(Value::as_str).unwrap_or(""),
        "cantidad_requerida": fila.get("cantidad_requerida").cloned().unwrap_or(Value::Null),
        "cantidad_gratis": fila.get("cantidad_gratis").cloned().unwrap_or(Value::Null),
        "descuento_porcentaje": fila
            .get("descuento_porcentaje")
            .cloned()
            .unwrap_or(Value::Null),
        "fecha_desde": fecha_corta(fila.get("fecha_desde")),
        "fecha_hasta": fecha_corta(fila.get("fecha_hasta")),
        "estado": estado_serializado(fila.get("estado")),
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
        "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn serializar_movimiento(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "motivo": fila.get("motivo").and_then(Value::as_str).unwrap_or(""),
        "referencia": fila.get("referencia").and_then(Value::as_str).unwrap_or(""),
        "ubicacion_origen": fila
            .get("ubicacion_origen")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "ubicacion_destino": fila
            .get("ubicacion_destino")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "lineas": fila.get("lineas").cloned().unwrap_or_else(|| json!([])),
        "creado_por": fila.get("creado_por").cloned().unwrap_or(Value::Null),
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn validar_promocion(datos: &Value) -> Result<PromocionNormalizada, ApiError> {
    let nombre = validar_nombre(&texto(datos, "nombre"))?;
    let tipo = datos
        .get("tipo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !TIPOS_PROMOCION.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_PROMOCION_INVALIDO",
            "El tipo de promoción no es válido.",
        ));
    }
    let mut cantidad_requerida = entero_de(datos.get("cantidad_requerida"));
    let mut cantidad_gratis = entero_de(datos.get("cantidad_gratis"));
    let descuento = entero_de(datos.get("descuento_porcentaje"));
    match tipo.as_str() {
        "2x1" => {
            cantidad_requerida = Some(2);
            cantidad_gratis = Some(1);
        }
        "regalo" => {
            if cantidad_requerida.unwrap_or(0) < 1 || cantidad_gratis.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "La promoción de regalo requiere cantidades.",
                ));
            }
        }
        "descuento_cantidad" => {
            if cantidad_requerida.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "Indica la cantidad requerida.",
                ));
            }
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
        _ => {
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
    }
    let fecha_desde = fecha_corta(datos.get("fecha_desde"));
    let fecha_hasta = fecha_corta(datos.get("fecha_hasta"));
    if fecha_desde.is_empty() || fecha_hasta.is_empty() || fecha_hasta < fecha_desde {
        return Err(ApiError::dominio(
            "VIGENCIA_INVALIDA",
            "La vigencia de la promoción no es válida.",
        ));
    }
    Ok(PromocionNormalizada {
        nombre,
        tipo,
        item_id: None,
        sku: String::new(),
        cantidad_requerida,
        cantidad_gratis,
        descuento_porcentaje: descuento,
        fecha_desde,
        fecha_hasta,
        estado: estado_operativo(datos),
    })
}

pub(super) fn validar_cabecera_movimiento(datos: &Value) -> Result<CabeceraMovimiento, ApiError> {
    let tipo = datos
        .get("tipo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !TIPOS_MOVIMIENTO.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_MOVIMIENTO_INVALIDO",
            "El tipo de movimiento no es válido.",
        ));
    }
    let lineas = datos
        .get("lineas")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if lineas.is_empty() {
        return Err(ApiError::dominio(
            "LINEAS_REQUERIDAS",
            "El movimiento requiere al menos una línea.",
        ));
    }
    if lineas.len() > MAX_LINEAS {
        return Err(ApiError::dominio(
            "DEMASIADAS_LINEAS",
            format!("El máximo es {MAX_LINEAS} líneas."),
        ));
    }
    let origen = texto_opcional(datos, "ubicacion_origen").unwrap_or_default();
    let destino = texto_opcional(datos, "ubicacion_destino").unwrap_or_default();
    if tipo == "traslado" && (origen.is_empty() || destino.is_empty() || origen == destino) {
        return Err(ApiError::dominio(
            "TRASLADO_INVALIDO",
            "Un traslado requiere ubicación de origen y destino distintas.",
        ));
    }
    Ok(CabeceraMovimiento {
        tipo,
        origen,
        destino,
    })
}

pub(super) fn cantidad_componente(componente: &Value) -> Result<i64, ApiError> {
    entero_de(componente.get("cantidad"))
        .filter(|cantidad| *cantidad >= 1)
        .ok_or_else(|| {
            ApiError::dominio(
                "CANTIDAD_INVALIDA",
                "La cantidad del componente debe ser un entero positivo.",
            )
        })
}

pub(super) fn registrar_componente_unico(
    vistos: &mut Vec<String>,
    item_id: &str,
) -> Result<(), ApiError> {
    if vistos.iter().any(|visto| visto == item_id) {
        return Err(ApiError::dominio(
            "COMPONENTE_DUPLICADO",
            "Un producto no puede repetirse en el combo.",
        ));
    }
    vistos.push(item_id.to_string());
    Ok(())
}

pub(super) fn calcular_stock(tipo: &str, anterior: i64, cantidad: i64) -> i64 {
    match tipo {
        "entrada" => anterior.saturating_add(cantidad),
        "salida" => anterior.saturating_sub(cantidad),
        "ajuste" => cantidad,
        _ => anterior,
    }
}

/// `Math.round(((precio - costo) / precio) * 1000) / 10`, con el redondeo de
/// JavaScript (mitad hacia +∞) para conservar el `margen` del contrato.
pub(super) fn calcular_margen(precio_cents: i64, costo_total_cents: i64) -> f64 {
    if precio_cents <= 0 {
        return 0.0;
    }
    let proporcion = (precio_cents - costo_total_cents) as f64 / precio_cents as f64;
    redondear_js(proporcion * 1000.0) / 10.0
}

pub(super) fn redondear_js(valor: f64) -> f64 {
    if valor.is_sign_negative() {
        -((-valor + 0.5).floor())
    } else {
        (valor + 0.5).floor()
    }
}

pub(super) fn descuento_valido(descuento: Option<i64>) -> bool {
    matches!(descuento, Some(valor) if (1..=100).contains(&valor))
}

pub(super) fn normalizar_sku(valor: &str) -> Result<String, ApiError> {
    let sku = valor.trim().to_uppercase();
    let longitud = sku.chars().count();
    let valido = (2..=32).contains(&longitud)
        && sku.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "SKU_INVALIDO",
            "El SKU solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(sku)
}

pub(super) fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim().to_string();
    let longitud = nombre.chars().count();
    if !(2..=200).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(nombre)
}

pub(super) fn monto_de(valor: Option<&Value>) -> Result<Money, ApiError> {
    match valor {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        Some(otro) => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            format!(
                "Formato de monto invalido: \"{}\". Use numero con hasta 2 decimales.",
                otro
            ),
        )),
        None => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

pub(super) fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(super) fn texto_opcional(valor: &Value, clave: &str) -> Option<String> {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .map(|texto| texto.trim().to_string())
}

pub(super) fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(Value::Number(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

pub(super) fn entero_de(valor: Option<&Value>) -> Option<i64> {
    match valor? {
        Value::Number(numero) => numero
            .as_i64()
            .or_else(|| numero.as_f64().and_then(f64_a_entero)),
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                Some(0)
            } else {
                texto
                    .parse::<i64>()
                    .ok()
                    .or_else(|| texto.parse::<f64>().ok().and_then(f64_a_entero))
            }
        }
        _ => None,
    }
}

pub(super) fn f64_a_entero(valor: f64) -> Option<i64> {
    if valor.is_finite()
        && valor.fract() == 0.0
        && valor >= i64::MIN as f64
        && valor <= i64::MAX as f64
    {
        Some(valor as i64)
    } else {
        None
    }
}

pub(super) fn fecha_corta(valor: Option<&Value>) -> String {
    let texto = match valor {
        Some(Value::String(texto)) => texto.clone(),
        Some(Value::Null) | None => String::new(),
        Some(otro) => otro.to_string(),
    };
    texto.chars().take(10).collect()
}

pub(super) fn estado_operativo(datos: &Value) -> String {
    if datos.get("estado").and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

pub(super) fn estado_serializado(valor: Option<&Value>) -> String {
    if valor.and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

pub(super) fn actual_sku(actual: &Value) -> &str {
    actual.get("sku").and_then(Value::as_str).unwrap_or("")
}

pub(super) fn es_uuid(valor: &str) -> bool {
    Uuid::parse_str(valor).is_ok()
}

pub(super) fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => matches!(db.code().as_deref(), Some("23505") | Some("23P01")),
        _ => false,
    }
}
