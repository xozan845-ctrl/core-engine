import { Readable } from 'stream';
import { PublicStorageController } from './public.controller';

describe('PublicStorageController (ADR-28)', () => {
  const crear = () => {
    const objetos = {
      leerPublico: vi.fn().mockResolvedValue({
        stream: Readable.from(['png']),
        content_type: 'image/png',
        tamano_bytes: 3,
        etag: 'etag-1',
        nombre: 'foto.png',
      }),
    };
    const res = { setHeader: vi.fn() };
    return { controller: new PublicStorageController(objetos as never), objetos, res };
  };

  it('debe servir la imagen con cache inmutable y ETag', async () => {
    const { controller, res } = crear();
    const archivo = await controller.servir('22222222-2222-4222-8222-222222222222', res as never);

    expect(archivo.getStream()).toBeDefined();
    expect(res.setHeader).toHaveBeenCalledWith('Content-Type', 'image/png');
    expect(res.setHeader).toHaveBeenCalledWith('Content-Length', '3');
    expect(res.setHeader).toHaveBeenCalledWith(
      'Cache-Control',
      'public, max-age=31536000, immutable',
    );
    expect(res.setHeader).toHaveBeenCalledWith('ETag', '"etag-1"');
  });

  it('debe propagar el 404 del servicio para objetos privados o inexistentes', async () => {
    const { controller, objetos, res } = crear();
    objetos.leerPublico.mockRejectedValue(new Error('no encontrado'));
    await expect(controller.servir('id', res as never)).rejects.toThrow('no encontrado');
  });
});
