use super::prelude::*;
use super::*;

impl Proyecciones {
    pub async fn crear(
        &self,
        nombre: &str,
        supuestos: SupuestosProyeccion,
    ) -> Result<Proyeccion, ApiError> {
        let (filas, totales) = Self::calcular(&supuestos)?;
        let indicadores = Self::indicadores(&filas, &supuestos);
        let fila = sqlx::query(
            "INSERT INTO finance.proyecciones
               (nombre, horizonte_meses, supuestos, filas, totales, inversion_inicial_cents)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, nombre, horizonte_meses, inversion_inicial_cents, creado_en",
        )
        .bind(nombre)
        .bind(supuestos.horizonte_meses)
        .bind(sqlx::types::Json(&supuestos))
        .bind(sqlx::types::Json(&filas))
        .bind(sqlx::types::Json(&totales))
        .bind(supuestos.inversion_inicial_cents)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio("PROYECCION_FALLIDA", "No se pudo guardar la proyeccion.")
        })?;
        Ok(Proyeccion {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            nombre: fila.try_get("nombre").map_err(error_db)?,
            horizonte_meses: fila
                .try_get::<i32, _>("horizonte_meses")
                .map_err(error_db)? as i64,
            supuestos,
            filas,
            totales,
            inversion_inicial_cents: fila
                .try_get::<i32, _>("inversion_inicial_cents")
                .map_err(error_db)? as i64,
            indicadores,
            creado_en: timestamp(&fila, "creado_en").map_err(error_db)?,
        })
    }

    pub async fn listar(&self) -> Result<Vec<ProyeccionResumen>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, nombre, horizonte_meses, creado_en
             FROM finance.proyecciones ORDER BY creado_en DESC",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas
            .iter()
            .map(|fila| ProyeccionResumen {
                id: uuid_texto(fila, "id").unwrap_or_default(),
                nombre: fila.try_get("nombre").unwrap_or_default(),
                horizonte_meses: fila.try_get::<i32, _>("horizonte_meses").unwrap_or(0) as i64,
                creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
            })
            .collect())
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Proyeccion>, ApiError> {
        let uuid = sqlx::types::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La proyeccion debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, nombre, horizonte_meses, supuestos, filas, totales,
                    inversion_inicial_cents, creado_en
             FROM finance.proyecciones WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let supuestos: SupuestosProyeccion =
            serde_json::from_value(jsonb(&fila, "supuestos").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        let filas: Vec<FilaProyeccion> =
            serde_json::from_value(jsonb(&fila, "filas").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        let totales: TotalesProyeccion =
            serde_json::from_value(jsonb(&fila, "totales").map_err(error_db)?)
                .map_err(|_| ApiError::interno())?;
        Ok(Some(Proyeccion {
            id: uuid_texto(&fila, "id").map_err(error_db)?,
            nombre: fila.try_get("nombre").map_err(error_db)?,
            horizonte_meses: fila
                .try_get::<i32, _>("horizonte_meses")
                .map_err(error_db)? as i64,
            indicadores: Self::indicadores(&filas, &supuestos),
            supuestos,
            filas,
            totales,
            inversion_inicial_cents: fila
                .try_get::<i32, _>("inversion_inicial_cents")
                .map_err(error_db)? as i64,
            creado_en: timestamp(&fila, "creado_en").map_err(error_db)?,
        }))
    }

    /// GMV real del mes para comparar con la proyección (tabla 8.10).
    pub async fn leer_gmv_real(&self, db: &Db, mes: &str) -> Result<Option<i64>, ApiError> {
        let inicio_mes = format!("{}-01", mes.get(0..7).unwrap_or("2026-01"));
        let total: Option<i32> = sqlx::query_scalar(
            "SELECT monto_cents FROM finance.metricas_mensuales
             WHERE periodo = $1::date AND tipo = 'gmv'",
        )
        .bind(&inicio_mes)
        .fetch_optional(db.pool())
        .await
        .map_err(error_db)?;
        Ok(total.map(|valor| valor as i64))
    }

    /// Serializa el detalle de una proyección para el contrato de respuesta.
    pub fn a_valor(proyeccion: &Proyeccion) -> Value {
        json!(proyeccion)
    }

    /// Utilería de fecha para respuestas de la API.
    pub fn fecha_iso(fecha: DateTime<Utc>) -> String {
        iso_ms(fecha)
    }
}
