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

    fn interno(_: sqlx::Error) -> ApiError {
        ApiError::interno()
    }

    async fn ejecutar_filas(&self, sql: &str, valores: Vec<Valor>) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        let filas = consulta(sql, &valores)
            .fetch_all(&mut *tx)
            .await
            .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        filas
            .iter()
            .map(fila_a_json)
            .collect::<Result<_, _>>()
            .map_err(Self::interno)
    }

    async fn ejecutar_uno(
        &self,
        sql: &str,
        valores: Vec<Valor>,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        let fila = consulta(sql, &valores)
            .fetch_optional(&mut *tx)
            .await
            .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map(Some).map_err(Self::interno),
            None => Ok(None),
        }
    }

    async fn validar_parte(
        &self,
        tenant: &str,
        tabla: &Tabla,
        cuerpo: &Value,
    ) -> Result<(), ApiError> {
        if !tabla.campos.iter().any(|campo| campo.col == "parte_id") {
            return Ok(());
        }
        let Some(objeto) = cuerpo.as_object() else {
            return Ok(());
        };
        let Some(valor) = objeto.get("parteId") else {
            return Ok(());
        };
        let Some(parte_id) = valor.as_str() else {
            return Err(ApiError::parte_invalida(
                "La parte debe ser un UUID valido.",
            ));
        };
        match self.partes.pertenece(tenant, parte_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(ApiError::parte_invalida(
                "La parte no pertenece al tenant activo.",
            )),
            Err(()) => Err(ApiError::servicio_no_disponible("identity-service")),
        }
    }

    async fn insertar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: Option<&Value>,
        cuerpo: &Value,
        upsert: bool,
    ) -> Result<Value, ApiError> {
        self.validar_parte(tenant, tabla, cuerpo).await?;

        let objeto = cuerpo.as_object();
        let mut columnas: Vec<(&'static str, TipoColumna, Valor)> = Vec::new();
        for campo in tabla.campos {
            let valor = objeto
                .and_then(|objeto| objeto.get(campo.json))
                .cloned()
                .or_else(|| defecto(tabla.nombre, campo.col));
            if let Some(valor) = valor {
                let tipo = tipo_columna(campo.col);
                columnas.push((campo.col, tipo, Valor::desde_json(&valor, tipo)));
            }
        }

        let id_texto = match id {
            Some(Value::String(texto)) => texto.clone(),
            Some(valor) => valor.to_string(),
            None => Uuid::new_v4().to_string(),
        };

        let mut sql = String::from("INSERT INTO field.");
        sql.push_str(tabla.nombre);
        sql.push_str(" (tenant_id, id");
        for (columna, _, _) in &columnas {
            sql.push_str(", ");
            sql.push_str(columna);
        }
        sql.push_str(", synced, created_at, updated_at) VALUES ($1::uuid, $2::uuid");
        for (indice, (_, tipo, _)) in columnas.iter().enumerate() {
            sql.push_str(&format!(", ${}{}", indice + 3, cast(*tipo)));
        }
        sql.push_str(", TRUE, NOW(), NOW())");
        if upsert {
            sql.push_str(" ON CONFLICT (id) DO UPDATE SET ");
            let sets = tabla
                .campos
                .iter()
                .map(|campo| format!("{} = EXCLUDED.{}", campo.col, campo.col))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&sets);
            sql.push_str(", updated_at = NOW(), deleted_at = NULL");
        }
        sql.push_str(" RETURNING *");

        let mut valores = vec![Valor::Texto(tenant.to_string()), Valor::Texto(id_texto)];
        valores.extend(columnas.into_iter().map(|(_, _, valor)| valor));
        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::persistencia_fallida("No se pudo guardar el registro."))
    }

    async fn actualizar_con_map(
        &self,
        tabla: &Tabla,
        campos: &[Campo],
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte(tenant, tabla, cuerpo).await?;
        let objeto = cuerpo.as_object();
        let mut sets: Vec<(String, TipoColumna, Valor)> = Vec::new();
        for campo in campos {
            let Some(valor) = objeto.and_then(|objeto| objeto.get(campo.json)) else {
                continue;
            };
            let tipo = tipo_columna(campo.col);
            sets.push((
                format!("{} = ${}{}", campo.col, sets.len() + 1, cast(tipo)),
                tipo,
                Valor::desde_json(valor, tipo),
            ));
        }
        if sets.is_empty() {
            return self.obtener(tabla, tenant, id).await;
        }

        let indice_tenant = sets.len() + 1;
        let indice_id = sets.len() + 2;
        let mut sql = String::from("UPDATE field.");
        sql.push_str(tabla.nombre);
        sql.push_str(" SET ");
        sql.push_str(
            &sets
                .iter()
                .map(|(set, _, _)| set.clone())
                .collect::<Vec<_>>()
                .join(", "),
        );
        sql.push_str(&format!(
            ", updated_at = NOW() WHERE tenant_id = ${indice_tenant}::uuid AND id = ${indice_id}::uuid AND deleted_at IS NULL RETURNING *"
        ));

        let mut valores = sets
            .into_iter()
            .map(|(_, _, valor)| valor)
            .collect::<Vec<_>>();
        valores.push(Valor::Texto(tenant.to_string()));
        valores.push(Valor::Texto(id.to_string()));

        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::no_encontrado(tabla.nombre, id))
    }

    async fn actualizar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar_con_map(tabla, tabla.campos, tenant, id, cuerpo)
            .await
    }

    async fn obtener(&self, tabla: &Tabla, tenant: &str, id: &str) -> Result<Value, ApiError> {
        let sql = format!(
            "SELECT * FROM field.{} WHERE tenant_id = $1::uuid AND id = $2::uuid AND deleted_at IS NULL",
            tabla.nombre
        );
        self.ejecutar_uno(
            &sql,
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(id.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| ApiError::no_encontrado(tabla.nombre, id))
    }

    async fn eliminar(&self, tabla: &Tabla, tenant: &str, id: &str) -> Result<(), ApiError> {
        let sql = format!(
            "UPDATE field.{} SET deleted_at = NOW() WHERE tenant_id = $1::uuid AND id = $2::uuid",
            tabla.nombre
        );
        let mut tx = self.db.transaccion().await.map_err(Self::interno)?;
        consulta(
            &sql,
            &[
                Valor::Texto(tenant.to_string()),
                Valor::Texto(id.to_string()),
            ],
        )
        .execute(&mut *tx)
        .await
        .map_err(Self::interno)?;
        tx.commit().await.map_err(Self::interno)?;
        Ok(())
    }

    async fn listar(
        &self,
        tabla: &Tabla,
        tenant: &str,
        filtros: Vec<Filtro>,
        orden: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let mut sql = format!(
            "SELECT * FROM field.{} WHERE tenant_id = $1::uuid AND deleted_at IS NULL",
            tabla.nombre
        );
        let mut valores = vec![Valor::Texto(tenant.to_string())];
        for filtro in filtros {
            let indice = valores.len() + 1;
            sql.push_str(&format!(
                " AND {} {} ${}{}",
                filtro.expresion,
                filtro.operador,
                indice,
                cast(filtro.tipo)
            ));
            valores.push(filtro.valor);
        }
        sql.push_str(&format!(" ORDER BY {orden} DESC LIMIT 2000"));
        self.ejecutar_filas(&sql, valores).await
    }

    // ── personal ──────────────────────────────────────────────────────────
    pub async fn listar_personal(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_PERSONAL, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_personal(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PERSONAL, tenant, id).await
    }

    pub async fn crear_personal(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_PERSONAL, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_personal(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_PERSONAL, tenant, id, cuerpo).await
    }

    pub async fn eliminar_personal(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_PERSONAL, tenant, id).await
    }

    pub async fn ubicacion_personal(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        let fila = self.obtener(&TABLA_PERSONAL, tenant, id).await?;
        Ok(json!({
            "personalId": id,
            "latitud": fila.get("ubicacion_lat").cloned().unwrap_or(Value::Null),
            "longitud": fila.get("ubicacion_lng").cloned().unwrap_or(Value::Null),
            "precision": fila.get("ubicacion_precision").cloned().unwrap_or(Value::Null),
            "timestamp": fila.get("ubicacion_ts").cloned().unwrap_or(Value::Null),
        }))
    }

    pub async fn actualizar_ubicacion(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PERSONAL, tenant, id).await?;
        self.actualizar_con_map(&TABLA_PERSONAL, MAP_UBICACION, tenant, id, cuerpo)
            .await
    }

    // ── clientes ──────────────────────────────────────────────────────────
    pub async fn listar_clientes(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_CLIENTES, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_cliente(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_CLIENTES, tenant, id).await
    }

    pub async fn crear_cliente(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_CLIENTES, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_cliente(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_CLIENTES, tenant, id, cuerpo).await
    }

    pub async fn eliminar_cliente(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_CLIENTES, tenant, id).await
    }

    // ── vehículos ─────────────────────────────────────────────────────────
    pub async fn listar_vehiculos(&self, tenant: &str) -> Result<Vec<Value>, ApiError> {
        self.listar(&TABLA_VEHICULOS, tenant, Vec::new(), "created_at")
            .await
    }

    pub async fn obtener_vehiculo(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_VEHICULOS, tenant, id).await
    }

    pub async fn crear_vehiculo(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_VEHICULOS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_vehiculo(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_VEHICULOS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_vehiculo(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_VEHICULOS, tenant, id).await
    }

    // ── rutas ─────────────────────────────────────────────────────────────
    pub async fn listar_rutas(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut rutas = match personal_id {
            Some(personal_id) => {
                let sql = "SELECT * FROM field.rutas WHERE tenant_id = $1::uuid AND personal_asignado_ids ?| array[$2] AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 2000";
                self.ejecutar_filas(
                    sql,
                    vec![
                        Valor::Texto(tenant.to_string()),
                        Valor::Texto(personal_id.to_string()),
                    ],
                )
                .await?
            }
            None => {
                self.listar(&TABLA_RUTAS, tenant, Vec::new(), "created_at")
                    .await?
            }
        };

        let paradas = self
            .ejecutar_filas(
                "SELECT * FROM field.paradas WHERE tenant_id = $1::uuid AND deleted_at IS NULL",
                vec![Valor::Texto(tenant.to_string())],
            )
            .await?;
        let mut por_ruta: HashMap<String, Vec<Value>> = HashMap::new();
        for parada in paradas {
            if let Some(ruta_id) = parada.get("ruta_id").and_then(Value::as_str) {
                por_ruta
                    .entry(ruta_id.to_string())
                    .or_default()
                    .push(parada);
            }
        }
        for ruta in &mut rutas {
            let id = ruta
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let mut paradas = por_ruta.remove(&id).unwrap_or_default();
            ordenar_paradas(&mut paradas);
            if let Some(objeto) = ruta.as_object_mut() {
                objeto.insert("paradas".into(), Value::Array(paradas));
            }
        }
        Ok(rutas)
    }

    pub async fn obtener_ruta(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        let mut ruta = self.obtener(&TABLA_RUTAS, tenant, id).await?;
        let mut paradas = self
            .ejecutar_filas(
                "SELECT * FROM field.paradas WHERE tenant_id = $1::uuid AND ruta_id = $2::uuid AND deleted_at IS NULL",
                vec![Valor::Texto(tenant.to_string()), Valor::Texto(id.to_string())],
            )
            .await?;
        ordenar_paradas(&mut paradas);
        if let Some(objeto) = ruta.as_object_mut() {
            objeto.insert("paradas".into(), Value::Array(paradas));
        }
        Ok(ruta)
    }

    pub async fn crear_ruta(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_RUTAS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_ruta(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_RUTAS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_ruta(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_RUTAS, tenant, id).await
    }

    pub async fn agregar_parada(
        &self,
        tenant: &str,
        ruta_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.obtener(&TABLA_RUTAS, tenant, ruta_id).await?;
        let mut objeto = cuerpo.as_object().cloned().unwrap_or_default();
        objeto.insert("rutaId".into(), Value::String(ruta_id.to_string()));
        self.insertar(&TABLA_PARADAS, tenant, None, &Value::Object(objeto), false)
            .await
    }

    pub async fn actualizar_parada(
        &self,
        tenant: &str,
        ruta_id: &str,
        parada_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let parada = self
            .ejecutar_uno(
                "SELECT id, ruta_id FROM field.paradas WHERE tenant_id = $1::uuid AND id = $2::uuid AND deleted_at IS NULL",
                vec![
                    Valor::Texto(tenant.to_string()),
                    Valor::Texto(parada_id.to_string()),
                ],
            )
            .await?
            .ok_or_else(|| ApiError::no_encontrado("parada", parada_id))?;
        let ruta_actual = parada
            .get("ruta_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if ruta_actual != ruta_id {
            return Err(ApiError::parada_invalida(
                "La parada no pertenece a la ruta indicada.",
            ));
        }
        self.actualizar(&TABLA_PARADAS, tenant, parada_id, cuerpo)
            .await
    }

    pub async fn asignar_ruta(
        &self,
        tenant: &str,
        ruta_id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let objeto = cuerpo.as_object();
        let mut sets: Vec<(String, TipoColumna, Valor)> = Vec::new();
        if let Some(valor) = objeto.and_then(|objeto| objeto.get("personalIds")) {
            sets.push((
                format!(
                    "personal_asignado_ids = ${}{}",
                    sets.len() + 1,
                    cast(TipoColumna::Jsonb)
                ),
                TipoColumna::Jsonb,
                Valor::desde_json(valor, TipoColumna::Jsonb),
            ));
        }
        if let Some(valor) = objeto.and_then(|objeto| objeto.get("vehiculoId")) {
            sets.push((
                format!(
                    "vehiculo_asignado_id = ${}{}",
                    sets.len() + 1,
                    cast(TipoColumna::Uuid)
                ),
                TipoColumna::Uuid,
                Valor::desde_json(valor, TipoColumna::Uuid),
            ));
        }
        let indice_tenant = sets.len() + 1;
        let indice_id = sets.len() + 2;
        let mut sql = String::from("UPDATE field.rutas SET ");
        if !sets.is_empty() {
            sql.push_str(
                &sets
                    .iter()
                    .map(|(set, _, _)| set.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "updated_at = NOW() WHERE tenant_id = ${indice_tenant}::uuid AND id = ${indice_id}::uuid RETURNING *"
        ));
        let mut valores = sets
            .into_iter()
            .map(|(_, _, valor)| valor)
            .collect::<Vec<_>>();
        valores.push(Valor::Texto(tenant.to_string()));
        valores.push(Valor::Texto(ruta_id.to_string()));
        self.ejecutar_uno(&sql, valores)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("ruta", ruta_id))
    }

    // ── pedidos ───────────────────────────────────────────────────────────
    pub async fn listar_pedidos(
        &self,
        tenant: &str,
        estado: Option<&str>,
        ruta_id: Option<&str>,
        parte_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(estado) = estado {
            filtros.push(Filtro::texto("estado", estado));
        }
        if let Some(ruta_id) = ruta_id {
            filtros.push(Filtro::uuid("ruta_id", ruta_id));
        }
        if let Some(parte_id) = parte_id {
            filtros.push(Filtro::uuid("parte_id", parte_id));
        }
        self.listar(&TABLA_PEDIDOS, tenant, filtros, "created_at")
            .await
    }

    pub async fn obtener_pedido(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_PEDIDOS, tenant, id).await
    }

    pub async fn crear_pedido(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_PEDIDOS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_pedido(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_PEDIDOS, tenant, id, cuerpo).await
    }

    pub async fn eliminar_pedido(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_PEDIDOS, tenant, id).await
    }

    pub async fn cambiar_estado_pedido(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let estado = cuerpo.get("estado").cloned().unwrap_or(Value::Null);
        self.actualizar(&TABLA_PEDIDOS, tenant, id, &json!({ "estado": estado }))
            .await
    }

    // ── asistencia ────────────────────────────────────────────────────────
    pub async fn listar_asistencia(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
        fecha: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(personal_id) = personal_id {
            filtros.push(Filtro::uuid("personal_id", personal_id));
        }
        if let Some(fecha) = fecha {
            filtros.push(Filtro::fecha(
                "DATE(registrado_en AT TIME ZONE 'America/Managua')",
                fecha,
            ));
        }
        self.listar(&TABLA_ASISTENCIA, tenant, filtros, "created_at")
            .await
    }

    pub async fn registrar_asistencia(
        &self,
        tenant: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.insertar(&TABLA_ASISTENCIA, tenant, None, cuerpo, false)
            .await
    }

    // ── incidencias ───────────────────────────────────────────────────────
    pub async fn listar_incidencias(
        &self,
        tenant: &str,
        estado: Option<&str>,
        ruta_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(estado) = estado {
            filtros.push(Filtro::texto("estado", estado));
        }
        if let Some(ruta_id) = ruta_id {
            filtros.push(Filtro::uuid("ruta_id", ruta_id));
        }
        self.listar(&TABLA_INCIDENCIAS, tenant, filtros, "created_at")
            .await
    }

    pub async fn obtener_incidencia(&self, tenant: &str, id: &str) -> Result<Value, ApiError> {
        self.obtener(&TABLA_INCIDENCIAS, tenant, id).await
    }

    pub async fn crear_incidencia(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_INCIDENCIAS, tenant, None, cuerpo, false)
            .await
    }

    pub async fn actualizar_incidencia(
        &self,
        tenant: &str,
        id: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        self.actualizar(&TABLA_INCIDENCIAS, tenant, id, cuerpo)
            .await
    }

    pub async fn eliminar_incidencia(&self, tenant: &str, id: &str) -> Result<(), ApiError> {
        self.eliminar(&TABLA_INCIDENCIAS, tenant, id).await
    }

    // ── tracking ──────────────────────────────────────────────────────────
    pub async fn listar_tracking(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(personal_id) = personal_id {
            filtros.push(Filtro::uuid("personal_id", personal_id));
        }
        if let Some(desde) = desde {
            filtros.push(Filtro::rango("registrado_en", ">=", desde));
        }
        if let Some(hasta) = hasta {
            filtros.push(Filtro::rango("registrado_en", "<=", hasta));
        }
        self.listar(&TABLA_TRACKING, tenant, filtros, "registrado_en")
            .await
    }

    pub async fn registrar_tracking(
        &self,
        tenant: &str,
        registros: &[Value],
    ) -> Result<Vec<Value>, ApiError> {
        let mut resultados = Vec::with_capacity(registros.len());
        for registro in registros {
            resultados.push(
                self.insertar(&TABLA_TRACKING, tenant, None, registro, false)
                    .await?,
            );
        }
        Ok(resultados)
    }

    // ── visitas ───────────────────────────────────────────────────────────
    pub async fn listar_visitas(
        &self,
        tenant: &str,
        personal_id: Option<&str>,
        fecha: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut filtros = Vec::new();
        if let Some(personal_id) = personal_id {
            filtros.push(Filtro::uuid("personal_id", personal_id));
        }
        if let Some(fecha) = fecha {
            filtros.push(Filtro::fecha(
                "DATE(created_at AT TIME ZONE 'America/Managua')",
                fecha,
            ));
        }
        self.listar(&TABLA_VISITAS, tenant, filtros, "created_at")
            .await
    }

    pub async fn registrar_visita(&self, tenant: &str, cuerpo: &Value) -> Result<Value, ApiError> {
        self.insertar(&TABLA_VISITAS, tenant, None, cuerpo, false)
            .await
    }

    // ── cumplimiento ──────────────────────────────────────────────────────
    pub async fn obtener_cumplimiento(
        &self,
        tenant: &str,
        ruta_id: &str,
        fecha: &str,
    ) -> Result<Value, ApiError> {
        self.ejecutar_uno(
            "SELECT * FROM field.cumplimiento WHERE tenant_id = $1::uuid AND ruta_id = $2::uuid AND fecha = $3::date",
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(ruta_id.to_string()),
                Valor::Texto(fecha.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| ApiError::no_encontrado("cumplimiento", &format!("{ruta_id}/{fecha}")))
    }

    pub async fn guardar_cumplimiento(
        &self,
        tenant: &str,
        cuerpo: &Value,
    ) -> Result<Value, ApiError> {
        let ruta_id = cuerpo
            .get("rutaId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let fecha = cuerpo
            .get("fecha")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let metricas = cuerpo.get("metricas").cloned().unwrap_or_else(|| json!({}));
        self.ejecutar_uno(
            "INSERT INTO field.cumplimiento (tenant_id, ruta_id, fecha, metricas, updated_at) VALUES ($1::uuid, $2::uuid, $3::date, $4::jsonb, NOW()) ON CONFLICT (tenant_id, ruta_id, fecha) DO UPDATE SET metricas = $4::jsonb, updated_at = NOW() RETURNING *",
            vec![
                Valor::Texto(tenant.to_string()),
                Valor::Texto(ruta_id),
                Valor::Texto(fecha),
                Valor::Texto(metricas.to_string()),
            ],
        )
        .await?
        .ok_or_else(|| {
            ApiError::persistencia_fallida("No se pudo guardar el cumplimiento.")
        })
    }

    // ── sync offline ──────────────────────────────────────────────────────
    pub async fn sincronizar(&self, tenant: &str, operaciones: &[Value]) -> Vec<Value> {
        let mut resultados = Vec::with_capacity(operaciones.len());
        for operacion in operaciones {
            let tipo_valor = operacion.get("tipo").cloned();
            let tipo = tipo_valor.as_ref().and_then(Value::as_str);
            let payload = operacion.get("payload").cloned().unwrap_or(Value::Null);
            let id = payload.get("id").cloned();

            let Some(tipo) = tipo else {
                let mut resultado = Map::new();
                if let Some(tipo) = tipo_valor {
                    if !tipo.is_null() {
                        resultado.insert("tipo".into(), tipo);
                    }
                }
                if let Some(id) = id {
                    resultado.insert("id".into(), id);
                }
                resultado.insert("ok".into(), Value::Bool(false));
                resultado.insert(
                    "error".into(),
                    Value::String("tipo de entidad no soportado".into()),
                );
                resultados.push(Value::Object(resultado));
                continue;
            };
            let Some(tabla) = tabla_sync(tipo) else {
                let mut resultado = Map::new();
                resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                if let Some(id) = id {
                    resultado.insert("id".into(), id);
                }
                resultado.insert("ok".into(), Value::Bool(false));
                resultado.insert(
                    "error".into(),
                    Value::String("tipo de entidad no soportado".into()),
                );
                resultados.push(Value::Object(resultado));
                continue;
            };

            match self
                .insertar(tabla, tenant, id.as_ref(), &payload, true)
                .await
            {
                Ok(fila) => {
                    let mut resultado = Map::new();
                    resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                    resultado.insert("id".into(), fila.get("id").cloned().unwrap_or(Value::Null));
                    resultado.insert("ok".into(), Value::Bool(true));
                    resultados.push(Value::Object(resultado));
                }
                Err(error) => {
                    let mut resultado = Map::new();
                    resultado.insert("tipo".into(), Value::String(tipo.to_string()));
                    if let Some(id) = id {
                        resultado.insert("id".into(), id);
                    }
                    resultado.insert("ok".into(), Value::Bool(false));
                    resultado.insert("error".into(), Value::String(error.mensaje));
                    resultados.push(Value::Object(resultado));
                }
            }
        }
        crate::logs::info(
            "field",
            crate::campos! {
                "msg": "sync completado",
                "tenant": tenant,
                "operaciones": operaciones.len(),
            },
        );
        resultados
    }
}

fn ordenar_paradas(paradas: &mut [Value]) {
    paradas.sort_by_key(|parada| parada.get("orden").and_then(Value::as_i64).unwrap_or(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_aplicar_los_defaults_del_ts() {
        assert_eq!(defecto("personal", "cargo"), Some(json!("conductor")));
        assert_eq!(defecto("rutas", "personal_asignado_ids"), Some(json!([])));
        assert_eq!(defecto("paradas", "completada"), Some(json!(false)));
        assert_eq!(defecto("clientes", "nombre_completo"), None);
    }

    #[test]
    fn debe_resolver_las_tablas_de_sync() {
        assert_eq!(tabla_sync("cliente").expect("cliente").nombre, "clientes");
        assert_eq!(tabla_sync("tracking").expect("tracking").nombre, "tracking");
        assert!(tabla_sync("desconocido").is_none());
    }

    #[test]
    fn debe_ordenar_las_paradas_por_orden() {
        let mut paradas = vec![
            json!({"id": "p2", "orden": 2}),
            json!({"id": "p1", "orden": 1}),
            json!({"id": "p3"}),
        ];
        ordenar_paradas(&mut paradas);
        assert_eq!(paradas[0]["id"], "p3");
        assert_eq!(paradas[1]["id"], "p1");
        assert_eq!(paradas[2]["id"], "p2");
    }
}
