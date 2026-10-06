import { ApiProperty } from '@nestjs/swagger';

export class PuntoCalorDto {
  @ApiProperty() lat: number;
  @ApiProperty() lng: number;
  @ApiProperty() peso: number;
  @ApiProperty() tipo: string;
}

export class MapaCalorResponseDto {
  @ApiProperty({ type: [PuntoCalorDto] }) puntos: PuntoCalorDto[];
  @ApiProperty() total_puntos: number;
}
