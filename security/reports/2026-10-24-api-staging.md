# Revisión de seguridad · `api.kbcoleccion.com` (staging)

**Fecha:** 2026-10-24
**Modo:** caja gris (código del gateway en este repo) + pruebas externas no destructivas
**Objetivo:** `https://api.kbcoleccion.com` → `179.236.225.19` (`srv2020744.hstgr.cloud`)
**Autorización:** el propietario del entorno solicitó la revisión.

## Estado de remediación (2026-10-24)

| Hallazgo | Estado | Cambio aplicado |
| --- | --- | --- |
| H-01 · `:8080` en HTTP plano | **Corregido en repo** | `127.0.0.1:8080:8080` en stage y prod; falta firewall en el VPS |
| H-02 · `/ready` público y sin límite | **Corregido en repo** | `x-internal-key` (o `READY_PUBLIC=true` en dev), 10 req/min y caché de 5 s |
| H-03 · rate limit compartido | **Corregido en repo** | `trust proxy` configurable (`TRUST_PROXY`, por defecto 1) |
| M-01 · staging sin gate | **Corregido en repo** | `REQUIRE_EDGE_AUTH=true` en stage: el arranque falla sin `EDGE_BASIC_AUTH`; Bearer inválido ya no salta el Basic |
| B-01 · cabeceras incompletas | **Corregido en repo** | `Permissions-Policy`, COOP, COEP, CORP y `X-Permitted-Cross-Domain-Policies` |
| B-02 · SSH SHA-1 | **Pendiente de aplicar en host** | `security/hardening/sshd_config.d/99-core-engine.conf` |
| B-03 · TLS 1.2 con CBC | **Pendiente de aplicar en host** | `security/hardening/traefik-tls.yml` |

Los cambios de repositorio no afectan al staging en ejecución hasta redesplegar
la imagen `:stage` y actualizar las variables del entorno en Dockploy.

## Resumen ejecutivo

El gateway expone una superficie razonablemente endurecida: TLS 1.2/1.3 con
ciphers A, HSTS, CSP, CORS con whitelist, `x-powered-by` desactivado, `/docs` y
`/metrics` cerrados, `.env` fuera de git y denegación por defecto de rutas no
autenticadas (401 en vez de 404). No se encontraron vulnerabilidades críticas ni
altas explotables en el escaneo no intrusivo.

Sí hay tres problemas que conviene corregir antes de producción:

1. **El puerto 8080 publica el gateway en HTTP plano**, saltándose TLS y el
   proxy de borde.
2. **`/ready` es público, no tiene rate limit y hace fan-out a 8 servicios**:
   vector de amplificación/DoS y filtración de topología.
3. **El rate limit se calcula sobre la IP del socket y no hay `trust proxy`**:
   detrás de Caddy todos los clientes comparten el mismo cubo de 300/min (y el
   de login de 10/min), lo que permite denegar el servicio o bloquear todos los
   inicios de sesión con muy poco tráfico.

## Hallazgos

### H-01 · Gateway en HTTP plano en `:8080` (Alto)

`http://api.kbcoleccion.com:8080/` responde directamente con las cabeceras del
gateway (`x-ratelimit-limit: 300`, CSP, HSTS…). El `docker-compose.stage.yml`
publica `8080:8080` en todas las interfaces.

Impacto:

- Bypass de TLS: credenciales y tokens viajan en claro si algún cliente usa ese
  puerto.
- Bypass de cualquier control del proxy de borde (Caddy): WAF, ACLs, límites o
  autenticación adicional.
- Superficie extra innecesaria.

Recomendación:

- Publicar solo en loopback: `127.0.0.1:8080:8080`, o eliminar el `ports` y
  dejar que Caddy lo alcance por la red interna de Docker.
- Cortar `8080/tcp` en el firewall del VPS (UFW/nftables/security group).
- Verificar que producción no tenga el mismo `ports`.

Evidencia:

```text
$ curl -sD- http://api.kbcoleccion.com:8080/ -o /dev/null
HTTP/1.1 404 Not Found
x-ratelimit-limit: 300
Strict-Transport-Security: max-age=31536000; includeSubDomains
```

### H-02 · `/ready` público, sin rate limit y con fan-out (Alto)

`GET /ready` responde `200` sin autenticación y devuelve el estado de los 8
microservicios. La ruta se registra directamente en `expressApp` antes de que
Nest monte `RateLimitMiddleware`, por lo que **no lleva cabeceras de rate
limit** (igual que `/health`). Cada petición dispara 8 `fetch` en paralelo
(`readiness.utils.ts`), con timeout de 1,5 s.

Impacto:

- Amplificación: ~1 petición externa → 8 peticiones internas.
- DoS de bajo coste contra microservicios y gateway (latencia observada
  0,26–0,68 s por petición).
- Divulgación de topología y estado interno (nombres de servicios).

Recomendación:

- Exigir el gate de borde (`EDGE_BASIC_AUTH`) o `x-internal-key` también para
  `/ready`, y limitarlo a la red interna / healthcheck de Dockploy.
- Aplicar rate limit explícito a `/ready` (p. ej. 10/min por IP) o cachear el
  resultado 5–10 s.
- Mantener `/health` abierto (liveness), pero sin datos de uptime si no es
  necesario.

Evidencia:

```text
$ curl -s https://api.kbcoleccion.com/ready
{"status":"ok","servicios":{"identity":"ok","field":"ok","storage":"ok", ...}}
# Sin x-ratelimit-*; 3 peticiones: 0.677 s / 0.285 s / 0.263 s
```

### H-03 · Rate limit compartido detrás del proxy (Alto/Medio)

`RateLimitMiddleware` usa `req.ip`, pero `main.ts` no configura
`app.set('trust proxy', ...)`. Express, con `trust proxy` desactivado, usa la IP
del socket: la del contenedor de Caddy, no la del cliente. Además,
`X-Forwarded-For` se ignora.

Evidencia de que los cubos son distintos: tras varias peticiones por `:443`,
`x-ratelimit-remaining` iba por 298/297/296; una petición por `:8080` mostraba
299 (otro socket/origen). Si el gateway viera la IP real del cliente en ambos
casos, sería el mismo cubo.

Impacto:

- Los 300 req/min globales son **para todos los clientes juntos** en `:443`.
  Un atacante puede agotarlos y dejar sin servicio a todo el mundo.
- El límite de login (10/min) también es compartido: 10 POST/min bloquean el
  login de todos los usuarios (lockout global).
- Con varias réplicas del gateway el límite es por réplica (memoria local).

Recomendación:

- Configurar `app.set('trust proxy', 1)` (o el número exacto de proxies) y
  validar que Caddy sobrescribe `X-Forwarded-For`.
- Usar un almacén compartido (Redis) para el rate limit si hay más de una
  réplica.
- Limitar login por combinación IP + cuenta, y añadir backoff/lockout por
  cuenta con protección anti-enumeración.
- Añadir tests que cubran `req.ip` con y sin proxy.

### M-01 · Staging sin gate de borde (Medio)

`EDGE_BASIC_AUTH` no está configurado: `/ready` y toda la API son públicos
(aunque las rutas de negocio exijan JWT). El propio código documenta el gate
como medida para entornos que no deben ser públicos.

Recomendación: definir `EDGE_BASIC_AUTH` en staging o restringir por IP/VPN.
Si se activa, recordar que `/health` queda siempre abierto por diseño.

### B-01 · Cabeceras de seguridad incompletas (Bajo)

Presentes: `Strict-Transport-Security`, `Content-Security-Policy`,
`X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`,
`Referrer-Policy: no-referrer`.

Faltan (detectadas por nuclei `http-missing-security-headers`):

- `Permissions-Policy`
- `Cross-Origin-Opener-Policy`
- `Cross-Origin-Embedder-Policy`
- `Cross-Origin-Resource-Policy`
- `X-Permitted-Cross-Domain-Policies`

Recomendación: añadirlas en el middleware de `main.ts`. Para una API JSON,
`Permissions-Policy: interest-cohort=()` (o `camera=(), microphone=(), geolocation=()`),
`Cross-Origin-Resource-Policy: same-site` y `X-Permitted-Cross-Domain-Policies: none`
son un buen punto de partida.

### B-02 · SSH con HMAC SHA-1 habilitado (Bajo)

nuclei detectó `ssh-sha1-hmac-algo` en `:22` (OpenSSH 10.2p1 Ubuntu 2ubuntu3.8).
SHA-1 sigue siendo aceptable en HMAC, pero conviene deshabilitar algoritmos
legacy y restringir SSH por IP/VPN.

Recomendación: `KexAlgorithms`, `MACs` y `Ciphers` modernos; `PasswordAuthentication no`
si se usa solo clave; fail2ban o equivalente.

### B-03 · TLS 1.2 con preferencia de cipher del cliente (Bajo)

`ssl-enum-ciphers` da A en todos los ciphers y solo TLS 1.2/1.3, pero marca
`cipher preference: client` en TLS 1.2 y una advertencia de fuerza de key
exchange (secp256r1) frente a la clave del certificado.

