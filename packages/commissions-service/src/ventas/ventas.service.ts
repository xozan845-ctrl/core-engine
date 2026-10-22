import { Injectable } from '@nestjs/common';
import { PgService, Money } from '@core/shared';
import { Comision } from '../comisiones/comisiones.service';
import { Liquidacion, LiquidacionEmpresa } from '../liquidaciones/liquidaciones.service';

export interface VentaVendedor {
  order_id: string;
  venta: string;
  comision: string;
  monto_vendedor: string;
  estado: string;
  fecha: string;
}

export interface EstadoCuenta {
  vendedor_id: string;
  ventas: VentaVendedor[];
  comisiones_pendientes: string;
  liquidaciones: (Liquidacion & { monto: string })[];
}

export interface VentaEmpresa {
  order_id: string;
  venta: string;
  comision: string;
  monto_empresario: string;
  estado: string;
  fecha: string;
}

export interface EstadoCuentaEmpresa {
  empresa_id: string;
  ventas: VentaEmpresa[];
  comisiones_pendientes: string;
  liquidaciones: (LiquidacionEmpresa & { monto: string })[];
}

/**
 * H-04: consultar comisiones y liquidaciones — libro del vendedor.
 * Lecturas contra el esquema propio (CQRS: proyeccion de comisiones).
 */
@Injectable()
export class VentasService {
  constructor(private readonly pg: PgService) {}

  async estadoCuenta(vendedorId: string): Promise<EstadoCuenta> {
    const comisiones = await this.pg.query<Comision>(
      `SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
              comision_cents, monto_vendedor_cents, monto_empresario_cents,
              participacion_porcentaje, estado, creado_en
       FROM commissions.comisiones WHERE vendedor_id = $1 ORDER BY creado_en DESC LIMIT 200`,
      [vendedorId],
    );
    const liquidaciones = await this.pg.query<Liquidacion>(
      `SELECT id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
       FROM commissions.liquidaciones WHERE vendedor_id = $1 ORDER BY periodo_inicio DESC LIMIT 50`,
      [vendedorId],
    );
    const pendientes = comisiones
      .filter((c) => c.estado === 'acreditada')
      .reduce((acc, c) => acc + c.monto_vendedor_cents, 0);

    return {
      vendedor_id: vendedorId,
      ventas: comisiones.map((c) => ({
        order_id: c.order_id,
        venta: Money.desdeCentavos(c.venta_cents).string(),
        comision: Money.desdeCentavos(c.comision_cents).string(),
        monto_vendedor: Money.desdeCentavos(c.monto_vendedor_cents).string(),
        estado: c.estado,
        fecha: c.creado_en,
      })),
      comisiones_pendientes: Money.desdeCentavos(pendientes).string(),
      liquidaciones: liquidaciones.map((l) => ({ ...l, monto: Money.desdeCentavos(l.monto_cents).string() })),
    };
  }

  /** Estado de cuenta de la empresa: comisiones por linea y liquidaciones propias. */
  async estadoCuentaEmpresa(empresaId: string): Promise<EstadoCuentaEmpresa> {
    const comisiones = await this.pg.query<Comision>(
      `SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
              comision_cents, monto_vendedor_cents, monto_empresario_cents,
              participacion_porcentaje, estado, creado_en
       FROM commissions.comisiones WHERE empresa_id = $1 ORDER BY creado_en DESC LIMIT 200`,
      [empresaId],
    );
    const liquidaciones = await this.pg.query<LiquidacionEmpresa>(
      `SELECT id, empresa_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
       FROM commissions.liquidaciones_empresas WHERE empresa_id = $1
       ORDER BY periodo_inicio DESC LIMIT 50`,
      [empresaId],
    );
    const pendientes = comisiones
      .filter((c) => c.estado === 'acreditada')
      .reduce((acc, c) => acc + c.monto_empresario_cents, 0);

    return {
      empresa_id: empresaId,
      ventas: comisiones.map((c) => ({
        order_id: c.order_id,
        venta: Money.desdeCentavos(c.venta_cents).string(),
        comision: Money.desdeCentavos(c.comision_cents).string(),
        monto_empresario: Money.desdeCentavos(c.monto_empresario_cents).string(),
        estado: c.estado,
        fecha: c.creado_en,
      })),
      comisiones_pendientes: Money.desdeCentavos(pendientes).string(),
      liquidaciones: liquidaciones.map((l) => ({
        ...l,
        monto: Money.desdeCentavos(l.monto_cents).string(),
      })),
    };
  }
}
