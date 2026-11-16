use super::helpers::*;
use super::*;

#[test]

fn debe_mapear_filas_de_importacion_sin_campos_ausentes() {
    let fila = json!({

        "sku": "SKU-1",

        "nombre": "Producto",

        "precio_base": "10.00",

        "stock": 5,

        "costo": "4.50",

        "codigo_barras": "750",

        "stock_maximo": null

    });

    let datos = datos_producto_importado(&fila, "empresa-1");

    assert_eq!(datos["empresa_id"], "empresa-1");

    assert_eq!(datos["sku"], "SKU-1");

    assert_eq!(datos["precio_base"], "10.00");

    assert_eq!(datos["costo"], "4.50");

    assert!(datos.get("descripcion").is_none());

    assert!(datos.get("stock_maximo").is_none());
}

#[test]

fn debe_normalizar_sku_y_validar_nombres() {
    assert_eq!(normalizar_sku(" sku-1 ").expect("sku"), "SKU-1");

    assert_eq!(
        normalizar_sku("a").expect_err("corto").codigo,
        "SKU_INVALIDO"
    );

    assert_eq!(
        normalizar_sku("skú").expect_err("simbolo").codigo,
        "SKU_INVALIDO"
    );

    assert_eq!(validar_nombre(" Producto ").expect("nombre"), "Producto");

    assert_eq!(
        validar_nombre("x").expect_err("corto").codigo,
        "NOMBRE_INVALIDO"
    );
}

#[test]

fn debe_calcular_el_margen_del_combo_como_el_contrato() {
    // precio 30.00, costo 20.00 -> 33.3

    assert_eq!(calcular_margen(3000, 2000), 33.3);

    // precio 0 -> 0 sin dividir entre cero.

    assert_eq!(calcular_margen(0, 2000), 0.0);

    // Sin costo -> 100 %.

    assert_eq!(calcular_margen(3000, 0), 100.0);

    // Costo mayor que el precio: margen negativo redondeado como JS.

    assert_eq!(calcular_margen(3000, 3010), -0.3);
}

#[test]

fn debe_validar_promociones_y_normalizar_2x1() {
    let dos_por_uno = validar_promocion(&json!({

        "nombre": "2x1",

        "tipo": "2x1",

        "fecha_desde": "2026-10-01T00:00:00.000Z",

        "fecha_hasta": "2026-12-31",

    }))
    .expect("2x1");

    assert_eq!(dos_por_uno.cantidad_requerida, Some(2));

    assert_eq!(dos_por_uno.cantidad_gratis, Some(1));

    assert_eq!(dos_por_uno.fecha_desde, "2026-10-01");

    let regalo_sin_cantidades = validar_promocion(&json!({

        "nombre": "Regalo",

        "tipo": "regalo",

        "fecha_desde": "2026-10-01",

        "fecha_hasta": "2026-12-31",

    }))
    .expect_err("regalo");

    assert_eq!(regalo_sin_cantidades.codigo, "PROMOCION_INVALIDA");

    let descuento_invalido = validar_promocion(&json!({

        "nombre": "Descuento",

        "tipo": "descuento_porcentaje",

        "descuento_porcentaje": 200,

        "fecha_desde": "2026-10-01",

        "fecha_hasta": "2026-12-31",

    }))
    .expect_err("descuento");

    assert_eq!(descuento_invalido.codigo, "PROMOCION_INVALIDA");

    let vigencia_invertida = validar_promocion(&json!({

        "nombre": "Promo",

        "tipo": "2x1",

        "fecha_desde": "2026-12-31",

        "fecha_hasta": "2026-10-01",

    }))
    .expect_err("vigencia");

    assert_eq!(vigencia_invertida.codigo, "VIGENCIA_INVALIDA");

    let tipo_invalido = validar_promocion(&json!({

        "nombre": "Promo",

        "tipo": "otro",

        "fecha_desde": "2026-10-01",

        "fecha_hasta": "2026-12-31",

    }))
    .expect_err("tipo");

    assert_eq!(tipo_invalido.codigo, "TIPO_PROMOCION_INVALIDO");

    let descuento_cantidad = validar_promocion(&json!({

        "nombre": "Descuento",

        "tipo": "descuento_cantidad",

        "cantidad_requerida": 3,

        "descuento_porcentaje": 10,

        "fecha_desde": "2026-10-01",

        "fecha_hasta": "2026-12-31",

    }))
    .expect("descuento_cantidad");

    assert_eq!(descuento_cantidad.cantidad_requerida, Some(3));

    assert_eq!(descuento_cantidad.descuento_porcentaje, Some(10));
}

