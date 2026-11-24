use super::prelude::*;
use super::*;

impl Tributacion {
    /// Periodo mensual (inicio día 1, fin último día) de una fecha.
    pub fn periodo_del_mes(fecha: NaiveDate) -> PeriodoMensual {
        let inicio = NaiveDate::from_ymd_opt(fecha.year(), fecha.month(), 1).unwrap_or(fecha);
        let fin = if fecha.month() == 12 {
            NaiveDate::from_ymd_opt(fecha.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(fecha.year(), fecha.month() + 1, 1)
        }
        .and_then(|siguiente| siguiente.pred_opt())
        .unwrap_or(fecha);
        PeriodoMensual {
            inicio: inicio.format("%Y-%m-%d").to_string(),
            fin: fin.format("%Y-%m-%d").to_string(),
        }
    }

    /// Mes anterior al recibido: es el corte que genera la declaración.
    pub fn mes_anterior(fecha: NaiveDate) -> PeriodoMensual {
        let primero = NaiveDate::from_ymd_opt(fecha.year(), fecha.month(), 1).unwrap_or(fecha);
        let previo = primero.pred_opt().unwrap_or(fecha);
        Self::periodo_del_mes(previo)
    }

    pub async fn declaraciones(
        &self,
        filtros: FiltrosDeclaracion,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, empresa_id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                    presentada_en, creado_en
             FROM finance.declaraciones
             WHERE ($1::uuid IS NULL OR empresa_id = $1::uuid)
               AND ($2::text IS NULL OR tipo = $2)
               AND ($3::date IS NULL OR periodo_inicio = $3::date)
               AND ($4::text IS NULL OR estado = $4)
             ORDER BY periodo_inicio DESC, tipo LIMIT 300",
        )
        .bind(filtros.empresa_id.as_deref())
        .bind(filtros.tipo.as_deref())
        .bind(filtros.periodo_inicio.as_deref())
        .bind(filtros.estado.as_deref())
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(declaracion_de_fila).collect())
    }

    pub async fn generar_declaraciones_del_mes_anterior(
        &self,
        fecha: NaiveDate,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let mes = Self::mes_anterior(fecha);
        let mut generadas = self.generar_declaraciones(Some(mes.clone()), None).await?;
        generadas.extend(self.generar_declaraciones_tenants(Some(mes)).await?);
        Ok(generadas)
    }

    /// Genera las declaraciones de todos los tenants con comprobantes emitidos
    /// en el periodo. Cada tenant usa su propia jurisdicción y series.
    pub async fn generar_declaraciones_tenants(
        &self,
        periodo: Option<PeriodoMensual>,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let mes = periodo.unwrap_or_else(|| Self::mes_anterior(Local::now().date_naive()));
        let filas = sqlx::query(
            "SELECT DISTINCT c.empresa_id::text AS empresa_id
               FROM finance.comprobantes c
              WHERE c.empresa_id IS NOT NULL
                AND c.estado = 'EMITIDO'
                AND c.creado_en >= $1::date
                AND c.creado_en < ($2::date + INTERVAL '1 day')
              ORDER BY 1",
        )
        .bind(&mes.inicio)
        .bind(&mes.fin)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        let mut generadas = Vec::new();
        for fila in filas {
            let empresa_id: String = fila.try_get("empresa_id").map_err(error_db)?;
            match self
                .generar_declaraciones(Some(mes.clone()), Some(&empresa_id))
                .await
            {
                Ok(declaraciones) => generadas.extend(declaraciones),
                // Un tenant legacy sin región no debe frenar al resto.
                Err(error) if error.codigo == "SIN_JURISDICCION" => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(generadas)
    }

    /// Genera IVA e IR (más CUOTA_FIJA informativa) del periodo.
    ///
    /// Con `empresa_id` la base sale de los comprobantes emitidos del tenant
    /// (IVA real por línea); sin él se conserva la declaración de plataforma
    /// sobre el libro mayor. Idempotente por
    /// `(empresa_id, jurisdiccion, tipo, periodo_inicio)`.
    pub async fn generar_declaraciones(
        &self,
        periodo: Option<PeriodoMensual>,
        empresa_id: Option<&str>,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let mes = periodo.unwrap_or_else(|| Self::mes_anterior(Local::now().date_naive()));

        let (jurisdiccion, base_gravada, exento, gastos, iva, fuente, hay_datos) = match empresa_id
        {
            Some(empresa) => {
                let jurisdiccion = self.jurisdiccion_de(empresa, None).await?.ok_or_else(|| {
                    ApiError::dominio(
                        "SIN_JURISDICCION",
                        "La empresa no tiene una región tributaria configurada.",
                    )
                })?;
                let fila = sqlx::query(
                    "SELECT COALESCE(SUM(c.base_gravada_cents), 0)::bigint AS base,
                            COALESCE(SUM(c.exento_cents), 0)::bigint AS exento,
                            COALESCE(SUM(c.iva_cents), 0)::bigint AS iva,
                            COUNT(*)::bigint AS comprobantes
                       FROM finance.comprobantes c
                       JOIN finance.series_comprobantes s ON s.serie = c.serie
                      WHERE c.empresa_id = $1::uuid
                        AND c.estado = 'EMITIDO'
                        AND s.jurisdiccion = $2
                        AND c.creado_en >= $3::date
                        AND c.creado_en < ($4::date + INTERVAL '1 day')",
                )
                .bind(empresa)
                .bind(&jurisdiccion.codigo_pais)
                .bind(&mes.inicio)
                .bind(&mes.fin)
                .fetch_one(self.db.pool())
                .await
                .map_err(error_db)?;
                let gastos: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(SUM((datos->>'amountCents')::bigint), 0)::bigint
                       FROM finance.workspace_registros
                      WHERE recurso = 'gastos'
                        AND tenant_id = $1::uuid
                        AND eliminado_en IS NULL
                        AND (datos->>'date')::date >= $2::date
                        AND (datos->>'date')::date <= $3::date",
                )
                .bind(empresa)
                .bind(&mes.inicio)
                .bind(&mes.fin)
                .fetch_one(self.db.pool())
                .await
                .map_err(error_db)?;
                (
                    jurisdiccion,
                    fila.try_get::<i64, _>("base").unwrap_or(0),
                    fila.try_get::<i64, _>("exento").unwrap_or(0),
                    gastos,
                    fila.try_get::<i64, _>("iva").unwrap_or(0),
                    json!({
                        "fuente": "comprobantes+gastos",
                        "detalle": "IVA de comprobantes EMITIDOS y gastos del tenant en el periodo.",
                    }),
                    fila.try_get::<i64, _>("comprobantes").unwrap_or(0) > 0,
                )
            }
            None => {
                let jurisdiccion = self.jurisdiccion_activa().await?.ok_or_else(|| {
                    ApiError::dominio(
                        "SIN_JURISDICCION",
                        "No hay jurisdiccion activa configurada.",
                    )
                })?;
                let inicio = fecha_utc(&mes.inicio)?;
                let fin = fecha_utc(&mes.fin)?;
                let ingresos = self.contabilidad.ingresos_del_periodo(inicio, fin).await?;
                let gastos = self.contabilidad.gastos_del_periodo(inicio, fin).await?;
                let iva = (ingresos as f64 * jurisdiccion.tasa_iva).round() as i64;
                (
                    jurisdiccion,
                    ingresos,
                    0,
                    gastos,
                    iva,
                    json!({
                        "fuente": "libro_mayor",
                        "detalle": "Declaracion de plataforma; el libro mayor no separa por tenant.",
                    }),
                    ingresos != 0 || gastos != 0,
                )
            }
        };

        let ingresos = base_gravada + exento;
        let ir_base = (ingresos - gastos).max(0);
        let ir = (ir_base as f64 * jurisdiccion.tasa_ir).round() as i64;

        let plan = vec![
            (
                "IVA",
                json!({
                    "base_gravada_cents": base_gravada,
                    "iva_debitado_cents": iva,
                    "iva_credito_cents": 0,
                    "iva_a_pagar_cents": iva,
                }),
                json!({
                    "tasa": jurisdiccion.tasa_iva,
                    "exento_cents": exento,
                    "ley": jurisdiccion
                        .leyes
                        .get("iva")
                        .and_then(Value::as_str)
                        .unwrap_or("Ley 822"),
                    "origen": fuente,
                }),
            ),
            (
                "IR",
                json!({
                    "renta_bruta_cents": ingresos,
                    "renta_gravable_cents": ir_base,
                    "ir_a_pagar_cents": ir,
                }),
                json!({
                    "tasa": jurisdiccion.tasa_ir,
                    "gastos_del_periodo_cents": gastos,
                    "ley": "Ley 822 (IR)",
                    "origen": fuente,
                }),
            ),
            (
                "CUOTA_FIJA",
                json!({"cuota_cents": 0}),
                json!({
                    "informativo": true,
                    "nota": "Cuota determinada por la DGI para cada contribuyente; la plataforma documenta comisiones para su declaracion.",
                    "origen": fuente,
                }),
            ),
        ];

        let mut generadas = Vec::new();
        for (tipo, campos, detalle) in plan {
            if !hay_datos && (empresa_id.is_some() || tipo == "CUOTA_FIJA") {
                continue;
            }
            let existente = sqlx::query(
                "SELECT id FROM finance.declaraciones
                 WHERE jurisdiccion = $1 AND tipo = $2 AND periodo_inicio = $3::date
                   AND empresa_id IS NOT DISTINCT FROM $4::uuid",
            )
            .bind(&jurisdiccion.codigo_pais)
            .bind(tipo)
            .bind(&mes.inicio)
            .bind(empresa_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            if let Some(existente) = existente {
                let id: String = uuid_texto(&existente, "id").map_err(error_db)?;
                generadas.push(self.declaracion(&id).await?);
                continue;
            }

            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            let fila = sqlx::query(
                "INSERT INTO finance.declaraciones
                   (empresa_id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en)
                 VALUES ($1::uuid, $2, $3, $4::date, $5::date,
                         COALESCE($6, 0), COALESCE($7, 0), COALESCE($8, 0), COALESCE($9, 0),
                         COALESCE($10, 0), COALESCE($11, 0), COALESCE($12, 0), COALESCE($13, 0),
                         'GENERADA', $14, NOW())
                 RETURNING id, empresa_id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                           base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                           iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                           ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                           presentada_en, creado_en",
            )
            .bind(empresa_id)
            .bind(&jurisdiccion.codigo_pais)
            .bind(tipo)
            .bind(&mes.inicio)
            .bind(&mes.fin)
            .bind(campo(&campos, "base_gravada_cents"))
            .bind(campo(&campos, "iva_debitado_cents"))
            .bind(campo(&campos, "iva_credito_cents"))
            .bind(campo(&campos, "iva_a_pagar_cents"))
            .bind(campo(&campos, "renta_bruta_cents"))
            .bind(campo(&campos, "renta_gravable_cents"))
            .bind(campo(&campos, "ir_a_pagar_cents"))
            .bind(campo(&campos, "cuota_cents"))
            .bind(sqlx::types::Json(&detalle))
            .fetch_one(&mut *tx)
            .await
            .map_err(error_db)?;
            let creada = declaracion_de_fila(&fila);
            let a_pagar = match creada.tipo.as_str() {
                "IVA" => creada.iva_a_pagar_cents,
                "IR" => creada.ir_a_pagar_cents,
                _ => creada.cuota_cents,
            };
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    DECLARACION_GENERADA,
                    json!({
                        "declaracion_id": creada.id,
                        "empresa_id": empresa_id,
                        "jurisdiccion": creada.jurisdiccion,
                        "tipo": creada.tipo,
                        "periodo_inicio": mes.inicio,
                        "periodo_fin": mes.fin,
                        "a_pagar_cents": a_pagar,
                        "estado": creada.estado,
                    }),
                )
                .await?;
            tx.commit().await.map_err(error_db)?;
            generadas.push(creada);
        }
        Ok(generadas)
    }

    async fn declaracion(&self, id: &str) -> Result<Declaracion, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La declaracion debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, empresa_id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                    presentada_en, creado_en
             FROM finance.declaraciones WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "DECLARACION_INEXISTENTE",
                format!("Declaracion {id} no existe."),
            )
        })?;
        Ok(declaracion_de_fila(&fila))
    }

    pub async fn presentar(&self, id: &str) -> Result<Option<Declaracion>, ApiError> {
        self.cambiar_estado(
            id,
            "UPDATE finance.declaraciones SET estado = 'PRESENTADA', presentada_en = NOW()
             WHERE id = $1 AND estado IN ('GENERADA', 'PENDIENTE')
             RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                       base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                       iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                       ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                       presentada_en, creado_en",
        )
        .await
    }

    pub async fn marcar_pagada(&self, id: &str) -> Result<Option<Declaracion>, ApiError> {
        self.cambiar_estado(
            id,
            "UPDATE finance.declaraciones SET estado = 'PAGADA'
             WHERE id = $1 AND estado IN ('GENERADA', 'PRESENTADA')
             RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                       base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                       iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                       ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                       presentada_en, creado_en",
        )
        .await
    }

    async fn cambiar_estado(&self, id: &str, sql: &str) -> Result<Option<Declaracion>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La declaracion debe ser un UUID."))?;
        let fila = sqlx::query(sql)
            .bind(uuid)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(fila.as_ref().map(declaracion_de_fila))
    }
}
