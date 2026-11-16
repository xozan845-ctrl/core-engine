use super::*;

impl FieldService {
    pub(super) fn interno(_: sqlx::Error) -> ApiError {
        ApiError::interno()
    }

    pub(super) async fn ejecutar_filas(
        &self,
        sql: &str,
        valores: Vec<Valor>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        let filas = consulta(sql, &valores)
            .fetch_all(&mut *tx)
            .await
            .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        filas
            .iter()
            .map(fila_a_json)
            .collect::<Result<_, _>>()
            .map_err(Self::interno)
    }

    pub(super) async fn ejecutar_uno(
        &self,
        sql: &str,
        valores: Vec<Valor>,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        let fila = consulta(sql, &valores)
            .fetch_optional(&mut *tx)
            .await
            .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map(Some).map_err(Self::interno),
            None => Ok(None),
        }
    }

    pub(super) async fn validar_parte(
        &self,
        tenant: &str,
        tabla: &Tabla,
        cuerpo: &Value,
    ) -> Result<(), ApiError> {
        if !tabla.campos.iter().any(|campo| campo.col == "parte_id") {
            return Ok(());
        }
        let Some(objeto) = cuerpo.as_object() else {
            return Ok(());
        };
        let Some(valor) = objeto.get("parteId") else {
            return Ok(());
        };
        let Some(parte_id) = valor.as_str() else {
            return Err(ApiError::parte_invalida(
                "La parte debe ser un UUID valido.",
            ));
        };
        match self.partes.pertenece(tenant, parte_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(ApiError::parte_invalida(
                "La parte no pertenece al tenant activo.",
            )),
            Err(()) => Err(ApiError::servicio_no_disponible("identity-service")),
        }
    }

    pub(super) async fn insertar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: Option<&Value>,
        cuerpo: &Value,
        upsert: bool,
    ) -> Result<Value, ApiError> {
        self.validar_parte(tenant, tabla, cuerpo).await?;

        let objeto = cuerpo.as_object();
        let mut columnas: Vec<(&'static str, TipoColumna, Valor)> = Vec::new();
        for campo in tabla.campos {
            let valor = objeto
                .and_then(|objeto| objeto.get(campo.json))
                .cloned()
                .or_else(|| defecto(tabla.nombre, campo.col));
            if let Some(valor) = valor {
                let tipo = tipo_columna(campo.col);
                columnas.push((campo.col, tipo, Valor::desde_json(&valor, tipo)));
            }
        }

        let id_texto = match id {
            Some(Value::String(texto)) => texto.clone(),
            Some(valor) => valor.to_string(),
            None => Uuid::new_v4().to_string(),
        };

        let mut sql = String::from("INSERT INTO field.");
        sql.push_str(tabla.nombre);
        sql.push_str(" (tenant_id, id");
        for (columna, _, _) in &columnas {
            sql.push_str(", ");
            sql.push_str(columna);
        }
        sql.push_str(", synced, created_at, updated_at) VALUES ($1::uuid, $2::uuid");
        for (indice, (_, tipo, _)) in columnas.iter().enumerate() {
            sql.push_str(&format!(", ${}{}", indice + 3, cast(*tipo)));
        }
        sql.push_str(", TRUE, NOW(), NOW())");
        if upsert {
            sql.push_str(" ON CONFLICT (id) DO UPDATE SET ");
            let sets = tabla
                .campos
                .iter()
                .map(|campo| format!("{} = EXCLUDED.{}", campo.col, campo.col))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&sets);
            sql.push_str(", updated_at = NOW(), deleted_at = NULL");
        }
        sql.push_str(" RETURNING *");

        let mut valores = vec![Valor::Texto(tenant.to_string()), Valor::Texto(id_texto)];
        valores.extend(columnas.into_iter().map(|(_, _, valor)| valor));
        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::persistencia_fallida("No se pudo guardar el registro."))
    }

    pub(super) async fn actualizar_con_map(
        &self,
        tabla: &Tabla,
        campos: &[Campo],
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte(tenant, tabla, cuerpo).await?;
        let objeto = cuerpo.as_object();
        let mut sets: Vec<(String, TipoColumna, Valor)> = Vec::new();
        for campo in campos {
            let Some(valor) = objeto.and_then(|objeto| objeto.get(campo.json)) else {
                continue;
            };
            let tipo = tipo_columna(campo.col);
            sets.push((
                format!("{} = ${}{}", campo.col, sets.len() + 1, cast(tipo)),
                tipo,
                Valor::desde_json(valor, tipo),
            ));
        }
        if sets.is_empty() {
            return self.obtener(tabla, tenant, id).await;
        }

        let indice_tenant = sets.len() + 1;
        let indice_id = sets.len() + 2;
        let mut sql = String::from("UPDATE field.");
        sql.push_str(tabla.nombre);
        sql.push_str(" SET ");
        sql.push_str(
            &sets
                .iter()
                .map(|(set, _, _)| set.clone())
                .collect::<Vec<_>>()
                .join(", "),
        );
        sql.push_str(&format!(
            ", updated_at = NOW() WHERE tenant_id = ${indice_tenant}::uuid AND id = ${indice_id}::uuid AND deleted_at IS NULL RETURNING *"
        ));

        let mut valores = sets
            .into_iter()
            .map(|(_, _, valor)| valor)
            .collect::<Vec<_>>();
        valores.push(Valor::Texto(tenant.to_string()));
        valores.push(Valor::Texto(id.to_string()));

        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::no_encontrado(tabla.nombre, id))
    }

    pub(super) async fn actualizar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar_con_map(tabla, tabla.campos, tenant, id, cuerpo)
            .await
    }

    pub(super) async fn obtener(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: &str,
    ) -> Result<Value, ApiError> {
        let sql = format!(
            "SELECT * FROM field.{} WHERE tenant_id = $1::uuid AND id = $2::uuid AND deleted_at IS NULL",
            tabla.nombre
        );
        self.ejecutar_uno(
            &sql,
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(id.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| ApiError::no_encontrado(tabla.nombre, id))
    }

    pub(super) async fn eliminar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: &str,
    ) -> Result<(), ApiError> {
        let sql = format!(
            "UPDATE field.{} SET deleted_at = NOW() WHERE tenant_id = $1::uuid AND id = $2::uuid",
            tabla.nombre
        );
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        consulta(
            &sql,
            &[
                Valor::Texto(tenant.to_string()),
                Valor::Texto(id.to_string()),
            ],
        )
        .execute(&mut *tx)
        .await
        .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        Ok(())
    }

    pub(super) async fn listar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        filtros: Vec<Filtro>,
        orden: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let mut sql = format!(
            "SELECT * FROM field.{} WHERE tenant_id = $1::uuid AND deleted_at IS NULL",
            tabla.nombre
        );
        let mut valores = vec![Valor::Texto(tenant.to_string())];
        for filtro in filtros {
            let indice = valores.len() + 1;
            sql.push_str(&format!(
                " AND {} {} ${}{}",
                filtro.expresion,
                filtro.operador,
                indice,
                cast(filtro.tipo)
            ));
            valores.push(filtro.valor);
        }
        sql.push_str(&format!(" ORDER BY {orden} DESC LIMIT 2000"));
        self.ejecutar_filas(&sql, valores).await
    }

    // ── personal ──────────────────────────────────────────────────────────
}
