import { Controller, Get, Param } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiParam, ApiResponse, ApiTags } from '@nestjs/swagger';
import { NotFoundError, ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { UsuariosService } from './usuarios.service';
import { UsuarioResponseDto } from './usuarios.response.dto';

/**
 * Gestion de usuarios (sustituye `gestion-usuarios.functions.adapter` de Firebase).
 * Las rutas estan protegidas por rol en el API Gateway (solo admin).
 */
@ApiTags('Usuarios')
@ApiBearerAuth()
@Controller('api/v1/usuarios')
export class UsuariosController {
  constructor(private readonly usuarios: UsuariosService) {}

  /** GET /api/v1/usuarios — lista (admin). */
  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar usuarios (admin)' })
  @ApiOkResponse({ type: [UsuarioResponseDto], description: 'Usuarios registrados' })
  async listar(): Promise<ReturnType<UsuariosService['listar']>> {
    return this.usuarios.listar();
  }

  /** GET /api/v1/usuarios/:id — perfil (admin o el propio usuario). */
  @Get(':id')
  @ApiOperation({ summary: 'Perfil de un usuario (admin o el propio)' })
  @ApiParam({ name: 'id', description: 'Id del usuario' })
  @ApiOkResponse({ type: UsuarioResponseDto, description: 'Usuario encontrado' })
  @ApiResponse({ status: 404, description: 'Usuario inexistente o sin permiso' })
  async obtener(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Awaited<ReturnType<UsuariosService['encontrarPorId']>>> {
    const esGlobal = usuario?.rol === ROLES.SUPER_ADMIN || usuario?.rol === ROLES.ADMIN;
    if (!esGlobal && usuario?.user_id !== id) {
      throw new NotFoundError('Usuario', id);
    }
    const encontrado = await this.usuarios.encontrarPorId(id);
    if (!encontrado) throw new NotFoundError('Usuario', id);
    return encontrado;
  }
}
