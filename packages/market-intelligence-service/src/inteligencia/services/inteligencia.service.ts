import { Injectable } from '@nestjs/common';
import { InteligenciaRepository } from '../repositories/inteligencia.repository';
import { AgregadorService } from './agregador.service';
import { FiltrosInteligenciaDto } from '../models/dto/filtros.request.dto';
import { MapaCalorResponseDto } from '../models/dto/mapa-calor.response.dto';
import { MetricsService, VentaGeolocalizadaData } from '@core/shared';

@Injectable()
export class InteligenciaService {
  constructor(
    private readonly repo: InteligenciaRepository,
    private readonly agregador: AgregadorService,
    private readonly metrics: MetricsService,
  ) {}

  // ── Mapa de calor ─────────────────────────────────────────────────────────

  async obtenerMapaCalor(filtros: FiltrosInteligenciaDto): Promise<MapaCalorResponseDto> {
    const puntos = await this.repo.obtenerMapaCalor(filtros);
    return { puntos, total_puntos: puntos.length };
  }

  // ── Rendimiento de vendedores ─────────────────────────────────────────────

  async obtenerRendimientoVendedores(
    filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number },
  ) {
    return this.repo.obtenerRendimientoVendedores(filtros);
  }

  // ── Cobertura geografica ──────────────────────────────────────────────────

  async obtenerCoberturaZona(filtros: any) {
    return this.repo.obtenerCoberturaZona(filtros);
  }

  // ── Demanda de productos ──────────────────────────────────────────────────

  async obtenerDemandaProductos(
    filtros: FiltrosInteligenciaDto & { cursor?: string; limite?: number },
  ) {
    return this.repo.obtenerDemandaProductos(filtros);
  }

  // ── KPIs globales (resumen ejecutivo) ─────────────────────────────────────

  async obtenerResumen(filtros: FiltrosInteligenciaDto) {
    return this.repo.obtenerResumen(filtros);
  }

  // ── Tendencias temporales ─────────────────────────────────────────────────

  async obtenerTendencias(filtros: FiltrosInteligenciaDto) {
    return this.repo.obtenerTendencias(filtros);
  }

  // ── Ingesta: registrar venta geolocalizada ────────────────────────────────

  async registrarVenta(data: VentaGeolocalizadaData): Promise<void> {
    await this.agregador.procesarVentaGeolocalizada(data);
  }
}
