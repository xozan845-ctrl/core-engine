import { Injectable } from '@nestjs/common';
import { randomInt, randomUUID } from 'crypto';
import type { PoolClient } from 'pg';
import {
  DomainError,
  Logger,
  Money,
  NotFoundError,
  PgService,
  ROLES,
  UsuarioContexto,
} from '@core/shared';
import {
  ActualizarClienteComercioRequestDto,
  ActualizarConfiguracionDocumentosRequestDto,
  ActualizarCuentaBancoRequestDto,
  ActualizarGastoRequestDto,
  ActualizarMetodoPagoRequestDto,
  CrearCierreCajaRequestDto,
  CrearClienteComercioRequestDto,
  CrearCodigoBarrasRequestDto,
  CrearCompraDirectaRequestDto,
  CrearCompraProductoNuevoRequestDto,
  CrearCompraRequestDto,
  CrearCreditoRequestDto,
  CrearCuentaBancoRequestDto,
  CrearCuentaPorPagarRequestDto,
  CrearFondoCajaChicaRequestDto,
  CrearGastoRequestDto,
  CrearMetodoPagoRequestDto,
  CrearMovimientoCajaChicaRequestDto,
  CrearMovimientoCajaRequestDto,
  CrearOrdenCompraRequestDto,
  CrearProveedorRequestDto,
  CrearTransferenciaCajaRequestDto,
  ImportarClientesRequestDto,
  LineaCompraRequestDto,
} from './comercio.dtos';
import {
  AmbitoComercio,
  FilaRegistroComercio,
  RECURSOS_SOLO_EMPRESA,
  RecursoComercio,
  RegistroComercio,
} from './comercio.types';

const ROLES_PERMITIDOS: ReadonlySet<string> = new Set([
  ROLES.SUPER_ADMIN,
  ROLES.ADMIN,
  ROLES.EMPRESARIO,
  ROLES.VENDEDOR,
]);

type Datos = Record<string, unknown>;

/**
 * Libro de operación de comercio. Persiste los recursos que el frontend de
 * empresa/vendedor administra (bancos, créditos, gastos, caja, compras,
 * clientes y códigos de barras) con aislamiento por tenant o por vendedor.
 */
@Injectable()
export class ComercioService {
  private readonly logger = Logger.create('finance.comercio');

  constructor(private readonly pg: PgService) {}

  // ── consultas y CRUD genérico ────────────────────────────────────────────

  /**
   * Lista los registros no eliminados del recurso dentro del ámbito del
   * usuario. Los administradores pueden acotar con `empresaId`; sin él ven
   * todos los tenants.
   */
  async listar(
    recurso: RecursoComercio,
    usuario: UsuarioContexto,
    empresaId?: string,
  ): Promise<RegistroComercio[]> {
    const ambito = this.ambito(recurso, usuario, empresaId);
    const filas = await this.pg.query<FilaRegistroComercio>(
      `SELECT id, datos, creado_en, actualizado_en
       FROM finance.workspace_registros
       WHERE recurso = $1 AND eliminado_en IS NULL
         AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
         AND ($3::uuid IS NULL OR propietario_id = $3::uuid)
       ORDER BY creado_en DESC, id`,
      [recurso, ambito.tenantId, ambito.propietarioId],
    );
    return filas.map((fila) => this.aRegistro(fila));
  }

  async obtener(
    recurso: RecursoComercio,
    id: string,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const fila = await this.buscar(recurso, id, usuario);
    return this.aRegistro(fila);
  }

  async crear(
    recurso: RecursoComercio,
    datos: Datos,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const ambito = this.ambito(recurso, usuario);
    if (!ambito.tenantId && !ambito.propietarioId) {
      throw new DomainError('TENANT_REQUERIDO', 'El contexto no tiene empresa ni vendedor.');
    }
    const payload = this.normalizar(recurso, datos);
    const fila = await this.insertar(recurso, payload, ambito);
    return this.aRegistro(fila);
  }

  async actualizar(
    recurso: RecursoComercio,
    id: string,
    datos: Datos,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const ambito = this.ambito(recurso, usuario);
    const fila = await this.pg.queryOne<FilaRegistroComercio>(
      `UPDATE finance.workspace_registros
       SET datos = datos || $4::jsonb, actualizado_en = NOW()
       WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
         AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
         AND ($5::uuid IS NULL OR propietario_id = $5::uuid)
       RETURNING id, datos, creado_en, actualizado_en`,
      [id, recurso, ambito.tenantId, this.json(datos), ambito.propietarioId],
    );
    if (!fila) throw new NotFoundError('Registro', id);
    return this.aRegistro(fila);
  }

