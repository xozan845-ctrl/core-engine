# Runbook — migración multi-vertical

Este runbook cubre la adopción de packs por tenants existentes y la validación
de los tres pilotos: farmacia/retail, belleza y flota. La migración es aditiva;
no elimina columnas, capacidades ni datos legacy.

## Precondiciones

1. Ejecutar las migraciones 24, 29 y 30 en el entorno objetivo.
2. Confirmar backup y ventana de observación.
3. Verificar que `pack_migraciones` no tenga filas `requiere_revision`.
4. Mantener activo el flag de compatibilidad legacy hasta cerrar los pilotos.

## Verificación del backfill

La auditoría automatizada de cierre se ejecuta con una conexión de solo lectura
o de servicio restringida:

```bash
TEST_DATABASE_URL=postgres://core_engine_test:test_only@127.0.0.1:5432/core_engine_test \
  npm run multi-vertical:check
```

El comando termina con código distinto de cero ante cualquier inconsistencia.

```sql
SELECT estado, COUNT(*)
  FROM identity.pack_migraciones
 GROUP BY estado;

SELECT e.id, e.nombre, e.vertical_id, e.pack_codigo, e.pack_version,
       pm.estado, pm.detalles
  FROM identity.empresas e
  LEFT JOIN identity.pack_migraciones pm ON pm.empresa_id = e.id
 WHERE e.pack_codigo IS NULL OR pm.estado = 'requiere_revision';
```

Una fila `requiere_revision` detiene la promoción del piloto. Se corrige la
vertical o se crea el pack aprobado antes de relanzar la migración; no se
asigna un pack manualmente sin dejar evidencia en la auditoría.

## Pilotos

Crear un tenant nuevo por pack y validar, en este orden:

1. farmacia/retail: catálogo, stock, lotes cuando corresponda y facturación;
2. belleza: servicio, recurso, disponibilidad, reserva y recordatorio;
3. flota: rutas/vehículos, pickup/onsite, cotización y facturación.

Para cada tenant:

- consultar `GET /api/v2/packs?vertical_id=<vertical>`;
- crear la empresa con `pack_codigo` explícito;
- comprobar `identity.empresas.pack_codigo/pack_version`;
- comprobar que las capacidades activas coinciden con el pack;
- desactivar una capacidad y verificar `CAPACIDAD_NO_ACTIVA`;
- verificar que un tenant distinto no puede leer ni modificar sus datos.

## Rollback

El rollback operativo consiste en apagar la capacidad o el flag del piloto y
conservar la compatibilidad legacy. No se ejecutan `DROP`, `DELETE` ni cambios
destructivos como parte de este runbook. Las filas de auditoría son evidencia de
lo aplicado y se conservan.

## Cierre

El piloto se considera adoptado cuando no hay revisiones pendientes, los tres
flujos pasan sus pruebas, p95 se mantiene dentro de la línea base y el equipo
aprueba la deprecación gradual de `tipo_empresa`/`modalidad_logistica`.

## Automatización de la matriz

La validación de piloto está cubierta en `scripts/integration/` y en el job
`API + BD contracts`. Además de `vertical-matrix`, `capability-enforcement` y
`vertical-operations`, se añaden:

| Suite | Casos | Qué fija |
|---|---:|---|
| `pack-lifecycle.test.mjs` | 5 | Vertical/modelo/pack v1 y v2, herencia, upgrade auditado y rollback de packs inválidos. |
| `vertical-rls.test.mjs` | 4 | Aislamiento de items, capacidades y citas por tenant; admin global. |
| `capability-routes.test.mjs` | 5 | Permite/rechaza stock, agenda, rutas y lotes según el pack. |
| `vertical-pilots.test.mjs` | 4 | Lotes, cita con outbox y vehículo → ruta → parada. |
| `field-rls.test.mjs` | 3 | Aislamiento de las 11 tablas de flota por tenant. |
| `lot-fefo.test.mjs` | 4 | Consumo FEFO, división entre lotes y rollback por stock insuficiente. |

El CI ejecuta `npm run multi-vertical:check` contra la misma BD efímera para
detectar tenants sin pack, packs inactivos y migraciones pendientes.

```bash
TEST_DATABASE_URL=postgres://core_engine_test:test_only@127.0.0.1:5432/core_engine_test \
  npm run test:integration
TEST_DATABASE_URL=postgres://core_engine_test:test_only@127.0.0.1:5432/core_engine_test \
  npm run multi-vertical:check
```

Estas suites usan PostgreSQL real y no dobles de prueba. No sustituyen al E2E
del stack: los consumidores de RabbitMQ y el gateway se siguen validando con
`npm run demo`.

