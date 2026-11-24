use super::prelude::*;
use super::*;

pub(super) fn lineas_de_carrito(items: &[ItemCarrito]) -> Vec<LineaEntrada> {
    items
        .iter()
        .map(|item| LineaEntrada {
            oferta_id: item.oferta_id.clone(),
            cantidad: item.cantidad,
            fulfillment: Some("none".to_string()),
            fulfillment_config: None,
            ubicacion_id: None,
            impuesto_codigo: None,
        })
        .collect()
}

pub(super) fn descuento_de_promocion(
    promocion: Option<&PromocionLinea>,
    precio_unitario: i64,
    cantidad: i64,
) -> Result<i64, ApiError> {
    let Some(promocion) = promocion else {
        return Ok(0);
    };
    let bruto = money::multiplicar(precio_unitario, cantidad);
    let descuento = match promocion.tipo.as_str() {
        "descuento_porcentaje" => {
            let porcentaje = promocion.descuento_porcentaje.unwrap_or(0).clamp(0, 100);
            ((bruto as f64 * porcentaje as f64) / 100.0).round() as i64
        }
        "2x1" => {
            let gratis = cantidad / promocion.cantidad_requerida.unwrap_or(2).max(1);
            gratis.saturating_mul(precio_unitario)
        }
        "descuento_cantidad" | "regalo" => {
            let requerida = promocion.cantidad_requerida.unwrap_or(1).max(1);
            let gratis = promocion.cantidad_gratis.unwrap_or(0).max(0);
            (cantidad / requerida)
                .saturating_mul(gratis)
                .min(cantidad)
                .saturating_mul(precio_unitario)
        }
        _ => 0,
    };
    Ok(descuento.clamp(0, bruto))
}

pub(super) fn agrupar(lineas: &[LineaEntrada]) -> Result<Vec<LineaAgrupada>, ApiError> {
    let mut mapa: HashMap<String, LineaAgrupada> = HashMap::new();
    let mut orden: Vec<String> = Vec::new();
    for linea in lineas {
        let clave = linea.oferta_id.clone();
        let ubicacion = linea
            .ubicacion_id
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string);
        match mapa.get_mut(&clave) {
            Some(actual) => {
                if actual.fulfillment != linea.fulfillment {
                    return Err(ApiError::dominio(
                        "FULFILLMENT_CONFLICTO",
                        "Una misma oferta no puede tener dos fulfillments distintos en una orden.",
                    ));
                }
                if actual.ubicacion_id != ubicacion {
                    return Err(ApiError::dominio(
                        "UBICACION_CONFLICTO",
                        "Una misma oferta no puede despacharse desde dos ubicaciones en una orden.",
                    ));
                }
                let impuesto = linea
                    .impuesto_codigo
                    .as_deref()
                    .map(str::trim)
                    .filter(|valor| !valor.is_empty());
                if impuesto.is_some() && actual.impuesto_codigo.as_deref() != impuesto {
                    return Err(ApiError::dominio(
                        "IMPUESTO_CONFLICTO",
                        "Una misma oferta no puede usar dos códigos de impuesto en una orden.",
                    ));
                }
                actual.cantidad += linea.cantidad;
                if linea.fulfillment_config.is_some() {
                    actual.fulfillment_config = linea.fulfillment_config.clone();
                }
            }
            None => {
                orden.push(clave.clone());
                mapa.insert(
                    clave,
                    LineaAgrupada {
                        oferta_id: linea.oferta_id.clone(),
                        cantidad: linea.cantidad,
                        fulfillment: linea.fulfillment.clone(),
                        fulfillment_config: linea.fulfillment_config.clone(),
                        ubicacion_id: ubicacion,
                        impuesto_codigo: linea
                            .impuesto_codigo
                            .as_deref()
                            .map(str::trim)
                            .filter(|valor| !valor.is_empty())
                            .map(str::to_string),
                    },
                );
            }
        }
    }
    Ok(orden
        .into_iter()
        .filter_map(|clave| mapa.remove(&clave))
        .collect())
}

pub(super) fn validar_fulfillment(
    valor: Option<&str>,
    tipo_orden: &str,
    item_type: &str,
    permitidos: &[String],
) -> Result<String, ApiError> {
    let fulfillment = valor.unwrap_or("none");
    if !permitidos.iter().any(|permitido| permitido == fulfillment) {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "El fulfillment no es valido.",
        ));
    }
    if fulfillment == "delivery" && tipo_orden == "reserva" {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "Una reserva no puede usar delivery.",
        ));
    }
    if tipo_orden == "reserva" && !["servicio", "alquiler"].contains(&item_type) {
        return Err(ApiError::dominio(
            "TIPO_ITEM_INVALIDO",
            "Las reservas aplican a servicios o alquileres.",
        ));
    }
    if tipo_orden == "reserva" && fulfillment != "onsite" {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "Una reserva requiere fulfillment onsite.",
        ));
    }
    Ok(fulfillment.to_string())
}

pub(super) fn texto_config(config: &Map<String, Value>, clave: &str) -> Option<String> {
    config
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
}

pub(super) fn es_fecha(valor: &str) -> bool {
    regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").is_ok_and(|regex| regex.is_match(valor))
}

