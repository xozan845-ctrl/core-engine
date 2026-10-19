import { ApiProperty } from '@nestjs/swagger';

export class SesionUsuarioResponseDto {
  @ApiProperty({ description: 'Id del usuario', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Nombre del usuario' })
  nombre: string;

  @ApiProperty({ description: 'Correo del usuario', format: 'email' })
  correo: string;

  @ApiProperty({
    description: 'Rol del usuario',
    enum: ['admin', 'vendedor', 'comprador', 'logistica', 'coordinador', 'supervisor', 'operativo'],
    example: 'comprador',
  })
  rol: string;

  @ApiProperty({ description: 'Tenant del usuario', required: false, format: 'uuid' })
  tenant_id?: string;

  @ApiProperty({ description: 'Ficha de personal vinculada', required: false, format: 'uuid' })
  personal_id?: string;
}

export class SesionResponseDto {
  @ApiProperty({ description: 'Access token JWT (HS256)', example: 'eyJhbGciOiJIUzI1NiIs...' })
  access_token: string;

  @ApiProperty({ description: 'Refresh token JWT', example: 'eyJhbGciOiJIUzI1NiIs...' })
  refresh_token: string;

  @ApiProperty({ description: 'Vigencia del access token en segundos', example: 900 })
  expira_en: number;

  @ApiProperty({ description: 'Datos del usuario autenticado', type: SesionUsuarioResponseDto })
  usuario: SesionUsuarioResponseDto;
}

export class UsuarioContextoResponseDto {
  @ApiProperty({ description: 'Id del usuario autenticado', format: 'uuid' })
  user_id: string;

  @ApiProperty({ description: 'Correo del usuario', format: 'email' })
  email: string;

  @ApiProperty({
    description: 'Rol del usuario',
    enum: ['admin', 'vendedor', 'comprador', 'logistica', 'coordinador', 'supervisor', 'operativo'],
    example: 'admin',
  })
  rol: string;

  @ApiProperty({ description: 'Nombre del usuario', required: false })
  nombre?: string;

  @ApiProperty({ description: 'Tenant del usuario', required: false, format: 'uuid' })
  tenant_id?: string;

  @ApiProperty({ description: 'Ficha de personal vinculada', required: false, format: 'uuid' })
  personal_id?: string;
}

export class OkResponseDto {
  @ApiProperty({ description: 'Operación completada', example: true })
  ok: boolean;
}
