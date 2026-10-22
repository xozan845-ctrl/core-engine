import type { Mock } from 'vitest';
import { EnviosService } from './envios.service';

/**
 * Logistica: guia de despacho idempotente al procesarse el pago,
 * avance de estado via orders (REST mocked, R-QA-2) y consulta de orden.
 */
describe('EnviosService (guia de despacho y transiciones)', () => {
  const crear = () => {
    const pg = {
      query: vi.fn().mockResolvedValue([]),
      queryOne: vi.fn().mockResolvedValue(null),
    };
    const servicio = new EnviosService(pg as never);
    return { servicio, pg };
  };

  const envio = {
    id: 'e1',
    guia: 'BGH-ABC-123456',
    order_id: 'o1',
    empresa_id: 'empresa-1',
    modalidad_logistica: 'propia',
    proveedor_tipo: 'empresa',
    proveedor_tenant_id: 'empresa-1',
    estado: 'en_preparacion',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  afterEach(() => vi.restoreAllMocks());

  describe('prepararEnvio (payment.procesado)', () => {
    it('debe devolver el envio existente sin insertar nada (idempotencia del consumidor)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(envio);
      const resultado = await servicio.prepararEnvio('o1', 'empresa-1', 150000, 'propia');
      expect(resultado).toEqual(envio);
      expect(pg.queryOne).toHaveBeenCalledTimes(1); // solo el SELECT de busqueda
      const [sql] = (pg.queryOne as Mock).mock.calls[0];
      expect(sql).toContain('SELECT');
    });

    it('debe crear el envio en preparacion con la guia cuando no existe', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(null) // no existe
        .mockResolvedValueOnce(envio); // INSERT ... RETURNING
      const resultado = await servicio.prepararEnvio('o1', 'empresa-1', 150000, 'propia');
      expect(resultado.guia).toMatch(/^BGH-/); // guia con prefijo del proyecto
      const [sql, params] = (pg.queryOne as Mock).mock.calls[1];
      expect(sql).toContain('INSERT INTO logistics.envios');
      expect(sql).toContain("'en_preparacion'");
      expect(params[2]).toBe('o1');
      expect(params[3]).toBe('empresa-1');
      expect(params[7]).toBe(150000); // monto en centavos (A02)
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('logistics.asignaciones_envio'),
        ['e1', null, 'empresa', 'empresa-1', 'automatica', null],
      );
    });

    it('debe devolver el envio existente sin re-registrar asignacion cuando sigue sin proveedor', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue({ ...envio, proveedor_tipo: null, proveedor_tenant_id: null });
      const resultado = await servicio.prepararEnvio('o1', 'empresa-1', 150000, 'hibrida');
      expect(resultado.proveedor_tipo).toBeNull();
      expect(pg.query).not.toHaveBeenCalled(); // no inserta asignacion ni envio
      expect(pg.queryOne).toHaveBeenCalledTimes(1);
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);
      await expect(servicio.prepararEnvio('o1', 'empresa-1', 1, 'propia')).rejects.toThrow(/no se pudo preparar/i);
    });

    it('debe dejar pendiente de proveedor un envio hibrido', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(envio);
      await servicio.prepararEnvio('o1', 'empresa-1', 1000, 'hibrida');
      const [, params] = (pg.queryOne as Mock).mock.calls[1];
      expect(params.slice(4, 7)).toEqual(['hibrida', null, null]);
    });

    it('debe asignar la flota global en modalidad plataforma', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(envio);
      await servicio.prepararEnvio('o1', 'empresa-1', 1000, 'plataforma');
      const [, params] = (pg.queryOne as Mock).mock.calls[1];
      expect(params.slice(4, 7)).toEqual([
        'plataforma', 'plataforma', '00000000-0000-0000-0000-000000000001',
      ]);
    });
  });

  describe('prepararEnviosDeOrden', () => {
    it('agrupa lineas por empresa y congela la modalidad vigente', async () => {
      const { servicio } = crear();
      vi.spyOn(servicio, 'consultarOrden').mockResolvedValue({
        id: 'o1', cliente_id: 'c1', total: '35.00', total_cents: 3500, estado: 'pagada',
        items: [
          { sku: 'A', cantidad: 2, producto_nombre: 'A', empresa_id: 'e1', precio_unitario_cents: 1000 },
          { sku: 'B', cantidad: 1, producto_nombre: 'B', empresa_id: 'e1', precio_unitario_cents: 500 },
          { sku: 'C', cantidad: 1, producto_nombre: 'C', empresa_id: 'e2', precio_unitario_cents: 1000 },
        ],
      });
      vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => ({
        ok: true,
        json: async () => ({ id: String(url).includes('/e1') ? 'e1' : 'e2', modalidad_logistica: 'hibrida' }),
      } as Response));
      const preparar = vi.spyOn(servicio, 'prepararEnvio')
        .mockResolvedValueOnce({ ...envio, empresa_id: 'e1' } as never)
        .mockResolvedValueOnce({ ...envio, empresa_id: 'e2' } as never);

      await expect(servicio.prepararEnviosDeOrden('o1')).resolves.toHaveLength(2);
      expect(preparar).toHaveBeenNthCalledWith(1, 'o1', 'e1', 2500, 'hibrida');
      expect(preparar).toHaveBeenNthCalledWith(2, 'o1', 'e2', 1000, 'hibrida');
    });

    it('rechaza una orden sin lineas o con linea sin empresa', async () => {
      const { servicio } = crear();
      vi.spyOn(servicio, 'consultarOrden').mockResolvedValueOnce(null).mockResolvedValueOnce({
        id: 'o1', cliente_id: 'c1', total: '1.00', total_cents: 100, estado: 'pagada',
        items: [{ sku: 'A', cantidad: 1, producto_nombre: 'A', empresa_id: '', precio_unitario_cents: 100 }],
      });
      await expect(servicio.prepararEnviosDeOrden('o1')).rejects.toThrow(/no tiene líneas/);
      await expect(servicio.prepararEnviosDeOrden('o1')).rejects.toThrow(/no tiene empresa/);
    });

    it('falla de forma controlada si identidad no resuelve la empresa', async () => {
      const { servicio } = crear();
      vi.spyOn(servicio, 'consultarOrden').mockResolvedValue({
        id: 'o1', cliente_id: 'c1', total: '1.00', total_cents: 100, estado: 'pagada',
        items: [{ sku: 'A', cantidad: 1, producto_nombre: 'A', empresa_id: 'e1', precio_unitario_cents: 100 }],
      });
      vi.spyOn(globalThis, 'fetch').mockResolvedValue({ ok: false } as Response);
      await expect(servicio.prepararEnviosDeOrden('o1')).rejects.toThrow(/modalidad logística/);
    });
  });

  describe('asignacion hibrida', () => {
    it('debe asignar la flota propia usando el tenant de la empresa', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, modalidad_logistica: 'hibrida', proveedor_tipo: null })
        .mockResolvedValueOnce({ ...envio, modalidad_logistica: 'hibrida' });
      await servicio.asignarProveedor('e1', 'empresa-1', 'empresa');
      expect(pg.queryOne).toHaveBeenNthCalledWith(
        2, expect.stringContaining('UPDATE logistics.envios'),
        ['empresa', 'empresa-1', 'e1', 'empresa-1'],
      );
      expect(pg.query).toHaveBeenCalledWith(
        expect.stringContaining('logistics.asignaciones_envio'),
        ['e1', null, 'empresa', 'empresa-1', 'empresario', null],
      );
    });

    it('debe impedir cambiar la asignacion fija de una modalidad propia', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(envio);
      await expect(servicio.asignarProveedor('e1', 'empresa-1', 'plataforma'))
        .rejects.toThrow(/Solo un envío híbrido/);
    });

    it('rechaza un envio ajeno o una actualizacion perdida', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.asignarProveedor('e-x', 'empresa-1', 'empresa'))
        .rejects.toThrow(/no pertenece/);
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, modalidad_logistica: 'hibrida' })
        .mockResolvedValueOnce(null);
      await expect(servicio.asignarProveedor('e1', 'empresa-1', 'plataforma', 'u-e'))
        .rejects.toThrow(/No se pudo asignar/);
    });
  });

  describe('avance aislado por flota', () => {
    it('debe avanzar la orden solo cuando todos sus envios alcanzan el estado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, estado: 'entregada' })
        .mockResolvedValueOnce({ total: 0 });
      const avanzarOrden = vi.spyOn(servicio, 'avanzarEstadoOrden').mockResolvedValue({ estado: 'entregada' } as never);

      await servicio.avanzarEstadoEnvio('e1', 'empresa-1', 'entregada', 'recibido');

      expect((pg.queryOne as Mock).mock.calls[0][1]).toEqual(['entregada', 'e1', 'empresa-1']);
      expect(avanzarOrden).toHaveBeenCalledWith('o1', 'entregada', 'recibido');
    });

    it('no debe avanzar la orden cuando el conteo de pendientes no responde', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, estado: 'entregada' })
        .mockResolvedValueOnce(null);
      const avanzarOrden = vi.spyOn(servicio, 'avanzarEstadoOrden');

      await servicio.avanzarEstadoEnvio('e1', 'empresa-1', 'entregada');
      expect(avanzarOrden).not.toHaveBeenCalled();
    });

    it('no debe avanzar la orden mientras otra empresa tenga un envio pendiente', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, estado: 'entregada' })
        .mockResolvedValueOnce({ total: 1 });
      const avanzarOrden = vi.spyOn(servicio, 'avanzarEstadoOrden');

      await servicio.avanzarEstadoEnvio('e1', 'empresa-1', 'entregada');
      expect(avanzarOrden).not.toHaveBeenCalled();
    });

    it('rechaza un envio que no pertenece a la flota', async () => {
      const { servicio } = crear();
      await expect(servicio.avanzarEstadoEnvio('e-x', 'flota-1', 'enviada'))
        .rejects.toThrow(/no pertenece/);
    });

    it('considera enviada o entregada como avance suficiente para despacho', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock)
        .mockResolvedValueOnce({ ...envio, estado: 'enviada' })
        .mockResolvedValueOnce({ total: 1 });
      await servicio.avanzarEstadoEnvio('e1', 'empresa-1', 'enviada');
      expect((pg.queryOne as Mock).mock.calls[1][1]).toEqual(['o1', ['enviada', 'entregada']]);
    });
  });

  it('debe listar envios de una empresa', async () => {
    const { servicio, pg } = crear();
    (pg.query as Mock).mockResolvedValue([envio]);
    await expect(servicio.deEmpresa('empresa-1')).resolves.toEqual([envio]);
  });

  describe('listar', () => {
    it('debe serializar el monto en centavos a texto', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([
        { ...envio, monto_cents: 12345 },
      ]);
      const [fila] = await servicio.listar();
      expect(fila.monto).toBe('123.45');
      expect(fila.guia).toBe('BGH-ABC-123456');
    });
  });

  describe('avanzarEstadoOrden (Tabla 13)', () => {
    const orden = {
      id: 'o1',
      cliente_id: 'c1',
      items: [],
      total: '1500.00',
      total_cents: 150000,
      estado: 'enviado',
    };

    it('debe reenviar el codigo y mensaje del orders cuando la transicion es rechazada', async () => {
      vi.spyOn(globalThis, 'fetch').mockResolvedValue({
        ok: false,
        json: async () => ({
          codigo: 'ESTADO_INVALIDO',
          mensaje: 'No puede ir de entregada a pagada.',
        }),
      } as unknown as Response);
      const { servicio } = crear();

      await expect(servicio.avanzarEstadoOrden('o1', 'pagada')).rejects.toThrow(
        /No puede ir de entregada a pagada/,
      );
      await expect(servicio.avanzarEstadoOrden('o1', 'pagada')).rejects.toHaveProperty(
        'codigo',
        'ESTADO_INVALIDO',
      );
    });

    it('debe usar el error generico cuando el orders responde sin cuerpo util', async () => {
      vi.spyOn(globalThis, 'fetch').mockResolvedValue({
        ok: false,
        json: async () => {
          throw new Error('sin json');
        },
      } as unknown as Response);
      const { servicio } = crear();
      await expect(servicio.avanzarEstadoOrden('o1', 'pagada')).rejects.toHaveProperty(
        'codigo',
        'TRANSICION_FALLIDA',
      );
    });

    it('debe llamar al endpoint interno por POST y devolver la orden cuando es aceptada', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: true, json: async () => orden } as unknown as Response);
      const { servicio } = crear();

      const resultado = await servicio.avanzarEstadoOrden('o1', 'enviado', 'salio de bodega');

      expect(resultado.estado).toBe('enviado');
      expect(fetchSpy).toHaveBeenCalledWith(
        expect.stringContaining('/internal/orders/o1/transicion?estado=enviado&motivo=salio%20de%20bodega'),
        expect.objectContaining({ method: 'POST' }),
      );
    });

    it('debe permitir al empresario avanzar una orden que contiene su empresa', async () => {
      const fetchSpy = vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: true, json: async () => orden } as unknown as Response);
      const { servicio } = crear();
      const consultar = vi.spyOn(servicio, 'consultarOrden').mockResolvedValue({
        ...orden,
        items: [{ empresa_id: 'empresa-1' }],
      } as never);

      await expect(servicio.avanzarEstadoOrden('o1', 'enviado', undefined, 'empresa-1')).resolves.toEqual(
        orden,
      );
      expect(consultar).toHaveBeenCalledWith('o1');
      expect(fetchSpy).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar al empresario cuando la orden es de otra empresa', async () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch');
      const { servicio } = crear();
      vi.spyOn(servicio, 'consultarOrden').mockResolvedValue({
        ...orden,
        items: [{ empresa_id: 'empresa-2' }],
      } as never);

      await expect(
        servicio.avanzarEstadoOrden('o1', 'enviado', undefined, 'empresa-1'),
      ).rejects.toHaveProperty('codigo', 'ACCESO_DENEGADO');
      expect(fetchSpy).not.toHaveBeenCalled();
    });

    it('debe lanzar NotFound cuando el empresario intenta avanzar una orden inexistente', async () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch');
      const { servicio } = crear();
      vi.spyOn(servicio, 'consultarOrden').mockResolvedValue(null);

      await expect(
        servicio.avanzarEstadoOrden('o1', 'enviado', undefined, 'empresa-1'),
      ).rejects.toHaveProperty('codigo', 'NO_ENCONTRADO');
      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });

  describe('consultarOrden', () => {
    it('debe devolver null cuando el orders responde con error', async () => {
      vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: false, json: async () => ({}) } as unknown as Response);
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toBeNull();
    });

    it('debe devolver null cuando la red falla', async () => {
      vi
        .spyOn(globalThis, 'fetch')
        .mockRejectedValue(new Error('red caida'));
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toBeNull();
    });

    it('debe devolver la orden cuando el orders responde ok', async () => {
      vi
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: true, json: async () => ({ id: 'o1', estado: 'enviado' }) } as unknown as Response);
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toEqual({ id: 'o1', estado: 'enviado' });
    });
  });

  describe('deOrden', () => {
    it('debe consultar el envio por order_id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(envio);
      expect(await servicio.deOrden('o1')).toEqual(envio);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('logistics.envios'), ['o1']);
    });
  });
});
