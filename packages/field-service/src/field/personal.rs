use super::*;

impl FieldService {
    pub async fn listar_personal(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_PERSONAL, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_personal(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PERSONAL, tenant, id).await
    }

    pub async fn crear_personal(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_PERSONAL, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_personal(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_PERSONAL, tenant, id, cuerpo).await
    }

    pub async fn eliminar_personal(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_PERSONAL, tenant, id).await
    }

    pub async fn ubicacion_personal(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        let fila = self.obtener(&TABLA_PERSONAL, tenant, id).await?;
        Ok(json!({
            "personalId": id,
            "latitud": fila.get("ubicacion_lat").cloned().unwrap_or(Value::Null),
            "longitud": fila.get("ubicacion_lng").cloned().unwrap_or(Value::Null),
            "precision": fila.get("ubicacion_precision").cloned().unwrap_or(Value::Null),
            "timestamp": fila.get("ubicacion_ts").cloned().unwrap_or(Value::Null),
        }))
    }

    pub async fn actualizar_ubicacion(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PERSONAL, tenant, id).await?;
        self.actualizar_con_map(&TABLA_PERSONAL, MAP_UBICACION, tenant, id, cuerpo)
            .await
    }

    // ── clientes ──────────────────────────────────────────────────────────
}
