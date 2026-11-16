use super::*;

impl FieldService {
    pub async fn listar_vehiculos(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_VEHICULOS, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_vehiculo(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_VEHICULOS, tenant, id).await
    }

    pub async fn crear_vehiculo(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_VEHICULOS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_vehiculo(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_VEHICULOS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_vehiculo(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_VEHICULOS, tenant, id).await
    }

    // ── rutas ─────────────────────────────────────────────────────────────
}
