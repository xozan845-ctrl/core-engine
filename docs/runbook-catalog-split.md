# Runbook — separación de `catalog-service`

Complementa [ADR-0018](./adr/0018-catalog-split-item-pricing-inventory.md). La
migración se ejecuta en la rama `feat/catalog-split`, con PRs apilados. **Nada
se promueve a `develop`/`staging` hasta que toda la cadena esté verde y el
smoke de `staging` haya pasado.**

## Regla de oro

- Cada PR deja el repositorio arrancable y la suite de su ámbito en verde.
- No hay doble escritura permanente ni transacciones distribuidas.
- El contrato visible (rutas, envelope, errores y eventos) no cambia durante la
  migración.
- `item_id` es el único identificador compartido entre los tres servicios.
- Cada servicio es dueño exclusivo de su esquema; sin FK entre esquemas.

## Seguimiento de PRs

| PR | Rama | Estado | Evidencia |
|---|---|---|---|
| 1 | `feat/catalog-split` (ADR) | Completo | `rules:check` 83/83 |
| 2 | `feat/inventory-scaffold` | Completo | `cargo fmt/clippy/test/llvm-cov` (83,30/75,60) |
| 3a | `feat/inventory-core` | Completo | PostgreSQL + HTTP (81,15/76,44) |
| 3b | `feat/inventory-locations` | Completo | PostgreSQL + HTTP (85,75/81,91) |
| 4 | `feat/inventory-saga` | Pendiente | Saga + concurrencia + Outbox |
| 5 | `feat/inventory-cutover` | Pendiente | Integración + contrato |
| 6 | `feat/pricing-scaffold-domain` | Pendiente | Suite + contrato |
| 7 | `feat/pricing-cutover` | Pendiente | Integración + contrato |
| 8 | `feat/item-service` | Pendiente | Build + OpenAPI |
| 9 | `feat/catalog-retire` | Pendiente | `SERVICES`, compose, CI, docs |
| 10 | `feat/catalog-split-stage-smoke` | Pendiente | Smoke de staging |

## Matriz de pruebas por PR

| Capa | Comando | Cuándo |
|---|---|---|
| Formato | `cargo fmt --check` | Cada PR Rust |
| Lint | `cargo clippy --locked --all-targets -- -D warnings` | Cada PR Rust |
| Unitarias | `cargo test` | Cada PR Rust |
| Cobertura | `cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' --fail-under-lines <gate> --fail-under-functions <gate>` | Cada PR Rust |
| PostgreSQL | `TEST_DATABASE_URL=... cargo test` | PRs de dominio y corte |
| Contrato | `npm run swagger:export && git diff --exit-code docs/openapi docs/api` | PRs de rutas |
| Reglas | `npm run rules:check` | Cada PR |
| Baselines | `npm run baseline:guard` | Cada PR de corte |
| Migraciones | `npm run db:migrations` | PR de esquema |
| RLS | `npm run db:rls` | PR de esquema |
| Build | `npm run build` | PR de corte |
| E2E local | `npm run demo` | PR de corte |
| Smoke staging | `npm run smoke:remote` (o el runner de staging) | PR 10 / promoción |

## Base de datos

- El esquema nuevo se crea en `infra/db/init/75_inventory_schema.sql` (o el
  número siguiente disponible) y se registra con
  `npm run db:migrations -- --update`.
- Las tablas movidas se copian, se validan y luego se retiran del esquema
  `catalog` en un PR posterior; no se renombra una tabla en el mismo PR que se
  cambia el código.
- Las vistas de compatibilidad se mantienen solo mientras haya consumidores
  legacy; cada una debe tener fecha de retirada.
- RLS: toda tabla nueva entra con política por tenant y se actualiza el
  baseline solo cuando corresponda.

## Corte de tráfico

1. Desplegar el servicio nuevo con el contrato interno y sus pruebas.
2. Cambiar el gateway por prefijo y los clientes internos.
3. Verificar `health`, `ready`, `metrics` y una lectura/escritura de cada
   dominio.
4. Retirar la ruta del servicio viejo y apagar la imagen anterior.
5. Si falla, revertir el flag de gateway y el compose; los datos nuevos deben
   ser compatibles hacia atrás.

## Rollback

- El rollback de código es el flag de gateway y la imagen anterior.
- El rollback de datos solo se hace con una migración inversa probada; nunca
  con `DELETE` manual.
- Si el stock queda inconsistente, se congela la escritura de inventario y se
  reejecuta la proyección desde el Outbox/eventos, no se corrige a mano.

## Criterio de salida de la migración

- Los tres servicios pasan `fmt`, `clippy`, `test` y cobertura.
- `orders`, `stores` y `commissions` siguen funcionando contra los nuevos
  endpoints internos.
- La saga `order.created → stock → payment → commission → asiento` pasa en
  integración y en el smoke de staging.
- `catalog-service` ya no existe en `SERVICES`, compose, CI ni docs.
- El PR de promoción incluye la evidencia de todos los gates y del smoke.
