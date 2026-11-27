use super::prelude::*;
use super::*;

impl Facturacion {
    /// Verifica que el tenant no esté emitiendo un comprobante "electrónico"
    /// con un simulador o un proveedor sin certificar. Se ejecuta antes de
    /// consumir el secuencial y crear el comprobante.
    async fn validar_electronica(
        &self,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let Some(empresa_id) = empresa_id else {
            return Ok(());
        };
        let fila = sqlx::query(
            "SELECT facturacion_electronica, proveedor_electronico
               FROM finance.configuracion_fiscal
              WHERE empresa_id = $1::uuid
                AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Ok(());
        };
        let electronica: bool = fila.try_get("facturacion_electronica").unwrap_or(false);
        if !electronica {
            return Ok(());
        }
        let proveedor = fila
            .try_get::<String, _>("proveedor_electronico")
            .unwrap_or_else(|_| "simulado".to_string());
        match proveedor.as_str() {
            "simulado" if !self.permitir_sin_certificado => Err(ApiError::dominio(
                "FACTURACION_NO_CERTIFICADA",
                "El piloto no puede emitir comprobantes electronicos con el simulador. Configure un proveedor certificado o acepte FACTURACION_PERMITIR_SIN_CERTIFICADO=true.",
            )),
            "http" if self.http.is_none() => Err(ApiError::dominio(
                "PROVEEDOR_NO_CONFIGURADO",
                "No hay URL de facturacion electronica configurada.",
            )),
            "http" | "dgi" if !self.proveedor_certificado => Err(ApiError::dominio(
                "FACTURACION_NO_CERTIFICADA",
                "El proveedor electronico no esta certificado (FACTURACION_PROVEEDOR_CERTIFICADO=false).",
            )),
            "dgi" if self.dgi.is_none() => Err(ApiError::dominio(
                "PROVEEDOR_NO_CONFIGURADO",
                "No hay gateway DGI configurado.",
            )),
            _ => Ok(()),
        }
    }

