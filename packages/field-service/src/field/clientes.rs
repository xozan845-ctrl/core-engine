use super::*;

impl FieldService {
    pub async fn listar_clientes(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_CLIENTES, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_cliente(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_CLIENTES, tenant, id).await
    }

    pub async fn crear_cliente(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_CLIENTES, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_cliente(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_CLIENTES, tenant, id, cuerpo).await
    }

    pub async fn eliminar_cliente(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_CLIENTES, tenant, id).await
    }

    // ── vehículos ─────────────────────────────────────────────────────────
}
