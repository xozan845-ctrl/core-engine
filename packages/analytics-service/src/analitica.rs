//! Analítica de negocio, empresa y plataforma para consumo del frontend.
//!
//! No renderiza nada: entrega JSON/CSV con totales, desglose por negocio y
//! evolución mensual. La empresa sigue siendo el límite de datos.

use crate::db::{error_db, Db};
use crate::errors::ApiError;
use serde_json::{json, Value};
use sqlx::Row;

pub async fn resumen_negocios(
    db: &Db,
    empresa_id: &str,
    negocio_id: Option<&str>,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Value, ApiError> {
    let filas = sqlx::query(
        "WITH ventas AS (
           SELECT COALESCE(h.negocio_id, '00000000-0000-0000-0000-000000000000'::uuid)
                    AS negocio_id,
                  COUNT(DISTINCT h.order_id)::bigint AS ordenes,
                  COUNT(DISTINCT h.comprador_id)::bigint AS clientes_unicos,
                  COALESCE(SUM(h.venta_cents), 0)::bigint AS ventas_cents
             FROM analytics.hechos_venta h
            WHERE h.empresa_id = $1::uuid
              AND ($2::uuid IS NULL OR h.negocio_id = $2::uuid)
              AND ($3::timestamptz IS NULL OR h.orden_creada_en >= $3::timestamptz)
              AND ($4::timestamptz IS NULL OR h.orden_creada_en <= $4::timestamptz)
            GROUP BY h.negocio_id
         ),
         comisiones AS (
           SELECT COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid)
                    AS negocio_id,
                  COALESCE(SUM(comision_cents), 0)::bigint AS comisiones_cents
             FROM analytics.comisiones
            WHERE empresa_id = $1::uuid
              AND ($2::uuid IS NULL OR negocio_id = $2::uuid)
              AND ($3::timestamptz IS NULL OR creado_en >= $3::timestamptz)
              AND ($4::timestamptz IS NULL OR creado_en <= $4::timestamptz)
            GROUP BY negocio_id
         )
         SELECT NULLIF(COALESCE(v.negocio_id, c.negocio_id),
                       '00000000-0000-0000-0000-000000000000'::uuid) AS negocio_id,
                n.nombre AS negocio_nombre,
                COALESCE(v.ordenes, 0) AS ordenes,
                COALESCE(v.clientes_unicos, 0) AS clientes_unicos,
                COALESCE(v.ventas_cents, 0) AS ventas_cents,
                COALESCE(c.comisiones_cents, 0) AS comisiones_cents
           FROM ventas v
           FULL OUTER JOIN comisiones c ON c.negocio_id = v.negocio_id
           LEFT JOIN identity.negocios n
             ON n.id = NULLIF(COALESCE(v.negocio_id, c.negocio_id),
                              '00000000-0000-0000-0000-000000000000'::uuid)
          ORDER BY ventas_cents DESC",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .bind(desde)
    .bind(hasta)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;

    let mut negocios = Vec::with_capacity(filas.len());
    for fila in &filas {
        let ventas: i64 = fila.try_get("ventas_cents").map_err(error_db)?;
        let comisiones: i64 = fila.try_get("comisiones_cents").map_err(error_db)?;
        let ordenes: i64 = fila.try_get("ordenes").map_err(error_db)?;
        negocios.push(json!({
            "negocio_id": fila.try_get::<Option<uuid::Uuid>, _>("negocio_id").map_err(error_db)?,
            "negocio_nombre": fila.try_get::<Option<String>, _>("negocio_nombre").map_err(error_db)?,
            "ordenes": ordenes,
            "clientes_unicos": fila.try_get::<i64, _>("clientes_unicos").map_err(error_db)?,
            "ventas_cents": ventas,
            "comisiones_cents": comisiones,
            "margen_estimado_cents": ventas - comisiones,
            "ticket_promedio_cents": if ordenes > 0 { ventas / ordenes } else { 0 },
        }));
    }

    let total_ventas: i64 = negocios
        .iter()
        .filter_map(|n| n.get("ventas_cents").and_then(Value::as_i64))
        .sum();
    let total_comisiones: i64 = negocios
        .iter()
        .filter_map(|n| n.get("comisiones_cents").and_then(Value::as_i64))
        .sum();
    let total_ordenes: i64 = negocios
        .iter()
        .filter_map(|n| n.get("ordenes").and_then(Value::as_i64))
        .sum();

    let evolucion = sqlx::query(
        "SELECT h.negocio_id,
                TO_CHAR(h.orden_creada_en, 'YYYY-MM') AS periodo,
                COUNT(DISTINCT h.order_id)::bigint AS ordenes,
                COALESCE(SUM(h.venta_cents), 0)::bigint AS ventas_cents
           FROM analytics.hechos_venta h
          WHERE h.empresa_id = $1::uuid
            AND ($2::uuid IS NULL OR h.negocio_id = $2::uuid)
            AND ($3::timestamptz IS NULL OR h.orden_creada_en >= $3::timestamptz)
            AND ($4::timestamptz IS NULL OR h.orden_creada_en <= $4::timestamptz)
          GROUP BY h.negocio_id, periodo
          ORDER BY periodo, h.negocio_id",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .bind(desde)
    .bind(hasta)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;
    let evolucion = evolucion
        .iter()
        .map(|fila| {
            Ok(json!({
                "negocio_id": fila.try_get::<Option<uuid::Uuid>, _>("negocio_id").map_err(error_db)?,
                "periodo": fila.try_get::<String, _>("periodo").map_err(error_db)?,
                "ordenes": fila.try_get::<i64, _>("ordenes").map_err(error_db)?,
                "ventas_cents": fila.try_get::<i64, _>("ventas_cents").map_err(error_db)?,
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(json!({
        "empresa_id": empresa_id,
        "negocio_id": negocio_id,
        "desde": desde,
        "hasta": hasta,
        "totales": {
            "ordenes": total_ordenes,
            "ventas_cents": total_ventas,
            "comisiones_cents": total_comisiones,
            "margen_estimado_cents": total_ventas - total_comisiones,
            "ticket_promedio_cents": if total_ordenes > 0 { total_ventas / total_ordenes } else { 0 },
        },
        "negocios": negocios,
        "evolucion": evolucion,
    }))
}

pub fn resumen_csv(resumen: &Value) -> String {
    let mut salida = String::from(
        "negocio_id,negocio_nombre,ordenes,clientes_unicos,ventas_cents,comisiones_cents,margen_estimado_cents,ticket_promedio_cents\n",
    );
    let Some(negocios) = resumen.get("negocios").and_then(Value::as_array) else {
        return salida;
    };
    for negocio in negocios {
        let texto = |clave: &str| {
            negocio
                .get(clave)
                .map(|valor| match valor {
                    Value::String(valor) => valor.replace('"', "\"\""),
                    Value::Null => String::new(),
                    otro => otro.to_string(),
                })
                .unwrap_or_default()
        };
        salida.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
            texto("negocio_id"),
            texto("negocio_nombre"),
            texto("ordenes"),
            texto("clientes_unicos"),
            texto("ventas_cents"),
            texto("comisiones_cents"),
            texto("margen_estimado_cents"),
            texto("ticket_promedio_cents"),
        ));
    }
    salida
}

/// Cohortes de clientes por mes de primera compra y retención posterior.
pub async fn cohortes_negocios(
    db: &Db,
    empresa_id: &str,
    negocio_id: Option<&str>,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Value, ApiError> {
    let filas = sqlx::query(
        "WITH compras AS (
           SELECT h.comprador_id,
                  h.order_id,
                  date_trunc('month', h.orden_creada_en)::date AS mes,
                  COALESCE(SUM(h.venta_cents), 0)::bigint AS ventas_cents
             FROM analytics.hechos_venta h
            WHERE h.empresa_id = $1::uuid
              AND ($2::uuid IS NULL OR h.negocio_id = $2::uuid)
              AND ($3::timestamptz IS NULL OR h.orden_creada_en >= $3::timestamptz)
              AND ($4::timestamptz IS NULL OR h.orden_creada_en <= $4::timestamptz)
            GROUP BY h.comprador_id, h.order_id, mes
         ),
         primera AS (
           SELECT comprador_id, MIN(mes) AS cohorte
             FROM compras
            GROUP BY comprador_id
         ),
         actividad AS (
           SELECT TO_CHAR(p.cohorte, 'YYYY-MM') AS cohorte,
                  ((EXTRACT(YEAR FROM c.mes) - EXTRACT(YEAR FROM p.cohorte)) * 12
                   + (EXTRACT(MONTH FROM c.mes) - EXTRACT(MONTH FROM p.cohorte)))::int
                    AS indice,
                  TO_CHAR(p.cohorte + make_interval(months => (
                    (EXTRACT(YEAR FROM c.mes) - EXTRACT(YEAR FROM p.cohorte)) * 12
                    + (EXTRACT(MONTH FROM c.mes) - EXTRACT(MONTH FROM p.cohorte))
                  )::int), 'YYYY-MM') AS periodo,
                  COUNT(DISTINCT c.comprador_id)::bigint AS clientes,
                  COUNT(DISTINCT c.order_id)::bigint AS ordenes,
                  COALESCE(SUM(c.ventas_cents), 0)::bigint AS ventas_cents
             FROM compras c
             JOIN primera p USING (comprador_id)
            GROUP BY p.cohorte, indice
         )
         SELECT cohorte, indice, periodo, clientes, ordenes, ventas_cents
           FROM actividad
          ORDER BY cohorte, indice",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .bind(desde)
    .bind(hasta)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;

    let mut cohortes: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for fila in &filas {
        let cohorte: String = fila.try_get("cohorte").map_err(error_db)?;
        let clientes: i64 = fila.try_get("clientes").map_err(error_db)?;
        cohortes.entry(cohorte).or_default().push(json!({
            "indice": fila.try_get::<i32, _>("indice").map_err(error_db)?,
            "periodo": fila.try_get::<String, _>("periodo").map_err(error_db)?,
            "clientes": clientes,
            "ordenes": fila.try_get::<i64, _>("ordenes").map_err(error_db)?,
            "ventas_cents": fila.try_get::<i64, _>("ventas_cents").map_err(error_db)?,
        }));
    }

    let mut total_clientes = 0i64;
    let mut retencion_mes_1 = 0.0f64;
    let mut cohorts = Vec::with_capacity(cohortes.len());
    for (periodo, mut meses) in cohortes {
        let tamano = meses
            .iter()
            .find(|mes| mes["indice"] == 0)
            .and_then(|mes| mes["clientes"].as_i64())
            .unwrap_or(0);
        total_clientes += tamano;
        for mes in &mut meses {
            let clientes = mes["clientes"].as_i64().unwrap_or(0);
            let retencion = if tamano > 0 {
                (clientes as f64 * 10_000.0 / tamano as f64).round() / 100.0
            } else {
                0.0
            };
            mes["retencion_pct"] = json!(retencion);
            if mes["indice"] == 1 {
                retencion_mes_1 += retencion * tamano as f64;
            }
        }
        cohorts.push(json!({
            "periodo": periodo,
            "clientes": tamano,
            "meses": meses,
        }));
    }
    let retencion_mes_1 = if total_clientes > 0 {
        (retencion_mes_1 / total_clientes as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };

    Ok(json!({
        "empresa_id": empresa_id,
        "negocio_id": negocio_id,
        "desde": desde,
        "hasta": hasta,
        "totales": {
            "cohortes": cohorts.len(),
            "clientes": total_clientes,
            "retencion_mes_1_pct": retencion_mes_1,
        },
        "cohortes": cohorts,
    }))
}

pub fn cohortes_csv(resumen: &Value) -> String {
    let mut salida =
        String::from("cohorte,indice,periodo,clientes,ordenes,ventas_cents,retencion_pct\n");
    let Some(cohortes) = resumen.get("cohortes").and_then(Value::as_array) else {
        return salida;
    };
    for cohorte in cohortes {
        let periodo = cohorte
            .get("periodo")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(meses) = cohorte.get("meses").and_then(Value::as_array) else {
            continue;
        };
        for mes in meses {
            let valor = |clave: &str| {
                mes.get(clave)
                    .map(|valor| match valor {
                        Value::String(valor) => valor.replace('"', "\"\""),
                        Value::Null => String::new(),
                        otro => otro.to_string(),
                    })
                    .unwrap_or_default()
            };
            salida.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                periodo,
                valor("indice"),
                valor("periodo"),
                valor("clientes"),
                valor("ordenes"),
                valor("ventas_cents"),
                valor("retencion_pct"),
            ));
        }
    }
    salida
}

/// Consolidado de toda la plataforma: una fila por empresa.
pub async fn resumen_plataforma(
    db: &Db,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Value, ApiError> {
    let filas = sqlx::query(
        "WITH ventas AS (
           SELECT h.empresa_id,
                  COUNT(DISTINCT h.order_id)::bigint AS ordenes,
                  COUNT(DISTINCT h.comprador_id)::bigint AS clientes_unicos,
                  COUNT(DISTINCT h.negocio_id)::bigint AS negocios,
                  COALESCE(SUM(h.venta_cents), 0)::bigint AS ventas_cents
             FROM analytics.hechos_venta h
            WHERE ($1::timestamptz IS NULL OR h.orden_creada_en >= $1::timestamptz)
              AND ($2::timestamptz IS NULL OR h.orden_creada_en <= $2::timestamptz)
            GROUP BY h.empresa_id
         ),
         comisiones AS (
           SELECT c.empresa_id,
                  COALESCE(SUM(c.comision_cents), 0)::bigint AS comisiones_cents
             FROM analytics.comisiones c
            WHERE ($1::timestamptz IS NULL OR c.creado_en >= $1::timestamptz)
              AND ($2::timestamptz IS NULL OR c.creado_en <= $2::timestamptz)
            GROUP BY c.empresa_id
         )
         SELECT v.empresa_id,
                e.nombre AS empresa_nombre,
                v.ordenes,
                v.clientes_unicos,
                v.negocios,
                v.ventas_cents,
                COALESCE(c.comisiones_cents, 0) AS comisiones_cents
           FROM ventas v
           LEFT JOIN comisiones c ON c.empresa_id = v.empresa_id
           LEFT JOIN identity.empresas e ON e.id = v.empresa_id
          ORDER BY v.ventas_cents DESC",
    )
    .bind(desde)
    .bind(hasta)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;

    let mut empresas = Vec::with_capacity(filas.len());
    for fila in &filas {
        let ventas: i64 = fila.try_get("ventas_cents").map_err(error_db)?;
        let comisiones: i64 = fila.try_get("comisiones_cents").map_err(error_db)?;
        let ordenes: i64 = fila.try_get("ordenes").map_err(error_db)?;
        empresas.push(json!({
            "empresa_id": fila.try_get::<uuid::Uuid, _>("empresa_id").map_err(error_db)?,
            "empresa_nombre": fila.try_get::<Option<String>, _>("empresa_nombre").map_err(error_db)?,
            "negocios": fila.try_get::<i64, _>("negocios").map_err(error_db)?,
            "ordenes": ordenes,
            "clientes_unicos": fila.try_get::<i64, _>("clientes_unicos").map_err(error_db)?,
            "ventas_cents": ventas,
            "comisiones_cents": comisiones,
            "margen_estimado_cents": ventas - comisiones,
            "ticket_promedio_cents": if ordenes > 0 { ventas / ordenes } else { 0 },
        }));
    }

    let total_ventas: i64 = empresas
        .iter()
        .filter_map(|e| e.get("ventas_cents").and_then(Value::as_i64))
        .sum();
    let total_comisiones: i64 = empresas
        .iter()
        .filter_map(|e| e.get("comisiones_cents").and_then(Value::as_i64))
        .sum();
    let total_ordenes: i64 = empresas
        .iter()
        .filter_map(|e| e.get("ordenes").and_then(Value::as_i64))
        .sum();

    let evolucion = sqlx::query(
        "SELECT TO_CHAR(h.orden_creada_en, 'YYYY-MM') AS periodo,
                COUNT(DISTINCT h.order_id)::bigint AS ordenes,
                COALESCE(SUM(h.venta_cents), 0)::bigint AS ventas_cents
           FROM analytics.hechos_venta h
          WHERE ($1::timestamptz IS NULL OR h.orden_creada_en >= $1::timestamptz)
            AND ($2::timestamptz IS NULL OR h.orden_creada_en <= $2::timestamptz)
          GROUP BY 1
          ORDER BY 1",
    )
    .bind(desde)
    .bind(hasta)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;
    let evolucion = evolucion
        .iter()
        .map(|fila| {
            Ok(json!({
                "periodo": fila.try_get::<String, _>("periodo").map_err(error_db)?,
                "ordenes": fila.try_get::<i64, _>("ordenes").map_err(error_db)?,
                "ventas_cents": fila.try_get::<i64, _>("ventas_cents").map_err(error_db)?,
            }))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(json!({
        "desde": desde,
        "hasta": hasta,
        "totales": {
            "empresas": empresas.len(),
            "ordenes": total_ordenes,
            "ventas_cents": total_ventas,
            "comisiones_cents": total_comisiones,
            "margen_estimado_cents": total_ventas - total_comisiones,
            "ticket_promedio_cents": if total_ordenes > 0 { total_ventas / total_ordenes } else { 0 },
        },
        "empresas": empresas,
        "evolucion": evolucion,
    }))
}

pub fn plataforma_csv(resumen: &Value) -> String {
    let mut salida = String::from(
        "empresa_id,empresa_nombre,negocios,ordenes,clientes_unicos,ventas_cents,comisiones_cents,margen_estimado_cents,ticket_promedio_cents\n",
    );
    let Some(empresas) = resumen.get("empresas").and_then(Value::as_array) else {
        return salida;
    };
    for empresa in empresas {
        let valor = |clave: &str| {
            empresa
                .get(clave)
                .map(|valor| match valor {
                    Value::String(valor) => valor.replace('"', "\"\""),
                    Value::Null => String::new(),
                    otro => otro.to_string(),
                })
                .unwrap_or_default()
        };
        salida.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
            valor("empresa_id"),
            valor("empresa_nombre"),
            valor("negocios"),
            valor("ordenes"),
            valor("clientes_unicos"),
            valor("ventas_cents"),
            valor("comisiones_cents"),
            valor("margen_estimado_cents"),
            valor("ticket_promedio_cents"),
        ));
    }
    salida
}
