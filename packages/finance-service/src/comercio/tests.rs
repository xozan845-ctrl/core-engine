use super::*;

use serde_json::json as j;

fn usuario(rol: &str, tenant: Option<&str>) -> UsuarioContexto {
    UsuarioContexto {
        user_id: "u-1".into(),

        rol: rol.into(),

        tenant_id: tenant.map(str::to_string),

        ..Default::default()
    }
}

#[test]

fn debe_resolver_el_ambito_de_cada_rol() {
    let vendedor = Comercio::ambito("clientes", &usuario("vendedor", None), None).unwrap();

    assert_eq!(vendedor.propietario_id.as_deref(), Some("u-1"));

    assert!(Comercio::ambito("gastos", &usuario("vendedor", None), None).is_err());

    let empresario = Comercio::ambito("gastos", &usuario("empresario", Some("t-1")), None).unwrap();

    assert_eq!(empresario.tenant_id.as_deref(), Some("t-1"));

    let admin = Comercio::ambito("gastos", &usuario("admin", None), Some("t-9")).unwrap();

    assert_eq!(admin.tenant_id.as_deref(), Some("t-9"));

    assert!(Comercio::ambito("clientes", &usuario("comprador", None), None).is_err());
}

#[test]

fn debe_aplicar_defaults_de_recursos() {
    let normalizado = Comercio::normalizar("gastos", j!({"concept": "luz"})).expect("normalizado");

    assert_eq!(normalizado["status"], "pendiente");

    assert_eq!(normalizado["includesVat"], false);

    assert!(normalizado["createdAt"].is_string());

    let credito = Comercio::normalizar("creditos", j!({"amountCents": 1000, "customer": "Ana"}))
        .expect("credito");

    assert_eq!(credito["balanceCents"], 1000);

    assert_eq!(credito["date"].as_str().unwrap().len(), 10);
}

#[test]

fn debe_calcular_los_totales_de_compras() {
    let compra = Comercio::normalizar(
        "compras",
        j!({"lines": [

            {"quantity": 2, "unitCostCents": 500},

            {"quantity": 3, "unitCostCents": 1000}

        ]}),
    )
    .expect("compra");

    assert_eq!(compra["subtotalCents"], 4000);

    assert_eq!(compra["totalCents"], 4000);

    assert_eq!(compra["status"], "pendiente");
}

#[test]

fn debe_generar_el_codigo_de_barras_si_falta() {
    let ean = Comercio::normalizar(
        "codigos-barras",
        j!({"productName": "x", "sku": "1", "symbology": "EAN13", "copies": 1}),
    )
    .expect("ean");

    let codigo = ean["code"].as_str().unwrap();

    assert_eq!(codigo.len(), 13);

    assert_eq!(
        codigo.chars().last().unwrap().to_digit(10).unwrap(),
        digito_ean13(&codigo[..12])
    );

    let code128 = Comercio::normalizar(
        "codigos-barras",
        j!({"productName": "x", "sku": "1", "symbology": "CODE128", "copies": 1}),
    )
    .expect("code128");

    assert!(code128["code"].as_str().unwrap().starts_with("C128-"));
}

#[test]

fn debe_calcular_el_digito_ean13() {
    assert_eq!(digito_ean13("400638133393"), 1);

    assert_eq!(digito_ean13("590123412345"), 7);
}

#[test]

fn debe_normalizar_la_clave_de_cliente() {
    let valor = j!({"name": " Ana  Pérez ", "phone": "(505) 8888-1111"});

    let clave = clave_cliente(valor.as_object().expect("objeto"));

    assert_eq!(clave, "ana  pérez|50588881111");
}

#[test]

fn debe_calcular_saldo_de_movimientos() {
    let registros = vec![
        j!({"type": "ingreso", "amountCents": 1000})
            .as_object()
            .cloned()
            .unwrap(),
        j!({"type": "egreso", "amountCents": 400})
            .as_object()
            .cloned()
            .unwrap(),
    ];

    assert_eq!(saldo(&registros), 600);

    assert_eq!(suma(&registros), 1400);
}

#[test]

fn debe_validar_las_lineas_de_compra() {
    let linea = LineaCompra {
        name: "Producto".into(),

        quantity: 1.0,

        unit_cost_cents: 100,

        ..Default::default()
    };

    validar_lineas(&[linea]).expect("linea valida");

    let invalida = LineaCompra {
        name: "".into(),

        quantity: 1.0,

        unit_cost_cents: 100,

        ..Default::default()
    };

    assert!(validar_lineas(&[invalida]).is_err());
}
