# Guía: dominios personalizados por empresa / tienda

> **Estado**: propuesta de diseño (no implementado).
> **Objetivo**: que un `empresario` pueda registrar, verificar y administrar un
> dominio propio para su empresa o su tienda, con TLS automático, sobre la
> infraestructura actual (Docker + Dokploy + Traefik + PostgreSQL).
>
> Documentos relacionados: `docs/decisiones.md` (ADR-26 slugs, ADR-44 propuesto
> al final de esta guía), `docs/dockploy-setup.md`, `docs/adr/0021-negocios-por-empresa.md`.

---

## 0. Qué significa "gestionar su dominio" (alcance)

Hay que separar tres cosas que suelen confundirse:

| Capa | Quién la controla | Ejemplo |
|---|---|---|
| **DNS** (registro A/CNAME) | El cliente, en su registrador (GoDaddy, Cloudflare, Namecheap…) | `tienda.micliente.com → IP del VPS` |
| **TLS** (certificado) | La plataforma, automático | Let's Encrypt para `tienda.micliente.com` |
| **Routing + tenant** | La plataforma | Ese host sirve el storefront de la tienda `X` |

El empresario **no** debe tocar DNS de la plataforma ni certificados. Lo que
"gestiona" desde el panel es:

1. Declarar el host deseado (`tienda.micliente.com`).
2. Recibir instrucciones exactas de DNS y un token de verificación.
3. Ver el estado (`pendiente → verificando → activo / error`).
4. Poder cambiarlo o eliminarlo.
5. (Opcional) Elegir cuál de sus dominios es el principal.

Todo lo demás es automatización de plataforma.

### Decisión de producto previa

Antes de escribir código hay que elegir **a qué apunta el dominio del tenant**:

- **Opción A (recomendada): el dominio apunta al storefront**, no a la API.
  La API sigue en `api.plataforma.com`. El storefront resuelve el tenant por
  `Host`. Mucho menos riesgo: un dominio de cliente mal configurado no puede
  romper la autenticación ni el enrutado de la API.
- **Opción B: el dominio sirve API + storefront** en el mismo host. Exige que el
  gateway resuelva tenant por `Host`, toque CORS, cookies y políticas de borde.
  Es más caro y más frágil; solo tiene sentido si se quiere API "blanca".

Esta guía desarrolla A y deja B como fase opcional.

---

## 1. Arquitectura propuesta

```text
                    Internet
                       │
        ┌──────────────┴───────────────┐
        │  Traefik (Dokploy)  :80/:443 │
        │  - routers por Host          │
        │  - ACME / TLS                │
        └───────┬──────────────┬───────┘
                │              │
   api.plataforma.com    tienda.cliente.com
                │              │
        ┌───────▼──────┐  ┌────▼─────────────┐
        │ api-gateway  │  │ storefront (web) │
        │ /api/v2/*    │  │ resuelve tenant  │
        └───────┬──────┘  └────┬─────────────┘
                │              │ GET /api/v2/tiendas/por-dominio/{host}
                │              │ (o BFF interno)
        ┌───────▼──────────────▼───────┐
        │ microservicios Rust + Postgres│
        │ stores-service (dominios)     │
        └───────────────┬───────────────┘
                        │
              edge-manager (sidecar)
              - verifica DNS
              - reconcilia routers Traefik
              - escribe /etc/dokploy/traefik/dynamic/tenants.yml
```

Componentes nuevos:

1. **Tabla de dominios** (en `stores-service` para tiendas; `identity-service`
   para empresas si se quiere a nivel empresa).
2. **Endpoints de gestión** con autorización `empresario` / propietario.
3. **Verificador DNS** (worker) con estados y reintentos.
4. **`edge-manager`**: reconcilia la tabla con routers dinámicos de Traefik.
5. **Resolución pública** `host → tienda` para el storefront.
6. **TLS**: wildcard para subdominios de plataforma + certificado por host para
   dominios de cliente.

