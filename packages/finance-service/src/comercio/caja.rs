use super::helpers::*;
use super::*;

impl Comercio {
    pub async fn obtener_configuracion_documentos(
        &self,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let registros = self
            .listar("configuracion-documentos", usuario, None)
            .await?;
        if let Some(existente) = registros.into_iter().next() {
            return Ok(existente);
        }
        self.crear(
            "configuracion-documentos",
            json!({
                "logoSize": "mediano",
                "headerColor": "#0f172a",
                "matchTableColor": false,
                "footerMessage": "",
                "showRuc": true,
                "showAddress": true,
                "showPhone": true,
            }),
            usuario,
        )
        .await
    }

    pub async fn actualizar_configuracion_documentos(
        &self,
        dto: ConfiguracionDocumentos,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_in(
            &dto.logo_size,
            "logoSize",
            &["pequeno", "mediano", "grande"],
        )?;
        validar_texto(&dto.header_color, "headerColor", 1)?;
        let actual = self.obtener_configuracion_documentos(usuario).await?;
        let id = actual
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ApiError::interno)?
            .to_string();
        self.actualizar("configuracion-documentos", &id, json!(dto), usuario)
            .await
    }

    // ── caja ───────────────────────────────────────────────────────────────

    pub async fn eliminar_metodo_pago(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        let metodos = self.listar("caja-metodos-pago", usuario, None).await?;
        if metodos.len() <= 1 {
            return Err(ApiError::dominio(
                "ULTIMO_METODO",
                "Debe quedar al menos un método de pago.",
            ));
        }
        let movimientos = self.listar("caja-movimientos", usuario, None).await?;
        if movimientos
            .iter()
            .any(|movimiento| movimiento.get("paymentMethod").and_then(Value::as_str) == Some(id))
        {
            return Err(ApiError::dominio(
                "METODO_EN_USO",
                "No se puede eliminar un método de pago con movimientos registrados.",
            ));
        }
        self.eliminar("caja-metodos-pago", id, usuario).await
    }

    /// Calcula el corte del día sin persistirlo (pantalla de cierre).
    pub async fn preview_cierre(
        &self,
        usuario: &UsuarioContexto,
        fecha: Option<&str>,
    ) -> Result<PreviewCierre, ApiError> {
        let dia = fecha.map(str::to_string).unwrap_or_else(hoy);
        let movimientos: Vec<Registro> = self
            .listar("caja-movimientos", usuario, None)
            .await?
            .into_iter()
            .filter(|movimiento| {
                movimiento.get("status").and_then(Value::as_str) == Some("completado")
            })
            .collect();
        let anteriores: Vec<Registro> = movimientos
            .iter()
            .filter(|movimiento| {
                movimiento
                    .get("date")
                    .and_then(Value::as_str)
                    .map(|valor| valor < dia.as_str())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let del_dia: Vec<Registro> = movimientos
            .iter()
            .filter(|movimiento| {
                movimiento.get("date").and_then(Value::as_str) == Some(dia.as_str())
            })
            .cloned()
            .collect();
        let abrir = saldo(&anteriores);
        let ingresos = suma(
            &del_dia
                .iter()
                .filter(|movimiento| {
                    movimiento.get("type").and_then(Value::as_str) == Some("ingreso")
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let egresos = suma(
            &del_dia
                .iter()
                .filter(|movimiento| {
                    movimiento.get("type").and_then(Value::as_str) == Some("egreso")
                })
                .cloned()
                .collect::<Vec<_>>(),
        );
        let existente = self
            .listar("caja-cierres", usuario, None)
            .await?
            .into_iter()
            .find(|cierre| cierre.get("date").and_then(Value::as_str) == Some(dia.as_str()));
        Ok(PreviewCierre {
            date: dia,
            opening_balance_cents: abrir,
            income_cents: ingresos,
            expense_cents: egresos,
            expected_balance_cents: abrir + ingresos - egresos,
            movements_count: del_dia.len() as i64,
            existing: existente,
        })
    }

    pub async fn crear_cierre(
        &self,
        dto: CierreCaja,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_rango(
            dto.counted_balance_cents as f64,
            "countedBalanceCents",
            0.0,
            f64::MAX,
        )?;
        let preview = self.preview_cierre(usuario, Some(&dto.date)).await?;
        if preview.existing.is_some() {
            return Err(ApiError::dominio(
                "CIERRE_EXISTENTE",
                format!("Ya existe un cierre para {}.", dto.date),
            ));
        }
        self.crear(
            "caja-cierres",
            json!({
                "date": preview.date,
                "openingBalanceCents": preview.opening_balance_cents,
                "incomeCents": preview.income_cents,
                "expenseCents": preview.expense_cents,
                "expectedBalanceCents": preview.expected_balance_cents,
                "countedBalanceCents": dto.counted_balance_cents,
                "differenceCents": dto.counted_balance_cents - preview.expected_balance_cents,
                "movementsCount": preview.movements_count,
                "notes": dto.notes.unwrap_or_default(),
                "closedAt": iso_ms(Utc::now()),
            }),
            usuario,
        )
        .await
    }

    pub async fn obtener_fondo_caja_chica(
        &self,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let registros = self.listar("caja-chica-fondo", usuario, None).await?;
        if let Some(existente) = registros.into_iter().next() {
            return Ok(existente);
        }
        self.crear(
            "caja-chica-fondo",
            json!({"amountCents": 0, "responsible": "", "updatedAt": iso_ms(Utc::now())}),
            usuario,
        )
        .await
    }

    pub async fn guardar_fondo_caja_chica(
        &self,
        dto: FondoCajaChica,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_rango(dto.amount_cents as f64, "amountCents", 0.0, f64::MAX)?;
        validar_texto(&dto.responsible, "responsible", 1)?;
        let actual = self.obtener_fondo_caja_chica(usuario).await?;
        let id = actual
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ApiError::interno)?
            .to_string();
        self.actualizar(
            "caja-chica-fondo",
            &id,
            json!({
                "amountCents": dto.amount_cents,
                "responsible": dto.responsible,
                "updatedAt": iso_ms(Utc::now()),
            }),
            usuario,
        )
        .await
    }

    /// Registra una transferencia interna y sus dos movimientos de caja.
    pub async fn crear_transferencia(
        &self,
        dto: TransferenciaCaja,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_fecha(&dto.date, "date")?;
        validar_texto(&dto.from_id, "fromId", 1)?;
        validar_texto(&dto.from_label, "fromLabel", 1)?;
        validar_in(&dto.from_kind, "fromKind", &["cash", "bank"])?;
        validar_texto(&dto.to_id, "toId", 1)?;
        validar_texto(&dto.to_label, "toLabel", 1)?;
        validar_in(&dto.to_kind, "toKind", &["cash", "bank"])?;
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        validar_texto(&dto.concept, "concept", 1)?;
        if dto.from_id == dto.to_id {
            return Err(ApiError::dominio(
                "TRANSFERENCIA_INVALIDA",
                "Las cuentas origen y destino deben ser distintas.",
            ));
        }
        let ambito = Self::ambito("caja-transferencias", usuario, None)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if dto.from_kind == "bank" {
            mover_banco(&mut tx, &dto.from_id, -dto.amount_cents, &ambito).await?;
        }
        if dto.to_kind == "bank" {
            mover_banco(&mut tx, &dto.to_id, dto.amount_cents, &ambito).await?;
        }
        let transferencia = insertar_en(
            &mut tx,
            "caja-transferencias",
            Self::normalizar(
                "caja-transferencias",
                json!({
                    "fromId": dto.from_id,
                    "fromLabel": dto.from_label,
                    "fromKind": dto.from_kind,
                    "toId": dto.to_id,
                    "toLabel": dto.to_label,
                    "toKind": dto.to_kind,
                    "amountCents": dto.amount_cents,
                    "concept": dto.concept,
                    "date": dto.date,
                    "notes": dto.notes.clone().unwrap_or_default(),
                    "status": "completado",
                }),
            )?,
            &ambito,
        )
        .await?;
        let transfer_id = uuid_texto(&transferencia, "id").map_err(error_db)?;
        let base = json!({
            "date": dto.date,
            "concept": dto.concept,
            "amountCents": dto.amount_cents,
            "paymentMethod": "transferencia",
            "category": "transferencia",
            "currency": "NIO",
            "originalAmountCents": dto.amount_cents,
            "exchangeRate": 1,
            "source": "transferencia",
            "status": "completado",
            "reference": transfer_id,
            "observations": dto.notes.clone().unwrap_or_default(),
            "transferId": transfer_id,
        });
        for tipo in ["egreso", "ingreso"] {
            let mut datos = base.clone();
            datos["type"] = json!(tipo);
            insertar_en(
                &mut tx,
                "caja-movimientos",
                Self::normalizar("caja-movimientos", datos)?,
                &ambito,
            )
            .await?;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(a_registro(&transferencia))
    }

    /// Anula la transferencia, sus movimientos ligados y revierte los bancos.
    pub async fn anular_transferencia(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito("caja-transferencias", usuario, None)?;
        let actual = self.buscar("caja-transferencias", id, usuario).await?;
        let datos: Value = jsonb(&actual, "datos").map_err(error_db)?;
        if datos.get("status").and_then(Value::as_str) == Some("anulado") {
            return Ok(a_registro(&actual));
        }
        let monto = numero(datos.get("amountCents"));
        let uuid = parsear_uuid(id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if datos.get("fromKind").and_then(Value::as_str) == Some("bank") {
            let from_id = datos
                .get("fromId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            mover_banco(&mut tx, from_id, monto, &ambito).await?;
        }
        if datos.get("toKind").and_then(Value::as_str) == Some("bank") {
            let to_id = datos
                .get("toId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            mover_banco(&mut tx, to_id, -monto, &ambito).await?;
        }
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || '{\"status\":\"anulado\"}'::jsonb, actualizado_en = NOW()
             WHERE id = $1 AND recurso = 'caja-transferencias'
             RETURNING id, datos, creado_en, actualizado_en",
        )
        .bind(uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || '{\"status\":\"anulado\"}'::jsonb, actualizado_en = NOW()
             WHERE recurso = 'caja-movimientos' AND eliminado_en IS NULL
               AND datos->>'transferId' = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(a_registro(&fila))
    }

    // ── clientes y códigos de barras ───────────────────────────────────────
}