    /// Emite (o reserva como BORRADOR) un comprobante de la venta.
    pub async fn emitir(&self, dto: EmitirComprobante) -> Result<Comprobante, ApiError> {
        if dto.tipo != "FACTURA" && dto.tipo != "NOTA_CREDITO" {
            return Err(ApiError::validacion(json!([
                {"campo": "tipo", "restricciones": ["isIn"]}
            ])));
        }
        let orden = self
            .ordenes
            .consultar(&dto.orden_id)
            .await
            .ok_or_else(|| ApiError::no_encontrado("Orden", &dto.orden_id))?;
        let negocio_uuid = orden
            .negocio_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::interno())?;
        let empresa_uuid = orden
            .empresa_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::interno())?;
        self.validar_electronica(orden.empresa_id.as_deref(), orden.negocio_id.as_deref())
            .await?;
        let region = match orden.empresa_id.as_deref() {
            Some(empresa_id) => sqlx::query_scalar::<_, Option<String>>(
                "SELECT region_tributaria FROM identity.empresas WHERE id = $1::uuid",
            )
            .bind(empresa_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?
            .flatten(),
            None => None,
        };

        let serie = sqlx::query(
            "SELECT serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
             FROM finance.series_comprobantes
              WHERE tipo = $1 AND activa
                AND ($2::uuid IS NULL OR empresa_id = $2::uuid OR empresa_id IS NULL)
                AND ($3::uuid IS NULL OR negocio_id = $3::uuid OR negocio_id IS NULL)
                AND ($4::text IS NULL OR jurisdiccion = $4)
              ORDER BY (empresa_id IS NOT NULL) DESC,
                       (negocio_id IS NOT NULL) DESC,
                       serie
              LIMIT 1",
        )
        .bind(&dto.tipo)
        .bind(empresa_uuid)
        .bind(negocio_uuid)
        .bind(region.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "SERIE_NO_DISPONIBLE",
                format!(
                    "No hay serie activa para {} en la región {}.",
                    dto.tipo,
                    region.as_deref().unwrap_or("sin región")
                ),
            )
        })?;
        let serie = serie_de_fila(&serie);

        let fila_jurisdiccion = sqlx::query(
            "SELECT tasa_iva, moneda, retencion_activa, tasa_retencion, retencion_codigo
               FROM finance.jurisdicciones WHERE codigo_pais = $1",
        )
        .bind(&serie.jurisdiccion)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        // La tasa y la moneda salen de la región de la serie. Sin región no se
        // inventa un IVA: fallar cerrado es parte del contrato fiscal.
        let Some(fila_jurisdiccion) = fila_jurisdiccion else {
            return Err(ApiError::dominio(
                "SIN_JURISDICCION",
                format!(
                    "La serie {} apunta a la región {} que no existe.",
                    serie.serie, serie.jurisdiccion
                ),
            ));
        };
        let tasa_iva = fila_jurisdiccion
            .try_get::<f64, _>("tasa_iva")
            .map_err(error_db)?;
        let moneda = fila_jurisdiccion
            .try_get::<String, _>("moneda")
            .map_err(error_db)?;
        let retencion_activa: bool = fila_jurisdiccion
            .try_get("retencion_activa")
            .unwrap_or(false);
        let tasa_retencion: f64 = fila_jurisdiccion.try_get("tasa_retencion").unwrap_or(0.0);
        let retencion_codigo: String = fila_jurisdiccion
            .try_get("retencion_codigo")
            .unwrap_or_else(|_| "IR".to_string());
        let mut exento: i64 = 0;
        let (base, iva) = if orden.items.is_empty() {
            let base = money::desde_centavos(orden.total_cents)?;
            let iva = money::multiplicar_por(base, tasa_iva)?;
            (base, iva)
        } else {
            let mut base = 0;
            let mut iva = 0;
            let con_snapshot = orden
                .items
                .iter()
                .any(|item| item.total_linea_cents > 0 || item.impuesto_cents > 0);
            for item in &orden.items {
                let neto = item
                    .precio_unitario_cents
                    .saturating_mul(item.cantidad)
                    .saturating_sub(item.descuento_cents);
                if neto <= 0 && item.total_linea_cents <= 0 {
                    continue;
                }
                if item.total_linea_cents > 0 || item.impuesto_cents > 0 {
                    let base_linea = item
                        .total_linea_cents
                        .saturating_sub(item.impuesto_cents)
                        .max(0);
                    if item.impuesto_codigo.as_deref() == Some("EXENTA") {
                        exento = exento.saturating_add(base_linea);
                    }
                    base = money::sumar(base, money::desde_centavos(base_linea)?)?;
                    iva = money::sumar(iva, money::desde_centavos(item.impuesto_cents)?)?;
                } else {
                    let base_linea = money::desde_centavos(neto)?;
                    base = money::sumar(base, base_linea)?;
                    iva = money::sumar(iva, money::multiplicar_por(base_linea, tasa_iva)?)?;
                }
            }
            if !con_snapshot && orden.descuento_cupon_cents > 0 && base > 0 {
                let cupon = orden.descuento_cupon_cents.min(base);
                base -= cupon;
                iva = money::multiplicar_por(base, tasa_iva)?;
            }
            (base, iva)
        };
        let total = money::sumar(base, iva)?;
        let base_gravada = base.saturating_sub(exento);
        let retencion = if retencion_activa && tasa_retencion > 0.0 {
            money::multiplicar_por(base_gravada, tasa_retencion)?
        } else {
            0
        };
        let retencion_bps = if retencion_activa {
            (tasa_retencion * 10_000.0).round() as i64
        } else {
            0
        };
        let numero = format!("{}{:08}", serie.prefijo, serie.secuencial_actual + 1);
        let estado = if self.emitir { "EMITIDO" } else { "BORRADOR" };
        let parte_id = dto.parte_id.clone();
        let items_fiscales: Vec<Value> = orden
            .items
            .iter()
            .map(|item| {
                let neto = item
                    .precio_unitario_cents
                    .saturating_mul(item.cantidad)
                    .saturating_sub(item.descuento_cents);
                let (base_linea, iva_linea, total_linea) =
                    if item.total_linea_cents > 0 || item.impuesto_cents > 0 {
                        let base = item
                            .total_linea_cents
                            .saturating_sub(item.impuesto_cents)
                            .max(0);
                        (base, item.impuesto_cents, item.total_linea_cents)
                    } else {
                        let base = neto.max(0);
                        let iva = (base as f64 * tasa_iva).round() as i64;
                        (base, iva, base + iva)
                    };
                let exento_linea = item.impuesto_codigo.as_deref() == Some("EXENTA");
                json!({
                    "sku": item.sku,
                    "cantidad": item.cantidad,
                    "base_cents": base_linea,
                    "iva_cents": iva_linea,
                    "total_cents": total_linea,
                    "tasa_impuesto_bps": item.tasa_impuesto_bps,
                    "impuesto_codigo": item.impuesto_codigo.clone(),
                    "exento": exento_linea,
                })
            })
            .collect();
        let datos_cliente = json!({
            "parte_id": dto.parte_id,
            "razon_social": dto.razon_social,
            "ruc": dto.ruc,
            "emisor_ruc": serie.ruc_emisor,
            "orden_id": dto.orden_id,
            "items": items_fiscales,
            "retencion": {
                "activa": retencion_activa,
                "codigo": retencion_codigo,
                "tasa_bps": retencion_bps,
                "monto_cents": retencion,
            },
        });

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.comprobantes
               (id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                iva_cents, exento_cents, retencion_cents, retencion_tasa_bps, total_cents,
                moneda, estado, datos_cliente, emitido_en)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                     $11, $12, $13, $14, $15, $16,
                     CASE WHEN $15 = 'EMITIDO' THEN NOW() ELSE NULL END)
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, retencion_cents, retencion_tasa_bps,
                       total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(&serie.serie)
        .bind(&numero)
        .bind(&dto.tipo)
        .bind(uuid::Uuid::parse_str(&dto.orden_id).map_err(|_| {
            ApiError::validacion(json!([
                {"campo": "orden_id", "restricciones": ["isUuid"]}
            ]))
        })?)
        .bind(
            orden
                .empresa_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| ApiError::interno())?,
        )
        .bind(negocio_uuid)
        .bind(
            parte_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| {
                    ApiError::validacion(json!([
                        {"campo": "parte_id", "restricciones": ["isUuid"]}
                    ]))
                })?,
        )
        .bind(base_gravada as i32)
        .bind(iva as i32)
        .bind(exento as i32)
        .bind(retencion as i32)
        .bind(retencion_bps as i32)
        .bind(total as i32)
        .bind(&moneda)
        .bind(estado)
        .bind(sqlx::types::Json(&datos_cliente))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let comprobante = comprobante_de_fila(&fila);

        // el secuencial se consume siempre (también en BORRADOR) para no romper
        // la consecutividad exigida por la DGI al activar la facturación real.
        sqlx::query(
            "UPDATE finance.series_comprobantes
             SET secuencial_actual = secuencial_actual + 1 WHERE serie = $1",
        )
        .bind(&serie.serie)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        if estado == "EMITIDO" {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!(
                            "IVA trasladado {} {} {}",
                            dto.tipo, comprobante.serie, comprobante.numero
                        ),
                        tipo: "AJUSTE".to_string(),
                        referencia_tipo: Some("comprobante".to_string()),
                        referencia_id: Some(comprobante.id.clone()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: iva,
                                haber_cents: 0,
                                concepto: Some("IVA trasladado".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "2.1.3".to_string(),
                                debe_cents: 0,
                                haber_cents: iva,
                                concepto: Some("IVA por pagar".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    COMPROBANTE_EMITIDO,
                    json!({
                        "comprobante_id": comprobante.id,
                        "serie": comprobante.serie,
                        "numero": comprobante.numero,
                        "tipo": dto.tipo,
                        "orden_id": dto.orden_id,
                        "parte_id": comprobante.parte_id,
                        "empresa_id": comprobante.empresa_id,
                        "base_gravada_cents": base_gravada,
                        "iva_cents": iva,
                        "exento_cents": exento,
                        "retencion_cents": retencion,
                        "total_cents": total,
                        "moneda": moneda,
                    }),
                )
                .await?;
        }
        tx.commit().await.map_err(error_db)?;
        self.enviar_electronico(comprobante).await
    }

    /// Envía el comprobante al proveedor electrónico si el tenant lo activó.
    /// Un fallo del proveedor no invalida el comprobante: queda con
    /// `electronica_estado = error` para reintento operativo.
    async fn enviar_electronico(
        &self,
        mut comprobante: Comprobante,
    ) -> Result<Comprobante, ApiError> {
        if let Some(empresa_id) = comprobante.empresa_id.as_deref() {
            let configuracion = sqlx::query(
                "SELECT facturacion_electronica, proveedor_electronico, ambiente_electronico
                   FROM finance.configuracion_fiscal
                  WHERE empresa_id = $1::uuid
                    AND (negocio_id = $2::uuid OR negocio_id IS NULL)
                  ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
            )
            .bind(empresa_id)
            .bind(comprobante.negocio_id.as_deref())
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            if let Some(fila) = configuracion {
                let electronica: bool = fila.try_get("facturacion_electronica").unwrap_or(false);
                if electronica {
                    let proveedor = fila
                        .try_get::<String, _>("proveedor_electronico")
                        .unwrap_or_else(|_| "simulado".to_string());
                    let ambiente = fila
                        .try_get::<String, _>("ambiente_electronico")
                        .unwrap_or_else(|_| "pruebas".to_string());
                    let resultado = match proveedor.as_str() {
                        "http" => match self.http.as_ref() {
                            Some(http) => http.emitir(&comprobante, &ambiente).await,
                            None => Err(ApiError::dominio(
                                "PROVEEDOR_NO_CONFIGURADO",
                                "No hay URL de facturación electrónica configurada.",
                            )),
                        },
                        "dgi" => match self.dgi.as_ref() {
                            Some(dgi) => dgi.emitir(&comprobante, &ambiente).await,
                            None => Err(ApiError::dominio(
                                "PROVEEDOR_NO_CONFIGURADO",
                                "No hay gateway DGI configurado.",
                            )),
                        },
                        _ => FacturacionSimulada.emitir(&comprobante, &ambiente).await,
                    };
                    let (estado, referencia) = match resultado {
                        Ok(referencia) => (referencia.estado, referencia.referencia),
                        Err(_) => ("error".to_string(), String::new()),
                    };
                    sqlx::query(
                        "UPDATE finance.comprobantes
                            SET electronica_estado = $2, electronica_referencia = $3,
                                electronica_enviada_en = NOW()
                          WHERE id = $1::uuid",
                    )
                    .bind(&comprobante.id)
                    .bind(&estado)
                    .bind(&referencia)
                    .execute(self.db.pool())
                    .await
                    .map_err(error_db)?;
                    comprobante.electronica_estado = Some(estado);
                    comprobante.electronica_referencia =
                        Some(referencia).filter(|valor| !valor.is_empty());
                    comprobante.electronica_enviada_en = Some(crate::db::iso_ms(Utc::now()));
                }
            }
        }
        Ok(comprobante)
    }

    /// Emite una nota de crédito parcial con montos explícitos de una RMA.
    pub async fn emitir_nota_credito(&self, dto: NotaCreditoDto) -> Result<Comprobante, ApiError> {
        if dto.base_cents < 0
            || dto.iva_cents < 0
            || dto.total_cents <= 0
            || dto.base_cents.saturating_add(dto.iva_cents) != dto.total_cents
        {
            return Err(ApiError::dominio(
                "MONTO_NOTA_CREDITO_INVALIDO",
                "La nota de crédito requiere base + IVA = total.",
            ));
        }
        let negocio_uuid = dto
            .negocio_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::interno())?;
        let empresa_uuid = uuid::Uuid::parse_str(&dto.empresa_id).map_err(|_| {
            ApiError::validacion(json!([
                {"campo": "empresa_id", "restricciones": ["isUuid"]}
            ]))
        })?;
        self.validar_electronica(Some(&dto.empresa_id), dto.negocio_id.as_deref())
            .await?;
        let region = sqlx::query_scalar::<_, Option<String>>(
            "SELECT region_tributaria FROM identity.empresas WHERE id = $1::uuid",
        )
        .bind(&dto.empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .flatten();
        let serie = sqlx::query(
            "SELECT serie, empresa_id, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
               FROM finance.series_comprobantes
              WHERE tipo = 'NOTA_CREDITO' AND activa
                AND (empresa_id = $1::uuid OR empresa_id IS NULL)
                AND (negocio_id = $2::uuid OR negocio_id IS NULL)
                AND ($3::text IS NULL OR jurisdiccion = $3)
              ORDER BY (empresa_id IS NOT NULL) DESC, (negocio_id IS NOT NULL) DESC, serie
              LIMIT 1",
        )
        .bind(empresa_uuid)
        .bind(negocio_uuid)
        .bind(region.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "SERIE_NO_DISPONIBLE",
                "No hay serie activa para NOTA_CREDITO en la región del tenant.",
            )
        })?;
        let serie = serie_de_fila(&serie);
        let moneda = sqlx::query_scalar::<_, String>(
            "SELECT moneda FROM finance.jurisdicciones WHERE codigo_pais = $1",
        )
        .bind(&serie.jurisdiccion)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .or(dto.moneda.clone())
        .ok_or_else(|| {
            ApiError::dominio(
                "SIN_JURISDICCION",
                format!(
                    "La serie {} apunta a la región {} que no existe.",
                    serie.serie, serie.jurisdiccion
                ),
            )
        })?;
        let numero = format!("{}{:08}", serie.prefijo, serie.secuencial_actual + 1);
        let estado = if self.emitir { "EMITIDO" } else { "BORRADOR" };
        let datos_cliente = json!({
            "devolucion_id": dto.devolucion_id,
            "razon_social": Value::Null,
            "ruc": Value::Null,
            "emisor_ruc": serie.ruc_emisor,
            "orden_id": dto.orden_id,
            "items": dto.items,
        });

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.comprobantes
               (id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                estado, datos_cliente, emitido_en)
             VALUES (gen_random_uuid(), $1, $2, 'NOTA_CREDITO', $3::uuid, $4::uuid,
                     $5::uuid, $6::uuid, $7, $8, 0, $9, $10, $11, $12,
                     CASE WHEN $11 = 'EMITIDO' THEN NOW() ELSE NULL END)
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                       base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                       estado, datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(&serie.serie)
        .bind(&numero)
        .bind(
            uuid::Uuid::parse_str(&dto.orden_id)
                .map_err(|_| ApiError::validacion(json!([{"campo": "orden_id"}])))?,
        )
        .bind(
            uuid::Uuid::parse_str(&dto.empresa_id)
                .map_err(|_| ApiError::validacion(json!([{"campo": "empresa_id"}])))?,
        )
        .bind(negocio_uuid)
        .bind(
            dto.parte_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| ApiError::interno())?,
        )
        .bind(dto.base_cents as i32)
        .bind(dto.iva_cents as i32)
        .bind(dto.total_cents as i32)
        .bind(&moneda)
        .bind(estado)
        .bind(sqlx::types::Json(&datos_cliente))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let comprobante = comprobante_de_fila(&fila);
        sqlx::query(
            "UPDATE finance.series_comprobantes
             SET secuencial_actual = secuencial_actual + 1 WHERE serie = $1",
        )
        .bind(&serie.serie)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        if estado == "EMITIDO" && dto.iva_cents > 0 {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!("IVA revertido nota de credito {}", comprobante.numero),
                        tipo: "AJUSTE".to_string(),
                        referencia_tipo: Some("comprobante".to_string()),
                        referencia_id: Some(comprobante.id.clone()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.3".to_string(),
                                debe_cents: dto.iva_cents,
                                haber_cents: 0,
                                concepto: Some("IVA por pagar revertido".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: 0,
                                haber_cents: dto.iva_cents,
                                concepto: Some("IVA trasladado revertido".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    COMPROBANTE_EMITIDO,
                    json!({
                        "comprobante_id": comprobante.id,
                        "serie": comprobante.serie,
                        "numero": comprobante.numero,
                        "tipo": "NOTA_CREDITO",
                        "orden_id": dto.orden_id,
                        "devolucion_id": dto.devolucion_id,
                        "base_gravada_cents": dto.base_cents,
                        "iva_cents": dto.iva_cents,
                        "total_cents": dto.total_cents,
                        "moneda": moneda,
                    }),
                )
                .await?;
        }
        tx.commit().await.map_err(error_db)?;
        self.enviar_electronico(comprobante).await
    }
}
