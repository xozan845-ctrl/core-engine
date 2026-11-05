# Auditoría de seguridad · stack Docker local (`core-engine`)

**Fecha:** 2026-11-05
**Modo:** caja gris/blanca (código del repo + stack Docker en ejecución) + explotación controlada no destructiva
**Alcance:** `127.0.0.1:8080` (API Gateway), red Docker `core-engine`, servicios internos y dependencias publicadas por `docker-compose.dev.yml`
**Autorización:** el propietario del repositorio y del entorno solicitó la revisión.
**Estado del entorno al terminar:** Postgres, RabbitMQ y MinIO se levantaron para poder probar la lógica de negocio; los 14 contenedores quedaron `running` y `/ready` en verde. Se aplicaron las correcciones de la sección *Correcciones aplicadas*: secretos rotados (JWT, clave interna y contraseña de admin), puertos en loopback y dos migraciones que faltaban. No se borraron datos de negocio.

> Nota: no existe una “skill de red team” en el repo; lo que está configurado es el servidor MCP `pentest` en `opencode.jsonc` (nmap, nuclei, ffuf, nikto, etc.). Esta auditoría combinó ese tooling con revisión de código y pruebas HTTP manuales.

---

## Resumen ejecutivo

El aislamiento multi-tenant **a nivel de aplicación** está bien implementado: no conseguí IDOR en órdenes, ni lectura cruzada de empresas/usuarios, ni inyección SQL, ni bypass del gateway por *path traversal*, ni `alg=none`, ni spoofing de cabeceras `X-User-*`.

El problema es **la capa de configuración y secretos**. El entorno de desarrollo usa exactamente las credenciales versionadas en `.env.example`:

- `JWT_SECRET` público y predecible → **cualquiera con acceso al repo puede firmar un token `super_admin`** y leer usuarios, finanzas, reportes y pedidos.
- `ADMIN_PASSWORD` público → login directo como administrador.
- `INTERNAL_API_KEY` público → acceso a endpoints internos desde cualquier contenedor de la red.
- Postgres, RabbitMQ (consola de administración) y MinIO (consola) publicados en `0.0.0.0` con esas mismas credenciales.
- El rate-limit anti fuerza bruta es **evadible** con `X-Forwarded-For`.
- El login permite **enumerar correos** por diferencia de tiempo (~73 ms vs ~5 ms).
- El catálogo y la tienda públicos **filtran costo, margen, precio base, stock y IDs internos**.
- La base del stack Docker corre como **superusuario con `BYPASSRLS`** y RLS está desactivado: el aislamiento depende solo de los filtros de la aplicación.

**Riesgo global:** Crítico en desarrollo (y potencialmente en cualquier entorno que reutilice `.env.example`). En producción el riesgo baja a Alto/Medio **solo si** los placeholders `<CAMBIAR …>` se reemplazaron y el borde (Traefik) no reenvía `X-Forwarded-For` del cliente.

---

## Correcciones aplicadas en esta sesión

