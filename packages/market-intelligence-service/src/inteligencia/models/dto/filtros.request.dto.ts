import { ApiProperty } from '@nestjs/swagger';
import { IsOptional, IsString, IsDateString, IsUUID } from 'class-validator';

export class FiltrosInteligenciaDto {
  @ApiProperty({ description: 'Fecha desde (ISO 8601)', required: false })
  @IsOptional()
  @IsDateString()
  desde?: string;

  @ApiProperty({ description: 'Fecha hasta (ISO 8601)', required: false })
  @IsOptional()
  @IsDateString()
  hasta?: string;

  @ApiProperty({ description: 'SKU', required: false })
  @IsOptional()
  @IsString()
  sku?: string;

  @ApiProperty({ description: 'Id del vendedor (UUID)', required: false })
  @IsOptional()
  @IsUUID('4')
  vendedor_id?: string;
}
