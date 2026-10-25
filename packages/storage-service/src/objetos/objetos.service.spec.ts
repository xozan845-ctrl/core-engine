import type { Mock } from 'vitest';
import { ObjetosService } from './objetos.service';
import {
  ConflictError,
  DomainError,
  EVENTOS,
  ForbiddenError,
  NotFoundError,
  ROLES,
  UsuarioContexto,
} from '@core/shared';
import type { StorageConfig } from '../config/storage.config';
import type { BucketsService } from '../buckets/buckets.service';

const TENANT = '11111111-1111-4111-8111-111111111111';
const OBJETO_ID = '22222222-2222-4222-8222-222222222222';
const BUCKET = `t-${TENANT}`;

const filaObjeto = (extra: Record<string, unknown> = {}) => ({
  id: OBJETO_ID,
  tenant_id: TENANT,
  bucket: BUCKET,
  objeto: 'productos/22222222-2222-4222-8222-222222222222/foto.png',
  nombre_original: 'foto.png',
  content_type: 'image/png',
  carpeta: 'productos',
  descripcion: 'foto del producto',
  etiquetas: { origen: 'test' },
  tamano_bytes: '2048',
  etag: null,
  estado: 'pendiente',
  publico: false,
  subido_por: 'usuario-1',
  creado_en: '2026-10-16T00:00:00.000Z',
  confirmado_en: null,
  eliminado_en: null,
  ...extra,
});

const estadoBucket = (extra: Record<string, unknown> = {}) => ({
  tenant_id: TENANT,
  bucket: BUCKET,
  cuota_bytes: 100000,
  usado_bytes: 1000,
  disponible_bytes: 99000,
  objetos: 1,
  estado: 'activo' as const,
  creado_en: '2026-10-16T00:00:00.000Z',
  actualizado_en: '2026-10-16T00:00:00.000Z',
  ...extra,
});

const crear = () => {
  const client = {
    query: vi.fn().mockResolvedValue({ rows: [filaObjeto({ estado: 'activo' })], rowCount: 1 }),
  };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const buckets = {
    asegurar: vi.fn().mockResolvedValue(estadoBucket()),
    estado: vi.fn().mockResolvedValue(estadoBucket()),
    verificarCuota: vi.fn(),
    sumarUsoEnTransaccion: vi.fn().mockResolvedValue(undefined),
    restarUsoEnTransaccion: vi.fn().mockResolvedValue(undefined),
  };
  const outbox = {
    insertarEnTransaccion: vi.fn().mockResolvedValue('evt-1'),
    insertar: vi.fn(),
  };
  const almacen = {
    asegurarBucket: vi.fn(),
    existeBucket: vi.fn(),
    urlSubida: vi.fn().mockResolvedValue('http://minio/firmada'),
    urlDescarga: vi.fn().mockResolvedValue('http://minio/descarga'),
    estadisticas: vi.fn().mockResolvedValue({
      bucket: BUCKET,
      objeto: 'productos/22222222-2222-4222-8222-222222222222/foto.png',
      tamano_bytes: 2048,
      content_type: 'image/png',
      etag: 'etag-1',
    }),
    leer: vi.fn().mockResolvedValue({
      stream: { pipe: vi.fn() },
      tamano_bytes: 2048,
      content_type: 'image/png',
      etag: 'etag-1',
    }),
    eliminar: vi.fn().mockResolvedValue(undefined),
  };
  const config = {
    endpoint: 'minio',
    puerto: 9000,
    usarSsl: false,
    accessKey: 'u',
    secretKey: 'p',
    prefijoBucket: 't-',
    maxObjetoBytes: 10 * 1024 * 1024,
    cuotaDefaultBytes: 100000,
    urlTtlSegundos: 900,
    contentTypesPermitidos: ['image/png', 'application/pdf'],
  } as StorageConfig;
  return {
    servicio: new ObjetosService(
      pg as never,
      buckets as unknown as BucketsService,
      outbox as never,
      almacen as never,
      config,
    ),
    pg,
    buckets,
    outbox,
    almacen,
    client,
  };
};