| Hallazgo | Estado | Cambio |
| --- | --- | --- |
| C-01 · Secretos versionados | **Corregido en el entorno** | `JWT_SECRET`, `INTERNAL_API_KEY` y `ADMIN_PASSWORD` rotados; `.env` modo `600`; `.env.example` con placeholders; nuevo `npm run env:dev` genera secretos aleatorios por máquina |
| C-01 · *Fallback* `dev_secret` | **Corregido en código** | `leerSecretoJwt()`/`leerClaveInterna()` en `@core/shared`; sin valor por defecto y *fail-fast* en producción |
| C-01 · *Seed* con contraseña por defecto | **Corregido en código** | `SeedService` exige `ADMIN_EMAIL` y `ADMIN_PASSWORD` (>= 12); no siembra nada si faltan |
| C-02 · Infra expuesta en `0.0.0.0` | **Corregido** | Postgres, RabbitMQ, MinIO y gateway publicados solo en `127.0.0.1` |
| C-01 · Credenciales de datos (Postgres/RabbitMQ/MinIO) | **Pendiente (ventana)** | Las del volumen actual siguen vigentes; `.env.example` ya no las expone y los puertos están en loopback. Rotarlas exige actualizar el volumen y reiniciar los servicios |
| H-01 · Bypass de rate-limit por XFF | **Corregido** | `TRUST_PROXY` por defecto `loopback`; cubo adicional por cuenta (`login-cuenta:<correo>`) que no depende de IP |
| H-02 · Enumeración por timing | **Corregido** | El login compara siempre contra un hash *dummy* cuando el correo no existe |
| M-01 · Sin validación de placeholders | **Corregido** | Validación de longitud/placeholder para JWT y clave interna en producción |
| M-02 · Migraciones no aplicadas | **Parcial** | Aplicadas `09a_storage_publico.sql` y la tabla de `01g`; el índice único de tienda por vendedor se omitió porque el seed tiene `vendedor_id` duplicados |
| M-03 · `/auth/me` pública en el gateway | **Corregido** | Ahora exige token (`roles: Object.values(ROLES)`) |
| M-04 · Refresh sin rotación | **Corregido** | Cada refresh revoca el `jti` usado y emite uno nuevo; el reuso falla |
| M-05 · `.env` legible por terceros | **Corregido** | `chmod 600` y creación con modo `600` en el generador |
| Nuevo · Endpoints de cuenta bloqueados | **Corregido** | `/auth/cambiar-contrasena` y `/auth/vincular-personal` caían en el *deny-by-default* (403); ahora exigen sesión válida |
| H-03 · Fuga de costo/margen/stock | **Pendiente** | Requiere decisión de contrato: DTO público específico (no se tocó para no romper el frontend) |
| H-04 · RLS con superusuario | **Pendiente** | Requiere rol `NOSUPERUSER NOBYPASSRLS`, funciones `auth.*` y aplicar `99_rls.sql` |
| M-06 · Diagnóstico público en dev | **Pendiente/DX** | Sigue abierto en dev (`DIAGNOSTICOS_PUBLICOS/READY_PUBLIC=true`) pero ya solo escucha en loopback |

Verificación posterior al cambio:

```text
token firmado con el secreto viejo de .env.example -> 401
login con la contraseña antigua de admin            -> 401
login con la contraseña nueva                       -> 200
clave interna vieja                                 -> 403
clave interna nueva                                 -> 200
12 intentos de login rotando IP, mismo correo       -> 10 x 401 + 429
GET invitación pública con token inválido           -> 410 (antes 500)
POST /storage/subidas                               -> 201 (antes 500)
empresa A leyendo objeto de empresa B               -> 404
```

> Nota: al rotar `JWT_SECRET` se invalidaron las sesiones existentes (esperado). El stack se recreó con los nuevos valores.

---

## Cadena de ataque principal (demostrada antes de la remediación)

> Tras rotar los secretos, el token firmado con el valor de la plantilla devuelve `401`. Los pasos se conservan como evidencia de la explotación original.

1. Leer `.env.example` (versionado) → obtener `JWT_SECRET` y `ADMIN_PASSWORD`.
2. Firmar un JWT HS256 con `rol: "super_admin"`.
3. El gateway lo acepta y lo propaga como `X-User-Rol: super_admin`.
4. Acceso total de lectura (y escritura) sin pasar por login:

```bash
# El secreto sale de la plantilla versionada
SECRET=$(grep '^JWT_SECRET=' .env.example | cut -d= -f2-)
TOKEN=$(node -e "console.log(require('jsonwebtoken').sign(
  {sub:'00000000-0000-0000-0000-000000000001',email:'attacker@evil.test',
   rol:'super_admin',tipo:'access'}, process.argv[1], {algorithm:'HS256',expiresIn:'1h'}))" "$SECRET")

curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8080/api/v1/usuarios
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8080/api/v1/admin/reportes
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8080/api/v1/finanzas/kpis
```

Evidencia (respuestas reales del stack):

```text
TOKEN FORJADO -> /api/v1/auth/me            : 200 {"rol":"super_admin",...}
TOKEN FORJADO -> /api/v1/usuarios           : 200 [144 usuarios con correo y rol]
TOKEN FORJADO -> /api/v1/admin/reportes     : 200 {"gmv_mes_cents":2690000,...}
TOKEN FORJADO -> /api/v1/finanzas/kpis      : 200 {"gmv_cents":7751000,...}
TOKEN FORJADO -> /api/v1/orders/admin/todas : 200 [200 pedidos con comprador/vendedor]
```

