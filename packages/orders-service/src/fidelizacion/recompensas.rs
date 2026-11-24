use super::prelude::*;
use super::*;

impl Fidelizacion {
    pub async fn listar_recompensas(&self, empresa_id: &str) -> Result<Vec<Recompensa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_RECOMPENSA}
               FROM orders.fidelizacion_recompensas
              WHERE empresa_id = $1::uuid
              ORDER BY activo DESC, puntos, codigo"
        ))
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recompensa).collect()
    }

    pub async fn crear_recompensa(
        &self,
        empresa_id: &str,
        dto: &Value,
    ) -> Result<Recompensa, ApiError> {
        let codigo = texto(dto, "codigo").to_lowercase();
        if !codigo_recompensa_valido(&codigo) {
            return Err(ApiError::validacion(json!([
                {"campo": "codigo", "restricciones": ["matches"]}
            ])));
        }
        let nombre = texto(dto, "nombre");
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let descripcion = texto(dto, "descripcion");
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let puntos = entero(dto, "puntos").unwrap_or_default();
        if puntos <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["min"]}
            ])));
        }
        let stock = entero(dto, "stock");
        if stock.is_some_and(|stock| stock < 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "stock", "restricciones": ["min"]}
            ])));
        }
        let activo = dto.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let insertada = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_recompensas
               (empresa_id, codigo, nombre, descripcion, puntos, stock, activo, config)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8)
             RETURNING {COLUMNAS_RECOMPENSA}"
        ))
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(&descripcion)
        .bind(puntos)
        .bind(stock)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await;
        let fila = match insertada {
            Ok(fila) => fila,
            Err(error) if es_unico(&error) => {
                return Err(ApiError::conflicto(
                    "Ya existe una recompensa con ese código.",
                ));
            }
            Err(error) => return Err(error_db(error)),
        }
        .ok_or_else(ApiError::interno)?;
        tx.commit().await.map_err(error_db)?;
        fila_recompensa(&fila)
    }

    pub async fn actualizar_recompensa(
        &self,
        empresa_id: &str,
        recompensa_id: &str,
        dto: &Value,
    ) -> Result<Recompensa, ApiError> {
        let actual = {
            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            let fila = sqlx::query(&format!(
                "SELECT {COLUMNAS_RECOMPENSA}
                   FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid"
            ))
            .bind(empresa_id)
            .bind(recompensa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", recompensa_id))?;
            tx.commit().await.map_err(error_db)?;
            fila_recompensa(&fila)?
        };
        let nombre = dto
            .get("nombre")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
            .unwrap_or(actual.nombre);
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let descripcion = dto
            .get("descripcion")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(actual.descripcion);
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let puntos = entero(dto, "puntos").unwrap_or(actual.puntos);
        if puntos <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["min"]}
            ])));
        }
        let stock = if dto.get("stock").is_some() {
            entero(dto, "stock")
        } else {
            actual.stock
        };
        if stock.is_some_and(|stock| stock < 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "stock", "restricciones": ["min"]}
            ])));
        }
        let activo = dto
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(actual.activo);
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or(actual.config);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "UPDATE orders.fidelizacion_recompensas
                SET nombre = $3, descripcion = $4, puntos = $5, stock = $6,
                    activo = $7, config = $8, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid
              RETURNING {COLUMNAS_RECOMPENSA}"
        ))
        .bind(empresa_id)
        .bind(recompensa_id)
        .bind(&nombre)
        .bind(&descripcion)
        .bind(puntos)
        .bind(stock)
        .bind(activo)
        .bind(&config)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_recompensa(&fila)
    }

    /// Canjea una recompensa por una tarjeta en una sola transacción.
    pub async fn canjear(
        &self,
        empresa_id: &str,
        dto: &Value,
        actor_id: &str,
    ) -> Result<Canje, ApiError> {
        let tarjeta_ref = texto(dto, "tarjeta_id");
        let tarjeta_codigo = texto(dto, "codigo_tarjeta").to_uppercase();
        if tarjeta_ref.is_empty() && tarjeta_codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "tarjeta_id", "restricciones": ["isUuid"]}
            ])));
        }
        let recompensa_ref = texto(dto, "recompensa_id");
        let recompensa_codigo = texto(dto, "recompensa_codigo").to_lowercase();
        if recompensa_ref.is_empty() && recompensa_codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "recompensa_id", "restricciones": ["isUuid"]}
            ])));
        }
        let order_id = dto
            .get("order_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string);

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let tarjeta = if tarjeta_ref.is_empty() {
            sqlx::query(
                "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
                  WHERE empresa_id = $1::uuid AND codigo = $2 FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&tarjeta_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", &tarjeta_codigo))?
        } else {
            sqlx::query(
                "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&tarjeta_ref)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", &tarjeta_ref))?
        };
        let tarjeta_id = uuid_texto(&tarjeta, "id").map_err(error_db)?;
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            return Err(ApiError::dominio(
                "TARJETA_NO_ACTIVA",
                "La tarjeta no está activa.",
            ));
        }
        let recompensa = if recompensa_ref.is_empty() {
            sqlx::query(
                "SELECT id, puntos, stock, activo FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND codigo = $2 FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&recompensa_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", &recompensa_codigo))?
        } else {
            sqlx::query(
                "SELECT id, puntos, stock, activo FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&recompensa_ref)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", &recompensa_ref))?
        };
        let recompensa_id = uuid_texto(&recompensa, "id").map_err(error_db)?;
        let recompensa_activa: bool = recompensa.try_get("activo").map_err(error_db)?;
        if !recompensa_activa {
            return Err(ApiError::dominio(
                "RECOMPENSA_NO_ACTIVA",
                "La recompensa no está disponible.",
            ));
        }
        let stock: Option<i32> = recompensa.try_get("stock").map_err(error_db)?;
        if stock.is_some_and(|stock| stock <= 0) {
            return Err(ApiError::dominio(
                "RECOMPENSA_SIN_STOCK",
                "La recompensa no tiene stock disponible.",
            ));
        }
        let costo: i32 = recompensa.try_get("puntos").map_err(error_db)?;
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        if actuales < costo {
            return Err(ApiError::dominio(
                "PUNTOS_INSUFICIENTES",
                "La tarjeta no tiene puntos suficientes para el canje.",
            ));
        }
        let saldo = actuales - costo;
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        if stock.is_some() {
            sqlx::query(
                "UPDATE orders.fidelizacion_recompensas
                    SET stock = stock - 1, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND id = $2::uuid",
            )
            .bind(empresa_id)
            .bind(&recompensa_id)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }
        let movimiento = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id,
                descripcion, creado_por)
             VALUES ($1::uuid, $2::uuid, 'canje', $3, $4, $5::uuid, $6, $7::uuid)
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(-costo)
        .bind(saldo)
        .bind(&order_id)
        .bind(format!("Canje de recompensa {recompensa_id}"))
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let movimiento_id = uuid_texto(&movimiento, "id").map_err(error_db)?;
        let canje = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_canjes
               (empresa_id, tarjeta_id, recompensa_id, movimiento_id, order_id,
                puntos, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6, $7::uuid)
             RETURNING {COLUMNAS_CANJE}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(&recompensa_id)
        .bind(&movimiento_id)
        .bind(&order_id)
        .bind(costo)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_canje(&canje)
    }
}
