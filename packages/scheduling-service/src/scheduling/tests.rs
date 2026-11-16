use super::*;

const UUID: &str = "123e4567-e89b-42d3-a456-426614174000";

#[test]

fn debe_aceptar_el_recurso_valido_y_normalizar_atributos() {
    let dto = validar_crear_recurso(&json!({

        "tipo": "sala", "nombre": " Sala 1 ", "atributos": {"piso": 2}

    }))
    .expect("valido");

    assert_eq!(dto.tipo, "sala");

    assert_eq!(dto.nombre, " Sala 1 ");

    assert_eq!(dto.capacidad, 1);

    assert_eq!(dto.atributos["piso"], 2);
}

#[test]

fn debe_validar_la_capacidad_del_recurso() {
    let dto = validar_crear_recurso(&json!({

        "tipo": "sala", "nombre": "Clase grupal", "capacidad": 20

    }))
    .expect("valido");

    assert_eq!(dto.capacidad, 20);

    let error = validar_crear_recurso(&json!({

        "tipo": "sala", "nombre": "Clase grupal", "capacidad": 0

    }))
    .expect_err("invalido");

    assert_eq!(
        error.cuerpo()["detalles"],
        json!([{"campo": "capacidad", "restricciones": ["isInt", "min"]}])
    );
}

#[test]

fn debe_reportar_las_restricciones_en_orden_de_declaracion() {
    let error =
        validar_crear_recurso(&json!({ "tipo": "nave", "atributos": [] })).expect_err("invalido");

    assert_eq!(
        error.cuerpo()["detalles"],
        json!([

            {"campo": "tipo", "restricciones": ["isIn"]},

            {"campo": "nombre", "restricciones": ["isString"]},

            {"campo": "atributos", "restricciones": ["isObject"]},

        ])
    );
}

#[test]

fn debe_validar_uuid_con_version_y_variante() {
    assert!(es_uuid(UUID));

    assert!(!es_uuid("123e4567-e89b-02d3-a456-426614174000"));

    assert!(!es_uuid("123e4567-e89b-42d3-c456-426614174000"));

    assert!(!es_uuid("no-es-uuid"));
}

#[test]

fn debe_rechazar_dia_semana_fuera_de_rango_con_min_o_max() {
    let fuera = validar_crear_disponibilidad(&json!({

        "recurso_id": UUID, "dia_semana": 7, "inicia_hora": "08:00", "termina_hora": "17:00"

    }))
    .expect_err("invalido");

    assert_eq!(
        fuera.cuerpo()["detalles"],
        json!([{"campo": "dia_semana", "restricciones": ["max"]}])
    );
}

#[test]

fn debe_validar_las_fechas_opcionales_como_iso8601() {
    let error = validar_crear_disponibilidad(&json!({

        "recurso_id": UUID, "dia_semana": 1, "inicia_hora": "08:00", "termina_hora": "17:00",

        "vigente_desde": "2026-13-01"

    }))
    .expect_err("invalido");

    assert_eq!(
        error.cuerpo()["detalles"],
        json!([{"campo": "vigente_desde", "restricciones": ["isDateString"]}])
    );
}

#[test]

fn debe_validar_el_periodo_igual_que_date_parse() {
    assert!(validar_periodo("2030-04-15T14:00:00.000Z", "2030-04-15T15:00:00.000Z").is_ok());

    assert_eq!(
        validar_periodo("2030-04-15T15:00:00Z", "2030-04-15T14:00:00Z")
            .expect_err("invalido")
            .codigo,
        "PERIODO_INVALIDO"
    );

    assert_eq!(
        validar_periodo("no-fecha", "2030-04-15T14:00:00Z")
            .expect_err("invalido")
            .codigo,
        "PERIODO_INVALIDO"
    );
}

#[test]

fn debe_aceptar_un_cuerpo_vacio_como_objeto_sin_campos() {
    let error = validar_crear_bloqueo(&Value::Null).expect_err("invalido");

    assert_eq!(
        error.cuerpo()["detalles"],
        json!([

            {"campo": "recurso_id", "restricciones": ["isUuid"]},

            {"campo": "inicia_en", "restricciones": ["isDateString"]},

            {"campo": "termina_en", "restricciones": ["isDateString"]},

        ])
    );
}

#[test]

fn debe_validar_la_cita_como_el_dto_original() {
    let valida = validar_crear_cita(&json!({

        "recurso_id": UUID,

        "parte_id": "22222222-2222-4222-8222-222222222222",

        "order_id": "33333333-3333-4333-8333-333333333333",

        "inicia_en": "2030-04-15T14:00:00.000Z",

        "termina_en": "2030-04-15T15:00:00.000Z",

        "metadata": {"canal": "web"}

    }))
    .expect("valida");

    assert_eq!(valida.metadata["canal"], "web");

    assert_eq!(valida.notas, None);

    let invalida = validar_crear_cita(&json!({

        "recurso_id": "no-uuid",

        "order_id": "tampoco",

        "inicia_en": "2030-04-15",

        "metadata": []

    }))
    .expect_err("invalida");

    assert_eq!(
        invalida.cuerpo()["detalles"],
        json!([

            {"campo": "recurso_id", "restricciones": ["isUuid"]},

            {"campo": "parte_id", "restricciones": ["isUuid"]},

            {"campo": "order_id", "restricciones": ["isUuid"]},

            {"campo": "termina_en", "restricciones": ["isDateString"]},

            {"campo": "metadata", "restricciones": ["isObject"]},

        ])
    );
}
