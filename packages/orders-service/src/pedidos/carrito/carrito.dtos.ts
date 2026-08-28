import { IsBoolean, IsInt, IsNotEmpty, IsOptional, IsString, Max, Min } from 'class-validator';

export class AgregarItemDto {
  @IsString()
  @IsNotEmpty()
  oferta_id: string;

  @IsInt()
  @Min(1)
  @Max(99)
  cantidad: number;
}

export class ActualizarCantidadDto {
  /** 0 elimina el item del carrito. */
  @IsInt()
  @Min(0)
  @Max(99)
  cantidad: number;
}

/** Opcional: marcar el checkout como "crear orden desde el carrito" (Tabla 21). */
export class UsarCarritoDto {
  @IsOptional()
  @IsBoolean()
  usar_carrito?: boolean;
}