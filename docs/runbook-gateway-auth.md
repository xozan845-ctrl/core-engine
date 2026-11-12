# Runbook — autenticación del gateway y API keys (ADR-0005)

## 1. Desplegar `staging` sin el gate Basic

**Prerrequisito obligatorio:** `staging` debe estar cerrado en el borde externo
(Cloudflare Access, VPN o allowlist de IP en Traefik/Dockploy). El gateway ya no
implementa Basic Auth ni `/api-proxy`; sin ese control, el entorno queda público.

Orden:

1. Configurar el acceso externo y verificar que sin sesión/VPN la API responde
   `403`/`302` en el borde, no en el gateway.
2. Promover `develop → staging` (R-GE-7).
3. Verificar la CI de `staging` (R-PR-8).
4. Ejecutar el smoke (desde dentro del perímetro o con el service token del
   borde):

   ```bash
   BASE_URL=https://<staging> \
   ADMIN_EMAIL=... ADMIN_PASSWORD=... \
   npm run smoke:remote
   ```

5. Rotar/eliminar `EDGE_BASIC_AUTH` y `REQUIRE_EDGE_AUTH` de los entornos.

Diagnóstico:

- `/health` público.
- `/ready` y `/docs*`/`/metrics` exigen `x-internal-key` (404 sin ella) salvo
  `READY_PUBLIC`/`DIAGNOSTICOS_PUBLICOS=true`.

## 2. API keys de integración

Crear (empresario sobre su tenant; admin con `empresa_id`):

```bash
curl -X POST https://api.example.com/api/v2/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"nombre":"Contabilidad","rol":"vendedor","expira_en_dias":90}'
```

La respuesta incluye `key` **una sola vez**. Guardarla en el gestor de secretos
del cliente; el servidor solo conserva el hash SHA-256 y el prefijo.

Intercambiar por JWT corto (900 s por defecto):

```bash
curl -X POST https://api.example.com/api/v2/auth/token \
  -H 'x-api-key: cek_<prefijo>_<secreto>'
```

Usar el `access_token` como `Authorization: Bearer` en el resto de llamadas.
Sin refresh: volver a intercambiar al caducar.

Listar y revocar:

```bash
curl https://api.example.com/api/v2/api-keys -H "Authorization: Bearer $TOKEN"
curl -X DELETE https://api.example.com/api/v2/api-keys/<id> \
  -H "Authorization: Bearer $TOKEN"
```

Reglas de uso:

- La key **nunca** va al navegador ni a un microservicio; solo al endpoint de
  intercambio por TLS.
- Roles permitidos: `empresario`, `vendedor`, `logistica`, `coordinador`,
  `supervisor`, `operativo`. Nunca `admin`/`super_admin`.
- Rotación: crear una key nueva, desplegar el cliente con ella y revocar la
  anterior. Los JWT ya emitidos caducan en el TTL.
- El endpoint de intercambio comparte el límite estricto de 10/min con login.

## 3. Rollback

Revertir el PR de ADR-0005 restaura Basic y `/api-proxy`, y exige volver a
configurar `EDGE_BASIC_AUTH` en `staging`. No se recomienda: reintroduce el
bypass y la credencial estática.
