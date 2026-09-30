# 01 — Reglas Doradas de API Gateway y Autenticación

Aplica a: `packages/api-gateway` — único punto de entrada, perímetro,
validación JWT y contrato de errores hacia los clientes externos.

## Reglas

| ID | Regla |
|---|---|
| R-GW-1 | **Único punto de entrada (single entry point)**: todo el tráfico desde clientes externos (web, app móvil) al cluster debe obligatoriamente ingresar a través del **API Gateway**. Las llamadas directas a los puertos de los microservicios están estrictamente prohibidas y bloqueadas a nivel de red interna. |
| R-GW-2 | **Descarga perimetral (edge offloading)**: el Gateway es responsable exclusivo del CORS, rate limiting (throttling) y TLS. Los microservicios internos asumirán tráfico de confianza libre de TLS y delegarán el peso criptográfico al Gateway. |
| R-GW-3 | **Validación JWT y prevención de spoofing**: el Gateway valida y procesa la integridad del JWT y su firma (HS256). Inyecta la información decodificada del usuario (`uid`, `rol`, `email`) en los headers de la petición hacia el microservicio final. El microservicio final confía ciegamente en esos headers. Por lo tanto, el Gateway **DEBE limpiar (scrub)** cualquier cabecera interna (`x-user-id`, `x-internal-key`, etc.) enviada maliciosamente por el cliente externo ANTES de sobrescribirlas o reenviar la petición, previniendo el *header spoofing*. |
| R-GW-4 | **Expiraciones estratégicas (TTL)**: los tiempos de expiración de tokens DEBEN configurarse mediante variables de entorno (`JWT_ACCESS_TTL`, `JWT_REFRESH_TTL`), no valores hardcodeados en código. Los valores de referencia para **producción** son: access token de vida corta (**15 minutos**) y refresh token de vida larga (**7 días**). Staging/desarrollo puede usar valores distintos, pero nunca se aceptará un access token mayor a 24h en producción. Los refresh tokens pueden revocarse cerrando sesión. |
| R-GW-5 | **No almacenar JWT en LocalStorage indefinidamente**: preferir cookies HttpOnly para tokens, pero si se usa cabecera `Authorization` (Bearer), limpiarlos del cliente al cerrar la aplicación. |
| R-GW-6 | **Errores estandarizados (RFC 7807)**: toda respuesta de error hacia el cliente debe cumplir con un formato estructurado (Problem Details for HTTP APIs), incluyendo códigos HTTP precisos (`status`), un mensaje claro (`title`) y detalles técnicos solo si se está en desarrollo. No arrojar trazas del stack (stacktrace) a los clientes de producción. |
| R-GW-7 | **Versionamiento de API**: toda ruta expuesta debe incluir un prefijo de versión (`/api/v1/`). Cualquier cambio rompente (breaking change) en contratos requerirá una `/v2/`, garantizando retrocompatibilidad para clientes de aplicaciones móviles antiguas. |
