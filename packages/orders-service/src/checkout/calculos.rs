use super::comunes::*;
use super::prelude::*;
use super::*;

impl Checkout {
    pub(super) async fn resolver_precios(
        &self,
        lineas: &[LineaAgrupada],
        por_id: &HashMap<String, OfertaExterna>,
        comando: &ComandoCrear,
    ) -> Result<HashMap<String, PrecioResuelto>, ApiError> {
        let mut resultado = HashMap::new();
        let mut por_empresa: HashMap<String, Vec<Value>> = HashMap::new();
        for linea in lineas {
            let Some(oferta) = por_id.get(&linea.oferta_id) else {
                continue;
            };
            if oferta.item_type == "alquiler" {
                continue;
            }
            por_empresa
                .entry(oferta.empresa_id.clone())
                .or_default()
                .push(json!({
                    "clave": oferta.id,
                    "item_id": oferta.item_id,
                    "cantidad": linea.cantidad,
                }));
        }

        for (empresa_id, grupo) in por_empresa {
            let respuesta = self
                .clientes
                .precios(
                    &empresa_id,
                    grupo,
                    comando.lista_precio_codigo.as_deref(),
                    comando.segmento_codigo.as_deref(),
                )
                .await;
            match respuesta {
                PreciosExternos::NoEncontrado => {
                    if comando.lista_precio_codigo.is_some() || comando.segmento_codigo.is_some() {
                        let id = comando
                            .lista_precio_codigo
                            .as_deref()
                            .or(comando.segmento_codigo.as_deref())
                            .unwrap_or("");
                        return Err(ApiError::no_encontrado("Lista de precios", id));
                    }
                }
                PreciosExternos::Error => {
                    if comando.lista_precio_codigo.is_some() || comando.segmento_codigo.is_some() {
                        return Err(ApiError::servicio_no_disponible("pricing-service"));
                    }
                }
                PreciosExternos::Ok(precios) => {
                    for precio in precios {
                        resultado.insert(
                            precio.clave,
                            PrecioResuelto {
                                origen: precio.origen,
                                precio_unitario_cents: precio.precio_unitario_cents,
                                promocion: precio.promocion,
                            },
                        );
                    }
                }
            }
        }
        Ok(resultado)
    }

