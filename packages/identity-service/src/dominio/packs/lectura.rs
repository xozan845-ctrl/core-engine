use super::comunes::*;
use super::prelude::*;
use super::*;

impl Packs {
    pub async fn listar(&self, vertical: Option<&str>) -> Result<Vec<Pack>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(SQL_LISTAR)
            .bind(vertical)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        agrupar(filas)
    }

    pub async fn obtener(&self, codigo: &str, version: i64) -> Result<Pack, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(SQL_OBTENER)
            .bind(codigo)
            .bind(version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        agrupar(filas)?
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::no_encontrado("Pack", &format!("{codigo}@{version}")))
    }

    pub async fn listar_modelos(&self) -> Result<Vec<ModeloOperacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, descripcion, config, activo
               FROM identity.modelos_operacion
              WHERE activo = true
              ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                Ok(ModeloOperacion {
                    codigo: fila.try_get("codigo").map_err(interno)?,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    descripcion: fila.try_get("descripcion").map_err(interno)?,
                    config: fila.try_get("config").map_err(interno)?,
                    activo: fila.try_get("activo").map_err(interno)?,
                })
            })
            .collect::<Result<_, _>>()
    }

    pub async fn listar_verticales(&self) -> Result<Vec<VerticalOperacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, modelo_operacion, activo
               FROM identity.verticales
              WHERE activo = true
              ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                Ok(VerticalOperacion {
                    codigo: fila.try_get("codigo").map_err(interno)?,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    modelo_operacion: fila.try_get("modelo_operacion").map_err(interno)?,
                    activo: fila.try_get("activo").map_err(interno)?,
                })
            })
            .collect::<Result<_, _>>()
    }

    pub async fn regiones(
        &self,
        codigo: &str,
        version: i32,
    ) -> Result<Vec<RegionTributaria>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT 1 FROM identity.packs WHERE codigo = $1 AND version = $2 AND activo",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado(
                "Pack",
                &format!("{codigo}@{version}"),
            ));
        }
        let filas = sqlx::query(
            "SELECT j.codigo_pais, j.nombre, j.pais_iso, j.subdivision, j.moneda,
                    j.moneda_iso, j.simbolo_moneda, j.locale, j.zona_horaria,
                    j.tasa_iva, j.tasa_ir, j.periodicidad_declaracion,
                    j.documento_fiscal, j.perfil, j.leyes, j.vigente_desde, j.activa,
                    pr.predeterminada
               FROM identity.pack_regiones pr
               JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
              WHERE pr.pack_codigo = $1 AND pr.pack_version = $2
              ORDER BY pr.predeterminada DESC, j.codigo_pais",
        )
        .bind(codigo)
        .bind(version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(region_de_fila).collect()
    }
}

const SQL_LISTAR: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos,
        COALESCE(regiones.items, '[]'::jsonb) AS regiones
   FROM identity.packs p
   LEFT JOIN identity.pack_capacidades pc
     ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
   LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'modelo_codigo', pm.modelo_codigo,
       'nombre', mo.nombre,
       'descripcion', mo.descripcion,
       'activo', pm.activo,
       'config', pm.config
     ) ORDER BY pm.modelo_codigo) AS items
       FROM identity.pack_modelos pm
       JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
      WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
   ) modelos ON true
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'codigo', j.codigo_pais,
       'nombre', j.nombre,
       'pais_iso', j.pais_iso,
       'subdivision', j.subdivision,
       'moneda', j.moneda,
       'moneda_iso', j.moneda_iso,
       'simbolo_moneda', j.simbolo_moneda,
       'locale', j.locale,
       'zona_horaria', j.zona_horaria,
       'tasa_iva', j.tasa_iva,
       'tasa_ir', j.tasa_ir,
       'periodicidad_declaracion', j.periodicidad_declaracion,
       'documento_fiscal', j.documento_fiscal,
       'perfil', j.perfil,
       'leyes', j.leyes,
       'vigente_desde', j.vigente_desde,
       'predeterminada', pr.predeterminada
     ) ORDER BY pr.predeterminada DESC, j.codigo_pais) AS items
       FROM identity.pack_regiones pr
       JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
      WHERE pr.pack_codigo = p.codigo AND pr.pack_version = p.version
        AND pr.activo AND j.activa
   ) regiones ON true
  WHERE p.activo = true
    AND ($1::text IS NULL OR p.vertical_codigo = $1)
  ORDER BY p.vertical_codigo, p.codigo, pc.capacidad_codigo";

const SQL_OBTENER: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos,
        COALESCE(regiones.items, '[]'::jsonb) AS regiones
   FROM identity.packs p
   LEFT JOIN identity.pack_capacidades pc
     ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
   LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'modelo_codigo', pm.modelo_codigo,
       'nombre', mo.nombre,
       'descripcion', mo.descripcion,
       'activo', pm.activo,
       'config', pm.config
     ) ORDER BY pm.modelo_codigo) AS items
       FROM identity.pack_modelos pm
       JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
      WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
   ) modelos ON true
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'codigo', j.codigo_pais,
       'nombre', j.nombre,
       'pais_iso', j.pais_iso,
       'subdivision', j.subdivision,
       'moneda', j.moneda,
       'moneda_iso', j.moneda_iso,
       'simbolo_moneda', j.simbolo_moneda,
       'locale', j.locale,
       'zona_horaria', j.zona_horaria,
       'tasa_iva', j.tasa_iva,
       'tasa_ir', j.tasa_ir,
       'periodicidad_declaracion', j.periodicidad_declaracion,
       'documento_fiscal', j.documento_fiscal,
       'perfil', j.perfil,
       'leyes', j.leyes,
       'vigente_desde', j.vigente_desde,
       'predeterminada', pr.predeterminada
     ) ORDER BY pr.predeterminada DESC, j.codigo_pais) AS items
       FROM identity.pack_regiones pr
       JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
      WHERE pr.pack_codigo = p.codigo AND pr.pack_version = p.version
        AND pr.activo AND j.activa
   ) regiones ON true
  WHERE p.codigo = $1 AND p.version = $2 AND p.activo = true
  ORDER BY pc.capacidad_codigo";
