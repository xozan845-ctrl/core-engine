use super::*;

// ── helpers puros ─────────────────────────────────────────────────────────

pub(super) fn variable(clave: &str, por_defecto: &str) -> String {
    env::var(clave)
        .ok()
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| por_defecto.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub(super) fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn texto_js(valor: &Value) -> String {
    match valor {
        Value::String(texto) => texto.clone(),
        Value::Number(numero) => numero.to_string(),
        Value::Bool(valor) => valor.to_string(),
        Value::Null => "null".to_string(),
        otro => otro.to_string(),
    }
}

pub(super) fn valor_verdadero(valor: &Value) -> bool {
    match valor {
        Value::Null => false,
        Value::Bool(valor) => *valor,
        Value::Number(numero) => numero.as_f64().map(|valor| valor != 0.0).unwrap_or(true),
        Value::String(texto) => !texto.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(super) fn entero_json(valor: &Value) -> Option<i64> {
    match valor {
        Value::Number(numero) => numero.as_i64().or_else(|| {
            numero
                .as_f64()
                .filter(|valor| valor.is_finite() && valor.fract() == 0.0)
                .map(|valor| valor as i64)
        }),
        _ => None,
    }
}

/// Los agregados `SUM(int)` llegan como `int8`; `fila_a_json` los representa
/// como string igual que node-postgres. El contrato de producto los expone
/// como número.
pub(super) fn entero_valor(valor: &Value) -> Option<i64> {
    match valor {
        Value::String(texto) => texto.trim().parse::<i64>().ok(),
        otro => entero_json(otro),
    }
}

pub(super) fn es_uuid(valor: &str) -> bool {
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

pub(super) fn monto_de_valor(valor: &Value) -> Result<Money, ApiError> {
    match valor {
        Value::String(texto) => Money::parsear(texto),
        Value::Number(numero) => Money::parsear(&numero.to_string()),
        Value::Bool(valor) => Money::parsear(if *valor { "true" } else { "false" }),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

pub(super) fn validar_precio(valor: Option<&Value>) -> Result<Money, ApiError> {
    let Some(valor) = valor else {
        return Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        ));
    };
    let precio = monto_de_valor(valor)?;
    if precio.centavos <= 0 {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio base debe ser mayor a 0.",
        ));
    }
    Ok(precio)
}

pub(super) fn validar_stock(valor: Option<&Value>) -> Result<i64, ApiError> {
    match valor.and_then(entero_json) {
        Some(stock) if stock >= 0 => Ok(stock),
        _ => Err(ApiError::dominio(
            "STOCK_INVALIDO",
            "El stock debe ser un entero mayor o igual a 0.",
        )),
    }
}

pub(super) fn normalizar_sku(valor: &str) -> Result<String, ApiError> {
    let sku = valor.trim().to_uppercase();
    let valido = (2..=32).contains(&sku.len())
        && sku.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "SKU_INVALIDO",
            "El SKU solo admite letras, numeros y guiones (2-32).",
        ));
    }
    Ok(sku)
}

pub(super) fn categoria_de(datos: &Value) -> String {
    match datos.get("categoria") {
        Some(Value::String(categoria)) => categoria.clone(),
        _ => "general".to_string(),
    }
}

pub(super) fn atributos_de(datos: &Value) -> Value {
    datos
        .get("atributos")
        .filter(|valor| !valor.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

pub(super) fn paginacion_de(filtros: &Value) -> crate::paginacion::Paginacion {
    let pagina = filtros.get("pagina").map(texto_js);
    let limite = filtros.get("limite").map(texto_js);
    parsear_paginacion(pagina.as_deref(), limite.as_deref())
}

pub(super) fn exigir_empresa(valor: Option<&Value>) -> Result<String, ApiError> {
    let empresa = valor.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if empresa.is_empty() {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "El catálogo se consulta por empresa (empresa_id).",
        ));
    }
    if empresa == EMPRESA_LEGACY_ID {
        return Err(ApiError::dominio(
            "EMPRESA_NO_PUBLICA",
            "La empresa comodín del catálogo legado no publica productos.",
        ));
    }
    Ok(empresa.to_string())
}

