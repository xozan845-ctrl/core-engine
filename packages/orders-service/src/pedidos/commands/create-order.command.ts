import { Type } from 'class-transformer';
import {
  IsArray,
  IsBoolean,
  IsInt,
  IsNotEmpty,
  IsOptional,
  IsString,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';

export class ItemOrdenDto {
  @IsString()
  oferta_id: string;

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
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ItemOrdenDto)
  items?: ItemOrdenDto[];

  @IsOptional()
  @IsBoolean()
  usar_carrito?: boolean;
}