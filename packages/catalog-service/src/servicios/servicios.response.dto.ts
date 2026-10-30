import { ApiProperty } from '@nestjs/swagger';

export class ServicioResponseDto {
  @ApiProperty({ format: 'uuid' })
  id: string;

  @ApiProperty({ format: 'uuid' })
  empresa_id: string;

  @ApiProperty({ example: 'SERV-001' })
  codigo: string;

  @ApiProperty()
  nombre: string;

  @ApiProperty()
  descripcion: string;

  @ApiProperty()
  categoria: string;

  @ApiProperty()
  marca: string;

  @ApiProperty()
  codigo_barras: string;

  @ApiProperty({ example: '1500.00' })
  precio: string;

  @ApiProperty({ example: '500.00' })
  costo: string;

  @ApiProperty({ example: 'Hora (h)' })
  unidad: string;

  @ApiProperty({ example: 60 })
  duracion_minutos: number;

  @ApiProperty({ enum: ['activo', 'inactivo'] })
  estado: 'activo' | 'inactivo';

  @ApiProperty({ format: 'date-time' })
  creado_en: string;

  @ApiProperty({ format: 'date-time' })
  actualizado_en: string;
}

export class PaginaServiciosResponseDto {
  @ApiProperty({ type: () => [ServicioResponseDto] })
  items: ServicioResponseDto[];

  @ApiProperty()
  total: number;

  @ApiProperty()
  pagina: number;

  @ApiProperty()
  limite: number;

  @ApiProperty()
  paginas: number;
}
