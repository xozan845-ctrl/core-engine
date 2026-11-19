//! Dominio de logística de campo: CRUD multi-tenant sobre `field.*`.
//!
//! Réplica de `field.service.ts`: inserciones/actualizaciones dinámicas por
//! mapa de columnas, defaults de columna, borrado lógico, filtros acotados y
//! validación de `parte_id` contra identity.

pub mod validacion;

use crate::clients::Partes;
use crate::db::{cast, consulta, fila_a_json, tipo_columna, Db, TipoColumna, Valor};
use crate::errors::ApiError;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct Campo {
    pub json: &'static str,
    pub col: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Tabla {
    pub nombre: &'static str,
    pub campos: &'static [Campo],
}

const fn c(json: &'static str, col: &'static str) -> Campo {
    Campo { json, col }
}

pub const MAP_PERSONAL: &[Campo] = &[
    c("nombre", "nombre"),
    c("apellido", "apellido"),
    c("cargo", "cargo"),
    c("estado", "estado"),
    c("telefono", "telefono"),
    c("email", "email"),
    c("rutaAsignadaId", "ruta_asignada_id"),
    c("vehiculoAsignadoId", "vehiculo_asignado_id"),
    c("horaCheckIn", "hora_check_in"),
    c("horaCheckOut", "hora_check_out"),
    c("ubicacionLat", "ubicacion_lat"),
    c("ubicacionLng", "ubicacion_lng"),
    c("ubicacionPrecision", "ubicacion_precision"),
    c("ubicacionTs", "ubicacion_ts"),
];

pub const MAP_CLIENTES: &[Campo] = &[
    c("nombreCompleto", "nombre_completo"),
    c("tipoDocumento", "tipo_documento"),
    c("numeroDocumento", "numero_documento"),
    c("tipoCliente", "tipo_cliente"),
    c("email", "email"),
    c("telefono", "telefono"),
    c("telefonoAlternativo", "telefono_alternativo"),
    c("direccionPrincipal", "direccion_principal"),
    c("direccionSecundaria", "direccion_secundaria"),
    c("referenciasDireccion", "referencias_direccion"),
    c("lat", "lat"),
    c("lng", "lng"),
    c("notasAdicionales", "notas_adicionales"),
    c("urlGoogleMaps", "url_google_maps"),
    c("parteId", "parte_id"),
];

pub const MAP_VEHICULOS: &[Campo] = &[
    c("placa", "placa"),
    c("tipo", "tipo"),
    c("marca", "marca"),
    c("modelo", "modelo"),
    c("anio", "anio"),
    c("estado", "estado"),
    c("color", "color"),
    c("capacidadCargaKg", "capacidad_carga_kg"),
    c("numeroChasis", "numero_chasis"),
    c("numeroMotor", "numero_motor"),
    c("tipoCombustible", "tipo_combustible"),
    c("vencimientoSeguro", "vencimiento_seguro"),
    c("vencimientoCirculacion", "vencimiento_circulacion"),
    c("notasAdicionales", "notas_adicionales"),
    c("conductorId", "conductor_id"),
    c("rutaActivaId", "ruta_activa_id"),
];

pub const MAP_RUTAS: &[Campo] = &[
    c("nombre", "nombre"),
    c("descripcion", "descripcion"),
    c("estado", "estado"),
    c("personalIds", "personal_asignado_ids"),
    c("vehiculoAsignadoId", "vehiculo_asignado_id"),
    c("fechaInicio", "fecha_inicio"),
    c("fechaFin", "fecha_fin"),
];

pub const MAP_PARADAS: &[Campo] = &[
    c("rutaId", "ruta_id"),
    c("orden", "orden"),
    c("nombre", "nombre"),
    c("direccion", "direccion"),
    c("lat", "lat"),
    c("lng", "lng"),
    c("completada", "completada"),
    c("pedidoId", "pedido_id"),
    c("tipo", "tipo"),
    c("telefono", "telefono"),
    c("referencias", "referencias"),
    c("horarioAtencion", "horario_atencion"),
    c("notas", "notas"),
    c("tipoCombustible", "tipo_combustible"),
    c("parteId", "parte_id"),
];

pub const MAP_PEDIDOS: &[Campo] = &[
    c("cliente", "cliente"),
    c("direccionEntrega", "direccion_entrega"),
    c("lat", "lat"),
    c("lng", "lng"),
    c("estado", "estado"),
    c("rutaId", "ruta_id"),
    c("notas", "notas"),
    c("parteId", "parte_id"),
];

pub const MAP_ASISTENCIA: &[Campo] = &[
    c("personalId", "personal_id"),
    c("tipo", "tipo"),
    c("timestamp", "registrado_en"),
    c("lat", "lat"),
    c("lng", "lng"),
    c("estadoPuntualidad", "estado_puntualidad"),
    c("enSede", "en_sede"),
    c("horaRealLlegada", "hora_real_llegada"),
    c("minutosRetrasoReal", "minutos_retraso_real"),
    c("monitoreoActivo", "monitoreo_activo"),
    c("notas", "notas"),
    c("justificacion", "justificacion"),
    c("fotosJustificacion", "fotos_justificacion"),
];

pub const MAP_INCIDENCIAS: &[Campo] = &[
    c("tipo", "tipo"),
    c("estado", "estado"),
    c("descripcion", "descripcion"),
    c("personalId", "personal_id"),
    c("vehiculoId", "vehiculo_id"),
    c("rutaId", "ruta_id"),
    c("resolucion", "resolucion"),
];

pub const MAP_TRACKING: &[Campo] = &[
    c("personalId", "personal_id"),
    c("latitud", "latitud"),
    c("longitud", "longitud"),
    c("precision", "precision"),
    c("velocidad", "velocidad"),
    c("rumbo", "rumbo"),
    c("timestamp", "registrado_en"),
];

pub const MAP_VISITAS: &[Campo] = &[
    c("personalId", "personal_id"),
    c("rutaId", "ruta_id"),
    c("tipoActividad", "tipo_actividad"),
    c("resultado", "resultado"),
    c("lat", "lat"),
    c("lng", "lng"),
    c("distanciaAlCliente", "distancia_al_cliente"),
    c("horaLlegada", "hora_llegada"),
    c("horaSalida", "hora_salida"),
    c("duracionMinutos", "duracion_minutos"),
    c("notas", "notas"),
    c("parteId", "parte_id"),
];

pub const MAP_UBICACION: &[Campo] = &[
    c("lat", "ubicacion_lat"),
    c("lng", "ubicacion_lng"),
    c("precision", "ubicacion_precision"),
    c("timestamp", "ubicacion_ts"),
];

pub const TABLA_PERSONAL: Tabla = Tabla {
    nombre: "personal",
    campos: MAP_PERSONAL,
};
pub const TABLA_CLIENTES: Tabla = Tabla {
    nombre: "clientes",
    campos: MAP_CLIENTES,
};
pub const TABLA_VEHICULOS: Tabla = Tabla {
    nombre: "vehiculos",
    campos: MAP_VEHICULOS,
};
pub const TABLA_RUTAS: Tabla = Tabla {
    nombre: "rutas",
    campos: MAP_RUTAS,
};
pub const TABLA_PARADAS: Tabla = Tabla {
    nombre: "paradas",
    campos: MAP_PARADAS,
};
pub const TABLA_PEDIDOS: Tabla = Tabla {
    nombre: "pedidos",
    campos: MAP_PEDIDOS,
};
pub const TABLA_ASISTENCIA: Tabla = Tabla {
    nombre: "asistencia",
    campos: MAP_ASISTENCIA,
};
pub const TABLA_INCIDENCIAS: Tabla = Tabla {
    nombre: "incidencias",
    campos: MAP_INCIDENCIAS,
};
pub const TABLA_TRACKING: Tabla = Tabla {
    nombre: "tracking",
    campos: MAP_TRACKING,
};
pub const TABLA_VISITAS: Tabla = Tabla {
    nombre: "visitas",
    campos: MAP_VISITAS,
};

/// Defaults de columna que el TS aplica antes del INSERT.
fn defecto(tabla: &str, col: &str) -> Option<Value> {
    match (tabla, col) {
        ("personal", "cargo") => Some(json!("conductor")),
        ("personal", "estado") => Some(json!("activo")),
        ("clientes", "tipo_cliente") => Some(json!("particular")),
        ("vehiculos", "tipo") => Some(json!("camioneta")),
        ("vehiculos", "estado") => Some(json!("disponible")),
        ("rutas", "estado") => Some(json!("pendiente")),
        ("rutas", "personal_asignado_ids") => Some(json!([])),
        ("paradas", "tipo") => Some(json!("cliente")),
        ("paradas", "completada") => Some(json!(false)),
        ("pedidos", "estado") => Some(json!("pendiente")),
        ("incidencias", "estado") => Some(json!("abierta")),
        ("visitas", "resultado") => Some(json!("no_visitado")),
        _ => None,
    }
}

fn tabla_sync(tipo: &str) -> Option<&'static Tabla> {
    match tipo {
        "personal" => Some(&TABLA_PERSONAL),
        "cliente" => Some(&TABLA_CLIENTES),
        "vehiculo" => Some(&TABLA_VEHICULOS),
        "ruta" => Some(&TABLA_RUTAS),
        "parada" => Some(&TABLA_PARADAS),
        "pedido" => Some(&TABLA_PEDIDOS),
        "asistencia" => Some(&TABLA_ASISTENCIA),
        "incidencia" => Some(&TABLA_INCIDENCIAS),
        "visita" => Some(&TABLA_VISITAS),
        "tracking" => Some(&TABLA_TRACKING),
        _ => None,
    }
}

