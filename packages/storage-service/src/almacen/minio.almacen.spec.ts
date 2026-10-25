import { vi } from 'vitest';

const { cliente } = vi.hoisted(() => ({
  cliente: {
    bucketExists: vi.fn(),
    makeBucket: vi.fn(),
    presignedPutObject: vi.fn(),
    presignedGetObject: vi.fn(),
    statObject: vi.fn(),
    getObject: vi.fn(),
    removeObject: vi.fn(),
  },
}));

vi.mock('minio', () => ({
  // Vitest 5 exige que el mock de una clase use `function` o `class` para poder
  // invocarse con `new`; devolver el objeto emula la instancia real.
  Client: vi.fn(function ClienteMock() {
    return cliente;
  }),
}));

import { Client } from 'minio';
import { MinioAlmacen } from './minio.almacen';
import { StorageConfig } from '../config/storage.config';

const config: StorageConfig = {
  endpoint: 'minio',
  puerto: 9000,
  usarSsl: false,
  publicEndpoint: 'almacen.kbcoleccion.com',
  publicPuerto: 443,
  publicUsarSsl: true,
  accessKey: 'usuario',
  secretKey: 'secreto',
  region: 'us-east-1',
  prefijoBucket: 't-',
  maxObjetoBytes: 1048576,
  cuotaDefaultBytes: 2097152,
  urlTtlSegundos: 900,
  contentTypesPermitidos: ['image/png'],
};

const errorConCodigo = (code: string): Error => Object.assign(new Error(code), { code });