> **Por qué un `edge-manager` y no llamar a Dokploy por API**: Dokploy está
> pensado para dominios a nivel *environment* (uno o pocos por app), no para
> cientos de hosts de tenants con alta y baja continua. Crear un "domain" de
> Dokploy por tenant llena la UI, no se puede verificar y acopla el producto a
> la herramienta de despliegue. Traefik sí soporta routers dinámicos por archivo.

---

## 2. Fase 0 — Decisiones y ADR

Antes de tocar código, cerrar estas decisiones y registrarlas en un ADR nuevo
(el siguiente libre es **ADR-44**, ver `docs/decisiones.md`):

| Decisión | Recomendación |
|---|---|
| ¿Dominio por empresa o por tienda? | **Por tienda** en el MVP (es lo que el cliente percibe); por empresa después |
| ¿Cuántos dominios por tienda? | 1 principal + 1 alias (`www`) en el MVP; ampliable con cuota |
| ¿Quién lo gestiona? | `empresario` transversal de la empresa **o** propietario de la tienda |
| ¿Monetización? | Capacidad de negocio `dominios_personalizados` (ver §7) |
| ¿Verificación? | TXT `_coreengine.<host>` y/o CNAME a `edge.<plataforma>` |
| ¿TLS? | Let's Encrypt HTTP-01 por host; wildcard DNS-01 para `*.plataforma.com` |
| ¿Cloudflare delante? | Soportado, pero documentar `Full (strict)` y desactivar proxy si se usa HTTP-01 |
| ¿Dominios de plataforma? | Lista reservada + hosts ya usados por la plataforma y otros tenants |

Escribe el ADR con el formato existente: contexto, decisión numerada,
consecuencias, alternativas descartadas.

---

## 3. Fase 1 — Modelo de datos

### 3.1 Migración

El siguiente número libre es **93** (`infra/db/init/` termina en `92_analytics.sql`).
Crea `infra/db/init/93_dominios_tienda.sql` y actualiza el manifiesto con
`npm run db:migrations -- --update` (R-DB-6).

```sql
-- 93_dominios_tienda.sql — dominios personalizados de tienda (ADR-44)
-- Aditiva e idempotente (R-DB-6). RLS por empresa (R-DB-2/R-SB-2).

CREATE TABLE IF NOT EXISTS stores.tienda_dominios (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  negocio_id        uuid,
  tienda_id         uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
  host              text NOT NULL,
  es_principal      boolean NOT NULL DEFAULT false,
  estado            text NOT NULL DEFAULT 'pendiente'
                    CHECK (estado IN ('pendiente','verificando','activo','error','suspendido')),
  metodo            text NOT NULL DEFAULT 'txt'
                    CHECK (metodo IN ('txt','cname')),
  token             text NOT NULL,
  ultimo_error      text,
  intentos          integer NOT NULL DEFAULT 0,
  verificado_en     timestamptz,
  cert_estado       text NOT NULL DEFAULT 'no_emitido'
                    CHECK (cert_estado IN ('no_emitido','emitiendo','emitido','error')),
  creado_por        uuid NOT NULL,
  creado_en         timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en    timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT tienda_dominios_host_formato
    CHECK (host ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$'),
  CONSTRAINT tienda_dominios_host_len CHECK (char_length(host) BETWEEN 4 AND 253)
);

-- Un host pertenece a un solo tenant en toda la plataforma.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tienda_dominios_host
  ON stores.tienda_dominios (host);
-- Un solo dominio principal por tienda.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tienda_dominios_principal
  ON stores.tienda_dominios (tienda_id) WHERE es_principal;
CREATE INDEX IF NOT EXISTS idx_tienda_dominios_empresa
  ON stores.tienda_dominios (empresa_id, estado);

-- Historial: el host no se reutiliza nunca (mismo criterio que ADR-26).
CREATE TABLE IF NOT EXISTS stores.tienda_dominios_historial (
  host          text PRIMARY KEY,
  tienda_id     uuid NOT NULL,
  empresa_id    uuid NOT NULL,
  liberado_en   timestamptz NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION stores.fn_dominio_historial()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    INSERT INTO stores.tienda_dominios_historial (host, tienda_id, empresa_id)
    VALUES (OLD.host, OLD.tienda_id, OLD.empresa_id)
    ON CONFLICT (host) DO NOTHING;
    RETURN OLD;
  END IF;
  RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS trg_tienda_dominio_historial ON stores.tienda_dominios;
CREATE TRIGGER trg_tienda_dominio_historial
AFTER DELETE ON stores.tienda_dominios
FOR EACH ROW EXECUTE FUNCTION stores.fn_dominio_historial();

-- Un host liberado no se reutiliza nunca (mismo criterio que ADR-26).
CREATE OR REPLACE FUNCTION stores.fn_dominio_no_reutilizable()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM stores.tienda_dominios_historial WHERE host = NEW.host) THEN
    RAISE EXCEPTION 'El host % ya fue utilizado y no puede reutilizarse', NEW.host
      USING ERRCODE = 'unique_violation';
  END IF;
  RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS trg_tienda_dominio_no_reutilizable ON stores.tienda_dominios;
CREATE TRIGGER trg_tienda_dominio_no_reutilizable
BEFORE INSERT ON stores.tienda_dominios
FOR EACH ROW EXECUTE FUNCTION stores.fn_dominio_no_reutilizable();
```

