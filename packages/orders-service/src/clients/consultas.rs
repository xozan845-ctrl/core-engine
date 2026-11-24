use super::prelude::*;
use super::*;

impl Clientes {
    /// `None` = identity no disponible (fail-closed en el llamador).
    pub async fn capacidades(
        &self,
        empresas: &[String],
        negocio_id: Option<&str>,
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>> {
        if empresas.is_empty() || capacidades.is_empty() {
            return Some(Vec::new());
        }
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/empresas/capacidades",
                self.identity_url
            ))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_ids": empresas,
                "capacidades": capacidades,
                "negocio_id": negocio_id,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo = respuesta.json::<RespuestaCapacidades>().await.ok()?;
        Some(cuerpo.capacidades)
    }

    pub async fn tipo_cambio(&self, base: &str, quote: &str) -> Option<f64> {
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/finance/tipos-cambio",
                self.finance_url
            ))
            .query(&[("base", base), ("quote", quote)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        let tasa = cuerpo.get("tasa")?;
        tasa.as_str()
            .and_then(|valor| valor.parse::<f64>().ok())
            .or_else(|| tasa.as_f64())
    }

    pub async fn configuracion_fiscal(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Option<ConfiguracionFiscal> {
        let mut peticion = self
            .http
            .get(format!(
                "{}/internal/finance/configuracion-fiscal",
                self.finance_url
            ))
            .query(&[("empresa_id", empresa_id)])
            .header("x-internal-key", &self.clave);
        if let Some(negocio_id) = negocio_id {
            peticion = peticion.query(&[("negocio_id", negocio_id)]);
        }
        let respuesta = peticion.send().await.ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        let tasas = cuerpo
            .get("tasas")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(TasaImpuestoExterna {
                            codigo: item.get("codigo")?.as_str()?.to_string(),
                            nombre: item
                                .get("nombre")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            tasa: item.get("tasa")?.as_f64()?,
                            exenta: item.get("exenta").and_then(Value::as_bool).unwrap_or(false),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Some(ConfiguracionFiscal {
            tasa_iva: cuerpo.get("tasa_iva").and_then(Value::as_f64)?,
            impuesto_activo: cuerpo
                .get("impuesto_activo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            precios_con_impuesto: cuerpo
                .get("precios_con_impuesto")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            moneda: cuerpo
                .get("moneda")
                .and_then(Value::as_str)
                .unwrap_or("NIO")
                .to_string(),
            moneda_iso: cuerpo
                .get("moneda_iso")
                .and_then(Value::as_str)
                .unwrap_or("NIO")
                .to_string(),
            tasas,
            tasa_default_codigo: cuerpo
                .get("tasa_default_codigo")
                .and_then(Value::as_str)
                .unwrap_or("GENERAL")
                .to_string(),
            retencion_activa: cuerpo
                .get("retencion_activa")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            tasa_retencion: cuerpo
                .get("tasa_retencion")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            retencion_codigo: cuerpo
                .get("retencion_codigo")
                .and_then(Value::as_str)
                .unwrap_or("IR")
                .to_string(),
        })
    }

    pub async fn configuracion(&self, empresa_id: &str) -> ConfiguracionExterna {
        let respuesta = match self
            .http
            .get(format!(
                "{}/internal/empresas/{empresa_id}/configuracion",
                self.identity_url
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
        {
            Ok(respuesta) => respuesta,
            Err(_) => return ConfiguracionExterna::Error,
        };
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return ConfiguracionExterna::SinConfiguracion;
        }
        if !respuesta.status().is_success() {
            return ConfiguracionExterna::Error;
        }
        match respuesta.json::<ConfiguracionOperativa>().await {
            Ok(config) => ConfiguracionExterna::Ok(config),
            Err(_) => ConfiguracionExterna::Error,
        }
    }

    pub async fn ofertas(&self, ids: &[String]) -> Vec<OfertaExterna> {
        if ids.is_empty() {
            return Vec::new();
        }
        let unicos: Vec<&String> = {
            let mut vistos = std::collections::HashSet::new();
            ids.iter().filter(|id| vistos.insert(*id)).collect()
        };
        let unidos = unicos
            .iter()
            .map(|id| urlencoding::encode(id).into_owned())
            .collect::<Vec<_>>()
            .join(",");
        let Some(respuesta) = self
            .http
            .get(format!("{}/internal/ofertas?ids={unidos}", self.stores_url))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()
        else {
            return Vec::new();
        };
        if !respuesta.status().is_success() {
            return Vec::new();
        }
        respuesta
            .json::<Vec<OfertaExterna>>()
            .await
            .unwrap_or_default()
    }

    pub async fn precios(
        &self,
        empresa_id: &str,
        lineas: Vec<Value>,
        lista_codigo: Option<&str>,
        segmento_codigo: Option<&str>,
    ) -> PreciosExternos {
        if lineas.is_empty() {
            return PreciosExternos::Ok(Vec::new());
        }
        let mut cuerpo = json!({ "empresa_id": empresa_id, "lineas": lineas });
        if let Some(lista) = lista_codigo {
            cuerpo["lista_codigo"] = json!(lista);
        }
        if let Some(segmento) = segmento_codigo {
            cuerpo["segmento_codigo"] = json!(segmento);
        }
        let Some(respuesta) = self
            .http
            .post(format!("{}/internal/items/precios", self.pricing_url))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&cuerpo)
            .send()
            .await
            .ok()
        else {
            return PreciosExternos::Error;
        };
        let status = respuesta.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return PreciosExternos::NoEncontrado;
        }
        if !status.is_success() {
            return PreciosExternos::Error;
        }
        respuesta
            .json::<RespuestaPrecios>()
            .await
            .map(|cuerpo| PreciosExternos::Ok(cuerpo.precios))
            .unwrap_or(PreciosExternos::Error)
    }

    pub async fn cotizacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        desde: &str,
        hasta: &str,
    ) -> Option<Cotizacion> {
        let query = url::form_urlencoded_pair(&[
            ("empresa_id", empresa_id),
            ("desde", desde),
            ("hasta", hasta),
        ]);
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/items/{}/cotizacion?{query}",
                self.pricing_url,
                urlencoding::encode(item_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<Cotizacion>().await.ok()
    }

    /// `None` = scheduling no disponible.
    pub async fn disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
        inicia_en: &str,
        termina_en: &str,
    ) -> Option<bool> {
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/scheduling/disponibilidad",
                self.scheduling_url
            ))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "recurso_id": recurso_id,
                "inicia_en": inicia_en,
                "termina_en": termina_en,
            }))
            .send()
            .await
            .ok()?;
        if respuesta.status() == reqwest::StatusCode::CONFLICT {
            return Some(false);
        }
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo = respuesta.json::<RespuestaDisponibilidad>().await.ok()?;
        Some(cuerpo.disponible)
    }

    pub async fn items_lote(&self, skus: &[String]) -> Option<Vec<ItemStock>> {
        if skus.is_empty() {
            return Some(Vec::new());
        }
        let unidos = skus
            .iter()
            .map(|sku| urlencoding::encode(sku).into_owned())
            .collect::<Vec<_>>()
            .join(",");
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/v2/items/lote?skus={unidos}",
                self.item_url
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let items = respuesta.json::<Vec<ItemV2Stock>>().await.ok()?;

        let mut empresas: Vec<String> = Vec::new();
        for item in &items {
            if !empresas.iter().any(|empresa| empresa == &item.empresa_id) {
                empresas.push(item.empresa_id.clone());
            }
        }
        let mut stock_por_empresa: Vec<Vec<StockV2>> = Vec::new();
        for empresa in &empresas {
            let codigos: Vec<String> = items
                .iter()
                .filter(|item| &item.empresa_id == empresa)
                .map(|item| item.sku.clone())
                .collect();
            let respuesta = self
                .http
                .post(format!("{}/internal/inventario/stock", self.inventory_url))
                .header("x-internal-key", &self.clave)
                .json(&json!({ "empresa_id": empresa, "skus": codigos }))
                .send()
                .await
                .ok()?;
            if !respuesta.status().is_success() {
                return None;
            }
            stock_por_empresa.push(respuesta.json::<Vec<StockV2>>().await.ok()?);
        }

        let mut salida = Vec::with_capacity(items.len());
        for item in items {
            let indice = empresas
                .iter()
                .position(|empresa| empresa == &item.empresa_id)?;
            let stock = stock_por_empresa
                .get(indice)?
                .iter()
                .find(|linea| linea.sku.eq_ignore_ascii_case(&item.sku));
            salida.push(ItemStock {
                empresa_id: item.empresa_id,
                sku: item.sku,
                stock: stock.map(|linea| linea.stock).unwrap_or(0),
                track_serial: item.track_serial,
                seriales_disponibles: stock.map(|linea| linea.seriales_disponibles).unwrap_or(0),
                estado: item.estado,
            });
        }
        Some(salida)
    }

    pub async fn stock_ubicacion(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
        skus: &[String],
    ) -> Option<Vec<StockUbicacion>> {
        if skus.is_empty() {
            return Some(Vec::new());
        }
        let respuesta = self
            .http
            .post(format!("{}/internal/inventario/stock", self.inventory_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "ubicacion_id": ubicacion_id,
                "skus": skus,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<Vec<StockUbicacion>>().await.ok()
    }
}
