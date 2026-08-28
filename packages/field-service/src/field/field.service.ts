import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService, DomainError, NotFoundError, Logger } from '@core/shared';
import {
  CrearPersonalDto,
  ActualizarPersonalDto,
  CrearClienteDto,
  ActualizarClienteDto,
  CrearVehiculoDto,
  ActualizarVehiculoDto,
  CrearRutaDto,
  ActualizarRutaDto,
  ParadaDto,
  AsignarRutaDto,
  CrearPedidoDto,
  ActualizarPedidoDto,
  CambiarEstadoPedidoDto,
  CrearAsistenciaDto,
  CrearIncidenciaDto,
  ActualizarIncidenciaDto,
  CrearTrackingDto,
  CrearVisitaDto,
  GuardarCumplimientoDto,
  UbicacionDto,
  SyncOperacionDto,
} from './field.dtos';

type Mapa = Record<string, string>;

const MAP_PERSONAL: Mapa = {
  nombre: 'nombre',
  apellido: 'apellido',
  cargo: 'cargo',
  estado: 'estado',
  telefono: 'telefono',
  email: 'email',
  rutaAsignadaId: 'ruta_asignada_id',
  vehiculoAsignadoId: 'vehiculo_asignado_id',
  horaCheckIn: 'hora_check_in',
  horaCheckOut: 'hora_check_out',
  ubicacionLat: 'ubicacion_lat',
  ubicacionLng: 'ubicacion_lng',
  ubicacionPrecision: 'ubicacion_precision',
  ubicacionTs: 'ubicacion_ts',
};
const DEF_PERSONAL = { cargo: 'conductor', estado: 'activo' };

const MAP_CLIENTES: Mapa = {
  nombreCompleto: 'nombre_completo',
  tipoDocumento: 'tipo_documento',
  numeroDocumento: 'numero_documento',
  tipoCliente: 'tipo_cliente',
  email: 'email',
  telefono: 'telefono',
  telefonoAlternativo: 'telefono_alternativo',
  direccionPrincipal: 'direccion_principal',
  direccionSecundaria: 'direccion_secundaria',
  referenciasDireccion: 'referencias_direccion',
  lat: 'lat',
  lng: 'lng',
  notasAdicionales: 'notas_adicionales',
  urlGoogleMaps: 'url_google_maps',
};
const DEF_CLIENTES = { tipoCliente: 'particular' };

const MAP_VEHICULOS: Mapa = {
  placa: 'placa',
  tipo: 'tipo',
  marca: 'marca',
  modelo: 'modelo',
  anio: 'anio',
  estado: 'estado',
  color: 'color',
  capacidadCargaKg: 'capacidad_carga_kg',
  numeroChasis: 'numero_chasis',
  numeroMotor: 'numero_motor',
  tipoCombustible: 'tipo_combustible',
  vencimientoSeguro: 'vencimiento_seguro',
  vencimientoCirculacion: 'vencimiento_circulacion',
  notasAdicionales: 'notas_adicionales',
  conductorId: 'conductor_id',
  rutaActivaId: 'ruta_activa_id',
};
const DEF_VEHICULOS = { tipo: 'camioneta', estado: 'disponible' };

const MAP_RUTAS: Mapa = {
  nombre: 'nombre',
  descripcion: 'descripcion',
  estado: 'estado',
  personalAsignadoIds: 'personal_asignado_ids',
  vehiculoAsignadoId: 'vehiculo_asignado_id',
  fechaInicio: 'fecha_inicio',
  fechaFin: 'fecha_fin',
};
const DEF_RUTAS = { estado: 'pendiente', personalAsignadoIds: [] };

const MAP_PARADAS: Mapa = {
  rutaId: 'ruta_id',
  orden: 'orden',
  nombre: 'nombre',
  direccion: 'direccion',
  lat: 'lat',
  lng: 'lng',
  completada: 'completada',
  pedidoId: 'pedido_id',
  tipo: 'tipo',
  clienteId: 'cliente_id',
  telefono: 'telefono',
  referencias: 'referencias',
  horarioAtencion: 'horario_atencion',
  notas: 'notas',
  tipoCombustible: 'tipo_combustible',
};
const DEF_PARADAS = { tipo: 'cliente', completada: false };