El mismo `ADMIN_PASSWORD` de la plantilla permite además un login legítimo como `super_admin`.

---

## Hallazgos

### C-01 · Secretos de desarrollo versionados y usados por el stack (Crítico)

`.env.example` contiene credenciales reales, no placeholders, y `.env` es idéntico para las claves críticas:

> Estado **original**, antes de la remediación. Tras rotar los secretos, `.env` ya no coincide con la plantilla.

| Variable | Ubicación del valor literal | Prefijo (parcial) | ¿Igual en `.env`? (antes) |
| --- | --- | --- | --- |
| `JWT_SECRET` | `.env.example:56` | `dev_secret…` (32 chars) | Sí |
| `INTERNAL_API_KEY` | `.env.example:61` | `dev_internal_key…` | Sí |
| `ADMIN_PASSWORD` | `.env.example:65` | `AdminCore …` | Sí |
| `RABBITMQ_PASSWORD` | `.env.example:24` | `0b66c112…` | Sí |
| `POSTGRES_PASSWORD` | `.env.example:15` | `core_engine…` | Sí |
| `MINIO_ROOT_PASSWORD` | `.env.example:30` | `core_engine…` | Sí |

> Los valores completos están en `.env.example` (versionado); este informe no los reproduce para no ampliar su exposición.

Impacto: forja de JWT, takeover de administrador, acceso a datos internos y a la infraestructura. Cualquier persona con acceso al repositorio (contratista, fork, backup, CI) tiene las llaves.

Además, ambos servicios tienen *fallback* silencioso:

- `packages/api-gateway/src/proxy/gateway-auth.middleware.ts:268` → `process.env.JWT_SECRET ?? 'dev_secret'`
- `packages/identity-service/src/auth/auth.service.ts:56` → `process.env.JWT_SECRET ?? 'dev_secret'`

Si `JWT_SECRET` falta, el sistema arranca igual y acepta tokens firmados con `dev_secret`.

**Remediación**

1. Rotar **todas** las credenciales de la tabla anterior (dev, stage y prod).
2. Sustituir los valores de `.env.example` por placeholders inequívocos (`<GENERAR_32_BYTES_ALEATORIOS>`).
3. Validar al arranque: si `NODE_ENV=production` y `JWT_SECRET`/`INTERNAL_API_KEY`/`ADMIN_PASSWORD` faltan, miden menos de 32 caracteres o coinciden con valores conocidos de plantilla → `process.exit(1)`.
4. Eliminar el fallback `?? 'dev_secret'` (o permitirlo solo con `NODE_ENV=development` explícito).

---

### C-02 · Infraestructura de desarrollo expuesta en `0.0.0.0` con credenciales de plantilla (Alto/Crítico según red)

`docker-compose.dev.yml` publica en todas las interfaces:

```text
5432/tcp  -> 0.0.0.0:5432   postgres
5672/tcp  -> 0.0.0.0:5672   amqp
15672/tcp -> 0.0.0.0:15672  RabbitMQ management
9000/tcp  -> 0.0.0.0:9000   MinIO S3
9001/tcp  -> 0.0.0.0:9001   MinIO console
8080/tcp  -> 0.0.0.0:8080   gateway
```

Verificado con `nmap -Pn -sV` y con las credenciales de `.env.example`:

```text
Postgres remoto: 200 OK, current_user=core_engine, identity.usuarios = 144 filas
RabbitMQ mgmt:   200 OK, versión 3.13.7, 20 colas, 9 conexiones
MinIO console:   POST /api/v1/login -> 204 (login root correcto)
```

En una red compartida (oficina, coworking, Wi-Fi pública, VPS mal firewallado) esto es compromiso directo de datos y del broker.

**Remediación**

