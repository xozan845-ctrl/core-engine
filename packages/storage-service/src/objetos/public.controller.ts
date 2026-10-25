import { Controller, Get, Param, Res, StreamableFile } from '@nestjs/common';
import { ApiNotFoundResponse, ApiOkResponse, ApiOperation, ApiParam, ApiTags } from '@nestjs/swagger';
import type { Response } from 'express';
import { ObjetosService } from './objetos.service';

/**
 * Servido estático de imágenes públicas (ADR-28). Sin autenticación: el UUID
 * del objeto es la capacidad. Solo se sirven objetos `activo`, `publico` y de
 * tipo imagen; el resto responde 404.
 */
@ApiTags('Almacenamiento público')
@Controller('api/v1/storage/public')
export class PublicStorageController {
  constructor(private readonly objetos: ObjetosService) {}

  @Get('objetos/:id')
  @ApiOperation({ summary: 'Servir una imagen pública (cacheable, sin auth)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @ApiOkResponse({
    description: 'Imagen (binario)',
    content: { 'image/*': { schema: { type: 'string', format: 'binary' } } },
  })
  @ApiNotFoundResponse({ description: 'Objeto inexistente, privado, eliminado o no imagen' })
  async servir(
    @Param('id') id: string,
    @Res({ passthrough: true }) res: Response,
  ): Promise<StreamableFile> {
    const objeto = await this.objetos.leerPublico(id);
    const etag = objeto.etag
      ? objeto.etag.startsWith('"')
        ? objeto.etag
        : `"${objeto.etag}"`
      : undefined;

    res.setHeader('Content-Type', objeto.content_type);
    res.setHeader('Content-Length', String(objeto.tamano_bytes));
    res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Content-Disposition', 'inline');
    if (etag) res.setHeader('ETag', etag);

    return new StreamableFile(objeto.stream);
  }
}
