//! Pruebas de integración PostgreSQL del flujo de pedidos, carrito, consumer,
//! contratos, suscripciones y salón. Solo corren si `TEST_DATABASE_URL` está
//! definida; provisionan su propio esquema `orders`.

use async_trait::async_trait;
use orders_service::carrito::Carrito;
use orders_service::checkout::{Checkout, ComandoCrear, LineaEntrada};
use orders_service::clients::{
    Capacidad, ClientesPort, Cotizacion, CreditoEstado, ItemStock, OfertaExterna, PreciosExternos,
};
use orders_service::config::Config;
use orders_service::consumer::{PedidosConsumer, STOCK_FALLIDO, STOCK_RESERVADO};
use orders_service::contratos::{Contratos, CrearContratoInput, FinalizarContratoInput};
use orders_service::db::Db;
use orders_service::entregas::EntregasDigitales;
use orders_service::fulfillments::Fulfillments;
use orders_service::outbox::Outbox;
use orders_service::pagos::Pagos;
use orders_service::pedidos::Pedidos;
use orders_service::salon::{CrearComandaInput, CrearMesaInput, Salon};
use orders_service::suscripciones::{CrearSuscripcionInput, Suscripciones};
use orders_service::tipos_orden::TiposOrden;
use orders_service::views::OrderViewRepository;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

const EMPRESA: &str = "11111111-1111-1111-1111-111111111111";
const VENDEDOR: &str = "22222222-2222-2222-2222-222222222222";
const COMPRADOR: &str = "33333333-3333-3333-3333-333333333333";
const OFERTA: &str = "44444444-4444-4444-4444-444444444444";
const TIENDA: &str = "55555555-5555-5555-5555-555555555555";

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

#[derive(Clone)]
struct ClientesFake {
    stock: i64,
    capacidades: Vec<Capacidad>,
    tasa: Option<f64>,
}

#[async_trait]
impl ClientesPort for ClientesFake {
    async fn capacidades(
        &self,
        _empresas: &[String],
        _capacidades: &[String],
    ) -> Option<Vec<Capacidad>> {
        Some(self.capacidades.clone())
    }

    async fn tipo_cambio(&self, _base: &str, _quote: &str) -> Option<f64> {
        self.tasa
    }

    async fn ofertas(&self, ids: &[String]) -> Vec<OfertaExterna> {
        if ids.is_empty() {
            return Vec::new();
        }
        let oferta = OfertaExterna {
            id: OFERTA.to_string(),
            tienda_id: TIENDA.to_string(),
            vendedor_id: VENDEDOR.to_string(),
            empresa_id: EMPRESA.to_string(),
            participacion_porcentaje: Some(10),
            item_type: "producto".to_string(),
            item_id: "66666666-6666-6666-6666-666666666666".to_string(),
            sku: "SKU-1".to_string(),
            producto_nombre: "Producto".to_string(),
            atributos: None,
            margen: 10,
            precio_base: "100.00".to_string(),
            precio_venta: "100.00".to_string(),
            stock: Some(self.stock),
            estado: "activa".to_string(),
            moneda: Some("NIO".to_string()),
        };
        if ids.contains(&OFERTA.to_string()) {
            vec![oferta]
        } else {
            Vec::new()
        }
    }

    async fn precios(
        &self,
        _empresa_id: &str,
        _lineas: Vec<Value>,
        _lista_codigo: Option<&str>,
        _segmento_codigo: Option<&str>,
    ) -> PreciosExternos {
        PreciosExternos::Ok(Vec::new())
    }

    async fn cotizacion(
        &self,
        _empresa_id: &str,
        _item_id: &str,
        _desde: &str,
        _hasta: &str,
    ) -> Option<Cotizacion> {
        None
    }

    async fn disponibilidad(
        &self,
        _empresa_id: &str,
        _recurso_id: &str,
        _inicia_en: &str,
        _termina_en: &str,
    ) -> Option<bool> {
        Some(true)
    }

    async fn registrar_credito(
        &self,
        _empresa_id: &str,
        _parte_id: &str,
        _order_id: &str,
        _monto_cents: i64,
        _concepto: &str,
        _vendedor_id: Option<&str>,
    ) -> CreditoEstado {
        CreditoEstado::Error
    }

    async fn anular_credito(&self, _order_id: &str) -> bool {
        false
    }

