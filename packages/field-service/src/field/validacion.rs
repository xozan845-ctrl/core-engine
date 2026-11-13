//! Validación de DTOs con el mismo shape de error que `class-validator` +
//! `DtoValidationPipe`: `VALIDACION` con `detalles[{campo, restricciones}]`.
//!
//! Las reglas replican los decoradores declarados en `field.dtos.ts` y el
//! `whitelist` del pipe (los campos desconocidos se ignoran).

use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Regla {
    Texto,
    Email,
    En(&'static [&'static str]),
    Numero,
    Entero,
    Min(f64),
    Booleano,
    Arreglo,
    Objeto,
    Fecha,
    Uuid,
}

impl Regla {
    fn nombre(self) -> &'static str {
        match self {
            Regla::Texto => "isString",
            Regla::Email => "isEmail",
            Regla::En(_) => "isIn",
            Regla::Numero => "isNumber",
            Regla::Entero => "isInt",
            Regla::Min(_) => "min",
            Regla::Booleano => "isBoolean",
            Regla::Arreglo => "isArray",
            Regla::Objeto => "isObject",
            Regla::Fecha => "isDateString",
            Regla::Uuid => "isUuid",
        }
    }

    fn cumple(self, valor: &Value) -> bool {
        match self {
            Regla::Texto => valor.is_string(),
            Regla::Email => valor.as_str().is_some_and(es_email),
            Regla::En(opciones) => valor
                .as_str()
                .is_some_and(|texto| opciones.contains(&texto)),
            Regla::Numero => valor.is_number(),
            Regla::Entero => valor
                .as_f64()
                .is_some_and(|numero| numero.is_finite() && numero.fract() == 0.0),
            Regla::Min(minimo) => valor.as_f64().is_some_and(|numero| numero >= minimo),
            Regla::Booleano => valor.is_boolean(),
            Regla::Arreglo => valor.is_array(),
            Regla::Objeto => valor.is_object(),
            Regla::Fecha => valor.as_str().is_some_and(es_fecha_iso),
            Regla::Uuid => valor
                .as_str()
                .is_some_and(|texto| Uuid::parse_str(texto).is_ok()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReglaCampo {
    pub campo: &'static str,
    pub requerido: bool,
    pub reglas: &'static [Regla],
}

pub fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    let vacio = Map::new();
    let objeto = cuerpo.as_object().unwrap_or(&vacio);
    let mut detalles = Vec::new();

    for campo in esquema {
        let valor = objeto.get(campo.campo);
        let ausente_o_nulo = matches!(valor, None | Some(Value::Null));
        if ausente_o_nulo {
            if campo.requerido {
                let restricciones = campo
                    .reglas
                    .iter()
                    .map(|regla| regla.nombre())
                    .collect::<Vec<_>>();
                detalles.push(json!({
                    "campo": campo.campo,
                    "restricciones": restricciones,
                }));
            }
            continue;
        }
        let valor = valor.expect("valor presente");
        let fallidas = campo
            .reglas
            .iter()
            .filter(|regla| !regla.cumple(valor))
            .map(|regla| regla.nombre())
            .collect::<Vec<_>>();
        if !fallidas.is_empty() {
            detalles.push(json!({
                "campo": campo.campo,
                "restricciones": fallidas,
            }));
        }
    }

    if detalles.is_empty() {
        Ok(())
    } else {
        Err(ApiError::validacion(Value::Array(detalles)))
    }
}

fn es_email(texto: &str) -> bool {
    let mut partes = texto.split('@');
    let Some(local) = partes.next() else {
        return false;
    };
    let Some(dominio) = partes.next() else {
        return false;
    };
    if partes.next().is_some() || local.is_empty() || dominio.is_empty() {
        return false;
    }
    if local.chars().any(char::is_whitespace) || dominio.chars().any(char::is_whitespace) {
        return false;
    }
    let mut etiquetas = dominio.split('.');
    etiquetas
        .next()
        .is_some_and(|etiqueta| !etiqueta.is_empty())
        && etiquetas
            .next()
            .is_some_and(|etiqueta| !etiqueta.is_empty())
}

fn es_fecha_iso(texto: &str) -> bool {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};
    if DateTime::parse_from_rfc3339(texto).is_ok() {
        return true;
    }
    if NaiveDate::parse_from_str(texto, "%Y-%m-%d").is_ok() {
        return true;
    }
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ]
    .iter()
    .any(|formato| NaiveDateTime::parse_from_str(texto, formato).is_ok())
}