  async eliminar(
    recurso: RecursoComercio,
    id: string,
    usuario: UsuarioContexto,
  ): Promise<void> {
    const ambito = this.ambito(recurso, usuario);
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE finance.workspace_registros
       SET eliminado_en = NOW(), actualizado_en = NOW()
       WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
         AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
         AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
       RETURNING id`,
      [id, recurso, ambito.tenantId, ambito.propietarioId],
    );
    if (!fila) throw new NotFoundError('Registro', id);
  }

  // ── bancos ───────────────────────────────────────────────────────────────

  listarBancos(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('bancos-cuentas', usuario);
  }

  crearBanco(dto: CrearCuentaBancoRequestDto, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.crear(
      'bancos-cuentas',
      {
        bank: dto.bank,
        alias: dto.alias,
        accountNumber: dto.accountNumber,
        type: dto.type,
        balanceCents: dto.balanceCents,
      },
      usuario,
    );
  }

  actualizarBanco(
    id: string,
    dto: ActualizarCuentaBancoRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.actualizar('bancos-cuentas', id, dto as Datos, usuario);
  }

  // ── créditos ─────────────────────────────────────────────────────────────

  listarCreditos(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('creditos', usuario);
  }

  crearCredito(dto: CrearCreditoRequestDto, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.crear(
      'creditos',
      {
        customer: dto.customer,
        concept: dto.concept,
        date: dto.date ?? this.hoy(),
        dueDate: dto.dueDate,
        amountCents: dto.amountCents,
        balanceCents: dto.amountCents,
      },
      usuario,
    );
  }

  pagarCredito(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.actualizar('creditos', id, { balanceCents: 0, paidAt: this.hoy() }, usuario);
  }

  // ── gastos ───────────────────────────────────────────────────────────────

  listarGastos(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('gastos', usuario);
  }

  crearGasto(dto: CrearGastoRequestDto, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.crear('gastos', { ...dto }, usuario);
  }

  actualizarGasto(
    id: string,
    dto: ActualizarGastoRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.actualizar('gastos', id, dto as Datos, usuario);
  }

  // ── cuentas por pagar ────────────────────────────────────────────────────

  listarCuentasPorPagar(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('cuentas-por-pagar', usuario);
  }

  crearCuentaPorPagar(
    dto: CrearCuentaPorPagarRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('cuentas-por-pagar', { ...dto }, usuario);
  }

  pagarCuentaPorPagar(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.actualizar(
      'cuentas-por-pagar',
      id,
      { status: 'pagada', paidAt: this.hoy() },
      usuario,
    );
  }

  // ── proveedores y compras ────────────────────────────────────────────────

  listarProveedores(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('proveedores', usuario);
  }

  crearProveedor(
    dto: CrearProveedorRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('proveedores', { ...dto }, usuario);
  }

  listarCompras(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('compras', usuario);
  }

  async crearCompra(
    dto: CrearCompraRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const numero = dto.number ?? (await this.siguienteNumero('compras', 'C', usuario));
    return this.crear(
      'compras',
      {
        number: numero,
        date: dto.date,
        supplierId: dto.supplierId ?? null,
        supplierName: dto.supplierName ?? '',
        paymentMethod: dto.paymentMethod,
        status: dto.status,
        origin: dto.origin,
        costType: dto.costType,
        lines: this.lineasConId(dto.lines),
        notes: dto.notes ?? '',
        retentionCents: dto.retentionCents ?? 0,
        userName: dto.userName ?? '',
      },
      usuario,
    );
  }

  async crearCompraDirecta(
    dto: CrearCompraDirectaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const numero = dto.number ?? (await this.siguienteNumero('compras', 'C', usuario));
    const linea: LineaCompraRequestDto = {
      productId: null,
      name: dto.concept,
      sku: '',
      quantity: 1,
      unitCostCents: dto.amountCents,
      salePriceCents: null,
    };
    return this.crear(
      'compras',
      {
        number: numero,
        date: dto.date,
        supplierId: dto.supplierId ?? null,
        supplierName: dto.supplierName ?? '',
        paymentMethod: dto.paymentMethod,
        status: 'recibida',
        origin: 'directa',
        costType: dto.costType,
        lines: this.lineasConId([linea]),
        notes: dto.notes ?? '',
        retentionCents: dto.retentionCents ?? 0,
        userName: '',
      },
      usuario,
    );
  }

  async crearCompraProductoNuevo(
    dto: CrearCompraProductoNuevoRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const numero = dto.number ?? (await this.siguienteNumero('compras', 'C', usuario));
    const linea: LineaCompraRequestDto = {
      productId: null,
      name: dto.productName,
      sku: dto.sku ?? '',
      quantity: dto.quantity,
      unitCostCents: dto.unitCostCents,
      salePriceCents: dto.salePriceCents,
    };
    return this.crear(
      'compras',
      {
        number: numero,
        date: dto.date,
        supplierId: dto.supplierId ?? null,
        supplierName: dto.supplierName ?? '',
        paymentMethod: dto.paymentMethod,
        status: 'recibida',
        origin: 'productos',
        costType: 'inventario',
        lines: this.lineasConId([linea]),
        notes: dto.notes ?? '',
        retentionCents: 0,
        userName: '',
      },
      usuario,
    );
  }

  anularCompra(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.actualizar('compras', id, { status: 'anulada' }, usuario);
  }

  listarOrdenesCompra(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('ordenes-compra', usuario);
  }

  async crearOrdenCompra(
    dto: CrearOrdenCompraRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const numero = dto.number ?? (await this.siguienteNumero('ordenes-compra', 'OC', usuario));
    return this.crear(
      'ordenes-compra',
      {
        number: numero,
        date: dto.date,
        supplierId: dto.supplierId ?? null,
        supplierName: dto.supplierName ?? '',
        lines: this.lineasConId(dto.lines),
        notes: dto.notes ?? '',
      },
      usuario,
    );
  }

  /**
   * Convierte una orden pendiente en compra. La compra nace pendiente y con
   * pago en efectivo, igual que el flujo local del frontend; el usuario puede
   * ajustar el documento después.
   */
  async convertirOrdenCompra(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    const orden = await this.obtener('ordenes-compra', id, usuario);
    if (orden.status !== 'pendiente') {
      throw new DomainError('ORDEN_NO_PENDIENTE', 'La orden ya no está pendiente.');
    }
    const numero = await this.siguienteNumero('compras', 'C', usuario);
    await this.crear(
      'compras',
      {
        number: numero,
        date: this.hoy(),
        supplierId: (orden.supplierId as string | null) ?? null,
        supplierName: (orden.supplierName as string | undefined) ?? '',
        paymentMethod: 'efectivo',
        status: 'pendiente',
        origin: 'productos',
        costType: 'inventario',
        lines: orden.lines,
        notes: `Convertida desde la orden ${String(orden.number ?? '')}.`,
        retentionCents: 0,
        userName: '',
      },
      usuario,
    );
    return this.actualizar('ordenes-compra', id, { status: 'convertida' }, usuario);
  }

  anularOrdenCompra(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.actualizar('ordenes-compra', id, { status: 'anulada' }, usuario);
  }

  async obtenerConfiguracionDocumentos(usuario: UsuarioContexto): Promise<RegistroComercio> {
    const registros = await this.listar('configuracion-documentos', usuario);
    const existente = registros[0];
    if (existente) return existente;
    return this.crear(
      'configuracion-documentos',
      {
        logoSize: 'mediano',
        headerColor: '#0f172a',
        matchTableColor: false,
        footerMessage: '',
        showRuc: true,
        showAddress: true,
        showPhone: true,
      },
      usuario,
    );
  }

  async actualizarConfiguracionDocumentos(
    dto: ActualizarConfiguracionDocumentosRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const actual = await this.obtenerConfiguracionDocumentos(usuario);
    return this.actualizar(
      'configuracion-documentos',
      actual.id,
      dto as unknown as Datos,
      usuario,
    );
  }

  // ── caja ─────────────────────────────────────────────────────────────────

  listarMetodosPago(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('caja-metodos-pago', usuario);
  }

  crearMetodoPago(
    dto: CrearMetodoPagoRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('caja-metodos-pago', { ...dto }, usuario);
  }

  actualizarMetodoPago(
    id: string,
    dto: ActualizarMetodoPagoRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.actualizar('caja-metodos-pago', id, dto as Datos, usuario);
  }

  async eliminarMetodoPago(id: string, usuario: UsuarioContexto): Promise<void> {
    const metodos = await this.listar('caja-metodos-pago', usuario);
    if (metodos.length <= 1) {
      throw new DomainError('ULTIMO_METODO', 'Debe quedar al menos un método de pago.');
    }
    const movimientos = await this.listar('caja-movimientos', usuario);
    if (movimientos.some((movimiento) => movimiento.paymentMethod === id)) {
      throw new DomainError(
        'METODO_EN_USO',
        'No se puede eliminar un método de pago con movimientos registrados.',
      );
    }
    await this.eliminar('caja-metodos-pago', id, usuario);
  }

  listarMovimientosCaja(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('caja-movimientos', usuario);
  }

  crearMovimientoCaja(
    dto: CrearMovimientoCajaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('caja-movimientos', { ...dto }, usuario);
  }

  anularMovimientoCaja(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    return this.actualizar('caja-movimientos', id, { status: 'anulado' }, usuario);
  }

  listarCierres(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('caja-cierres', usuario);
  }

  /** Calcula el corte del día sin persistirlo (pantalla de cierre). */
  async previewCierre(
    usuario: UsuarioContexto,
    fecha?: string,
  ): Promise<{
    date: string;
    openingBalanceCents: number;
    incomeCents: number;
    expenseCents: number;
    expectedBalanceCents: number;
    movementsCount: number;
    existing: RegistroComercio | null;
  }> {
    const dia = fecha ?? this.hoy();
    const movimientos = (await this.listar('caja-movimientos', usuario)).filter(
      (movimiento) => movimiento.status === 'completado',
    );
    const anteriores = movimientos.filter((movimiento) => String(movimiento.date) < dia);
    const delDia = movimientos.filter((movimiento) => movimiento.date === dia);
    const abrir = this.saldo(anteriores);
    const ingresos = this.suma(delDia.filter((movimiento) => movimiento.type === 'ingreso'));
    const egresos = this.suma(delDia.filter((movimiento) => movimiento.type === 'egreso'));
    const esperado = abrir + ingresos - egresos;
    const cierres = await this.listar('caja-cierres', usuario);
    return {
      date: dia,
      openingBalanceCents: abrir,
      incomeCents: ingresos,
      expenseCents: egresos,
      expectedBalanceCents: esperado,
      movementsCount: delDia.length,
      existing: cierres.find((cierre) => cierre.date === dia) ?? null,
    };
  }

  async crearCierre(
    dto: CrearCierreCajaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const preview = await this.previewCierre(usuario, dto.date);
    if (preview.existing) {
      throw new DomainError('CIERRE_EXISTENTE', `Ya existe un cierre para ${dto.date}.`);
    }
    return this.crear(
      'caja-cierres',
      {
        date: preview.date,
        openingBalanceCents: preview.openingBalanceCents,
        incomeCents: preview.incomeCents,
        expenseCents: preview.expenseCents,
        expectedBalanceCents: preview.expectedBalanceCents,
        countedBalanceCents: dto.countedBalanceCents,
        differenceCents: dto.countedBalanceCents - preview.expectedBalanceCents,
        movementsCount: preview.movementsCount,
        notes: dto.notes ?? '',
        closedAt: new Date().toISOString(),
      },
      usuario,
    );
  }

  async obtenerFondoCajaChica(usuario: UsuarioContexto): Promise<RegistroComercio> {
    const registros = await this.listar('caja-chica-fondo', usuario);
    const existente = registros[0];
    if (existente) return existente;
    return this.crear(
      'caja-chica-fondo',
      { amountCents: 0, responsible: '', updatedAt: new Date().toISOString() },
      usuario,
    );
  }

  async guardarFondoCajaChica(
    dto: CrearFondoCajaChicaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    const actual = await this.obtenerFondoCajaChica(usuario);
    return this.actualizar(
      'caja-chica-fondo',
      actual.id,
      { amountCents: dto.amountCents, responsible: dto.responsible, updatedAt: new Date().toISOString() },
      usuario,
    );
  }

  listarMovimientosCajaChica(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('caja-chica-movimientos', usuario);
  }

  crearMovimientoCajaChica(
    dto: CrearMovimientoCajaChicaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('caja-chica-movimientos', { ...dto }, usuario);
  }

  listarTransferencias(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('caja-transferencias', usuario);
  }

  /**
   * Registra una transferencia interna y sus dos movimientos de caja en una
   * sola transacción. Si la cuenta origen es bancaria se valida y descuenta el
   * saldo con un UPDATE condicional (evita sobregiros por concurrencia).
   */
  async crearTransferencia(
    dto: CrearTransferenciaCajaRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    if (dto.fromId === dto.toId) {
      throw new DomainError('TRANSFERENCIA_INVALIDA', 'Las cuentas origen y destino deben ser distintas.');
    }
    const ambito = this.ambito('caja-transferencias', usuario);
    return this.pg.transaccion(async (client) => {
      if (dto.fromKind === 'bank') {
        await this.moverBanco(client, dto.fromId, -dto.amountCents, ambito);
      }
      if (dto.toKind === 'bank') {
        await this.moverBanco(client, dto.toId, dto.amountCents, ambito);
      }
      const transferencia = await this.insertar(
        'caja-transferencias',
        {
          ...dto,
          notes: dto.notes ?? '',
          status: 'completado',
        },
        ambito,
        client,
      );
      const transferId = transferencia.id;
      const base = {
        date: dto.date,
        concept: dto.concept,
        amountCents: dto.amountCents,
        paymentMethod: 'transferencia',
        category: 'transferencia',
        currency: 'NIO',
        originalAmountCents: dto.amountCents,
        exchangeRate: 1,
        source: 'transferencia',
        status: 'completado',
        reference: transferId,
        observations: dto.notes ?? '',
        transferId,
      };
      await this.insertar(
        'caja-movimientos',
        { ...base, type: 'egreso' },
        ambito,
        client,
      );
      await this.insertar(
        'caja-movimientos',
        { ...base, type: 'ingreso' },
        ambito,
        client,
      );
      this.logger.info({
        msg: 'transferencia de caja registrada',
        transferId,
        amountCents: dto.amountCents,
      });
      return this.aRegistro(transferencia);
    });
  }

  /** Anula la transferencia, sus movimientos ligados y revierte los bancos. */
  async anularTransferencia(id: string, usuario: UsuarioContexto): Promise<RegistroComercio> {
    const ambito = this.ambito('caja-transferencias', usuario);
    const actual = await this.buscar('caja-transferencias', id, usuario);
    if (actual.datos.status === 'anulado') return this.aRegistro(actual);
    const monto = this.numero(actual.datos.amountCents);
    return this.pg.transaccion(async (client) => {
      if (actual.datos.fromKind === 'bank') {
        await this.moverBanco(client, String(actual.datos.fromId), monto, ambito);
      }
      if (actual.datos.toKind === 'bank') {
        await this.moverBanco(client, String(actual.datos.toId), -monto, ambito);
      }
      const fila = await client.query<FilaRegistroComercio>(
        `UPDATE finance.workspace_registros
         SET datos = datos || '{"status":"anulado"}'::jsonb, actualizado_en = NOW()
         WHERE id = $1 AND recurso = 'caja-transferencias'
         RETURNING id, datos, creado_en, actualizado_en`,
        [id],
      );
      await client.query(
        `UPDATE finance.workspace_registros
         SET datos = datos || '{"status":"anulado"}'::jsonb, actualizado_en = NOW()
         WHERE recurso = 'caja-movimientos' AND eliminado_en IS NULL
           AND datos->>'transferId' = $1`,
        [id],
      );
      return this.aRegistro(fila.rows[0]);
    });
  }

  // ── clientes ─────────────────────────────────────────────────────────────

  listarClientes(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('clientes', usuario);
  }

  crearCliente(
    dto: CrearClienteComercioRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('clientes', { ...dto }, usuario);
  }

  actualizarCliente(
    id: string,
    dto: ActualizarClienteComercioRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.actualizar('clientes', id, dto as Datos, usuario);
  }

  /**
   * Importa una cartera manteniendo la semántica del frontend: omite filas sin
   * nombre y duplicados por nombre + teléfono normalizados.
   */
  async importarClientes(
    dto: ImportarClientesRequestDto,
    usuario: UsuarioContexto,
  ): Promise<{ imported: number; skipped: number; errors: string[] }> {
    if (dto.clientes.length > 500) {
      throw new DomainError('IMPORTACION_DEMASIADO_GRANDE', 'El máximo es 500 clientes por importación.');
    }
    const existentes = await this.listar('clientes', usuario);
    const claves = new Set(existentes.map((cliente) => this.claveCliente(cliente)));
    let imported = 0;
    let skipped = 0;
    const errors: string[] = [];
    for (const [indice, fila] of dto.clientes.entries()) {
      const nombre = (fila.name ?? '').trim();
      if (!nombre) {
        skipped += 1;
        errors.push(`Fila ${indice + 1}: nombre vacío.`);
        continue;
      }
      const clave = this.claveCliente(fila as unknown as Datos);
      if (claves.has(clave)) {
        skipped += 1;
        continue;
      }
      try {
        await this.crear('clientes', { ...fila, name: nombre, source: 'import' }, usuario);
        claves.add(clave);
        imported += 1;
      } catch (error) {
        errors.push(`Fila ${indice + 1}: ${error instanceof Error ? error.message : 'error'}`);
      }
    }
    return { imported, skipped, errors };
  }

  // ── códigos de barras ────────────────────────────────────────────────────

  listarCodigosBarras(usuario: UsuarioContexto): Promise<RegistroComercio[]> {
    return this.listar('codigos-barras', usuario);
  }

  crearCodigoBarras(
    dto: CrearCodigoBarrasRequestDto,
    usuario: UsuarioContexto,
  ): Promise<RegistroComercio> {
    return this.crear('codigos-barras', { ...dto }, usuario);
  }

  // ── helpers ──────────────────────────────────────────────────────────────

  private ambito(
    recurso: RecursoComercio,
    usuario: UsuarioContexto,
    empresaId?: string,
  ): AmbitoComercio {
    if (!usuario || !ROLES_PERMITIDOS.has(usuario.rol)) {
      throw new DomainError('ACCESO_DENEGADO', 'El rol no tiene acceso a la operación de comercio.');
    }
    if (!empresaId && usuario.rol === ROLES.VENDEDOR && RECURSOS_SOLO_EMPRESA.has(recurso)) {
      throw new DomainError('ACCESO_DENEGADO', 'Este recurso pertenece al workspace de empresa.');
    }
    if (usuario.rol === ROLES.VENDEDOR) {
      return { tenantId: null, propietarioId: usuario.user_id };
    }
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) {
        throw new DomainError('TENANT_REQUERIDO', 'El empresario no tiene una empresa activa.');
      }
      return { tenantId: usuario.tenant_id, propietarioId: null };
    }
    return { tenantId: empresaId ?? usuario.tenant_id ?? null, propietarioId: null };
  }

  private async buscar(
    recurso: RecursoComercio,
    id: string,
    usuario: UsuarioContexto,
  ): Promise<FilaRegistroComercio> {
    const ambito = this.ambito(recurso, usuario);
    const fila = await this.pg.queryOne<FilaRegistroComercio>(
      `SELECT id, datos, creado_en, actualizado_en
       FROM finance.workspace_registros
       WHERE id = $1 AND recurso = $2 AND eliminado_en IS NULL
         AND ($3::uuid IS NULL OR tenant_id = $3::uuid)
         AND ($4::uuid IS NULL OR propietario_id = $4::uuid)`,
      [id, recurso, ambito.tenantId, ambito.propietarioId],
    );
    if (!fila) throw new NotFoundError('Registro', id);
    return fila;
  }

  private async insertar(
    recurso: RecursoComercio,
    datos: Datos,
    ambito: AmbitoComercio,
    client?: PoolClient,
  ): Promise<FilaRegistroComercio> {
    const sql = `INSERT INTO finance.workspace_registros (tenant_id, propietario_id, recurso, datos)
                 VALUES ($1::uuid, $2::uuid, $3, $4::jsonb)
                 RETURNING id, datos, creado_en, actualizado_en`;
    const params = [ambito.tenantId, ambito.propietarioId, recurso, this.json(datos)];
    if (client) {
      const resultado = await client.query<FilaRegistroComercio>(sql, params);
      return resultado.rows[0];
    }
    const fila = await this.pg.queryOne<FilaRegistroComercio>(sql, params);
    if (!fila) throw new DomainError('REGISTRO_NO_CREADO', 'No se pudo crear el registro.');
    return fila;
  }

  /**
   * Construye el payload final del recurso. Es el único punto donde se aplican
   * defaults y se calculan totales para que el contrato no los exija.
   */
  private normalizar(recurso: RecursoComercio, datos: Datos): Datos {
    const ahora = new Date().toISOString();
    switch (recurso) {
      case 'creditos':
        return {
          ...datos,
          date: datos.date ?? this.hoy(),
          balanceCents: datos.balanceCents ?? datos.amountCents,
        };
      case 'gastos':
        return {
          status: 'pendiente',
          includesVat: false,
          irRetentionCents: 0,
          beneficiary: '',
          notes: '',
          scheduledFor: '',
          recurrence: '',
          ...datos,
          createdAt: ahora,
        };
      case 'cuentas-por-pagar':
        return { status: 'pendiente', paidAt: '', ...datos, createdAt: ahora };
      case 'proveedores':
        return {
          ruc: '',
          phone: '',
          email: '',
          address: '',
          ...datos,
          createdAt: ahora,
        };
      case 'compras':
        return this.normalizarCompra(datos, ahora);
      case 'ordenes-compra':
        return {
          ...datos,
          status: 'pendiente',
          totalCents: this.totalLineas(datos.lines),
          createdAt: ahora,
        };
      case 'caja-metodos-pago':
        return {
          name: datos.name,
          kind: datos.kind,
          icon: (datos.icon as string | undefined) ?? 'payments',
          active: datos.active ?? true,
          createdAt: ahora,
        };
      case 'caja-movimientos':
        return {
          date: datos.date,
          type: datos.type,
          concept: datos.concept,
          amountCents: datos.amountCents,
          currency: datos.currency ?? 'NIO',
          originalAmountCents: datos.originalAmountCents ?? datos.amountCents,
          exchangeRate: datos.exchangeRate ?? 1,
          paymentMethod: datos.paymentMethod,
          category: datos.category ?? 'otro',
          source: datos.source ?? 'manual',
          status: 'completado',
          reference: datos.reference ?? '',
          observations: datos.observations ?? '',
          ...(datos.transferId ? { transferId: datos.transferId } : {}),
          createdAt: ahora,
        };
      case 'caja-cierres':
        return { ...datos, createdAt: ahora };
      case 'caja-chica-fondo':
        return {
          amountCents: datos.amountCents ?? 0,
          responsible: datos.responsible ?? '',
          updatedAt: ahora,
          createdAt: ahora,
        };
      case 'caja-chica-movimientos':
        return { ...datos, createdAt: ahora };
      case 'caja-transferencias':
        return { ...datos, status: datos.status ?? 'completado', createdAt: ahora };
      case 'configuracion-documentos':
        return { ...datos, createdAt: ahora };
      case 'clientes':
        return {
          name: datos.name,
          phone: datos.phone ?? '',
          email: datos.email ?? '',
          company: datos.company ?? '',
          ...(datos.buyerId ? { buyerId: datos.buyerId } : {}),
          source: datos.source ?? 'manual',
          createdAt: ahora,
        };
      case 'codigos-barras':
        return {
          productName: datos.productName,
          sku: datos.sku,
          code: (datos.code as string | undefined)?.trim() || this.generarCodigo(String(datos.symbology)),
          symbology: datos.symbology,
          copies: datos.copies,
          createdAt: ahora,
        };
      case 'bancos-cuentas':
      default:
        return { ...datos, createdAt: ahora };
    }
  }

  private normalizarCompra(datos: Datos, ahora: string): Datos {
    const lineas = Array.isArray(datos.lines) ? datos.lines : [];
    const subtotalCents = this.totalLineas(lineas);
    return {
      ...datos,
      status: datos.status ?? 'pendiente',
      statusLabel: '',
      lines: lineas,
      subtotalCents,
      retentionCents: datos.retentionCents ?? 0,
      totalCents: subtotalCents,
      notes: datos.notes ?? '',
      userName: datos.userName ?? '',
      createdAt: ahora,
    };
  }

  private lineasConId(lineas: readonly LineaCompraRequestDto[]): Datos[] {
    return lineas.map((linea) => ({
      id: randomUUID(),
      productId: linea.productId ?? null,
      name: linea.name,
      sku: linea.sku ?? '',
      quantity: linea.quantity,
      unitCostCents: linea.unitCostCents,
      salePriceCents: linea.salePriceCents ?? null,
    }));
  }

  private totalLineas(lineas: unknown): number {
    if (!Array.isArray(lineas)) return 0;
    return lineas.reduce((total, linea) => {
      const fila = linea as Record<string, unknown>;
      const precio = this.numero(fila.unitCostCents);
      const cantidad = this.numero(fila.quantity);
      return Money.desdeCentavos(total).sumar(
        Money.desdeCentavos(Math.round(precio * cantidad)),
      ).centavos;
    }, 0);
  }

  private async siguienteNumero(
    recurso: RecursoComercio,
    prefijo: string,
    usuario: UsuarioContexto,
  ): Promise<string> {
    const registros = await this.listar(recurso, usuario);
    return `${prefijo}-${String(registros.length + 1).padStart(6, '0')}`;
  }

  /** Ajusta el saldo de una cuenta bancaria dentro de la transacción actual. */
  private async moverBanco(
    client: PoolClient,
    id: string,
    delta: number,
    ambito: AmbitoComercio,
  ): Promise<void> {
    if (delta === 0) return;
    const condicion = delta < 0 ? `AND (datos->>'balanceCents')::bigint >= $3` : '';
    const params: unknown[] = [id, ambito.tenantId, Math.abs(delta), ambito.propietarioId];
    if (delta < 0) {
      const resultado = await client.query(
        `UPDATE finance.workspace_registros
         SET datos = jsonb_set(datos, '{balanceCents}',
                 to_jsonb((datos->>'balanceCents')::bigint - $3)),
             actualizado_en = NOW()
         WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
           AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
           AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
           ${condicion}
         RETURNING id`,
        params,
      );
      if (resultado.rowCount === 0) {
        const existe = await client.query(
          `SELECT 1 FROM finance.workspace_registros
           WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
             AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
             AND ($4::uuid IS NULL OR propietario_id = $4::uuid)`,
          params,
        );
        if (existe.rowCount === 0) throw new NotFoundError('Cuenta bancaria', id);
        throw new DomainError('SALDO_INSUFICIENTE', 'La cuenta no tiene saldo suficiente.');
      }
      return;
    }
    const resultado = await client.query(
      `UPDATE finance.workspace_registros
       SET datos = jsonb_set(datos, '{balanceCents}',
               to_jsonb((datos->>'balanceCents')::bigint + $3)),
           actualizado_en = NOW()
       WHERE id = $1 AND recurso = 'bancos-cuentas' AND eliminado_en IS NULL
         AND ($2::uuid IS NULL OR tenant_id = $2::uuid)
         AND ($4::uuid IS NULL OR propietario_id = $4::uuid)
       RETURNING id`,
      params,
    );
    if (resultado.rowCount === 0) throw new NotFoundError('Cuenta bancaria', id);
  }

  private aRegistro(fila: FilaRegistroComercio): RegistroComercio {
    return {
      ...fila.datos,
      id: fila.id,
      createdAt: this.iso(fila.creado_en),
      updatedAt: this.iso(fila.actualizado_en),
    };
  }

  private suma(registros: readonly RegistroComercio[]): number {
    return registros.reduce(
      (total, registro) => Money.desdeCentavos(total).sumar(
        Money.desdeCentavos(this.numero(registro.amountCents)),
      ).centavos,
      0,
    );
  }

  private saldo(registros: readonly RegistroComercio[]): number {
    return registros.reduce((total, registro) => {
      const monto = this.numero(registro.amountCents);
      return registro.type === 'ingreso' ? total + monto : total - monto;
    }, 0);
  }

  private claveCliente(datos: Datos): string {
    const nombre = String(datos.name ?? '').trim().toLowerCase();
    const telefono = String(datos.phone ?? '').replace(/\D/g, '');
    return `${nombre}|${telefono}`;
  }

  private generarCodigo(simbologia: string): string {
    if (simbologia === 'EAN13') {
      const base = Array.from({ length: 12 }, () => randomInt(0, 10)).join('');
      return `${base}${this.digitoEan13(base)}`;
    }
    const sufijo = Date.now().toString(36).toUpperCase();
    return `C128-${sufijo}`;
  }

  /** Dígito verificador EAN-13 (pesos 1 y 3 alternados desde la izquierda). */
  private digitoEan13(base: string): number {
    const suma = base.split('').reduce((total, digito, indice) => {
      return total + Number(digito) * (indice % 2 === 0 ? 1 : 3);
    }, 0);
    return (10 - (suma % 10)) % 10;
  }

  private json(datos: Datos): string {
    return JSON.stringify(datos);
  }

  private numero(valor: unknown): number {
    const numero = typeof valor === 'number' ? valor : Number(valor);
    return Number.isFinite(numero) ? numero : 0;
  }

  private iso(valor: Date | string): string {
    return valor instanceof Date ? valor.toISOString() : new Date(valor).toISOString();
  }

  private hoy(): string {
    return new Date().toISOString().slice(0, 10);
  }
}