const MAP_PEDIDOS: Mapa = {
  cliente: 'cliente',
  clienteId: 'cliente_id',
  direccionEntrega: 'direccion_entrega',
  lat: 'lat',
  lng: 'lng',
  estado: 'estado',
  rutaId: 'ruta_id',
  notas: 'notas',
};
const DEF_PEDIDOS = { estado: 'pendiente' };

const MAP_ASISTENCIA: Mapa = {
  personalId: 'personal_id',
  tipo: 'tipo',
  timestamp: 'registrado_en',
  lat: 'lat',
  lng: 'lng',
  estadoPuntualidad: 'estado_puntualidad',
  enSede: 'en_sede',
  horaRealLlegada: 'hora_real_llegada',
  minutosRetrasoReal: 'minutos_retraso_real',
  monitoreoActivo: 'monitoreo_activo',
  notas: 'notas',
  justificacion: 'justificacion',
  fotosJustificacion: 'fotos_justificacion',
};

const MAP_INCIDENCIAS: Mapa = {
  tipo: 'tipo',
  estado: 'estado',
  descripcion: 'descripcion',
  personalId: 'personal_id',
  vehiculoId: 'vehiculo_id',
  rutaId: 'ruta_id',
  resolucion: 'resolucion',
};
const DEF_INCIDENCIAS = { estado: 'abierta' };

const MAP_TRACKING: Mapa = {
  personalId: 'personal_id',
  latitud: 'latitud',
  longitud: 'longitud',
  precision: 'precision',
  velocidad: 'velocidad',
  rumbo: 'rumbo',
  timestamp: 'registrado_en',
};

const MAP_VISITAS: Mapa = {
  personalId: 'personal_id',
  clienteId: 'cliente_id',
  rutaId: 'ruta_id',
  tipoActividad: 'tipo_actividad',
  resultado: 'resultado',
  lat: 'lat',
  lng: 'lng',
  distanciaAlCliente: 'distancia_al_cliente',
  horaLlegada: 'hora_llegada',
  horaSalida: 'hora_salida',
  duracionMinutos: 'duracion_minutos',
  notas: 'notas',
};
const DEF_VISITAS = { resultado: 'no_visitado' };

const TABLA_SYNC: Record<string, { tabla: string; map: Mapa; def: Record<string, unknown> }> = {
  personal: { tabla: 'personal', map: MAP_PERSONAL, def: DEF_PERSONAL },
  cliente: { tabla: 'clientes', map: MAP_CLIENTES, def: DEF_CLIENTES },
  vehiculo: { tabla: 'vehiculos', map: MAP_VEHICULOS, def: DEF_VEHICULOS },
  ruta: { tabla: 'rutas', map: MAP_RUTAS, def: DEF_RUTAS },
  parada: { tabla: 'paradas', map: MAP_PARADAS, def: DEF_PARADAS },
  pedido: { tabla: 'pedidos', map: MAP_PEDIDOS, def: DEF_PEDIDOS },
  asistencia: { tabla: 'asistencia', map: MAP_ASISTENCIA, def: {} },
  incidencia: { tabla: 'incidencias', map: MAP_INCIDENCIAS, def: DEF_INCIDENCIAS },
  visita: { tabla: 'visitas', map: MAP_VISITAS, def: DEF_VISITAS },
  tracking: { tabla: 'tracking', map: MAP_TRACKING, def: {} },
};

/**
 * field-service: dominio de logistica de campo (app-test). Multi-tenancy por
 * tenant_id en todas las tablas del esquema `field.*`.
 */
@Injectable()
export class FieldService {
  private readonly logger = Logger.create('field');

  constructor(private readonly pg: PgService) {}

  // ── helpers genericos ──────────────────────────────────────────────────
  private async insertar<T>(
    tabla: string,
    tenant: string,
    id: string | undefined,
    dto: object,
    map: Mapa,
    def: Record<string, unknown>,
    upsert: boolean,
  ): Promise<T> {
    const rec = dto as Record<string, unknown>;
    const rid = id ?? randomUUID();
    const now = new Date();
    const cols = ['tenant_id', 'id'];
    const vals: unknown[] = [tenant, rid];
    for (const [k, col] of Object.entries(map)) {
      let v = rec[k] !== undefined ? rec[k] : def[k];
      // Omitir campos no provistos para que aplique el DEFAULT de la columna
      // (insertar NULL anularía restricciones NOT NULL con default, p.ej. apellido).
      if (v === undefined) continue;
      if (v && typeof v === 'object') v = JSON.stringify(v);
      cols.push(col);
      vals.push(v);
    }
    cols.push('synced', 'created_at', 'updated_at');
    vals.push(true, now, now);
    const ph = vals.map((_, i) => `$${i + 1}`).join(', ');
    let sql = `INSERT INTO field.${tabla} (${cols.join(', ')}) VALUES (${ph})`;
    if (upsert) {
      const sets = Object.values(map).map((c) => `${c} = EXCLUDED.${c}`);
      sets.push('updated_at = NOW()');
      sql += ` ON CONFLICT (id) DO UPDATE SET ${sets.join(', ')} RETURNING *`;
    } else {
      sql += ' RETURNING *';
    }
    const fila = await this.pg.queryOne<T>(sql, vals);
    if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo guardar el registro.');
    return fila;
  }

