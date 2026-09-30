# 02 — Reglas Doradas de Nomenclatura

Aplica a: nombres de archivos, clases, variables, tablas y mensajes de commit
en todo el monorepo (`packages/*`).

## Reglas

| ID | Regla |
|---|---|
| R-NC-1 | **Archivos y carpetas (kebab case + notación de punto)**: los archivos de NestJS usan la convención `[nombre].[tipo].ts`, donde el nombre va en `kebab-case` y el tipo es el sufijo NestJS separado por un **punto**. Ejemplos correctos: `usuario.controller.ts`, `auth.guard.ts`, `pedidos-service.module.ts`. El **nombre** (parte antes del punto) usa `kebab-case`. El **sufijo** (parte después del punto) es siempre en inglés y en singular: `controller`, `service`, `module`, `guard`, `handler`, `command`, `event`, `consumer`. Las carpetas deben usar `kebab-case` o minúsculas si son una sola palabra (ej. `pedidos/`, `pagos-recurrentes/`). |
| R-NC-2 | **Sufijos por tipo de archivo**: NestJS: `*.controller.ts`, `*.service.ts`, `*.module.ts`, `*.guard.ts`, `*.decorator.ts`, `*.middleware.ts`. Patrón CQRS: `*.command.ts`, `*.query.ts`, `*.handler.ts`. Eventos y consumidores: `*.event.ts`, `*.consumer.ts`. Pruebas: `*.spec.ts` (unitarias) o `*.e2e-spec.ts` (end-to-end). Los archivos de test de un handler o command específico pueden anidar sufijos: `[nombre].[tipo].spec.ts` (ej: `crear-pedido.handler.spec.ts`). El sufijo `spec` o `e2e-spec` es siempre el terminal. |
| R-NC-3 | **Clases y tipos (PascalCase)**: toda clase, interfaz o tipo de TypeScript debe declararse usando PascalCase. Ejemplo: `class CrearPedidoCommand`, `interface DetalleLiquidacion`. **Nomenclatura de DTOs:** los DTOs deben indicar explícitamente su dirección y propósito con un sufijo: `[Nombre]RequestDto` (payload de entrada del cliente), `[Nombre]ResponseDto` (respuesta hacia el cliente), `[Nombre]InternalDto` (comunicación entre servicios internos). Nunca usar `[Nombre]Dto` genérico que no aclare su rol. |
| R-NC-4 | **Variables y funciones (camelCase)**: todas las variables, instancias y funciones de bloque deben ser en camelCase. Ejemplo: `const orderRepository`, `function calcularDescuento(montoBase)`. |
| R-NC-5 | **Constantes y enum (UPPER_SNAKE_CASE)**: variables globales o enumeraciones de estado de dominio. Ejemplo: `const REINTENTOS_MAXIMOS = 3;`, `enum EstadoPedido { EN_PREPARACION, ENVIADO }`. |
| R-NC-6 | **Idiomas y consistencia**: se debe mantener consistencia. El código (TypeScript) y la arquitectura de alto nivel pueden estar en inglés (`shared/`, `api-gateway/`, `CQRS`), pero las reglas de negocio, endpoints y base de datos de dominio específico pueden usar español si se acordó así (como en `pedidos`, `productos`). Nunca mezclar Spanglish en el mismo nombre (evitar cosas como `getPedidosList()`, usar `obtenerPedidos()` o `getOrders()`). |
| R-NC-7 | **Base de datos (snake_case)**: las tablas y columnas en Postgres deben usar `snake_case` (ej. `id_usuario`, `fecha_creacion`). El ORM (TypeORM) debe encargarse del mapeo hacia camelCase en la entidad TypeScript. |
| R-NC-8 | **Commits (Conventional Commits)**: todo mensaje de commit de Git debe seguir el estándar Conventional Commits (ej. `feat(orders): agregar validacion de stock`, `fix(gateway): corregir expiracion de JWT`). El detalle del formato vive en `R-GC-1` (`../git/02-commits.md`). |
