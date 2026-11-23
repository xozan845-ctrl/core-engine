# Runbook — lanzamiento MVP: pagos sandbox, operación y smoke de staging

Checklist ejecutable para cerrar los tres frentes que faltan antes de un piloto
con dinero real. No contiene secretos: cada valor se inyecta en el entorno.

## 1. Pasarela en sandbox

### 1.1 Variables por proveedor

| Variable | Tilopay | Pagadito |
|---|---|---|
| `PAGO_PROVEEDOR` | `tilopay` | `pagadito` |
| `PAGO_PROVEEDOR_URL` | opcional; default `https://app.tilopay.com` | `https://sandbox.pagadito.com/comercios/apipg/charges.php` |
| Credenciales | `TILOPAY_APIUSER`, `TILOPAY_PASSWORD`, `TILOPAY_KEY` | `PAGADITO_UID`, `PAGADITO_WSK` |
| Moneda | ISO 4217 de la orden | `config.currency` o la moneda de la orden |

> **Tilopay no tiene host de sandbox.** Testing y producción comparten
> `app.tilopay.com`; el modo se cambia en la cuenta. Antes de probar, confirma
> que la cuenta está en **Test mode** y que `consult` devuelve
> `"environment": "Test"`. Si devuelve `"Production"`, detente: estarías
> cobrando dinero real. El adaptador acepta `config.ambiente="test"` y falla
> con `TILOPAY_AMBIENTE_INESPERADO` si `consult` reporta otro ambiente.

Pagadito sí tiene sandbox separado. Su APIPG devuelve la URL de pago
URL-encoded; el adaptador la decodifica.

### 1.2 Activar proveedor y método

Con un token de `admin`/`super_admin`:

```bash
# Proveedor (nace inactivo). En Tilopay sandbox añade ambiente=test.
curl -fsS -X PATCH "$BASE_URL/api/v2/admin/proveedores-pago/tilopay" \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"activo":true,"config":{"ambiente":"test"}}'

# Método de tarjeta (nace inactivo)
curl -fsS -X PATCH "$BASE_URL/api/v2/admin/metodos-pago/tarjeta" \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"activo":true}'
```

Si se usa el proveedor global (`PAGO_PROVEEDOR=tilopay|pagadito`), no hace falta
configuración por tenant. Si se usa por empresa:

```bash
curl -fsS -X PUT "$BASE_URL/api/v2/empresas/$EMPRESA_ID/configuracion-pagos" \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"proveedor_codigo":"tilopay","activo":true,"config":{}}'
```

Redesplegar `orders-service` después de cambiar variables de entorno.

### 1.3 E2E manual de una orden

1. Crear una orden con `metodo_pago=tarjeta` desde el frontend o el flujo de
   checkout del tenant piloto.
2. La respuesta de checkout debe incluir `pago_url`.
3. Abrir `pago_url` y pagar con una tarjeta de prueba del proveedor.
4. Verificar el pago y la orden:

```bash
curl -fsS "$BASE_URL/api/v2/admin/pagos?order_id=$ORDER_ID" \
  -H "authorization: Bearer $TOKEN"
curl -fsS "$BASE_URL/api/v2/orders/$ORDER_ID" \
  -H "authorization: Bearer $TOKEN"
```

Esperado: pago `confirmado`, `proveedor=tilopay|pagadito`, orden `pagada`.
La referencia interna de la pasarela queda en
`orders.pagos.referencia_proveedor` (no se expone en la API).

Evidencia en base:

```sql
SELECT estado, proveedor, referencia, referencia_proveedor, conciliado_en,
       confirmado_en, monto_cents
  FROM orders.pagos WHERE order_id = :order_id;

SELECT tipo, motivo, creado_en
  FROM orders.pagos_eventos
 WHERE pago_id = (SELECT id FROM orders.pagos WHERE order_id = :order_id)
 ORDER BY creado_en;
```

### 1.4 Probar la conciliación (webhook perdido)

Para validar que el job de 15 minutos funciona sin depender del webhook:

1. Crear otra orden con `tarjeta` sin configurar `webhook_url` en el proveedor
   (Tilopay) o deshabilitando temporalmente el webhook (Pagadito).
2. Pagar en sandbox.
3. No confirmar nada a mano. Esperar hasta `PAGOS_CONCILIACION_INTERVALO_SEGUNDOS`
   (900 s por defecto) + la ventana de gracia (10 min).
4. Verificar que el pago pasó a `confirmado` y la orden a `pagada`.

### 1.5 Reembolso

Tilopay soporta reembolso total/parcial por API:

```bash
curl -fsS -X POST "$BASE_URL/api/v2/admin/pagos/$PAGO_ID/reembolsar" \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"monto_cents":1000,"motivo":"prueba sandbox"}'
```

Pagadito no documenta reembolsos por API: se gestionan desde su panel. El
adaptador devuelve `PROVEEDOR_NO_SOPORTA_REEMBOLSO`, no finge éxito.

### 1.6 Rollback de la pasarela

1. `PATCH /api/v2/admin/metodos-pago/tarjeta {"activo":false}`.
2. `PATCH /api/v2/admin/proveedores-pago/{codigo} {"activo":false}`.
3. `PAGO_PROVEEDOR=simulado` en el entorno y redesplegar (solo desarrollo; el
   método `tarjeta` seguirá rechazando el simulado por seguridad).

