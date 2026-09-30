# 02 — Reglas Doradas de Seguridad de Datos

Aplica a: secretos, PII, autorización, validación de entradas, tráfico
interno, rate limiting y dependencias en todos los servicios.

## Reglas

| ID | Regla |
|---|---|
| R-DS-1 | **Gestión de secretos**: ningún secreto (contraseñas, claves JWT, API keys, credenciales de DB/RabbitMQ) debe estar codificado (hardcoded) en el repositorio. Todo debe provenir de variables de entorno (`.env` localmente o gestores de secretos en producción). |
| R-DS-2 | **Protección de datos personales (PII)**: las contraseñas en la base de datos deben estar cifradas o hasheadas estrictamente con bcrypt (salteado) o algoritmos superiores (Argon2). No se guarda información de tarjetas de crédito en texto plano (se usarán tokens de pasarelas PCI-DSS). |
| R-DS-3 | **Roles y privilegios mínimos** (principio de mínimo privilegio): toda acción en el sistema debe verificar el rol del actor que la invoca usando guardias. No confiar en el frontend para ocultar información confidencial: siempre omitirla y validarla en el backend. |
| R-DS-4 | **Validación de entradas estricta (OWASP)**: cada payload entrante en la API DEBE estar respaldado por un DTO (Data Transfer Object) válido. Utilizar `class-validator` y `class-transformer` de NestJS con configuración estricta (`whitelist: true`, `forbidNonWhitelisted: true`) para limpiar la entrada y prevenir inyección masiva (mass assignment). |
| R-DS-5 | **Comunicaciones seguras inter-servicio**: cualquier endpoint etiquetado como `internal/` (exclusivo para backend) DEBE requerir el header `x-internal-key` con un valor coincidente con `INTERNAL_API_KEY`. El Gateway JAMÁS debe rutear tráfico externo hacia rutas `/internal/`. **Nota de flujo:** el Gateway y los microservicios, al invocar intencionalmente estos endpoints internos, **agregan** este header. Por otro lado, como defensa en profundidad (defense in depth) ante malas configuraciones de ruteo, el Gateway **elimina (scrub)** cualquier header `x-internal-key` que provenga de peticiones públicas del exterior (ver `R-GW-3`, `../gateway/01-auth.md`). |
| R-DS-6 | **Limitación de tasa estricta (rate limiting)**: puntos de entrada sensibles (login, reseteo de contraseña, creación de tiendas/órdenes) deben poseer restricciones de rate limiting por IP/usuario para mitigar fuerza bruta o ataques DDoS de capa de aplicación. **Responsabilidad:** esta limitación DEBE configurarse exclusivamente en el **API Gateway** (usando `RateLimitMiddleware`), ya que los microservicios internos no gestionan tráfico externo directo y asumen que las peticiones ya fueron limitadas en el perímetro. |
| R-DS-7 | **Escaneo de dependencias**: es obligatorio mantener saneadas las dependencias. Se debe auditar (`npm audit`) en integración continua y prohibir el despliegue con vulnerabilidades críticas/altas. |
