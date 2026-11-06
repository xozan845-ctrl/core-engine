# k6 · capacidad y no-regresión

Pruebas de carga con **k6** para medir la capacidad real del stack y evitar que
un cambio baje el soporte de concurrencia. No sustituyen al smoke funcional ni
a los tests de integración: miden el perímetro (gateway + servicios + BD) con
un mix de lecturas reales.

## Escenarios

| Archivo | Uso | Ejecución |
| --- | --- | --- |
| `smoke.js` | 2 VUs / 20 s; valida rutas críticas | Manual o CI |
| `load.js` | Ritmo sostenido (`K6_LOAD_RATE`) | **Gate de deploy** |
| `breakpoint.js` | Rampa 10→800 req/s; aborta a p95>2 s o error>5 % | Manual, para calibrar |
| `write.js` | Carrito + creación de orden con pool de compradores | Manual; no va en el deploy |

Mix de `load`/`breakpoint`: catálogo público (45 %), tienda pública (30 %),
`/health` (10 %), pedidos autenticados (10 %) y `/ready` (5 %, opcional).
El login se hace **una vez en `setup()`** para no chocar con el rate-limit por
cuenta; los escenarios usan un `X-Forwarded-For` aleatorio para simular muchos
clientes y evitar el cubo de 300/min por IP.

## Resultado local (dev, 2026-11-06)

Stack local de un host, 101 ofertas, sin `/ready`, mix de lecturas:

| Ritmo | p95 | p99 | Errores |
| --- | --- | --- | --- |
| 300 req/s (baseline) | 19 ms | 62 ms | 0 % |
| 400 req/s | 31 ms | 49 ms | 0 % |
| 450 req/s | 92 ms | 189 ms | 0 % |
| **500 req/s** | **274 ms** | 350 ms | 0 % |
| 600 req/s | 4.0 s | 15.2 s | 0 % |
| 700 req/s | 13.6 s | 18.0 s | 0 % |
| 800 req/s | 15.6 s | 17.5 s | 0 % |

Conclusión local: el codo está en **~500 req/s**; el margen de seguridad usado
es **300 req/s (60 %)**, con `p95 < 800 ms`, `p99 < 1500 ms` y error < 1 %.
Stage y producción deben calibrarse en su propio hardware/VPS; no copies estos
números sin medir.

## Variables

| Variable | Uso |
| --- | --- |
| `BASE_URL` | Gateway (default `http://127.0.0.1:8080`) |
| `K6_EMPRESA_ID` | Empresa del catálogo público (sin ella se omite) |
| `K6_STORE_SLUG` | Slug de la tienda pública (sin él se omite) |
| `K6_BUYER_EMAIL` / `K6_BUYER_PASSWORD` | Comprador para `/orders` (opcional) |
| `K6_READY=true` + `K6_INTERNAL_KEY` | Incluye `/ready` (stage/prod) |
| `K6_EDGE_BASIC_AUTH` | `usuario:clave` del gate de borde (stage/prod) |
| `K6_LOAD_RATE` / `K6_LOAD_DURATION` | Ritmo/duración de `load.js` |
| `K6_LOAD_PRE_VUS` / `K6_LOAD_MAX_VUS` | VUs reservadas/máximas de `load.js` |
| `K6_BREAK_*` | Rampa de `breakpoint.js` |
| `K6_WRITE_RATE` / `K6_WRITE_DURATION` | Ritmo/duración de `write.js` |
| `K6_WRITE_BUYERS` / `K6_WRITE_PASSWORD` | Pool de compradores y su clave |
| `K6_SUMMARY` | Ruta del resumen JSON |

> No uses `K6_DURATION`, `K6_VUS`, `K6_ITERATIONS` ni `K6_STAGES`: son
> variables reservadas de k6 y anulan el escenario del script.

## Local

```bash
# Fixture: 100 productos + ofertas sobre el tenant Flow Co (dev)
npm run k6:seed

export BASE_URL=http://127.0.0.1:8080
export K6_EMPRESA_ID=<uuid-empresa>
export K6_STORE_SLUG=<slug-tienda>
export K6_BUYER_EMAIL=<comprador> K6_BUYER_PASSWORD=<clave>

npm run k6:smoke
K6_LOAD_RATE=300 K6_LOAD_DURATION=2m npm run k6:load
node scripts/tools/k6-compare.mjs --summary tests/k6/k6-summary.json --env local --scenario load

# Calibrar la línea base (una vez, con el entorno estable)
node scripts/tools/k6-compare.mjs --update --summary tests/k6/k6-summary.json \
  --env local --scenario load --rate 300 --duration 2m

# Breakpoint manual (no en prod sin aprobación)
npm run k6:breakpoint

# Escritura: 20 compradores, 5 órdenes/s durante 30 s (crea datos reales)
K6_WRITE_PASSWORD='<clave-fixture>' K6_WRITE_RATE=5 K6_WRITE_DURATION=30s npm run k6:write
node scripts/tools/k6-compare.mjs --update --summary tests/k6/k6-summary.json \
  --env local --scenario write --rate 5 --duration 30s
```

> `write.js` crea usuarios `k6.write.*`, carritos y órdenes, y consume stock.
> Úsalo solo contra el tenant de prueba. No está en el gate de deploy.
> Mantén `K6_WRITE_BUYERS >= K6_WRITE_MAX_VUS` para que cada VU tenga su propio
> carrito y no haya carreras.

Los scripts de npm usan `grafana/k6` con `--network host` para alcanzar el
gateway en loopback. En CI se usa la misma imagen.

## CI

- **`.github/workflows/k6.yml`** (manual): elige entorno (`stage`/`prod`),
  escenario (`smoke`/`load`/`breakpoint`), URL, ritmo/duración y si actualiza
  la línea base. El entorno de GitHub aplica aprobaciones para `prod`.
- **`.github/workflows/deploy.yml`**: tras el smoke de stage/prod ejecuta
  `load.js` y `k6-compare.mjs --allow-missing`. Si no hay baseline, avisa; en
  cuanto se calibra, el gate falla si p95/p99/error/RPS empeoran.
- **Secrets/vars**: `K6_EMPRESA_ID`, `K6_STORE_SLUG`, `K6_BUYER_EMAIL`,
  `K6_BUYER_PASSWORD`, `K6_INTERNAL_KEY`; vars `K6_RATE_STAGE`,
  `K6_RATE_PROD`, `K6_DURATION`, `K6_READY`.

La línea base vive en `scripts/config/k6-baseline.json`. El comparador tolera
+25 % en p95, +30 % en p99, +1 punto de error y -20 % de RPS; si se supera,
sale con código 1 y bloquea el deploy.

## Seguridad de la prueba

- El breakpoint es **manual**: puede degradar el entorno mientras dura.
- No lo ejecutes contra producción sin aprobación y sin ventana.
- El deploy usa solo el ritmo de seguridad (50-70 % del codo).
- El `X-Forwarded-For` aleatorio evita el rate-limit por IP para poder medir el
  backend; en producción el rate-limit sigue activo para clientes reales.