## 2. Operación de producción

### 2.1 Secretos de backup/MinIO

Configurar en GitHub Environments `stage` y `prod` (no en el repo):

```bash
gh secret set BACKUP_DATABASE_URL --env prod
gh secret set MINIO_BACKUP_SOURCE --env prod
gh secret set MINIO_BACKUP_DEST --env prod
gh secret set MINIO_BACKUP_ACCESS_KEY --env prod
gh secret set MINIO_BACKUP_SECRET_KEY --env prod
gh secret set MINIO_BACKUP_DEST_ACCESS_KEY --env prod
gh secret set MINIO_BACKUP_DEST_SECRET_KEY --env prod
# Repetir con --env stage para stage.
```

Sin `BACKUP_DATABASE_URL` el workflow diario se omite con warning; sin los
secretos de MinIO, el espejo se omite. El backup local lo hace el sidecar
`db-backup` cada 24 h.

### 2.2 Restore drill

En una instancia limpia (nunca sobre la base de origen):

```bash
PGHOST=<host> PGUSER=<user> PGPASSWORD=<pass> PGDATABASE=<db> \
  sh infra/db/restore-verify.sh /backups/<archivo>.dump
```

Debe imprimir `restore verificado: ... con_rls=...`. Si falta RLS, el drill
falla: no declares el backup verificado.

Evidencia local ya ejecutada en este repo: migraciones completas en base limpia
(185 tablas), dump de 700 KB, checksum OK y restore con 5 esquemas, 182 tablas y
116 tablas con RLS.

### 2.3 Activar RLS en Dockploy

Seguir `docs/runbook-rls.md`. Resumen:

1. Backup reciente + restore drill OK.
2. En Dockploy → Environment, cambiar:
   ```dotenv
   DATABASE_URL=postgres://core_engine_app:<password>@<host>:5432/<db>
   CORE_ENGINE_APP_PASSWORD=<password>
   ```
3. Redesplegar. `db-migrate` asegura `LOGIN` y membership `authenticated`.
4. Verificar con token del tenant A que no ve datos del tenant B, que `/ready`
   sigue en verde y que el outbox publica.

Rollback: volver a `DATABASE_URL` con el rol owner y redesplegar. Las políticas
quedan, pero el owner las omite.

### 2.4 Healthchecks y alertas

- `GET /health` → 200 (liveness del gateway).
- `GET /ready` con `x-internal-key` → 200 y `"status":"ok"` (todos los
  servicios). En stage/prod `READY_PUBLIC=false`.
- Prometheus: `up == 0` para cualquier servicio debe disparar alerta.
- Alertmanager: probar una alerta sintética y confirmar que llega al canal.
- Revisar que `/docs`, `/docs-json` y `/metrics` no sean públicos.

## 3. Smoke E2E de staging

> **Ojo con la ref del workflow.** `gh workflow run deploy.yml` sin `--ref` usa
> la rama por defecto (`main`), no `staging`, y puede desplegar un compose/
> migraciones viejas contra imágenes `:stage`. Para stage usa siempre:
>
> ```bash
> gh workflow run deploy.yml --ref staging -f environment=stage
> ```
>
> Para producción, `--ref main -f environment=prod`.

Requisitos: dominio/TLS de staging, `INTERNAL_API_KEY`, credenciales de admin y,
si aplica, de empresario; tenant piloto con catálogo real.

```bash
# Solo si el borde de staging lo exige:
# export EDGE_BASIC_AUTH='usuario:clave'
BASE_URL=https://<staging> \
ADMIN_EMAIL=<admin> ADMIN_PASSWORD=<password> \
EMPRESARIO_EMAIL=<empresario> EMPRESARIO_PASSWORD=<password> \
SMOKE_EMPRESA_ID=<empresa-piloto> \
npm run smoke:remote
```

El smoke actual valida: `/health`, login, catálogo con datos, inventario,
configuración de inventario y ventas de empresa. No crea datos.

Comprobaciones adicionales:

```bash
curl -fsSI "https://<staging>/health"
curl -fsS "https://<staging>/ready" -H "x-internal-key: $INTERNAL_API_KEY"
curl -fsS "https://<staging>/api/v2/admin/proveedores-pago" \
  -H "authorization: Bearer $TOKEN"
curl -fsS "https://<staging>/api/v2/orders/metodos-pago" \
  -H "authorization: Bearer $TOKEN"
```

Esperado: TLS válido, `/ready` en `ok`, proveedor de sandbox activo y `tarjeta`
activo. Si el catálogo está vacío, el smoke falla a propósito: primero hay que
sembrar el tenant piloto.

## 4. Criterio de cierre

- Pago sandbox confirmado por webhook **y** por conciliación (webhook perdido).
- Reembolso parcial probado en Tilopay; limitación de Pagadito documentada.
- Backup diario con secretos configurados y restore drill verde.
- RLS activo con `core_engine_app` y rollback probado.
- `/health`, `/ready`, TLS y alertas verificados.
- Smoke remoto de staging verde con el tenant piloto.
