import { Controller, Get, Head, Headers, Param, Res, StreamableFile } from '@nestjs/common';
import {
  ApiNotFoundResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import type { Response } from 'express';
import { ObjetosService } from './objetos.service';
import { interpretarRango } from './range';

/**
 * Servido estático de media pública (ADR-28). Sin autenticación: el UUID del
 * objeto es la capacidad. Solo se sirven objetos `activo`, `publico` y de tipo
 * imagen raster o video; el resto responde 404.
 *
 * Soporta `Range` de un solo tramo (206/416) y `HEAD`, requisito de los fondos
 * de video que el navegador reproduce por streaming.
 */
@ApiTags('Almacenamiento público')
@Controller('api/v2/storage/public')
export class PublicStorageController {
  constructor(private readonly objetos: ObjetosService) {}

  @Get('objetos/:id')
  @ApiOperation({ summary: 'Servir una imagen o video público (cacheable, sin auth, Range)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @ApiOkResponse({
    description: 'Media (binario); con Range responde 206',
    content: {
      'image/*': { schema: { type: 'string', format: 'binary' } },
      'video/*': { schema: { type: 'string', format: 'binary' } },
    },
  })
  @ApiResponse({
    status: 206,
    description: 'Tramo parcial solicitado con Range',
    headers: {
      'Content-Range': { description: 'Tramo servido y tamaño total', schema: { type: 'string' } },
      'Accept-Ranges': { description: 'Unidad de rangos soportada', schema: { type: 'string' } },
    },
  })
  @ApiResponse({
    status: 416,
    description: 'Rango inválido o fuera del objeto',
    headers: {
      'Content-Range': { description: 'Tamaño total del objeto', schema: { type: 'string' } },
    },
  })
  @ApiNotFoundResponse({ description: 'Objeto inexistente, privado, eliminado o no servible' })
  async servir(
    @Param('id') id: string,
    @Headers('range') range: string | undefined,
    @Res({ passthrough: true }) res: Response,
  ): Promise<StreamableFile | undefined> {
    return this.responder(id, range, res, false);
  }

  @Head('objetos/:id')
  @ApiOperation({ summary: 'Cabeceras de una imagen o video público (sin cuerpo)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @ApiOkResponse({
    description: 'Cabeceras de la media (sin cuerpo); con Range responde 206',
    content: {
      'image/*': { schema: { type: 'string', format: 'binary' } },
      'video/*': { schema: { type: 'string', format: 'binary' } },
    },
  })
  @ApiResponse({ status: 206, description: 'Cabeceras del tramo solicitado con Range' })
  @ApiResponse({ status: 416, description: 'Rango inválido o fuera del objeto' })
  @ApiNotFoundResponse({ description: 'Objeto inexistente, privado, eliminado o no servible' })
  async cabeceras(
    @Param('id') id: string,
    @Headers('range') range: string | undefined,
    @Res({ passthrough: true }) res: Response,
  ): Promise<void> {
    await this.responder(id, range, res, true);
  }

  private async responder(
    id: string,
    range: string | undefined,
    res: Response,
    soloCabeceras: boolean,
  ): Promise<StreamableFile | undefined> {
    // Primero los metadatos: el tamaño total define si el rango es válido.
    const meta = await this.objetos.obtenerPublicoMeta(id);
    const etag = meta.etag
      ? meta.etag.startsWith('"')
        ? meta.etag
        : `"${meta.etag}"`
      : undefined;

    res.setHeader('Content-Type', meta.content_type);
    res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Content-Disposition', 'inline');
    res.setHeader('Accept-Ranges', 'bytes');
    if (etag) res.setHeader('ETag', etag);

    const resultado = interpretarRango(range, meta.tamano_bytes);
    if (resultado.tipo === 'invalido') {
      res.status(416);
      res.setHeader('Content-Range', `bytes */${meta.tamano_bytes}`);
      res.setHeader('Content-Length', '0');
      return undefined;
    }

    if (resultado.tipo === 'completo') {
      res.setHeader('Content-Length', String(meta.tamano_bytes));
      if (soloCabeceras) return undefined;
      const objeto = await this.objetos.leerPublico(id);
      return new StreamableFile(objeto.stream);
    }

    const { inicio, fin } = resultado.rango;
    res.status(206);
    res.setHeader('Content-Range', `bytes ${inicio}-${fin}/${meta.tamano_bytes}`);
    res.setHeader('Content-Length', String(fin - inicio + 1));
    if (soloCabeceras) return undefined;
    const objeto = await this.objetos.leerPublico(id, { inicio, fin });
    return new StreamableFile(objeto.stream);
  }
}
