import { ApiProperty } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  ArrayMinSize,
  IsArray,
  IsInt,
  IsOptional,
  IsString,
  IsUUID,
  Matches,
  Min,
  ValidateNested,
} from 'class-validator';

export class LineaPrecioInternaRequestDto {
  @ApiProperty({ description: 'Clave opaca para correlacionar la respuesta', example: 'oferta-1' })
  @IsString()
  clave: string;

  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  item_id: string;

  @ApiProperty({ minimum: 1 })
  @IsInt()
  @Min(1)
  cantidad: number;
}

export class ResolverPreciosInternoRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  empresa_id: string;

  @ApiProperty({ required: false, example: 'mayorista' })
  @IsOptional()
  @Matches(/^[a-z][a-z0-9_-]{1,39}$/)
  lista_codigo?: string;

  @ApiProperty({ required: false, example: 'mayorista' })
  @IsOptional()
  @Matches(/^[a-z][a-z0-9_-]{0,39}$/)
  segmento_codigo?: string;

  @ApiProperty({ type: () => [LineaPrecioInternaRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => LineaPrecioInternaRequestDto)
  lineas: LineaPrecioInternaRequestDto[];
}

export class PrecioLineaInternaResponseDto {
  @ApiProperty() clave: string;
  @ApiProperty({ enum: ['lista', 'escala', 'base'] }) origen: 'lista' | 'escala' | 'base';
  @ApiProperty() precio_unitario_cents: number;
  @ApiProperty() precio_total_cents: number;
}

export class PreciosInternosResponseDto {
  @ApiProperty({ type: () => [PrecioLineaInternaResponseDto] })
  precios: PrecioLineaInternaResponseDto[];
}

export class PrecioItemInternoResponseDto {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() cantidad: number;
  @ApiProperty({ enum: ['lista', 'escala', 'base'] }) origen: 'lista' | 'escala' | 'base';
  @ApiProperty() precio_unitario_cents: number;
  @ApiProperty() precio_total_cents: number;
}