Recomendación: forzar preferencia del servidor en TLS 1.2 y priorizar
ECDHE con X25519/secp384r1. Es un ajuste menor de Caddy.

### I-01 · Divulgación de uptime (Informativo)

`/health` devuelve `{"api_gateway":"ok","servicio":"api-gateway","uptime_seg":2207}`.
El uptime permite inferir reinicios/despliegues. No es grave, pero puede
eliminarse del liveness público.

## Lo que está bien

- `/docs`, `/docs-json` y `/metrics` devuelven 404 sin `x-internal-key`.
- CORS con whitelist: un `Origin: https://evil.example` no recibe
  `Access-Control-Allow-Origin`.
- `x-powered-by` desactivado; cabeceras base de hardening presentes.
- Rutas desconocidas devuelven 401 (deny-by-default), no 404 enumerable.
- `.env` no está trackeado y `.gitignore` cubre `.env*` (solo se versionan
  plantillas).
- TLS moderno, HSTS de un año con `includeSubDomains`, HTTP/2 y HTTP/3.
- No se encontraron CVEs altas/críticas con el set de plantillas usado.

## Pruebas ejecutadas

| Prueba | Herramienta | Resultado |
| --- | --- | --- |
| Puertos top-100 + versiones | nmap (Connect) | 22, 80, 443, 8080 abiertos |
| TLS/ciphers | nmap `ssl-enum-ciphers` | TLS 1.2/1.3, A |
| Fingerprint HTTP | httpx | 404 en `/`, HSTS, HTTP/3 |
| Vulnerabilidades web | nuclei (tech, exposures, misconfig, ssl) | 10 info, 0 high/critical |
| Cabeceras/CORS | curl | ver hallazgos |
| Carga baseline `/health` | k6, 5 VUs / 20 s | 50 req, 0 fallos, p95 183 ms, max 480 ms |

No se ejecutaron: explotación, fuerza bruta, pruebas de credenciales, fuzzing
masivo, pruebas de `/ready` bajo carga ni denegación de servicio.

## Próximos pasos sugeridos

1. Cerrar `:8080` (H-01) y desplegar el cambio en staging.
2. Proteger y limitar `/ready` (H-02).
3. Corregir `trust proxy` + rate limit compartido (H-03).
4. Activar `EDGE_BASIC_AUTH` en staging (M-01).
5. Repetir el escaneo tras los cambios y, si se autoriza, hacer una prueba de
   carga controlada sobre un endpoint de negocio con datos de prueba.

## Verificación post-deploy

```sh
# 1) 8080 ya no responde en HTTP plano
curl -m 5 http://api.kbcoleccion.com:8080/ || echo "8080 cerrado"

# 2) /ready oculto sin clave y operativo con la clave interna
curl -s -o /dev/null -w '%{http_code}\n' https://api.kbcoleccion.com/ready
curl -s -o /dev/null -w '%{http_code}\n' \
  -H "x-internal-key: $INTERNAL_API_KEY" https://api.kbcoleccion.com/ready

# 3) cabeceras nuevas
curl -sI https://api.kbcoleccion.com/health | grep -iE \
  'permissions-policy|cross-origin|x-permitted'

# 4) rate limit por IP real (dos clientes distintos deben tener cubos distintos)
curl -sI https://api.kbcoleccion.com/ | grep -i x-ratelimit

# 5) TLS sin CBC y SSH sin SHA-1
nmap -p443 --script ssl-enum-ciphers api.kbcoleccion.com
nmap -p22 --script ssh2-enum-algos api.kbcoleccion.com
```

Criterios de aceptación:

- `:8080` no devuelve JSON del gateway desde fuera del VPS.
- `/ready` sin clave devuelve 404; con `x-internal-key` devuelve 200/503.
- `Permissions-Policy`, COOP, COEP, CORP y `X-Permitted-Cross-Domain-Policies`
  aparecen en las respuestas.
- `x-ratelimit-remaining` no decrece de forma global entre clientes distintos.
- `ssl-enum-ciphers` no lista suites CBC; `ssh2-enum-algos` no lista SHA-1.

## Riesgo residual

- El rate limit sigue siendo **en memoria**: con más de una réplica del gateway
  cada réplica tiene su propio cubo. Si se escala horizontalmente, moverlo a
  Redis o a un limitador del proxy de borde.
- `TRUST_PROXY=1` asume **un** proxy (Traefik/Dockploy). Si se añade otro salto
  (CDN, WAF), ajustar el número o validar la cadena `X-Forwarded-For`.
- Los cambios de SSH/TLS son del host y no se aplican con el despliegue de la
  imagen; requieren la intervención descrita en `security/hardening/README.md`.
