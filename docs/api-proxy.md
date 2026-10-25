# Proxy público del API para el frontend de Firebase (ADR-29)

## Configuración del frontend

`public/config.json` y `environment.prod.ts` deben apuntar a:

```json
{ "apiUrl": "https://api.kbcoleccion.com/api-proxy/api/v1", "aiScraperUrl": "" }
```

El navegador ya no recibe ni envía el Basic. La API directa sigue con su gate.

## Cómo funciona

```text
Navegador (web.app)
  └─ POST https://api.kbcoleccion.com/api-proxy/api/v1/auth/login
       └─ gateway: handler /api-proxy (público)
            └─ añade Authorization: Basic … (EDGE_BASIC_AUTH del servidor)
                 └─ POST http://127.0.0.1:8080/api/v1/auth/login
```

- `login`, `registro`, `refresh`, `restablecer-contrasena`: Basic interno.
- Resto: se reenvía el `Bearer` del navegador; el gateway valida el JWT.
- CORS: solo los orígenes de `CORS_ORIGINS`; el preflight se responde en el proxy.
- Body máximo: 1 MB (los archivos van directos a MinIO con URL prefirmada).
- Respuesta en streaming; imágenes públicas incluidas.
- Timeout: 60 s. Un 504 de identity/DB sigue siendo del backend.

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
curl -i -X POST https://api.kbcoleccion.com/api-proxy/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"correo":"noexiste@example.invalid","contrasena":"x"}'
# 400/401 del API, NO 401 del gate

# Catálogo público
curl -i 'https://api.kbcoleccion.com/api-proxy/api/v1/catalog/productos?empresa_id=11111111-1111-4111-8111-111111111111'

# Ruta directa sigue protegida
curl -i https://api.kbcoleccion.com/api/v1/catalog/productos
# 401 Basic
```

## Rollback

Cambia de nuevo `apiUrl` a `https://api.kbcoleccion.com/api/v1` y despliega el
frontend. El proxy puede quedarse sin uso; no afecta a la API directa.