RLS: replica el patrón de `infra/db/init/87_rls_negocio.sql` y
`infra/db/supabase/99_rls.sql`: `empresa_id = current_setting('app.empresa_id')::uuid`,
`FORCE ROW LEVEL SECURITY`, y grants solo a `core_engine_app`.

### 3.2 ¿Por qué en `stores` y no en `identity`?

Por coherencia con ADR-26: el slug de tienda vive en `stores` y el de empresa en
`identity`. Si el MVP es "dominio de tienda", la tabla vive junto a la tienda y
evita dependencias circulares. Si más adelante se agrega dominio de empresa, se
crea `identity.empresa_dominios` con el mismo patrón y el `edge-manager`
consulta ambas fuentes por API interna.

### 3.3 Datos iniciales

- `host`: minúsculas, IDN convertido a punycode (`ICANN`/IDNA), sin esquema,
  sin puerto, sin path, sin `www` implícito (se registra aparte si se quiere).
- `token`: 32 bytes aleatorios en hex; rotación al reintentar; caducidad 7 días.
- `es_principal`: solo uno por tienda; al activar uno, desmarcar el anterior en
  la misma transacción.

---

## 4. Fase 2 — API de gestión

### 4.1 Rutas

En `packages/stores-service/src/http/mod.rs`:

```rust
.route("/api/v2/tiendas/{id}/dominios", get(tiendas::listar_dominios).post(tiendas::crear_dominio))
.route("/api/v2/tiendas/{id}/dominios/{dominio_id}", delete(tiendas::eliminar_dominio))
.route("/api/v2/tiendas/{id}/dominios/{dominio_id}/principal", patch(tiendas::marcar_principal))
.route("/api/v2/tiendas/{id}/dominios/{dominio_id}/verificar", post(tiendas::verificar_dominio))
```

Ruta pública para el storefront (sin JWT, solo lectura y datos mínimos):

```rust
.route("/api/v2/tiendas/por-dominio/{host}", get(tiendas::por_dominio))
```

### 4.2 Reglas de autorización

En `packages/api-gateway/src/policy.rs`, añade (respetando deny-by-default):

```rust
p(r"^/api/v2/tiendas/[^/]+/dominios(/|$)", &["*"],
  Some(vec![VENDEDOR, EMPRESARIO, SUPER_ADMIN, ADMIN])),
p(r"^/api/v2/tiendas/por-dominio/[^/]+$", &["GET"], None),
```

Y en el servicio, la autorización fina (igual que `actualizar_slug`):

1. `super_admin`/`admin`: todo.
2. `empresario` con `x-tenant == tienda.empresa_id`: toda la empresa.
3. `vendedor` con `propietario_usuario_id == x-user-id`: solo su tienda.
4. Alternativa: permiso `tienda.dominios.gestionar` en `roles_empresa.permisos`
   (patrón de ADR-0019).

