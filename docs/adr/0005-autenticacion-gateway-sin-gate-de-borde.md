# ADR-0005: Autenticación del gateway sin gate de borde y credenciales de máquina

- **Fecha**: 2026-11-12
- **Estado**: Aceptado — fases 1-3 implementadas; despliegue de `staging` pendiente del acceso externo
- **Ámbito**: `api-gateway` (perímetro), `identity-service` (emisión de credenciales), despliegue de `staging`

## Contexto

El gateway acumuló dos mecanismos para resolver un problema que no es suyo:

1. **Gate de borde con Basic Auth** (`edge-auth.middleware.ts`): obligatorio en
   `staging` (`EDGE_BASIC_AUTH` + `REQUIRE_EDGE_AUTH=true`) para que el entorno
   no sea público. Incluye un bypass para `Bearer` válido, reglas especiales
   para `/ready`, y ocultación de `/docs` y `/metrics`.
2. **Proxy público `/api-proxy`** (ADR-29): el frontend de Firebase no puede
   guardar el Basic en el bundle, así que el gateway expone un prefijo público
   que inyecta el Basic del servidor y reenvía al gateway interno. Es un bypass
   explícito: público por diseño y **no es una frontera de seguridad**.

Además, no existe una credencial para clientes máquina (integraciones,
automatizaciones, apps server-side) distinta del JWT de usuario.

El resultado es ~1.100 líneas de middleware y specs, variables de entorno
cruzadas (Basic, `READY_PUBLIC`, `DIAGNOSTICOS_PUBLICOS`), un salto de red extra
y dos caminos de autenticación que hay que mantener sincronizados.

## Decisión

### 1. La privacidad de `staging` sale del gateway

El gateway deja de implementar Basic Auth y el proxy `/api-proxy`. `staging`
se protege **en el borde externo** (Cloudflare Access, VPN o allowlist de IP en
Traefik/Dockploy), antes de llegar al proceso. Producción no tiene gate de
borde y sigue igual.

El gateway queda con un único camino de autenticación de usuario: **JWT** +
roles (R-GW-3). Se elimina el bypass Bearer del Basic porque ya no hay Basic.

### 2. Diagnóstico mínimo, sin gate de borde

- `/health`: siempre público (liveness de Docker/Dockploy).
- `/ready`: requiere `x-internal-key` salvo `READY_PUBLIC=true` (solo desarrollo).
- `/docs`, `/docs-json`, `/docs-yaml`, `/metrics`: requieren `x-internal-key`
  salvo `DIAGNOSTICOS_PUBLICOS=true`; en caso contrario responden `404`.
- El resto de la superficie exige JWT y política de rol.

Esto conserva el cierre de la superficie de diagnóstico (R-DS-3) sin Basic.

### 3. API keys para clientes máquina (propiedad de `identity`)

`identity` es la autoridad de credenciales de máquina:

- Tabla `identity.api_keys` con `empresa_id`, `nombre`, `prefijo`, `hash`
  (SHA-256), `rol`, `estado`, `expira_en`, `creado_por`, `creado_en` y
  `ultimo_uso_en`.
- La clave se muestra **una sola vez** al crearla (`cek_<prefijo>_<secreto>`);
  en reposo solo queda el hash y el prefijo para identificarla.
- Roles permitidos: roles de tenant (`empresario`, `vendedor`, `logistica`,
  `coordinador`, `supervisor`, `operativo`). **Nunca** `super_admin` ni `admin`
  como key de máquina.
- Creación/listado/revocación bajo `/api/v2/api-keys` (empresario sobre su
  tenant; `admin`/`super_admin` sobre cualquier tenant).
- Rotación = crear una key nueva y revocar la anterior; expiración opcional.

### 4. Las API keys se intercambian por JWT corto

El cliente máquina no envía la key a cada microservicio. La intercambia en
`identity` por un **JWT de acceso corto**:

```http
POST /api/v2/auth/token
x-api-key: cek_ab12cd34_<secreto>
```

Respuesta: `{ access_token, token_type, expires_in }` con `rol`, `tenant_id` y
`sub = api-key:<id>`. TTL = `JWT_ACCESS_TTL` (900 s por defecto), **sin refresh
token**: el cliente vuelve a intercambiar.

