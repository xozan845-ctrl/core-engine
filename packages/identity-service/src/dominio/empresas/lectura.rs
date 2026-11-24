use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    /// Lectura interna por id (sin autorización de miembro).
    pub async fn obtener(&self, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(empresa_id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e WHERE e.id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| {
            let datos: Value = fila.try_get("datos").map_err(interno)?;
            Ok(Self::con_urls(&datos))
        })
        .transpose()
    }

    /// Overrides crudos de tema para que stores resuelva la herencia (ADR-42).
    pub async fn tema_overrides(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT plantilla_override, tema_override FROM identity.empresas WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(json!({ "plantilla_override": Value::Null, "tema_override": {} }));
        };
        let plantilla: Option<String> = fila.try_get("plantilla_override").map_err(interno)?;
        let tema: Value = fila.try_get("tema_override").map_err(interno)?;
        Ok(json!({
            "plantilla_override": plantilla_de(plantilla.as_ref().map(|valor| json!(valor)).as_ref()),
            "tema_override": tema_parcial_de(&tema),
        }))
    }
}