// ── opciones de enum ──────────────────────────────────────────────────────
const CARGOS: &[&str] = &["conductor", "auxiliar", "supervisor", "coordinador"];
const ESTADOS_PERSONAL: &[&str] = &["activo", "en_ruta", "descansando", "inactivo"];
const TIPOS_CLIENTE: &[&str] = &["particular", "minorista", "mayorista", "corporativo"];
const TIPOS_VEHICULO: &[&str] = &["camioneta", "furgon", "camion", "moto", "otro"];
const ESTADOS_VEHICULO: &[&str] = &[
    "disponible",
    "en_ruta",
    "mantenimiento",
    "fuera_de_servicio",
];
const ESTADOS_RUTA: &[&str] = &["pendiente", "en_curso", "completada", "cancelada"];
const TIPOS_PARADA: &[&str] = &["cliente", "logistica"];
const ESTADOS_PEDIDO: &[&str] = &[
    "pendiente",
    "asignado",
    "en_camino",
    "entregado",
    "fallido",
    "cancelado",
];
const TIPOS_ASISTENCIA: &[&str] = &["entrada", "salida"];
const PUNTUALIDAD: &[&str] = &["puntual", "tardanza", "salida_anticipada", "salida_tardia"];
const TIPOS_INCIDENCIA: &[&str] = &["mecanica", "accidente", "salud", "clima", "otro"];
const ESTADOS_INCIDENCIA: &[&str] = &["abierta", "en_progreso", "resuelta"];
const TIPOS_ACTIVIDAD: &[&str] = &["impulsacion", "venta", "entrega", "reparto"];
const RESULTADOS_VISITA: &[&str] = &["visitado", "cerca_no_visitado", "no_visitado"];

// ── esquemas ──────────────────────────────────────────────────────────────
pub const CREAR_PERSONAL: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto]),
    campo("apellido", false, &[Regla::Texto]),
    campo("cargo", false, &[Regla::En(CARGOS)]),
    campo("estado", false, &[Regla::En(ESTADOS_PERSONAL)]),
    campo("telefono", false, &[Regla::Texto]),
    campo("email", false, &[Regla::Email]),
    campo("rutaAsignadaId", false, &[Regla::Texto]),
    campo("vehiculoAsignadoId", false, &[Regla::Texto]),
    campo("horaCheckIn", false, &[Regla::Fecha]),
    campo("horaCheckOut", false, &[Regla::Fecha]),
    campo("ubicacionLat", false, &[Regla::Numero]),
    campo("ubicacionLng", false, &[Regla::Numero]),
    campo("ubicacionPrecision", false, &[Regla::Numero]),
    campo("ubicacionTs", false, &[Regla::Fecha]),
];

pub const ACTUALIZAR_PERSONAL: &[ReglaCampo] = &[
    campo("nombre", false, &[Regla::Texto]),
    campo("apellido", false, &[Regla::Texto]),
    campo("cargo", false, &[Regla::En(CARGOS)]),
    campo("estado", false, &[Regla::En(ESTADOS_PERSONAL)]),
    campo("telefono", false, &[Regla::Texto]),
    campo("email", false, &[Regla::Email]),
    campo("rutaAsignadaId", false, &[Regla::Texto]),
    campo("vehiculoAsignadoId", false, &[Regla::Texto]),
    campo("horaCheckIn", false, &[Regla::Fecha]),
    campo("horaCheckOut", false, &[Regla::Fecha]),
    campo("ubicacionLat", false, &[Regla::Numero]),
    campo("ubicacionLng", false, &[Regla::Numero]),
    campo("ubicacionPrecision", false, &[Regla::Numero]),
    campo("ubicacionTs", false, &[Regla::Fecha]),
];

