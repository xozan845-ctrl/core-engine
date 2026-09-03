import { IsOptional, IsString, IsDateString, IsUUID } from 'class-validator';

export class FiltrosInteligenciaDto {
  @IsOptional()
  @IsDateString()
  desde?: string;

  @IsOptional()
  @IsDateString()
  hasta?: string;

  @IsOptional()
  @IsString()
  sku?: string;

  @IsOptional()
  @IsUUID('4')
  vendedor_id?: string;
}
