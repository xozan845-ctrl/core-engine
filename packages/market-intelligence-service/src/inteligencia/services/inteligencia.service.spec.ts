import { Test, TestingModule } from '@nestjs/testing';
import { InteligenciaService } from './inteligencia.service';
import { InteligenciaRepository } from '../repositories/inteligencia.repository';
import { AgregadorService } from './agregador.service';
import { MetricsService } from '@core/shared';

// ── Mocks ────────────────────────────────────────────────────────────────────

const mockRepo = {
  obtenerMapaCalor: jest.fn(),
};

const mockAgregador = {
  procesarVentaGeolocalizada: jest.fn(),
};

const mockMetrics = {
  incrementarContador: jest.fn(),
};

// ── Suite ────────────────────────────────────────────────────────────────────

describe('InteligenciaService', () => {
  let service: InteligenciaService;

  beforeEach(async () => {
    jest.clearAllMocks();

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        InteligenciaService,
        { provide: InteligenciaRepository, useValue: mockRepo },
        { provide: AgregadorService, useValue: mockAgregador },
        { provide: MetricsService, useValue: mockMetrics },
      ],
    }).compile();

    service = module.get<InteligenciaService>(InteligenciaService);
  });

  describe('obtenerMapaCalor', () => {
    it('debe devolver puntos y el conteo total', async () => {
      const puntosEsperados = [
        { lat: 4.7, lng: -74.0, peso: 1, tipo: 'venta' },
        { lat: 4.8, lng: -74.1, peso: 2, tipo: 'venta' },
      ];
      mockRepo.obtenerMapaCalor.mockResolvedValueOnce(puntosEsperados);

      const resultado = await service.obtenerMapaCalor({});

      expect(resultado.puntos).toEqual(puntosEsperados);
      expect(resultado.total_puntos).toBe(2);
    });

    it('debe pasar los filtros correctamente al repositorio', async () => {
      mockRepo.obtenerMapaCalor.mockResolvedValueOnce([]);

      const filtros = { vendedor_id: 'v-uuid-1', sku: 'SKU-TEST' };
      await service.obtenerMapaCalor(filtros);

      expect(mockRepo.obtenerMapaCalor).toHaveBeenCalledWith(filtros);
    });

    it('debe devolver lista vacia si no hay puntos', async () => {
      mockRepo.obtenerMapaCalor.mockResolvedValueOnce([]);

      const resultado = await service.obtenerMapaCalor({});

      expect(resultado.puntos).toHaveLength(0);
      expect(resultado.total_puntos).toBe(0);
    });
  });
});
