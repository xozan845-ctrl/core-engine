use super::*;

impl Envios {
    /// Prepara la guía de despacho cuando el pago fue procesado
    /// (`payment.procesado`). Idempotente por `(order_id, empresa_id)`.
    pub async fn preparar_envio(
        &self,
        order_id: &str,
        empresa_id: &str,
        negocio_id: Option<&str>,
        monto_cents: i64,
        parte_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let empresa_uuid = parsear_uuid(empresa_id)?;
        let negocio_uuid = negocio_id.map(parsear_uuid).transpose()?;

        let existente = sqlx::query(
            "SELECT * FROM logistics.envios
              WHERE order_id = $1 AND empresa_id = $2
                AND negocio_id IS NOT DISTINCT FROM $3",
        )
        .bind(order_uuid)
        .bind(empresa_uuid)
        .bind(negocio_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;

        if let Some(fila) = existente {
            let envio = fila_envio(&fila)?;
            if let (Some(tipo), Some(tenant)) = (
                envio.proveedor_tipo.as_deref(),
                envio.proveedor_tenant_id.as_deref(),
            ) {
                self.registrar_asignacion(&envio.id, None, tipo, tenant, "automatica", None)
                    .await?;
            }
            return Ok(envio);
        }

        let guia = generar_guia();
        let proveedor = self.proveedor_de_empresa(empresa_id, negocio_id).await?;
        let parte_uuid = match parte_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };

        let fila = sqlx::query(
            "INSERT INTO logistics.envios
                (id, guia, order_id, empresa_id, negocio_id, parte_id, proveedor_tipo,
                 proveedor_tenant_id, monto_cents, estado)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'en_preparacion') RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(&guia)
        .bind(order_uuid)
        .bind(empresa_uuid)
        .bind(negocio_uuid)
        .bind(parte_uuid)
        .bind(proveedor.tipo.as_deref())
        .bind(
            proveedor
                .tenant_id
                .as_deref()
                .map(parsear_uuid)
                .transpose()?,
        )
        .bind(monto_cents)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::dominio("ENVIO_FALLIDO", "No se pudo preparar el envio."))?;