describe('MinioAlmacen (implementacion del puerto AlmacenObjetos, R-SB-6)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('debe exigir la credencial de MinIO al arrancar (onModuleInit)', () => {
    const sinCredenciales = new MinioAlmacen({ ...config, accessKey: '', secretKey: '' });
    expect(() => sinCredenciales.onModuleInit()).toThrow('MINIO_ROOT_USER y MINIO_ROOT_PASSWORD');

    const conCredenciales = new MinioAlmacen(config);
    expect(() => conCredenciales.onModuleInit()).not.toThrow();
  });

  it('debe crear el bucket la primera vez y memorizarlo para el resto del proceso', async () => {
    cliente.bucketExists.mockResolvedValue(false);
    cliente.makeBucket.mockResolvedValue(undefined);
    const almacen = new MinioAlmacen(config);

    await almacen.asegurarBucket('t-1111');
    await almacen.asegurarBucket('t-1111');

    expect(cliente.bucketExists).toHaveBeenCalledTimes(1);
    expect(cliente.makeBucket).toHaveBeenCalledTimes(1);
    expect(cliente.makeBucket).toHaveBeenCalledWith('t-1111');
  });

  it('no debe crear el bucket cuando ya existe en MinIO', async () => {
    cliente.bucketExists.mockResolvedValue(true);
    const almacen = new MinioAlmacen(config);

    await almacen.asegurarBucket('t-2222');

    expect(cliente.makeBucket).not.toHaveBeenCalled();
  });

  it('debe informar si un bucket existe', async () => {
    cliente.bucketExists.mockResolvedValue(true);
    const almacen = new MinioAlmacen(config);

    await expect(almacen.existeBucket('t-3333')).resolves.toBe(true);
  });

  it('debe firmar la URL de subida con la expiracion solicitada', async () => {
    cliente.presignedPutObject.mockResolvedValue('http://minio/t-1/a.png');
    const almacen = new MinioAlmacen(config);

    const url = await almacen.urlSubida({
      bucket: 't-1',
      objeto: 'productos/1/a.png',
      content_type: 'image/png',
      expira_segundos: 120,
    });

    expect(url).toBe('http://minio/t-1/a.png');
    expect(cliente.presignedPutObject).toHaveBeenCalledWith('t-1', 'productos/1/a.png', 120);
  });

  it('debe firmar contra el endpoint PUBLICO (el que resuelve el cliente)', () => {
    new MinioAlmacen(config);

    expect(Client).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ endPoint: 'minio', port: 9000, useSSL: false }),
    );
    expect(Client).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        endPoint: 'almacen.kbcoleccion.com',
        port: 443,
        useSSL: true,
        accessKey: 'usuario',
      }),
    );
  });

  it('debe firmar la URL de descarga con la expiracion solicitada', async () => {
    cliente.presignedGetObject.mockResolvedValue('http://minio/t-1/a.png?firma');
    const almacen = new MinioAlmacen(config);

    await expect(almacen.urlDescarga('t-1', 'a.png', 60)).resolves.toBe(
      'http://minio/t-1/a.png?firma',
    );
    expect(cliente.presignedGetObject).toHaveBeenCalledWith('t-1', 'a.png', 60);
  });

  it('debe abrir el objeto para servirlo por streaming (ADR-28)', async () => {
    const stream = { pipe: vi.fn() };
    cliente.statObject.mockResolvedValue({
      size: 10,
      etag: 'etag-stream',
      metaData: { 'content-type': 'image/png' },
    });
    cliente.getObject.mockResolvedValue(stream);
    const almacen = new MinioAlmacen(config);

    await expect(almacen.leer('t-1', 'a.png')).resolves.toEqual({
      stream,
      tamano_bytes: 10,
      content_type: 'image/png',
      etag: 'etag-stream',
    });
    expect(cliente.getObject).toHaveBeenCalledWith('t-1', 'a.png');
  });

  it('debe leer un objeto sin metadata ni etag (ramas opcionales)', async () => {
    const stream = { pipe: vi.fn() };
    cliente.statObject.mockResolvedValue({ size: 1, metaData: null });
    cliente.getObject.mockResolvedValue(stream);
    const almacen = new MinioAlmacen(config);

    await expect(almacen.leer('t-1', 'a.bin')).resolves.toEqual({
      stream,
      tamano_bytes: 1,
    });
  });

  it('debe devolver null al leer un objeto inexistente', async () => {
    cliente.statObject.mockRejectedValue(errorConCodigo('NoSuchKey'));
    const almacen = new MinioAlmacen(config);
    await expect(almacen.leer('t-1', 'no-existe.png')).resolves.toBeNull();
  });

  it('debe propagar errores no relacionados con ausencia al leer', async () => {
    cliente.statObject.mockRejectedValue(new Error('sin conexion'));
    const almacen = new MinioAlmacen(config);
    await expect(almacen.leer('t-1', 'a.png')).rejects.toThrow('sin conexion');
  });

  it('debe devolver las estadisticas del objeto subido', async () => {
    cliente.statObject.mockResolvedValue({
      size: 2048,
      etag: 'abc123',
      metaData: { 'content-type': 'image/png' },
    });
    const almacen = new MinioAlmacen(config);

    await expect(almacen.estadisticas('t-1', 'a.png')).resolves.toEqual({
      bucket: 't-1',
      objeto: 'a.png',
      tamano_bytes: 2048,
      content_type: 'image/png',
      etag: 'abc123',
    });
  });

  it('debe devolver null cuando el objeto no existe (subida no completada)', async () => {
    cliente.statObject.mockRejectedValue(errorConCodigo('NoSuchKey'));
    const almacen = new MinioAlmacen(config);

    await expect(almacen.estadisticas('t-1', 'a.png')).resolves.toBeNull();
  });

  it('debe propagar los errores de MinIO que no son "no encontrado"', async () => {
    cliente.statObject.mockRejectedValue(new Error('conexion perdida'));
    const almacen = new MinioAlmacen(config);

    await expect(almacen.estadisticas('t-1', 'a.png')).rejects.toThrow('conexion perdida');
  });

  it('debe tolerar eliminar un objeto que ya no existe', async () => {
    cliente.removeObject.mockRejectedValue(errorConCodigo('NotFound'));
    const almacen = new MinioAlmacen(config);

    await expect(almacen.eliminar('t-1', 'a.png')).resolves.toBeUndefined();
  });

  it('debe propagar los errores al eliminar', async () => {
    cliente.removeObject.mockRejectedValue(new Error('permiso denegado'));
    const almacen = new MinioAlmacen(config);

    await expect(almacen.eliminar('t-1', 'a.png')).rejects.toThrow('permiso denegado');
  });
});
