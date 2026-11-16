use super::helpers::*;
use super::*;

impl Agenda {
    pub async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        match self.partes.pertenece(empresa_id, &dto.parte_id).await {
            Err(()) => return Err(ApiError::servicio_no_disponible("identity-service")),
            Ok(false) => {
                return Err(ApiError::dominio(
                    "PARTE_INVALIDA",
                    "La parte no pertenece al tenant activo.",
                ))
            }
            Ok(true) => {}
        }
        let order_id = dto.order_id.as_deref().map(parsear_uuid).transpose()?;
        let item_id = dto.item_id.as_deref().map(parsear_uuid).transpose()?;

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let recurso_activo = sqlx::query(
            "SELECT id FROM scheduling.recursos
             WHERE id = $1 AND empresa_id = $2 AND estado = 'activo' FOR UPDATE",
        )
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if recurso_activo.is_none() {
            return Err(ApiError::no_encontrado("Recurso", &dto.recurso_id));
        }
        validar_disponibilidad_tx(&mut tx, recurso, &dto.inicia_en, &dto.termina_en).await?;
        let bloqueo = sqlx::query(
            "SELECT 1 FROM scheduling.bloqueos
             WHERE recurso_id = $1
               AND periodo && tstzrange($2::timestamptz, $3::timestamptz, '[)') LIMIT 1",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if bloqueo.is_some() {
            return Err(ApiError::recurso_no_disponible(
                "El recurso está bloqueado en ese horario.",
            ));
        }

        let id = Uuid::new_v4();
        let cita = sqlx::query(
            "INSERT INTO scheduling.citas
               (id, empresa_id, recurso_id, parte_id, order_id, item_id,
                inicia_en, termina_en, notas, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9, $10::jsonb)
             RETURNING id, empresa_id, recurso_id, parte_id, order_id, item_id,
                       estado, inicia_en, termina_en, notas, metadata, creado_en, actualizado_en",
        )
        .bind(id)
        .bind(empresa)
        .bind(recurso)
        .bind(parsear_uuid(&dto.parte_id)?)
        .bind(order_id)
        .bind(item_id)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .bind(dto.notas.as_deref().unwrap_or(""))
        .bind(sqlx::types::Json(&dto.metadata))
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if es_conflicto_periodo(&error) {
                ApiError::recurso_no_disponible("El recurso ya está reservado en ese horario.")
            } else {
                error_db(error)
            }
        })?;

        crear_recordatorios(&mut tx, empresa, id, &dto.inicia_en).await?;

        let mut data = json!({
            "cita_id": uuid_texto(&cita, "id").map_err(error_db)?,
            "empresa_id": uuid_texto(&cita, "empresa_id").map_err(error_db)?,
            "recurso_id": uuid_texto(&cita, "recurso_id").map_err(error_db)?,
        });
        if let Some(parte) = uuid_opcional(&cita, "parte_id").map_err(error_db)? {
            data["cliente_parte_id"] = json!(parte);
        }
        if let Some(order) = uuid_opcional(&cita, "order_id").map_err(error_db)? {
            data["order_id"] = json!(order);
        }
        if let Some(item) = uuid_opcional(&cita, "item_id").map_err(error_db)? {
            data["item_id"] = json!(item);
        }
        data["inicia_en"] = json!(timestamp(&cita, "inicia_en").map_err(error_db)?);
        data["termina_en"] = json!(timestamp(&cita, "termina_en").map_err(error_db)?);
        data["estado"] = json!(cita.try_get::<String, _>("estado").map_err(error_db)?);
        self.outbox
            .insertar_en_transaccion(&mut tx, APPOINTMENT_CREATED, data)
            .await?;

        tx.commit().await.map_err(error_db)?;
        fila_cita(&cita)
    }

    pub async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                    inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
             FROM scheduling.citas
             WHERE empresa_id = $1
               AND ($2::timestamptz IS NULL OR termina_en > $2::timestamptz)
               AND ($3::timestamptz IS NULL OR inicia_en < $3::timestamptz)
             ORDER BY inicia_en ASC LIMIT 500",
        )
        .bind(empresa)
        .bind(desde)
        .bind(hasta)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_cita).collect()
    }

    pub async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let cita = parsear_uuid(cita_id)?;
        let estados: Vec<String> = ESTADOS_ACTIVOS.iter().map(|e| e.to_string()).collect();

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE scheduling.citas
                SET estado = 'cancelada', actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2
                AND estado = ANY($3::text[])
             RETURNING id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                       inicia_en, termina_en, notas, metadata, creado_en, actualizado_en",
        )
        .bind(cita)
        .bind(empresa)
        .bind(&estados)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Cita activa", cita_id))?;

        sqlx::query(
            "UPDATE scheduling.recordatorios SET estado = 'cancelado'
             WHERE cita_id = $1 AND estado = 'pendiente'",
        )
        .bind(cita)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        let mut data = json!({
            "cita_id": uuid_texto(&fila, "id").map_err(error_db)?,
            "empresa_id": uuid_texto(&fila, "empresa_id").map_err(error_db)?,
        });
        if let Some(order) = uuid_opcional(&fila, "order_id").map_err(error_db)? {
            data["order_id"] = json!(order);
        }
        if let Some(motivo) = motivo.filter(|valor| !valor.is_empty()) {
            data["motivo"] = json!(motivo);
        }
        self.outbox
            .insertar_en_transaccion(&mut tx, APPOINTMENT_CANCELLED, data)
            .await?;

        tx.commit().await.map_err(error_db)?;
        fila_cita(&fila)
    }
}
