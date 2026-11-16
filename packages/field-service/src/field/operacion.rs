use super::*;

impl FieldService {
    // ── tracking ──────────────────────────────────────────────────────────
    pub async fn listar_tracking(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(personal_id) = personal_id {
            filtros.push(Filtro::uuid("personal_id", personal_id));
        }
        if let Some(desde) = desde {
            filtros.push(Filtro::rango("registrado_en", ">=", desde));
        }
        if let Some(hasta) = hasta {
            filtros.push(Filtro::rango("registrado_en", "<=", hasta));
        }
        self.listar(&TABLA_TRACKING, tenant, filtros, "registrado_en")
            .await
    }

    pub async fn registrar_tracking(
        &self,
        tenant: &str,
        registros: &[Value],
    ) -> Result<Vec<Value>, ApiError> {
        let mut resultados = Vec::with_capacity(registros.len());
        for registro in registros {
            resultados.push(
                self.insertar(&TABLA_TRACKING, tenant, None, registro, false)
                    .await?,
            );
        }
        Ok(resultados)
    }

    // ── visitas ───────────────────────────────────────────────────────────
    pub async fn listar_visitas(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
        fecha: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(personal_id) = personal_id {
            filtros.push(Filtro::uuid("personal_id", personal_id));
        }
        if let Some(fecha) = fecha {
            filtros.push(Filtro::fecha(
                "DATE(created_at AT TIME ZONE 'America/Managua')",
                fecha,
            ));
        }
        self.listar(&TABLA_VISITAS, tenant, filtros, "created_at")
            .await
    }

    pub async fn registrar_visita(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_VISITAS, tenant, None, cuerpo, false)
            .await
    }

    // ── cumplimiento ──────────────────────────────────────────────────────
    pub async fn obtener_cumplimiento(
        &self,
        tenant: &str,
        ruta_id: &str,
        fecha: &str,
    ) -> Result<Value, ApiError> {
        self.ejecutar_uno(
            "SELECT * FROM field.cumplimiento WHERE tenant_id = $1::uuid AND ruta_id = $2::uuid AND fecha = $3::date",
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(ruta_id.to_string()),
                Valor::Texto(fecha.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| ApiError::no_encontrado("cumplimiento", &format!("{ruta_id}/{fecha}")))
    }

    pub async fn guardar_cumplimiento(
        &self,
        tenant: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let ruta_id = cuerpo
            .get("rutaId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let fecha = cuerpo
            .get("fecha")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let metricas = cuerpo.get("metricas").cloned().unwrap_or_else(|| json!({}));
        self.ejecutar_uno(
            "INSERT INTO field.cumplimiento (tenant_id, ruta_id, fecha, metricas, updated_at) VALUES ($1::uuid, $2::uuid, $3::date, $4::jsonb, NOW()) ON CONFLICT (tenant_id, ruta_id, fecha) DO UPDATE SET metricas = $4::jsonb, updated_at = NOW() RETURNING *",
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(ruta_id),
                Valor::Texto(fecha),
                Valor::Texto(metricas.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| {
            ApiError::persistencia_fallida("No se pudo guardar el cumplimiento.")
        })
    }

    // ── sync offline ──────────────────────────────────────────────────────
    pub async fn sincronizar(&self, tenant: &str, operaciones: &[Value]) -> Vec<Value> {
        let mut resultados = Vec::with_capacity(operaciones.len());
        for operacion in operaciones {
            let tipo_valor = operacion.get("tipo").cloned();
            let tipo = tipo_valor.as_ref().and_then(Value::as_str);
            let payload = operacion.get("payload").cloned().unwrap_or(Value::Null);
            let id = payload.get("id").cloned();

            let Some(tipo) = tipo else {
                let mut resultado = Map::new();
                if let Some(tipo) = tipo_valor {
                    if !tipo.is_null() {
                        resultado.insert("tipo".into(), tipo);
                    }
                }
                if let Some(id) = id {
                    resultado.insert("id".into(), id);
                }
                resultado.insert("ok".into(), Value::Bool(false));
                resultado.insert(
                    "error".into(),
                    Value::String("tipo de entidad no soportado".into()),
                );
                resultados.push(Value::Object(resultado));
                continue;
            };
            let Some(tabla) = tabla_sync(tipo) else {
                let mut resultado = Map::new();
                resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                if let Some(id) = id {
                    resultado.insert("id".into(), id);
                }
                resultado.insert("ok".into(), Value::Bool(false));
                resultado.insert(
                    "error".into(),
                    Value::String("tipo de entidad no soportado".into()),
                );
                resultados.push(Value::Object(resultado));
                continue;
            };

            match self
                .insertar(tabla, tenant, id.as_ref(), &payload, true)
                .await
            {
                Ok(fila) => {
                    let mut resultado = Map::new();
                    resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                    resultado.insert("id".into(), fila.get("id").cloned().unwrap_or(Value::Null));
                    resultado.insert("ok".into(), Value::Bool(true));
                    resultados.push(Value::Object(resultado));
                }
                Err(error) => {
                    let mut resultado = Map::new();
                    resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                    if let Some(id) = id {
                        resultado.insert("id".into(), id);
                    }
                    resultado.insert("ok".into(), Value::Bool(false));
                    resultado.insert("error".into(), Value::String(error.mensaje));
                    resultados.push(Value::Object(resultado));
                }
            }
        }
        crate::logs::info(
            "field",
            crate::campos! {
                "msg": "sync completado",
                "tenant": tenant,
                "operaciones": operaciones.len(),
            },
        );
        resultados
    }
}
