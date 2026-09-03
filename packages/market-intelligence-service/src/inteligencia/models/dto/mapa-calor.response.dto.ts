export class PuntoCalorDto {
  lat: number;
  lng: number;
  peso: number;
  tipo: string;
}

export class MapaCalorResponseDto {
  puntos: PuntoCalorDto[];
  total_puntos: number;
}