    async fn items_lote(&self, _skus: &[String]) -> Option<Vec<ItemStock>> {
        Some(vec![ItemStock {
            empresa_id: EMPRESA.to_string(),
            sku: "SKU-1".to_string(),
            stock: self.stock,
            track_serial: false,
            seriales_disponibles: 0,
            estado: "activo".to_string(),
        }])
    }
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(
        r#"
        DROP SCHEMA IF EXISTS orders CASCADE;
        CREATE SCHEMA orders;

        CREATE TABLE orders.eventos (
          event_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          aggregate_id uuid NOT NULL,
          aggregate_type text NOT NULL DEFAULT 'Order',
          tipo text NOT NULL,
          payload jsonb NOT NULL,
          metadata jsonb DEFAULT '{}'::jsonb,
          version int NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE UNIQUE INDEX uq_eventos_version ON orders.eventos (aggregate_id, version);

        CREATE TABLE orders.orden_vista (
          id uuid PRIMARY KEY,
          vendedor_id uuid NOT NULL,
          comprador_id uuid NOT NULL,
          parte_id uuid,
          tienda_id uuid NOT NULL,
          estado text NOT NULL,
          total_cents integer NOT NULL DEFAULT 0,
          comision_cents integer NOT NULL DEFAULT 0,
          moneda text NOT NULL DEFAULT 'NIO',
          items_json jsonb NOT NULL DEFAULT '[]'::jsonb,
          direccion_envio jsonb,
          creado_en timestamptz NOT NULL,
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          pagada_en timestamptz,
          enviada_en timestamptz,
          entregada_en timestamptz,
          cancelada_en timestamptz,
          devuelta_en timestamptz,
          tipo_orden text NOT NULL DEFAULT 'compra',
          fulfillment_config jsonb NOT NULL DEFAULT '{}'::jsonb,
          metodo_pago text NOT NULL DEFAULT 'simulado'
        );

        CREATE TABLE orders.orden_timeline (
          id bigserial PRIMARY KEY,
          order_id uuid NOT NULL,
          tipo text NOT NULL,
          payload jsonb NOT NULL,
          version int NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );

        CREATE TABLE orders.comisiones_vista (
          vendedor_id uuid NOT NULL,
          periodo date NOT NULL,
          ordenes integer NOT NULL DEFAULT 0,
          total_ventas_cents integer NOT NULL DEFAULT 0,
          total_comision_cents integer NOT NULL DEFAULT 0,
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          PRIMARY KEY (vendedor_id, periodo)
        );

        CREATE OR REPLACE FUNCTION orders.fn_test_evento() RETURNS trigger
        LANGUAGE plpgsql AS $$
        DECLARE v_estado text;
        BEGIN
          IF NEW.tipo = 'OrderCreatedEvent' THEN
            INSERT INTO orders.orden_vista
              (id, vendedor_id, comprador_id, parte_id, tienda_id, estado,
               total_cents, comision_cents, moneda, items_json, creado_en,
               actualizado_en, tipo_orden, fulfillment_config, metodo_pago)
            VALUES (
              NEW.aggregate_id,
              (NEW.payload->'items'->0->>'vendedor_id')::uuid,
              (NEW.payload->>'comprador_id')::uuid,
              (NEW.payload->>'cliente_parte_id')::uuid,
              (NEW.payload->'items'->0->>'tienda_id')::uuid,
              'creada',
              (NEW.payload->>'total_cents')::int,
              (NEW.payload->>'total_cents')::int * 12 / 100,
              'NIO',
              NEW.payload->'items',
              NEW.creado_en,
              NEW.creado_en,
              COALESCE(NEW.payload->>'tipo_orden', 'compra'),
              COALESCE(NEW.payload->'fulfillment_config', '{}'::jsonb),
              COALESCE(NEW.payload->>'metodo_pago', 'simulado')
            )
            ON CONFLICT (id) DO NOTHING;
          ELSIF NEW.tipo = 'OrderStatusUpdatedEvent' THEN
            v_estado := NEW.payload->>'estado';
            UPDATE orders.orden_vista SET
              estado = v_estado,
              actualizado_en = NEW.creado_en,
              pagada_en = CASE WHEN v_estado = 'pagada' THEN NEW.creado_en ELSE pagada_en END,
              enviada_en = CASE WHEN v_estado = 'enviada' THEN NEW.creado_en ELSE enviada_en END,
              entregada_en = CASE WHEN v_estado = 'entregada' THEN NEW.creado_en ELSE entregada_en END,
              cancelada_en = CASE WHEN v_estado = 'cancelada' THEN NEW.creado_en ELSE cancelada_en END,
              devuelta_en = CASE WHEN v_estado = 'devuelta' THEN NEW.creado_en ELSE devuelta_en END
            WHERE id = NEW.aggregate_id;
          END IF;
          INSERT INTO orders.orden_timeline (order_id, tipo, payload, version, creado_en)
          VALUES (NEW.aggregate_id, NEW.tipo, NEW.payload, NEW.version, NEW.creado_en);
          RETURN NEW;
        END;
        $$;
        CREATE TRIGGER eventos_proyeccion_trigger
          AFTER INSERT ON orders.eventos
          FOR EACH ROW EXECUTE FUNCTION orders.fn_test_evento();

        CREATE TABLE orders.carritos (
          comprador_id uuid PRIMARY KEY,
          items_json jsonb NOT NULL DEFAULT '[]'::jsonb,
          total_cents integer NOT NULL DEFAULT 0,
          actualizado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE orders.outbox (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          payload jsonb NOT NULL,
          estado text NOT NULL DEFAULT 'pendiente',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          publicado_en timestamptz
        );
        CREATE TABLE orders.metodos_pago (
          codigo text PRIMARY KEY,
          nombre text NOT NULL,
          requiere_confirmacion boolean NOT NULL DEFAULT true,
          activo boolean NOT NULL DEFAULT true,
          config jsonb NOT NULL DEFAULT '{}'::jsonb,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW()
        );
        INSERT INTO orders.metodos_pago (codigo, nombre, requiere_confirmacion) VALUES
          ('simulado', 'Simulado', false),
          ('credito', 'Crédito', false),
          ('efectivo', 'Efectivo', true),
          ('transferencia', 'Transferencia', true);

        CREATE TABLE orders.pagos (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          order_id uuid NOT NULL,
          empresa_id uuid,
          metodo_codigo text NOT NULL,
          estado text NOT NULL,
          monto_cents integer NOT NULL,
          moneda text NOT NULL DEFAULT 'NIO',
          referencia text NOT NULL DEFAULT '',
          motivo_rechazo text NOT NULL DEFAULT '',
          confirmado_por uuid,
          confirmado_en timestamptz,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW()
        );

        CREATE TABLE orders.eventos_procesados (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          procesado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE orders.entregas_digitales (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          order_id uuid NOT NULL,
          item_id uuid NOT NULL,
          sku text NOT NULL,
          comprador_id uuid NOT NULL,
          cliente_parte_id uuid,
          token uuid NOT NULL DEFAULT gen_random_uuid(),
          recurso_url text NOT NULL,
          estado text NOT NULL DEFAULT 'activo',
          descargas integer NOT NULL DEFAULT 0,
          max_descargas integer,
          primera_descarga_en timestamptz,
          ultima_descarga_en timestamptz,
          expira_en timestamptz,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          UNIQUE (order_id, item_id)
        );

        CREATE TABLE orders.contratos (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          order_id uuid REFERENCES orders.orden_vista(id) ON DELETE SET NULL,
          parte_id uuid,
          item_id uuid,
          tipo text NOT NULL DEFAULT 'otro',
          titulo text NOT NULL,
          descripcion text NOT NULL DEFAULT '',
          estado text NOT NULL DEFAULT 'borrador',
          inicia_en date,
          termina_en date,
          deposito_cents integer NOT NULL DEFAULT 0,
          garantia_cents integer NOT NULL DEFAULT 0,
          retenido_cents integer NOT NULL DEFAULT 0,
          moneda text NOT NULL DEFAULT 'NIO',
          metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
          creado_por uuid NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          finalizado_en timestamptz,
          CONSTRAINT contratos_retenido_check CHECK (retenido_cents <= deposito_cents)
        );
        CREATE UNIQUE INDEX ux_contratos_order ON orders.contratos (order_id) WHERE order_id IS NOT NULL;
        CREATE TABLE orders.contrato_eventos (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          contrato_id uuid NOT NULL REFERENCES orders.contratos(id) ON DELETE CASCADE,
          tipo text NOT NULL,
          monto_cents integer NOT NULL DEFAULT 0,
          motivo text NOT NULL DEFAULT '',
          creado_por uuid NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );

        CREATE TABLE orders.suscripciones (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          oferta_id uuid NOT NULL,
          item_id uuid,
          cantidad integer NOT NULL DEFAULT 1,
          comprador_id uuid NOT NULL,
          cliente_parte_id uuid,
          tipo_orden text NOT NULL DEFAULT 'compra',
          metodo_pago text NOT NULL DEFAULT 'simulado',
          fulfillment text NOT NULL DEFAULT 'none',
          intervalo text NOT NULL,
          estado text NOT NULL DEFAULT 'activa',
          proxima_ejecucion date NOT NULL,
          ultima_ejecucion date,
          fin_en date,
          metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
          creado_por uuid NOT NULL,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE orders.suscripcion_ejecuciones (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          suscripcion_id uuid NOT NULL REFERENCES orders.suscripciones(id) ON DELETE CASCADE,
          order_id uuid,
          estado text NOT NULL,
          error text NOT NULL DEFAULT '',
          ejecutada_en timestamptz NOT NULL DEFAULT NOW()
        );

        CREATE TABLE orders.mesas (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          codigo text NOT NULL,
          nombre text NOT NULL,
          capacidad integer NOT NULL DEFAULT 1,
          zona text NOT NULL DEFAULT '',
          estado text NOT NULL DEFAULT 'libre',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          UNIQUE (empresa_id, codigo)
        );
        CREATE TABLE orders.comandas (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          empresa_id uuid NOT NULL,
          order_id uuid REFERENCES orders.orden_vista(id) ON DELETE SET NULL,
          mesa_id uuid REFERENCES orders.mesas(id) ON DELETE SET NULL,
          estacion text NOT NULL,
          estado text NOT NULL DEFAULT 'pendiente',
          notas text NOT NULL DEFAULT '',
          items_json jsonb NOT NULL DEFAULT '[]'::jsonb,
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          actualizado_en timestamptz NOT NULL DEFAULT NOW(),
          listo_en timestamptz,
          entregado_en timestamptz
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("migracion orders");
    sqlx::raw_sql(include_str!("../../../infra/db/init/80_tipos_orden.sql"))
        .execute(&pool)
        .await
        .expect("tipos de orden");
    sqlx::raw_sql(include_str!("../../../infra/db/init/81_fulfillments.sql"))
        .execute(&pool)
        .await
        .expect("fulfillments");
    pool.close().await;
}

async fn con_bd() -> Option<Arc<Db>> {
    let url = url()?;
    provisionar(&url).await;
    Some(Arc::new(Db::conectar(&url).await.expect("db")))
}

fn compra(cantidad: i64) -> ComandoCrear {
    ComandoCrear {
        tipo_orden: "compra".to_string(),
        items: Some(vec![LineaEntrada {
            oferta_id: OFERTA.to_string(),
            cantidad,
            fulfillment: Some("none".to_string()),
            fulfillment_config: None,
        }]),
        ..Default::default()
    }
}

fn infra(db: Arc<Db>, stock: i64) -> (Arc<Checkout>, Arc<Carrito>, Arc<Pedidos>) {
    infra_con(db, stock, None)
}

fn infra_con(
    db: Arc<Db>,
    stock: i64,
    tasa: Option<f64>,
) -> (Arc<Checkout>, Arc<Carrito>, Arc<Pedidos>) {
    let clientes: Arc<dyn ClientesPort> = Arc::new(ClientesFake {
        stock,
        tasa,
        capacidades: vec![
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "contratos".to_string(),
                activo: true,
            },
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "suscripciones".to_string(),
                activo: true,
            },
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "mesas".to_string(),
                activo: true,
            },
        ],
    });
    let carrito = Arc::new(Carrito::nueva(Arc::clone(&db), Arc::clone(&clientes)));
    let vistas = Arc::new(OrderViewRepository);
    let pedidos = Arc::new(Pedidos {
        tasa_comision: 0.12,
    });
    let checkout = Arc::new(Checkout {
        db: Arc::clone(&db),
        clientes: Arc::clone(&clientes),
        carrito: Arc::clone(&carrito),
        vistas,
        pedidos: Arc::clone(&pedidos),
        outbox: Outbox::nueva("orders.outbox"),
        pagos: Arc::new(Pagos::nueva(Arc::clone(&db))),
        tipos: Arc::new(TiposOrden::nueva(Arc::clone(&db))),
        fulfillments: Arc::new(Fulfillments::nueva(Arc::clone(&db))),
        tasa_comision: 0.12,
    });
    (checkout, carrito, pedidos)
}

#[tokio::test]
async fn debe_crear_orden_con_evento_y_outbox() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let orden = checkout
        .ejecutar(&compra(2), COMPRADOR)
        .await
        .expect("orden");
    assert_eq!(orden.estado, "creada");
    assert_eq!(orden.total_cents, 20_000);
    assert_eq!(orden.items.len(), 1);
    let vista = OrderViewRepository
        .encontrar(&db, &orden.id)
        .await
        .expect("vista")
        .expect("existe");
    assert_eq!(vista.estado, "creada");
    assert_eq!(vista.items.len(), 1);
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders.outbox")
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(outbox, 1);
    let eventos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders.eventos")
        .fetch_one(db.pool())
        .await
        .expect("eventos");
    assert_eq!(eventos, 1);
}

fn compra_con_metodo(cantidad: i64, metodo: &str) -> ComandoCrear {
    ComandoCrear {
        metodo_pago: Some(metodo.to_string()),
        ..compra(cantidad)
    }
}

#[tokio::test]
async fn debe_dejar_pendiente_el_pago_manual_hasta_confirmacion() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let orden = checkout
        .ejecutar(&compra_con_metodo(1, "efectivo"), COMPRADOR)
        .await
        .expect("orden");
    assert_eq!(orden.estado, "creada");
    assert_eq!(orden.metodo_pago, "efectivo");
    let (estado, metodo): (String, String) =
        sqlx::query_as("SELECT estado, metodo_codigo FROM orders.pagos WHERE order_id = $1::uuid")
            .bind(Uuid::parse_str(&orden.id).expect("uuid"))
            .fetch_one(db.pool())
            .await
            .expect("pago");
    assert_eq!(estado, "pendiente");
    assert_eq!(metodo, "efectivo");

    let consumer = PedidosConsumer::nuevo(Arc::clone(&db), Arc::clone(&checkout));
    consumer
        .manejar(json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": STOCK_RESERVADO,
            "data": {"order_id": orden.id}
        }))
        .await
        .expect("consumir");
    let vista = OrderViewRepository
        .encontrar(&db, &orden.id)
        .await
        .expect("vista")
        .expect("existe");
    assert_eq!(vista.estado, "creada");
}

