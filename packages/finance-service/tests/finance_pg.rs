//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida). Provisionan su propio esquema `finance` para correr en CI.

use async_trait::async_trait;
use finance_service::auth::UsuarioContexto;
use finance_service::comercio::{Comercio, CreditoInterno, CuentaCreditoInput, Gasto};
use finance_service::consumidor::Consumidor;
use finance_service::contabilidad::{Contabilidad, DetalleAsiento, ParametrosAsiento};
use finance_service::db::Db;
use finance_service::facturacion::{
    Comprobante, EmitirComprobante, Facturacion, OrdenResumen, Ordenes,
};
use finance_service::finanzas::proyecciones::{Proyecciones, SupuestosProyeccion};
use finance_service::outbox::Outbox;
use finance_service::tributacion::{FiltrosDeclaracion, PeriodoMensual, Tributacion};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

fn usuario(rol: &str, tenant: Option<&str>) -> UsuarioContexto {
    UsuarioContexto {
        user_id: "11111111-1111-4111-8111-111111111111".into(),
        rol: rol.into(),
        tenant_id: tenant.map(str::to_string),
        ..Default::default()
    }
}

struct OrdenesFijas;

#[async_trait]
impl Ordenes for OrdenesFijas {
    async fn consultar(&self, order_id: &str) -> Option<OrdenResumen> {
        (order_id == "22222222-2222-4222-8222-222222222222").then_some(OrdenResumen {
            total_cents: 100_000,
        })
    }
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(
        r#"
        DROP SCHEMA IF EXISTS finance CASCADE;
        CREATE SCHEMA finance;
        CREATE SCHEMA IF NOT EXISTS identity;

        CREATE TABLE IF NOT EXISTS identity.partes (
          id uuid PRIMARY KEY,
          empresa_id uuid NOT NULL,
          nombre text NOT NULL,
          eliminado_en timestamptz
        );

        CREATE TABLE finance.plan_cuentas (
          codigo text PRIMARY KEY,
          nombre text NOT NULL,
          tipo text NOT NULL,
          naturaleza text NOT NULL,
          nivel integer NOT NULL DEFAULT 1,
          estado text NOT NULL DEFAULT 'activa'
        );
        CREATE TABLE finance.asientos (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          fecha timestamptz NOT NULL DEFAULT NOW(),
          concepto text NOT NULL,
          tipo text NOT NULL DEFAULT 'MANUAL',
          referencia_tipo text,
          referencia_id text,
          moneda text NOT NULL DEFAULT 'C$',
          estado text NOT NULL DEFAULT 'REGISTRADO',
          creado_por text,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE finance.asiento_detalles (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          asiento_id uuid NOT NULL REFERENCES finance.asientos(id) ON DELETE CASCADE,
          cuenta_codigo text NOT NULL,
          debe_cents integer NOT NULL DEFAULT 0,
          haber_cents integer NOT NULL DEFAULT 0,
          concepto text,
          orden integer NOT NULL DEFAULT 0
        );
        CREATE TABLE finance.asientos_vista (
          id uuid PRIMARY KEY,
          fecha timestamptz NOT NULL,
          concepto text NOT NULL,
          tipo text NOT NULL,
          referencia_tipo text,
          referencia_id text,
          moneda text NOT NULL,
          estado text NOT NULL,
          creado_por text,
          creado_en timestamptz NOT NULL,
          debe_cents integer NOT NULL,
          haber_cents integer NOT NULL,
          periodo date NOT NULL,
          detalles_jsonb jsonb NOT NULL DEFAULT '[]'
        );
        CREATE TABLE finance.libro_mayor_mv (
          cuenta_codigo text NOT NULL,
          periodo date NOT NULL,
          debe_cents integer NOT NULL DEFAULT 0,
          haber_cents integer NOT NULL DEFAULT 0,
          saldo_cents integer NOT NULL DEFAULT 0,
          movimientos_jsonb jsonb NOT NULL DEFAULT '[]',
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          PRIMARY KEY (cuenta_codigo, periodo)
        );
        CREATE OR REPLACE FUNCTION finance.fn_refrescar_mes(p_mes date)
        RETURNS void LANGUAGE plpgsql AS $$
        BEGIN
          DELETE FROM finance.asientos_vista WHERE periodo = p_mes;
          INSERT INTO finance.asientos_vista
            (id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado,
             creado_por, creado_en, debe_cents, haber_cents, periodo, detalles_jsonb)
          SELECT a.id, a.fecha, a.concepto, a.tipo, a.referencia_tipo, a.referencia_id,
                 a.moneda, a.estado, a.creado_por, a.creado_en,
                 COALESCE(d.debe_cents, 0), COALESCE(d.haber_cents, 0),
                 date_trunc('month', a.fecha)::date, COALESCE(d.aggs, '[]'::jsonb)
          FROM finance.asientos a
          LEFT JOIN (
            SELECT asiento_id, SUM(debe_cents) AS debe_cents, SUM(haber_cents) AS haber_cents,
                   jsonb_agg(jsonb_build_object(
                     'cuenta_codigo', cuenta_codigo, 'debe_cents', debe_cents,
                     'haber_cents', haber_cents, 'concepto', concepto, 'orden', orden
                   ) ORDER BY orden) AS aggs
            FROM finance.asiento_detalles GROUP BY asiento_id
          ) d ON d.asiento_id = a.id
          WHERE date_trunc('month', a.fecha)::date = p_mes;

          DELETE FROM finance.libro_mayor_mv WHERE periodo = p_mes;
          INSERT INTO finance.libro_mayor_mv
            (cuenta_codigo, periodo, debe_cents, haber_cents, saldo_cents, movimientos_jsonb)
          SELECT d.cuenta_codigo, p_mes, SUM(d.debe_cents), SUM(d.haber_cents),
                 CASE WHEN MAX(c.naturaleza) = 'DEUDORA'
                      THEN SUM(d.debe_cents) - SUM(d.haber_cents)
                      ELSE SUM(d.haber_cents) - SUM(d.debe_cents) END,
                 jsonb_agg(d.mov ORDER BY d.fecha, d.asiento_id)
          FROM (
            SELECT a.id AS asiento_id, a.fecha, d.cuenta_codigo, d.debe_cents, d.haber_cents,
                   jsonb_build_object('asiento_id', a.id, 'fecha', a.fecha,
                     'concepto', d.concepto, 'debe_cents', d.debe_cents,
                     'haber_cents', d.haber_cents, 'orden', d.orden) AS mov
            FROM finance.asiento_detalles d JOIN finance.asientos a ON a.id = d.asiento_id
            WHERE a.estado = 'REGISTRADO' AND date_trunc('month', a.fecha)::date = p_mes
          ) d
          JOIN finance.plan_cuentas c ON c.codigo = d.cuenta_codigo
          GROUP BY d.cuenta_codigo;
        END;
        $$;
        CREATE OR REPLACE FUNCTION finance.fn_trigger_detalle() RETURNS trigger LANGUAGE plpgsql AS $$
        DECLARE mes date;
        BEGIN
          SELECT date_trunc('month', a.fecha)::date INTO mes FROM finance.asientos a
          WHERE a.id = COALESCE(NEW.asiento_id, OLD.asiento_id);
          IF FOUND THEN PERFORM finance.fn_refrescar_mes(mes); END IF;
          RETURN NULL;
        END;
        $$;
        CREATE TRIGGER detalles_proyeccion_trigger
          AFTER INSERT OR UPDATE OR DELETE ON finance.asiento_detalles
          FOR EACH ROW EXECUTE FUNCTION finance.fn_trigger_detalle();

        CREATE TABLE finance.workspace_registros (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          tenant_id uuid,
          propietario_id uuid,
          recurso text NOT NULL,
          datos jsonb NOT NULL DEFAULT '{}'::jsonb,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          eliminado_en timestamptz,
          parte_id uuid
        );
        CREATE TABLE finance.credito_cuentas (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          parte_id uuid NOT NULL,
          limite_cents integer NOT NULL,
          activo boolean NOT NULL DEFAULT true,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          UNIQUE (empresa_id, parte_id)
        );
        CREATE TABLE finance.jurisdicciones (
          codigo_pais text PRIMARY KEY,
          nombre text NOT NULL,
          moneda text NOT NULL,
          simbolo_moneda text NOT NULL,
          tasa_iva double precision NOT NULL,
          tasa_ir double precision NOT NULL,
          periodicidad_declaracion text NOT NULL DEFAULT 'MENSUAL',
          leyes jsonb NOT NULL DEFAULT '{}',
          activa boolean NOT NULL DEFAULT true
        );
        CREATE TABLE finance.regimenes_fiscales (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          jurisdiccion text NOT NULL,
          codigo text NOT NULL,
          nombre text NOT NULL,
          descripcion text NOT NULL DEFAULT '',
          periodicidad text NOT NULL DEFAULT 'MENSUAL',
          condicion_ingresos_anuales_cents integer,
          estado text NOT NULL DEFAULT 'activo',
          UNIQUE (jurisdiccion, codigo)
        );
        CREATE TABLE finance.sujetos_fiscales (
          id uuid PRIMARY KEY,
          jurisdiccion text NOT NULL,
          regimen_id uuid,
          razon_social text NOT NULL,
          ruc text,
          es_plataforma boolean NOT NULL DEFAULT false,
          estado text NOT NULL DEFAULT 'ACTIVO',
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE finance.declaraciones (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          jurisdiccion text NOT NULL,
          tipo text NOT NULL,
          periodo_inicio date NOT NULL,
          periodo_fin date NOT NULL,
          base_gravada_cents integer NOT NULL DEFAULT 0,
          iva_debitado_cents integer NOT NULL DEFAULT 0,
          iva_credito_cents integer NOT NULL DEFAULT 0,
          iva_a_pagar_cents integer NOT NULL DEFAULT 0,
          renta_bruta_cents integer NOT NULL DEFAULT 0,
          renta_gravable_cents integer NOT NULL DEFAULT 0,
          ir_a_pagar_cents integer NOT NULL DEFAULT 0,
          cuota_cents integer NOT NULL DEFAULT 0,
          estado text NOT NULL DEFAULT 'GENERADA',
          detalle jsonb NOT NULL DEFAULT '{}',
          generada_en timestamptz,
          presentada_en timestamptz,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          UNIQUE (jurisdiccion, tipo, periodo_inicio)
        );
        CREATE TABLE finance.comprobantes (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          serie text NOT NULL,
          numero text NOT NULL,
          tipo text NOT NULL,
          orden_id uuid,
          parte_id uuid,
          base_gravada_cents integer NOT NULL,
          iva_cents integer NOT NULL,
          exento_cents integer NOT NULL DEFAULT 0,
          total_cents integer NOT NULL,
          moneda text NOT NULL DEFAULT 'C$',
          estado text NOT NULL DEFAULT 'BORRADOR',
          datos_cliente jsonb,
          emitido_en timestamptz,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE finance.series_comprobantes (
          serie text PRIMARY KEY,
          tipo text NOT NULL,
          prefijo text NOT NULL,
          secuencial_actual integer NOT NULL DEFAULT 0,
          ruc_emisor text NOT NULL,
          jurisdiccion text NOT NULL,
          activa boolean NOT NULL DEFAULT true
        );
        CREATE TABLE finance.proyecciones (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          nombre text NOT NULL,
          horizonte_meses integer NOT NULL,
          supuestos jsonb NOT NULL,
          filas jsonb NOT NULL,
          totales jsonb NOT NULL,
          inversion_inicial_cents integer NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE finance.metricas_mensuales (
          periodo date NOT NULL,
          tipo text NOT NULL,
          monto_cents integer NOT NULL DEFAULT 0,
          cantidad integer NOT NULL DEFAULT 0,
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          PRIMARY KEY (periodo, tipo)
        );
        CREATE TABLE finance.metricas_vendedores (
          vendedor_id uuid NOT NULL,
          periodo date NOT NULL,
          ventas integer NOT NULL DEFAULT 0,
          monto_cents integer NOT NULL DEFAULT 0,
          comision_cents integer NOT NULL DEFAULT 0,
          devoluciones integer NOT NULL DEFAULT 0,
          primera_venta_en timestamptz,
          ultima_venta_en timestamptz,
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          PRIMARY KEY (vendedor_id, periodo)
        );
        CREATE TABLE finance.eventos_procesados (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          procesado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE finance.outbox (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          payload jsonb NOT NULL,
          estado text NOT NULL DEFAULT 'pendiente',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          publicado_en timestamptz
        );

        INSERT INTO finance.plan_cuentas (codigo, nombre, tipo, naturaleza, nivel) VALUES
          ('1.1.1', 'Caja', 'ACTIVO', 'DEUDORA', 3),
          ('2.1.1', 'Acreedores vendedores', 'PASIVO', 'ACREEDORA', 3),
          ('2.1.2', 'Fondos por liquidar', 'PASIVO', 'ACREEDORA', 3),
          ('2.1.3', 'IVA por pagar', 'PASIVO', 'ACREEDORA', 3),
          ('2.1.6', 'Acreedores empresarios', 'PASIVO', 'ACREEDORA', 3),
          ('4.1', 'Ingresos por comisiones', 'INGRESO', 'ACREEDORA', 1),
          ('5.1', 'Gastos operativos', 'GASTO', 'DEUDORA', 1);

        INSERT INTO finance.jurisdicciones
          (codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir, leyes)
        VALUES ('NI', 'Nicaragua', 'NIO', 'C$', 0.15, 0.30, '{"iva":"Ley 822"}');

        INSERT INTO finance.series_comprobantes
          (serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion)
        VALUES ('F001', 'FACTURA', 'F001-', 0, 'RUC-1', 'NI'),
               ('NC001', 'NOTA_CREDITO', 'NC001-', 0, 'RUC-1', 'NI');
        "#,
    )
    .execute(&pool)
    .await
    .expect("migracion finance");
    pool.close().await;
}

#[tokio::test]
async fn debe_registrar_asientos_y_leer_libros() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let contabilidad = Contabilidad::nueva(Arc::clone(&db), Outbox::nueva("finance.outbox"));

    let asiento = contabilidad
        .registrar(ParametrosAsiento {
            concepto: "Venta de prueba".into(),
            tipo: "INGRESO".into(),
            referencia_tipo: Some("order".into()),
            referencia_id: Some("orden-1".into()),
            fecha: None,
            moneda: None,
            creado_por: Some("tester".into()),
            detalles: vec![
                DetalleAsiento {
                    cuenta_codigo: "1.1.1".into(),
                    debe_cents: 100_000,
                    haber_cents: 0,
                    concepto: Some("Caja".into()),
                    orden: None,
                },
                DetalleAsiento {
                    cuenta_codigo: "2.1.2".into(),
                    debe_cents: 0,
                    haber_cents: 100_000,
                    concepto: Some("Fondos por liquidar".into()),
                    orden: None,
                },
            ],
        })
        .await
        .expect("asiento");
    assert_eq!(asiento.debe_cents, 100_000);
    assert_eq!(asiento.haber_cents, 100_000);

    let libro = contabilidad
        .libro_diario(None, None, 10)
        .await
        .expect("libro diario");
    assert_eq!(libro.len(), 1);
    assert_eq!(libro[0].concepto, "Venta de prueba");
    assert_eq!(libro[0].detalles.len(), 2);

    let mayor = contabilidad
        .libro_mayor("1.1.1", None)
        .await
        .expect("libro mayor");
    assert_eq!(mayor.saldo_final_cents, 100_000);
    assert_eq!(mayor.movimientos.len(), 1);

    let evento: (String, String) =
        sqlx::query_as("SELECT tipo, estado FROM finance.outbox LIMIT 1")
            .fetch_one(db.pool())
            .await
            .expect("outbox");
    assert_eq!(evento.0, "asiento.registrado");
    assert_eq!(evento.1, "pendiente");

    let anulado = contabilidad.anular(&asiento.id).await.expect("anular");
    assert_eq!(anulado.expect("asiento").estado, "ANULADO");
    assert!(contabilidad
        .anular(&Uuid::new_v4().to_string())
        .await
        .expect("inexistente")
        .is_none());
}

#[tokio::test]
async fn debe_rechazar_asientos_descuadrados_en_la_base() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let contabilidad = Contabilidad::nueva(Arc::clone(&db), Outbox::nueva("finance.outbox"));
    let error = contabilidad
        .registrar(ParametrosAsiento {
            concepto: "Descuadre".into(),
            tipo: "MANUAL".into(),
            referencia_tipo: None,
            referencia_id: None,
            fecha: None,
            moneda: None,
            creado_por: None,
            detalles: vec![
                DetalleAsiento {
                    cuenta_codigo: "1.1.1".into(),
                    debe_cents: 100,
                    haber_cents: 0,
                    concepto: None,
                    orden: None,
                },
                DetalleAsiento {
                    cuenta_codigo: "2.1.2".into(),
                    debe_cents: 0,
                    haber_cents: 90,
                    concepto: None,
                    orden: None,
                },
            ],
        })
        .await
        .expect_err("descuadrado");
    assert_eq!(error.codigo, "PARTIDA_DOBLE_DESCUADRADA");
}

#[tokio::test]
async fn debe_aislar_el_libro_de_comercio_por_ambito() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let comercio = Comercio::nuevo(Arc::clone(&db));

    let vendedor = usuario("vendedor", None);
    let otro = UsuarioContexto {
        user_id: "33333333-3333-4333-8333-333333333333".into(),
        rol: "vendedor".into(),
        ..Default::default()
    };
    let creado = comercio
        .crear(
            "clientes",
            json!({"name": "Ana", "phone": "8888-1111"}),
            &vendedor,
        )
        .await
        .expect("cliente");
    assert_eq!(creado["name"], "Ana");
    assert!(comercio
        .listar("clientes", &otro, None)
        .await
        .expect("listado")
        .is_empty());
    assert_eq!(
        comercio
            .listar("clientes", &vendedor, None)
            .await
            .expect("listado")
            .len(),
        1
    );

    let empresario = usuario("empresario", Some("44444444-4444-4444-8444-444444444444"));
    let gasto = comercio
        .crear_gasto(
            Gasto {
                date: "2026-11-13".into(),
                concept: "Luz".into(),
                category: "servicios".into(),
                tipo: "inmediato".into(),
                amount_cents: 1_500,
                payment_method: "efectivo".into(),
                ..Default::default()
            },
            &empresario,
        )
        .await
        .expect("gasto");
    assert_eq!(gasto["status"], "pendiente");
    assert!(comercio
        .crear_gasto(
            Gasto {
                date: "2026-11-13".into(),
                concept: "Luz".into(),
                category: "x".into(),
                tipo: "inmediato".into(),
                amount_cents: 1,
                payment_method: "efectivo".into(),
                ..Default::default()
            },
            &vendedor,
        )
        .await
        .is_err());
}

#[tokio::test]
async fn debe_registrar_y_compensar_el_credito_de_una_orden() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let comercio = Comercio::nuevo(Arc::clone(&db));
    let empresa = "44444444-4444-4444-8444-444444444444";
    let parte = "55555555-5555-4555-8555-555555555555";
    sqlx::query("INSERT INTO identity.partes (id, empresa_id, nombre) VALUES ($1, $2, 'Ana')")
        .bind(Uuid::parse_str(parte).unwrap())
        .bind(Uuid::parse_str(empresa).unwrap())
        .execute(db.pool())
        .await
        .expect("parte");
    let empresario = usuario("empresario", Some(empresa));
    let cuenta = comercio
        .guardar_cuenta_credito(
            parte,
            CuentaCreditoInput {
                limite_cents: 10_000,
                activo: Some(true),
            },
            &empresario,
            None,
        )
        .await
        .expect("cuenta");
    assert_eq!(cuenta.disponible_cents, 10_000);

    let entrada = CreditoInterno {
        empresa_id: empresa.into(),
        parte_id: parte.into(),
        order_id: "22222222-2222-4222-8222-222222222222".into(),
        monto_cents: 4_000,
        concepto: "Pedido".into(),
        vendedor_id: None,
    };
    let credito = comercio
        .registrar_credito_orden(entrada.clone())
        .await
        .expect("credito");
    let credito_id = credito["credito_id"].as_str().expect("id").to_string();
    let repetido = comercio
        .registrar_credito_orden(entrada)
        .await
        .expect("idempotente");
    assert_eq!(repetido["credito_id"].as_str(), Some(credito_id.as_str()));

    let cuentas = comercio
        .listar_cuentas_credito(&empresario, None)
        .await
        .expect("cuentas");
    assert_eq!(cuentas[0].usado_cents, 4_000);
    assert_eq!(cuentas[0].disponible_cents, 6_000);

    assert!(comercio
        .anular_credito_orden("22222222-2222-4222-8222-222222222222")
        .await
        .expect("anular"));
    assert!(!comercio
        .anular_credito_orden("22222222-2222-4222-8222-222222222222")
        .await
        .expect("anular de nuevo"));
}

#[tokio::test]
async fn debe_generar_declaraciones_idempotentes() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let contabilidad = Arc::new(Contabilidad::nueva(
        Arc::clone(&db),
        Outbox::nueva("finance.outbox"),
    ));
    let tributacion = Tributacion::nueva(
        Arc::clone(&db),
        Outbox::nueva("finance.outbox"),
        Arc::clone(&contabilidad),
    );
    let periodo = PeriodoMensual {
        inicio: "2026-10-01".into(),
        fin: "2026-10-31".into(),
    };
    let generadas = tributacion
        .generar_declaraciones(Some(periodo.clone()))
        .await
        .expect("declaraciones");
    let tipos: Vec<&str> = generadas.iter().map(|d| d.tipo.as_str()).collect();
    assert!(tipos.contains(&"IVA"));
    assert!(tipos.contains(&"IR"));
    assert_eq!(tipos.len(), 2);

