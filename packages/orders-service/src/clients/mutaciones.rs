use super::prelude::*;
use super::*;

impl Clientes {
    pub async fn registrar_credito(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        monto_cents: i64,
        concepto: &str,
        vendedor_id: Option<&str>,
    ) -> CreditoEstado {
        let mut cuerpo = json!({
            "empresa_id": empresa_id,
            "parte_id": parte_id,
            "order_id": order_id,
            "monto_cents": monto_cents,
            "concepto": concepto,
        });
        if let Some(vendedor) = vendedor_id {
            cuerpo["vendedor_id"] = json!(vendedor);
        }
        let Some(respuesta) = self
            .http
            .post(format!("{}/internal/finance/creditos", self.finance_url))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&cuerpo)
            .send()
            .await
            .ok()
        else {
            return CreditoEstado::Error;
        };
        if respuesta.status() == reqwest::StatusCode::CONFLICT {
            return CreditoEstado::Rechazado;
        }
        if !respuesta.status().is_success() {
            return CreditoEstado::Error;
        }
        CreditoEstado::Ok
    }

    pub async fn anular_credito(&self, order_id: &str) -> bool {
        let Some(respuesta) = self
            .http
            .delete(format!(
                "{}/internal/finance/creditos/orden/{}",
                self.finance_url,
                urlencoding::encode(order_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()
        else {
            return false;
        };
        respuesta.status().is_success()
    }

    pub async fn cupon_validar(
        &self,
        empresa_id: &str,
        codigo: &str,
        monto_cents: i64,
    ) -> CuponExterno {
        let respuesta = match self
            .http
            .post(format!("{}/internal/cupones/validar", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "monto_cents": monto_cents,
            }))
            .send()
            .await
        {
            Ok(respuesta) => respuesta,
            Err(_) => return CuponExterno::NoDisponible,
        };
        match respuesta.status().as_u16() {
            200..=299 => match respuesta.json::<CuponValidado>().await {
                Ok(cupon) => CuponExterno::Ok(cupon),
                Err(_) => CuponExterno::NoDisponible,
            },
            400..=499 => {
                let cuerpo = respuesta.json::<Value>().await.unwrap_or(Value::Null);
                CuponExterno::Invalido {
                    codigo: cuerpo
                        .get("codigo")
                        .and_then(Value::as_str)
                        .unwrap_or("CUPON_INVALIDO")
                        .to_string(),
                    mensaje: cuerpo
                        .get("mensaje")
                        .and_then(Value::as_str)
                        .unwrap_or("El cupón no es válido.")
                        .to_string(),
                }
            }
            _ => CuponExterno::NoDisponible,
        }
    }

    pub async fn cupon_canjear(
        &self,
        empresa_id: &str,
        codigo: &str,
        order_id: &str,
        monto_cents: i64,
    ) -> Option<CuponValidado> {
        let respuesta = self
            .http
            .post(format!("{}/internal/cupones/canjear", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "order_id": order_id,
                "monto_cents": monto_cents,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<CuponValidado>().await.ok()
    }

    pub async fn cupon_liberar(&self, empresa_id: &str, codigo: &str, order_id: &str) {
        let _ = self
            .http
            .post(format!("{}/internal/cupones/liberar", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "order_id": order_id,
            }))
            .send()
            .await;
    }

    pub async fn devolver_stock(
        &self,
        order_id: &str,
        devolucion_id: &str,
        items: &[Value],
    ) -> bool {
        self.http
            .post(format!(
                "{}/internal/inventario/devolucion",
                self.inventory_url
            ))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "order_id": order_id,
                "devolucion_id": devolucion_id,
                "items": items,
            }))
            .send()
            .await
            .map(|respuesta| respuesta.status().is_success())
            .unwrap_or(false)
    }

    pub async fn nota_credito(&self, payload: &Value) -> Option<String> {
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/finance/notas-credito",
                self.finance_url
            ))
            .header("x-internal-key", &self.clave)
            .json(payload)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        cuerpo
            .get("comprobante_id")
            .or_else(|| cuerpo.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}