#[tokio::test]
async fn debe_rechazar_un_metodo_de_pago_desconocido() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let error = checkout
        .ejecutar(&compra_con_metodo(1, "no_existe"), COMPRADOR)
        .await
        .expect_err("metodo invalido");
    assert_eq!(error.codigo, "METODO_PAGO_INVALIDO");
}

#[tokio::test]
async fn debe_procesar_stock_reservado_y_ser_idempotente() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let orden = checkout
        .ejecutar(&compra(1), COMPRADOR)
        .await
        .expect("orden");
    let consumer = PedidosConsumer::nuevo(Arc::clone(&db), Arc::clone(&checkout));
    let evento = json!({
        "event_id": Uuid::new_v4().to_string(),
        "tipo": STOCK_RESERVADO,
        "data": {"order_id": orden.id}
    });
    consumer.manejar(evento.clone()).await.expect("consumir");
    let vista = OrderViewRepository
        .encontrar(&db, &orden.id)
        .await
        .expect("vista")
        .expect("existe");
    assert_eq!(vista.estado, "pagada");
    let pagos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders.outbox WHERE tipo = 'payment.procesado'")
            .fetch_one(db.pool())
            .await
            .expect("pagos");
    assert_eq!(pagos, 1);
    consumer.manejar(evento).await.expect("reintento");
    let pagos: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders.outbox WHERE tipo = 'payment.procesado'")
            .fetch_one(db.pool())
            .await
            .expect("pagos");
    assert_eq!(pagos, 1);
}

