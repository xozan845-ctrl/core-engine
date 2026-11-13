import { Controller, Get, Param } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiTags,
} from '@nestjs/swagger';
import {
  CAPACIDADES,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import {
  DescargaDigital,
  EntregaDigital,
  EntregasDigitalesService,
} from './entregas-digitales.service';

export class EntregaDigitalResponseDto implements EntregaDigital {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) order_id: string;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() sku: string;
  @ApiProperty({ format: 'uuid' }) token: string;
  @ApiProperty({ enum: ['activo', 'revocado'] }) estado: 'activo' | 'revocado';
  @ApiProperty() descargas: number;
  @ApiProperty({ nullable: true }) max_descargas: number | null;
  @ApiProperty({ format: 'date-time', nullable: true }) expira_en: string | null;
  @ApiProperty({ format: 'date-time', nullable: true }) primera_descarga_en: string | null;
  @ApiProperty({ format: 'date-time', nullable: true }) ultima_descarga_en: string | null;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
}

export class DescargaDigitalResponseDto extends EntregaDigitalResponseDto implements DescargaDigital {
  @ApiProperty({ format: 'uuid' }) comprador_id: string;
  @ApiProperty() recurso_url: string;
}

@ApiTags('Entregas digitales')
@ApiBearerAuth()
@Controller('api/v2')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.COMPRADOR)
@RequiereCapacidades(CAPACIDADES.FULFILLMENT_DIGITAL)
export class EntregasDigitalesController {
  constructor(private readonly entregas: EntregasDigitalesService) {}

  @Get('pedidos/:id/entregas-digitales')
  @ApiOperation({ summary: 'Listar los accesos digitales de una orden' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: [EntregaDigitalResponseDto], description: 'Accesos digitales' })
  listar(@Param('id') orderId: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.entregas.listarPorOrden(orderId, usuario);
  }

  @Get('entregas-digitales/:token')
  @ApiOperation({ summary: 'Consumir un acceso digital y obtener su recurso' })
  @ApiParam({ name: 'token', format: 'uuid' })
  @ApiOkResponse({ type: DescargaDigitalResponseDto, description: 'Acceso digital' })
  descargar(@Param('token') token: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.entregas.descargar(token, usuario);
  }
}
