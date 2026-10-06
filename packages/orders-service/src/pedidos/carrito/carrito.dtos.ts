import { ApiProperty } from '@nestjs/swagger';
import { IsBoolean, IsInt, IsNotEmpty, IsOptional, IsString, Max, Min } from 'class-validator';

export class AgregarItemRequestDto {
  @ApiProperty({ description: 'Id de la oferta (vendedor-producto)', example: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11' })
  @IsString()
  @IsNotEmpty()
  oferta_id: string;

  @ApiProperty({ description: 'Cantidad (1-99)', minimum: 1, maximum: 99, example: 2 })
  @IsInt()
  @Min(1)
  @Max(99)
  cantidad: number;
}

export class ActualizarCantidadRequestDto {
  /** 0 elimina el item del carrito. */
  @ApiProperty({ description: 'Cantidad (0 elimina el item; 1-99)', minimum: 0, maximum: 99, example: 3 })
  @IsInt()
  @Min(0)
  @Max(99)
  cantidad: number;
}

/** Opcional: marcar el checkout como "crear orden desde el carrito" (Tabla 21). */
export class UsarCarritoRequestDto {
  @ApiProperty({ description: 'Crear la orden desde el carrito', required: false, example: true })
  @IsOptional()
  @IsBoolean()
  usar_carrito?: boolean;
}
