# api-gateway · borde único de Core Engine

El **API Gateway** es el único punto de entrada HTTP de la plataforma. Publica
el puerto `8080` al host, valida el JWT de acceso, aplica la tabla de políticas
por ruta y rol, inyecta el contexto de identidad y enruta la petición al
microservicio dueño del recurso dentro de la red Docker `core-engine`.

Es un crate Rust autónomo (`axum` + `tokio` + `reqwest` + `jsonwebtoken`), sin
dependencia del antiguo `packages/shared` de TypeScript. La decisión de
reescritura y sus invariantes están en
[ADR-0015](../../docs/adr/0015-api-gateway-en-rust.md); las reglas de borde en
[`docs/rules/gateway/`](../../docs/rules/gateway/).

> **Regla mental:** el gateway **autentica, autoriza por rol y enruta**. No
> conoce reglas de negocio, no consulta la base de datos y no aplica gating de
> capacidades: eso vive en cada microservicio (`CapabilityGuard`) y en
> `identity-service`.

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `api-gateway` (`packages/api-gateway`) |
| Runtime | Rust 1.89+ (imagen base `rust:1.94.1-bookworm`) |
| Framework HTTP | `axum` 0.8 |
| Cliente upstream | `reqwest` 0.12 (rustls, streaming) |
| JWT | `jsonwebtoken` 9, HS256 |
| Puerto por defecto | `8080` (`PORT`) |
| Único puerto publicado al host | Sí (`8080:8080`) |
| Esquema de BD | Ninguno: el gateway es *stateless* |
| Outbox / RabbitMQ | No participa |
| Readiness | Agrega `/health` de 11 servicios en producción y 12 en stage |
| Documentación | `/docs`, `/docs-json`, `/docs-yaml` (privadas fuera de dev) |
| Métricas | `/metrics` (formato Prometheus) |
| Apagado | Ordenado ante `SIGINT` / `SIGTERM` |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `PORT` | No | `8080` | Puerto de escucha |
| `SERVICIO` | No | `api-gateway` | Etiqueta `servicio` en métricas y logs |
| `JWT_SECRET` | **Sí en producción** (`NODE_ENV=production`) | `dev_secret` en dev | Firma/validación HS256 |
| `NODE_ENV` | No | — | `production` activa el *fail-fast* de `JWT_SECRET` |
| `ENTORNO` | No | vacío | `stage`/`staging` selecciona la lista de readiness de 12 servicios; cualquier otro valor usa la de producción (11) |
| `INTERNAL_API_KEY` | Recomendada | — | Habilita `/ready`, `/metrics`, `/docs*` cuando el diagnóstico no es público |
| `CORS_ORIGINS` | No | vacío | Lista separada por comas; sin coincidencia no se emite CORS |
| `DIAGNOSTICOS_PUBLICOS` | No | `false` | `true` abre `/docs`, `/docs-json`, `/docs-yaml`, `/metrics` sin clave (solo dev) |
| `READY_PUBLIC` | No | `false` | `true` abre `/ready` sin clave (solo dev/CI) |
| `PROXY_UPSTREAM_TIMEOUT_MS` | No | `30000` | Timeout de la llamada al servicio destino |
| `READY_CACHE_MS` | No | `5000` | TTL de la caché del resultado de `/ready` |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Destino |
| `ITEM_SERVICE_URL` | No | `http://item-service:3015` | Destino |
| `INVENTORY_SERVICE_URL` | No | `http://inventory-service:3013` | Destino |
| `PRICING_SERVICE_URL` | No | `http://pricing-service:3014` | Destino |
| `STORES_SERVICE_URL` | No | `http://stores-service:3003` | Destino |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Destino |
| `LOGISTICS_SERVICE_URL` | No | `http://logistics-service:3005` | Destino |
| `COMMISSIONS_SERVICE_URL` | No | `http://commissions-service:3006` | Destino (stage) |
| `FINANCE_SERVICE_URL` | No | `http://finance-service:3007` | Destino |
| `FIELD_SERVICE_URL` | No | `http://field-service:3008` | Destino |
| `INTELLIGENCE_SERVICE_URL` | No | `http://market-intelligence-service:3009` | Destino (dev) |
| `SCHEDULING_SERVICE_URL` | No | `http://scheduling-service:3011` | Destino |
| `STORAGE_SERVICE_URL` | No | `http://storage-service:3010` | Destino |