  private async actualizar<T>(
    tabla: string,
    tenant: string,
    id: string,
    dto: object,
    map: Mapa,
  ): Promise<T> {
    const rec = dto as Record<string, unknown>;
    const sets: string[] = [];
    const vals: unknown[] = [];
    let i = 1;
    for (const [k, col] of Object.entries(map)) {
      if (rec[k] !== undefined) {
        let v = rec[k];
        if (v && typeof v === 'object') v = JSON.stringify(v);
        sets.push(`${col} = $${i}`);
        vals.push(v);
        i++;
      }
    }
    if (sets.length === 0) return this.obtener<T>(tabla, tenant, id);
    sets.push(`updated_at = $${i}`);
    vals.push(new Date());
    vals.push(tenant, id);
    const sql = `UPDATE field.${tabla} SET ${sets.join(', ')} WHERE tenant_id = $${i + 1} AND id = $${i + 2} RETURNING *`;
    const fila = await this.pg.queryOne<T>(sql, vals);
    if (!fila) throw new NotFoundError(tabla, id);
    return fila;
  }

  private async obtener<T>(tabla: string, tenant: string, id: string): Promise<T> {
    const fila = await this.pg.queryOne<T>(
      `SELECT * FROM field.${tabla} WHERE tenant_id = $1 AND id = $2`,
      [tenant, id],
    );
    if (!fila) throw new NotFoundError(tabla, id);
    return fila;
  }

  private async listar<T>(
    tabla: string,
    tenant: string,
    extras: { col: string; val: unknown }[] = [],
    orderBy = 'created_at',
  ): Promise<T[]> {
    const cond = ['tenant_id = $1', ...extras.map((_, i) => `${extras[i].col} = $${i + 2}`)];
    const sql = `SELECT * FROM field.${tabla} WHERE ${cond.join(' AND ')} ORDER BY ${orderBy} DESC LIMIT 2000`;
    return this.pg.query<T>(sql, [tenant, ...extras.map((e) => e.val)]);
  }

  private async eliminar(tabla: string, tenant: string, id: string): Promise<void> {
    await this.pg.query(`DELETE FROM field.${tabla} WHERE tenant_id = $1 AND id = $2`, [tenant, id]);
  }

  private requerirTenant(tenant?: string): string {
    if (!tenant) throw new DomainError('TENANT_REQUERIDO', 'La operacion requiere un tenant valido (x-tenant).');
    return tenant;
  }

  // ── personal ───────────────────────────────────────────────────────────
  listarPersonal(tenant: string) {
    return this.listar('personal', tenant);
  }
  obtenerPersonal(tenant: string, id: string) {
    return this.obtener('personal', tenant, id);
  }
  crearPersonal(tenant: string, dto: CrearPersonalDto) {
    return this.insertar('personal', tenant, undefined, dto, MAP_PERSONAL, DEF_PERSONAL, false);
  }
  actualizarPersonal(tenant: string, id: string, dto: ActualizarPersonalDto) {
    return this.actualizar('personal', tenant, id, dto, MAP_PERSONAL);
  }
  eliminarPersonal(tenant: string, id: string) {
    return this.eliminar('personal', tenant, id);
  }
  async ubicacionPersonal(tenant: string, id: string) {
    const p = await this.obtener<{ ubicacion_lat: number; ubicacion_lng: number; ubicacion_precision: number; ubicacion_ts: string }>(
      'personal',
      tenant,
      id,
    );
    return {
      personalId: id,
      latitud: p.ubicacion_lat,
      longitud: p.ubicacion_lng,
      precision: p.ubicacion_precision,
      timestamp: p.ubicacion_ts,
    };
  }
  async actualizarUbicacion(tenant: string, id: string, dto: UbicacionDto) {
    await this.obtener('personal', tenant, id);
    return this.actualizar('personal', tenant, id, dto, {
      ubicacionLat: 'ubicacion_lat',
      ubicacionLng: 'ubicacion_lng',
      ubicacionPrecision: 'ubicacion_precision',
      ubicacionTs: 'ubicacion_ts',
    });
  }