#[test]

fn debe_validar_cantidades_y_duplicados_de_componentes() {
    assert_eq!(
        cantidad_componente(&json!({"cantidad": 2})).expect("cantidad"),
        2
    );

    assert_eq!(
        cantidad_componente(&json!({"cantidad": 0}))
            .expect_err("cero")
            .codigo,
        "CANTIDAD_INVALIDA"
    );

    assert_eq!(
        cantidad_componente(&json!({"cantidad": 1.5}))
            .expect_err("fraccion")
            .codigo,
        "CANTIDAD_INVALIDA"
    );

    let mut vistos = Vec::new();

    registrar_componente_unico(&mut vistos, "item-1").expect("primero");

    assert_eq!(
        registrar_componente_unico(&mut vistos, "item-1")
            .expect_err("duplicado")
            .codigo,
        "COMPONENTE_DUPLICADO"
    );
}

#[test]

fn debe_calcular_el_stock_de_cada_movimiento() {
    assert_eq!(calcular_stock("entrada", 10, 5), 15);

    assert_eq!(calcular_stock("salida", 10, 5), 5);

    assert_eq!(calcular_stock("ajuste", 10, 3), 3);

    assert_eq!(calcular_stock("traslado", 10, 5), 10);
}

#[test]

fn debe_validar_cabeceras_de_movimiento() {
    let cabecera = validar_cabecera_movimiento(&json!({

        "tipo": "traslado",

        "ubicacion_origen": "A",

        "ubicacion_destino": "B",

        "lineas": [{"sku": "SKU-1", "cantidad": 1}],

    }))
    .expect("traslado");

    assert_eq!(cabecera.tipo, "traslado");

    assert_eq!(cabecera.origen, "A");

    let sin_lineas = validar_cabecera_movimiento(&json!({

        "tipo": "entrada",

        "lineas": [],

    }))
    .expect_err("sin lineas");

    assert_eq!(sin_lineas.codigo, "LINEAS_REQUERIDAS");

    let traslado_invalido = validar_cabecera_movimiento(&json!({

        "tipo": "traslado",

        "ubicacion_origen": "A",

        "ubicacion_destino": "A",

        "lineas": [{"sku": "SKU-1", "cantidad": 1}],

    }))
    .expect_err("traslado invalido");

    assert_eq!(traslado_invalido.codigo, "TRASLADO_INVALIDO");

    let demasiadas = validar_cabecera_movimiento(&json!({

        "tipo": "entrada",

        "lineas": (0..201).map(|_| json!({"sku": "SKU-1", "cantidad": 1}))

            .collect::<Vec<_>>(),

    }))
    .expect_err("demasiadas");

    assert_eq!(demasiadas.codigo, "DEMASIADAS_LINEAS");
}

#[test]

fn debe_interpretar_cantidades_como_number_de_javascript() {
    assert_eq!(entero_de(Some(&json!(5))), Some(5));

    assert_eq!(entero_de(Some(&json!(5.0))), Some(5));

    assert_eq!(entero_de(Some(&json!("5"))), Some(5));

    assert_eq!(entero_de(Some(&json!(""))), Some(0));

    assert_eq!(entero_de(Some(&json!(1.5))), None);
}