> No confíes solo en el gateway: el servicio es la autoridad (ADR-0001).

### 4.3 Validación del host (Rust)

Nuevo módulo `packages/stores-service/src/tiendas/dominio.rs`:

```rust
pub struct CrearDominio {
    pub host: String,
    pub es_principal: bool,
}

pub fn normalizar_host(bruto: &str) -> Result<String, ApiError> {
    let host = bruto.trim().to_ascii_lowercase();
    // Rechazar esquema, puerto, path, userinfo, comodines e IPs.
    if host.contains("://") || host.contains('/') || host.contains(':')
        || host.contains('@') || host.contains('*') || host.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(ApiError::dominio("HOST_INVALIDO", "Indica solo el nombre del dominio."));
    }
    // IDNA → punycode (crate `idna`), longitud y etiquetas.
    let ascii = idna::domain_to_ascii(&host)
        .map_err(|_| ApiError::dominio("HOST_INVALIDO", "El dominio no es válido."))?;
    if ascii.len() > 253 || !ascii.contains('.') {
        return Err(ApiError::dominio("HOST_INVALIDO", "El dominio no es válido."));
    }
    // Sufijo público (crate `psl`) para exigir un dominio registrable real.
    if psl::domain(ascii.as_bytes()).is_none() {
        return Err(ApiError::dominio("HOST_INVALIDO", "El dominio no es válido."));
    }
    // Lista reservada: hosts de plataforma, otros tenants y dominios ya liberados.
    Ok(ascii)
}
```

Comprobaciones adicionales:

- `host` no está en `stores.tienda_dominios` ni en
  `stores.tienda_dominios_historial` (409).
- `host` no pertenece a la lista reservada (`api.`, `www.`, `admin.`,
  `app.`, `staging.`, el dominio raíz de la plataforma…).
- Cuota: máximo por capacidad del negocio (§7).
- No permitir hosts que ya usan otro tenant aunque estén libres en DNS.

### 4.4 Respuesta

```json
{
  "data": {
    "id": "…",
    "host": "tienda.micliente.com",
    "estado": "pendiente",
    "es_principal": true,
    "verificacion": {
      "metodo": "txt",
      "registro": "_coreengine.tienda.micliente.com",
      "valor": "coreengine-verify=abc123…"
    }
  }
}
```

El frontend muestra instrucciones copiables ("crea este TXT en tu proveedor") y
un botón **Verificar ahora**.

---

## 5. Fase 3 — Verificación DNS

### 5.1 Flujo

```text
POST /dominios            → estado=pendiente, token nuevo
POST /dominios/{id}/verificar
        │
        ├─ resuelve TXT _coreengine.<host>  (hickory-resolver, timeout 5s)
        ├─ o CNAME <host> → edge.plataforma.com
        │
        ├─ coincide  → estado=activo, verificado_en=NOW
        └─ no        → estado=error, ultimo_error, intentos++
```

Reglas:

- Reintentos con backoff (1 min, 5 min, 15 min, 1 h…), máximo 20 intentos.
- Límite por IP y por usuario (evitar abuso de resolución DNS y de emisión de
  certificados).
- El worker corre cada 60 s para los `pendiente` con `intentos < 20`.
- `verificar` manual no salta el rate limit.
- Si el host deja de resolver el TXT **después** de estar activo, un chequeo
  diario lo pasa a `suspendido` (no borra el certificado de inmediato).

### 5.2 Por qué TXT y no solo CNAME

CNAME demuestra que el cliente controla el DNS y además deja el tráfico listo.
Pero un CNAME a un host compartido puede colisionar con la verificación de otro
tenant si la plataforma no valida el destino. Recomendación: **exigir TXT** para
la propiedad y **usar CNAME** para el tráfico. Aceptar CNAME solo como atajo en
el MVP, validando que el destino sea exactamente `edge.plataforma.com`.

### 5.3 Apex / dominio raíz

`micliente.com` no admite CNAME en DNS clásico. Opciones: registro `A` a la IP
del VPS, `ALIAS/ANAME` del proveedor, o CNAME flattening de Cloudflare. La guía
de UI debe explicar esto y validar el caso.