pub(super) fn imagen_de_valor(valor: &Value) -> ImagenInput {
    ImagenInput {
        objeto_id: valor
            .get("objeto_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        orden: valor.get("orden").and_then(entero_json),
        es_principal: valor
            .get("es_principal")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

pub(super) fn imagenes_de(valor: Option<&Value>) -> Vec<ImagenInput> {
    valor
        .and_then(Value::as_array)
        .map(|imagenes| imagenes.iter().map(imagen_de_valor).collect())
        .unwrap_or_default()
}

pub(super) fn variantes_de(datos: &Value) -> Vec<VarianteInput> {
    datos
        .get("variantes")
        .and_then(Value::as_array)
        .map(|variantes| {
            variantes
                .iter()
                .map(|variante| VarianteInput {
                    sku: texto(variante, "sku"),
                    nombre: variante
                        .get("nombre")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    atributos: atributos_de(variante),
                    precio_base: variante.get("precio_base").cloned(),
                    stock: variante.get("stock").cloned(),
                    imagenes: imagenes_de(variante.get("imagenes")),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn siguiente_orden(usados: &HashSet<i64>) -> i64 {
    for indice in 0..MAX_IMAGENES {
        if !usados.contains(&indice) {
            return indice;
        }
    }
    -1
}

/// Reparte una cantidad entre lotes ya ordenados por FEFO, ignorando los
/// vencidos. Devuelve las asignaciones o el faltante si no alcanza.
pub(super) fn normalizar_metadatos(datos: &Value) -> Result<Metadatos, ApiError> {
    let costo = match datos.get("costo") {
        Some(Value::Null) | None => Money::parsear("0")?,
        Some(valor) => monto_de_valor(valor)?,
    };
    let contenido_cantidad = metadato_entero(datos, "contenido_cantidad", 0)?;
    let stock_minimo = metadato_entero(datos, "stock_minimo", 0)?;
    let stock_maximo = metadato_entero_opcional(datos, "stock_maximo")?;
    let garantia_meses = metadato_entero_opcional(datos, "garantia_meses")?;
    if let Some(maximo) = stock_maximo {
        if maximo < stock_minimo {
            return Err(ApiError::dominio(
                "METADATO_INVALIDO",
                "stock_maximo no puede ser menor que stock_minimo.",
            ));
        }
    }
    Ok(Metadatos {
        costo_cents: costo.centavos,
        marca: texto(datos, "marca").trim().to_string(),
        codigo_barras: texto(datos, "codigo_barras").trim().to_string(),
        unidad: defecto(texto(datos, "unidad"), "Unidad (u)"),
        permite_fracciones: datos
            .get("permite_fracciones")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unidad_compra: defecto(texto(datos, "unidad_compra"), "Igual que la de venta"),
        contenido_cantidad,
        contenido_unidad: defecto(texto(datos, "contenido_unidad"), "Sin contenido"),
        tipo_precio: if texto(datos, "tipo_precio") == "escala" {
            "escala".to_string()
        } else {
            "fijo".to_string()
        },
        tipo_costo: if texto(datos, "tipo_costo") == "escala" {
            "escala".to_string()
        } else {
            "fijo".to_string()
        },
        stock_minimo,
        stock_maximo,
        garantia_meses,
        es_materia_prima: datos
            .get("es_materia_prima")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ubicacion: texto(datos, "ubicacion").trim().to_string(),
        estado_operativo: if texto(datos, "estado_operativo") == "inactivo" {
            "inactivo".to_string()
        } else {
            "activo".to_string()
        },
    })
}

pub(super) fn defecto(valor: String, por_defecto: &str) -> String {
    let valor = valor.trim().to_string();
    if valor.is_empty() {
        por_defecto.to_string()
    } else {
        valor
    }
}

pub(super) fn metadato_entero(
    datos: &Value,
    clave: &str,
    por_defecto: i64,
) -> Result<i64, ApiError> {
    match datos.get(clave) {
        Some(Value::Null) | None => Ok(por_defecto),
        Some(valor) => match entero_json(valor) {
            Some(numero) if numero >= 0 => Ok(numero),
            _ => Err(ApiError::dominio(
                "METADATO_INVALIDO",
                format!("{clave} debe ser un entero mayor o igual a 0."),
            )),
        },
    }
}

pub(super) fn metadato_entero_opcional(
    datos: &Value,
    clave: &str,
) -> Result<Option<i64>, ApiError> {
    match datos.get(clave) {
        Some(Value::Null) | None => Ok(None),
        Some(valor) => match entero_json(valor) {
            Some(numero) if numero >= 0 => Ok(Some(numero)),
            _ => Err(ApiError::dominio(
                "METADATO_INVALIDO",
                format!("{clave} debe ser un entero mayor o igual a 0 o null."),
            )),
        },
    }
}

pub(super) fn combinar_metadatos(previo: &Value, cambios: &Value) -> Result<Metadatos, ApiError> {
    let mut combinado = Map::new();
    for clave in [
        "costo",
        "marca",
        "codigo_barras",
        "unidad",
        "permite_fracciones",
        "unidad_compra",
        "contenido_cantidad",
        "contenido_unidad",
        "tipo_precio",
        "tipo_costo",
        "stock_minimo",
        "es_materia_prima",
        "ubicacion",
        "estado_operativo",
    ] {
        let valor = cambios
            .get(clave)
            .filter(|valor| !valor.is_null())
            .or_else(|| previo.get(clave))
            .cloned()
            .unwrap_or(Value::Null);
        combinado.insert(clave.to_string(), valor);
    }
    for clave in ["stock_maximo", "garantia_meses"] {
        let valor = if cambios.get(clave).is_some() {
            cambios.get(clave).cloned().unwrap_or(Value::Null)
        } else {
            previo.get(clave).cloned().unwrap_or(Value::Null)
        };
        combinado.insert(clave.to_string(), valor);
    }
    normalizar_metadatos(&Value::Object(combinado))
}

pub(super) fn agregar_set(
    sets: &mut Vec<String>,
    params: &mut Vec<Param>,
    columna: &str,
    parametro: Param,
) {
    params.push(parametro);
    sets.push(format!("{columna} = ${}", params.len()));
}

pub(super) async fn registrar_historico(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: &str,
    anterior: Option<i64>,
    nuevo: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO item.historico_precios
           (id, empresa_id, item_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
         VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3, $4, NOW())",
    )
    .bind(empresa_id)
    .bind(item_id)
    .bind(anterior)
    .bind(nuevo)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

/// Agrega stock y precio mínimo de las variantes al item padre.
pub(super) async fn recalcular_padre(
    tx: &mut Transaction<'_, Postgres>,
    padre_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE item.items p
            SET stock = agregado.stock,
                precio_cents = agregado.precio,
                actualizado_en = NOW()
           FROM (
             SELECT SUM(c.stock)::int AS stock, MIN(c.precio_cents)::int AS precio
               FROM item.items c
              WHERE c.parent_item_id = $1::uuid AND c.eliminado_en IS NULL
           ) agregado
          WHERE p.id = $1::uuid AND agregado.stock IS NOT NULL",
    )
    .bind(padre_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

pub(super) async fn insertar_imagenes(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: Uuid,
    imagenes: &[ImagenInput],
) -> Result<(), ApiError> {
    if imagenes.is_empty() {
        return Ok(());
    }
    let mut usados: HashSet<i64> = HashSet::new();
    let hay_principal = imagenes.iter().any(|imagen| imagen.es_principal);
    for imagen in imagenes {
        let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&usados));
        if !(0..MAX_IMAGENES).contains(&orden) || usados.contains(&orden) {
            return Err(ApiError::dominio(
                "IMAGEN_ORDEN_INVALIDO",
                "El orden de la imagen no es válido.",
            ));
        }
        usados.insert(orden);
        sqlx::query(
            "INSERT INTO item.item_imagenes
               (id, empresa_id, item_id, objeto_id, orden, es_principal)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(empresa_id)
        .bind(item_id)
        .bind(&imagen.objeto_id)
        .bind(orden)
        .bind(imagen.es_principal)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
    }
    if !hay_principal {
        promover_primera_imagen(tx, &item_id.to_string()).await?;
    }
    Ok(())
}

pub(super) async fn promover_primera_imagen(
    tx: &mut Transaction<'_, Postgres>,
    item_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE item.item_imagenes
            SET es_principal = true
          WHERE id = (
            SELECT id FROM item.item_imagenes
             WHERE item_id = $1::uuid ORDER BY orden LIMIT 1
          )",
    )
    .bind(item_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

pub(super) fn clave_linea(linea: &Linea) -> String {
    format!("{}:{}", linea.empresa_id, linea.sku.to_lowercase())
}

pub(super) fn fallido(linea: &Linea, motivo: &str) -> Value {
    json!({
        "empresa_id": linea.empresa_id,
        "sku": linea.sku,
        "cantidad": linea.cantidad,
        "motivo": motivo,
    })
}

pub(super) fn serializar(
    fila: &Value,
    imagenes: &[Value],
    variantes: Vec<Value>,
) -> Result<Value, ApiError> {
    let precio = Money::desde_centavos(
        fila.get("precio_base_cents")
            .and_then(entero_valor)
            .unwrap_or(0),
    )?;
    let costo = Money::desde_centavos(fila.get("costo_cents").and_then(entero_valor).unwrap_or(0))?;
    let precio_desde = Money::desde_centavos(
        fila.get("precio_desde_cents")
            .and_then(entero_valor)
            .unwrap_or_else(|| {
                fila.get("precio_base_cents")
                    .and_then(entero_valor)
                    .unwrap_or(0)
            }),
    )?;
    let stock = fila.get("stock").and_then(entero_valor).unwrap_or(0);
    let mut objeto = Map::new();
    objeto.insert("id".into(), Value::String(texto(fila, "id")));
    objeto.insert(
        "empresa_id".into(),
        Value::String(texto(fila, "empresa_id")),
    );
    objeto.insert("sku".into(), Value::String(texto(fila, "sku")));
    objeto.insert("nombre".into(), Value::String(texto(fila, "nombre")));
    objeto.insert(
        "descripcion".into(),
        Value::String(texto(fila, "descripcion")),
    );
    objeto.insert("categoria".into(), Value::String(texto(fila, "categoria")));
    objeto.insert("precio_base".into(), Value::String(precio.string()));
    objeto.insert("stock".into(), json!(stock));
    objeto.insert(
        "estado".into(),
        Value::String(if stock > 0 { "disponible" } else { "agotado" }.to_string()),
    );
    objeto.insert(
        "estado_operativo".into(),
        Value::String(
            if texto(fila, "estado") == "inactivo" {
                "inactivo"
            } else {
                "activo"
            }
            .to_string(),
        ),
    );
    objeto.insert("costo".into(), Value::String(costo.string()));
    objeto.insert("marca".into(), Value::String(texto(fila, "marca")));
    objeto.insert(
        "codigo_barras".into(),
        Value::String(texto(fila, "codigo_barras")),
    );
    objeto.insert(
        "unidad".into(),
        Value::String(defecto(texto(fila, "unidad"), "Unidad (u)")),
    );
    objeto.insert(
        "permite_fracciones".into(),
        Value::Bool(
            fila.get("permite_fracciones")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "unidad_compra".into(),
        Value::String(defecto(
            texto(fila, "unidad_compra"),
            "Igual que la de venta",
        )),
    );
    objeto.insert(
        "contenido_cantidad".into(),
        json!(fila
            .get("contenido_cantidad")
            .and_then(entero_valor)
            .unwrap_or(0)),
    );
    objeto.insert(
        "contenido_unidad".into(),
        Value::String(defecto(texto(fila, "contenido_unidad"), "Sin contenido")),
    );
    objeto.insert(
        "tipo_precio".into(),
        Value::String(
            if texto(fila, "tipo_precio") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "tipo_costo".into(),
        Value::String(
            if texto(fila, "tipo_costo") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "stock_minimo".into(),
        json!(fila.get("stock_minimo").and_then(entero_valor).unwrap_or(0)),
    );
    objeto.insert(
        "stock_maximo".into(),
        fila.get("stock_maximo").cloned().unwrap_or(Value::Null),
    );
    objeto.insert(
        "garantia_meses".into(),
        fila.get("garantia_meses").cloned().unwrap_or(Value::Null),
    );
    objeto.insert(
        "es_materia_prima".into(),
        Value::Bool(
            fila.get("es_materia_prima")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert("ubicacion".into(), Value::String(texto(fila, "ubicacion")));
    objeto.insert(
        "actualizado_en".into(),
        Value::String(if texto(fila, "actualizado_en").is_empty() {
            texto(fila, "creado_en")
        } else {
            texto(fila, "actualizado_en")
        }),
    );
    objeto.insert("creado_en".into(), Value::String(texto(fila, "creado_en")));
    objeto.insert(
        "producto_padre_id".into(),
        fila.get("producto_padre_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    objeto.insert(
        "atributos".into(),
        fila.get("atributos")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    objeto.insert(
        "tiene_variantes".into(),
        Value::Bool(
            fila.get("tiene_variantes")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || !variantes.is_empty(),
        ),
    );
    objeto.insert("imagenes".into(), Value::Array(imagenes.to_vec()));
    if !variantes.is_empty() {
        objeto.insert("variantes".into(), Value::Array(variantes));
    }
    objeto.insert("precio_desde".into(), Value::String(precio_desde.string()));
    objeto.insert(
        "stock_total".into(),
        json!(fila
            .get("stock_total")
            .and_then(entero_valor)
            .unwrap_or(stock)),
    );
    Ok(Value::Object(objeto))
}

pub(super) fn serializar_imagen(fila: &Value) -> Value {
    json!({
        "id": texto(fila, "id"),
        "objeto_id": texto(fila, "objeto_id"),
        "url": format!("{URL_IMAGEN_PUBLICA}/{}", texto(fila, "objeto_id")),
        "orden": fila.get("orden").and_then(Value::as_i64).unwrap_or(0),
        "es_principal": fila.get("es_principal").and_then(Value::as_bool).unwrap_or(false),
    })
}
