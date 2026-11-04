import { Readable } from 'stream';
import { PublicStorageController } from './public.controller';

const ID = '22222222-2222-4222-8222-222222222222';

describe('PublicStorageController (ADR-28, media + Range)', () => {
  const crear = () => {
    const objetos = {
      obtenerPublicoMeta: vi.fn().mockResolvedValue({
        content_type: 'image/png',
        tamano_bytes: 3,
        etag: 'etag-1',
        nombre: 'foto.png',
      }),
      leerPublico: vi.fn().mockResolvedValue({
        stream: Readable.from(['png']),
        content_type: 'image/png',
        tamano_bytes: 3,
        etag: 'etag-1',
        nombre: 'foto.png',
      }),
    };
    const res = { setHeader: vi.fn(), status: vi.fn().mockReturnThis() };
    return { controller: new PublicStorageController(objetos as never), objetos, res };
  };

  it('debe servir la imagen completa con cache inmutable, ETag y Accept-Ranges', async () => {
    const { controller, objetos, res } = crear();
    const archivo = await controller.servir(ID, undefined, res as never);

    expect(archivo?.getStream()).toBeDefined();
    expect(objetos.leerPublico).toHaveBeenCalledWith(ID);
    expect(res.setHeader).toHaveBeenCalledWith('Content-Type', 'image/png');
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '3');
    expect(res.setHeader).toHaveBeenCalledWith('Accept-Ranges', 'bytes');
    expect(res.setHeader).toHaveBeenCalledWith(
      'Cache-Control',
      'public, max-age=31536000, immutable',
    );
    expect(res.setHeader).toHaveBeenCalledWith('ETag', '"etag-1"');
    expect(res.status).not.toHaveBeenCalled();
  });

  it('debe servir un video público completo', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'video/mp4',
      tamano_bytes: 2048,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });
    objetos.leerPublico.mockResolvedValue({
      stream: Readable.from(['mp4']),
      content_type: 'video/mp4',
      tamano_bytes: 2048,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });

    const archivo = await controller.servir(ID, undefined, res as never);

    expect(archivo?.getStream()).toBeDefined();
    expect(res.setHeader).toHaveBeenCalledWith('Content-Type', 'video/mp4');
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '2048');
  });

  it('debe responder 206 con Content-Range y abrir solo el tramo', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'video/mp4',
      tamano_bytes: 1000,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });
    objetos.leerPublico.mockResolvedValue({
      stream: Readable.from(['parcial']),
      content_type: 'video/mp4',
      tamano_bytes: 1000,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });

    const archivo = await controller.servir(ID, 'bytes=10-19', res as never);

    expect(archivo?.getStream()).toBeDefined();
    expect(res.status).toHaveBeenCalledWith(206);
    expect(res.setHeader).toHaveBeenCalledWith('Content-Range', 'bytes 10-19/1000');
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '10');
    expect(objetos.leerPublico).toHaveBeenCalledWith(ID, { inicio: 10, fin: 19 });
  });

  it('debe conservar un ETag ya entrecomillado sin duplicar comillas', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'image/png',
      tamano_bytes: 3,
      etag: '"etag-entrecomillado"',
      nombre: 'foto.png',
    });

    await controller.servir(ID, undefined, res as never);

    expect(res.setHeader).toHaveBeenCalledWith('ETag', '"etag-entrecomillado"');
  });

  it('debe responder 416 y no abrir stream cuando el rango es inválido', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'video/mp4',
      tamano_bytes: 1000,
      nombre: 'fondo.mp4',
    });

    const archivo = await controller.servir(ID, 'bytes=1000-', res as never);

    expect(archivo).toBeUndefined();
    expect(res.status).toHaveBeenCalledWith(416);
    expect(res.setHeader).toHaveBeenCalledWith('Content-Range', 'bytes */1000');
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '0');
    expect(objetos.leerPublico).not.toHaveBeenCalled();
  });

  it('debe responder HEAD solo con cabeceras, sin abrir el stream', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'video/mp4',
      tamano_bytes: 1000,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });

    await controller.cabeceras(ID, 'bytes=0-99', res as never);

    expect(res.status).toHaveBeenCalledWith(206);
    expect(res.setHeader).toHaveBeenCalledWith('Content-Range', 'bytes 0-99/1000');
    expect(objetos.leerPublico).not.toHaveBeenCalled();
  });

  it('debe responder HEAD completo con Content-Length y sin abrir el stream', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockResolvedValue({
      content_type: 'video/mp4',
      tamano_bytes: 1000,
      etag: 'etag-video',
      nombre: 'fondo.mp4',
    });

    await controller.cabeceras(ID, undefined, res as never);

    expect(res.status).not.toHaveBeenCalled();
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '1000');
    expect(res.setHeader).toHaveBeenCalledWith('Accept-Ranges', 'bytes');
    expect(objetos.leerPublico).not.toHaveBeenCalled();
  });

  it('debe propagar el 404 del servicio para objetos privados o inexistentes', async () => {
    const { controller, objetos, res } = crear();
    objetos.obtenerPublicoMeta.mockRejectedValue(new Error('no encontrado'));
    await expect(controller.servir(ID, undefined, res as never)).rejects.toThrow('no encontrado');
  });
});