    pub(super) async fn validar_capacidades(
        &self,
        items: &[ItemOrden],
        tipo_orden: &str,
        metodo_pago: &str,
    ) -> Result<HashMap<String, ConfiguracionOperativa>, ApiError> {
        let mut grupos: HashMap<(String, Option<String>), Vec<&ItemOrden>> = HashMap::new();
        for item in items {
            grupos
                .entry((item.empresa_id.clone(), item.negocio_id.clone()))
                .or_default()
                .push(item);
        }
        let empresas: Vec<String> = grupos
            .keys()
            .map(|(empresa, _)| empresa.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for ((empresa, negocio), grupo) in grupos {
            let mut requeridas: HashSet<&str> = HashSet::new();
            if tipo_orden == "cotizacion" {
                requeridas.insert("cotizaciones");
            }
            if metodo_pago == "credito" {
                requeridas.insert("ventas_credito");
            }
            for item in &grupo {
                match item.fulfillment.as_deref() {
                    Some("delivery") => {
                        requeridas.insert("fulfillment_delivery");
                    }
                    Some("pickup") => {
                        requeridas.insert("fulfillment_pickup");
                    }
                    Some("onsite") => {
                        requeridas.insert("fulfillment_onsite");
                    }
                    Some("digital") => {
                        requeridas.insert("fulfillment_digital");
                    }
                    _ => {}
                }
            }
            if requeridas.is_empty() {
                continue;
            }
            let capacidades: Vec<String> = requeridas.iter().map(|c| c.to_string()).collect();
            let filas = self
                .clientes
                .capacidades(
                    std::slice::from_ref(&empresa),
                    negocio.as_deref(),
                    &capacidades,
                )
                .await
                .ok_or_else(|| ApiError::servicio_no_disponible("identity-service"))?;
            let activas: HashSet<&str> = filas
                .iter()
                .filter(|fila| fila.activo && fila.empresa_id == empresa)
                .map(|fila| fila.capacidad_codigo.as_str())
                .collect();
            let faltantes: Vec<String> = capacidades
                .iter()
                .filter(|capacidad| !activas.contains(capacidad.as_str()))
                .cloned()
                .collect();
            if !faltantes.is_empty() {
                return Err(ApiError::capacidad_no_activa(&faltantes, &empresa));
            }
        }
        let mut configuraciones = HashMap::new();
        for empresa in &empresas {
            let config = match self.clientes.configuracion(empresa).await {
                ConfiguracionExterna::Ok(config) => config,
                ConfiguracionExterna::SinConfiguracion => continue,
                ConfiguracionExterna::Error => {
                    return Err(ApiError::servicio_no_disponible("identity-service"))
                }
            };
            if !config.fulfillment.is_empty() {
                for item in items.iter().filter(|item| &item.empresa_id == empresa) {
                    let fulfillment = item.fulfillment.as_deref();
                    if !fulfillment_permitido(&config, fulfillment) {
                        return Err(ApiError::dominio(
                            "FULFILLMENT_NO_PERMITIDO",
                            format!(
                                "El modelo operativo de la empresa no permite fulfillment \"{}\".",
                                fulfillment.unwrap_or("none")
                            ),
                        ));
                    }
                }
            }
            configuraciones.insert(empresa.clone(), config);
        }
        Ok(configuraciones)
    }

    /// Aplica IVA por línea con la configuración fiscal efectiva del tenant.
    /// Devuelve `(total de la orden, impuesto total)`. El descuento de cupón se
    /// reparte proporcionalmente para que el desglose por línea sea coherente.
    pub(super) async fn calcular_impuestos(
        &self,
        items: &mut [ItemOrden],
        descuento_cupon: i64,
        total_bruto: i64,
    ) -> Result<(i64, i64), ApiError> {
        let mut configs: HashMap<(String, Option<String>), Option<ConfiguracionFiscal>> =
            HashMap::new();
        for item in items.iter() {
            let clave = (item.empresa_id.clone(), item.negocio_id.clone());
            if let std::collections::hash_map::Entry::Vacant(entrada) = configs.entry(clave) {
                let config = self
                    .clientes
                    .configuracion_fiscal(&item.empresa_id, item.negocio_id.as_deref())
                    .await;
                entrada.insert(config);
            }
        }

        let ultimo = items.len().saturating_sub(1);
        let mut cupon_restante = descuento_cupon.max(0);
        let mut impuesto_total = 0i64;
        let mut total = 0i64;
        for (indice, item) in items.iter_mut().enumerate() {
            let neto = item
                .precio_unitario_cents
                .saturating_mul(item.cantidad)
                .saturating_sub(item.descuento_cents)
                .max(0);
            let cupon_linea = if total_bruto > 0 && cupon_restante > 0 {
                let calculado = if indice == ultimo {
                    cupon_restante
                } else {
                    ((descuento_cupon as f64 * neto as f64) / total_bruto as f64).round() as i64
                };
                calculado.clamp(0, neto.min(cupon_restante))
            } else {
                0
            };
            cupon_restante = (cupon_restante - cupon_linea).max(0);
            let neto_final = (neto - cupon_linea).max(0);
            let config = configs
                .get(&(item.empresa_id.clone(), item.negocio_id.clone()))
                .and_then(|config| config.as_ref());
            let (tasa, exenta) = match config {
                Some(config) if config.impuesto_activo => config
                    .tasa(item.impuesto_codigo.as_deref())
                    .ok_or_else(|| {
                        ApiError::dominio(
                            "IMPUESTO_NO_DISPONIBLE",
                            format!(
                                "La región no tiene configurada la tasa {}.",
                                item.impuesto_codigo
                                    .as_deref()
                                    .unwrap_or(config.tasa_default_codigo.as_str())
                            ),
                        )
                    })?,
                Some(_) | None => (0.0, false),
            };
            let activo = config
                .map(|config| config.impuesto_activo && !exenta && tasa > 0.0)
                .unwrap_or(false);
            let tasa_bps = (tasa * 10_000.0).round().max(0.0) as i64;
            let (impuesto, total_linea) = if activo {
                if config
                    .map(|config| config.precios_con_impuesto)
                    .unwrap_or(true)
                {
                    let base = if tasa > 0.0 {
                        ((neto_final as f64) / (1.0 + tasa)).round().max(0.0) as i64
                    } else {
                        neto_final
                    };
                    ((neto_final - base).max(0), neto_final)
                } else {
                    let impuesto = ((neto_final as f64) * tasa).round().max(0.0) as i64;
                    (impuesto, neto_final + impuesto)
                }
            } else {
                (0, neto_final)
            };
            item.impuesto_cents = impuesto;
            item.tasa_impuesto_bps = if activo { tasa_bps } else { 0 };
            item.total_linea_cents = total_linea;
            impuesto_total = impuesto_total.saturating_add(impuesto);
            total = total.saturating_add(total_linea);
        }
        Ok((total, impuesto_total))
    }

    pub(super) async fn verificar_stock(&self, items: &[ItemOrden]) -> Result<(), ApiError> {
        let productos: Vec<&ItemOrden> = items
            .iter()
            .filter(|item| {
                item.item_type == "producto" && item.fulfillment.as_deref() != Some("digital")
            })
            .collect();
        if productos.is_empty() {
            return Ok(());
        }
        let skus: Vec<String> = {
            let mut vistos = HashSet::new();
            productos
                .iter()
                .filter(|item| vistos.insert(item.sku.to_lowercase()))
                .map(|item| item.sku.clone())
                .collect()
        };
        let Some(productos_catalogo) = self.clientes.items_lote(&skus).await else {
            return Ok(());
        };
        let por_clave: HashMap<String, &crate::clients::ItemStock> = productos_catalogo
            .iter()
            .map(|p| (format!("{}:{}", p.empresa_id, p.sku.to_lowercase()), p))
            .collect();
        let mut stock_por_ubicacion: HashMap<
            (String, String),
            Vec<crate::clients::StockUbicacion>,
        > = HashMap::new();
        for item in productos {
            let producto =
                por_clave.get(&format!("{}:{}", item.empresa_id, item.sku.to_lowercase()));
            let disponible = if let Some(ubicacion_id) = item.ubicacion_id.as_deref() {
                let clave = (item.empresa_id.clone(), ubicacion_id.to_string());
                if !stock_por_ubicacion.contains_key(&clave) {
                    let Some(stocks) = self
                        .clientes
                        .stock_ubicacion(
                            &item.empresa_id,
                            ubicacion_id,
                            std::slice::from_ref(&item.sku),
                        )
                        .await
                    else {
                        return Err(ApiError::servicio_no_disponible("inventory-service"));
                    };
                    stock_por_ubicacion.insert(clave.clone(), stocks);
                }
                let linea = stock_por_ubicacion.get(&clave).and_then(|lineas| {
                    lineas
                        .iter()
                        .find(|linea| linea.sku.eq_ignore_ascii_case(&item.sku))
                });
                let stock = linea.map(|linea| linea.stock).unwrap_or(0);
                let seriales = linea.map(|linea| linea.seriales_disponibles).unwrap_or(0);
                match producto {
                    Some(p) if p.track_serial => stock.min(seriales),
                    _ => stock,
                }
            } else {
                match producto {
                    Some(p) if p.track_serial => p.stock.min(p.seriales_disponibles),
                    Some(p) => p.stock,
                    None => 0,
                }
            };
            if producto.is_none() || disponible < item.cantidad {
                return Err(ApiError::nueva(
                    "CONFLICTO",
                    "Stock insuficiente (RN-03): se rechaza la orden.",
                    axum::http::StatusCode::CONFLICT,
                )
                .con_detalles(json!({
                    "sku": item.sku,
                    "cantidad_solicitada": item.cantidad,
                    "ubicacion_id": item.ubicacion_id,
                })));
            }
        }
        Ok(())
    }
}