El gateway **no** habla con `notifications-service` ni `analytics-service` por
HTTP: el primero es consumidor de eventos y el segundo expone rutas que hoy se
enrutan por prefijo de analytics (ver §5). Las URLs sin esquema o con `/` final
se normalizan (`trim_end_matches('/')`).

---

## 2. Ciclo de vida de una petición

```
Cliente
  │
  ▼
axum (fallback → pasarela)
  │
  ├─ 1. OPTIONS → 204 + CORS + cabeceras de seguridad (sin tocar auth)
  ├─ 2. diagnostics::autorizado → /health siempre; /ready, /metrics y /docs*
  │      según flags/clave interna; el resto pasa
  ├─ 3. Rate limit por IP (excepto media pública)
  │      global 300/min · login/token 10/min · /ready 10/min
  ├─ 4. x-request-id (respeta el entrante o genera UUID v4)
  ├─ 5. proxy::reenviar
  │      a. encontrar_politica(ruta, método)   ← deny-by-default
  │      b. aplicar_auth(JWT, roles)           → 401 / 403
  │      c. encontrar_destino(ruta, método)    → 404 si no hay patrón
  │      d. bufferiza el cuerpo (excepto GET/HEAD/OPTIONS)
  │      e. construye cabeceras limpias (no reenvía las del cliente)
  │      f. reqwest con timeout, redirects manuales, streaming de respuesta
  ├─ 6. métricas + x-request-id + CORS + hardening en la respuesta
  ▼
Microservicio destino
```

### Orden exacto del middleware de borde (`http::borde`)

1. `OPTIONS` responde `204` inmediatamente con CORS/hardening.
2. `diagnostics::autorizado` puede ocultar rutas de diagnóstico con `404`
   (no `403`) para no revelar su existencia.
3. Rate limit en memoria; añade `x-ratelimit-limit` y
   `x-ratelimit-remaining`; al exceder devuelve `429` +
   `Retry-After` + `x-ratelimit-remaining: 0`.
4. Correlación: `x-request-id` se propaga o se crea.
5. Proxy y autenticación.
6. Métricas, correlación, CORS y cabeceras de seguridad en la respuesta.

---

## 3. Autenticación y contexto inyectado

### Token esperado

- Firma **HS256** con `JWT_SECRET` compartido con `identity-service`.
- Debe tener `tipo = "access"`; un refresh token se rechaza con `401`
  (`NO_AUTORIZADO`).
- `exp` se valida siempre (`Validation::validate_exp = true`).
- No se valida `aud` ni `iss`; el contrato es el payload de `identity`.

### Claims leídos (`TokenPayload`)

| Claim | Tipo | Se propaga como |
|---|---|---|
| `sub` | UUID | `x-user-id` |
| `email` | string | `x-user-email` |
| `rol` | string | `x-user-rol` |
| `nombre` | string? | `x-user-nombre` |
| `tenant_id` | UUID? | `x-tenant` |
| `personal_id` | UUID? | `x-user-personal` |
| `participacion_porcentaje` | entero? | `x-user-participacion` |
| `rol_empresa_id` | UUID? | `x-user-rol-empresa` |
| `negocio_id` | UUID? | `x-negocio` |
| `permisos` | string[] | `x-user-permisos` (separados por coma) |
| `tipo` | `access` | No se propaga |

### Anti-spoofing

El gateway **no copia** las cabeceras entrantes: construye un `HeaderMap` nuevo
con `x-request-id`, `Content-Type`, `Accept`, `Range` (solo media) y las
cabeceras derivadas del JWT. Cualquier `x-user-*`, `x-tenant`, `x-negocio`,
`x-internal-key` enviada por el cliente se descarta. Esto implementa R-GW-3.

### Rutas públicas y deny-by-default

- Una política con `roles: None` es pública (p. ej. login, registro,
  catálogo público de tienda, media pública).
- Si la ruta no coincide con ninguna política, se aplica la última regla
  `^/api/v2/` con lista de roles vacía: exige token válido pero **ningún rol
  alcanza**, por lo que responde `403`.