  // ── clientes ───────────────────────────────────────────────────────────
  listarClientes(tenant: string) {
    return this.listar('clientes', tenant);
  }
  obtenerCliente(tenant: string, id: string) {
    return this.obtener('clientes', tenant, id);
  }
  crearCliente(tenant: string, dto: CrearClienteDto) {
    return this.insertar('clientes', tenant, undefined, dto, MAP_CLIENTES, DEF_CLIENTES, false);
  }
  actualizarCliente(tenant: string, id: string, dto: ActualizarClienteDto) {
    return this.actualizar('clientes', tenant, id, dto, MAP_CLIENTES);
  }
  eliminarCliente(tenant: string, id: string) {
    return this.eliminar('clientes', tenant, id);
  }

  // ── vehiculos ──────────────────────────────────────────────────────────
  listarVehiculos(tenant: string) {
    return this.listar('vehiculos', tenant);
  }
  obtenerVehiculo(tenant: string, id: string) {
    return this.obtener('vehiculos', tenant, id);
  }
  crearVehiculo(tenant: string, dto: CrearVehiculoDto) {
    return this.insertar('vehiculos', tenant, undefined, dto, MAP_VEHICULOS, DEF_VEHICULOS, false);
  }
  actualizarVehiculo(tenant: string, id: string, dto: ActualizarVehiculoDto) {
    return this.actualizar('vehiculos', tenant, id, dto, MAP_VEHICULOS);
  }
  eliminarVehiculo(tenant: string, id: string) {
    return this.eliminar('vehiculos', tenant, id);
  }

  // ── rutas ──────────────────────────────────────────────────────────────
  async listarRutas(tenant: string, personalId?: string) {
    let rutas: Record<string, unknown>[];
    if (personalId) {
      rutas = await this.pg.query(
        `SELECT * FROM field.rutas WHERE tenant_id = $1 AND personal_asignado_ids ?| array[$2] ORDER BY created_at DESC LIMIT 2000`,
        [tenant, personalId],
      );
    } else {
      rutas = await this.listar('rutas', tenant);
    }
    const paradas = await this.pg.query<any>(`SELECT * FROM field.paradas WHERE tenant_id = $1`, [tenant]);
    const porRuta = new Map<string, any[]>();
    for (const p of paradas as any[]) {
      const arr = porRuta.get(p.ruta_id) ?? [];
      arr.push(p);
      porRuta.set(p.ruta_id, arr);
    }
    for (const r of rutas) {
      const ps = porRuta.get(r.id as string) ?? [];
      ps.sort((a, b) => (Number(a.orden) || 0) - (Number(b.orden) || 0));
      r.paradas = ps;
    }
    return rutas;
  }
  async obtenerRuta(tenant: string, id: string) {
    const r = await this.obtener('rutas', tenant, id);
    const paradas = await this.pg.query<any>(
      `SELECT * FROM field.paradas WHERE tenant_id = $1 AND ruta_id = $2`,
      [tenant, id],
    );
    paradas.sort((a, b) => (Number(a.orden) || 0) - (Number(b.orden) || 0));
    (r as Record<string, unknown>).paradas = paradas;
    return r;
  }
  crearRuta(tenant: string, dto: CrearRutaDto) {
    return this.insertar('rutas', tenant, undefined, dto, MAP_RUTAS, DEF_RUTAS, false);
  }
  actualizarRuta(tenant: string, id: string, dto: ActualizarRutaDto) {
    return this.actualizar('rutas', tenant, id, dto, MAP_RUTAS);
  }
  eliminarRuta(tenant: string, id: string) {
    return this.eliminar('rutas', tenant, id);
  }
  async agregarParada(tenant: string, rutaId: string, dto: ParadaDto) {
    await this.obtener('rutas', tenant, rutaId);
    // Inyectar el ruta_id del path param (el DTO lo trae opcional y puede venir vacío)
    const conRuta = { ...(dto as unknown as Record<string, unknown>), rutaId };
    return this.insertar('paradas', tenant, undefined, conRuta, MAP_PARADAS, DEF_PARADAS, false);
  }
  async actualizarParada(tenant: string, rutaId: string, paradaId: string, dto: ParadaDto) {
    // verifica pertenencia a la ruta y tenant
    const parada = await this.pg.queryOne<{ id: string; ruta_id: string }>(
      `SELECT id, ruta_id FROM field.paradas WHERE tenant_id = $1 AND id = $2`,
      [tenant, paradaId],
    );
    if (!parada) throw new NotFoundError('parada', paradaId);
    if (parada.ruta_id !== rutaId) throw new DomainError('PARADA_INVALIDA', 'La parada no pertenece a la ruta indicada.');
    return this.actualizar('paradas', tenant, paradaId, dto, MAP_PARADAS);
  }
  async asignarRuta(tenant: string, rutaId: string, dto: AsignarRutaDto) {
    const sets: string[] = [];
    const vals: unknown[] = [];
    let i = 1;
    if (dto.personalIds !== undefined) {
      sets.push(`personal_asignado_ids = $${i}`);
      vals.push(JSON.stringify(dto.personalIds));
      i++;
    }
    if (dto.vehiculoId !== undefined) {
      sets.push(`vehiculo_asignado_id = $${i}`);
      vals.push(dto.vehiculoId);
      i++;
    }
    sets.push(`updated_at = $${i}`);
    vals.push(new Date());
    vals.push(tenant, rutaId);
    const sql = `UPDATE field.rutas SET ${sets.join(', ')} WHERE tenant_id = $${i + 1} AND id = $${i + 2} RETURNING *`;
    const fila = await this.pg.queryOne(sql, vals);
    if (!fila) throw new NotFoundError('ruta', rutaId);
    return fila;
  }

