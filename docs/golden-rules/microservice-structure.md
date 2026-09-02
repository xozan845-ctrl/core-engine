# Reglas Doradas de Estructura de Microservicios (Microservice Structure)

1. **Ubicación de Código y Responsabilidades**:
   - `packages/shared`: Uso estricto para lógica transversal (Logging, Money, Decorators, Database Base, Events, Interfaces globales).
   - `packages/[dominio]-service/src`: Cada microservicio anida el contexto en una carpeta descriptiva (e.g. `pedidos/`, `productos/`).
2. **Separación Interna de Módulos (Clean Architecture)**:
   - `controllers/`: Punto de entrada HTTP. Contienen decoradores de rutas, extraen el request y llaman al Service o Mediator. No llevan lógica de negocio. **Excepción válida:** los decoradores de seguridad transversal (`@Roles()`, `@UseGuards()`) sí pertenecen aqui — son infraestructura, no lógica de dominio. La validación de *qué* puede hacer el usuario con los datos (reglas de negocio) pertenece al `service/`.
   - `services/`: Contienen lógica de negocio, reglas de dominio y orquestación síncrona **dentro del mismo servicio**. Ejemplos permitidos: llamar a otro método interno, calcular totales, validar reglas de negocio. **No permitido:** llamar síncronamente via HTTP a otro microservicio para ejecutar una mutación crítica (eso viola `architecture.md` regla 2 y debe ir por eventos).
   - `repositories/`: Únicos responsables de comunicarse con la base de datos (Postgres). Ocultan las consultas SQL/TypeORM.
   - `models/`: Definición de las entidades e interfaces del dominio interno.
   - `commands/` y `queries/` (donde aplica CQRS): Separan las intenciones de escritura de las peticiones de solo lectura.
   - `handlers/`: Escuchan y ejecutan Comandos (Commands) y Consultas (Queries).
   - `events/`: Esquemas locales de eventos y consumidores de RabbitMQ (`.consumer.ts`).
3. **Inyección de Dependencias (DI)**: Todos los componentes (servicios, repositorios) deben resolverse mediante el framework (NestJS `@Injectable()`). Prohibido usar el patrón Singleton manualmente o llamadas globales estáticas que dificulten las pruebas unitarias.
4. **Acoplamiento Débil de Modelos DTO**: Los DTO de respuesta y solicitud nunca deben compartir referencia estricta con las Entidades de la Base de datos (Modelos/TypeORM). Se debe realizar siempre el mapeo para evitar filtraciones de esquema de DB al usuario (Ej: contraseña o `id` interno).
5. **Validación de Entorno (Environment Variables)**: Todo microservicio debe validar (usando Joi, Zod o la infraestructura de NestJS) la presencia y tipo de sus variables de entorno al iniciar. Si falta una variable crítica (como credenciales de DB o RabbitMQ), el servicio DEBE fallar rápido (Fail-fast) y no arrancar.
6. **Graceful Shutdown (Apagado Elegante)**: Todo servicio debe interceptar señales (SIGINT, SIGTERM) y cerrar ordenadamente sus conexiones (Postgres, RabbitMQ) y no aceptar nuevas peticiones antes de morir, evitando transacciones corrompidas durante despliegues (Kubernetes/Docker).
