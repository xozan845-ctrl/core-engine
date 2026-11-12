import { Body, Controller, Get, Param, Patch } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsOptional, IsUUID } from 'class-validator';
import { NotFoundError, ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { UsuariosService } from './usuarios.service';
import { UsuarioResponseDto } from './usuarios.response.dto';

class AvatarUsuarioRequestDto {
  @ApiProperty({
    description: 'Objeto de storage público confirmado como imagen; null para quitar el avatar',
    required: false,
    nullable: true,
    format: 'uuid',
  })
  @IsOptional()
  @IsUUID()
  avatar_objeto_id?: string | null;
}

/**
 * Gestion de usuarios (sustituye `gestion-usuarios.functions.adapter` de Firebase).
 * Las rutas estan protegidas por rol en el API Gateway (solo admin).
 */
@ApiTags('Usuarios')
@ApiBearerAuth()
@Controller('api/v2/usuarios')
export class UsuariosController {
  constructor(private readonly usuarios: UsuariosService) {}

  /** GET /api/v2/usuarios — lista (admin). */
  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar usuarios (admin)' })
  @ApiOkResponse({ type: [UsuarioResponseDto], description: 'Usuarios registrados' })
  async listar(): Promise<ReturnType<UsuariosService['listar']>> {
    return this.usuarios.listar();
  }

  @Patch(':id/avatar')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Asociar o quitar el avatar de un usuario/vendedor' })
  @ApiParam({ name: 'id', description: 'Id del usuario' })
  @ApiOkResponse({ type: UsuarioResponseDto, description: 'Avatar actualizado' })
  actualizarAvatar(
    @Param('id') id: string,
    @Body() dto: AvatarUsuarioRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.usuarios.actualizarAvatar(id, dto.avatar_objeto_id ?? null, usuario);
  }

  /** GET /api/v2/usuarios/:id — perfil (admin o el propio usuario). */
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
