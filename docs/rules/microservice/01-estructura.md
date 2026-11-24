# 01 — Reglas Doradas de Estructura de Microservicios

Aplica a: layout de `packages/*`, separación de capas dentro de cada crate Rust,
inyección de dependencias, DTOs y ciclo de vida del proceso.

> **Stack actual:** todos los servicios son crates Rust (`axum` + `tokio` +
> `sqlx` + `lapin`). La antigua librería `packages/shared` en TypeScript se
> retiró (ADR-0013..0017); no existe un paquete compartido de runtime. La
> autonomía del bounded context (R-AR-5) se mantiene aunque eso duplique
> infraestructura entre servicios.

## Reglas

| ID | Regla |
|---|---|
| R-MS-1 | **Ubicación de código y responsabilidades**: cada servicio vive en `packages/<servicio>/` como crate autónomo con su `Cargo.toml`, `Dockerfile`, `README.md`, `src/` y `tests/`. `src/main.rs` es solo el bootstrap (config, pool, router, señales); `src/lib.rs` declara los módulos y permite que las suites de `tests/` ejerzan el crate como biblioteca. El dominio de cada servicio se anida en un módulo descriptivo en español (`dominio/`, `items/`, `pedidos/`, `scheduling/`, `field/`…), nunca en un único archivo plano cuando cubre más de un agregado. Prohibido crear un módulo o crate compartido de infraestructura sin ADR que lo justifique: la duplicación de `config/db/errors/outbox/rabbit` es el precio explícito de la autonomía (R-AR-5). |
| R-MS-2 | **Separación interna por capas**: (a) `http/` = router, handlers, envelope `{data,meta}`, validación de entrada, autenticación y métricas; **no** contiene SQL ni reglas de negocio. (b) `<dominio>/` = reglas de negocio, casos de uso y acceso a datos con **SQL explícito** vía `sqlx`; se organiza **por agregado o caso de uso**, no por tipo de archivo (nada de `controllers/`, `services/`, `repositories/` genéricos). (c) Infraestructura del crate: `config.rs` (entorno fail-fast), `db.rs` (pool, RLS, helpers de filas), `errors.rs` (errores estructurados), `state.rs` (`AppState`), `outbox.rs`, `rabbit.rs`, `logs.rs`, `metrics.rs`. (d) Los consumidores de eventos viven junto a su dominio o en un módulo `consumer.rs` del mismo crate. **Prohibido** llamar por HTTP síncrono a otro servicio para una mutación crítica: eso va por eventos (R-AR-2). |
| R-MS-3 | **Inyección de dependencias explícita**: las dependencias se resuelven por `AppState` con `Arc` (pool `Db`, `Outbox`, clientes internos, puertos por trait) y se pasan al constructor del dominio (`Productos::nueva(db, outbox, items)`). Prohibido el singleton global mutable o el estado estático compartido que impida tests aislados. Los traits de puerto (`AgendaPort`, clientes internos) permiten dobles de prueba sin framework de DI. |
| R-MS-4 | **DTO ≠ persistencia**: los cuerpos HTTP se validan y normalizan en `http/` (o en el constructor del dominio) y se mapean a tipos de dominio o `serde_json::Value` controlado; nunca se devuelve una fila cruda de Postgres ni se exponen columnas internas (password, `empresa_id` ajeno, flags internos). La serialización de salida es explícita (`serializar`, DTOs `*Response`), con montos en centavos y strings de dos decimales (ADR-01). |
| R-MS-5 | **Validación de entorno fail-fast**: cada servicio valida al arrancar las variables críticas que usa (`DATABASE_URL`/`POSTGRES_*`, `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`, `OUTBOX_TABLA`, URLs internas) en `config.rs`. Si falta o es inválida una variable crítica, el proceso **no arranca** y registra el error en JSON. Prohibido leer `env::var` disperso en el dominio: la configuración se resuelve una vez en `config.rs`. |
| R-MS-6 | **Graceful shutdown**: `main.rs` intercepta `SIGINT`/`SIGTERM` con `tokio::signal`, deja de aceptar peticiones (`axum::serve(...).with_graceful_shutdown(...)`), espera el cierre ordenado del pool y del consumidor/outbox, y termina. Nada de abortar a mitad de una transacción durante un despliegue. |
| R-MS-7 | **Mínima responsabilidad por archivo**: ningún archivo Rust de `src/` supera las **800 líneas de producción** (sin `#[cfg(test)]`). Además, un archivo de dominio con SQL no puede exponer más de **20 funciones públicas** (los módulos `http/` quedan fuera de este presupuesto: cada handler es una ruta); superar cualquiera de los dos presupuestos obliga a dividir en una carpeta con `mod.rs` (contrato y re-exports) + archivos por responsabilidad (`consultas.rs`, `escritura.rs`, `variantes.rs`, `inventario.rs`, `validacion.rs`…). Cualquier archivo que mezcle **más de un agregado o caso de uso** se divide aunque cumpla los presupuestos; el objetivo de referencia son **400 líneas por archivo de dominio**. Prohibido resolver la división con `include!`. Los `impl` pueden repartirse en submodules (`impl super::Tipo { … }`); los métodos y helpers compartidos entre submodules se declaran `pub(super)`. Los tests unitarios pueden vivir en un `tests.rs` del módulo. La deuda anterior a esta regla se registra en `scripts/config/module-size-baseline.json` y solo puede reducirse. |
| R-MS-8 | **README por paquete**: cada `packages/<servicio>/README.md` describe su bounded context, sus capacidades (`CapabilityGuard`), las tablas/esquema que posee, sus comandos `cargo` y sus variables de entorno propias. Un cambio de contrato o de esquema del servicio actualiza su README en el mismo PR (R-DO-1/R-DO-2). |

> **Enmienda 2026-11-24 (R-MS-7):** el límite duro baja de 1.500 a **800
> líneas de producción** y se añade el presupuesto de **20 funciones
> públicas** en dominio con SQL. El check vive en
> `scripts/rules/microservices/tamano-modulos.test.mjs`; la deuda existente está
> en `scripts/config/module-size-baseline.json` y `baseline:guard` impide
> añadirla o relajar los presupuestos.