- Si el método no coincide, tampoco se relaja la política: cae en
  deny-by-default. Ejemplo: `GET /api/v2/auth/login` no es pública.

---

## 4. Roles reconocidos

`super_admin`, `admin`, `empresario`, `vendedor`, `comprador`, `logistica`,
`coordinador`, `supervisor`, `operativo`.

La autorización fina por negocio, permisos JSON, puesto o asignación **no** se
resuelve aquí: el gateway solo comprueba el rol global del JWT. Los servicios
(`identity`, `orders`, etc.) validan `negocio_id`, permisos y capacidades.

---

## 5. Tabla de enrutamiento (prefijo → servicio)

Las reglas se evalúan **en orden**; gana la primera coincidencia de ruta y
método.

| Servicio | Patrones | Métodos |
|---|---|---|
| `identity` | `/api/v2/auth/*` | `*` |
| | `/api/v2/usuarios/*` | `*` |
| | `/api/v2/empresas/*` | `*` |
| | `/api/v2/incorporaciones/*` | `*` |
| | `/api/v2/packs/*` | `GET`, `POST` |
| | `/api/v2/configuracion/*` | `*` |
| `storage` | `/api/v2/storage/*`, `/api/v2/admin/storage/*` | `*` |
| `inventory` | `/api/v2/admin/inventario/*`, `/api/v2/admin/ubicaciones/*`, `/api/v2/catalog/inventario/*`, `/api/v2/catalog/configuracion-inventario` | `*` |
| `pricing` | `/api/v2/catalog/listas-precio/*`, `/api/v2/catalog/items/:id/{precio,escalas,tarifas,cotizacion}/*`, `/api/v2/catalog/promociones/*` | `*` |
| `item` | `/api/v2/catalog/*` (resto) | `*` |
| `stores` | `/api/v2/vendedores/{productos,tienda,me/ofertas,me/tienda}`, `/api/v2/empresarios/me/tiendas`, `/api/v2/tiendas/*` | `*` / `GET` |
| `scheduling` | `/api/v2/scheduling/*` | `*` |
| `logistics` | `PATCH|POST /api/v2/orders/:id/estado`, `/api/v2/admin/envios*`, `/api/v2/empresarios/me/envios*`, `PATCH /api/v2/envios/:id/estado` | según patrón |
| `orders` | `/api/v2/orders/*`, `/api/v2/carrito/*` | `*` (salvo `orders/:id/estado`, que ya capturó logistics) |
| `commissions` | `/api/v2/vendedores/me/{ventas,liquidaciones}`, `/api/v2/empresarios/me/{liquidaciones,ventas}`, `/api/v2/admin/reportes*`, `/api/v2/admin/liquidaciones*` | `*` |
| `finance` | `/api/v2/finanzas/*`, `/api/v2/comercio/*`, `/api/v2/vendedores/me/fiscal` | `*` |
| `field` | `/api/v2/field/*` | `*` |
| `intelligence` | `/api/v2/inteligencia/*` | `*` |

> `analytics-service` todavía no tiene destino propio en la tabla; sus rutas
> `/api/v2/analytics/**` no son alcanzables por el gateway actual. Si se
> habilita, debe añadirse un `Destino` y su `*_SERVICE_URL`. Es una deuda
> documentada, no un comportamiento accidental.

### Rutas propias del gateway (no proxificadas)

| Ruta | Método | Acceso |
|---|---|---|
| `/health` | GET | Siempre público |
| `/ready` | GET | `READY_PUBLIC=true` o `x-internal-key` |
| `/metrics` | GET | `DIAGNOSTICOS_PUBLICOS=true` o clave |
| `/docs` | GET | idem |
| `/docs-json` | GET | idem (sirve `docs/api/core-engine.openapi.json`) |
| `/docs-yaml` | GET | idem (mismo JSON con `application/yaml`) |

Cualquier otra ruta fuera de `/api/v2/` responde `404 NO_ENCONTRADO`.

---

## 6. Políticas destacadas (resumen funcional)

La tabla completa vive en `src/policy.rs` y se evalúa en orden. Las familias
más relevantes:

| Recurso | Roles |
|---|---|
| `POST /auth/{registro,login,refresh,token,restablecer-contrasena*}` | Público |
| `GET /configuracion/sitio`, `GET /tiendas/:id`, `GET /tiendas/:id/tema.css` | Público |
| `GET /incorporaciones/:token`, `POST /incorporaciones/:token/aceptar` | Público |
| `GET /storage/public/objetos/:id` | Público |
| `POST /empresas` | `empresario` |
| `GET/POST /packs*` | `super_admin`, `admin` (GET también `empresario`) |
| `PATCH /empresas/:id/{perfil,tipo,slug,vendedores}` | `empresario` (+admin donde aplica) |
| `GET/PATCH /empresas/:id/negocios/:n/capacidades` | Todos los roles globales (autorización fina en `identity`) |
| `* /catalog/items*`, precios, inventario | `super_admin`, `admin`, `empresario` |
| `POST /orders`, `GET /orders`, carrito, cotización aceptar/rechazar | `comprador` |
| `GET /orders/{admin/todas,empresa,…}` | `admin`/`empresario` según ruta |
| `PATCH /orders/:id/estado` | `super_admin`, `empresario`, `admin`, `logistica` |
| `* /finanzas/*` | `super_admin`, `admin` |
| `* /comercio/*` | `super_admin`, `admin`, `empresario`, `vendedor` |
| `* /field/*` | Roles operativos y de administración |
| `* /scheduling/*` | Administración + operación |
| `GET /inteligencia/*` | `super_admin`, `admin` (variantes `/me` para `vendedor`) |
| `* /api-keys` | `super_admin`, `admin`, `empresario` |
| `* /api/v2/*` no declarada | Deny-by-default |

---

## 7. Rate limiting

Implementación **en memoria por proceso**, con ventanas fijas de 60 s y clave
`<cubo>:<ip>`:

| Cubo | Límite | Rutas |
|---|---|---|
| `global` | 300 req/min | Todo el tráfico restante |
| `login` | 10 req/min | `POST /api/v2/auth/login`, `POST /api/v2/auth/token` |
| `ready` | 10 req/min | `GET /ready` |

- La IP se toma de `X-Forwarded-For` (primer valor), luego `X-Real-IP`, luego
  `"desconocida"`. **No** hay validación de proxies de confianza en este
  componente; el despliegue debe garantizar que el borde de infraestructura
  reescribe esas cabeceras.
- La media pública (`GET/HEAD /api/v2/storage/public/objetos/:id`) no consume
  cuota.
- Al reiniciar el gateway se reinician los contadores; con varias réplicas cada
  una limita por separado. Para límites distribuidos haría falta un backend
  compartido (Redis), hoy fuera de alcance.
- Respuesta al exceder: `429 DEMASIADAS_PETICIONES`,
  `detalles: {limite, ventana_seg, reintente_en_seg}`, `Retry-After`.

---

## 8. CORS, cabeceras y compresión

- CORS solo se emite si el `Origin` coincide **exactamente** con un elemento de
  `CORS_ORIGINS`. No hay comodines.
- Métodos permitidos: `GET, POST, PATCH, PUT, DELETE, OPTIONS`.
- Cabeceras permitidas: `Content-Type, Authorization, x-request-id`.
- Expuestas: `x-request-id`.
- `Vary: Origin`.
- Cabeceras de hardening aplicadas a toda respuesta (incluidas las proxificadas):
  `x-content-type-options`, `x-frame-options: DENY`,
  `referrer-policy: no-referrer`, CSP `default-src 'none'`,
  HSTS (`max-age=31536000; includeSubDomains`),
  `permissions-policy`, COOP, COEP `require-corp`, CORP `same-site`,
  `x-permitted-cross-domain-policies: none`.

---

## 9. Proxy: detalles de transporte

- Solo se bufferiza el cuerpo en métodos con cuerpo; `GET/HEAD/OPTIONS` viajan
  sin cuerpo.
- Los redirects **no** se siguen (`Policy::none`): el `3xx` del servicio llega
  al cliente (necesario para los `301` de slugs históricos).
- La respuesta se devuelve en **streaming** (`Body::from_stream`), sin cargarla
  en memoria.
- Se eliminan cabeceras hop-by-hop antes de reenviar la respuesta:
  `connection`, `keep-alive`, `transfer-encoding`, `upgrade`,
  `proxy-authenticate`, `proxy-authorization`, `te`, `trailer`.