- En `docker-compose.dev.yml`, publicar solo en loopback: `127.0.0.1:5432:5432`, `127.0.0.1:15672:15672`, etc., o no publicar (usar `docker exec`/`psql` desde el host).
- Credenciales de desarrollo aleatorias por máquina (no compartidas en el repo).
- Firewall del host: denegar 5432/5672/9000/9001/15672 desde fuera de loopback.
- Añadir `cap_drop: [ALL]`, `security_opt: [no-new-privileges:true]` y redes separadas (edge / servicios / datos).

---

### H-01 · Bypass del rate-limit de login con `X-Forwarded-For` (Alto)

El gateway usa `TRUST_PROXY=1` por defecto cuando la variable no está definida (`packages/api-gateway/src/main.ts:53`), y el rate-limit se calcula con `req.ip` (`packages/api-gateway/src/proxy/rate-limit.middleware.ts:50`). Si el gateway es alcanzable directamente (dev/stage) o el borde no sanea `X-Forwarded-For`, el atacante rota la cabecera y obtiene un cubo nuevo por petición.

Evidencia:

```text
11 intentos con X-Forwarded-For fija:       502 502 502 502 502 502 502 502 502 502 429
15 intentos rotando X-Forwarded-For:        502 × 15  (nunca 429)
```

Esto anula el límite de 10/min de `/auth/login` y permite fuerza bruta de contraseñas (política mínima de 8 caracteres, sin bloqueo por cuenta).

**Remediación**

- No confiar en `X-Forwarded-For` del cliente. Opciones:
  - `TRUST_PROXY=false` cuando se accede directo; o `app.set('trust proxy', '<IP/CIDR del borde>')`.
  - Rate-limit por `(IP real del socket, correo)` y bloqueo progresivo por cuenta.
- En el borde (Traefik), limpiar `X-Forwarded-For` entrante y fijarlo al cliente real.
- Añadir bloqueo temporal de cuenta tras N fallos y CAPTCHA/proof-of-work en login.

---

### H-02 · Enumeración de usuarios por timing en login (Medio)

`verificarCredenciales` solo ejecuta `bcrypt.compare` si el correo existe (`packages/identity-service/src/usuarios/usuarios.service.ts:112-119`). La diferencia es medible:

```text
login con correo existente:   70,71,72,73,74,126 ms  (mediana ~73 ms)
login con correo inexistente: 1,2,5,5,5,6 ms          (mediana ~5 ms)
```

Permite confirmar qué correos tienen cuenta (incluido `admin@core-engine.test`) y alimentar la fuerza bruta.

**Remediación:** ejecutar siempre un `bcrypt.compare` contra un hash *dummy* de coste equivalente cuando el usuario no existe, y unificar mensaje y tiempo de respuesta.

---

### H-03 · Fuga de datos comerciales en catálogo y tienda públicos (Medio)

`GET /api/v1/catalog/productos?empresa_id=<uuid>` es público y devuelve campos internos:

```text
['costo', 'precio_base', 'stock', 'stock_minimo', 'stock_maximo', 'codigo_barras',
 'tipo_costo', 'estado_operativo', 'es_materia_prima', 'ubicacion', ...]
```

`GET /api/v1/tiendas/<slug>` es público y expone, por oferta:

```text
precio_base=50.00  precio_venta=55.00  margen=10
participacion_porcentaje=100
vendedor_id / empresa_id / propietario_usuario_id
```

Cadena: la tienda pública entrega `empresa_id` → con ese UUID se descarga el catálogo completo de la empresa (incluido costo/stock). En los datos sembrados `costo` es 0, pero el contrato de la API lo expone y en producción llevará el costo real.

**Remediación:** DTOs públicos explícitos (nombre, precio de venta, imagen, disponibilidad) y eliminar `costo`, `precio_base` si es costo, `margen`, `stock_min/max`, `codigo_barras`, `ubicacion` e IDs internos de las respuestas no autenticadas.

---

### H-04 · RLS desactivado y rol de aplicación `SUPERUSER`/`BYPASSRLS` (Medio)

En el Postgres del stack Docker:

```text
current_user = core_engine, rolsuper = t, rolbypassrls = t
pg_policies  = 0
pg_tables    → rowsecurity = false en todas las tablas inspeccionadas
auth.*       → no existen auth.uid()/auth.rol()/auth.jwt()
```

