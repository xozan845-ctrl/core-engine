use super::helpers::*;
use super::*;

impl Agenda {
    // ── Recursos ────────────────────────────────────────────────────────────

    pub async fn crear_recurso(
        &self,
        empresa_id: &str,
        dto: &CrearRecurso,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO scheduling.recursos (id, empresa_id, tipo, nombre, capacidad, atributos)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb) RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(&dto.tipo)
        .bind(dto.nombre.trim())
        .bind(dto.capacidad)
        .bind(sqlx::types::Json(&dto.atributos))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_recurso(&fila)
    }

    pub async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT * FROM scheduling.recursos
             WHERE empresa_id = $1 ORDER BY estado ASC, nombre ASC",
        )
        .bind(empresa)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recurso).collect()
    }

    // ── Disponibilidad ──────────────────────────────────────────────────────

    pub async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        recurso_de_empresa(&mut tx, empresa, recurso).await?;
        let fila = sqlx::query(
            "INSERT INTO scheduling.disponibilidad
               (id, empresa_id, recurso_id, dia_semana, inicia_hora, termina_hora,
                zona_horaria, vigente_desde, vigente_hasta)
             VALUES ($1, $2, $3, $4, $5::time, $6::time, $7, COALESCE($8::date, CURRENT_DATE), $9::date)
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(recurso)
        .bind(dto.dia_semana as i16)
        .bind(&dto.inicia_hora)
        .bind(&dto.termina_hora)
        .bind(dto.zona_horaria.as_deref().unwrap_or("America/Managua"))
        .bind(dto.vigente_desde.as_deref())
        .bind(dto.vigente_hasta.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_disponibilidad(&fila)
    }

    pub async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(recurso_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT * FROM scheduling.disponibilidad
             WHERE empresa_id = $1 AND recurso_id = $2
             ORDER BY dia_semana, inicia_hora",
        )
        .bind(empresa)
        .bind(recurso)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_disponibilidad).collect()
    }

    // ── Bloqueos ────────────────────────────────────────────────────────────

    pub async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        let actor = match actor_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        recurso_de_empresa_bloqueado(&mut tx, empresa, recurso).await?;
        let resultado = sqlx::query(
            "INSERT INTO scheduling.bloqueos
               (id, empresa_id, recurso_id, periodo, motivo, creado_por)
             VALUES ($1, $2, $3, tstzrange($4::timestamptz, $5::timestamptz, '[)'), $6, $7)
             RETURNING id, empresa_id, recurso_id, lower(periodo) AS inicia_en,
                       upper(periodo) AS termina_en, motivo, creado_en",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .bind(dto.motivo.as_deref().unwrap_or(""))
        .bind(actor)
        .fetch_one(&mut *tx)
        .await;

        let fila = match resultado {
            Ok(fila) => fila,
            Err(error) if es_conflicto_periodo(&error) => {
                return Err(ApiError::recurso_no_disponible(
                    "El recurso ya tiene un bloqueo en ese horario.",
                ))
            }
            Err(error) => return Err(error_db(error)),
        };
        tx.commit().await.map_err(error_db)?;
        fila_bloqueo(&fila)
    }

    // ── Disponibilidad interna (orders) ─────────────────────────────────────

    pub async fn verificar_disponibilidad(
        &self,
        dto: &VerificarDisponibilidad,
    ) -> Result<(), ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(&dto.empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;

        let mut tx = self.db.transaccion().await.map_err(error_db)?;

        let recurso_activo = sqlx::query(
            "SELECT capacidad FROM scheduling.recursos
              WHERE id = $1 AND empresa_id = $2 AND estado = 'activo'",
        )
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(fila_recurso) = recurso_activo else {
            return Err(ApiError::no_encontrado("Recurso", &dto.recurso_id));
        };
        let capacidad: i64 = i64::from(
            fila_recurso
                .try_get::<i32, _>("capacidad")
                .map_err(error_db)?,
        );

        let ventana = sqlx::query(
            "SELECT 1 FROM scheduling.disponibilidad d
              WHERE d.recurso_id = $1
                AND d.dia_semana = EXTRACT(DOW FROM ($2::timestamptz AT TIME ZONE d.zona_horaria))::int
                AND d.inicia_hora <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::time)
                AND d.termina_hora >= (($3::timestamptz AT TIME ZONE d.zona_horaria)::time)
                AND d.vigente_desde <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date)
                AND (d.vigente_hasta IS NULL OR d.vigente_hasta >= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date))
              LIMIT 1",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if ventana.is_none() {
            return Err(ApiError::recurso_no_disponible(
                "El recurso no tiene disponibilidad en ese horario.",
            ));
        }

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
                "El recurso esta bloqueado en ese horario.",
            ));
        }

        let ocupadas: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM scheduling.citas
              WHERE recurso_id = $1
                AND estado IN ('reservada', 'confirmada', 'en_atencion')
                AND periodo && tstzrange($2::timestamptz, $3::timestamptz, '[)')",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        if ocupadas >= capacidad {
            return Err(ApiError::recurso_no_disponible(
                "El recurso alcanzo su capacidad en ese horario.",
            ));
        }

        tx.commit().await.map_err(error_db)?;
        Ok(())
    }
}