    let repetidas = tributacion
        .generar_declaraciones(Some(periodo))
        .await
        .expect("repetidas");
    let ids: Vec<&str> = generadas.iter().map(|d| d.id.as_str()).collect();
    let ids_repetidos: Vec<&str> = repetidas.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ids_repetidos);

    let presentada = tributacion
        .presentar(&generadas[0].id)
        .await
        .expect("presentar")
        .expect("declaracion");
    assert_eq!(presentada.estado, "PRESENTADA");
    let pagada = tributacion
        .marcar_pagada(&generadas[0].id)
        .await
        .expect("pagar")
        .expect("declaracion");
    assert_eq!(pagada.estado, "PAGADA");

    let listado = tributacion
        .declaraciones(FiltrosDeclaracion {
            tipo: Some("IVA".into()),
            periodo_inicio: None,
            estado: None,
        })
        .await
        .expect("listado");
    assert_eq!(listado.len(), 1);
}

#[tokio::test]
async fn debe_emitir_comprobantes_con_secuencial_consecutivo() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let contabilidad = Arc::new(Contabilidad::nueva(
        Arc::clone(&db),
        Outbox::nueva("finance.outbox"),
    ));
    let facturacion = Facturacion::nueva(
        Arc::clone(&db),
        Outbox::nueva("finance.outbox"),
        Arc::clone(&contabilidad),
        Arc::new(OrdenesFijas),
        false,
    );
    let primero: Comprobante = facturacion
        .emitir(EmitirComprobante {
            tipo: "FACTURA".into(),
            orden_id: "22222222-2222-4222-8222-222222222222".into(),
            parte_id: None,
            razon_social: Some("Ana".into()),
            ruc: None,
        })
        .await
        .expect("comprobante");
    assert_eq!(primero.estado, "BORRADOR");
    assert_eq!(primero.numero, "F001-00000001");
    assert_eq!(primero.base_gravada_cents, 100_000);
    assert_eq!(primero.iva_cents, 15_000);
    assert_eq!(primero.total_cents, 115_000);
    assert!(primero.emitido_en.is_none());

    let segundo = facturacion
        .emitir(EmitirComprobante {
            tipo: "FACTURA".into(),
            orden_id: "22222222-2222-4222-8222-222222222222".into(),
            parte_id: None,
            razon_social: None,
            ruc: None,
        })
        .await
        .expect("segundo");
    assert_eq!(segundo.numero, "F001-00000002");

    let anulado = facturacion
        .anular(&primero.id)
        .await
        .expect("anular")
        .expect("comprobante");
    assert_eq!(anulado.estado, "ANULADO");

    let series = facturacion.series().await.expect("series");
    assert_eq!(series[0].secuencial_actual, 2);

    let inexistente = facturacion
        .emitir(EmitirComprobante {
            tipo: "FACTURA".into(),
            orden_id: "99999999-9999-4999-8999-999999999999".into(),
            parte_id: None,
            razon_social: None,
            ruc: None,
        })
        .await
        .expect_err("orden inexistente");
    assert_eq!(inexistente.codigo, "NO_ENCONTRADO");
}

