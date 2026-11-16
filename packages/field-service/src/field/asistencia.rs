use super::*;

impl FieldService {
    pub async fn listar_asistencia(
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
                "DATE(registrado_en AT TIME ZONE 'America/Managua')",
                fecha,
            ));
        }
        self.listar(&TABLA_ASISTENCIA, tenant, filtros, "created_at")
            .await
    }

    pub async fn registrar_asistencia(
        &self,
        tenant: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.insertar(&TABLA_ASISTENCIA, tenant, None, cuerpo, false)
            .await
    }
}