pub(super) fn url_entrega_digital(atributos: Option<&Value>) -> Option<String> {
    let valor = atributos?.get("entrega_digital_url")?.as_str()?.trim();
    if !(valor.starts_with("http://") || valor.starts_with("https://") || valor.starts_with('/'))
        || valor.len() > 2048
    {
        return None;
    }
    Some(valor.to_string())
}

pub(super) fn max_descargas_digital(atributos: Option<&Value>) -> Option<i64> {
    let valor = atributos?.get("entrega_digital_max_descargas")?;
    let numero = valor
        .as_i64()
        .or_else(|| valor.as_f64().map(|n| n as i64))?;
    (numero >= 1).then_some(numero)
}

pub(super) fn expira_digital(atributos: Option<&Value>) -> Option<chrono::DateTime<Utc>> {
    let valor = atributos?.get("entrega_digital_expira_dias")?;
    let dias = valor
        .as_i64()
        .or_else(|| valor.as_f64().map(|n| n as i64))?;
    (dias >= 1).then(|| Utc::now() + Duration::days(dias))
}

pub(super) fn resumen_fulfillment(items: &[ItemOrden]) -> Value {
    json!({
        "por_linea": items
            .iter()
            .map(|item| json!({
                "oferta_id": item.oferta_id,
                "fulfillment": item.fulfillment.clone().unwrap_or_else(|| "none".to_string()),
                "config": item.fulfillment_config.clone().unwrap_or_else(|| json!({})),
            }))
            .collect::<Vec<_>>(),
    })
}

pub(super) fn reconstruir_desde_eventos(
    eventos: &[crate::event_store::EventoDeOrden],
) -> Option<EstadoDerivado> {
    let primero = eventos.first()?;
    if primero.tipo != "OrderCreatedEvent" {
        return None;
    }
    let inicial = primero.payload.as_object()?;
    let tipo_orden = inicial.get("tipo_orden")?.as_str()?.to_string();
    let mut estado = EstadoDerivado {
        order_id: inicial
            .get("order_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        comprador_id: inicial
            .get("comprador_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        items: inicial
            .get("items")
            .cloned()
            .and_then(|valor| serde_json::from_value(valor).ok())
            .unwrap_or_default(),
        total_cents: inicial
            .get("total_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        estado: inicial
            .get("estado")
            .and_then(Value::as_str)
            .unwrap_or("creada")
            .to_string(),
        motivo: None,
        creado_en: inicial
            .get("creado_en")
            .and_then(Value::as_str)
            .unwrap_or(&primero.creado_en)
            .to_string(),
        actualizado_en: primero.creado_en.clone(),
        tipo_orden,
    };
    for evento in eventos.iter().skip(1) {
        if evento.tipo == "OrderStatusUpdatedEvent" {
            let Some(datos) = evento.payload.as_object() else {
                continue;
            };
            if let Some(nuevo) = datos.get("estado").and_then(Value::as_str) {
                estado.estado = nuevo.to_string();
                estado.motivo = datos
                    .get("motivo")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                estado.actualizado_en = evento.creado_en.clone();
            }
        }
    }
    Some(estado)
}

pub(super) fn normalizar_moneda(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = codigo.len() == 3 && codigo.chars().all(|c| c.is_ascii_uppercase());
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "moneda_pago", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

/// Estados en los que la venta ya se considera pagada y, por tanto, una
/// cancelación debe revertir finanzas y comisiones además de liberar stock.
pub(super) fn estado_pagado(estado: &str) -> bool {
    matches!(
        estado,
        "pagada" | "en_preparacion" | "enviada" | "entregada"
    )
}

/// Líneas que comprometieron stock. La entrega digital no reserva inventario.
pub(super) fn items_para_liberacion(items: &[ItemOrden]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| item.fulfillment.as_deref() != Some("digital"))
        .map(|item| {
            json!({
                "empresa_id": &item.empresa_id,
                "sku": &item.sku,
                "cantidad": item.cantidad,
                "item_id": &item.item_id,
                "ubicacion_id": &item.ubicacion_id,
            })
        })
        .collect()
}

/// Empresa del pago cuando todas las líneas pertenecen a la misma; si no,
/// `None` (orden multi-empresa).
pub(super) fn empresa_unica(items: &[ItemOrden]) -> Option<String> {
    let mut empresas = items.iter().map(|item| item.empresa_id.as_str());
    let primera = empresas.next()?;
    if empresas.all(|empresa| empresa == primera) {
        Some(primera.to_string())
    } else {
        None
    }
}

pub(super) fn empresas_de_items(items: &[ItemOrden]) -> Vec<String> {
    let mut empresas = Vec::new();
    for item in items {
        if !empresas.contains(&item.empresa_id) {
            empresas.push(item.empresa_id.clone());
        }
    }
    empresas
}

/// `true` si el fulfillment está permitido por el pack; lista vacía = sin
/// restricción (compatibilidad con packs antiguos).
pub(super) fn fulfillment_permitido(
    config: &crate::clients::ConfiguracionOperativa,
    fulfillment: Option<&str>,
) -> bool {
    if config.fulfillment.is_empty() {
        return true;
    }
    match fulfillment {
        Some(valor) => config
            .fulfillment
            .iter()
            .any(|permitido| permitido == valor),
        None => true,
    }
}