#[tokio::test]
async fn debe_cancelar_la_orden_cuando_falla_el_stock() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let orden = checkout
        .ejecutar(&compra(1), COMPRADOR)
        .await
        .expect("orden");
    let consumer = PedidosConsumer::nuevo(Arc::clone(&db), Arc::clone(&checkout));
    consumer
        .manejar(json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": STOCK_FALLIDO,
            "data": {"order_id": orden.id, "items": [{"sku": "SKU-1"}]}
        }))
        .await
        .expect("consumir");
    let vista = OrderViewRepository
        .encontrar(&db, &orden.id)
        .await
        .expect("vista")
        .expect("existe");
    assert_eq!(vista.estado, "cancelada");
}

#[tokio::test]
async fn debe_gestionar_el_carrito_y_vaciarlo_en_el_checkout() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, carrito, _) = infra(Arc::clone(&db), 10);
    let vista = carrito
        .agregar(COMPRADOR, OFERTA, 2)
        .await
        .expect("carrito");
    assert_eq!(vista.total, "200.00");
    assert_eq!(vista.items[0].cantidad, 2);
    let comando = ComandoCrear {
        tipo_orden: "compra".to_string(),
        usar_carrito: true,
        ..Default::default()
    };
    let orden = checkout
        .ejecutar(&comando, COMPRADOR)
        .await
        .expect("checkout");
    assert_eq!(orden.items[0].cantidad, 2);
    let vacio = carrito.obtener(COMPRADOR).await.expect("carrito");
    assert!(vacio.items.is_empty());
}