#[tokio::test]
async fn debe_procesar_eventos_financieros_una_sola_vez() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let contabilidad = Arc::new(Contabilidad::nueva(
        Arc::clone(&db),
        Outbox::nueva("finance.outbox"),
    ));
    let consumidor = Consumidor::nuevo(
        Arc::clone(&db),
        Arc::clone(&contabilidad),
        Arc::new(OrdenesFijas),
        0.12,
    );
    let evento = json!({
        "event_id": "66666666-6666-4666-8666-666666666666",
        "tipo": "payment.procesado",
        "ocurrido_en": "2026-11-13T10:00:00.000Z",
        "data": {
            "order_id": "22222222-2222-4222-8222-222222222222",
            "monto_cents": 115000,
            "estado": "procesado",
            "metodo": "simulado"
        }
    });
    consumidor.manejar(&evento).await.expect("evento");
    consumidor.manejar(&evento).await.expect("idempotente");

    let asientos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM finance.asientos")
        .fetch_one(db.pool())
        .await
        .expect("asientos");
    assert_eq!(asientos, 1);
    let gmv: i32 =
        sqlx::query_scalar("SELECT monto_cents FROM finance.metricas_mensuales WHERE tipo = 'gmv'")
            .fetch_one(db.pool())
            .await
            .expect("gmv");
    assert_eq!(gmv, 115_000);
    let pedidos: i32 = sqlx::query_scalar(
        "SELECT cantidad FROM finance.metricas_mensuales WHERE tipo = 'pedidos'",
    )
    .fetch_one(db.pool())
    .await
    .expect("pedidos");
    assert_eq!(pedidos, 1);

    let comision = json!({
        "event_id": "77777777-7777-4777-8777-777777777777",
        "tipo": "comision.acreditada",
        "ocurrido_en": "2026-11-13T10:05:00.000Z",
        "data": {
            "order_id": "22222222-2222-4222-8222-222222222222",
            "monto_cents": 13800,
            "monto_vendedor_cents": 100000,
            "monto_empresario_cents": 1150,
            "vendedor_id": "11111111-1111-4111-8111-111111111111",
            "empresa_id": "44444444-4444-4444-8444-444444444444",
            "tienda_id": "88888888-8888-4888-8888-888888888888",
            "participacion_porcentaje": 10
        }
    });
    consumidor.manejar(&comision).await.expect("comision");
    let total_asientos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM finance.asientos")
        .fetch_one(db.pool())
        .await
        .expect("asientos");
    assert_eq!(total_asientos, 2);
    let ventas: i32 = sqlx::query_scalar("SELECT ventas FROM finance.metricas_vendedores LIMIT 1")
        .fetch_one(db.pool())
        .await
        .expect("vendedor");
    assert_eq!(ventas, 1);
}

#[tokio::test]
async fn debe_guardar_y_releer_proyecciones() {
    let Some(url) = url() else { return };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let proyecciones = Proyecciones::nueva(Arc::clone(&db));
    let creada = proyecciones
        .crear("Plan base", SupuestosProyeccion::supuestos_informe())
        .await
        .expect("crear");
    assert_eq!(creada.filas.len(), 12);
    let listado = proyecciones.listar().await.expect("listar");
    assert_eq!(listado.len(), 1);
    assert_eq!(listado[0].nombre, "Plan base");
    let obtenida = proyecciones
        .obtener(&creada.id)
        .await
        .expect("obtener")
        .expect("proyeccion");
    assert_eq!(obtenida.totales.gmv_cents, creada.totales.gmv_cents);
    assert_eq!(obtenida.filas.len(), 12);
}