- `Content-Type`/`Accept` se fijan a JSON salvo media pública, donde se usa
  `application/octet-stream` / `image/*,video/*`.
- Solo se reenvía `Range` en media pública (soporte de vídeo/imagen parcial).
- Timeout de upstream: `PROXY_UPSTREAM_TIMEOUT_MS` (30 s por defecto). Al
  expirar: `504 DESTINO_TIMEOUT` con `{servicio, timeout_ms}`. Cualquier otro
  error de red: `502 DESTINO_NO_DISPONIBLE` con `{servicio}`.
- El gateway no reintenta peticiones no idempotentes; un fallo de red se
  propaga como `502`.

---

## 10. Errores

Formato único (R-GW-6/R-C-8):

```json
{ "codigo": "NO_AUTORIZADO", "mensaje": "Se requiere un token JWT valido." }
```

Con detalles opcionales:

```json
{
  "codigo": "DEMASIADAS_PETICIONES",
  "mensaje": "Se excedio el limite de peticiones; intente de nuevo en unos segundos.",
  "detalles": { "limite": 300, "ventana_seg": 60, "reintente_en_seg": 42 }
}
```

| Código | HTTP | Cuándo |
|---|---|---|
| `NO_AUTORIZADO` | 401 | Sin token, token inválido/expirado o refresh usado como acceso |
| `ACCESO_DENEGADO` | 403 | Rol sin permiso o ruta deny-by-default |
| `NO_ENCONTRADO` | 404 | Ruta fuera de `/api/v2`, sin destino o diagnóstico oculto |
| `DEMASIADAS_PETICIONES` | 429 | Rate limit |
| `HTTP_413` | 413 | Cuerpo demasiado grande al bufferizar |
| `DESTINO_TIMEOUT` | 504 | Upstream sin respuesta en el timeout |
| `DESTINO_NO_DISPONIBLE` | 502 | Error de conexión con upstream |
| `ERROR_INTERNO` | 500 | Fallo no clasificado |

Los errores de los microservicios **se reenvían sin transformar** (código,
cuerpo y status), de modo que el contrato de dominio se mantiene.

---

## 11. Readiness agregada

`GET /ready` consulta en paralelo secuencial `GET http://<servicio>-service:<puerto>/health`
con timeout de **1500 ms** por servicio y cachea el resultado `READY_CACHE_MS`.

- Producción (`ENTORNO` distinto de `stage`/`staging`): 11 servicios
  (`identity`, `item`, `inventory`, `pricing`, `stores`, `orders`, `logistics`,
  `scheduling`, `finance`, `field`, `storage`).
- Stage: 12 servicios (suma `commissions`).
- `commissions` **no** se consulta en producción porque no se despliega allí.
- `notifications`, `analytics` e `intelligence` no forman parte del readiness.
- `200` si todos responden `2xx`; `503` + `status: "degraded"` si alguno falla.
- El cuerpo es `{status, servicios: {nombre: "ok"|"error:500"|"caido"}}`.

> La evaluación es secuencial; con un servicio caído puede tardar hasta
> `n × 1500 ms` la primera vez. La caché evita repetirlo en cada sonda.

---

## 12. Observabilidad

### Métricas Prometheus

| Métrica | Tipo | Etiquetas |
|---|---|---|
| `core_engine_http_requests_total` | counter | `estado`, `metodo`, `ruta`, `servicio` |
| `core_engine_http_server_errors_total` | counter | idem (5xx) |
| `core_engine_http_relevant_client_errors_total` | counter | idem (401, 403, 409) |
| `core_engine_http_request_duration_seconds` | histogram | idem + `le` |

Buckets: `0.05, 0.1, 0.2, 0.4, 0.8, 1.6, 3.2` segundos.

La ruta se **parametriza** (UUID/números → `:id`) para evitar cardinalidad
infinita. Los contadores/gauges de negocio deben llamarse
`core_engine_<subsistema>_<nombre>_total` o
`core_engine_<subsistema>_<nombre>_<unidad>`; el registro lo valida con
`assert!`.

### Logs

