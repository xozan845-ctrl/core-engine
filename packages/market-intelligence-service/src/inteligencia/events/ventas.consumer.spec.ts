import { Test, TestingModule } from '@nestjs/testing';
import { VentasConsumer } from './ventas.consumer';
import { RabbitService, PgService, EVENTOS, Logger } from '@core/shared';
import { AgregadorService } from '../services/agregador.service';
import { DataQualityService } from '../services/data-quality.service';

// ── Mocks ───────────────────────────────────────────────────────────────────

const mockRabbit = {
  declararColas: jest.fn().mockResolvedValue(undefined),
  consumir: jest.fn().mockResolvedValue(undefined),
  activarReintento: jest.fn(),
};

const mockPg = {
  queryOne: jest.fn(),
  query: jest.fn().mockResolvedValue(undefined),
};

const mockAgregador = {
  procesarVentaGeolocalizada: jest.fn().mockResolvedValue(undefined),
  procesarOrdenCompletada: jest.fn().mockResolvedValue(undefined),
  procesarStockActualizado: jest.fn().mockResolvedValue(undefined),
};

const mockDataQuality = {
  validarVentaGeolocalizada: jest.fn().mockReturnValue({ isValid: true }),
};

// Silenciar logs en tests
jest.spyOn(Logger, 'create').mockReturnValue({
  info: jest.fn(),
  warn: jest.fn(),
  error: jest.fn(),
  debug: jest.fn(),
} as any);

// ── Suite ────────────────────────────────────────────────────────────────────

describe('VentasConsumer', () => {
  let consumer: VentasConsumer;

  beforeEach(async () => {
    jest.clearAllMocks();

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        VentasConsumer,
        { provide: RabbitService, useValue: mockRabbit },
        { provide: PgService, useValue: mockPg },
        { provide: AgregadorService, useValue: mockAgregador },
        { provide: DataQualityService, useValue: mockDataQuality },
      ],
    }).compile();

    consumer = module.get<VentasConsumer>(VentasConsumer);
  });

  describe('onModuleInit', () => {
    it('debe declarar cola y activar consumer y reintento', async () => {
      await consumer.onModuleInit();

      expect(mockRabbit.declararColas).toHaveBeenCalledWith([
        expect.objectContaining({ routingKeys: [EVENTOS.VENTA_GEOLOCALIZADA] }),
        expect.objectContaining({ routingKeys: [EVENTOS.COMISION_ACREDITADA] }),
        expect.objectContaining({ routingKeys: [EVENTOS.STOCK_UPDATED] }),
      ]);
      expect(mockRabbit.consumir).toHaveBeenCalledTimes(3);
      expect(mockRabbit.activarReintento).toHaveBeenCalled();
    });
  });

  describe('manejar (via consumir callback)', () => {
    let manejador: (e: any) => Promise<void>;

    beforeEach(async () => {
      await consumer.onModuleInit();
      // Capturar el manejador que se pasa a rabbit.consumir
      manejador = mockRabbit.consumir.mock.calls[0][1];
    });

    it('debe ignorar eventos de otro tipo', async () => {
      await manejador({ tipo: 'order.created', event_id: 'e-1', data: {} });
      expect(mockAgregador.procesarVentaGeolocalizada).not.toHaveBeenCalled();
    });

    it('debe ignorar eventos ya procesados (idempotencia)', async () => {
      mockPg.queryOne.mockResolvedValueOnce({ 1: 1 }); // ya procesado

      await manejador({
        tipo: EVENTOS.VENTA_GEOLOCALIZADA,
        event_id: 'e-dup-1',
        data: { order_id: 'o1', vendedor_id: 'v1', skus: ['SKU1'], monto_cents: 5000, lat: 4.5, lng: -74.1 },
      });

      expect(mockAgregador.procesarVentaGeolocalizada).not.toHaveBeenCalled();
      expect(mockPg.query).not.toHaveBeenCalled();
    });

    it('debe procesar y marcar evento nuevo como procesado', async () => {
      mockPg.queryOne.mockResolvedValueOnce(null); // no procesado aún

      const data = {
        order_id: 'o2',
        vendedor_id: 'v1',
        skus: ['SKU1'],
        monto_cents: 10000,
        lat: 4.7,
        lng: -74.0,
      };

      await manejador({
        tipo: EVENTOS.VENTA_GEOLOCALIZADA,
        event_id: 'e-new-1',
        data,
      });

      expect(mockAgregador.procesarVentaGeolocalizada).toHaveBeenCalledWith(data);
      expect(mockPg.query).toHaveBeenCalledWith(
        expect.stringContaining('INSERT INTO intelligence.eventos_procesados'),
        ['e-new-1', EVENTOS.VENTA_GEOLOCALIZADA],
      );
    });
  });
});
