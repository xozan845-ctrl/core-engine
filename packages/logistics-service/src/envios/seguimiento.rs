use super::*;

impl Envios {
    pub async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<Envio>, ApiError> {
        let filas = sqlx::query(
            "SELECT * FROM logistics.envios WHERE empresa_id = $1
             ORDER BY creado_en DESC LIMIT 200",
        )
        .bind(parsear_uuid(empresa_id)?)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(fila_envio).collect()
    }

    /// Avanza un envío de la flota y, si todos los envíos de la orden alcanzan
    /// el estado, propaga la transición a orders.
    /// Registra la prueba de entrega (POD) del envío. No cambia el estado;
    /// el paso a `entregada` la exige cuando la capacidad está activa.
    pub async fn registrar_pod(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        receptor: &str,
        foto_objeto_id: Option<&str>,
        firma_objeto_id: Option<&str>,
        notas: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let receptor = receptor.trim();
        if receptor.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "receptor", "restricciones": ["isNotEmpty"]}
            ])));
        }
        if foto_objeto_id.is_none() && firma_objeto_id.is_none() {
            return Err(ApiError::validacion(json!([
                {"campo": "evidencia", "restricciones": ["required"]}
            ])));
        }
        let foto = foto_objeto_id.map(parsear_uuid).transpose()?;
        let firma = firma_objeto_id.map(parsear_uuid).transpose()?;
        let fila = sqlx::query(
            "UPDATE logistics.envios
                SET pod_receptor = $1,
                    pod_notas = $2,
                    pod_foto_objeto_id = $3,
                    pod_firma_objeto_id = $4,
                    pod_capturado_en = NOW()
              WHERE id = $5 AND proveedor_tenant_id = $6
              RETURNING *",
        )
        .bind(receptor)
        .bind(notas.unwrap_or(""))
        .bind(foto)
        .bind(firma)
        .bind(parsear_uuid(envio_id)?)
        .bind(parsear_uuid(proveedor_tenant_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "ENVIO_NO_ENCONTRADO",
                "El envío no pertenece a la flota activa.",
            )
        })?;
        fila_envio(&fila)
    }

    pub async fn avanzar_estado_envio(
        &self,
        envio_id: &str,
        proveedor_tenant_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<Envio, ApiError> {
        let actual = sqlx::query(
            "SELECT * FROM logistics.envios
              WHERE id = $1 AND proveedor_tenant_id = $2",
        )
        .bind(parsear_uuid(envio_id)?)
        .bind(parsear_uuid(proveedor_tenant_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "ENVIO_NO_ENCONTRADO",
                "El envío no pertenece a la flota activa.",
            )
        })?;
        let actual = fila_envio(&actual)?;
        if estado == "entregada"
            && actual.pod_capturado_en.is_none()
            && self
                .pod_requerido(&actual.empresa_id, actual.negocio_id.as_deref())
                .await
        {
            return Err(ApiError::dominio(
                "POD_REQUERIDO",
                "La entrega requiere prueba de entrega (receptor y evidencia).",
            ));
        }
        let fila = sqlx::query(
            "UPDATE logistics.envios SET estado = $1
             WHERE id = $2 AND proveedor_tenant_id = $3
             RETURNING *",
        )
        .bind(estado)
        .bind(parsear_uuid(envio_id)?)
        .bind(parsear_uuid(proveedor_tenant_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "ENVIO_NO_ENCONTRADO",
                "El envío no pertenece a la flota activa.",
            )
        })?;
        let envio = fila_envio(&fila)?;

        let aceptados = estados_aceptados(estado);
        let pendientes = sqlx::query(
            "SELECT COUNT(*)::int AS total FROM logistics.envios
             WHERE order_id = $1 AND NOT (estado = ANY($2::text[]))",
        )
        .bind(parsear_uuid(&envio.order_id)?)
        .bind(&aceptados)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let total = pendientes
            .as_ref()
            .and_then(|fila| fila.try_get::<i32, _>("total").ok())
            .unwrap_or(1);
        if total == 0 {
            self.avanzar_estado_orden(&envio.order_id, estado, motivo, None, actor_usuario_id)
                .await?;
        }
        Ok(envio)
    }

    async fn pod_requerido(&self, empresa_id: &str, negocio_id: Option<&str>) -> bool {
        self.capacidades
            .activas_en(empresa_id, negocio_id, &["logistica_pod"])
            .await
            .map(|filas| {
                filas
                    .iter()
                    .any(|fila| fila.activo && fila.capacidad_codigo == "logistica_pod")
            })
            .unwrap_or(false)
    }

    pub async fn de_orden(&self, order_id: &str) -> Result<Option<Envio>, ApiError> {
        let fila = sqlx::query(
            "SELECT * FROM logistics.envios WHERE order_id = $1
             ORDER BY creado_en LIMIT 1",
        )
        .bind(parsear_uuid(order_id)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.as_ref().map(fila_envio).transpose()
    }

    pub async fn listar(&self) -> Result<Vec<EnvioConMonto>, ApiError> {
        let filas = sqlx::query("SELECT * FROM logistics.envios ORDER BY creado_en DESC LIMIT 200")
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;
        filas
            .iter()
            .map(|fila| fila_envio(fila).map(EnvioConMonto::from))
            .collect()
    }

    /// `PATCH /orders/:id/estado`: avanza el ciclo de vida en orders (Tabla 13).
    pub async fn avanzar_estado_orden(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        empresa_id: Option<&str>,
        actor_usuario_id: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        if estado_nuevo == "entregada" {
            let envios = sqlx::query("SELECT * FROM logistics.envios WHERE order_id = $1")
                .bind(parsear_uuid(order_id)?)
                .fetch_all(self.db.pool())
                .await
                .map_err(error_db)?;
            for envio in &envios {
                let envio = fila_envio(envio)?;
                if envio.pod_capturado_en.is_none()
                    && self
                        .pod_requerido(&envio.empresa_id, envio.negocio_id.as_deref())
                        .await
                {
                    return Err(ApiError::dominio(
                        "POD_REQUERIDO",
                        "La entrega requiere prueba de entrega (receptor y evidencia).",
                    ));
                }
            }
        }
        if let Some(empresa_id) = empresa_id {
            let orden = self
                .consultar_orden(order_id)
                .await?
                .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
            if !orden.items.iter().any(|item| item.empresa_id == empresa_id) {
                return Err(ApiError::acceso_denegado(
                    "La orden no pertenece a la empresa activa.",
                ));
            }
        }

        let orden = self
            .orders
            .avanzar(order_id, estado_nuevo, motivo, actor_usuario_id)
            .await?;
        logs::info(
            "envios",
            crate::campos! {
                "msg": "Estado de orden avanzado",
                "order_id": order_id,
                "estado": estado_nuevo,
            },
        );
        Ok(orden)
    }

    /// Consulta una orden; `None` si orders no responde o responde error.
    pub async fn consultar_orden(&self, order_id: &str) -> Result<Option<OrdenExterna>, ApiError> {
        Ok(self.orders.consultar(order_id).await.unwrap_or(None))
    }
}