pub const CREAR_CLIENTE: &[ReglaCampo] = &[
    campo("nombreCompleto", true, &[Regla::Texto]),
    campo("tipoDocumento", false, &[Regla::Texto]),
    campo("numeroDocumento", false, &[Regla::Texto]),
    campo("tipoCliente", false, &[Regla::En(TIPOS_CLIENTE)]),
    campo("email", false, &[Regla::Email]),
    campo("telefono", false, &[Regla::Texto]),
    campo("telefonoAlternativo", false, &[Regla::Texto]),
    campo("direccionPrincipal", false, &[Regla::Texto]),
    campo("direccionSecundaria", false, &[Regla::Texto]),
    campo("referenciasDireccion", false, &[Regla::Texto]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("notasAdicionales", false, &[Regla::Texto]),
    campo("urlGoogleMaps", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const ACTUALIZAR_CLIENTE: &[ReglaCampo] = &[
    campo("nombreCompleto", false, &[Regla::Texto]),
    campo("tipoDocumento", false, &[Regla::Texto]),
    campo("numeroDocumento", false, &[Regla::Texto]),
    campo("tipoCliente", false, &[Regla::En(TIPOS_CLIENTE)]),
    campo("email", false, &[Regla::Email]),
    campo("telefono", false, &[Regla::Texto]),
    campo("telefonoAlternativo", false, &[Regla::Texto]),
    campo("direccionPrincipal", false, &[Regla::Texto]),
    campo("direccionSecundaria", false, &[Regla::Texto]),
    campo("referenciasDireccion", false, &[Regla::Texto]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("notasAdicionales", false, &[Regla::Texto]),
    campo("urlGoogleMaps", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const CREAR_VEHICULO: &[ReglaCampo] = &[
    campo("placa", true, &[Regla::Texto]),
    campo("tipo", false, &[Regla::En(TIPOS_VEHICULO)]),
    campo("marca", false, &[Regla::Texto]),
    campo("modelo", false, &[Regla::Texto]),
    campo("anio", false, &[Regla::Entero, Regla::Min(1900.0)]),
    campo("estado", false, &[Regla::En(ESTADOS_VEHICULO)]),
    campo("color", false, &[Regla::Texto]),
    campo("capacidadCargaKg", false, &[Regla::Numero]),
    campo("numeroChasis", false, &[Regla::Texto]),
    campo("numeroMotor", false, &[Regla::Texto]),
    campo("tipoCombustible", false, &[Regla::Texto]),
    campo("vencimientoSeguro", false, &[Regla::Fecha]),
    campo("vencimientoCirculacion", false, &[Regla::Fecha]),
    campo("notasAdicionales", false, &[Regla::Texto]),
    campo("conductorId", false, &[Regla::Texto]),
    campo("rutaActivaId", false, &[Regla::Texto]),
];

pub const ACTUALIZAR_VEHICULO: &[ReglaCampo] = &[
    campo("placa", false, &[Regla::Texto]),
    campo("tipo", false, &[Regla::En(TIPOS_VEHICULO)]),
    campo("marca", false, &[Regla::Texto]),
    campo("modelo", false, &[Regla::Texto]),
    campo("anio", false, &[Regla::Entero, Regla::Min(1900.0)]),
    campo("estado", false, &[Regla::En(ESTADOS_VEHICULO)]),
    campo("color", false, &[Regla::Texto]),
    campo("capacidadCargaKg", false, &[Regla::Numero]),
    campo("numeroChasis", false, &[Regla::Texto]),
    campo("numeroMotor", false, &[Regla::Texto]),
    campo("tipoCombustible", false, &[Regla::Texto]),
    campo("vencimientoSeguro", false, &[Regla::Fecha]),
    campo("vencimientoCirculacion", false, &[Regla::Fecha]),
    campo("notasAdicionales", false, &[Regla::Texto]),
    campo("conductorId", false, &[Regla::Texto]),
    campo("rutaActivaId", false, &[Regla::Texto]),
];

pub const CREAR_RUTA: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("estado", false, &[Regla::En(ESTADOS_RUTA)]),
    campo("personalIds", false, &[Regla::Arreglo]),
    campo("vehiculoAsignadoId", false, &[Regla::Texto]),
    campo("fechaInicio", false, &[Regla::Fecha]),
    campo("fechaFin", false, &[Regla::Fecha]),
];

pub const ACTUALIZAR_RUTA: &[ReglaCampo] = &[
    campo("nombre", false, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("estado", false, &[Regla::En(ESTADOS_RUTA)]),
    campo("personalIds", false, &[Regla::Arreglo]),
    campo("vehiculoAsignadoId", false, &[Regla::Texto]),
    campo("fechaInicio", false, &[Regla::Fecha]),
    campo("fechaFin", false, &[Regla::Fecha]),
];

pub const PARADA: &[ReglaCampo] = &[
    campo("rutaId", false, &[Regla::Texto]),
    campo("orden", false, &[Regla::Entero, Regla::Min(0.0)]),
    campo("nombre", true, &[Regla::Texto]),
    campo("direccion", false, &[Regla::Texto]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("completada", false, &[Regla::Booleano]),
    campo("pedidoId", false, &[Regla::Texto]),
    campo("tipo", false, &[Regla::En(TIPOS_PARADA)]),
    campo("telefono", false, &[Regla::Texto]),
    campo("referencias", false, &[Regla::Texto]),
    campo("horarioAtencion", false, &[Regla::Texto]),
    campo("notas", false, &[Regla::Texto]),
    campo("tipoCombustible", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const ASIGNAR_RUTA: &[ReglaCampo] = &[
    campo("personalIds", false, &[Regla::Arreglo]),
    campo("vehiculoId", false, &[Regla::Texto]),
];

pub const CREAR_PEDIDO: &[ReglaCampo] = &[
    campo("cliente", false, &[Regla::Texto]),
    campo("direccionEntrega", false, &[Regla::Texto]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("estado", false, &[Regla::En(ESTADOS_PEDIDO)]),
    campo("rutaId", false, &[Regla::Texto]),
    campo("notas", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const ACTUALIZAR_PEDIDO: &[ReglaCampo] = &[
    campo("cliente", false, &[Regla::Texto]),
    campo("direccionEntrega", false, &[Regla::Texto]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("estado", false, &[Regla::En(ESTADOS_PEDIDO)]),
    campo("rutaId", false, &[Regla::Texto]),
    campo("notas", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const CAMBIAR_ESTADO_PEDIDO: &[ReglaCampo] = &[
    campo("estado", true, &[Regla::En(ESTADOS_PEDIDO)]),
    campo("motivo", false, &[Regla::Texto]),
];

pub const CREAR_ASISTENCIA: &[ReglaCampo] = &[
    campo("personalId", true, &[Regla::Texto]),
    campo("tipo", true, &[Regla::En(TIPOS_ASISTENCIA)]),
    campo("timestamp", false, &[Regla::Fecha]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("estadoPuntualidad", true, &[Regla::En(PUNTUALIDAD)]),
    campo("enSede", false, &[Regla::Booleano]),
    campo("horaRealLlegada", false, &[Regla::Texto]),
    campo("minutosRetrasoReal", false, &[Regla::Entero]),
    campo("monitoreoActivo", false, &[Regla::Booleano]),
    campo("notas", false, &[Regla::Texto]),
    campo("justificacion", false, &[Regla::Texto]),
    campo("fotosJustificacion", false, &[Regla::Arreglo]),
];

pub const CREAR_INCIDENCIA: &[ReglaCampo] = &[
    campo("tipo", true, &[Regla::En(TIPOS_INCIDENCIA)]),
    campo("estado", false, &[Regla::En(ESTADOS_INCIDENCIA)]),
    campo("descripcion", true, &[Regla::Texto]),
    campo("personalId", false, &[Regla::Texto]),
    campo("vehiculoId", false, &[Regla::Texto]),
    campo("rutaId", false, &[Regla::Texto]),
    campo("resolucion", false, &[Regla::Texto]),
];

pub const ACTUALIZAR_INCIDENCIA: &[ReglaCampo] = &[
    campo("tipo", false, &[Regla::En(TIPOS_INCIDENCIA)]),
    campo("estado", false, &[Regla::En(ESTADOS_INCIDENCIA)]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("personalId", false, &[Regla::Texto]),
    campo("vehiculoId", false, &[Regla::Texto]),
    campo("rutaId", false, &[Regla::Texto]),
    campo("resolucion", false, &[Regla::Texto]),
];

pub const CREAR_TRACKING: &[ReglaCampo] = &[
    campo("personalId", true, &[Regla::Texto]),
    campo("latitud", true, &[Regla::Numero]),
    campo("longitud", true, &[Regla::Numero]),
    campo("precision", false, &[Regla::Numero]),
    campo("velocidad", false, &[Regla::Numero]),
    campo("rumbo", false, &[Regla::Numero]),
    campo("timestamp", false, &[Regla::Fecha]),
];

pub const TRACKING_BULK: &[ReglaCampo] = &[campo("registros", true, &[Regla::Arreglo])];

pub const CREAR_VISITA: &[ReglaCampo] = &[
    campo("personalId", true, &[Regla::Texto]),
    campo("rutaId", false, &[Regla::Texto]),
    campo("tipoActividad", true, &[Regla::En(TIPOS_ACTIVIDAD)]),
    campo("resultado", false, &[Regla::En(RESULTADOS_VISITA)]),
    campo("lat", false, &[Regla::Numero]),
    campo("lng", false, &[Regla::Numero]),
    campo("distanciaAlCliente", false, &[Regla::Numero]),
    campo("horaLlegada", false, &[Regla::Texto]),
    campo("horaSalida", false, &[Regla::Texto]),
    campo("duracionMinutos", false, &[Regla::Entero]),
    campo("notas", false, &[Regla::Texto]),
    campo("parteId", false, &[Regla::Uuid]),
];

pub const GUARDAR_CUMPLIMIENTO: &[ReglaCampo] = &[
    campo("rutaId", true, &[Regla::Texto]),
    campo("fecha", true, &[Regla::Fecha]),
    campo("metricas", true, &[Regla::Objeto]),
];

pub const UBICACION: &[ReglaCampo] = &[
    campo("lat", true, &[Regla::Numero]),
    campo("lng", true, &[Regla::Numero]),
    campo("precision", false, &[Regla::Numero]),
    campo("timestamp", false, &[Regla::Fecha]),
];

pub const SYNC_REQUEST: &[ReglaCampo] = &[campo("operaciones", true, &[Regla::Arreglo])];

const fn campo(nombre: &'static str, requerido: bool, reglas: &'static [Regla]) -> ReglaCampo {
    ReglaCampo {
        campo: nombre,
        requerido,
        reglas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_aceptar_un_payload_valido() {
        assert!(validar(
            CREAR_PERSONAL,
            &json!({"nombre": "Ana", "email": "ana@test.dev", "cargo": "conductor"})
        )
        .is_ok());
    }

    #[test]
    fn debe_reportar_los_campos_requeridos_con_sus_restricciones() {
        let error = validar(CREAR_PERSONAL, &json!({})).expect_err("validacion");
        let cuerpo = error.cuerpo();
        assert_eq!(cuerpo["codigo"], "VALIDACION");
        assert_eq!(cuerpo["detalles"][0]["campo"], "nombre");
        assert_eq!(cuerpo["detalles"][0]["restricciones"], json!(["isString"]));
    }

    #[test]
    fn debe_validar_enums_email_fechas_uuids_y_numeros() {
        let error = validar(
            CREAR_CLIENTE,
            &json!({
                "nombreCompleto": "Ana",
                "tipoCliente": "otro",
                "email": "sin-arroba",
                "lat": "12",
                "parteId": "no-es-uuid",
            }),
        )
        .expect_err("validacion");
        let detalles = error.cuerpo()["detalles"].clone();
        let campos = detalles
            .as_array()
            .expect("detalles")
            .iter()
            .map(|detalle| detalle["campo"].as_str().expect("campo").to_string())
            .collect::<Vec<_>>();
        assert_eq!(campos, vec!["tipoCliente", "email", "lat", "parteId"]);
    }

    #[test]
    fn debe_ignorar_campos_desconocidos_y_nulos_opcionales() {
        assert!(validar(
            ACTUALIZAR_PERSONAL,
            &json!({"nombre": null, "desconocido": true})
        )
        .is_ok());
    }

    #[test]
    fn debe_validar_anio_entero_y_minimo() {
        let error =
            validar(CREAR_VEHICULO, &json!({"placa": "M1", "anio": 1800})).expect_err("validacion");
        let restricciones = error.cuerpo()["detalles"][0]["restricciones"].clone();
        assert_eq!(restricciones, json!(["min"]));
    }

    #[test]
    fn debe_aceptar_fechas_iso_y_rechazar_texto_libre() {
        assert!(validar(
            GUARDAR_CUMPLIMIENTO,
            &json!({"rutaId": "r1", "fecha": "2026-10-14", "metricas": {}})
        )
        .is_ok());
        assert!(validar(
            GUARDAR_CUMPLIMIENTO,
            &json!({"rutaId": "r1", "fecha": "ayer", "metricas": {}})
        )
        .is_err());
    }

    #[test]
    fn debe_validar_los_objetos_y_arreglos() {
        assert!(validar(SYNC_REQUEST, &json!({"operaciones": []})).is_ok());
        assert!(validar(SYNC_REQUEST, &json!({"operaciones": {}})).is_err());
        assert!(validar(TRACKING_BULK, &json!({"registros": []})).is_ok());
        assert!(validar(TRACKING_BULK, &json!({"registros": "x"})).is_err());
    }
}
