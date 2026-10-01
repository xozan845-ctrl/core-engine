import 'reflect-metadata';
import { IsString, IsInt, Min } from 'class-validator';
import { of } from 'rxjs';
import type { ExecutionContext, CallHandler } from '@nestjs/common';
import { DtoValidationPipe, cuerpoDeError, TrazabilidadInterceptor } from './validation.pipe';
import { ValidationError } from '../errors';

class CrearOfertaDto {
  @IsString()
  nombre!: string;

  @IsInt()
  @Min(1)
  stock!: number;
}

describe('DtoValidationPipe (validacion de DTOs con class-validator)', () => {
  const pipe = new DtoValidationPipe();

  it('debe transformar y devolver una instancia del DTO cuando el payload es valido', async () => {
    const resultado = await pipe.transform(
      { nombre: 'teclado', stock: 5 },
      { metatype: CrearOfertaDto, type: 'body' },
    );
    expect(resultado).toBeInstanceOf(CrearOfertaDto);
    expect(resultado as CrearOfertaDto).toMatchObject({ nombre: 'teclado', stock: 5 });
  });

  it('debe lanzar ValidationError con detalles por campo cuando el DTO no cumple', async () => {
    let error: ValidationError | undefined;
    try {
      await pipe.transform(
        { nombre: '', stock: 0 },
        { metatype: CrearOfertaDto, type: 'body' },
      );
    } catch (e) {
      error = e as ValidationError;
    }
    expect(error?.codigo).toBe('VALIDACION');
    expect(error?.toResponse().detalles).toBeDefined();
  });

  it('debe devolver el valor tal cual cuando no hay metatype o es primitivo', async () => {
    expect(await pipe.transform('texto', { metatype: String, type: 'param' })).toBe('texto');
    expect(await pipe.transform({ a: 1 }, { metatype: undefined, type: 'body' })).toEqual({ a: 1 });
  });
});

describe('cuerpoDeError (shape de errores, doc 5.7)', () => {
  it('debe omitir detalles cuando no existen', () => {
    expect(cuerpoDeError('X', 'mensaje')).toEqual({ codigo: 'X', mensaje: 'mensaje' });
  });

  it('debe incluir detalles cuando existen', () => {
    expect(cuerpoDeError('X', 'mensaje', { campo: 'a' })).toEqual({
      codigo: 'X',
      mensaje: 'mensaje',
      detalles: { campo: 'a' },
    });
  });
});

describe('TrazabilidadInterceptor (x-request-id y metricas, doc 5.3)', () => {
  it('debe fijar x-request-id y registrar la peticion al finalizar', () => {
    const headers: Record<string, string> = {};
    const res = {
      headers,
      setHeader: (k: string, v: string) => {
        headers[k] = v;
      },
      on: (_evento: string, cb: () => void) => {
        res.finish = cb;
      },
      finish: undefined as (() => void) | undefined,
      statusCode: 200,
    };
    const req = { method: 'GET', originalUrl: '/health', headers: {} };
    const metrics = { registrarPeticion: jest.fn() };
    const interceptor = new TrazabilidadInterceptor(
      metrics as never,
      'test',
    );
    const contexto = {
      switchToHttp: () => ({ getRequest: () => req, getResponse: () => res }),
    } as unknown as ExecutionContext;
    const next = { handle: () => of(null) } as unknown as CallHandler;

    const observable = interceptor.intercept(contexto, next);
    observable.subscribe(() => {
      expect(res.headers['x-request-id']).toBeDefined();
    });
    // la fuente (of) completa de forma sincrona: el finalize ya registro on('finish')
    expect(res.finish).toBeDefined();
    res.finish?.();
    expect(metrics.registrarPeticion).toHaveBeenCalledWith('GET', '/health', 200, expect.any(Number));
  });
});