---

## 6. Fase 4 — Borde: Traefik + Dokploy

### 6.1 Lo que **no** hay que hacer

- No crear un "domain" de Dokploy por tenant. La UI y el ciclo de vida no están
  hechos para eso, y Dokploy no sabe de verificación ni de estados.
- No tocar los labels de `docker-compose.yml` por cada tenant.
- No reiniciar Traefik en cada alta.

### 6.2 Lo que sí

1. Un dominio de plataforma en Dokploy: `*.plataforma.com` (o
   `app.plataforma.com` + `*.app.plataforma.com`) apuntando al storefront.
2. Habilitar el **file provider** de Traefik con un directorio de configuración
   dinámica. En instalaciones Dokploy suele existir
   `/etc/dokploy/traefik/dynamic/`; **verifícalo** en tu instalación:

```bash
docker ps --format '{{.Names}}' | grep -i traefik
docker inspect <traefik> --format '{{json .Mounts}}' | jq
docker exec <traefik> cat /etc/traefik/traefik.yml
```

Busca en la configuración estática:

```yaml
providers:
  file:
    directory: /etc/dokploy/traefik/dynamic
    watch: true
```

Si no está, tienes tres caminos:

- **A**: añadirlo desde la configuración de Traefik de Dokploy (si tu versión lo
  permite) y reiniciar el contenedor de Traefik.
- **B**: montar el directorio en el contenedor de Traefik y reiniciarlo una vez.
- **C**: ejecutar un Traefik propio en el compose (80/443) y dejar el de Dokploy
  solo para el panel. Es la opción con más control y más mantenimiento.

3. El `edge-manager` escribe un único archivo `tenants.yml` de forma atómica
   (`tmp` + `rename`), y Traefik lo recarga solo (`watch: true`).

### 6.3 Plantilla de router dinámico

```yaml
# /etc/dokploy/traefik/dynamic/tenants.yml  (generado, no editar a mano)
http:
  routers:
    t-<tienda_id>:
      rule: "Host(`tienda.micliente.com`)"
      entryPoints: [websecure]
      service: storefront
      middlewares: [sec-headers]
      tls:
        certResolver: letsencrypt
        domains:
          - main: "tienda.micliente.com"

    t-<tienda_id>-http:
      rule: "Host(`tienda.micliente.com`)"
      entryPoints: [web]
      middlewares: [https-redirect]
      service: storefront

  middlewares:
    https-redirect:
      redirectScheme:
        scheme: https
        permanent: true
    sec-headers:
      headers:
        stsSeconds: 31536000
        stsIncludeSubdomains: true
        contentTypeNosniff: true
        frameDeny: true

  services:
    storefront:
      loadBalancer:
        servers:
          - url: "http://<storefront>:3000"
```

Notas:

- `rule` con `Host(...)` exacto, nunca un comodín por tenant.
- Un router por host; el nombre del router debe ser determinista
  (`t-<uuid>`) para poder borrarlo sin ambigüedad.
- El `service` es compartido; si hay varios storefronts por región, se elige por
  etiqueta del tenant.
- `tls.domains` fuerza el SAN correcto antes de pedir el certificado.

### 6.4 TLS

| Caso | Método | Notas |
|---|---|---|
| `*.plataforma.com` | ACME **DNS-01** | Un certificado wildcard, sin límites por tenant. Requiere API del proveedor DNS (Cloudflare, Route53…) |
| `tienda.micliente.com` | ACME **HTTP-01** | Se emite al añadir el router, **después** de verificar el TXT. Requiere puerto 80 accesible |
| Cliente con Cloudflare proxied | TLS en Cloudflare (`Full strict`) | Traefik puede usar certificado de origen o Let's Encrypt; documentar |

Advertencias:

- Let's Encrypt limita los certificados por dominio registrado y semana. Emite
  **solo** para hosts verificados y cachea el resultado; no reintentes en bucle.
