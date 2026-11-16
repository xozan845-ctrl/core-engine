use super::*;

impl FieldService {
    pub async fn listar_rutas(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut rutas = match personal_id {
            Some(personal_id) => {
                let sql = "SELECT * FROM field.rutas WHERE tenant_id = $1::uuid AND personal_asignado_ids ?| array[$2] AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 2000";
                self.ejecutar_filas(
                    sql,
                    vec![
                        Valor::Texto(tenant.to_string()),
                        Valor::Texto(personal_id.to_string()),
                    ],
                )
                .await?
            }
            None => {
                self.listar(&TABLA_RUTAS, tenant, Vec::new(), "created_at")
                    .await?
            }
        };

        let paradas = self
            .ejecutar_filas(
                "SELECT * FROM field.paradas WHERE tenant_id = $1::uuid AND deleted_at IS NULL",
                vec![Valor::Texto(tenant.to_string())],
            )
            .await?;
        let mut por_ruta: HashMap<String, Vec<Value>> = HashMap::new();
        for parada in paradas {
            if let Some(ruta_id) = parada.get("ruta_id").and_then(Value::as_str) {
                por_ruta
                    .entry(ruta_id.to_string())
                    .or_default()
                    .push(parada);
            }
        }
        for ruta in &mut rutas {
            let id = ruta
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let mut paradas = por_ruta.remove(&id).unwrap_or_default();
            ordenar_paradas(&mut paradas);
            if let Some(objeto) = ruta.as_object_mut() {
                objeto.insert("paradas".into(), Value::Array(paradas));
            }
        }
        Ok(rutas)
    }

    pub async fn obtener_ruta(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        let mut ruta = self.obtener(&TABLA_RUTAS, tenant, id).await?;
        let mut paradas = self
            .ejecutar_filas(
                "SELECT * FROM field.paradas WHERE tenant_id = $1::uuid AND ruta_id = $2::uuid AND deleted_at IS NULL",
                vec![Valor::Texto(tenant.to_string()), Valor::Texto(id.to_string())],
            )
            .await?;
        ordenar_paradas(&mut paradas);
        if let Some(objeto) = ruta.as_object_mut() {
            objeto.insert("paradas".into(), Value::Array(paradas));
        }
        Ok(ruta)
    }

    pub async fn crear_ruta(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_RUTAS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_ruta(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_RUTAS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_ruta(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_RUTAS, tenant, id).await
    }

    pub async fn agregar_parada(
        &self,
        tenant: &str,
        ruta_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.obtener(&TABLA_RUTAS, tenant, ruta_id).await?;
        let mut objeto = cuerpo.as_object().cloned().unwrap_or_default();
        objeto.insert("rutaId".into(), Value::String(ruta_id.to_string()));
        self.insertar(&TABLA_PARADAS, tenant, None, &Value::Object(objeto), false)
            .await
    }

    pub async fn actualizar_parada(
        &self,
        tenant: &str,
        ruta_id: &str,
        parada_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let parada = self
            .ejecutar_uno(
                "SELECT id, ruta_id FROM field.paradas WHERE tenant_id = $1::uuid AND id = $2::uuid AND deleted_at IS NULL",
                vec![
                    Valor::Texto(tenant.to_string()),
                    Valor::Texto(parada_id.to_string()),
                ],
            )
            .await?
            .ok_or_else(|| ApiError::no_encontrado("parada", parada_id))?;
        let ruta_actual = parada
            .get("ruta_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if ruta_actual != ruta_id {
            return Err(ApiError::parada_invalida(
                "La parada no pertenece a la ruta indicada.",
            ));
        }
        self.actualizar(&TABLA_PARADAS, tenant, parada_id, cuerpo)
            .await
    }

    pub async fn asignar_ruta(
        &self,
        tenant: &str,
        ruta_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let objeto = cuerpo.as_object();
        let mut sets: Vec<(String, TipoColumna, Valor)> = Vec::new();
        if let Some(valor) = objeto.and_then(|objeto| objeto.get("personalIds")) {
            sets.push((
                format!(
                    "personal_asignado_ids = ${}{}",
                    sets.len() + 1,
                    cast(TipoColumna::Jsonb)
                ),
                TipoColumna::Jsonb,
                Valor::desde_json(valor, TipoColumna::Jsonb),
            ));
        }
        if let Some(valor) = objeto.and_then(|objeto| objeto.get("vehiculoId")) {
            sets.push((
                format!(
                    "vehiculo_asignado_id = ${}{}",
                    sets.len() + 1,
                    cast(TipoColumna::Uuid)
                ),
                TipoColumna::Uuid,
                Valor::desde_json(valor, TipoColumna::Uuid),
            ));
        }
        let indice_tenant = sets.len() + 1;
        let indice_id = sets.len() + 2;
        let mut sql = String::from("UPDATE field.rutas SET ");
        if !sets.is_empty() {
            sql.push_str(
                &sets
                    .iter()
                    .map(|(set, _, _)| set.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "updated_at = NOW() WHERE tenant_id = ${indice_tenant}::uuid AND id = ${indice_id}::uuid RETURNING *"
        ));
        let mut valores = sets
            .into_iter()
            .map(|(_, _, valor)| valor)
            .collect::<Vec<_>>();
        valores.push(Valor::Texto(tenant.to_string()));
        valores.push(Valor::Texto(ruta_id.to_string()));
        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("ruta", ruta_id))
    }

    // ── pedidos ───────────────────────────────────────────────────────────
}