Ventajas:

- El gateway **no cambia su camino de autorización**: sigue validando JWT y
  políticas de rol (R-GW-3).
- Los servicios no conocen API keys ni leen su tabla.
- Revocar la key invalida los intercambios futuros; los JWT ya emitidos caducan
  en minutos (ventana acotada y documentada).
- El endpoint se limita por IP (10/min, igual que login) y no registra la key.

### 5. Orden de despliegue (obligatorio)

1. Configurar el acceso externo de `staging` (Cloudflare Access/VPN/allowlist) y
   verificarlo.
2. Desplegar el gateway sin Basic ni `/api-proxy`.
3. Ejecutar `smoke-remote` (la URL de staging ya no pide Basic; si el borde
   externo lo exige, el smoke usa su propio mecanismo).
4. Rotar/eliminar `EDGE_BASIC_AUTH` y `REQUIRE_EDGE_AUTH` de los entornos.

Sin el paso 1, `staging` quedaría público. La decisión de infraestructura es
requisito de la promoción, no del PR de código.

## Consecuencias

### Positivas

- Se eliminan `edge-auth`, `public-proxy` y sus specs (~1.100 líneas), el bypass
  y las variables `EDGE_BASIC_AUTH`/`REQUIRE_EDGE_AUTH`.
- Un solo mecanismo de autenticación (JWT) y un solo lugar que emite
  credenciales (`identity`).
- Credenciales de máquina revocables, con expiración y último uso, sin secretos
  estáticos compartidos en varios servicios.

### Costes y riesgos

- `staging` depende de una configuración externa; un error la deja pública. Se
  mitiga con el orden de despliegue y el checklist de Dockploy.
- Un API key es un bearer: solo por TLS, nunca en un navegador. Para el SPA se
  sigue usando el JWT del usuario.
- Los JWT emitidos por key no se revocan individualmente; caducan en el TTL.
  Aceptable para 900 s.
- Sin scopes por endpoint en la v1: el rol de la key define el acceso. Se deja
  como evolución si aparecen integraciones de solo lectura.

## Criterios de aceptación

- El gateway no contiene Basic Auth ni `/api-proxy`; CI y smoke verdes.
- `staging` solo es accesible a través del control de acceso externo.
- `POST /api/v2/auth/token` emite un JWT válido para una key activa y rechaza
  revocadas/expiradas con `401`/`403` normalizados.
- La key se muestra una vez, se guarda hasheada y se puede revocar.
- OpenAPI y reglas (`R-GW-8`) actualizados; `R-COV` y auditoría reflejan el
  cambio.

## Plan de PRs atómicos

| PR | Alcance |
|---|---|
| 1 | Este ADR + `R-GW-8` + mapa de reglas + estado de ADR-29 |
| 2 | Gateway sin Basic/`/api-proxy`; middleware de diagnóstico; compose/docs |
| 3 | `identity.api_keys` (migración 62), CRUD, intercambio por JWT, tests |
| 4 | Runbook de despliegue de staging y smoke; auditoría |

## Estado de implementación (2026-11-12)

| PR | Estado | Alcance |
|---|---|---|
| #571 | MERGED | ADR-0005 + R-GW-8 + estado de ADR-29 |
| #573 | MERGED | Gateway sin Basic ni `/api-proxy`; diagnóstico mínimo |
| #574 | MERGED | `identity.api_keys`, CRUD e intercambio por JWT |

Verificación:

- Gateway: 99 tests; identity: 279 tests; integración de API keys contra PostgreSQL 16 (alta, listado, token, revocación y rechazo posterior).
- `node scripts/cli.mjs rules` y `baseline:guard` verdes; OpenAPI de identity y agregado regenerados.
- Migración `62_api_keys.sql` registrada; política RLS de servicio en
  `infra/db/supabase/99_rls.sql`; `identity.api_keys` fuera del baseline de deuda.

**Pendiente operativo:** configurar el control de acceso externo de `staging`
(Cloudflare Access/VPN/allowlist) antes de desplegar; el gateway ya no protege
el entorno. Ver [`../runbook-gateway-auth.md`](../runbook-gateway-auth.md).
