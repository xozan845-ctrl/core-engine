use super::prelude::*;
use super::*;

impl Facturacion {
    /// Exporta los datos fiscales del tenant. No incluye bytes de PDF.
    pub async fn exportar_tenant(
        &self,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Result<Value, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
        let negocios: Vec<uuid::Uuid> = negocio_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let usuarios: Vec<uuid::Uuid> = usuario_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let mut datos = Map::new();
        let comprobantes: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.creado_en), '[]'::jsonb)
               FROM finance.comprobantes c WHERE c.empresa_id = $1",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("comprobantes".into(), comprobantes);
        let pdfs: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                        'comprobante_id', p.comprobante_id,
                        'objeto_id', p.objeto_id,
                        'generado_en', p.generado_en,
                        'contenido_base64', encode(p.contenido, 'base64')
                    )), '[]'::jsonb)
               FROM finance.comprobante_pdf p
              WHERE p.comprobante_id IN (
                  SELECT id FROM finance.comprobantes WHERE empresa_id = $1
              )",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("comprobantes_pdf".into(), pdfs);
        for (nombre, sql) in [
            (
                "configuracion_fiscal",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.configuracion_fiscal t WHERE t.empresa_id = $1",
            ),
            (
                "credito_cuentas",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.credito_cuentas t WHERE t.empresa_id = $1",
            ),
            (
                "workspace_registros",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.workspace_registros t WHERE t.tenant_id = $1",
            ),
        ] {
            let valor: Value = sqlx::query_scalar(sql)
                .bind(empresa)
                .fetch_one(self.db.pool())
                .await
                .map_err(error_db)?;
            datos.insert(nombre.into(), valor);
        }
        let series: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM finance.series_comprobantes t WHERE t.negocio_id = ANY($1::uuid[])",
        )
        .bind(&negocios)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("series_comprobantes".into(), series);
        let asientos: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.creado_en), '[]'::jsonb)
               FROM finance.asientos a
              WHERE a.referencia_tipo = 'comprobante'
                AND a.referencia_id IN (
                    SELECT id::text FROM finance.comprobantes WHERE empresa_id = $1
                )",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("asientos".into(), asientos);
        let metricas: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM finance.metricas_vendedores t WHERE t.vendedor_id = ANY($1::uuid[])",
        )
        .bind(&usuarios)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("metricas_vendedores".into(), metricas);
        Ok(Value::Object(datos))
    }

    /// Anonimiza los datos personales del tenant en finanzas y elimina su
    /// configuración fiscal. Los comprobantes y asientos se conservan por
    /// retención contable.
    pub async fn purgar_tenant(
        &self,
        empresa_id: &str,
        _negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Result<Value, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
        let usuarios: Vec<uuid::Uuid> = usuario_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        sqlx::query(
            "UPDATE finance.comprobantes
                SET datos_cliente = jsonb_build_object('redactado', true),
                    parte_id = NULL
              WHERE empresa_id = $1",
        )
        .bind(empresa)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query(
            "DELETE FROM finance.comprobante_pdf
              WHERE comprobante_id IN (
                  SELECT id FROM finance.comprobantes WHERE empresa_id = $1
              )",
        )
        .bind(empresa)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.configuracion_fiscal WHERE empresa_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.credito_cuentas WHERE empresa_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.workspace_registros WHERE tenant_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        // Las series se conservan: los comprobantes retenidos las referencian.
        sqlx::query("DELETE FROM finance.metricas_vendedores WHERE vendedor_id = ANY($1::uuid[])")
            .bind(&usuarios)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(json!({ "empresa_id": empresa_id, "anonimizado": true }))
    }
}