- Si el cliente activa el proxy de Cloudflare y bloquea HTTP-01, el certificado
  falla. Alternativa: DNS-01 delegada (el cliente crea el CNAME de validación) o
  Cloudflare for SaaS.
- No uses `onDemand` a ciegas: permite pedir certificados para hosts no
  verificados y es un vector de abuso de cuota.

### 6.5 `edge-manager` (reconciliador)

Contrato mínimo:

```text
cada 15 s:
  deseado := SELECT host, tienda_id FROM stores.tienda_dominios
             WHERE estado='activo' AND cert_estado IN ('no_emitido','emitido')
  actual   := leer tenants.yml
  diff     := altas, bajas, cambios
  si diff: escribir tenants.yml.tmp + rename
  actualizar cert_estado según logs/estado de Traefik (o marcar 'emitiendo'
  y confirmar con un HEAD HTTPS al host)
```

Recomendaciones:

- Idempotente y tolerante a fallos: si Traefik no está, reintenta.
- Nunca borra el archivo completo: solo las entradas huérfanas.
- Métricas: dominios activos, errores de emisión, tiempo de reconciliación.
- Logs con `host` y `tienda_id`, sin tokens.
- Si se despliega como contenedor, monta el directorio del host:
  `-v /etc/dokploy/traefik/dynamic:/dynamic`.

### 6.6 ¿Y si se prefiere no tocar Traefik?

Alternativa "Cloudflare for SaaS" (Custom Hostnames): el cliente hace CNAME a
`ssl.plataforma.com`, Cloudflare emite y renueva certificados, y el origen solo
ve un host interno. Implica coste y dependencia de Cloudflare, pero elimina la
gestión de ACME por tenant. Sirve como plan B si el file provider de Dokploy no
es viable.

---

## 7. Fase 5 — Storefront y resolución de tenant

### 7.1 Endpoint público

```http
GET /api/v2/tiendas/por-dominio/tienda.micliente.com
```

Respuesta mínima (nada de datos privados):

```json
{
  "data": {
    "tienda_id": "…",
    "empresa_id": "…",
    "negocio_id": "…",
    "slug": "mi-tienda",
    "nombre": "Mi Tienda",
    "tema": { "…": "…" }
  }
}
```

Reglas:

- Solo responde para dominios `activo`; si no, 404 genérico (no revelar que el
  dominio existe pero está pendiente).
- Cachea en el storefront (p. ej. 60 s) y en el gateway (capa en memoria) para
  no golpear Postgres en cada request.
- `Cache-Control: public, max-age=60` y `Vary: Host` si hay CDN delante.

### 7.2 Storefront

En el middleware de la app web:

```ts
const host = req.headers.get('host')!.split(':')[0].toLowerCase();
const tienda = await resolverTienda(host); // cacheado
if (!tienda) return notFound();
```

- El host propio **no** debe confundirse con el dominio de la plataforma: si es
  `app.plataforma.com`, se usa la ruta `/tienda/{slug}` como hoy.
- Enlaces internos: construir URLs relativas para no filtrar el host canónico.
- `canonical` por defecto al dominio principal de la tienda; los alias
  redirigen 301 al principal.

### 7.3 CORS y API

Si el storefront llama a la API directamente desde el navegador, `CORS_ORIGINS`
estático deja de servir: cada dominio de tenant es un origen nuevo. Opciones:

1. **BFF (recomendada)**: el storefront proxya `/api/*` desde su servidor; el
   navegador solo habla con el mismo origen. No hay que tocar CORS.
2. **CORS dinámico en el gateway**: validar `Origin` contra
   `stores.tienda_dominios` (cacheado) y reflejarlo solo si está `activo`.
   Requiere invalidación de caché y cuidado con `Vary: Origin`.
3. **Comodín** `*.plataforma.com` solo para subdominios de plataforma, nunca para
   dominios de cliente.

No uses `Access-Control-Allow-Origin: *` con credenciales. Revisa también
`cross-origin-resource-policy: same-site` en `packages/api-gateway/src/http/mod.rs`:
con dominios de cliente conviene `cross-origin` para los recursos públicos
(imágenes del storefront) o servirlos desde el mismo host.

