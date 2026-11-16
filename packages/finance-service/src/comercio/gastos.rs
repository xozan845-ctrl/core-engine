use super::helpers::*;
use super::*;

impl Comercio {
    pub async fn crear_gasto(
        &self,
        dto: Gasto,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_texto(&dto.category, "category", 1)?;
        validar_in(&dto.tipo, "type", &TIPOS_GASTO)?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        self.crear("gastos", json!(dto), usuario).await
    }

    pub async fn crear_cuenta_por_pagar(
        &self,
        dto: CuentaPorPagar,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.supplier, "supplier", 1)?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_fecha(&dto.due_date, "dueDate")?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        self.crear("cuentas-por-pagar", json!(dto), usuario).await
    }

    // ── proveedores y compras ──────────────────────────────────────────────

    pub async fn crear_proveedor(
        &self,
        dto: Proveedor,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.name, "name", 1)?;
        if let Some(email) = &dto.email {
            validar_email(email, "email")?;
        }
        self.crear("proveedores", json!(dto), usuario).await
    }

    pub async fn crear_compra(
        &self,
        dto: Compra,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        validar_in(&dto.status, "status", &ESTADOS_COMPRA)?;
        validar_in(&dto.origin, "origin", &ORIGENES_COMPRA)?;
        validar_in(&dto.cost_type, "costType", &TIPOS_COSTO)?;
        if dto.lines.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lines", "restricciones": ["arrayMinSize"]}
            ])));
        }
        validar_lineas(&dto.lines)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": dto.status,
                "origin": dto.origin,
                "costType": dto.cost_type,
                "lines": lineas_con_id(&dto.lines),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": dto.retention_cents.unwrap_or(0),
                "userName": dto.user_name.unwrap_or_default(),
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_compra_directa(
        &self,
        dto: CompraDirecta,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.concept, "concept", 1)?;
        validar_in(&dto.cost_type, "costType", &TIPOS_COSTO)?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        let linea = LineaCompra {
            product_id: None,
            name: dto.concept,
            sku: Some(String::new()),
            quantity: 1.0,
            unit_cost_cents: dto.amount_cents,
            sale_price_cents: None,
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": "recibida",
                "origin": "directa",
                "costType": dto.cost_type,
                "lines": lineas_con_id(&[linea]),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": dto.retention_cents.unwrap_or(0),
                "userName": "",
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_compra_producto_nuevo(
        &self,
        dto: CompraProductoNuevo,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.product_name, "productName", 1)?;
        validar_in(&dto.payment_method, "paymentMethod", &METODOS_PAGO)?;
        validar_rango(dto.unit_cost_cents as f64, "unitCostCents", 0.0, f64::MAX)?;
        validar_rango(dto.sale_price_cents as f64, "salePriceCents", 0.0, f64::MAX)?;
        validar_rango(dto.quantity, "quantity", 1.0, f64::MAX)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => self.siguiente_numero("compras", "C", usuario).await?,
        };
        let linea = LineaCompra {
            product_id: None,
            name: dto.product_name,
            sku: dto.sku,
            quantity: dto.quantity,
            unit_cost_cents: dto.unit_cost_cents,
            sale_price_cents: Some(dto.sale_price_cents),
        };
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "paymentMethod": dto.payment_method,
                "status": "recibida",
                "origin": "productos",
                "costType": "inventario",
                "lines": lineas_con_id(&[linea]),
                "notes": dto.notes.unwrap_or_default(),
                "retentionCents": 0,
                "userName": "",
            }),
            usuario,
        )
        .await
    }

    pub async fn crear_orden_compra(
        &self,
        dto: OrdenCompra,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        if dto.lines.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lines", "restricciones": ["arrayMinSize"]}
            ])));
        }
        validar_lineas(&dto.lines)?;
        let numero = match dto.number {
            Some(numero) => numero,
            None => {
                self.siguiente_numero("ordenes-compra", "OC", usuario)
                    .await?
            }
        };
        self.crear(
            "ordenes-compra",
            json!({
                "number": numero,
                "date": dto.date,
                "supplierId": dto.supplier_id,
                "supplierName": dto.supplier_name.unwrap_or_default(),
                "lines": lineas_con_id(&dto.lines),
                "notes": dto.notes.unwrap_or_default(),
            }),
            usuario,
        )
        .await
    }

    pub async fn convertir_orden_compra(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let orden = self.obtener("ordenes-compra", id, usuario).await?;
        let estado = orden
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if estado != "pendiente" {
            return Err(ApiError::dominio(
                "ORDEN_NO_PENDIENTE",
                "La orden ya no está pendiente.",
            ));
        }
        let numero = self.siguiente_numero("compras", "C", usuario).await?;
        self.crear(
            "compras",
            json!({
                "number": numero,
                "date": hoy(),
                "supplierId": orden.get("supplierId").cloned().unwrap_or(Value::Null),
                "supplierName": orden.get("supplierName").cloned().unwrap_or_else(|| json!("")),
                "paymentMethod": "efectivo",
                "status": "pendiente",
                "origin": "productos",
                "costType": "inventario",
                "lines": orden.get("lines").cloned().unwrap_or_else(|| json!([])),
                "notes": format!(
                    "Convertida desde la orden {}.",
                    orden.get("number").and_then(Value::as_str).unwrap_or("")
                ),
                "retentionCents": 0,
                "userName": "",
            }),
            usuario,
        )
        .await?;
        self.actualizar(
            "ordenes-compra",
            id,
            json!({"status": "convertida"}),
            usuario,
        )
        .await
    }

    // ── configuración de documentos ────────────────────────────────────────
}
