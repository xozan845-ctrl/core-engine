# 01 — Reglas Doradas de Estructura de Microservicios

Aplica a: layout de `packages/shared` y `packages/*-service`, separación de
capas, DI, DTOs y ciclo de vida del proceso.

## Reglas

| ID | Regla |
|---|---|
| R-MS-1 | **Ubicación de código y responsabilidades**: `packages/shared` — uso estricto para lógica transversal (logging, Money, decorators, base de database, events, interfaces globales). `packages/[dominio]-service/src` — cada microservicio anida el contexto en una carpeta descriptiva (ej. `pedidos/`, `productos/`). |
| R-MS-2 | **Separación interna de módulos (clean architecture)**: `controllers/` = punto de entrada HTTP, extraen el request y llaman al service o mediator, sin lógica de negocio (**excepción válida:** los decoradores de seguridad transversal `@Roles()`, `@UseGuards()` sí pertenecen aquí — son infraestructura; la validación de *qué* puede hacer el usuario con los datos pertenece al `service/`); `services/` = lógica de negocio, reglas de dominio y orquestación síncrona **dentro del mismo servicio** (**no permitido:** llamar síncronamente vía HTTP a otro microservicio para ejecutar una mutación crítica — eso viola `R-AR-2` y debe ir por eventos); `repositories/` = únicos responsables de comunicarse con la base de datos (Postgres), ocultan las consultas SQL/TypeORM; `models/` = entidades e interfaces del dominio interno; `commands/` y `queries/` (donde aplica CQRS) = separan escrituras de lecturas; `handlers/` = ejecutan comandos y consultas; `events/` = esquemas locales de eventos y consumidores de RabbitMQ (`.consumer.ts`). |
| R-MS-3 | **Inyección de dependencias (DI)**: todos los componentes (servicios, repositorios) deben resolverse mediante el framework (NestJS `@Injectable()`). Prohibido usar el patrón singleton manual o llamadas globales estáticas que dificulten las pruebas unitarias. |
| R-MS-4 | **Acoplamiento débil de modelos DTO**: los DTO de respuesta y solicitud nunca deben compartir referencia estricta con las entidades de la base de datos (modelos/TypeORM). Se debe realizar siempre el mapeo para evitar filtraciones de esquema de DB al usuario (ej: contraseña o `id` interno). |
| R-MS-5 | **Validación de entorno (environment variables)**: todo microservicio debe validar (usando Joi, Zod o la infraestructura de NestJS) la presencia y tipo de sus variables de entorno al iniciar. Si falta una variable crítica (como credenciales de DB o RabbitMQ), el servicio DEBE fallar rápido (fail-fast) y no arrancar. |
| R-MS-6 | **Graceful shutdown (apagado elegante)**: todo servicio debe interceptar señales (SIGINT, SIGTERM) y cerrar ordenadamente sus conexiones (Postgres, RabbitMQ) y no aceptar nuevas peticiones antes de morir, evitando transacciones corrompidas durante despliegues (Kubernetes/Docker). |