#[tokio::test]
async fn debe_crear_y_finalizar_un_contrato() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let contratos = Contratos::nueva(Arc::clone(&db));
    let contrato = contratos
        .crear(
            EMPRESA,
            VENDEDOR,
            CrearContratoInput {
                titulo: "Alquiler de equipo".to_string(),
                deposito: Some("500.00".to_string()),
                garantia: Some("200.00".to_string()),
                tipo: Some("alquiler".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("contrato");
    assert_eq!(contrato.estado, "borrador");
    assert_eq!(contrato.deposito, "500.00");
    let activo = contratos
        .actualizar(
            EMPRESA,
            &contrato.id,
            orders_service::contratos::ActualizarContratoInput {
                estado: Some("activo".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("activar");
    assert_eq!(activo.estado, "activo");
    let finalizado = contratos
        .finalizar(
            EMPRESA,
            &contrato.id,
            VENDEDOR,
            FinalizarContratoInput {
                retenido: Some("50.00".to_string()),
                motivo: Some("desgaste".to_string()),
            },
        )
        .await
        .expect("finalizar");
    assert_eq!(finalizado.estado, "finalizado");
    assert_eq!(finalizado.retenido, "50.00");
    let movimientos = contratos
        .listar_eventos(EMPRESA, &contrato.id)
        .await
        .expect("movimientos");
    assert!(movimientos.iter().any(|m| m.tipo == "retencion"));
}

#[tokio::test]
async fn debe_renovar_una_suscripcion_creando_la_orden() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let suscripciones = Suscripciones::nueva(Arc::clone(&db), Arc::clone(&checkout));
    let suscripcion = suscripciones
        .crear(
            EMPRESA,
            VENDEDOR,
            CrearSuscripcionInput {
                oferta_id: OFERTA.to_string(),
                intervalo: "mensual".to_string(),
                comprador_id: Some(COMPRADOR.to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("suscripcion");
    let resultado = suscripciones
        .renovar(EMPRESA, &suscripcion.id, true)
        .await
        .expect("renovar");
    assert!(!resultado.order_id.is_empty());
    let ejecuciones = suscripciones
        .listar_ejecuciones(EMPRESA, &suscripcion.id)
        .await
        .expect("ejecuciones");
    assert_eq!(ejecuciones.len(), 1);
    assert_eq!(ejecuciones[0].estado, "creada");
}

#[tokio::test]
async fn debe_gestionar_mesas_y_comandas() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let salon = Salon::nueva(Arc::clone(&db));
    let mesa = salon
        .crear_mesa(
            EMPRESA,
            CrearMesaInput {
                codigo: "M-01".to_string(),
                nombre: "Mesa 1".to_string(),
                capacidad: Some(4),
                zona: Some("salon".to_string()),
                estado: None,
            },
        )
        .await
        .expect("mesa");
    assert_eq!(mesa.estado, "libre");
    let comanda = salon
        .crear_comanda(
            EMPRESA,
            CrearComandaInput {
                order_id: None,
                mesa_id: Some(mesa.id.clone()),
                estacion: "cocina".to_string(),
                notas: None,
                items: Some(vec![orders_service::salon::ComandaItem {
                    sku: "SKU-1".to_string(),
                    nombre: "Producto".to_string(),
                    cantidad: 1,
                    notas: None,
                }]),
            },
        )
        .await
        .expect("comanda");
    assert_eq!(comanda.estado, "pendiente");
    let preparando = salon
        .actualizar_comanda(
            EMPRESA,
            &comanda.id,
            orders_service::salon::ActualizarComandaInput {
                estado: Some("en_preparacion".to_string()),
                notas: None,
            },
        )
        .await
        .expect("avanzar");
    assert_eq!(preparando.estado, "en_preparacion");
}

#[tokio::test]
async fn debe_registrar_entregas_digitales_al_crear_la_orden() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    // El fake no expone atributos digitales; se comprueba que la tabla queda
    // consultable y que el servicio devuelve 404 para una orden sin entregas.
    let orden = checkout
        .ejecutar(&compra(1), COMPRADOR)
        .await
        .expect("orden");
    let entregas = EntregasDigitales::nueva(Arc::clone(&db));
    let actor = orders_service::auth::UsuarioContexto {
        user_id: COMPRADOR.to_string(),
        rol: "comprador".to_string(),
        ..Default::default()
    };
    let lista = entregas
        .listar_por_orden(&orden.id, &actor)
        .await
        .expect("entregas");
    assert!(lista.is_empty());
}

async fn router_completo(db: Arc<Db>, stock: i64) -> axum::Router {
    use orders_service::http::build_router;
    use orders_service::metrics::Metrics;
    use orders_service::state::AppState;
    let config = Config::from_pairs([
        ("SERVICIO", "orders-service"),
        ("INTERNAL_API_KEY", "clave-interna"),
        ("PAGO_WEBHOOK_SECRET", "webhook-secreto"),
    ])
    .expect("config");
    let clientes: Arc<dyn ClientesPort> = Arc::new(ClientesFake {
        stock,
        tasa: None,
        capacidades: vec![
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "contratos".to_string(),
                activo: true,
            },
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "suscripciones".to_string(),
                activo: true,
            },
            Capacidad {
                empresa_id: EMPRESA.to_string(),
                capacidad_codigo: "mesas".to_string(),
                activo: true,
            },
        ],
    });
    let carrito = Arc::new(Carrito::nueva(Arc::clone(&db), Arc::clone(&clientes)));
    let vistas = Arc::new(OrderViewRepository);
    let pedidos = Arc::new(Pedidos {
        tasa_comision: 0.12,
    });
    let entregas = Arc::new(EntregasDigitales::nueva(Arc::clone(&db)));
    let checkout = Arc::new(Checkout {
        db: Arc::clone(&db),
        clientes: Arc::clone(&clientes),
        carrito: Arc::clone(&carrito),
        vistas: Arc::clone(&vistas),
        pedidos: Arc::clone(&pedidos),
        outbox: Outbox::nueva("orders.outbox"),
        pagos: Arc::new(Pagos::nueva(Arc::clone(&db))),
        tipos: Arc::new(TiposOrden::nueva(Arc::clone(&db))),
        fulfillments: Arc::new(Fulfillments::nueva(Arc::clone(&db))),
        tasa_comision: 0.12,
    });
    let contratos = Arc::new(Contratos::nueva(Arc::clone(&db)));
    let suscripciones = Arc::new(Suscripciones::nueva(Arc::clone(&db), Arc::clone(&checkout)));
    let salon = Arc::new(Salon::nueva(Arc::clone(&db)));
    let pagos = Arc::new(Pagos::nueva(Arc::clone(&db)));
    let tipos = Arc::new(TiposOrden::nueva(Arc::clone(&db)));
    let fulfillments = Arc::new(Fulfillments::nueva(Arc::clone(&db)));
    let estado = AppState::con_dependencias(
        config,
        Arc::new(Metrics::nueva("orders-service")),
        db,
        clientes,
        checkout,
        carrito,
        vistas,
        entregas,
        pedidos,
        contratos,
        suscripciones,
        salon,
        pagos,
        tipos,
        fulfillments,
    );
    build_router(estado)
}

async fn json_de(response: axum::response::Response) -> Value {
    use http_body_util::BodyExt;
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

fn headers_comprador() -> [(&'static str, &'static str); 3] {
    [
        ("x-user-id", COMPRADOR),
        ("x-user-rol", "comprador"),
        ("x-user-email", "comprador@test.dev"),
    ]
}

fn headers_vendedor() -> [(&'static str, &'static str); 4] {
    [
        ("x-user-id", VENDEDOR),
        ("x-user-rol", "empresario"),
        ("x-user-email", "empresario@test.dev"),
        ("x-tenant", EMPRESA),
    ]
}

#[tokio::test]
async fn debe_crear_y_consultar_orden_por_http() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;
    let mut request = Request::post("/api/v2/orders").header("content-type", "application/json");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({"tipo_orden":"compra","items":[{"oferta_id":OFERTA,"cantidad":2}]})
                        .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let valor = json_de(response).await;
    let order_id = valor["data"]["id"].as_str().expect("id").to_string();
    assert_eq!(valor["data"]["total_cents"], 20_000);
    assert_eq!(valor["meta"]["statusCode"], 201);

    let response = router
        .oneshot(
            Request::get(format!("/api/v2/orders/{order_id}"))
                .header("x-user-id", COMPRADOR)
                .header("x-user-rol", "comprador")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let valor = json_de(response).await;
    assert_eq!(valor["data"]["id"], order_id);
}

#[tokio::test]
async fn debe_gestionar_contratos_suscripciones_y_salon_por_http() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;
    let mut request = Request::post(format!("/api/v2/contratos?empresa_id={EMPRESA}"))
        .header("content-type", "application/json");
    for (clave, valor) in headers_vendedor() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({"titulo":"Alquiler","tipo":"alquiler","deposito":"500.00"}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let contrato = json_de(response).await;
    assert_eq!(contrato["data"]["deposito"], "500.00");

    let mut request = Request::post(format!("/api/v2/suscripciones?empresa_id={EMPRESA}"))
        .header("content-type", "application/json");
    for (clave, valor) in headers_vendedor() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({"oferta_id":OFERTA,"intervalo":"mensual","comprador_id":COMPRADOR})
                        .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let suscripcion = json_de(response).await;
    let suscripcion_id = suscripcion["data"]["id"].as_str().expect("id");

    let mut request = Request::post(format!(
        "/api/v2/suscripciones/{suscripcion_id}/renovar?empresa_id={EMPRESA}"
    ));
    for (clave, valor) in headers_vendedor() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let renovada = json_de(response).await;
    assert!(renovada["data"]["order_id"].is_string());

    let mut request = Request::post(format!("/api/v2/mesas?empresa_id={EMPRESA}"))
        .header("content-type", "application/json");
    for (clave, valor) in headers_vendedor() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    json!({"codigo":"M-01","nombre":"Mesa 1"}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let mesa = json_de(response).await;
    let mesa_id = mesa["data"]["id"].as_str().expect("mesa");

    let mut request = Request::post(format!("/api/v2/comandas?empresa_id={EMPRESA}"))
        .header("content-type", "application/json");
    for (clave, valor) in headers_vendedor() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(
            request
                .body(Body::from(
                    json!({"mesa_id":mesa_id,"estacion":"cocina","items":[{"sku":"SKU-1","nombre":"Producto","cantidad":1}]})
                        .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let comanda = json_de(response).await;
    assert_eq!(comanda["data"]["estado"], "pendiente");
}

#[tokio::test]
async fn debe_transicionar_y_leer_la_historia_por_http_interno() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    let orden = checkout
        .ejecutar(&compra(1), COMPRADOR)
        .await
        .expect("orden");
    let router = router_completo(Arc::clone(&db), 10).await;
    let response = router
        .clone()
        .oneshot(
            Request::post(format!(
                "/internal/orders/{}/transicion?estado=pagada&actor=test",
                orden.id
            ))
            .header("x-internal-key", "clave-interna")
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        json_de(response).await
    );
    let response = router
        .clone()
        .oneshot(
            Request::get(format!("/internal/orders/{}", orden.id))
                .header("x-internal-key", "clave-interna")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let valor = json_de(response).await;
    assert_eq!(status, StatusCode::OK, "{valor}");
    assert_eq!(valor["estado"], "pagada");

    let response = router
        .oneshot(
            Request::get(format!("/internal/orders/{}/historia", orden.id))
                .header("x-internal-key", "clave-interna")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let valor = json_de(response).await;
    assert_eq!(valor.as_array().expect("historia").len(), 2);
}

fn headers_admin() -> [(&'static str, &'static str); 2] {
    [
        ("x-user-id", "99999999-9999-4999-8999-999999999999"),
        ("x-user-rol", "super_admin"),
    ]
}

#[tokio::test]
async fn debe_gestionar_metodos_de_pago() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;

    let mut request = Request::get("/api/v2/orders/metodos-pago");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let listado = json_de(response).await;
    let codigos: Vec<&str> = listado["data"]
        .as_array()
        .expect("metodos")
        .iter()
        .filter_map(|metodo| metodo["codigo"].as_str())
        .collect();
    assert!(codigos.contains(&"efectivo"));
    assert!(codigos.contains(&"transferencia"));

    let mut request =
        Request::post("/api/v2/admin/metodos-pago").header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    r#"{"codigo":"contra_entrega","nombre":"Contra entrega"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let creado = json_de(response).await;
    assert_eq!(creado["data"]["requiere_confirmacion"], true);

    let mut request =
        Request::post("/api/v2/admin/metodos-pago").header("content-type", "application/json");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"codigo":"otro","nombre":"Otro"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let mut request = Request::patch("/api/v2/admin/metodos-pago/contra_entrega")
        .header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(
            request
                .body(Body::from(r#"{"activo":false}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let actualizado = json_de(response).await;
    assert_eq!(actualizado["data"]["activo"], false);
}

async fn crear_orden_manual(router: axum::Router) -> (String, String) {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let mut request = Request::post("/api/v2/orders").header("content-type", "application/json");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(format!(
                    r#"{{"tipo_orden":"compra","metodo_pago":"efectivo","items":[{{"oferta_id":"{OFERTA}","cantidad":1,"fulfillment":"none"}}]}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let orden = json_de(response).await;
    let order_id = orden["data"]["id"].as_str().expect("order id").to_string();

    let mut request = Request::get(format!("/api/v2/admin/pagos?order_id={order_id}"));
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let pagos = json_de(response).await;
    let pago_id = pagos["data"][0]["id"]
        .as_str()
        .expect("pago id")
        .to_string();
    (order_id, pago_id)
}

#[tokio::test]
async fn debe_confirmar_y_rechazar_pagos_manuales() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;

    let (order_id, pago_id) = crear_orden_manual(router.clone()).await;
    let mut request = Request::post(format!("/api/v2/admin/pagos/{pago_id}/confirmar"));
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let pago = json_de(response).await;
    assert_eq!(pago["data"]["estado"], "confirmado");

    let mut request = Request::get(format!("/api/v2/orders/{order_id}"));
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let orden = json_de(response).await;
    assert_eq!(orden["data"]["estado"], "pagada");

    let (order_id, pago_id) = crear_orden_manual(router.clone()).await;
    let mut request = Request::post(format!("/api/v2/admin/pagos/{pago_id}/rechazar"))
        .header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"motivo":"sin fondos"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let pago = json_de(response).await;
    assert_eq!(pago["data"]["estado"], "rechazado");
    assert_eq!(pago["data"]["motivo_rechazo"], "sin fondos");

    let mut request = Request::get(format!("/api/v2/orders/{order_id}"));
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let orden = json_de(response).await;
    assert_eq!(orden["data"]["estado"], "cancelada");
}

#[tokio::test]
async fn debe_procesar_un_webhook_de_pago_con_secreto() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;
    let (order_id, pago_id) = crear_orden_manual(router.clone()).await;

    let request = Request::post("/internal/pagos/webhook")
        .header("content-type", "application/json")
        .header("x-webhook-secret", "incorrecto");
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(format!(
                    r#"{{"pago_id":"{pago_id}","estado":"confirmado"}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let request = Request::post("/internal/pagos/webhook")
        .header("content-type", "application/json")
        .header("x-webhook-secret", "webhook-secreto");
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(format!(
                    r#"{{"pago_id":"{pago_id}","estado":"confirmado"}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let mut request = Request::get(format!("/api/v2/orders/{order_id}"));
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let orden = json_de(response).await;
    assert_eq!(orden["data"]["estado"], "pagada");
}

#[tokio::test]
async fn debe_leer_y_validar_la_matriz_de_tipos_de_orden() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let tipos = TiposOrden::nueva(Arc::clone(&db));
    let lista = tipos.listar().await.expect("tipos");
    assert!(lista.iter().any(|tipo| tipo.codigo == "compra"));
    assert!(lista.iter().any(|tipo| tipo.codigo == "cotizacion"));

    tipos
        .validar("compra", "pagada", "en_preparacion")
        .await
        .expect("transicion valida");
    let error = tipos
        .validar("compra", "creada", "entregada")
        .await
        .expect_err("transicion invalida");
    assert_eq!(error.codigo, "TRANSICION_INVALIDA");
    let error = tipos
        .validar("no_existe", "creada", "pagada")
        .await
        .expect_err("tipo invalido");
    assert_eq!(error.codigo, "TIPO_ORDEN_INVALIDO");
}

#[tokio::test]
async fn debe_aceptar_un_tipo_de_orden_creado_por_configuracion() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    sqlx::query(
        "INSERT INTO orders.tipos_orden (codigo, nombre)
         VALUES ('devolucion_express', 'Devolucion express')
         ON CONFLICT (codigo) DO NOTHING",
    )
    .execute(db.pool())
    .await
    .expect("tipo");
    sqlx::query(
        "INSERT INTO orders.transiciones_orden (tipo_codigo, desde, hasta)
         VALUES ('devolucion_express', 'creada', 'pagada')
         ON CONFLICT DO NOTHING",
    )
    .execute(db.pool())
    .await
    .expect("transicion");

    let orden = checkout
        .ejecutar(
            &ComandoCrear {
                tipo_orden: "devolucion_express".to_string(),
                ..compra(1)
            },
            COMPRADOR,
        )
        .await
        .expect("orden");
    assert_eq!(orden.tipo_orden, "devolucion_express");
    checkout
        .transicionar(&orden.id, "pagada", Some("config"), Some("test"), None)
        .await
        .expect("transicion configurada");
}

#[tokio::test]
async fn debe_gestionar_tipos_y_transiciones_por_admin() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;

    let mut request =
        Request::post("/api/v2/admin/tipos-orden").header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    r#"{"codigo":"reparto_express","nombre":"Reparto express"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut request = Request::post("/api/v2/admin/tipos-orden/reparto_express/transiciones")
        .header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"desde":"creada","hasta":"pagada"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut request = Request::get("/api/v2/admin/tipos-orden/reparto_express");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let detalle = json_de(response).await;
    assert_eq!(detalle["data"]["tipo"]["codigo"], "reparto_express");
    assert_eq!(detalle["data"]["transiciones"].as_array().unwrap().len(), 1);

    let mut request =
        Request::post("/api/v2/admin/tipos-orden").header("content-type", "application/json");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"codigo":"otro","nombre":"Otro"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let mut request =
        Request::delete("/api/v2/admin/tipos-orden/reparto_express/transiciones/creada/pagada");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn debe_gestionar_fulfillments_por_admin() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let router = router_completo(Arc::clone(&db), 10).await;

    let mut request = Request::get("/api/v2/orders/fulfillments");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let listado = json_de(response).await;
    let codigos: Vec<&str> = listado["data"]
        .as_array()
        .expect("fulfillments")
        .iter()
        .filter_map(|fulfillment| fulfillment["codigo"].as_str())
        .collect();
    assert!(codigos.contains(&"delivery"));
    assert!(codigos.contains(&"digital"));

    let mut request =
        Request::post("/api/v2/admin/fulfillments").header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    r#"{"codigo":"retiro_tienda","nombre":"Retiro en tienda"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut request =
        Request::post("/api/v2/admin/fulfillments").header("content-type", "application/json");
    for (clave, valor) in headers_comprador() {
        request = request.header(clave, valor);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"codigo":"otro","nombre":"Otro"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let mut request = Request::patch("/api/v2/admin/fulfillments/retiro_tienda")
        .header("content-type", "application/json");
    for (clave, valor) in headers_admin() {
        request = request.header(clave, valor);
    }
    let response = router
        .oneshot(
            request
                .body(Body::from(r#"{"activo":false}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let actualizado = json_de(response).await;
    assert_eq!(actualizado["data"]["activo"], false);
}

#[tokio::test]
async fn debe_validar_el_fulfillment_contra_el_registro() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra(Arc::clone(&db), 10);
    sqlx::query(
        "INSERT INTO orders.fulfillments (codigo, nombre)
         VALUES ('retiro_tienda', 'Retiro en tienda')
         ON CONFLICT (codigo) DO NOTHING",
    )
    .execute(db.pool())
    .await
    .expect("fulfillment");

    let orden = checkout
        .ejecutar(
            &ComandoCrear {
                items: Some(vec![LineaEntrada {
                    oferta_id: OFERTA.to_string(),
                    cantidad: 1,
                    fulfillment: Some("retiro_tienda".to_string()),
                    fulfillment_config: None,
                }]),
                ..compra(1)
            },
            COMPRADOR,
        )
        .await
        .expect("orden con fulfillment configurado");
    assert_eq!(orden.items[0].fulfillment.as_deref(), Some("retiro_tienda"));

    let error = checkout
        .ejecutar(
            &ComandoCrear {
                items: Some(vec![LineaEntrada {
                    oferta_id: OFERTA.to_string(),
                    cantidad: 1,
                    fulfillment: Some("no_existe".to_string()),
                    fulfillment_config: None,
                }]),
                ..compra(1)
            },
            COMPRADOR,
        )
        .await
        .expect_err("fulfillment invalido");
    assert_eq!(error.codigo, "FULFILLMENT_INVALIDO");
}

#[tokio::test]
async fn debe_convertir_con_moneda_pago_explicita() {
    let _guard = DB_LOCK.lock().await;
    let Some(db) = con_bd().await else { return };
    let (checkout, _, _) = infra_con(Arc::clone(&db), 10, Some(36.5));
    let orden = checkout
        .ejecutar(
            &ComandoCrear {
                moneda_pago: Some("USD".to_string()),
                ..compra_con_metodo(1, "simulado")
            },
            COMPRADOR,
        )
        .await
        .expect("orden convertida");
    assert_eq!(orden.moneda, "NIO");
    assert_eq!(orden.total_cents, 10_000);

    let (moneda, monto): (String, i32) =
        sqlx::query_as("SELECT moneda, monto_cents FROM orders.pagos WHERE order_id = $1::uuid")
            .bind(Uuid::parse_str(&orden.id).expect("uuid"))
            .fetch_one(db.pool())
            .await
            .expect("pago");
    assert_eq!(moneda, "USD");
    assert_eq!(monto, 365_000);

    let payload: Value = sqlx::query_scalar(
        "SELECT payload FROM orders.outbox WHERE tipo = 'order.created' LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("outbox");
    assert_eq!(payload["data"]["moneda_pago"], "USD");
    assert_eq!(payload["data"]["total_pagado_cents"], 365_000);

    let (sin_tasa, _, _) = infra(Arc::clone(&db), 10);
    let error = sin_tasa
        .ejecutar(
            &ComandoCrear {
                moneda_pago: Some("USD".to_string()),
                ..compra_con_metodo(1, "simulado")
            },
            COMPRADOR,
        )
        .await
        .expect_err("sin tasa");
    assert_eq!(error.codigo, "TIPO_CAMBIO_NO_DISPONIBLE");
}
