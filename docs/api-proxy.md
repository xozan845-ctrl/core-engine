# Proxy público del API — retirado (ADR-0005)

El prefijo `/api-proxy` y el gate Basic de staging se retiraron junto con
`edge-auth` (ADR-0005, R-GW-8).

## Qué cambia para el frontend

- El frontend llama **directo** a `https://api.kbcoleccion.com/api/v2/...` con
  el `Authorization: Bearer <JWT>` del usuario.
- No hay credencial Basic que inyectar ni proxy intermedio.
- La privacidad de `staging` se resuelve en el borde externo (Cloudflare
  Access, VPN o allowlist de IP), no en el gateway.

## Clientes máquina

Las integraciones usan una **API key** emitida por `identity` y la intercambian
por un JWT corto:

```bash
curl -X POST https://api.kbcoleccion.com/api/v2/auth/token \
  -H 'x-api-key: cek_<prefijo>_<secreto>'
```

El resto de llamadas usan el `access_token` devuelto como Bearer. La key no se
envía a los microservicios ni se guarda en el navegador.

## Rollback

Si hiciera falta volver al proxy, se revierte el PR de ADR-0005 y se restaura
`EDGE_BASIC_AUTH` en `staging`. No se recomienda: reintroduce el bypass.