`packages/shared/src/db/pg.service.ts:70` sí inyecta `request.jwt.claims` en transacción, pero un rol superusuario **ignora RLS**. Además, `infra/db/supabase/99_rls.sql` depende de funciones `auth.*` que este Postgres no tiene, por lo que no puede aplicarse tal cual.

Impacto: el aislamiento multi-tenant queda al 100 % en los `WHERE tenant_id = $1` de la aplicación. Un fallo futuro en un endpoint (o una consulta nueva sin filtro) se convierte en fuga entre tenants. Hoy no encontré ese fallo, pero no hay red de seguridad.

**Remediación**

- Crear un rol `core_engine_app` `NOSUPERUSER NOBYPASSRLS` con permisos mínimos y usarlo en `DATABASE_URL`.
- Definir `auth.uid()`, `auth.rol()` y `auth.jwt()` sobre `current_setting('request.jwt.claims', true)` en el Postgres Docker.
- Aplicar `99_rls.sql`, activar `FORCE ROW LEVEL SECURITY` en tablas con datos de tenant y añadir pruebas negativas cross-tenant al CI.

---

### M-01 · Sin validación de placeholders ni *fail-fast* de secretos (Medio)

`.env.production.example` y `.env.stage.example` usan `<CAMBIAR …>`, pero nada impide arrancar con esos valores literales. `REQUIRE_EDGE_AUTH` solo valida `EDGE_BASIC_AUTH` (gateway `main.ts:34`). No hay comprobación de longitud/entropía ni de “valor de plantilla” para JWT, clave interna, admin, DB, Rabbit o MinIO.

**Remediación:** validación centralizada al arranque + test de CI que falle si un secreto es placeholder, repetido entre entornos o de longitud insuficiente.

---

### M-02 · Migraciones no aplicadas a volúmenes existentes → 500 en rutas públicas (Medio)

Los scripts de `infra/db/init` solo se ejecutan al crear el volumen. En el volumen actual faltan migraciones y hay *schema drift*:

```text
relation "identity.invitaciones_vendedor" does not exist
column "publico" of relation "objetos" does not exist
```

Consecuencias observadas:

- `GET /api/v1/invitaciones/vendedores/<token>` → **500** (`ERROR_INTERNO`) en una ruta pública.
- `POST /api/v1/storage/subidas` → **500** en todos los casos (subida de archivos rota).

**Remediación:** runner de migraciones idempotente en el arranque/despliegue (no solo `docker-entrypoint-initdb.d`), con tabla de versiones y verificación de columnas/tablas al iniciar.

---

### M-03 · `/api/v1/auth/me` es “pública” para el gateway (Bajo/Medio, defensa en profundidad)

`packages/api-gateway/src/proxy/gateway-auth.middleware.ts:118` declara `roles: null`, y `aplicar()` interpreta `roles === null` como “no requiere token”. Hoy el servicio responde 401 porque exige `X-User-Id`, pero la autenticación de esa ruta depende solo del downstream.

**Remediación:** `roles: Object.values(ROLES)` (sigue exigiendo token) o un flag explícito `requiereToken: true` separado de `roles`.

---

### M-04 · Refresh token sin rotación ni detección de reuso (Bajo/Medio)

`AuthService.refrescar()` reemite access+refresh con **el mismo `jti`** (`packages/identity-service/src/auth/auth.service.ts:104`). Un refresh robado es reutilizable durante 7 días, incluso después de usarlo; no hay detección de replay ni revocación del anterior.

**Remediación:** rotar el `jti` en cada refresh, revocar el anterior y, si se detecta reuso, revocar toda la familia de sesiones.

---

### M-05 · `.env` legible por grupo y otros usuarios (Bajo)

```text
664 xozan01:xozan01 .env
```

En un host multiusuario, cualquier cuenta local puede leer JWT, contraseña de admin y credenciales de DB/broker/MinIO. **Remediación:** `chmod 600 .env` y Docker/Secret Manager en lugar de archivos planos.

---

### M-06 · Diagnóstico abierto por configuración de desarrollo (Bajo)

