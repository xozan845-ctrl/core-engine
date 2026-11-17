use super::*;

fn fila_producto() -> Value {
    json!({

        "id": "11111111-2222-4333-8444-555555555555",

        "empresa_id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",

        "sku": "SKU-01",

        "nombre": "Teclado",

        "descripcion": "mecanico",

        "categoria": "perifericos",

        "precio_base_cents": 150000,

        "stock": 10,

        "estado": "activo",

        "creado_en": "2026-09-01T00:00:00.000Z",

        "color": null,

    })
}

#[test]

fn debe_normalizar_y_validar_el_sku() {
    assert_eq!(normalizar_sku(" sku-01 ").expect("sku"), "SKU-01");

    assert_eq!(
        normalizar_sku("A").expect_err("corto").codigo,
        "SKU_INVALIDO"
    );

    assert_eq!(
        normalizar_sku("SKU@01").expect_err("caracter").codigo,
        "SKU_INVALIDO"
    );
}

#[test]

fn debe_validar_stock_y_precio() {
    assert_eq!(validar_stock(Some(&json!(10))).expect("stock"), 10);

    assert_eq!(validar_stock(Some(&json!(2.0))).expect("entero"), 2);

    assert_eq!(
        validar_stock(Some(&json!(-1)))
            .expect_err("negativo")
            .codigo,
        "STOCK_INVALIDO"
    );

    assert_eq!(
        validar_stock(Some(&json!(2.5)))
            .expect_err("decimal")
            .codigo,
        "STOCK_INVALIDO"
    );

    assert_eq!(
        validar_precio(Some(&json!("0"))).expect_err("cero").codigo,
        "PRECIO_INVALIDO"
    );

    assert_eq!(
        validar_precio(Some(&json!("1500.00")))
            .expect("precio")
            .centavos,
        150_000
    );
}

#[test]

fn debe_normalizar_metadatos_con_defaults_estables() {
    let metadatos = normalizar_metadatos(&json!({

        "costo": "1000.00",

        "marca": " Core ",

        "codigo_barras": " 750 ",

        "stock_minimo": 2,

        "stock_maximo": 20,

        "garantia_meses": 12,

        "es_materia_prima": true,

        "ubicacion": " A-1 ",

        "estado_operativo": "inactivo",

    }))
    .expect("metadatos");

    assert_eq!(metadatos.costo_cents, 100_000);

    assert_eq!(metadatos.marca, "Core");

    assert_eq!(metadatos.codigo_barras, "750");

    assert_eq!(metadatos.unidad, "Unidad (u)");

    assert_eq!(metadatos.unidad_compra, "Igual que la de venta");

    assert_eq!(metadatos.contenido_unidad, "Sin contenido");

    assert_eq!(metadatos.tipo_precio, "fijo");

    assert_eq!(metadatos.tipo_costo, "fijo");

    assert_eq!(metadatos.ubicacion, "A-1");

    assert_eq!(metadatos.estado_operativo, "inactivo");
}

#[test]

fn debe_rechazar_metadatos_invalidos() {
    assert_eq!(
        normalizar_metadatos(&json!({"contenido_cantidad": -1}))
            .expect_err("negativo")
            .codigo,
        "METADATO_INVALIDO"
    );

    assert_eq!(
        normalizar_metadatos(&json!({"stock_minimo": 30, "stock_maximo": 20}))
            .expect_err("rango")
            .mensaje,
        "stock_maximo no puede ser menor que stock_minimo."
    );

    assert_eq!(
        normalizar_metadatos(&json!({"garantia_meses": 1.5}))
            .expect_err("decimal")
            .codigo,
        "METADATO_INVALIDO"
    );
}

#[test]

fn debe_combinar_metadatos_respetando_null_explicito() {
    let previo = serializar(&fila_producto(), &[], Vec::new()).expect("producto");

    let combinado = combinar_metadatos(
        &previo,
        &json!({"marca": "Nueva", "stock_maximo": null, "estado_operativo": "inactivo"}),
    )
    .expect("combinado");

    assert_eq!(combinado.marca, "Nueva");

    assert_eq!(combinado.stock_maximo, None);

    assert_eq!(combinado.estado_operativo, "inactivo");

    assert_eq!(combinado.unidad, "Unidad (u)");
}

#[test]

fn debe_calcular_el_siguiente_orden_de_imagen() {
    assert_eq!(siguiente_orden(&HashSet::new()), 0);

    assert_eq!(siguiente_orden(&HashSet::from([0, 1])), 2);

    assert_eq!(siguiente_orden(&HashSet::from([0, 1, 2])), -1);
}

#[test]

fn debe_validar_uuids_canonicos() {
    assert!(es_uuid("11111111-2222-4333-8444-555555555555"));

    assert!(es_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"));

    assert!(!es_uuid("11111111222243338444555555555555"));

    assert!(!es_uuid("no-uuid"));
}

#[test]

fn debe_serializar_el_producto_con_montos_y_defaults() {
    let producto = serializar(&fila_producto(), &[], Vec::new()).expect("producto");

    assert!(producto.get("precio_base").is_none());

    assert!(producto.get("precio_desde").is_none());

    assert!(producto.get("stock").is_none());

    assert_eq!(producto["estado"], "activo");

    assert_eq!(producto["estado_operativo"], "activo");

    assert_eq!(producto["unidad"], "Unidad (u)");

    assert_eq!(producto["unidad_compra"], "Igual que la de venta");

    assert_eq!(producto["contenido_unidad"], "Sin contenido");

    assert_eq!(producto["tipo_precio"], "fijo");

    assert!(producto.get("variantes").is_none());

    assert_eq!(producto["imagenes"], json!([]));
}

#[test]

fn debe_serializar_la_imagen_con_url_publica() {
    let imagen = serializar_imagen(&json!({

        "id": "33333333-4444-4555-8666-777777777777",

        "item_id": "11111111-2222-4333-8444-555555555555",

        "objeto_id": "22222222-3333-4444-8555-666666666666",

        "orden": 0,

        "es_principal": true,

    }));

    assert_eq!(
        imagen["url"],
        "/api/v2/storage/public/objetos/22222222-3333-4444-8555-666666666666"
    );

    assert_eq!(imagen["es_principal"], true);
}

#[test]

fn debe_ignorar_items_inactivos_por_defecto_en_los_filtros() {
    let empresa = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

    assert_eq!(
        exigir_empresa(Some(&json!(empresa))).expect("empresa"),
        empresa
    );

    assert_eq!(
        exigir_empresa(None).expect_err("requerida").codigo,
        "EMPRESA_REQUERIDA"
    );

    assert_eq!(
        exigir_empresa(Some(&json!(EMPRESA_LEGACY_ID)))
            .expect_err("legado")
            .codigo,
        "EMPRESA_NO_PUBLICA"
    );
}
