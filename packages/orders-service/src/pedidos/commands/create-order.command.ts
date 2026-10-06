import { ApiProperty } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  IsArray,
  IsBoolean,
  IsInt,
  IsOptional,
  IsString,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';

export class ItemOrdenRequestDto {
  @ApiProperty({ description: 'Id de la oferta (vendedor-producto)' })
  @IsString()
  oferta_id: string;

  @ApiProperty({ description: 'Cantidad (1-99)', minimum: 1, maximum: 99, example: 1 })
  @IsInt()
  @Min(1)
  @Max(99)
  cantidad: number;
}

/**
 * CreateOrderCommand (intencion: crear una orden: usuario, articulos y total a pagar).
 * El comprador envia solo oferta_id + cantidad; los precios los enriquece el
 * stores-service (RN-01) y el total se calcula con Money (nunca flotante).
 * Con usar_carrito=true la orden se crea DESDE el carrito (Tabla 21) y el
 * carrito se vacia en la misma transaccion (RN-05).
 */
export class CreateOrderCommand {
  @ApiProperty({ description: 'Artículos de la orden', required: false, type: [ItemOrdenRequestDto] })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ItemOrdenRequestDto)
  items?: ItemOrdenRequestDto[];

  @ApiProperty({ description: 'Crear la orden desde el carrito (RN-05)', required: false, example: false })
  @IsOptional()
  @IsBoolean()
  usar_carrito?: boolean;
}