JSON por línea con `nivel`, `ts`, `servicio`, `request_id` y el contexto de
correlación por tarea (`tokio::task_local`, equivalente a `AsyncLocalStorage`).
El `x-request-id` entra y sale de la petición.

### Salud

`GET /health` es siempre público y devuelve
`{"api_gateway":"ok","servicio":"api-gateway","uptime_seg":N}`.

---

## 13. Pruebas

```bash
cd packages/api-gateway
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --summary-only
```

Cobertura de los casos de borde:

- `config`: defaults de desarrollo, `JWT_SECRET` obligatorio en producción,
  normalización de URLs/CORS.
- `policy`: rutas públicas, deny-by-default, independencia por método, roles
  de negocios/capacidades/media.
- `auth`: aceptación y propagación de claims, rechazo de refresh y de rol
  insuficiente.
- `rate_limit`: ventana, agotamiento, aislamiento por IP y clasificación de
  login/media.
- `readiness`: selección de lista por entorno y mapeo a HTTP.
- `diagnostics`: cierre de `/ready`, `/metrics`, `/docs`; `health` siempre
  abierto.
- `proxy`: enrutado por servicio y método, prioridad de `logistics` sobre
  `orders` en `PATCH /orders/:id/estado`, 404 sin destino.
- `http`: `/health`, `/metrics` protegido, `/docs` con clave, `/ready`.

Los tests no requieren PostgreSQL ni RabbitMQ.

---

## 14. Operación y despliegue

```bash
# Build local de la imagen (contexto = raíz del repo)
docker build -f packages/api-gateway/Dockerfile -t core-engine-api-gateway .

# Desarrollo con el stack completo
docker compose -f docker-compose.dev.yml up -d --build gateway

# Verificación rápida
curl -s http://localhost:8080/health
curl -s -H "x-internal-key: $INTERNAL_API_KEY" http://localhost:8080/ready
curl -s -H "x-internal-key: $INTERNAL_API_KEY" http://localhost:8080/docs-json | head
```

La imagen es multi-stage, corre como `10001:10001`, no incluye toolchain ni
código fuente y hornea el contrato OpenAPI agregado en el binario
(`include_str!("docs/api/core-engine.openapi.json")`). **Si cambia el contrato,
hay que reconstruir la imagen**; en CI lo verifica el gate G-8.

### Checklist de producción

- [ ] `JWT_SECRET` idéntico al de `identity-service`.
- [ ] `INTERNAL_API_KEY` definida y `DIAGNOSTICOS_PUBLICOS`/`READY_PUBLIC`
      apagados.
- [ ] `CORS_ORIGINS` con los dominios reales (nunca `*`; el código no soporta
      comodines).
- [ ] `ENTORNO=stage` solo en staging (cambia el readiness a 12 servicios).
- [ ] Timeout de upstream alineado con el SLA del servicio más lento.
- [ ] TLS terminado antes del gateway (Traefik/Dockploy); el gateway no hace
      TLS.
- [ ] Una sola réplica o límites aceptados como “por réplica” (rate limit en
      memoria).

---

## 15. Límites conocidos y deudas

- **Rate limit no distribuido** y en memoria: se reinicia con el proceso.
- **Sin circuit breaker real**: el “circuit breaker” del mensaje 502 es un
  error de transporte, no un estado abierto con recuperación.
- **Sin reintentos** ni presupuesto de timeout por ruta.
- **Sin destino para `analytics-service`** en `proxy::destinos`.
- **Readiness secuencial** y con lista hardcodeada; un servicio nuevo exige
  tocar `readiness.rs`.
- La autenticación solo entiende HS256 y el payload de `identity`; no hay
  rotación de claves ni `kid`.

---

## 16. Referencias

- [ADR-0015 · API Gateway en Rust](../../docs/adr/0015-api-gateway-en-rust.md)
- [Reglas de gateway](../../docs/rules/gateway/README.md)
- [Políticas de autenticación de borde](../../docs/rules/gateway/01-auth.md)
- [Contrato público agregado](../../docs/api/core-engine.openapi.json)
- [Contrato interno agregado](../../docs/api/core-engine.internal.openapi.json)
- [Runbook de autenticación del gateway](../../docs/runbook-gateway-auth.md)
- [README general de paquetes](../README.md)
