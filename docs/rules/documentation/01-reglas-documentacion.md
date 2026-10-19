# 01 — Reglas de Documentación

La documentación es el único activo que sobrevive a las migraciones y refactorizaciones. En Core Engine, la documentación se trata como código: tiene convenciones, se revisa en PRs y debe mantenerse sincronizada con la implementación real.

## Reglas

| ID | Regla |
|---|---|
| R-DO-1 | **README.md por paquete:** Cada microservicio o librería dentro de `packages/` DEBE tener un `README.md` que describa: suBounded Context, dependencias exclusivas, modelo de datos resumido (si aplica), y comandos locales específicos para levantarlo. |
| R-DO-2 | **Sincronización del Root README:** El `README.md` de la raíz del proyecto es la única fuente de verdad sobre topología, puertos, variables de entorno comunes y pasos de despliegue. Ningún cambio de puertos, rutas del Gateway o servicios nuevos debe ser mergeado sin actualizar este archivo. |
| R-DO-3 | **Archivos de entorno:** Cualquier variable agregada o eliminada del código DEBE reflejarse en `.env.example` y `.env.production.example`. Estas plantillas sirven como documentación viva de la configuración. |
| R-DO-4 | **El Qué y el Por Qué:** Los comentarios en el código no deben explicar *qué* hace el código (eso lo explica el código mismo), sino *por qué* lo hace. Ej: "Se resta 1 milisegundo para asegurar que el rango de fechas no cruce al día siguiente". Las interfaces públicas y casos de uso DEBEN usar JSDoc para describir parámetros de entrada, salida y errores esperados. |
| R-DO-5 | **Decisiones Arquitectónicas (ADRs):** Cualquier cambio estructural, adición de nuevas herramientas (ej. Stryker, Playwright, RabbitMQ) o desvío de los patrones de Core Engine DEBE registrarse como un ADR en `decisiones.md` o en la carpeta equivalente, justificando el contexto, las opciones y la decisión. |
| R-DO-6 | **Documentación de API:** La fuente de verdad del contrato de la API es la especificación OpenAPI (Swagger). Los cambios en request/response DTOs deben ir acompañados de los decoradores `@ApiProperty` correspondientes. |
| R-DO-7 | **Evidencia en Meta:** Todo hallazgo de auditoría o checklist DEBE ir en `docs/rules/_meta/`. Prohibido incrustar snapshots transitorios o resultados de CI en los READMEs estructurales. |
| R-DO-8 | **Taxonomía y superficies del contrato:** toda ruta pública expuesta por el gateway DEBE empezar por `/api/v1/`; toda ruta interna servicio→servicio DEBE empezar por `/internal/`; el contrato público (`docs/api/core-engine.openapi.json`) no puede contener rutas `/internal/` y el interno no puede contener rutas `/api/v1/`; todo controlador `internal*` DEBE usar `ClaveInternaGuard` (R-DS-5). La verificación ejecutable vive en `scripts/rules/contract/openapi.test.mjs`. |

## Verificación en PRs

- El revisor del PR debe exigir actualizaciones al `README.md` de un microservicio si este incorpora lógica de negocio nueva y sustancial.
- En caso de añadir/quitar variables de entorno, el PR será rechazado si no se actualizan `.env.example` y `.env.production.example`.