  // ── pedidos ────────────────────────────────────────────────────────────
  listarPedidos(tenant: string, filtro: { estado?: string; rutaId?: string; clienteId?: string }) {
    const extras = [] as { col: string; val: unknown }[];
    if (filtro.estado) extras.push({ col: 'estado', val: filtro.estado });
    if (filtro.rutaId) extras.push({ col: 'ruta_id', val: filtro.rutaId });
    if (filtro.clienteId) extras.push({ col: 'cliente_id', val: filtro.clienteId });
    return this.listar('pedidos', tenant, extras);
  }
  obtenerPedido(tenant: string, id: string) {
    return this.obtener('pedidos', tenant, id);
  }
  crearPedido(tenant: string, dto: CrearPedidoDto) {
    return this.insertar('pedidos', tenant, undefined, dto, MAP_PEDIDOS, DEF_PEDIDOS, false);
  }
  actualizarPedido(tenant: string, id: string, dto: ActualizarPedidoDto) {
    return this.actualizar('pedidos', tenant, id, dto, MAP_PEDIDOS);
  }
  eliminarPedido(tenant: string, id: string) {
    return this.eliminar('pedidos', tenant, id);
  }
  cambiarEstadoPedido(tenant: string, id: string, dto: CambiarEstadoPedidoDto) {
    return this.actualizar('pedidos', tenant, id, { estado: dto.estado } as Record<string, unknown>, MAP_PEDIDOS);
  }

  // ── asistencia ─────────────────────────────────────────────────────────
  listarAsistencia(tenant: string, filtro: { personalId?: string; fecha?: string }) {
    const extras = [] as { col: string; val: unknown }[];
    if (filtro.personalId) extras.push({ col: 'personal_id', val: filtro.personalId });
    if (filtro.fecha) extras.push({ col: 'DATE(registrado_en)', val: filtro.fecha });
    return this.listar('asistencia', tenant, extras);
  }
  registrarAsistencia(tenant: string, dto: CrearAsistenciaDto) {
    return this.insertar('asistencia', tenant, undefined, dto, MAP_ASISTENCIA, {}, false);
  }

  // ── incidencias ───────────────────────────────────────────────────────
  listarIncidencias(tenant: string, filtro: { estado?: string; rutaId?: string }) {
    const extras = [] as { col: string; val: unknown }[];
    if (filtro.estado) extras.push({ col: 'estado', val: filtro.estado });
    if (filtro.rutaId) extras.push({ col: 'ruta_id', val: filtro.rutaId });
    return this.listar('incidencias', tenant, extras);
  }
  obtenerIncidencia(tenant: string, id: string) {
    return this.obtener('incidencias', tenant, id);
  }
  crearIncidencia(tenant: string, dto: CrearIncidenciaDto) {
    return this.insertar('incidencias', tenant, undefined, dto, MAP_INCIDENCIAS, DEF_INCIDENCIAS, false);
  }
  actualizarIncidencia(tenant: string, id: string, dto: ActualizarIncidenciaDto) {
    return this.actualizar('incidencias', tenant, id, dto, MAP_INCIDENCIAS);
  }
  eliminarIncidencia(tenant: string, id: string) {
    return this.eliminar('incidencias', tenant, id);
  }