Con `DIAGNOSTICOS_PUBLICOS=true` y `READY_PUBLIC=true` (valores de `.env.example`) quedan públicos:

- `/docs`, `/docs-json` (contrato completo de 186 endpoints, incluido el esquema `internalKey`)
- `/metrics` (métricas de proceso)
- `/ready` (topología y estado de los 8 servicios)

No es explotable por sí solo, pero facilita el reconocimiento y confirma la topología. En producción las plantillas ya traen `false`; el riesgo es copiar el `.env` de desarrollo.

**Remediación:** mantener `false` en stage/prod, proteger con `x-internal-key` o Basic, y añadir un test que falle si se activan en un entorno desplegado.

---

## Verificaciones negativas (lo que sí aguantó)

| Prueba | Resultado |
| --- | --- |
| Forja `alg=none` | 401 |
| Refresh token usado como access | 401 |
| Access token usado en `/refresh` | 401 |
| Spoofing `X-User-Id` / `X-User-Rol` / `X-Tenant` sin token | 401 (el gateway limpia y reinyecta) |
| IDOR en `GET /orders/:id`, `/timeline`, `PATCH /estado` | 403 por servicio |
| Lectura cruzada de empresas y usuarios entre tenants | 404/403 |
| `PATCH` de empresa/tienda de otro tenant | 403 |
| Path traversal codificado (`%2e%2e%2f`, doble encoding, `%5c`) | 404/400, sin bypass |
| Inyección SQL (revisión + pruebas en filtros) | Parameterizado; únicas interpolaciones son nombres fijos de tabla/columna |
| SSRF en *fetch* internos | URLs fijas por configuración; IDs con `encodeURIComponent` |
| SVG público / XSS almacenado en storage | Bloqueado (`image/svg+xml` excluido) |
| Content-type falsificado en subida | Revalidado con `statObject` al confirmar |
| CORS con `Origin` no permitido | Sin `Access-Control-Allow-Origin` |
| Contenedores | Usuario `node`, sin privilegios, sin `docker.sock` |
| Dependencias de producción | `npm audit --omit=dev` → 0 vulnerabilidades |

---

## Prioridad de remediación

1. **Hoy:** rotar `JWT_SECRET`, `INTERNAL_API_KEY`, `ADMIN_PASSWORD`, `POSTGRES_PASSWORD`, `RABBITMQ_PASSWORD`, `MINIO_ROOT_PASSWORD`; cerrar 5432/5672/9000/9001/15672 a loopback.
2. **Esta semana:** validación *fail-fast* de secretos + quitar `dev_secret`; corregir el rate-limit (no confiar en XFF) y el timing del login.
3. **Este sprint:** DTOs públicos sin costo/margen/stock; rol no-superusuario + RLS real o eliminar la falsa expectativa; runner de migraciones.
4. **Siguiente:** rotación de refresh tokens, política `/auth/me`, `.env` 600, pruebas negativas cross-tenant en CI.

---

## Anexo · Evidencia de infraestructura

```text
$ nmap -Pn -p 5432,5672,9000,9001,15672,8080 -sV 127.0.0.1
5432/tcp  open  postgresql
5672/tcp  open  amqp
8080/tcp  open  http-proxy
9000/tcp  open  http
9001/tcp  open  http
15672/tcp open  http

$ curl -u "$RABBITMQ_USER:$RABBITMQ_PASSWORD" http://127.0.0.1:15672/api/overview
200 OK · rabbitmq_version=3.13.7 · queues=20 · connections=9

$ psql -h 127.0.0.1 -U core_engine
current_user=core_engine · identity.usuarios = 144

$ curl -X POST http://127.0.0.1:9001/api/v1/login -d '{"accessKey":…,"secretKey":…}'
204 No Content
```

Acceso interno con la clave de plantilla (desde el propio contenedor del gateway):

```text
GET http://identity-service:3001/internal/usuarios/<id>   -> 200 (correo, rol, nombre)
GET http://identity-service:3001/internal/contar-vendedores -> 200 {"vendedores":27}
Sin cabecera / clave incorrecta -> 403
```

Esto demuestra que cualquier contenedor comprometido en la red `core-engine` puede suplantar servicios internos con una clave estática y versionada.