struct Filtro {
    expresion: &'static str,
    operador: &'static str,
    tipo: TipoColumna,
    valor: Valor,
}

impl Filtro {
    fn texto(expresion: &'static str, valor: impl Into<String>) -> Self {
        Self {
            expresion,
            operador: "=",
            tipo: TipoColumna::Texto,
            valor: Valor::Texto(valor.into()),
        }
    }

    fn uuid(expresion: &'static str, valor: impl Into<String>) -> Self {
        Self {
            expresion,
            operador: "=",
            tipo: TipoColumna::Uuid,
            valor: Valor::Texto(valor.into()),
        }
    }

    fn rango(expresion: &'static str, operador: &'static str, valor: impl Into<String>) -> Self {
        Self {
            expresion,
            operador,
            tipo: TipoColumna::Timestamptz,
            valor: Valor::Texto(valor.into()),
        }
    }

    fn fecha(expresion: &'static str, valor: impl Into<String>) -> Self {
        Self {
            expresion,
            operador: "=",
            tipo: TipoColumna::Fecha,
            valor: Valor::Texto(valor.into()),
        }
    }
}

pub struct FieldService {
    db: Arc<Db>,
    partes: Arc<dyn Partes>,
}

impl FieldService {
    pub fn nuevo(db: Arc<Db>, partes: Arc<dyn Partes>) -> Self {
        Self { db, partes }
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }
}

mod asistencia;
mod clientes;
mod generico;
mod incidencias;
mod operacion;
mod pedidos;
mod personal;
mod rutas;
mod vehiculos;

pub(super) fn ordenar_paradas(paradas: &mut [Value]) {
    paradas.sort_by_key(|parada| parada.get("orden").and_then(Value::as_i64).unwrap_or(0));
}
