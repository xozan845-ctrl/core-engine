import { ApiProperty } from '@nestjs/swagger';

export class UsuarioResponseDto {
  @ApiProperty({ description: 'Id del usuario', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Nombre del usuario' })
  nombre: string;

  @ApiProperty({ description: 'Correo del usuario', format: 'email' })
  correo: string;

  @ApiProperty({
    description: 'Rol del usuario',
    enum: ['admin', 'vendedor', 'comprador', 'logistica', 'coordinador', 'supervisor', 'operativo'],
    example: 'vendedor',
  })
  rol: string;

  @ApiProperty({ description: 'Tenant del usuario', required: false, format: 'uuid' })
  tenant_id?: string;

  @ApiProperty({ description: 'Ficha de personal vinculada', required: false, format: 'uuid' })
  personal_id?: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class ContarVendedoresResponseDto {
  @ApiProperty({ description: 'Cantidad de usuarios con rol vendedor', example: 12 })
  vendedores: number;
}
