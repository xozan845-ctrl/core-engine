# 02 — Reglas Doradas de Nomenclatura

Aplica a: nombres de archivos, módulos, tipos, funciones, tablas y mensajes de
commit en todo el monorepo (`packages/*`, crates Rust).

> **Stack actual:** Rust (`snake_case` en archivos y funciones). No hay ORM ni
> decoradores de framework que impongan sufijos: la claridad la da el módulo,
> no el nombre del archivo.

## Reglas

| ID | Regla |
|---|---|
| R-NC-1 | **Archivos y carpetas (`snake_case`)**: los archivos Rust usan `snake_case` y extensión `.rs` (`api_keys.rs`, `configuracion_sitio.rs`, `order_state.rs`). Las carpetas de módulo usan `snake_case` en minúsculas (`dominio/`, `items/`, `scheduling/`, `http/`) y su punto de entrada es `mod.rs`. Los archivos especiales del crate son `main.rs` (bootstrap) y `lib.rs` (superficie de biblioteca). Prohibido `kebab-case` y la notación con puntos del stack NestJS. |
| R-NC-2 | **Organización por módulo, no por sufijo**: un archivo se nombra por el **agregado o caso de uso** que contiene (`productos`, `inventario`, `pedidos`, `contratos`, `suscripciones`, `liquidaciones`), no por el tipo de pieza (`*.controller.rs`, `*.service.rs`, `*.repository.rs`). Cuando un módulo se divide (R-MS-7), los archivos hijos se nombran por responsabilidad (`consultas.rs`, `escritura.rs`, `variantes.rs`, `validacion.rs`). Las pruebas unitarias viven en `mod tests` del archivo que prueban; las de integración en `tests/<dominio>_<pg\|http>.rs`. |
| R-NC-3 | **Tipos (`PascalCase`)**: structs, enums, traits y type aliases en `PascalCase` (`AppState`, `ApiError`, `PedidosConsumer`, `AgendaPort`). Los DTO de entrada se nombran `[Accion]Request` (`CrearAsientoRequest`, `CrearPackRequest`); las respuestas se serializan con funciones explícitas (`serializar`, `fila_*`) hacia el envelope `{data,meta}`. Prohibido un `[Nombre]Dto` genérico que no aclare su rol. |
| R-NC-4 | **Funciones, variables y campos (`snake_case`)**: `calcular_descuento`, `validar_stock`, `empresa_id`. Los booleanos llevan prefijo semántico (`es_uuid`, `tiene_stock`, `debe_reintentar`). Prohibido `camelCase` en identificadores Rust. |
| R-NC-5 | **Constantes y variantes (`UPPER_SNAKE_CASE` / `PascalCase`)**: las constantes y variables de entorno en `UPPER_SNAKE_CASE` (`REINTENTOS_MAXIMOS`, `MARGEN_MAXIMO`, `OUTBOX_TABLA`); las variantes de enum en `PascalCase` (`EstadoPedido::EnPreparacion`). Los valores persistidos en BD van en `snake_case` minúsculas. |
| R-NC-6 | **Idiomas y consistencia**: el dominio se escribe en español (`pedidos`, `productos`, `liquidaciones`) y la infraestructura/crates externos conservan su nombre en inglés (`axum`, `sqlx`, `lapin`, `AppState`, `Outbox`). Nunca mezclar Spanglish en el mismo identificador (evitar `getPedidos`, usar `obtener_pedidos`). Los mensajes de error de API van en español y con `codigo` en `UPPER_SNAKE_CASE`. |
| R-NC-7 | **Base de datos (`snake_case`)**: tablas y columnas en `snake_case` (`empresa_id`, `fecha_creacion`). No hay ORM que remapee nombres: el SQL explícito de `sqlx` selecciona columnas y las mapea a structs o `serde_json::Value` en el repositorio; prohibido exponer el nombre de columna crudo en el contrato público (R-MS-4). Los esquemas se nombran por dominio (`identity`, `catalog`, `orders`…). |
| R-NC-8 | **Commits (Conventional Commits)**: todo mensaje de commit de Git debe seguir el estándar Conventional Commits (ej. `feat(orders): agregar validacion de stock`, `fix(gateway): corregir expiracion de JWT`). El detalle del formato vive en `R-GC-1` (`../git/02-commits.md`). |