const dto = (extra: Record<string, unknown> = {}) => ({
  nombre: 'foto.png',
  content_type: 'image/png',
  tamano_bytes: 2048,
  carpeta: 'productos',
  descripcion: 'foto del producto',
  etiquetas: { origen: 'test' },
  ...extra,
});

describe('ObjetosService (URLs prefirmadas y cuotas por tenant)', () => {
  describe('solicitarSubida', () => {
    it('debe rechazar content-types fuera de la whitelist', async () => {
      const { servicio, pg } = crear();
      await expect(
        servicio.solicitarSubida(TENANT, 'usuario-1', dto({ content_type: 'application/x-msdownload' })),
      ).rejects.toThrow(/CONTENT_TYPE_NO_PERMITIDO|no esta permitido/);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar tamanos invalidos o no positivos', async () => {
      const { servicio } = crear();
      await expect(
        servicio.solicitarSubida(TENANT, 'usuario-1', dto({ tamano_bytes: 0 })),
      ).rejects.toThrow(DomainError);
      await expect(
        servicio.solicitarSubida(TENANT, 'usuario-1', dto({ tamano_bytes: 1.5 })),
      ).rejects.toThrow(DomainError);
    });

    it('debe rechazar objetos por encima del maximo', async () => {
      const { servicio } = crear();
      await expect(
        servicio.solicitarSubida(TENANT, 'usuario-1', dto({ tamano_bytes: 20 * 1024 * 1024 })),
      ).rejects.toThrow(/OBJETO_DEMASIADO_GRANDE|excede el maximo/);
    });

    it('debe propagar el rechazo de cuota del tenant', async () => {
      const { servicio, buckets, pg } = crear();
      (buckets.verificarCuota as Mock).mockImplementation(() => {
        throw new DomainError('CUOTA_EXCEDIDA', 'La operacion excede la cuota.');
      });
      await expect(servicio.solicitarSubida(TENANT, 'usuario-1', dto())).rejects.toThrow(
        /CUOTA_EXCEDIDA|cuota/,
      );
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe registrar el objeto pendiente y devolver la URL prefirmada', async () => {
      const { servicio, pg, buckets, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      const autor = '33333333-3333-4333-8333-333333333333';

      const solicitud = await servicio.solicitarSubida(TENANT, autor, dto());

      expect(buckets.asegurar).toHaveBeenCalledWith(TENANT);
      const [sql, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(sql).toContain('INSERT INTO storage.objetos');
      expect(params[1]).toBe(TENANT);
      expect(params[3]).toMatch(/^productos\/[0-9a-f-]{36}\/foto\.png$/);
      expect(params[4]).toBe('foto.png');
      expect(params[5]).toBe('image/png');
      expect(params[8]).toBe(JSON.stringify({ origen: 'test' }));
      expect(params[10]).toBe(autor);
      expect(almacen.urlSubida).toHaveBeenCalledWith({
        bucket: BUCKET,
        objeto: expect.stringMatching(/^productos\/[0-9a-f-]{36}\/foto\.png$/),
        content_type: 'image/png',
        expira_segundos: 900,
      });
      expect(solicitud).toMatchObject({
        objeto_id: OBJETO_ID,
        bucket: BUCKET,
        url_subida: 'http://minio/firmada',
        metodo: 'PUT',
        headers: { 'Content-Type': 'image/png' },
        max_bytes: 10 * 1024 * 1024,
      });
      expect(new Date(solicitud.expira_en).getTime()).toBeGreaterThan(Date.now());
    });

    it('debe sanear el nombre (sin rutas) y usar la carpeta por defecto', async () => {      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());

      await servicio.solicitarSubida(
        TENANT,
        undefined,
        dto({ nombre: '../../etc/passwd', carpeta: '', etiquetas: 'no-es-objeto' }),
      );

      const [, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(params[3]).toMatch(/^general\/[0-9a-f-]{36}\/passwd$/);
      expect(params[4]).toBe('passwd');
      expect(params[8]).toBe('{}');
      expect(params[10]).toBeNull();
    });

    it('no debe romper si el id del autor no es uuid (contrato del gateway)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());

      await servicio.solicitarSubida(TENANT, 'no-es-uuid', dto());

      const [, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(params[10]).toBeNull();
    });
  });

  describe('confirmar', () => {
    it('debe fallar si el objeto no existe o el id no es uuid', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.confirmar(TENANT, 'no-uuid')).rejects.toThrow(NotFoundError);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });

    it('debe rechazar confirmar un objeto eliminado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'eliminado' }));
      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(ConflictError);
    });

    it('debe ser idempotente con un objeto ya activo', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo' }));

      const objeto = await servicio.confirmar(TENANT, OBJETO_ID);

      expect(objeto.estado).toBe('activo');
      expect(almacen.estadisticas).not.toHaveBeenCalled();
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe fallar cuando la subida no llego a MinIO', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      (almacen.estadisticas as Mock).mockResolvedValue(null);

      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(NotFoundError);
    });

    it('debe borrar y rechazar un objeto vacio', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      (almacen.estadisticas as Mock).mockResolvedValue({ bucket: BUCKET, objeto: 'x', tamano_bytes: 0 });

      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(/OBJETO_VACIO|vacio/);
      expect(almacen.eliminar).toHaveBeenCalledWith(BUCKET, 'productos/22222222-2222-4222-8222-222222222222/foto.png');
    });

    it('debe borrar y rechazar un objeto que excede el maximo', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      (almacen.estadisticas as Mock).mockResolvedValue({
        bucket: BUCKET,
        objeto: 'x',
        tamano_bytes: 20 * 1024 * 1024,
        content_type: 'image/png',
      });

      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(/OBJETO_DEMASIADO_GRANDE|excede el maximo/);
      expect(almacen.eliminar).toHaveBeenCalled();
    });

    it('debe borrar y rechazar un content-type real no permitido', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      (almacen.estadisticas as Mock).mockResolvedValue({
        bucket: BUCKET,
        objeto: 'x',
        tamano_bytes: 2048,
        content_type: 'application/x-msdownload',
      });

      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(/CONTENT_TYPE_NO_PERMITIDO|no permitido/);
      expect(almacen.eliminar).toHaveBeenCalled();
    });

    it('debe revalidar la cuota con el tamano real antes de contabilizar', async () => {
      const { servicio, pg, buckets, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      (almacen.estadisticas as Mock).mockResolvedValue({
        bucket: BUCKET,
        objeto: 'x',
        tamano_bytes: 5000,
        content_type: 'image/png',
      });
      (buckets.verificarCuota as Mock).mockImplementation(() => {
        throw new DomainError('CUOTA_EXCEDIDA', 'La operacion excede la cuota.');
      });

      await expect(servicio.confirmar(TENANT, OBJETO_ID)).rejects.toThrow(/CUOTA_EXCEDIDA|cuota/);
      expect(pg.transaccion).not.toHaveBeenCalled();
    });

    it('debe activar el objeto, contabilizar cuota y emitir el evento', async () => {
      const { servicio, pg, buckets, outbox, client } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());

      const objeto = await servicio.confirmar(TENANT, OBJETO_ID);

      const [sql, params] = client.query.mock.calls[0];
      expect(sql).toContain("SET estado = 'activo'");
      expect(params).toEqual([TENANT, OBJETO_ID, 2048, 'image/png', 'etag-1']);
      expect(buckets.sumarUsoEnTransaccion).toHaveBeenCalledWith(client, TENANT, 2048);
      expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
        client,
        EVENTOS.STORAGE_OBJETO_SUBIDO,
        expect.objectContaining({
          objeto_id: OBJETO_ID,
          tenant_id: TENANT,
          bucket: BUCKET,
          tamano_bytes: 2048,
          subido_por: 'usuario-1',
        }),
      );
      expect(objeto.estado).toBe('activo');
    });

    it('debe devolver el estado actual si otra confirmacion gano la carrera', async () => {
      const { servicio, pg, client } = crear();
      (pg.queryOne as Mock).mockResolvedValueOnce(filaObjeto());
      client.query.mockResolvedValue({ rows: [], rowCount: 0 });
      (pg.queryOne as Mock).mockResolvedValueOnce(filaObjeto({ estado: 'activo' }));

      const objeto = await servicio.confirmar(TENANT, OBJETO_ID);

      expect(objeto.estado).toBe('activo');
      expect(pg.queryOne).toHaveBeenCalledTimes(2);
    });
  });

  describe('listar', () => {
    it('debe excluir los eliminados cuando no se filtra por estado', async () => {
      const { servicio, pg } = crear();
      (pg.query as Mock).mockResolvedValue([filaObjeto()]);

      const pagina = await servicio.listar(TENANT, { pagina: 2, limite: 5 });

      const [sql, params] = (pg.query as Mock).mock.calls[0];
      expect(sql).toContain("estado <> 'eliminado'");
      expect(params).toEqual([TENANT, 5, 5]);
      expect(pagina.items[0].nombre).toBe('foto.png');
    });

    it('debe aplicar filtros de estado, carpeta y busqueda', async () => {
      const { servicio, pg } = crear();
      const pagina = await servicio.listar(TENANT, {
        estado: 'activo',
        carpeta: 'PRODUCTOS',
        q: 'FOTO',
        pagina: 1,
        limite: 10,
      });

      const [sql, params] = (pg.query as Mock).mock.calls[0];
      expect(sql).toContain('estado = $2');
      expect(sql).toContain('carpeta = $3');
      expect(sql).toContain('LOWER(nombre_original) LIKE $4');
      expect(params).toEqual([TENANT, 'activo', 'productos', '%foto%', 10, 0]);
      expect(pagina.total).toBe(0);
    });
  });

  describe('urlDescarga', () => {
    it('debe firmar la descarga del objeto del tenant', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo' }));

      const descarga = await servicio.urlDescarga(TENANT, OBJETO_ID);

      expect(almacen.urlDescarga).toHaveBeenCalledWith(BUCKET, 'productos/22222222-2222-4222-8222-222222222222/foto.png', 900);
      expect(descarga).toMatchObject({
        url_descarga: 'http://minio/descarga',
        nombre: 'foto.png',
        content_type: 'image/png',
        tamano_bytes: 2048,
      });
    });

    it('no debe firmar descargas de objetos eliminados', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'eliminado' }));

      await expect(servicio.urlDescarga(TENANT, OBJETO_ID)).rejects.toThrow(NotFoundError);
      expect(almacen.urlDescarga).not.toHaveBeenCalled();
    });
  });

  describe('eliminar', () => {
    const usuario = (extra: Partial<UsuarioContexto> = {}): UsuarioContexto => ({
      user_id: 'usuario-1',
      email: 'u@core-engine.test',
      rol: ROLES.VENDEDOR,
      ...extra,
    });

    it('debe impedir que otro usuario borre una subida ajena', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo' }));

      await expect(
        servicio.eliminar(TENANT, OBJETO_ID, usuario({ user_id: 'otro' })),
      ).rejects.toThrow(ForbiddenError);
    });

    it('debe permitir al admin borrar cualquier objeto del tenant', async () => {
      const { servicio, pg, almacen, outbox, buckets } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo' }));

      await servicio.eliminar(TENANT, OBJETO_ID, usuario({ rol: ROLES.ADMIN }));

      expect(almacen.eliminar).toHaveBeenCalled();
      expect(buckets.restarUsoEnTransaccion).toHaveBeenCalledWith(expect.anything(), TENANT, 2048);
      expect(outbox.insertarEnTransaccion).toHaveBeenCalledWith(
        expect.anything(),
        EVENTOS.STORAGE_OBJETO_ELIMINADO,
        expect.objectContaining({ objeto_id: OBJETO_ID }),
      );
    });

    it('debe ser idempotente con un objeto ya eliminado', async () => {
      const { servicio, pg, almacen, outbox } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'eliminado' }));

      await servicio.eliminar(TENANT, OBJETO_ID, usuario());

      expect(almacen.eliminar).not.toHaveBeenCalled();
      expect(outbox.insertarEnTransaccion).not.toHaveBeenCalled();
    });

    it('no debe descontar cuota por un objeto pendiente (nunca contabilizado)', async () => {
      const { servicio, pg, buckets } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'pendiente' }));

      await servicio.eliminar(TENANT, OBJETO_ID, usuario());

      expect(buckets.restarUsoEnTransaccion).not.toHaveBeenCalled();
      expect((pg.query as Mock).mock.calls.length).toBe(0);
    });
  });

  describe('serializacion de objetos', () => {
    it('debe normalizar etiquetas nulas, estado eliminado y publico ausente', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(
        filaObjeto({ etiquetas: null, estado: 'eliminado', publico: undefined }),
      );

      const objeto = await servicio.obtener(TENANT, OBJETO_ID);

      expect(objeto.etiquetas).toEqual({});
      expect(objeto.estado).toBe('eliminado');
      expect(objeto.publico).toBe(false);
    });

    it('debe descartar etiquetas con clave vacia', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto());
      const resultado = await servicio.solicitarSubida(TENANT, undefined, {
        ...dto(),
        etiquetas: { '   ': 'descartada', valida: 'ok' },
      });
      expect(resultado.objeto_id).toBeDefined();
    });
  });

  describe('leerPublico (ADR-28)', () => {
    it('debe servir un objeto activo, público y de imagen', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo', publico: true }));

      const objeto = await servicio.leerPublico(OBJETO_ID);

      expect(objeto.content_type).toBe('image/png');
      expect(objeto.tamano_bytes).toBe(2048);
      expect(almacen.leer).toHaveBeenCalledWith(
        BUCKET,
        'productos/22222222-2222-4222-8222-222222222222/foto.png',
      );
      const [sql, params] = (pg.queryOne as Mock).mock.calls[0];
      expect(sql).toContain('publico = true');
      expect(params).toEqual([OBJETO_ID]);
    });

    it('debe ocultar un objeto privado o inexistente', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(null);
      await expect(servicio.leerPublico(OBJETO_ID)).rejects.toThrow(NotFoundError);
    });

    it('debe ocultar el objeto si MinIO ya no lo tiene', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(filaObjeto({ estado: 'activo', publico: true }));
      (almacen.leer as Mock).mockResolvedValue(null);
      await expect(servicio.leerPublico(OBJETO_ID)).rejects.toThrow(NotFoundError);
    });

    it('debe usar el ETag de la fila si MinIO no lo devuelve', async () => {
      const { servicio, pg, almacen } = crear();
      (pg.queryOne as Mock).mockResolvedValue(
        filaObjeto({ estado: 'activo', publico: true, etag: 'etag-fila' }),
      );
      (almacen.leer as Mock).mockResolvedValue({
        stream: { pipe: vi.fn() },
        tamano_bytes: 2048,
      });
      const objeto = await servicio.leerPublico(OBJETO_ID);
      expect(objeto.etag).toBe('etag-fila');
    });

    it('debe ocultar un objeto que no es imagen', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as Mock).mockResolvedValue(
        filaObjeto({ estado: 'activo', publico: true, content_type: 'application/pdf' }),
      );
      await expect(servicio.leerPublico(OBJETO_ID)).rejects.toThrow(NotFoundError);
    });

    it('debe rechazar un id que no es UUID sin consultar', async () => {
      const { servicio, pg } = crear();
      await expect(servicio.leerPublico('no-es-uuid')).rejects.toThrow(NotFoundError);
      expect(pg.queryOne).not.toHaveBeenCalled();
    });
  });
});
