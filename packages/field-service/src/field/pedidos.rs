use super::*;

impl FieldService {
    pub async fn listar_pedidos(
        &self,
        tenant: &str,
        estado: Option<&str>,
        ruta_id: Option<&str>,
        parte_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(estado) = estado {
            filtros.push(Filtro::texto("estado", estado));
        }
        if let Some(ruta_id) = ruta_id {
            filtros.push(Filtro::uuid("ruta_id", ruta_id));
        }
        if let Some(parte_id) = parte_id {
            filtros.push(Filtro::uuid("parte_id", parte_id));
        }
        self.listar(&TABLA_PEDIDOS, tenant, filtros, "created_at")
            .await
    }

    pub async fn obtener_pedido(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PEDIDOS, tenant, id).await
    }

    pub async fn crear_pedido(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_PEDIDOS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_pedido(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_PEDIDOS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_pedido(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_PEDIDOS, tenant, id).await
    }

    pub async fn cambiar_estado_pedido(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let estado = cuerpo.get("estado").cloned().unwrap_or(Value::Null);
        self.actualizar(&TABLA_PEDIDOS, tenant, id, &json!({ "estado": estado }))
            .await
    }

    // ── asistencia ────────────────────────────────────────────────────────
}
