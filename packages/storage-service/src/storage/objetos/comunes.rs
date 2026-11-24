use super::*;

impl Objetos {
    pub(super) async fn obtener_fila(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<ObjetoStorage, ApiError> {
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_uuid = Uuid::parse_str(id).map_err(|_| ApiError::no_encontrado("Objeto", id))?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_uuid)
        .bind(objeto_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        serializar(&fila)
    }

    pub(super) fn expiracion(&self) -> String {
        (Utc::now() + Duration::seconds(self.config.url_ttl_segundos as i64))
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    }
}
