import { EnviosService } from './envios.service';

/**
 * Logistica: guia de despacho idempotente al procesarse el pago,
 * avance de estado via orders (REST mocked, R-QA-2) y consulta de orden.
 */
describe('EnviosService (guia de despacho y transiciones)', () => {
  const crear = () => {
    const pg = {
      query: jest.fn().mockResolvedValue([]),
      queryOne: jest.fn().mockResolvedValue(null),
    };
    const servicio = new EnviosService(pg as never);
    return { servicio, pg };
  };

  const envio = {
    id: 'e1',
    guia: 'BGH-ABC-123456',
    order_id: 'o1',
    estado: 'en_preparacion',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  afterEach(() => jest.restoreAllMocks());

  describe('prepararEnvio (payment.procesado)', () => {
    it('debe devolver el envio existente sin insertar nada (idempotencia del consumidor)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(envio);
      const resultado = await servicio.prepararEnvio('o1', 150000);
      expect(resultado).toEqual(envio);
      expect(pg.queryOne).toHaveBeenCalledTimes(1); // solo el SELECT de busqueda
      const [sql] = (pg.queryOne as jest.Mock).mock.calls[0];
      expect(sql).toContain('SELECT');
    });

    it('debe crear el envio en preparacion con la guia cuando no existe', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(null) // no existe
        .mockResolvedValueOnce(envio); // INSERT ... RETURNING
      const resultado = await servicio.prepararEnvio('o1', 150000);
      expect(resultado.guia).toMatch(/^BGH-/); // guia con prefijo del proyecto
      const [sql, params] = (pg.queryOne as jest.Mock).mock.calls[1];
      expect(sql).toContain('INSERT INTO logistics.envios');
      expect(sql).toContain("'en_preparacion'");
      expect(params[2]).toBe('o1');
      expect(params[3]).toBe(150000); // monto en centavos (A02)
    });

    it('debe lanzar DomainError cuando el INSERT no devuelve fila', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);
      await expect(servicio.prepararEnvio('o1', 1)).rejects.toThrow(/no se pudo preparar/i);
    });
  });

  describe('listar', () => {
    it('debe serializar el monto en centavos a texto', async () => {
      const { servicio, pg } = crear();
      (pg.query as jest.Mock).mockResolvedValue([
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
      jest.spyOn(globalThis, 'fetch').mockResolvedValue({
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
      jest.spyOn(globalThis, 'fetch').mockResolvedValue({
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
      const fetchSpy = jest
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
  });

  describe('consultarOrden', () => {
    it('debe devolver null cuando el orders responde con error', async () => {
      jest
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: false, json: async () => ({}) } as unknown as Response);
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toBeNull();
    });

    it('debe devolver null cuando la red falla', async () => {
      jest
        .spyOn(globalThis, 'fetch')
        .mockRejectedValue(new Error('red caida'));
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toBeNull();
    });

    it('debe devolver la orden cuando el orders responde ok', async () => {
      jest
        .spyOn(globalThis, 'fetch')
        .mockResolvedValue({ ok: true, json: async () => ({ id: 'o1', estado: 'enviado' }) } as unknown as Response);
      const { servicio } = crear();
      expect(await servicio.consultarOrden('o1')).toEqual({ id: 'o1', estado: 'enviado' });
    });
  });

  describe('deOrden', () => {
    it('debe consultar el envio por order_id', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(envio);
      expect(await servicio.deOrden('o1')).toEqual(envio);
      expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('logistics.envios'), ['o1']);
    });
  });
});
