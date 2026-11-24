use super::prelude::*;
use super::*;

impl Facturacion {
    pub async fn listar(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
        estado: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<Vec<Comprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                    iva_cents, exento_cents, retencion_cents, retencion_tasa_bps, total_cents,
                    moneda, estado, datos_cliente, emitido_en, creado_en,
                    electronica_estado, electronica_referencia, electronica_enviada_en
             FROM finance.comprobantes
             WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
               AND ($2::timestamptz IS NULL OR creado_en <= $2)
               AND ($3::text IS NULL OR estado = $3)
               AND ($4::uuid IS NULL OR negocio_id = $4::uuid)
             ORDER BY creado_en DESC
             LIMIT 300",
        )
        .bind(desde)
        .bind(hasta)
        .bind(estado)
        .bind(negocio_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(comprobante_de_fila).collect())
    }

    /// Indica si el tenant tiene PDF habilitado y devuelve el comprobante.
    pub async fn pdf_de(
        &self,
        id: &str,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<Vec<u8>>, ApiError> {
        let habilitado: Option<bool> = sqlx::query_scalar(
            "SELECT formato_pdf FROM finance.configuracion_fiscal
              WHERE empresa_id = $1::uuid AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if habilitado != Some(true) {
            return Err(ApiError::dominio(
                "PDF_NO_ACTIVADO",
                "La facturación PDF no está activada para este negocio.",
            ));
        }
        let comprobante_id = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let guardado: Option<(Option<Vec<u8>>, Option<uuid::Uuid>)> = sqlx::query_as(
            "SELECT contenido, objeto_id FROM finance.comprobante_pdf WHERE comprobante_id = $1",
        )
        .bind(comprobante_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some((contenido, objeto_id)) = &guardado {
            if let Some(pdf) = contenido {
                return Ok(Some(pdf.clone()));
            }
            if let (Some(objeto_id), Some(almacen)) = (objeto_id, self.pdf_almacen.as_ref()) {
                if let Ok(pdf) = almacen.leer(empresa_id, &objeto_id.to_string()).await {
                    return Ok(Some(pdf));
                }
            }
        }
        let comprobante = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Comprobante", id))?;
        let pdf = crate::pdf::comprobante_pdf(&comprobante);
        let mut objeto_id = None;
        let mut contenido = Some(pdf.clone());
        if let Some(almacen) = self.pdf_almacen.as_ref() {
            let nombre = format!("comprobante-{}.pdf", comprobante.numero);
            if let Ok(id) = almacen.subir(empresa_id, &nombre, &pdf).await {
                if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                    objeto_id = Some(uuid);
                    contenido = None;
                }
            }
        }
        sqlx::query(
            "INSERT INTO finance.comprobante_pdf (comprobante_id, contenido, objeto_id)
             VALUES ($1, $2, $3) ON CONFLICT (comprobante_id) DO NOTHING",
        )
        .bind(comprobante_id)
        .bind(&contenido)
        .bind(objeto_id)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        // En una carrera, devolvemos la copia persistida canónica.
        let guardado: Option<(Option<Vec<u8>>, Option<uuid::Uuid>)> = sqlx::query_as(
            "SELECT contenido, objeto_id FROM finance.comprobante_pdf WHERE comprobante_id = $1",
        )
        .bind(comprobante_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some((Some(pdf), _)) = guardado {
            return Ok(Some(pdf));
        }
        if let Some((_, Some(objeto_id))) = guardado {
            if let Some(almacen) = self.pdf_almacen.as_ref() {
                if let Ok(pdf) = almacen.leer(empresa_id, &objeto_id.to_string()).await {
                    return Ok(Some(pdf));
                }
            }
        }
        Ok(Some(pdf))
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Comprobante>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                    base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                    estado, datos_cliente, emitido_en, creado_en,
                    electronica_estado, electronica_referencia, electronica_enviada_en
               FROM finance.comprobantes WHERE id = $1::uuid",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(comprobante_de_fila))
    }

    /// Anula un comprobante (auditoría: el número emitido no se reutiliza).
    pub async fn anular(&self, id: &str) -> Result<Option<Comprobante>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.comprobantes SET estado = 'ANULADO'
             WHERE id = $1 AND estado IN ('BORRADOR', 'EMITIDO')
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(comprobante_de_fila))
    }
}