### 7.4 Cookies y sesión

- Si el login vive en `api.plataforma.com` y el storefront en el dominio del
  cliente, la cookie de sesión es de otro sitio: necesita `SameSite=None;
  Secure` o, mejor, sesión gestionada por el BFF.
- No compartas cookies de panel (`admin`) con dominios de tenant.
- `Domain` de cookie: nunca el dominio del cliente; usa el host de la API.

---

## 8. Fase 6 — Cuotas, capacidades y multi-negocio

El modelo ya tiene capacidades por negocio (`identity.negocio_capacidades`,
ADR-0019). Añade una capacidad nueva:

```text
dominios_personalizados   (booleano)
dominios_max              (entero, p. ej. 1, 3, 10)
```

Reglas:

- Sin `dominios_personalizados`, el panel muestra "mejora tu plan" y la API
  responde 403 `CAPACIDAD_NO_DISPONIBLE`.
- La cuota se evalúa en el negocio dueño de la tienda (`negocio_id`), no en el
  usuario.
- Un `empresario` transversal puede gestionar los dominios de todas las tiendas
  de su empresa; un `empresario` acotado a un negocio, solo las de ese negocio.
- Al crear un segundo negocio (que ya exige `empresario`), la capacidad se
  hereda del pack, no del negocio principal.

Esto responde a tu caso: el usuario con una tienda que "se vuelve empresario"
no recibe el privilegio por el rol, sino por la **capacidad del plan**; el rol
solo le da la autorización para administrarlo.

---

## 9. Fase 7 — Seguridad (imprescindible)

Checklist mínimo antes de exponer dominios de clientes:

- [ ] **Host header**: normalizar y comparar contra la tabla; nunca confiar en
      `Host` para decidir tenant sin lookup.
- [ ] **Domain takeover**: verificar propiedad (TXT) antes de activar; no
      permitir hosts huérfanos ni reutilizar hosts liberados.
- [ ] **Reservados**: `api`, `www`, `admin`, `app`, `staging`, `mail`, el dominio
      raíz de la plataforma y los de otros tenants.
- [ ] **IDN/punycode**: normalizar y mostrar el nombre Unicode, pero almacenar y
      enrutar el ASCII; evitar homógrafos.
- [ ] **Sin comodines de tenant**: `*.cliente.com` no se acepta.
- [ ] **TLS**: solo emitir tras verificación; HSTS; redirect 80→443; TLS 1.2+.
- [ ] **Certificados**: límite de emisiones por usuario/hora; alerta de errores;
      no reintentar indefinidamente.
- [ ] **Auditoría**: quién creó/cambió/eliminó un dominio (`identity.auditoria`
      o tabla equivalente), sin guardar tokens en claro en logs.
- [ ] **Rate limit** en `verificar` y en `por-dominio`.
- [ ] **Borrado**: al eliminar tienda, borrar routers y marcar host como
      liberado (nunca reutilizable).
- [ ] **CSP**: si el storefront sirve contenido de tenant, revisar
      `frame-ancestors` y `img-src`.
- [ ] **SSRF**: el verificador solo resuelve DNS; no hace HTTP a URLs del cliente.
- [ ] **Cache poisoning**: claves de caché por `Host` + tenant, no solo por path.

---

## 10. Fase 8 — Operación y pruebas

### 10.1 Pruebas

Unitarias (Rust):

- `normalizar_host`: mayúsculas, IDN, puerto, path, IP, comodín, longitud,
  sufijo público inválido, reservados.
- Autorización: propietario, empresario de la empresa, empresario de otra
  empresa, vendedor ajeno, sin capacidad.
- Cuota: 1.º OK, 2.º 403, borrar y volver a crear.
- Principal único: activar B desmarca A.

PostgreSQL (`tests/stores_pg.rs`):

- Unicidad global de `host` y 409 amigable.
- Historial impide reutilización.
- RLS: otro tenant no ve ni modifica.
- Cascada al borrar tienda + trigger de historial.

Integración:

- Alta → TXT simulado (resolver mock) → activo → aparece en `tenants.yml`.
- Baja → desaparece del YAML y el host queda en historial.
- `GET /por-dominio` 200 para activo, 404 para pendiente.

E2E (staging):

- Dominio real de prueba con Cloudflare/Route53: DNS, TLS, redirect, storefront.
- Renovación de certificado (forzar con `--dry-run` o staging de ACME).

### 10.2 Runbook

Documenta en `docs/runbook-dominios.md`:

1. Cómo verificar el file provider de Traefik.
2. Cómo inspeccionar el estado: `SELECT host, estado, cert_estado, ultimo_error…`.
3. Cómo reprocesar un dominio atascado.
4. Cómo revocar un dominio comprometido (borrar router + suspender).
5. Qué hacer si Let's Encrypt devuelve rate limit.
6. Rollback: desactivar la feature flag `DOMINIOS_PERSONALIZADOS=false`; los
   routers existentes se pueden dejar o limpiar con el reconciliador.

### 10.3 Feature flag

`DOMINIOS_PERSONALIZADOS=true|false` en el servicio y en el `edge-manager`.
Apagado: la API responde 404/403 y el reconciliador no escribe routers nuevos.
Permite desplegar el código sin exponer la función.

### 10.4 Observabilidad

- Métricas: altas, activos, pendientes, errores de verificación, errores de
  emisión, latencia de reconciliación.
- Alertas: >5 dominios en `error` 15 min; cert expira en <14 días; Traefik sin
  recargar 10 min.
- Logs estructurados con `host`, `tienda_id`, `empresa_id`, `estado`.

---

## 11. Plan de entrega sugerido

| Sprint | Entregable | Estimación |
|---|---|---|
| 0 | ADR-44, decisiones de producto, lista de reservados | 2–3 días |
| 1 | Migración 93 + RLS + modelo + pruebas PG | 3–4 días |
| 2 | API CRUD + autorización + validación + cuotas | 4–5 días |
| 3 | Verificador DNS + estados + worker | 3–5 días |
| 4 | `edge-manager` + Traefik file provider + TLS | 5–7 días |
| 5 | Endpoint `por-dominio` + storefront + CORS/BFF | 4–6 días |
| 6 | Seguridad, runbook, métricas, E2E staging | 4–5 días |

Total realista: **4–6 semanas** de una persona con dedicación completa para un
MVP sólido. Un "solo subdominios `{slug}.plataforma.com`" se puede tener en
**2–3 días** (wildcard DNS + wildcard TLS + resolución por subdominio), y es un
buen primer paso que no requiere verificación ni certificados por cliente.

---

## 12. Alternativas descartadas

| Alternativa | Por qué no |
|---|---|
| Un "domain" de Dokploy por tenant | No escala, no tiene verificación, acopla el producto a Dokploy, llena la UI |
| `Host` → tenant en el gateway con tabla en memoria | Duplica la autoridad de stores y complica la invalidación; mejor resolver en el storefront/BFF |
| Comodín `*.<dominio-cliente>` | Peligroso (takeover, certificados, colisiones) |
| Certificados on-demand sin verificación | Abuso de cuota de Let's Encrypt y phishing |
| Guardar el dominio solo en el frontend | Sin fuente de verdad, sin auditoría, sin TLS automatizado |

---

## 13. Resumen para decidir

- **Sí es posible**, pero no es un cambio de una tarde: es una feature de
  infraestructura + datos + seguridad.
- La ruta corta y de bajo riesgo es **subdominios de plataforma**
  (`{slug}.tudominio.com`), que ya encaja con los slugs de ADR-26.
- La ruta completa (dominio propio del cliente) exige: tabla + verificación DNS
  + routers dinámicos de Traefik + ACME + resolución en el storefront.
- El rol `empresario` no debería ser el que "da" el privilegio: la autorización
  la da el rol, la **capacidad del plan** decide si puede tener dominio propio.
- Con Dokploy, la clave es **no gestionar tenants desde Dokploy**: un solo
  dominio de plataforma en la UI y el resto por configuración dinámica de
  Traefik desde tu propio reconciliador.
