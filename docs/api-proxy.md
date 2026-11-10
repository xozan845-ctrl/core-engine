# Proxy público del API para el frontend de Firebase (ADR-29)

## Configuración del frontend

`public/config.json` y `environment.prod.ts` deben apuntar a:

```json
{ "apiUrl": "https://api.kbcoleccion.com/api-proxy/api/v2", "aiScraperUrl": "" }
```

El navegador ya no recibe ni envía el Basic. La API directa sigue con su gate.

## Cómo funciona

```text
Navegador (web.app)
  └─ POST https://api.kbcoleccion.com/api-proxy/api/v2/auth/login
       └─ gateway: handler /api-proxy (público)
            └─ añade Authorization: Basic … (EDGE_BASIC_AUTH del servidor)
                 └─ POST http://127.0.0.1:8080/api/v2/auth/login
```

- `login`, `registro`, `refresh`, `restablecer-contrasena`: Basic interno.
- Resto: se reenvía el `Bearer` del navegador; el gateway valida el JWT.
- CORS: solo los orígenes de `CORS_ORIGINS`; el preflight se responde en el proxy.
- Body máximo: 1 MB (los archivos van directos a MinIO con URL prefirmada).
- Respuesta en streaming; imágenes públicas incluidas.
- Timeout: 60 s en el proxy público. El gateway corta la espera a cada
  microservicio a los 30 s (`PROXY_UPSTREAM_TIMEOUT_MS`) y responde
  `504 DESTINO_TIMEOUT` con `detalles.servicio` y `detalles.timeout_ms`; si el
  cuerpo es `PROXY_TIMEOUT`, el que no respondió fue el propio gateway.
- Las validaciones internas contra storage (logo de empresa, avatar de vendedor
  y logo de tienda) tienen timeout de 4 s y fallan cerrado con
  `LOGO_NO_VERIFICABLE` / `AVATAR_NO_VERIFICABLE` (R-RB-4).

## Variables

- `EDGE_BASIC_AUTH` (staging): la misma del gate; ya existe.
- `CORS_ORIGINS`: debe incluir `https://kb-coleccion-internacional-ni.web.app`.
- No hay variables nuevas.

## Seguridad

La ruta `/api-proxy` es pública: cualquier cliente que la conozca puede usarla,
igual que puede usar el frontend público. **No es una frontera de seguridad**.
El Basic queda oculto del navegador, pero la autorización real es el JWT + RLS.
Si staging debe ser privado de verdad, usa Cloudflare Access/VPN o un proxy con
autenticación propia.

## Verificación

```sh
# Login sin Basic desde el navegador/proxy: debe llegar a identity
curl -i -X POST https://api.kbcoleccion.com/api-proxy/api/v2/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"correo":"noexiste@example.invalid","contrasena":"x"}'
# 400/401 del API, NO 401 del gate

# Catálogo público
curl -i 'https://api.kbcoleccion.com/api-proxy/api/v2/catalog/productos?empresa_id=11111111-1111-4111-8111-111111111111'

# Ruta directa sigue protegida
curl -i https://api.kbcoleccion.com/api/v2/catalog/productos
# 401 Basic
```

## Diagnóstico de un 504

| Cuerpo de la respuesta | Qué significa |
|---|---|
| `PROXY_TIMEOUT` | El proxy público agotó los 60 s esperando al gateway. |
| `DESTINO_TIMEOUT` con `detalles.servicio` | El gateway agotó los 30 s esperando a ese microservicio. |
| `LOGO_NO_VERIFICABLE` / `AVATAR_NO_VERIFICABLE` | Storage no confirmó la imagen en 4 s. |
| `DESTINO_NO_DISPONIBLE` | El microservicio rechazó la conexión o cerró la respuesta. |

La pestaña **Network** del navegador debe mostrar la URL exacta y el cuerpo; sin
esos dos datos un 504 no identifica al culpable.

## Rollback

Cambia de nuevo `apiUrl` a `https://api.kbcoleccion.com/api/v2` y despliega el
frontend. El proxy puede quedarse sin uso; no afecta a la API directa.