  // ── tracking (GPS) ────────────────────────────────────────────────────
  listarTracking(tenant: string, filtro: { personalId?: string; desde?: string; hasta?: string }) {
    const extras = [] as { col: string; val: unknown }[];
    if (filtro.personalId) extras.push({ col: 'personal_id', val: filtro.personalId });
    if (filtro.desde) extras.push({ col: 'registrado_en >=', val: filtro.desde });
    if (filtro.hasta) extras.push({ col: 'registrado_en <=', val: filtro.hasta });
    return this.listar('tracking', tenant, extras, 'registrado_en');
  }
  registrarTracking(tenant: string, registros: CrearTrackingDto[]) {
    return Promise.all(
      registros.map((r) => this.insertar('tracking', tenant, undefined, r, MAP_TRACKING, {}, false)),
    );
  }

  // ── visitas (telemetria) ──────────────────────────────────────────────
  listarVisitas(tenant: string, filtro: { personalId?: string; fecha?: string }) {
    const extras = [] as { col: string; val: unknown }[];
    if (filtro.personalId) extras.push({ col: 'personal_id', val: filtro.personalId });
    if (filtro.fecha) extras.push({ col: 'DATE(created_at)', val: filtro.fecha });
    return this.listar('visitas', tenant, extras);
  }
  registrarVisita(tenant: string, dto: CrearVisitaDto) {
    return this.insertar('visitas', tenant, undefined, dto, MAP_VISITAS, DEF_VISITAS, false);
  }

  // ── cumplimiento ──────────────────────────────────────────────────────
  async obtenerCumplimiento(tenant: string, rutaId: string, fecha: string) {
    const fila = await this.pg.queryOne(
      `SELECT * FROM field.cumplimiento WHERE tenant_id = $1 AND ruta_id = $2 AND fecha = $3`,
      [tenant, rutaId, fecha],
    );
    if (!fila) throw new NotFoundError('cumplimiento', `${rutaId}/${fecha}`);
    return fila;
  }
  async guardarCumplimiento(tenant: string, dto: GuardarCumplimientoDto & { rutaId: string; fecha: string }) {
    const metricas = dto.metricas && typeof dto.metricas === 'object' ? JSON.stringify(dto.metricas) : '{}';
    const fila = await this.pg.queryOne(
      `INSERT INTO field.cumplimiento (tenant_id, ruta_id, fecha, metricas, updated_at)
       VALUES ($1, $2, $3, $4, NOW())
       ON CONFLICT (tenant_id, ruta_id, fecha) DO UPDATE SET metricas = $4, updated_at = NOW()
       RETURNING *`,
      [tenant, dto.rutaId, dto.fecha, metricas],
    );
    if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo guardar el cumplimiento.');
    return fila;
  }

  // ── sync offline (app-test) ───────────────────────────────────────────
  async sincronizar(tenant: string, operaciones: SyncOperacionDto[]): Promise<
    { tipo: string; id?: string; ok: boolean; error?: string }[]
  > {
    const resultado: { tipo: string; id?: string; ok: boolean; error?: string }[] = [];
    for (const op of operaciones) {
      const conf = TABLA_SYNC[op.tipo];
      const id = (op.payload as { id?: string })?.id;
      if (!conf) {
        resultado.push({ tipo: op.tipo, id, ok: false, error: 'tipo de entidad no soportado' });
        continue;
      }
      try {
        const fila = await this.insertar(
          conf.tabla,
          tenant,
          id,
          op.payload as Record<string, unknown>,
          conf.map,
          conf.def,
          true,
        );
        resultado.push({ tipo: op.tipo, id: (fila as { id: string }).id, ok: true });
      } catch (err) {
        resultado.push({ tipo: op.tipo, id, ok: false, error: err instanceof Error ? err.message : String(err) });
      }
    }
    this.logger.info({ msg: 'sync completado', tenant, operaciones: operaciones.length });
    return resultado;
  }
}
