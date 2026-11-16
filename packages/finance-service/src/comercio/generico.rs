use super::helpers::*;
use super::*;

impl Comercio {
    pub async fn listar(
        &self,
        recurso: &str,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Vec<Registro>, ApiError> {
        let ambito = Self::ambito(recurso, usuario, empresa_id)?;
        let filas = sqlx::query(
            "SELECT id, datos, creado_en, actualizado_en
             FROM finance.workspace_registros
             WHERE recurso = $1 AND eliminado_en IS NULL
               AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
               AND ($3::uuid IS NULL OR propietario_id = $3::uuid)
             ORDER BY creado_en DESC, id",
        )
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(a_registro).collect())
    }

    pub async fn obtener(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let fila = self.buscar(recurso, id, usuario).await?;
        Ok(a_registro(&fila))
    }

    pub async fn crear(
        &self,
        recurso: &str,
        datos: Value,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        if ambito.tenant_id.is_none() && ambito.propietario_id.is_none() {
            return Err(ApiError::dominio(
                "TENANT_REQUERIDO",
                "El contexto no tiene empresa ni vendedor.",
            ));
        }
        let payload = Self::normalizar(recurso, datos)?;
        let fila = self.insertar(recurso, payload, &ambito).await?;
        Ok(a_registro(&fila))
    }

    pub async fn actualizar(
        &self,
        recurso: &str,
        id: &str,
        datos: Value,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET datos = datos || $4::jsonb, actualizado_en = NOW()
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($5::uuid IS NULL OR propietario_id = $5::uuid)
             RETURNING id, datos, creado_en, actualizado_en",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(sqlx::types::Json(&datos))
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.map(|fila| a_registro(&fila))
            .ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    pub async fn eliminar(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "UPDATE finance.workspace_registros
             SET eliminado_en = NOW(), actualizado_en = NOW()
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
             RETURNING id",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.map(|_| ())
            .ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    // ── bancos ─────────────────────────────────────────────────────────────

    pub(super) async fn buscar(
        &self,
        recurso: &str,
        id: &str,
        usuario: &UsuarioContexto,
    ) -> Result<sqlx::postgres::PgRow, ApiError> {
        let ambito = Self::ambito(recurso, usuario, None)?;
        let uuid = parsear_uuid(id)?;
        let fila = sqlx::query(
            "SELECT id, datos, creado_en, actualizado_en
             FROM finance.workspace_registros
             WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
               AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
               AND ($4::uuid IS NULL OR propietario_id = $4::uuid)",
        )
        .bind(uuid)
        .bind(recurso)
        .bind(ambito.tenant_id.as_deref())
        .bind(ambito.propietario_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        fila.ok_or_else(|| ApiError::no_encontrado("Registro", id))
    }

    pub(super) async fn insertar(
        &self,
        recurso: &str,
        datos: Value,
        ambito: &Ambito,
    ) -> Result<sqlx::postgres::PgRow, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = insertar_en(&mut tx, recurso, datos, ambito).await?;
        tx.commit().await.map_err(error_db)?;
        Ok(fila)
    }

    /// Construye el payload final del recurso: defaults y totales calculados.
    pub fn normalizar(recurso: &str, datos: Value) -> Result<Value, ApiError> {
        let ahora = iso_ms(Utc::now());
        let mut objeto = datos.as_object().cloned().unwrap_or_default();
        match recurso {
            "creditos" => {
                objeto.entry("date").or_insert_with(|| json!(hoy()));
                if !objeto.contains_key("balanceCents") {
                    let amount = objeto.get("amountCents").cloned().unwrap_or(json!(0));
                    objeto.insert("balanceCents".into(), amount);
                }
            }
            "gastos" => {
                let defaults = json!({
                    "status": "pendiente",
                    "includesVat": false,
                    "irRetentionCents": 0,
                    "beneficiary": "",
                    "notes": "",
                    "scheduledFor": "",
                    "recurrence": "",
                });
                objeto = con_defaults(objeto, defaults);
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "cuentas-por-pagar" => {
                objeto = con_defaults(objeto, json!({"status": "pendiente", "paidAt": ""}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "proveedores" => {
                objeto = con_defaults(
                    objeto,
                    json!({"ruc": "", "phone": "", "email": "", "address": ""}),
                );
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "compras" => {
                let lineas = objeto
                    .get("lines")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let subtotal = total_lineas(&Value::Array(lineas.clone()));
                objeto = con_defaults(objeto, json!({"status": "pendiente", "statusLabel": ""}));
                objeto.insert("lines".into(), Value::Array(lineas));
                objeto.insert("subtotalCents".into(), json!(subtotal));
                objeto = con_defaults(objeto, json!({"retentionCents": 0}));
                objeto.insert("totalCents".into(), json!(subtotal));
                objeto = con_defaults(objeto, json!({"notes": "", "userName": ""}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "ordenes-compra" => {
                let lineas = objeto.get("lines").cloned().unwrap_or_else(|| json!([]));
                objeto.insert("status".into(), json!("pendiente"));
                objeto.insert("totalCents".into(), json!(total_lineas(&lineas)));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-metodos-pago" => {
                objeto = con_defaults(objeto, json!({"icon": "payments", "active": true}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-movimientos" => {
                let amount = objeto.get("amountCents").cloned().unwrap_or(json!(0));
                objeto = con_defaults(objeto, json!({"currency": "NIO"}));
                objeto
                    .entry("originalAmountCents")
                    .or_insert_with(|| amount.clone());
                objeto.entry("exchangeRate").or_insert_with(|| json!(1));
                objeto = con_defaults(
                    objeto,
                    json!({
                        "category": "otro",
                        "source": "manual",
                        "reference": "",
                        "observations": "",
                    }),
                );
                objeto.insert("status".into(), json!("completado"));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-cierres" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-chica-fondo" => {
                objeto = con_defaults(objeto, json!({"amountCents": 0, "responsible": ""}));
                objeto.insert("updatedAt".into(), json!(ahora));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-chica-movimientos" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "caja-transferencias" => {
                objeto = con_defaults(objeto, json!({"status": "completado"}));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "configuracion-documentos" => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "clientes" => {
                objeto = con_defaults(
                    objeto,
                    json!({"phone": "", "email": "", "company": "", "source": "manual"}),
                );
                objeto.insert("createdAt".into(), json!(ahora));
            }
            "codigos-barras" => {
                let codigo = objeto
                    .get("code")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|valor| !valor.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        let simbologia = objeto
                            .get("symbology")
                            .and_then(Value::as_str)
                            .unwrap_or("CODE128");
                        generar_codigo(simbologia)
                    });
                objeto.insert("code".into(), json!(codigo));
                objeto.insert("createdAt".into(), json!(ahora));
            }
            _ => {
                objeto.insert("createdAt".into(), json!(ahora));
            }
        }
        Ok(Value::Object(objeto))
    }

    pub(super) async fn siguiente_numero(
        &self,
        recurso: &str,
        prefijo: &str,
        usuario: &UsuarioContexto,
    ) -> Result<String, ApiError> {
        let registros = self.listar(recurso, usuario, None).await?;
        Ok(format!("{prefijo}-{:06}", registros.len() + 1))
    }
}