        let envio = fila_envio(&fila)?;
        if let (Some(tipo), Some(tenant)) = (
            envio.proveedor_tipo.as_deref(),
            envio.proveedor_tenant_id.as_deref(),
        ) {
            self.registrar_asignacion(&envio.id, None, tipo, tenant, "automatica", None)
                .await?;
        }
        logs::info(
            "envios",
            crate::campos! {
                "msg": "Guia de despacho generada",
                "order_id": order_id,
                "guia": envio.guia,
            },
        );
        Ok(envio)
    }

    /// Consulta la orden a orders y agrupa las líneas por empresa.
    pub async fn preparar_envios_de_orden(&self, order_id: &str) -> Result<Vec<Envio>, ApiError> {
        let orden = self.consultar_orden(order_id).await?;
        let Some(orden) = orden else {
            return Err(ApiError::dominio(
                "ORDEN_SIN_LINEAS",
                "La orden no tiene líneas para despachar.",
            ));
        };
        if orden.items.is_empty() {
            return Err(ApiError::dominio(
                "ORDEN_SIN_LINEAS",
                "La orden no tiene líneas para despachar.",
            ));
        }

        let mut montos: Vec<((String, Option<String>), i64)> = Vec::new();
        for item in &orden.items {
            if item.empresa_id.trim().is_empty() {
                return Err(ApiError::dominio(
                    "EMPRESA_REQUERIDA",
                    "Una línea no tiene empresa asignada.",
                ));
            }
            // Solo delivery genera envío físico; pickup/onsite/digital los
            // gestiona su propio flujo.
            if item
                .fulfillment
                .as_deref()
                .is_some_and(|valor| valor != "delivery")
            {
                continue;
            }
            let subtotal = item.precio_unitario_cents.saturating_mul(item.cantidad);
            let clave = (item.empresa_id.clone(), item.negocio_id.clone());
            match montos.iter_mut().find(|(id, _)| id == &clave) {
                Some((_, total)) => *total += subtotal,
                None => montos.push((clave, subtotal)),
            }
        }

        let mut envios = Vec::with_capacity(montos.len());
        for ((empresa, negocio), monto) in montos {
            envios.push(
                self.preparar_envio(
                    order_id,
                    &empresa,
                    negocio.as_deref(),
                    monto,
                    orden.parte_id.as_deref(),
                )
                .await?,
            );
        }
        Ok(envios)
    }

    /// Deriva la flota de las capacidades activas de la empresa/negocio.
    async fn proveedor_de_empresa(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Proveedor, ApiError> {
        let filas = self
            .capacidades
            .activas_en(
                empresa_id,
                negocio_id,
                &["logistica_rutas", "fulfillment_delivery"],
            )
            .await
            .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
        let activa = |codigo: &str| {
            filas.iter().any(|fila| {
                fila.empresa_id == empresa_id && fila.activo && fila.capacidad_codigo == codigo
            })
        };
        let propia = activa("logistica_rutas");
        let delivery = activa("fulfillment_delivery");
        if propia && delivery {
            Ok(Proveedor {
                tipo: None,
                tenant_id: None,
            })
        } else if propia {
            Ok(Proveedor {
                tipo: Some("empresa".to_string()),
                tenant_id: Some(empresa_id.to_string()),
            })
        } else {
            Ok(Proveedor {
                tipo: Some("plataforma".to_string()),
                tenant_id: Some(self.tenant_plataforma.clone()),
            })
        }
    }

    pub async fn asignar_proveedor(
        &self,
        envio_id: &str,
        empresa_id: &str,
        tipo: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let envio_uuid = parsear_uuid(envio_id)?;
        let empresa_uuid = parsear_uuid(empresa_id)?;

        let envio = sqlx::query("SELECT * FROM logistics.envios WHERE id = $1 AND empresa_id = $2")
            .bind(envio_uuid)
            .bind(empresa_uuid)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?
            .ok_or_else(|| {
                ApiError::dominio(
                    "ENVIO_NO_ENCONTRADO",
                    "El envío no pertenece a la empresa activa.",
                )
            })?;
        let envio = fila_envio(&envio)?;
        if envio.proveedor_tipo.is_some() {
            return Err(ApiError::dominio(
                "ASIGNACION_FIJA",
                "Solo un envío híbrido permite elegir proveedor.",
            ));
        }

        let tenant = if tipo == "plataforma" {
            self.tenant_plataforma.clone()
        } else {
            empresa_id.to_string()
        };
        let fila = sqlx::query(
            "UPDATE logistics.envios SET proveedor_tipo = $1, proveedor_tenant_id = $2
             WHERE id = $3 AND empresa_id = $4 RETURNING *",
        )
        .bind(tipo)
        .bind(parsear_uuid(&tenant)?)
        .bind(envio_uuid)
        .bind(empresa_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::dominio("ENVIO_NO_ENCONTRADO", "No se pudo asignar el envío."))?;

        let actualizado = fila_envio(&fila)?;
        self.registrar_asignacion(
            envio_id,
            envio.proveedor_tipo.as_deref(),
            tipo,
            &tenant,
            "empresario",
            actor_usuario_id,
        )
        .await?;
        Ok(actualizado)
    }

    async fn registrar_asignacion(
        &self,
        envio_id: &str,
        anterior: Option<&str>,
        nuevo: &str,
        proveedor_tenant_id: &str,
        origen: &str,
        actor_usuario_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let sql = format!(
            "INSERT INTO logistics.asignaciones_envio
                (envio_id, proveedor_tipo_anterior, proveedor_tipo_nuevo,
                 proveedor_tenant_id, origen, actor_usuario_id)
             VALUES ($1,$2,$3,$4,$5,$6)
             {}",
            if origen == "automatica" {
                "ON CONFLICT DO NOTHING"
            } else {
                ""
            }
        );
        let actor = match actor_usuario_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };
        sqlx::query(&sql)
            .bind(parsear_uuid(envio_id)?)
            .bind(anterior)
            .bind(nuevo)
            .bind(parsear_uuid(proveedor_tenant_id)?)
            .bind(origen)
            .bind(actor)
            .execute(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(())
    }
}
