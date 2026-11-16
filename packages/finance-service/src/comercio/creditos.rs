use super::helpers::*;
use super::*;

impl Comercio {
    pub async fn crear_banco(
        &self,
        dto: CuentaBanco,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.bank, "bank", 1)?;
        validar_texto(&dto.alias, "alias", 1)?;
        validar_texto(&dto.account_number, "accountNumber", 1)?;
        validar_in(&dto.tipo, "type", &TIPOS_CUENTA_BANCO)?;
        validar_rango(dto.balance_cents as f64, "balanceCents", 0.0, f64::MAX)?;
        self.crear(
            "bancos-cuentas",
            json!({
                "bank": dto.bank,
                "alias": dto.alias,
                "accountNumber": dto.account_number,
                "type": dto.tipo,
                "balanceCents": dto.balance_cents,
            }),
            usuario,
        )
        .await
    }

    // ── créditos ───────────────────────────────────────────────────────────

    pub async fn crear_credito(
        &self,
        dto: Credito,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.customer, "customer", 1)?;
        validar_texto(&dto.concept, "concept", 3)?;
        validar_fecha(&dto.due_date, "dueDate")?;
        if let Some(fecha) = &dto.date {
            validar_fecha(fecha, "date")?;
        }
        validar_rango(dto.amount_cents as f64, "amountCents", 1.0, f64::MAX)?;
        self.crear(
            "creditos",
            json!({
                "customer": dto.customer,
                "concept": dto.concept,
                "date": dto.date.unwrap_or_else(hoy),
                "dueDate": dto.due_date,
                "amountCents": dto.amount_cents,
                "balanceCents": dto.amount_cents,
            }),
            usuario,
        )
        .await
    }

    pub async fn pagar_credito(
        &self,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        self.actualizar(
            "creditos",
            id,
            json!({"balanceCents": 0, "paidAt": hoy()}),
            usuario,
        )
        .await
    }

    pub async fn abonar_credito(
        &self,
        id: &str,
        dto: AbonoCredito,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let credito = self.buscar("creditos", id, usuario).await?;
        let datos: Value = jsonb(&credito, "datos").map_err(error_db)?;
        let saldo = datos
            .get("balanceCents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if dto.monto_cents <= 0 {
            return Err(ApiError::dominio(
                "ABONO_INVALIDO",
                "El abono debe ser un entero positivo en centavos.",
            ));
        }
        if dto.monto_cents > saldo {
            return Err(ApiError::dominio(
                "ABONO_EXCEDE_SALDO",
                "El abono supera el saldo pendiente del credito.",
            ));
        }
        let mut abonos = datos
            .get("abonos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let nuevo_saldo = saldo - dto.monto_cents;
        abonos.push(json!({
            "montoCents": dto.monto_cents,
            "fecha": hoy(),
            "metodoPago": dto.metodo_pago.unwrap_or_else(|| "efectivo".to_string()),
        }));
        let mut cambios = json!({
            "balanceCents": nuevo_saldo,
            "abonos": abonos,
        });
        if nuevo_saldo == 0 {
            cambios["paidAt"] = json!(hoy());
        }
        self.actualizar("creditos", id, cambios, usuario).await
    }

    // ── cuentas de crédito por parte (venta a crédito) ──────────────────────

    pub async fn listar_cuentas_credito(
        &self,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Vec<CuentaCredito>, ApiError> {
        let tenant = Self::tenant_credito(usuario, empresa_id, false)?;
        let tenant = parsear_uuid(&tenant)?;
        let filas = sqlx::query(
            "SELECT c.id, c.empresa_id, c.parte_id, p.nombre AS parte_nombre,
                    c.limite_cents, c.activo, c.actualizado_en,
                    COALESCE((
                      SELECT SUM((w.datos ->> 'balanceCents')::int)
                        FROM finance.workspace_registros w
                       WHERE w.recurso = 'creditos' AND w.eliminado_en IS NULL
                         AND w.tenant_id = c.empresa_id AND w.parte_id = c.parte_id
                    ), 0)::int AS usado_cents
             FROM finance.credito_cuentas c
             JOIN identity.partes p ON p.id = c.parte_id
             WHERE c.empresa_id = $1
             ORDER BY p.nombre",
        )
        .bind(tenant)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas
            .iter()
            .map(|fila| {
                let limite = fila.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;
                let usado = fila.try_get::<i32, _>("usado_cents").unwrap_or(0) as i64;
                CuentaCredito {
                    id: uuid_texto(fila, "id").unwrap_or_default(),
                    empresa_id: uuid_texto(fila, "empresa_id").unwrap_or_default(),
                    parte_id: uuid_texto(fila, "parte_id").unwrap_or_default(),
                    parte_nombre: fila.try_get("parte_nombre").unwrap_or_default(),
                    limite_cents: limite,
                    usado_cents: usado,
                    disponible_cents: limite - usado,
                    activo: fila.try_get("activo").unwrap_or(false),
                    actualizado_en: timestamp(fila, "actualizado_en").unwrap_or_default(),
                }
            })
            .collect())
    }

    pub async fn guardar_cuenta_credito(
        &self,
        parte_id: &str,
        dto: CuentaCreditoInput,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<CuentaCredito, ApiError> {
        let tenant = Self::tenant_credito(usuario, empresa_id, true)?;
        let tenant = parsear_uuid(&tenant)?;
        validar_rango(dto.limite_cents as f64, "limiteCents", 0.0, f64::MAX)?;
        let parte_uuid = parsear_uuid(parte_id)?;
        let parte = sqlx::query(
            "SELECT nombre FROM identity.partes
             WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(parte_uuid)
        .bind(tenant)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(parte) = parte else {
            return Err(ApiError::no_encontrado("Parte", parte_id));
        };
        let nombre: String = parte.try_get("nombre").unwrap_or_default();

        let fila = sqlx::query(
            "INSERT INTO finance.credito_cuentas (empresa_id, parte_id, limite_cents, activo)
             VALUES ($1, $2, $3, COALESCE($4, true))
             ON CONFLICT (empresa_id, parte_id) DO UPDATE
               SET limite_cents = EXCLUDED.limite_cents,
                   activo = COALESCE($4, finance.credito_cuentas.activo),
                   actualizado_en = NOW()
             RETURNING id, empresa_id, parte_id, limite_cents, activo, actualizado_en",
        )
        .bind(tenant)
        .bind(parte_uuid)
        .bind(dto.limite_cents as i32)
        .bind(dto.activo)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("CUENTA_CREDITO_FALLIDA", "No se pudo guardar la cuenta.")
        })?;

        let usado: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM((datos ->> 'balanceCents')::int), 0)::int AS usado
             FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND tenant_id = $1 AND parte_id = $2",
        )
        .bind(tenant)
        .bind(parte_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let limite = fila.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;
        let usado = usado.unwrap_or(0) as i64;
        Ok(CuentaCredito {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            empresa_id: uuid_texto(&fila, "empresa_id").map_err(error_db)?,
            parte_id: uuid_texto(&fila, "parte_id").map_err(error_db)?,
            parte_nombre: nombre,
            limite_cents: limite,
            usado_cents: usado,
            disponible_cents: limite - usado,
            activo: fila.try_get("activo").map_err(error_db)?,
            actualizado_en: timestamp(&fila, "actualizado_en").map_err(error_db)?,
        })
    }

    /// Registro interno (orders → finance) de una venta a crédito.
    pub async fn registrar_credito_orden(&self, input: CreditoInterno) -> Result<Value, ApiError> {
        if input.monto_cents <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "monto_cents", "restricciones": ["min:1"]}
            ])));
        }
        let empresa = parsear_uuid(&input.empresa_id)?;
        let parte = parsear_uuid(&input.parte_id)?;
        let cuenta = sqlx::query(
            "SELECT limite_cents, activo FROM finance.credito_cuentas
             WHERE empresa_id = $1 AND parte_id = $2",
        )
        .bind(empresa)
        .bind(parte)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(cuenta) = cuenta else {
            return Err(ApiError::conflicto(
                "La parte no tiene una cuenta de credito activa.",
            ));
        };
        let activo: bool = cuenta.try_get("activo").unwrap_or(false);
        if !activo {
            return Err(ApiError::conflicto(
                "La parte no tiene una cuenta de credito activa.",
            ));
        }
        let limite = cuenta.try_get::<i32, _>("limite_cents").unwrap_or(0) as i64;

        let existente = sqlx::query(
            "SELECT id, datos FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND datos ->> 'orderId' = $1
             LIMIT 1",
        )
        .bind(&input.order_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some(existente) = existente {
            let datos: Value = jsonb(&existente, "datos").map_err(error_db)?;
            return Ok(json!({
                "credito_id": uuid_texto(&existente, "id").map_err(error_db)?,
                "balance_cents": datos.get("balanceCents").and_then(Value::as_i64).unwrap_or(0),
                "limite_cents": limite,
            }));
        }

        let parte_fila = sqlx::query(
            "SELECT nombre FROM identity.partes
             WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL",
        )
        .bind(parte)
        .bind(empresa)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(parte_fila) = parte_fila else {
            return Err(ApiError::no_encontrado("Parte", &input.parte_id));
        };
        let nombre: String = parte_fila.try_get("nombre").unwrap_or_default();

        let usado: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(SUM((datos ->> 'balanceCents')::int), 0)::int AS usado
             FROM finance.workspace_registros
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND tenant_id = $1 AND parte_id = $2",
        )
        .bind(empresa)
        .bind(parte)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let disponible = limite - usado.unwrap_or(0) as i64;
        if input.monto_cents > disponible {
            return Err(ApiError::conflicto(
                "El credito supera el limite disponible de la parte.",
            ));
        }

        let datos = json!({
            "customer": nombre,
            "concept": input.concepto,
            "date": hoy(),
            "dueDate": "",
            "amountCents": input.monto_cents,
            "balanceCents": input.monto_cents,
            "orderId": input.order_id,
            "sellerId": input.vendedor_id.unwrap_or_default(),
        });
        let fila = sqlx::query(
            "INSERT INTO finance.workspace_registros
               (tenant_id, propietario_id, recurso, datos, parte_id)
             VALUES ($1, NULL, 'creditos', $2::jsonb, $3)
             RETURNING id",
        )
        .bind(empresa)
        .bind(sqlx::types::Json(&datos))
        .bind(parte)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("CREDITO_NO_CREADO", "No se pudo registrar el credito.")
        })?;
        Ok(json!({
            "credito_id": uuid_texto(&fila, "id").map_err(error_db)?,
            "balance_cents": input.monto_cents,
            "limite_cents": limite,
        }))
    }

    pub async fn anular_credito_orden(&self, order_id: &str) -> Result<bool, ApiError> {
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET eliminado_en = NOW(), actualizado_en = NOW()
             WHERE recurso = 'creditos' AND eliminado_en IS NULL
               AND datos ->> 'orderId' = $1
             RETURNING id",
        )
        .bind(order_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.is_some())
    }

    // ── gastos y cuentas por pagar ─────────────────────────────────────────

    pub(super) fn tenant_credito(
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
        escritura: bool,
    ) -> Result<String, ApiError> {
        if !ROLES_COMERCIO.contains(&usuario.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "El rol no tiene acceso a cuentas de credito.",
            ));
        }
        if escritura && !crate::auth::ROLES_CREDITO_ESCRITURA.contains(&usuario.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "Solo la empresa puede definir limites de credito.",
            ));
        }
        if usuario.es_admin_global() {
            return empresa_id
                .map(str::to_string)
                .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."));
        }
        let tenant = usuario.tenant().map(str::to_string)?;
        if let Some(empresa) = empresa_id {
            if empresa != tenant {
                return Err(ApiError::acceso_denegado("No puedes operar otra empresa."));
            }
        }
        Ok(tenant)
    }
}
