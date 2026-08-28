import { Controller, Get, Param, UseGuards } from '@nestjs/common';
import { ClaveInternaGuard, NotFoundError, UsuarioActual, UsuarioContexto } from '@core/shared';
import { UsuariosService } from './usuarios.service';

/**
 * Gestion de usuarios (sustituye `gestion-usuarios.functions.adapter` de Firebase).
 * Las rutas estan protegidas por rol en el API Gateway (solo admin).
 */
@Controller('api/v1/usuarios')
export class UsuariosController {
  constructor(private readonly usuarios: UsuariosService) {}

  /** GET /api/v1/usuarios — lista (admin). */
  @Get()
  async listar(): Promise<ReturnType<UsuariosService['listar']>> {
    return this.usuarios.listar();
  }

  /** GET /api/v1/usuarios/:id — perfil (admin o el propio usuario). */
  @Get(':id')
  async obtener(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Awaited<ReturnType<UsuariosService['encontrarPorId']>>> {
    if (usuario?.rol !== 'admin' && usuario?.user_id !== id) {
      throw new NotFoundError('Usuario', id);
    }
    const encontrado = await this.usuarios.encontrarPorId(id);
    if (!encontrado) throw new NotFoundError('Usuario', id);
    return encontrado;
  }
}
