# 02 — Reglas de Tests de Integración (API + BD)

Aplica a: pruebas que levantan la app NestJS real (o módulos reales con la capa de
datos real: SQL sobre Postgres) y ejecutan requests HTTP contra una **BD de test
real** (Postgres de test, no mock).

> Los mocks en los tests de controller prueban los mocks. Solo una prueba con BD real
> valida consultas SQL, filtros de fecha, paginación y transacciones.

## Alcance mínimo por endpoint

| ID | Regla |
|---|---|
| R-I-1 | Todo endpoint `GET/POST/PUT/DELETE` del API tiene **al menos un test de integración** happy-path con BD real (seed mínimo insertado y verificado en la respuesta). |
| R-I-2 | Endpoints con **filtros o paginación** deben tener tests que: (a) insertan datos con dates/paginación conocidos, (b) filtran, (c) asertan el conjunto **exacto** devuelto (conteo + IDs), (d) verifican que la paginación conserva los filtros. |
| R-I-3 | Fechas: todo filtro `dateFrom/dateTo` se prueba con datas que crucen la medianoche local (ej. registros a las `23:30` y `00:30` locales) asertando inclusión/exclusión correcta. La asertación es contra el **día local configurado (`TZ`)**, no contra UTC. |

## Autenticación y autorización

| ID | Regla |
|---|---|
| R-I-4 | Cada endpoint protegido tiene test: sin token → `401`; con token de rol insuficiente → `403`. |
| R-I-5 | El login completo (credenciales válidas → token → uso del token en otro endpoint) se prueba contra BD real. |

## Validación y errores

| ID | Regla |
|---|---|
| R-I-6 | Payload inválido → `400` con shape de error conocido (campo `message`/`errors`). |
| R-I-7 | Recurso inexistente → `404`; recurso duplicado/conflicto → `409`. |
| R-I-8 | Los casos de error deben **no dejar datos residuales** en la BD (rollback verificado en transacciones de escritura fallida). |

## Datos y aislamiento

| ID | Regla |
|---|---|
| R-I-9 | Cada test crea/limpia sus datos (transacción rollback o seed/truncate por test). Prohibido depend del estado dejado por otro test. |
| R-I-10 | El seed de test incluye datos **boundary** (registros en el límite del rango de fecha, recursos sin relación asociada — orden sin líneas, vendedor sin tienda — estados inactivos/cancelados). |
| R-I-11 | Las queries SQL crudas usadas por los repositorios/servicios se prueban por lo menos una vez con BD real (detecta errores de mapeo de columnas/tipos). |

## Ejecución

| ID | Regla |
|---|---|
| R-I-12 | Ejecutable con un comando (`npm run test:integration`), corriendo en CI, con BD de test efímera (docker o schema separado). |
| R-I-13 | No requiere infraestructura externa real (broker RabbitMQ, servicios vecinos, Clock): esos bordes se doblan con dobles reales del lado de la infraestructura, pero **el resto del stack es real**. |
