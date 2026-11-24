use super::prelude::*;
use super::*;

impl Fidelizacion {
    pub async fn listar_tarjetas(
        &self,
        empresa_id: &str,
        parte_id: Option<&str>,
    ) -> Result<Vec<Tarjeta>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.empresa_id = $1::uuid
                AND ($2::text IS NULL OR t.parte_id = $2::uuid)
              ORDER BY t.actualizado_en DESC"
        ))
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_tarjeta).collect()
    }

    pub async fn obtener_tarjeta(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
    ) -> Result<Tarjeta, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.empresa_id = $1::uuid AND t.id = $2::uuid"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref()
            .map(fila_tarjeta)
            .transpose()?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", tarjeta_id))
    }

    /// Tarjetas de un comprador: solo las partes que aparecen en sus propias
    /// órdenes (evita exponer tarjetas de otros clientes del mismo tenant).
    pub async fn listar_tarjetas_de_comprador(
        &self,
        comprador_id: &str,
    ) -> Result<Vec<Tarjeta>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.parte_id IN (
                    SELECT DISTINCT parte_id FROM orders.orden_vista
                     WHERE comprador_id = $1::uuid AND parte_id IS NOT NULL)
              ORDER BY t.actualizado_en DESC"
        ))
        .bind(comprador_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_tarjeta).collect()
    }

    pub async fn movimientos_de_comprador(
        &self,
        comprador_id: &str,
        tarjeta_id: &str,
        limite: i64,
    ) -> Result<Vec<Movimiento>, ApiError> {
        let limite = limite.clamp(1, 200);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let propia = sqlx::query(
            "SELECT 1 FROM orders.fidelizacion_tarjetas t
              WHERE t.id = $1::uuid
                AND t.parte_id IN (
                    SELECT DISTINCT parte_id FROM orders.orden_vista
                     WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
        )
        .bind(tarjeta_id)
        .bind(comprador_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if propia.is_none() {
            return Err(ApiError::no_encontrado("Tarjeta", tarjeta_id));
        }
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM orders.fidelizacion_movimientos
              WHERE tarjeta_id = $1::uuid
              ORDER BY creado_en DESC
              LIMIT $2"
        ))
        .bind(tarjeta_id)
        .bind(limite)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_movimiento).collect()
    }

    pub async fn crear_tarjeta(&self, empresa_id: &str, dto: &Value) -> Result<Tarjeta, ApiError> {
        let parte_id = texto(dto, "parte_id");
        if parte_id.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "parte_id", "restricciones": ["isUuid"]}
            ])));
        }
        let codigo = match texto(dto, "codigo") {
            valor if valor.is_empty() => None,
            valor => Some(valor.to_uppercase()),
        };
        if let Some(codigo) = codigo.as_deref() {
            if !codigo_valido(codigo) {
                return Err(ApiError::validacion(json!([
                    {"campo": "codigo", "restricciones": ["matches"]}
                ])));
            }
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let programa = sqlx::query(
            "SELECT id FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid AND activo",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "PROGRAMA_NO_CONFIGURADO",
                "Configura y activa el programa de fidelización antes de emitir tarjetas.",
            )
        })?;
        let programa_id = uuid_texto(&programa, "id").map_err(error_db)?;
        let parte = sqlx::query(
            "SELECT 1 FROM identity.partes
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(&parte_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if parte.is_none() {
            return Err(ApiError::no_encontrado("Parte", &parte_id));
        }
        let codigo = codigo.unwrap_or_else(|| {
            format!(
                "FID-{}",
                uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase()
            )
        });
        let insertada = sqlx::query(
            "INSERT INTO orders.fidelizacion_tarjetas
               (empresa_id, programa_id, parte_id, codigo)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             RETURNING id",
        )
        .bind(empresa_id)
        .bind(&programa_id)
        .bind(&parte_id)
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await;
        let fila = match insertada {
            Ok(fila) => fila,
            Err(error) if es_unico(&error) => {
                return Err(ApiError::conflicto(
                    "El cliente ya tiene una tarjeta o el código está en uso.",
                ));
            }
            Err(error) => return Err(error_db(error)),
        }
        .ok_or_else(ApiError::interno)?;
        let tarjeta_id = uuid_texto(&fila, "id").map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.id = $1::uuid"
        ))
        .bind(&tarjeta_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_tarjeta(&fila)
    }

    pub async fn listar_movimientos(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
        limite: i64,
    ) -> Result<Vec<Movimiento>, ApiError> {
        let limite = limite.clamp(1, 200);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM orders.fidelizacion_movimientos
              WHERE empresa_id = $1::uuid AND tarjeta_id = $2::uuid
              ORDER BY creado_en DESC
              LIMIT $3"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(limite)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_movimiento).collect()
    }

    /// Ajuste manual de puntos (positivo o negativo). El saldo nunca baja de 0.
    pub async fn ajustar(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
        dto: &Value,
        actor_id: &str,
    ) -> Result<Movimiento, ApiError> {
        let puntos = entero(dto, "puntos").unwrap_or_default();
        if puntos == 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["isInt", "notEquals"]}
            ])));
        }
        let descripcion = texto(dto, "descripcion");
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let tarjeta = sqlx::query(
            "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
              WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(tarjeta_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Tarjeta", tarjeta_id))?;
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            return Err(ApiError::dominio(
                "TARJETA_NO_ACTIVA",
                "La tarjeta no está activa.",
            ));
        }
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        let saldo = actuales + puntos;
        if saldo < 0 {
            return Err(ApiError::dominio(
                "PUNTOS_INSUFICIENTES",
                "El ajuste dejaría la tarjeta con saldo negativo.",
            ));
        }
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, descripcion, creado_por)
             VALUES ($1::uuid, $2::uuid, 'ajuste', $3, $4, $5, $6::uuid)
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(puntos)
        .bind(saldo)
        .bind(&descripcion)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_movimiento(&fila)
    }
}
