import { Controller, Get, Query } from '@nestjs/common';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';
import { TributacionService } from '../tributacion/tributacion.service';
import { KpisService } from './kpis.service';

/**
 * GET /api/v1/vendedores/me/fiscal — situacion tributaria del vendedor:
 * su regimen (Cuota Fija o General) y las comisiones documentadas para su
 * declaracion (cap. 4.4: "la plataforma documenta comisiones para su
 * declaracion"). Solo lectura para el vendedor (matriz RLS, tabla 8.15).
 */
@Controller('api/v1/vendedores')
@Roles(ROLES.VENDEDOR)
export class VendedorFiscalController {
  constructor(
    private readonly tributacion: TributacionService,
    private readonly kpis: KpisService,
  ) {}

  @Get('me/fiscal')
  async situacion(
    @UsuarioActual() usuario: UsuarioContexto,
    @Query('mes') mes?: string,
  ) {
    const sujeto = await this.tributacion.sujetoFiscalDe(usuario.user_id);
    const resumen = await this.kpis.resumenDelVendedor(usuario.user_id, mes);
    return {
      sujeto_fiscal: sujeto ?? null,
      resumen_comisiones: resumen,
      nota: 'Comisiones documentadas para la declaracion del vendedor ante la DGI (Ley 822 / regimen de Cuota Fija).',
    };
  }
}