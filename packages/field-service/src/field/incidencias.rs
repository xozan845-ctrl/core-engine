use super::*;

impl FieldService {
    // ── incidencias ───────────────────────────────────────────────────────
    pub async fn listar_incidencias(
        &self,
        tenant: &str,
        estado: Option<&str>,
        ruta_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(estado) = estado {
            filtros.push(Filtro::texto("estado", estado));
        }
        if let Some(ruta_id) = ruta_id {
            filtros.push(Filtro::uuid("ruta_id", ruta_id));
        }
        self.listar(&TABLA_INCIDENCIAS, tenant, filtros, "created_at")
            .await
    }

    pub async fn obtener_incidencia(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_INCIDENCIAS, tenant, id).await
    }

    pub async fn crear_incidencia(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_INCIDENCIAS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_incidencia(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_INCIDENCIAS, tenant, id, cuerpo)
            .await
    }

    pub async fn eliminar_incidencia(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_INCIDENCIAS, tenant, id).await
    }
}
