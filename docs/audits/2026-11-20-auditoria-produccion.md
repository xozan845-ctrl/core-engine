# Auditoría de salida a producción — Core Engine

- **Fecha:** 2026-11-20
- **Base auditada:** `develop` @ `82c90a65` (árbol limpio, sin cambios de código).
- **Alcance:** los 16 crates Rust, 149 scripts SQL, composes de dev/stage/prod,
  workflows de CI/CD, scripts de gobernanza, observabilidad, backups, contratos
  OpenAPI y documentación operativa.
- **Método:** lectura estática exhaustiva + ejecución de los gates de
  gobernanza disponibles en local + escaneo de dependencias Rust contra OSV.
  **No** se levantó el stack completo, **no** se ejecutó carga real y **no** se
  hizo pentest nuevo; el informe de staging de 2026-10-24 se usa solo como
  antecedente.
- **Regla de este documento:** cada afirmación lleva archivo:línea o comando
  reproducible. Lo que no se pudo verificar se declara como no verificado.

---

## 1. Veredicto

> **Core Engine NO está listo para producción general con clientes reales,
> dinero real y datos personales.**
>
> **Sí está listo para un piloto controlado** (1–3 tenants, tráfico bajo,
> operación asistida, sin promesas de disponibilidad) **solo si se cierran antes
> los P0 de las secciones 3.0–3.9, empezando por C1/C2/C3 y A1.**

La base de ingeniería es muy superior a la media de un MVP: bounded contexts
claros, dinero en centavos, outbox transaccional, idempotencia, RLS diseñado en el
núcleo, pruebas de integración contra PostgreSQL real y una cadena release →
promote → deploy bien pensada. El problema no es la arquitectura: es que **la
capa de operación, seguridad de perímetro, recuperación y cumplimiento todavía
es de laboratorio**, el aislamiento multi-tenant no está efectivo en la
configuración documentada, hay tres vías críticas de fuga/compromiso, y algunos
gates de calidad dan falso verde.

### Scorecard (0–5)

| Dimensión | Nota | Diagnóstico |
|---|---:|---|
| Arquitectura y dominio | **4,0** | Sólida y coherente; deuda de acoplamiento por esquema asumida. |
| Seguridad de aplicación | **1,5** | Buenas primitivas de auth, pero hay 3 críticos verificados de aislamiento/exfiltración y el perímetro es débil. |
| Aislamiento multi-tenant | **1,5** | RLS no se aplica en la configuración de producción documentada; registro público permite fijar tenant ajeno. |
| Datos y migraciones | **1,5** | Esquema correcto; sin runner/ledger, RLS no se aplica en Docker y el restore drill falla. |
| Fiabilidad y DR | **1,5** | Un solo nodo, RPO real indefinido, restore roto y sin backup de MinIO. |
| Observabilidad | **2,0** | Métricas y logs JSON, pero alertas que no se entregan y reglas de Node en stack Rust. |
| CI/CD y entrega | **2,5** | Pipeline serio; solo se construye 1 de 16 imágenes en CI, stage sin gate, deploy “prod” no real. |
| Operación y runbooks | **2,0** | Runbooks buenos; backup/restore y migraciones manuales, sin on-call ni SLO y `down -v` en prod. |
| Producto/funcional | **2,0** | Retail de producto funciona; servicios/reservas no se venden, 78 operaciones públicas sin destino, pagos/fiscal simulados y comisiones fuera de prod. |
| Cumplimiento/legal | **2,0** | Fiscal electrónico simulado; sin política de privacidad/DPA ni retención verificada end-to-end. |
| Documentación | **2,5** | Muy abundante, pero desactualizada y con contradicciones que erosionan la confianza. |

**Promedio ponderado: ≈ 1,9 / 5.** Traducción: *piloto sí, producción general no.*

---

## 2. Qué se ejecutó y qué se encontró

| Comando | Resultado real |
|---|---|
| `npm run security:secrets` | ✅ 0 secretos de alta confianza en 1.028 archivos versionados. |
| `npm run rules:check` | ✅ 83/83 tests, 22 suites — **pero solo 51/241 IDs de regla tienen check ejecutable**. |
| `npm run test:engine` | ✅ 21/21. |
| `npm run test:smoke` | ✅ 65/65. |
| `npm run baseline:guard` | ✅ Sin retroceso respecto a `origin/main`. |
| `npm run env:check` | ✅ Compose y plantillas alineados. |
| `npm run db:migrations` | ✅ 149 migraciones íntegras **en el manifiesto**, no aplicadas a ninguna BD. |
| `npm run db:rls` | ✅ 43 tablas en el baseline de deuda RLS. |
| `npm run test:integration` (PostGIS 16) | ✅ 7 archivos / **11 tests** (la documentación dice 20/67). |
| `cargo llvm-cov` (analytics) | ✅ 67,74 % líneas / 58,03 % funciones (umbral 50/45). |
| `cargo llvm-cov` (api-gateway) | ✅ 85,03 % líneas / 77,00 % funciones (umbral 78/73). |
| `docker compose config` sin `.env` | ❌ Falla: confirma que el deploy “prod” no arranca en un runner limpio. |
| Init completo en PostgreSQL 16.4/PostGIS | ✅ 149 scripts aplican; **181 tablas, 85 con RLS, 0 `FORCE`**. |
| Aplicar `99_rls.sql` sobre ese esquema | ❌ **44 errores**; quedan 44 tablas sin RLS. |
| Consulta cross-tenant como `core_engine_app` | ❌ `analytics.hechos_venta` y `finance.configuracion_fiscal` devuelven filas de 2 empresas. |
| `restore-verify.sh` en clúster limpio | ❌ Falla: `role "authenticated"` y `role "core_engine_app"` no existen. |
| Outbox de scheduling sin claims | ❌ 0 pendientes visibles: no drena. |
| `npm run multi-vertical:check` | ⚠️ No ejecutable sin `DATABASE_URL`/`TEST_DATABASE_URL`. |
| `npm run promotion:check` | ⚠️ Falla en `develop` (esperado: la rama no nace de `origin/staging`). |
| `npm audit --audit-level=high` | ✅ 0 vulnerabilidades (solo quedan 3 devDependencies npm). |
| Escaneo OSV de 494 crates únicos | ❌ **26 avisos** en 11 paquetes (detalle en 4.1). |
| `cargo test --locked` (api-gateway) | ✅ 34/34 en 0,17 s. |
| `cargo test --locked --lib` (identity-service) | ✅ 135/135 en 1,08 s. |
| `cargo audit` / `cargo deny` | ❌ No instalados y **no existen en CI**. |
| `npm test` / `npm run build` / `npm run coverage:compare` | ❌ **Ya no existen** en `package.json`; la documentación que los cita está obsoleta. |

Pruebas existentes: **≈1.121 tests unitarios inline + 244 tests de integración**
en 16 crates, 0 `#[ignore]`. Los tests de integración **se omiten silenciosamente**
si falta `TEST_DATABASE_URL` (`packages/orders-service/tests/pedidos_pg.rs:49`).
En CI la variable está definida, así que corren; en local pueden dar falsa
confianza.

---

## 3. Bloqueadores P0 (impiden un go-live con clientes reales)

### 3.0 Críticos de seguridad verificados (prioridad absoluta)

Estos cuatro hallazgos se confirmaron leyendo el código y, en varios casos,
permiten comprometer **todos los tenants**. Van antes que cualquier otra cosa.

#### C1. El registro público permite fijar `tenant_id` ajeno

- `REGISTRO` no declara `tenant_id`/`personal_id`
  (`packages/identity-service/src/http/auth.rs:15-20`), pero `validar()` **ignora
  los campos desconocidos** (`http/validacion.rs:199-237`) y el handler los lee
  igualmente (`http/auth.rs:115-122`).
- `usuarios.crear` los inserta sin comprobar pertenencia
  (`dominio/usuarios.rs:528-544`); `identity.usuarios.tenant_id` no tiene FK
  (`infra/db/init/01_esquemas.sql:17-26`).
- El JWT se emite con ese `tenant_id` (`dominio/auth.rs:322-334`); en login solo
  se valida que la empresa exista y esté `activa`
  (`dominio/auth.rs:262-282`), no que el usuario pertenezca a ella.

**Explotación:** cualquiera se registra como `empresario` en
`POST /api/v2/auth/registro` (ruta pública) con `tenant_id` de la víctima,
verifica su propio correo y obtiene un JWT con el tenant ajeno. Desde ahí puede
leer catálogo, archivos (`/storage/objetos`), pedidos y analítica del tenant,
según qué servicios confíen en `x-tenant`.

**Cierre:** eliminar `tenant_id`/`personal_id` del alta pública; asignarlos solo
server-side (onboarding/aceptar invitación) y validar membresía activa en cada
servicio antes de usar `x-tenant`.

#### C2. La configuración de pagos del tenant permite exfiltrar variables de entorno

- El tenant guarda `config` libre en `PUT /api/v2/empresas/{id}/configuracion-pagos`
  (`orders-service/src/http/pagos.rs:274-290`).
- Al resolver el proveedor, esa config se fusiona **encima** de la del proveedor
  (`orders-service/src/pagos.rs:623-646`, `fusionar_config` en `:1263-1271`).
- `clave_env` se interpreta como **nombre de variable de entorno** y su valor se
  envía como `Bearer` al `url` configurado (`pagos.rs:140-144`, `:636-646`).
- `validar_config_proveedor` solo exige que `url` empiece por `http(s)://` y que
  `clave_env` sea string; no restringe host, IP privada, redirecciones ni nombres
  de variables (`pagos.rs:1291-1320`).

**Explotación (cuando el proveedor `http` está activo, que es justo el estado
necesario para cobrar de verdad):** el empresario configura
`url=https://attacker.tld` y `clave_env=JWT_SECRET` (o `DATABASE_URL`,
`MINIO_ROOT_PASSWORD`, `INTERNAL_API_KEY`), provoca un cobro y el servicio envía
`Authorization: Bearer <JWT_SECRET>` a su servidor. Con ese secreto firma tokens
de cualquier usuario/tenant.

**Cierre:** el tenant no puede sobrescribir `url` ni `clave_env`; allowlist de
proveedores/host, bloqueo de IP privadas y redirecciones, HTTPS obligatorio y
secretos referenciados por identificadores internos, nunca por nombre de
variable de entorno.

#### C3. IDOR cross-tenant en webhooks de notificaciones

- `empresa(&actor, candidata)` acepta el `empresa_id` de query/body **sin
  compararlo** con `actor.tenant_id` (`notifications-service/src/http/webhooks.rs:55-76`).
- Todos los endpoints (listar, crear, actualizar, borrar, rotar secreto,
  reintentar) usan ese helper (`webhooks.rs:141,159,196,228,251,274,297`).
- El gateway permite al rol `empresario` en
  `/api/v2/admin/notifications/webhooks` (`api-gateway/src/policy.rs:134`).

**Explotación:** un empresario de A pasa `empresa_id=B`; puede crear un webhook
de B apuntando a su servidor y recibir todos los eventos de B, enumerar sus
webhooks, rotar su secreto o borrarlos.

**Cierre:** derivar `empresa_id` siempre de `actor.tenant_id`; permitir el
parámetro solo a `super_admin/admin` y verificar pertenencia del recurso.

#### A1. En la configuración de producción documentada, RLS no se aplica

- `.env.production.example:24` y `.env.stage.example:25` conectan como
  `core_engine`, el **owner** del contenedor, no como `core_engine_app`.
- Un owner de tabla **omite RLS** y no hay `FORCE ROW LEVEL SECURITY` en
  `infra/db` (0 ocurrencias).
- Además, las políticas del rol de servicio son de bypass: hay **52**
  `USING (true)` en `infra/db/supabase/99_rls.sql`, y las políticas “reales” son
  `TO authenticated`, un rol que los servicios nunca activan (solo hacen
  `set_config('request.jwt.claims', ...)`).
- `99_rls.sql` no se aplica en local (`docker-compose.yml:46-48`) y se aplica
  “manualmente” en stage/prod.

**Impacto:** el aislamiento real depende **solo** de los `WHERE empresa_id`
escritos a mano. C1 y C3 son la prueba de que eso no basta. Si Supabase/PostgREST
expone los esquemas, `identity.usuarios` (con hashes) tampoco tiene RLS.

**Cierre:** conectar con un rol `NOBYPASSRLS`, eliminar políticas `USING (true)`,
políticas por tenant/negocio para el rol de app y `FORCE ROW LEVEL SECURITY`;
aplicar RLS también en local/CI.

> **Actualización 2026-11-25:** cerrado por
> [ADR-0054](../adr/0054-rls-obligatorio.md). `apply.sh` aplica RLS siempre,
> `99_rls.sql` es idempotente, `99z_rls_hardening.sql` cubre las tablas
> restantes, `core_engine_app` es `NOBYPASSRLS`, no quedan `USING (true)` y
> `npm run test:rls` valida A/B en CI.

#### A2. Webhook de pagos sin HMAC, secreto global y comparación no constante

- `/internal/pagos/webhook` no exige `x-internal-key` (a diferencia del resto de
  rutas internas, `orders-service/src/http/mod.rs:221`).
- Compara `recibido != esperado` en tiempo variable, sin firmar el cuerpo, sin
  timestamp/nonce y tomando `actor` del body
  (`orders-service/src/http/pagos.rs:307-345`).
- Si el tenant no define secreto, usa `PAGO_WEBHOOK_SECRET` global para todos.

**Impacto:** quien obtenga el secreto confirma/rechaza pagos de cualquier tenant
y elige el actor auditado; replay trivial.

**Cierre:** HMAC-SHA256 sobre `timestamp.body` con secreto por proveedor/tenant,
comparación constante, ventana anti-replay + eventos procesados y
`x-internal-key`/mTLS en la ruta interna.

#### A3. SMTP sin TLS en identity (puerto 587)

- `identity-service/src/email.rs:67-80` usa `SmtpTransport::builder_dangerous`
  para todo puerto distinto de 465; el 587 (**el de la plantilla de producción**)
  sale en texto plano.
- `notifications-service/src/email.rs:82-94` usa `Tls::Opportunistic`, vulnerable
  a downgrade por MITM.

**Impacto:** credenciales SMTP y tokens de reset/verificación viajan sin cifrar.

**Cierre:** STARTTLS obligatorio (o 465 con TLS), rechazar el arranque si el
servidor no ofrece TLS y `Tls::Required` en producción.

#### A4. Escalada de privilegios dentro del tenant vía roles

- `asignar()` exige `personal.gestionar`, pero permite asignar **cualquier** rol
  activo de la empresa, incluido el rol transversal `empresario`
  (`personal.rs:408-475`); `assert_rol(..., false)` ni siquiera exige
  `activo=true` (`:1035-1060`).
- `contexto()` considera `empresa_global` a quien tenga una asignación
  `rol_negocio_id IS NULL` + `codigo == 'empresario'` (`:1148-1152`), y toda
  empresa tiene ese rol.
- `crear_rol()` solo valida que las acciones existan en el catálogo, sin
  limitarlas a las que posee el actor (`:1046-1083`).

**Impacto:** un supervisor/coordinador con `personal.gestionar` se asigna el rol
`empresario`, pasa a `empresa_global` y administra todos los negocios; o crea un
rol con permisos de finanzas/catálogo y se lo asigna.

**Cierre:** impedir asignar permisos superiores a los del actor, prohibir la
auto-asignación, reservar `empresario` a `empresario`/admin y exigir `activo=true`.

### 3.1 No existe un runner de migraciones para bases ya desplegadas

- Los 149 SQL viven en `infra/db/init/` y se montan en
  `docker-entrypoint-initdb.d` (`docker-compose.yml:48`). **Ese mecanismo solo
  corre en un volumen PostgreSQL vacío.**
- El pipeline de despliegue no aplica SQL: `deploy.yml` solo hace
  `docker compose up`; no hay paso `psql` ni job de migración.
- La propia guía lo confirma: “un DDL nuevo **no** se aplica solo… Aplícalo a
  mano” (`docs/dockploy-setup.md:117-122`).
- No hay tabla de ledger (`schema_migrations`), ni checksum aplicado en BD, ni
  idempotencia garantizada: 95 archivos usan `ALTER TABLE` y 29 contienen
  `DROP`, sin transacción de migración.
- `db:migrations` solo compara hashes de archivos contra
  `scripts/config/db-migrations.json`; no toca la base.

**Impacto:** el primer upgrade de esquema en producción es un procedimiento
manual, sin registro de qué se aplicó, sin orden verificable y sin rollback. Es
la causa más probable de un incidente grave en el primer mes.

**Cierre mínimo:** runner idempotente con ledger
(`schema_migrations(nombre, checksum, aplicado_en)`), ejecutado como job previo
al deploy, con dry-run, transacción por migración y verificación post-deploy.
Las migraciones deben ser aditivas y compatibles con la versión anterior.

### 3.2 El backup no garantiza recuperación ante pérdida del host

- `infra/db/backup.sh` hace un `pg_dump` custom con retención **local** en el
  volumen `db_backups`, en el mismo VPS (`docker-compose.yml:57-86`).
- El workflow offsite existe pero es **solo `workflow_dispatch`**, sin
  `schedule` (`backup.yml:16`), y depende de un secret que puede no estar
  configurado. El propio README del workflow lo admite.
- No hay backup de **MinIO**: se pierden imágenes, logos, PDFs y documentos
  subidos por los tenants.
- No hay PITR: RPO documentado de **24 h** y RTO de 2 h
  (`docs/runbook-backup-restore.md:8-17`).
- **El restore drill documentado no funciona en un clúster limpio.** El dump se
  genera con `--no-owner --no-privileges` (`backup.sh:25,30`) y
  `restore-verify.sh` restaura sin crear los roles `authenticated` ni
  `core_engine_app`. El resultado real es
  `ERROR: role "authenticated" does not exist` y
  `ERROR: role "core_engine_app" does not exist`; el script sale con código 1.
- Aunque se ignoraran esos errores, la base restaurada quedaría **sin roles,
  grants ni políticas RLS**, es decir, insegura.
- El drill no verifica el SHA-256 del dump ni conteos de tablas críticas; solo
  exige ≥3 esquemas.

**Impacto:** el RTO de 2 h no es alcanzable y el “restore verificado” que promete
el runbook es falso. Pérdida del VPS = pérdida de hasta 24 h de datos y de todos
los objetos. Para un SaaS que cobra y factura, es inaceptable sin aceptación
explícita del cliente.

**Cierre mínimo:** respaldar roles/globales (`pg_dumpall --globals-only` o
bootstrap), crearlos antes del restore, `--exit-on-error`, verificar checksum y
conteos, comprobar `relrowsecurity`/policies, ejecutar el drill en CI sobre un
clúster limpio. Además: backup diario offsite automático (S3/rclone) + backup de
MinIO + decisión documentada de RPO/RTO. PITR antes de superar ~50 tenants.

### 3.3 El dinero real no está cerrado end-to-end

- **Pagos:** `PAGO_PROVEEDOR=simulado` en la plantilla de producción
  (`.env.production.example:74`). Existe una ACL `ProveedorPagos` con adaptador
  HTTP genérico (`packages/orders-service/src/pagos.rs:121-229`), pero no hay
  adaptador de ninguna pasarela concreta (Stripe, BAC, POS, MercadoPago), ni
  conciliación automática: `conciliar` es un endpoint manual
  (`pagos.rs:986`).
- **Facturación electrónica:** el proveedor por defecto es `simulado`
  (`packages/finance-service/src/tributacion.rs:293-302`); no hay integración
  DGI/equivalente ni firma electrónica. El PDF existe, pero el comprobante
  fiscal legal no.
- **Comisiones:** `commissions-service` **no se despliega en producción**
  (`docker-compose.yml` no lo incluye; ADR-37). Las rutas
  `/empresarios/me/ventas` y `/vendedores/me/*` devuelven 502 en prod por diseño.
  > **Actualización 2026-11-25:** cerrado por
  > [ADR-0053](../adr/0053-cobro-centralizado-dispersion-diaria.md). Ver
  > [cierre de venta directa](./2026-11-25-cierre-venta-directa.md).
- **Suscripción de plataforma:** cobro manual con confirmación de admin
  (`docs/backlog-plataforma.md:52`).

**Impacto:** si el MVP cobra a clientes finales o promete comisiones/
liquidaciones, no puede operar legal ni operativamente. Si el piloto cobra por
transferencia manual y no liquida comisiones, es viable con aceptación escrita.

**Cierre mínimo:** elegir el alcance real del piloto (cobro manual vs pasarela),
implementar el adaptador concreto + webhook firmado + conciliación, y decidir si
`commissions` entra en prod o se retira de la promesa comercial.

### 3.4 El rate limit del gateway es evadible y puede agotar memoria

- `ip_de()` confía **siempre** en `X-Forwarded-For`, sin mirar
  `TRUST_FORWARDED_FOR` (`packages/api-gateway/src/rate_limit.rs:52-74`). El
  middleware lo usa para limitar login y tráfico global
  (`packages/api-gateway/src/http/mod.rs:150`).
- Un cliente puede enviar una IP distinta en cada petición y **saltarse los
  10/min de login** (fuerza bruta distribuida por cabecera) y los 300/min
  globales.
- El `HashMap` de ventanas **nunca elimina entradas vencidas**
  (`rate_limit.rs:10-36`). Con IPs falsas, el mapa crece sin cota: agotamiento
  de memoria del gateway.
- El gateway **bufferiza el cuerpo completo sin límite** antes de reenviar
  (`packages/api-gateway/src/proxy.rs:292-295`). Los servicios sí limitan a
  100 KiB, pero el gateway ya habrá consumido la memoria. Un POST grande y
  repetido tumba el proceso público.

**Cierre mínimo:** usar la IP del socket o `X-Forwarded-For` solo si
`TRUST_FORWARDED_FOR=true`; purgar ventanas vencidas; límite de cuerpo en el
gateway (p. ej. 1–2 MiB, salvo media); límite por usuario autenticado además de
por IP.

### 3.5 Deuda de RLS en tablas sensibles

Hay dos problemas superpuestos:

1. **El archivo que contiene la mayor parte de las políticas no se aplica en el
   despliegue Docker.** Los composes montan solo `infra/db/init`
   (`docker-compose.yml:46-48`); `99_rls.sql` “se aplica manualmente en
   staging/prod”. Tras el init estándar, la medición real en PostgreSQL 16.4 es
   **181 tablas, 85 con RLS y 0 con `FORCE RLS`**: quedan **95 tablas de
   aplicación sin RLS**, incluidas `identity.usuarios`, `identity.sesiones`,
   `identity.empresas`, `finance.asientos`, `finance.comprobantes`,
   `orders.eventos`, `storage.objetos` y `commissions.comisiones`.
2. **Aun aplicando `99_rls.sql`, quedan 43 tablas exentas** en el baseline
   (`scripts/config/rls-baseline.json`), incluidas las mismas tablas sensibles.

Además, hay **18 políticas `TO core_engine_app USING (true)`** (analytics,
fiscal, auditoría, personal, entregas digitales, negocios) que anulan el
aislamiento para el rol de servicio. La prueba real con claims del tenant A
devolvió **2 filas de dos empresas** en `analytics.hechos_venta` y en
`finance.configuracion_fiscal`.

Y la configuración de producción documentada conecta como `core_engine`, que es
**superusuario y `rolbypassrls=true`** (C-01/A1): en ese escenario RLS no se
aplica en absoluto.

**Cierre mínimo:** aplicar RLS como migración obligatoria en el despliegue (no
manual), conectar con un rol `NOBYPASSRLS`, eliminar las políticas
`USING (true)`, clasificar las 43 tablas del baseline, activar `FORCE ROW LEVEL
SECURITY` y añadir tests de aislamiento negativos por tabla crítica.

### 3.6 La cadena de suministro Rust no tiene gate

- CI solo ejecuta `npm audit` (`ci.yml:38-52`). **No hay `cargo audit`,
  `cargo deny`, `cargo vet`, SBOM ni escaneo de imágenes.**
- Dependabot solo cubre `github-actions` y `npm`
  (`.github/dependabot.yml`): **no actualiza crates**.
- El escaneo OSV de los 16 `Cargo.lock` encontró **26 avisos** en 11 paquetes.
  Los relevantes para el runtime:
  - `jsonwebtoken 9.3.1` (gateway + identity): **GHSA-h395-gr6q-cpjc**, type
    confusion en `exp`/`nbf`; corregido en **10.3.0**. En el uso actual
    (`Validation::new` exige `exp` y `validate_nbf=false`) no se pudo demostrar
    explotación, pero es la librería que valida todos los JWT.
  - `lettre 0.11.23` (identity + notifications): 6 avisos, incluido inyección
    STARTTLS (RUSTSEC-2026-0560). Afecta al correo transaccional si se usa
    STARTTLS.
  - `h2 0.3.27`: consumo de memoria con DATA frames vacíos
    (RUSTSEC-2026-0258).
  - `chrono`/`chrono-tz`: sin mantenimiento (RUSTSEC-2026-0543/0555).
  - `rsa 0.9.10`, `rustls 0.21.12`, `rustls-webpki 0.101.7`,
    `libsqlite3-sys`, `instant`, `rustls-pemfile`: aparecen en lockfiles; parte
    puede no estar compilada (p. ej. `cargo tree -i rsa` no la encuentra en
    identity). Aun así, no hay gate que distinga alcanzable de residual.

**Cierre mínimo:** job de `cargo audit`/`cargo deny` por servicio (o workspace
unificado), Dependabot para `cargo`, política de excepciones con fecha, y
actualización de `jsonwebtoken`/`lettre` antes del go-live.

### 3.7 Las alertas no llegan a nadie y varias reglas son de otro runtime

- Prometheus tiene reglas (`infra/prometheus/rules/alerts.yml`) pero
  **`prometheus.yml` no define `alerting:` ni Alertmanager**, y el compose de
  observabilidad no levanta Alertmanager. Las alertas solo se ven si alguien
  abre Prometheus.
- Las reglas `EventLoopLagAlto`, `GCDurationAlto` y `HeapUsageCritico` usan
  métricas `nodejs_*`. Los servicios son Rust y **no exponen ninguna métrica
  `nodejs_`** (`packages/api-gateway/src/metrics.rs` solo publica
  `core_engine_*`). Esas alertas nunca disparan.
- `ServicioCaido` usa `absent(up{job=...} == 1)`: con 16 targets, si uno se cae
  la serie sigue existiendo y la alerta no dispara. Debería ser `up == 0`.
- Solo `postgres` y `rabbitmq` tienen `healthcheck` en el compose de producción.
  Un servicio colgado sigue “up” y el gateway devuelve 502/timeout hasta que
  alguien lo reinicia.
- La readiness agregada cubre **11 servicios en producción** y excluye
  `notifications`, `analytics` y `market-intelligence`
  (`packages/api-gateway/src/readiness.rs:14-80`). `notifications` es crítico
  para verificación de correo y reset de contraseña: puede estar caído y
  `/ready` seguir en verde.
- No hay agregación de logs, trazas (OTel solo está como variable vacía) ni
  SLO/SLI. `docs/backlog-plataforma.md:46` lo reconoce.

**Cierre mínimo:** Alertmanager con un canal real (email/Slack/PagerDuty),
reglas basadas en `core_engine_*` y `up == 0`, healthcheck por servicio,
retención/rotación de logs y un dashboard con los cuatro Golden Signals.

### 3.8 Presupuesto de conexiones PostgreSQL insuficiente

- Cada servicio abre un pool de **10 conexiones**
  (`packages/*/src/db.rs:20-24`). Hay **15 servicios con pool**.
- Producción despliega 14 servicios con BD (sin `commissions`): **140
  conexiones potenciales**.
- No hay PgBouncer ni `max_connections` configurado en los composes; el default
  de PostgreSQL 16 es **100**.
- Bajo pico, los servicios compiten por conexiones y los timeouts se
  manifiestan como 500/`pool timed out`, no como un error claro de capacidad.

**Cierre mínimo:** PgBouncer en modo transaction, o bajar pools (p. ej. 3–5) y
subir `max_connections` con margen; medir con carga antes de fijar el número.

### 3.9 Todos los secretos están en todos los contenedores

- `x-servicio-base` aplica `env_file: .env` a **todos** los servicios
  (`docker-compose.yml:22-24`), incluido el gateway expuesto a Internet.
- Eso significa que el gateway, un servicio de notificaciones o storage
  contienen `POSTGRES_PASSWORD`, `MINIO_ROOT_PASSWORD`, `SMTP_PASSWORD`,
  `ADMIN_PASSWORD`, `JWT_SECRET`, claves de pasarela, etc.
- Un RCE o una dependencia comprometida en cualquier contenedor entrega **todas**
  las credenciales, no solo las suyas.

**Cierre mínimo:** variables por servicio (mínimo privilegio), secretos fuera de
`env_file` (Docker secrets / Vault / SSM), y rotación documentada. El gateway no
debe conocer credenciales de base de datos ni de MinIO.

### 3.10 La gobernanza de seguridad da falso verde

Este es un hallazgo de confianza, no de código de negocio:

- `scripts/rules/security/sql-injection.test.mjs`, `ssrf.test.mjs`,
  `sensitive-logs.test.mjs` y `scripts/rules/observability/tracing.test.mjs`
  buscan archivos **`.ts`**. En `packages/` hay **0 archivos `.ts`** (todo es
  Rust). Las suites pasan porque no encuentran nada que revisar.
- `scripts/rules/contract/openapi.test.mjs` parsea controladores
  `*.controller.ts` (`@Controller`, `@Get`, `@Post`) y comprueba que exista
  `EnvelopeInterceptor` en `main.ts`. **No hay ninguno**: las reglas R-DO-9,
  R-DS-5 y parte de R-C-1 son vacuas.
- `swagger:export` no extrae rutas del código Rust: solo agrega los JSON ya
  versionados. El gate G-8 demuestra que los agregados están sincronizados con
  los specs por servicio, **no que los specs reflejen las rutas reales**.
- `rules:check` reporta 83/83, pero solo **51/241 IDs** tienen check ejecutable.
- Las reglas de cobertura viven hardcodeadas en `.github/workflows/ci.yml`
  (líneas 111–174); `scripts/config/coverage-baseline.json` está vacío, así que
  bajar un umbral no lo detecta `baseline:guard`.

**Impacto:** la organización cree tener controles que en la práctica no cubren
el código actual. Es peor que no tenerlos, porque desplaza la revisión manual.

**Cierre mínimo:** portar las suites a Rust (o a un scanner real: Semgrep,
CodeQL, clippy lints de seguridad), generar OpenAPI desde el código (utoipa o
equivalente) o al menos comparar rutas `.route()` con los specs, y poner los
umbrales de cobertura en un baseline versionado.

### 3.11 El producto no incluye frontend

- El repositorio es **backend only**. El frontend Angular + Capacitor está
  declarado como pendiente y entregable separado (`docs/decisiones.md:58-69`,
  ADR-04).
- No hay panel de administración, storefront, app móvil ni flujo de onboarding
  visual. El “MVP” solo es usable por API/curl o por un cliente externo que
  alguien debe construir y desplegar.
- Si el negocio ya tiene frontend en otro repo, este punto se reduce a
  integración; si no, es un bloqueador de producto, no de plataforma.

### 3.12 Cumplimiento y legal

- Facturación electrónica **simulada** (3.3): en Nicaragua, un SaaS que factura
  necesita comprobante fiscal válido; un PDF interno no lo sustituye.
- No hay política de privacidad, términos de servicio, DPA ni registro de
  actividades de tratamiento en el repo. Hay exportación/borrado de tenant
  (`identity.empresas`, `.../exportacion`, `.../eliminacion`), pero no un
  procedimiento legal de atención de derechos.
- `AUDITORIA_RETENCION_DIAS=365` y modo `anonimizar` existen, pero no hay
  verificación de que la purga se ejecute ni de que los backups respeten la
  retención.
- Los logs pueden contener PII (emails, nombres, IDs) y no hay enmascarado
  sistemático; la suite que lo comprobaría es vacua (3.10).

---

### 3.13 El workflow de deploy a producción no despliega producción

- `deploy.yml` copia `.env.example` a `.env` **solo** para dev/stage
  (`deploy.yml:76-78`); para `prod` no prepara entorno.
- El job corre en `ubuntu-24.04` (runner hosted) y ejecuta `docker compose up -d`
  sin `.env` (`deploy.yml:80-87`). `docker-compose.yml:41` exige
  `POSTGRES_PASSWORD` con `${...:?}`: verificado, `docker compose config` falla
  sin `.env`.
- El healthcheck y el smoke apuntan a `http://localhost:8080`
  (`deploy.yml:92,120`), es decir, al stack efímero del runner, no al VPS ni a
  Dockploy.
- El teardown hace `docker compose ... down -v --remove-orphans` **también en
  prod** (`deploy.yml:133`). En un host real eso borra los volúmenes.

**Impacto:** el flujo llamado “prod” no puede desplegar producción tal como está
escrito y, si se ejecutara en el host, podría destruir datos. El despliegue real
depende de Dockploy y no está automatizado ni verificado por este workflow.

**Cierre:** separar “validación de compose” de “deploy real”; integrar
Dockploy/SSH o eliminar el job `prod`; apuntar el smoke a la URL real; nunca
`down -v` en producción.

### 3.14 Bloqueadores funcionales de la promesa multi-vertical

La revisión funcional contrastó el README/OpenAPI con el código. El retail de
productos funciona; el resto de la promesa, en gran parte, no.

- **Servicios y reservas no se pueden vender.** Inventory solo resuelve líneas
  contra `item.items` con `tipo='producto'`
  (`inventory-service/src/reservas.rs:76`) y el consumidor manda todas las
  líneas no digitales al inventario (`consumidor.rs:355-371`). Además, una orden
  tipo `reserva` exige `fulfillment=onsite`
  (`orders-service/src/checkout.rs:1384-1388`), capacidad que belleza y dental
  **no tienen** en el pack (`infra/db/init/24_multi_vertical.sql:100-127`). Una
  cita de belleza/dental termina en `stock.fallido` y orden cancelada.
- **78 operaciones públicas del OpenAPI no tienen destino en el gateway.**
  Calculado contra `proxy.rs`: 510 operaciones públicas, **78 sin ruta**. No
  están: `/fidelizacion/**` (12), `/contratos/**` (4), `/suscripciones/**` (8),
  `/mesas` y `/comandas` (5), `/entregas-digitales/**` (2),
  `/admin/devoluciones/**` (5), `/admin/pagos/**` (6), `/admin/proveedores-pago`
  (3), `/admin/tipos-orden/**` (7), `/api-keys` (3), `/comprador/resenas`,
  `/favoritos`, `/admin/resenas`, `/tienda/items/{id}/resenas`,
  `/exportaciones/{id}/descarga` y `/negocios/plantillas`. Varias tampoco tienen
  política, así que aun añadiendo el destino darían 403.
- **La expiración puede cancelar una orden pagada.** El job selecciona órdenes
  `creada` sin pago confirmado (`orders-service/src/expiracion.rs:39-59`) y
  luego llama `transicionar(..., "cancelada")` sin bloquear ni revalidar
  (`:69-75`). `transicionar` valida contra el estado leído antes de abrir la
  transacción (`checkout.rs:732-740`) y la matriz permite `pagada → cancelada`
  (`order_state.rs:73`). Si el pago entra entre la selección y la transición, la
  orden queda cancelada y el stock liberado, con el pago confirmado.
- **El ingreso contable depende de una comisión que no existe en producción.**
  Finance registra Caja/Fondos por liquidar en `payment.procesado` y solo
  realiza el ingreso con `comision.acreditada`
  (`finance-service/src/consumidor.rs:121-131,154-205`). `commissions-service`
  no se despliega en prod y, en venta directa, omite líneas sin vendedor
  (`commissions-service/src/pedidos.rs:141-151`): nunca llega
  `comision.acreditada`. El resultado es una venta cobrada que queda como pasivo
  y sin ingreso.
- **Logística crea envíos físicos para todo.** `payment.procesado` llama
  `preparar_envios_de_orden` para cualquier orden
  (`logistics-service/src/consumidor.rs:75`) y agrupa todas las líneas sin mirar
  `fulfillment` (`envios/mod.rs:193-244`). Digital, pickup y onsite generan guía;
  las verticales sin `fulfillment_delivery` no pueden avanzar el envío.
- **RMA/reembolsos no cierran financieramente.** La devolución parcial emite
  `devolucion.recibida`, pero finance solo consume `devolucion.solicitada`
  (`finance-service/src/main.rs:68-73`) y la nota de crédito solo revierte IVA
  (`facturacion.rs:790-850`); el reembolso actualiza el pago pero no emite
  asiento.
- **Crédito y multi-moneda se contabilizan mal.** El pago con método `credito`
  debita Caja (`finance/consumidor.rs:121-131`) y el asiento no lleva moneda;
  el evento automático usa la moneda de la orden aunque el pago sea en otra
  (`orders/consumer.rs:95`).
- **Notificaciones y webhooks fallan en venta directa.** `OrdenExterna.vendedor_id`
  es `String` (`notifications-service/src/tipos.rs:13`) y orders devuelve `null`
  (`orders-service/src/views.rs:24`); además `empresa_de` solo busca
  `data.empresa_id` y `order.created`/`order.status.updated`/`payment.procesado`
  no lo incluyen (`webhooks.rs:456-467`).

**Impacto:** el producto que se puede vender hoy es “retail de producto con un
vendedor y pago simulado”. Belleza, dental, restaurante, taller, alquiler,
fidelización, suscripciones, contratos, mesas y RMA no están operables de punta a
punta por la API pública.

**Cierre:** decidir el alcance real del MVP (retail de producto) y retirar de la
promesa el resto, o cerrar cada flujo: servicios/reservas sin inventory,
fulfillment por línea, gateway completo + políticas, ingreso contable
independiente de la comisión, y RMA con asiento.

### 3.15 Datos e infraestructura: fallos verificados en ejecución

- **`scheduling.outbox` no drena bajo RLS.** `31_multi_vertical_rls.sql:195`
  habilita RLS en `scheduling.outbox`, pero solo define política admin/insert
  (`:251-258`). El poller usa `db.transaccion()` sin claims
  (`scheduling-service/src/outbox.rs:40-49`, `main.rs:36-40`), así que no ve
  pendientes: la prueba real devolvió **0**. Los eventos de agenda se insertan
  pero nunca se publican.
- **Deadlock potencial en la reserva de inventario.** Las líneas se bloquean en
  el orden del JSON (`inventory-service/src/reservas.rs:31,38,77`); dos órdenes
  con los mismos ítems en orden inverso pueden bloquearse mutuamente (`40P01`).
- **Numeración fiscal con carrera.** `finance-service/src/facturacion.rs:384-395`
  lee la serie fuera de la transacción y sin `FOR UPDATE`; dos emisiones
  concurrentes pueden colisionar en `UNIQUE(serie,numero)` (`01_esquemas.sql:498`).
- **“Append-only” no está impuesto.** `orders.eventos` y `finance.asientos` no
  tienen trigger que rechace `UPDATE`/`DELETE`; el rol de app recibe
  `SELECT, INSERT, UPDATE, DELETE` sobre todas las tablas
  (`51_app_grants.sql:23-35`). Un bug o servicio comprometido puede reescribir la
  historia de pedidos o asientos.
- **Triggers financieros O(n²).** `01b_cqrs_finance.sql:111-132` reconstruye todo
  el mes por cada fila insertada; un asiento de N líneas reconstruye el mes N+1
  veces.
- **Dinero en `integer` y moneda inconsistente.** 27 columnas `_cents integer`
  limitan a ≈21,47 M de unidades; `orders.orden_vista.moneda` y
  `finance.comprobantes.moneda` usan `DEFAULT 'C$'` mientras otras tablas usan
  `'NIO'`, sin `CHECK` ISO.
- **Correo único global.** `identity.usuarios.correo` es `UNIQUE` global
  (`01_esquemas.sql:20`): una persona no puede pertenecer a dos tenants con el
  mismo correo.
- **`99_rls.sql` no es idempotente ni aplicable.** Sobre el esquema actual
  produce **44 errores** (políticas ya existentes, `text = uuid`, tabla
  `item.historico_precios` eliminada). El gate `db:rls` solo lee ese archivo, no
  la base real: es un falso verde.

**Cierre:** política de outbox de servicio, orden estable de locks con retry
`40P01`, secuencial fiscal con `UPDATE ... RETURNING` dentro de la transacción,
triggers/revocaciones para append-only, `bigint` + `CHECK` de moneda y RLS
idempotente probada dos veces en CI.

## 4. Hallazgos P1 (primeros 30 días / primer pico)

### 4.0 Seguridad: hallazgos medios verificados

| # | Hallazgo | Evidencia | Impacto |
|---|---|---|---|
| M1 | Aceptar una invitación marca el correo como verificado sin prueba de control del buzón | `identity-service/src/dominio/personal.rs:970-973` | Quien tenga el enlace (reenvío, logs, correo comprometido) activa la cuenta y salta la verificación. |
| M2 | Altas de vendedor e invitaciones solo exigen 8 caracteres, no la política de 12 + complejidad | `dominio/usuarios.rs:1028-1033`, `dominio/personal.rs:845-850`, `http/empresas.rs:133` | Cuentas operativas con contraseñas débiles. |
| M3 | Enumeración de usuarios, timing oracle y bloqueo de cuenta como DoS | `dominio/usuarios.rs:528-530,578-592` | Se puede descubrir qué correos existen y bloquear cuentas ajenas con 5 intentos. |
| M4 | MFA sin contador de intentos ni bloqueo | `dominio/auth.rs:77-98` | Fuerza bruta del TOTP de 6 dígitos (agravada por A5). |
| M5 | Un solo HS256 para access, refresh y API keys; sin `iss`/`aud`; access no revocable | `api-gateway/src/auth.rs:33-43`, `identity-service/src/auth.rs:215-255` | Un secreto filtrado forja tokens de cualquier rol; logout no invalida el access 15 min. |
| M6 | El rol `admin` es global, no de tenant | `identity-service/src/auth.rs:380-382`, `dominio/personal.rs:1090-1101` | Un admin de tenant opera sobre todos los tenants. |
| M8 | Los servicios confían en `x-user-*`/`x-tenant`; la red Docker es plana | `identity-service/src/auth.rs:317-359` y homólogos | Un contenedor comprometido llama directo a `identity:3001` y suplanta cualquier usuario. |
| M9 | Comparación de `x-internal-key` con `!=` en varios servicios (no timing-safe) | `orders-service/src/auth.rs:152-162`, `storage-service/src/auth.rs:106-114`, `finance-service/src/http/mod.rs:154-164` | Fuga por temporización de la clave interna que protege endpoints cross-tenant. |
| M10 | Dev/observabilidad publican puertos en `0.0.0.0` | `docker-compose.dev.yml:49-50,101-103,127-129`; `docker-compose.observability.yml:71-73` | Exposición de Postgres, RabbitMQ, MinIO y Grafana si el host es accesible. |
| M11 | MinIO usa credenciales root y hereda la contraseña de RabbitMQ como fallback | `docker-compose.yml:113-124` | Un compromiso de storage da control total del bucket; el fallback mezcla secretos entre servicios. |
| M12 | Registro, reset y reenvío de verificación no tienen cubo propio de rate limit | `api-gateway/src/rate_limit.rs:96-103` | Alta masiva de cuentas y email bombing a víctimas usando el SMTP de la plataforma. |

### 4.1 Dependencias Rust y actualizaciones

- Sin `cargo audit`, `cargo deny` ni Dependabot para `cargo` (3.6).
- 16 `Cargo.lock` independientes: drift entre servicios
  (`storage-service` exige Rust 1.94.1; el resto 1.89). No hay workspace Rust
  unificado, lo que multiplica el coste de actualizar.
- Imagen de build `rust:1.94.1-bookworm` por tag, no por digest; runtime
  `debian:bookworm-slim` sin digest. Reproducibilidad parcial.

### 4.2 Autenticación y sesiones

Bien: bcrypt cost 10, política de contraseña, bloqueo por intentos
(`usuarios.rs:568-640`), reset con hash SHA-256 + 30 min + un solo uso,
verificación de correo con hash + 24 h, TOTP y códigos de recuperación,
revocación de sesión por `jti`.

Pendiente:

- **Refresh sin rotación**: se reutiliza el mismo `jti` al renovar
  (`dominio/auth.rs:108-135`; backlog #13). Robo de refresh = sesión indefinida
  hasta expiración.
- **JWT HS256 con una sola clave, sin `kid` ni rotación** (backlog #5).
- **API keys sin scopes**: el rol de la key define todo el acceso (backlog #2).
- **Rate limit de login evadible** (3.4), lo que degrada el bloqueo por cuenta.
- No hay MFA obligatorio para `super_admin`/`admin`.

### 4.3 SSRF en webhooks salientes

`notifications-service` valida esquema, credenciales embebidas y longitud, pero
**acepta `http://127.0.0.1`** (test explícito en
`packages/notifications-service/src/webhooks.rs:568-570`). En producción
`WEBHOOK_HTTPS_OBLIGATORIO=true` bloquea HTTP plano, pero no bloquea
`https://` a IP privada, DNS rebinding ni metadata endpoints. Backlog #3.

**Cierre:** resolver DNS y rechazar rangos privados/loopback/link-local;
allowlist opcional por tenant; egress proxy.

### 4.4 Jobs de fondo y escala horizontal

Cada réplica ejecuta sus propios tickers: outbox (1 s), expiración de órdenes,
expiración de puntos (6 h), reportes, etc. El outbox usa `SKIP LOCKED` y es
seguro; la expiración de órdenes y los reportes no tienen evidencia de
coordinación multi-réplica. Hoy el despliegue es de **una réplica**, así que no
explota; al escalar, puede duplicar trabajo o generar carreras.

**Cierre:** advisory lock por job (`pg_try_advisory_lock`) o worker dedicado.

### 4.5 Sin límites de recursos ni política de logs

- Los composes no definen `deploy.resources`, `mem_limit`, `cpus` ni
  `logging`/rotación. Un servicio con fuga de memoria puede tumbar el VPS; los
  logs Docker pueden llenar el disco.
- No hay `stop_grace_period` ni `init: true`; el apagado ordenado depende del
  handler de señales de cada binario (el gateway sí lo tiene).
- No hay circuit breaker en el gateway (backlog #4): un upstream lento consume
  timeouts de 30 s y conexiones.

### 4.6 Versionado y release

- `package.json` está en **1.9.0**, pero el último tag es **v1.11.0** y los
  `Cargo.toml` en **1.0.9**. El commit `fbda4d91` “alinear la versión de develop
  con 1.9.0” retrocedió la versión respecto al tag.
- `release-check` exige que `package.json` y el lockfile coincidan con el tag;
  el próximo release fallará si no se hace `version:bump` antes.
- `Cargo.toml` no se versiona con el release; las imágenes llevan label de
  versión, pero el binario no reporta su versión.
- `stage-images.yml` construye y publica `:stage` en cada push a `staging` sin
  depender del éxito de CI; el deploy es manual, pero la imagen puede quedar
  publicada antes de que los checks terminen.

### 4.7 OpenAPI y contratos

- Hay **588 operaciones** en `docs/openapi` (510 públicas + 78 internas), no las
  484/440 que dice el README ni las 359 que dice `AUDIT.md`.
- El contrato se mantiene a mano por servicio; no se genera desde Rust. No hay
  verificación de que una ruta nueva en `.route()` esté en el spec, ni de que un
  DTO nuevo esté documentado. El gate G-8 no cubre eso (3.10).
- El envelope `{data,meta}` está implementado en los servicios, pero la
  comprobación automática de “toda respuesta 2xx lo usa” se apoya en specs
  manuales.

### 4.8 Documentación desactualizada

- `docs/rules/_meta/AUDIT.md` describe NestJS, `catalog-service`, 1.533 tests y
  `npm test`; nada de eso existe ya.
- El README cita `main.ts`, `@ApiProperty`, `AsyncLocalStorage`, 11 servicios y
  484 operaciones; el repo tiene 16 servicios Rust y 588 operaciones.
- `docs/guia-dominios-personalizados.md:124` dice que la próxima migración es la
  96; ya hay 149.
- `security/reports/2026-10-24-api-staging.md` describe Caddy/Express y un
  `:8080` público que el compose actual ya cerró en loopback; sirve como
  historia, no como estado.
- El `AUDIT.md` afirma que el gate de seguridad cubre SQLi/SSRF/logs, pero las
  suites son vacuas (3.10).

### 4.9 Configuración que puede divergir

- `COMMISSION_RATE` está fijado a `0.12` en el compose de `finance-service`
  (`docker-compose.yml:229`), mientras `orders` y `commissions` lo leen de
  `.env`. Si se cambia la tasa, **finance puede contabilizar una comisión
  distinta de la que cobra orders**.
- `CORS_ORIGINS=*` en dev no emite cabeceras CORS (el código compara con lista
  exacta), lo que puede confundir la puesta en marcha del frontend.
- `ADMIN_PASSWORD` solo se exige presente, no fuerte; la plantilla trae
  `<CAMBIAR>`, que es un valor válido para el seed. Rotar tras el primer arranque
  es un paso manual.

### 4.10 Stage se despliega con credenciales de ejemplo versionadas

`deploy.yml:76-78` copia `.env.example` a `.env` para dev y stage. Esa plantilla
trae `RABBITMQ_PASSWORD=0b66...` (`.env.example:25`), `ADMIN_PASSWORD=AdminCore
Engine2026!` (`:87`) y `GRAFANA_ADMIN_PASSWORD=admin` (`:118`). Si el borde de
stage falla, el compromiso es inmediato.

**Cierre:** secretos del Environment de GitHub/Dockploy, fallo si el valor es
placeholder y credenciales distintas por entorno.

### 4.11 CI construye una sola imagen; release publica 15 no probadas

- `ci.yml:213-215` solo hace `docker build` de `identity-service`.
- `release.yml` construye las 16 imágenes y `e2e.yml` prueba el stack **desde
  código fuente**, no las imágenes publicadas.

**Impacto:** un Dockerfile roto en cualquiera de las otras 15 pasa CI y falla en
release; la imagen publicada no es la que se probó.

**Cierre:** matriz de build de las 16 imágenes en CI + smoke sobre los digests
publicados + SBOM/escaneo/firma.

### 4.12 El restore drill no prueba integridad real

`infra/db/restore-verify.sh` solo comprueba que existan al menos 3 esquemas y
cuenta `identity.empresas`; no valida tablas críticas, conteos, secuencias,
constraints ni checksums. Un restore parcial puede pasar como “verificado”.

**Cierre:** checklist por esquema/tabla, conteos esperados, constraints y una
lectura de datos clave.

### 4.13 No hay SLO, on-call ni proceso de incidentes

No hay SLO/SLI ni error budget, guardia, severidades, escalado ni postmortem
fuera del runbook de backup. Las alertas no están enrutadas (3.7) y la carga se
limita a 2 VUs/20 s en E2E. No se puede operar con promesas de disponibilidad.

**Cierre:** SLO por servicio, Alertmanager + guardia, runbook de incidentes y
prueba de carga periódica con umbrales.

---

## 5. Hallazgos P2 (escala y madurez)

| Tema | Estado | Evidencia / impacto |
|---|---|---|
| Rate limit distribuido | Inexistente | En memoria por réplica; backlog #6. Dos réplicas duplican el límite. |
| Réplicas de lectura | Inexistente | Una sola `DATABASE_URL`; reportes compiten con checkout. Backlog #7. |
| Búsqueda dedicada | Trigram | Suficiente para MVP; backlog #8. |
| OpenTelemetry | Solo variable | `OTEL_EXPORTER_OTLP_ENDPOINT` no se usa; no hay trazas. Backlog #9. |
| PITR | Inexistente | RPO 24 h; backlog #1. |
| HA de Postgres/RabbitMQ/MinIO | Un solo nodo | Cualquier caída del host detiene la plataforma. |
| Canary / feature flags | Inexistente | Rollback = redesplegar digest anterior; no hay despliegue progresivo. |
| Pruebas de carga | 2 VUs / 20 s | `security/k6/order-flow.js`; no mide capacidad real ni p95 bajo carga. |
| Chaos / restore game day | Inexistente | El runbook lo pide, no hay evidencia. |
| Auditoría de acceso a datos | Parcial | `identity.auditoria` existe; no cubre todos los servicios. |
| Retención de `provisioning_jobs` | Sin limpieza | Backlog #19. |
| Dominios personalizados | No soportado | Backlog #18; `DOMINIO_NO_SOPORTADO`. |
| Datos demo por pack | No soportado | Backlog #17. |
| Cobro en el alta | Manual | Backlog #14; el registro sigue libre. |
| Duplicación de infraestructura | 16 copias | `metrics.rs`, `logs.rs`, `errors.rs` y `db.rs` replicados; un fix de seguridad hay que aplicarlo 16 veces. |
| Código muerto declarado | 10 crates | `#![allow(dead_code)]` en identity, inventory, item, logistics, market-intelligence, notifications, orders, pricing, scheduling y stores. |
| Funciones gigantes | 133 > 80 líneas | `orders/checkout.rs:126` (530 líneas), `finance/rutas.rs:12` (327), `identity/http/mod.rs:63` (321). |
| Tests del gateway | Solo unitarios | No hay `packages/api-gateway/tests/`; router, auth y proxy no se prueban con el binario real. |
| Deploy por tag mutable | `:prod`/`:stage` | La promoción es por digest, pero el compose consume tags; una re-promoción cambia el artefacto. |

---

## 6. Lo que sí está bien (no reescribir)

1. **Modelo de dominio**: bounded contexts por capacidad, no por rubro; vertical
   + pack + capacidades es una decisión acertada (ADR-0001).
2. **Dinero**: enteros en centavos, 49 columnas `*_cents`, 49 `CHECK >= 0`,
   formato de dos decimales; la comisión se calcula sobre el neto.
3. **Consistencia**: outbox transaccional con `SKIP LOCKED`, idempotencia HTTP,
   event store con versión única, DLQ y reintentos.
4. **Multi-tenancy (diseño)**: políticas RLS por rol/negocio escritas y JWT con
   `tenant_id`/`negocio_id`; el gateway reconstruye cabeceras limpias y no
   reenvía las del cliente (`proxy.rs:180-240`). El diseño es bueno, pero **no
   está efectivo** con la configuración de producción documentada (A1).
5. **Auth**: bcrypt, bloqueo por intentos, reset/verificación con tokens
   hasheados y un solo uso, TOTP, revocación de sesión.
6. **Ingeniería de entrega**: release por tag, promoción por digest, E2E antes
   de publicar imágenes, backups con checksum y runbooks. El deploy `prod` del
   workflow no es real (3.13); el diseño sí es correcto.
7. **Pruebas**: ~1.365 funciones de test, integración contra PostgreSQL real,
   cobertura medida por servicio, 0 `#[ignore]`.
8. **Contratos**: envelope `{data,meta}`, errores estructurados y superficies
   pública/interna separadas. El contrato está bien formado, pero **78
   operaciones públicas no tienen destino en el gateway** (3.14).

La deuda no es de diseño: es de **operación, perímetro, conexión de extremo a
extremo y verificación**. Lo que funciona hoy es un **retail de producto con un
vendedor**; el resto de la promesa multi-vertical no está conectado.

---

## 7. Plan de salida a producción

### Fase 0 — “No romper ni filtrar” (1–2 semanas)

| # | Acción | Criterio de aceptación |
|---|---|---|
| 0.0 | **Cerrar C1/C2/C3**: quitar `tenant_id`/`personal_id` del registro; impedir que el tenant defina `url`/`clave_env`; derivar la empresa del token en webhooks | Test de registro con `tenant_id` ajeno → 400/ignorado; test de config de pagos con `clave_env=JWT_SECRET` → 400; test de webhook con `empresa_id` ajeno → 403. |
| 0.1 | Corregir rate limit (IP confiable + purga + límite por usuario) y body limit en gateway | Test que falsifica `X-Forwarded-For` no evade el 429; mapa no crece con IPs aleatorias; POST > límite devuelve 413. |
| 0.2 | Alertmanager + canal real + reglas Rust (`up == 0`, 5xx, latencia, outbox, DLQ) | Una alerta de prueba llega al canal en < 5 min. |
| 0.3 | Healthcheck por servicio + restart policy + límites de memoria/CPU + rotación de logs | `docker compose ps` muestra healthy; matar un servicio lo reinicia; disco no crece sin control. |
| 0.4 | `cargo audit`/`cargo deny` en CI + actualizar `jsonwebtoken` y `lettre` | Gate rojo con un CVE; 0 avisos altos alcanzables. |
| 0.5 | Secretos por servicio; el gateway sin credenciales de BD/MinIO/SMTP | Inspección de `docker inspect` del gateway no muestra esos secretos. |
| 0.6 | Portar las suites vacuas a Rust o sustituirlas por Semgrep/CodeQL | Un SQLi/SSRF/log sensible de prueba hace fallar el gate. |
| 0.7 | **A1**: conectar servicios con rol `NOBYPASSRLS`, quitar políticas `USING (true)` y activar `FORCE RLS` | Un usuario del tenant A recibe 0 filas del tenant B en usuarios, sesiones, pedidos, finanzas y storage. |
| 0.8 | **A2/A3/A4**: HMAC + anti-replay en webhook de pagos, STARTTLS obligatorio, y prohibir auto-asignación de roles superiores | Tests negativos para secreto reutilizado, downgrade SMTP y escalada de rol. |

### Fase 1 — “Datos y dinero” (2–4 semanas)

| # | Acción | Criterio de aceptación |
|---|---|---|
| 1.1 | Runner de migraciones con ledger + job previo al deploy; incluir `99_rls.sql` y `00_roles.sh` | Upgrade de una BD existente en stage sin intervención manual; re-ejecutar es no-op; `99_rls.sql` aplica sin errores dos veces. |
| 1.2 | Backup offsite automático + backup de MinIO + restore drill automatizado en clúster limpio | Restore completo en < 2 h desde un artifact offsite, con roles creados, checksum y `relrowsecurity` verificados; objeto de MinIO restaurado. |
| 1.3 | Decidir y cerrar pagos: adaptador real + webhook firmado + conciliación | Un cobro y un reembolso reales en sandbox del proveedor, con conciliación automática. |
| 1.4 | Decidir comisiones: desplegar `commissions` en prod o retirar la promesa | `/empresarios/me/ventas` responde 200 en prod, o el módulo no se anuncia. |
| 1.5 | RLS real en despliegue: rol NOBYPASSRLS, 0 políticas `USING(true)`, 0 tablas de tenant sin RLS | Test de fuga cross-tenant en usuarios, sesiones, asientos, comprobantes y storage; `current_user` no es superusuario. |
| 1.6 | PgBouncer o presupuesto de conexiones validado con carga | 0 `pool timed out` a 2× el pico esperado. |
| 1.7 | Facturación electrónica: integración real o aceptación escrita de que no se emite | Comprobante válido ante la autoridad, o alcance del piloto limitado. |
| 1.8 | Outbox de scheduling, orden de locks de inventario y secuencial fiscal concurrente | Un evento de agenda se publica y consume; dos checkouts cruzados no dan `40P01`; dos comprobantes concurrentes no colisionan. |

### Fase 2 — “Escala y cumplimiento” (4–8 semanas)

| # | Acción | Criterio de aceptación |
|---|---|---|
| 2.1 | Réplicas + jobs con advisory lock + rate limit distribuido | Dos réplicas no duplican jobs ni límites. |
| 2.2 | OTel + trazas W3C + SLO/SLI + dashboards por tenant | Una orden se sigue de gateway a finance. |
| 2.3 | PITR + restore a timestamp + game day | RPO < 15 min probado. |
| 2.4 | Escaneo y firma de imágenes (Trivy + cosign) + SBOM | Imagen sin CVEs altas y verificable por digest. |
| 2.5 | Frontend/integración de cliente + onboarding visual | Un usuario crea empresa y opera sin curl. |
| 2.6 | Privacidad: ToS, política, DPA, retención y derechos | Exportación/borrado verificados end-to-end con backups incluidos. |
| 2.7 | Rotación JWT (`kid`) + refresh rotation + scopes de API keys | Rotar secreto sin cortar sesiones; refresh reutilizado revoca familia. |

---

## 8. Checklist go/no-go para el primer tenant

**No go si alguna respuesta es “no”:**

- [ ] ¿El registro público no permite fijar `tenant_id`/`personal_id`, y el JWT
      solo se emite con membresía verificada? (C1)
- [ ] ¿La configuración de pagos no permite al tenant elegir `url`/`clave_env`?
      (C2)
- [ ] ¿Los webhooks de notificaciones derivan la empresa del token y no del
      body/query? (C3)
- [ ] ¿RLS está realmente activo (rol NOBYPASSRLS + `FORCE RLS`), no solo
      declarado? (A1)
- [ ] ¿El alcance vendible está decidido: retail de producto, o también
      servicios/reservas? Si es lo segundo, ¿el checkout de reserva funciona?
      (3.14)
- [ ] ¿Todas las operaciones que el OpenAPI publica tienen destino y política en
      el gateway? (3.14)
- [ ] ¿La expiración no puede cancelar una orden pagada? (3.14)
- [ ] ¿El ingreso contable se reconoce al cobrar, sin depender de una comisión
      que puede no existir? (3.14)
- [ ] ¿Logística respeta `fulfillment` por línea y no crea envíos digitales o de
      pickup? (3.14)
- [ ] ¿Existe un plan de migración aplicado y probado en stage sobre una BD ya
      existente?
- [ ] ¿Hay un backup offsite verificado y un restore drill ejecutado esta
      semana?
- [ ] ¿El restore drill funciona en un clúster limpio, con roles creados y
      checksum verificado, y lo han ejecutado al menos una vez?
- [ ] ¿`scheduling.outbox` (y todos los outbox) drena con el rol de servicio?
- [ ] ¿La reserva de inventario ordena locks y tolera `40P01`?
- [ ] ¿La numeración fiscal es segura bajo concurrencia?
- [ ] ¿`orders.eventos` y `finance.asientos` rechazan UPDATE/DELETE?
- [ ] ¿El cobro y la facturación del tenant son reales o están explícitamente
      fuera de alcance por escrito?
- [ ] ¿El rate limit no se evade con cabeceras y el gateway tiene límite de
      cuerpo?
- [ ] ¿Las alertas llegan a una persona y hay alguien de guardia?
- [ ] ¿Los secretos están segregados por servicio?
- [ ] ¿Las tablas de usuarios/sesiones/finanzas están cubiertas por RLS o por
      un test de aislamiento?
- [ ] ¿El webhook de pagos usa HMAC y la ruta interna exige clave interna? (A2)
- [ ] ¿SMTP usa TLS obligatorio en producción? (A3)
- [ ] ¿Un rol operativo no puede auto-asignarse permisos superiores? (A4)
- [ ] ¿El equipo puede restaurar en < 2 h y lo ha hecho al menos una vez?
- [ ] ¿Hay un frontend o cliente que el tenant pueda usar?
- [ ] ¿El contrato con el tenant acepta RPO 24 h, disponibilidad best-effort y
      ausencia de facturación electrónica (si aplica)?

**Go con condiciones** solo si: 1–3 tenants, tráfico < 50 req/s, cobro manual,
sin promesas de SLA, con acompañamiento diario y capacidad de congelar
escrituras para restaurar.

---

## 9. Anexo — Evidencia reproducible

```bash
# Estado y versión
git log -1 --format='%H %ci %s'
git tag --sort=-v:refname | head
grep -m1 '"version"' package.json
grep -m1 '^version' packages/identity-service/Cargo.toml

# Gates ejecutados
npm run security:secrets
npm run rules:check
npm run test:engine
npm run test:smoke
npm run baseline:guard
npm run env:check
npm run db:migrations
npm run db:rls

# Deuda RLS (43 tablas)
cat scripts/config/rls-baseline.json | jq length

# C1 · registro acepta tenant_id ajeno
sed -n '15,20p' packages/identity-service/src/http/auth.rs
sed -n '115,122p' packages/identity-service/src/http/auth.rs
sed -n '528,544p' packages/identity-service/src/dominio/usuarios.rs
sed -n '17,26p' infra/db/init/01_esquemas.sql

# C2 · config de pagos del tenant sobrescribe url/clave_env
sed -n '457,530p' packages/orders-service/src/pagos.rs
sed -n '623,646p' packages/orders-service/src/pagos.rs
sed -n '1291,1320p' packages/orders-service/src/pagos.rs

# C3 · IDOR en webhooks de notificaciones
sed -n '55,76p' packages/notifications-service/src/http/webhooks.rs
sed -n '134,160p' packages/notifications-service/src/http/webhooks.rs

# A1 · producción conecta como owner y RLS no aplica
grep -n 'DATABASE_URL' .env.production.example
grep -rn 'FORCE ROW LEVEL SECURITY' infra/db | wc -l
grep -c 'USING (true)' infra/db/supabase/99_rls.sql
grep -n 'docker-entrypoint-initdb.d' docker-compose.yml

# A2 · webhook de pagos sin clave interna ni HMAC
sed -n '307,345p' packages/orders-service/src/http/pagos.rs

# A3 · SMTP sin TLS en 587
sed -n '67,80p' packages/identity-service/src/email.rs

# Datos/infra · RLS real y restore
grep -n 'docker-entrypoint-initdb.d' docker-compose.yml
grep -rn 'FORCE ROW LEVEL SECURITY' infra/db | wc -l
grep -c 'USING (true)' infra/db/supabase/99_rls.sql
sed -n '25,35p' infra/db/backup.sh
sed -n '31,45p' infra/db/restore-verify.sh
grep -n 'role "authenticated"\|role "core_engine_app"' /tmp/restore-*.log 2>/dev/null || true

# Outbox de scheduling y deadlock de inventario
sed -n '190,260p' infra/db/init/31_multi_vertical_rls.sql
sed -n '31,80p' packages/inventory-service/src/reservas.rs

# Rate limit / body / secretos
sed -n '52,74p' packages/api-gateway/src/rate_limit.rs
sed -n '292,295p' packages/api-gateway/src/proxy.rs
sed -n '22,24p' docker-compose.yml
grep -n 'PAGO_PROVEEDOR' .env.production.example

# Alertas sin Alertmanager y métricas Node en stack Rust
grep -n 'alerting\|alertmanagers' infra/prometheus/prometheus.yml
grep -n 'nodejs_' infra/prometheus/rules/alerts.yml
grep -n 'healthcheck' docker-compose.yml

# Migraciones solo-init
grep -n 'docker-entrypoint-initdb.d' docker-compose.yml
sed -n '117,122p' docs/dockploy-setup.md
grep -n 'schedule' .github/workflows/backup.yml

# Suites vacuas (0 archivos .ts)
find packages -name '*.ts' -not -path '*/node_modules/*' -not -path '*/target/*' | wc -l
grep -n "endsWith('.ts')" scripts/rules/security/*.mjs
grep -n "controller.ts" scripts/rules/contract/openapi.test.mjs

# Dependencias Rust
grep -n 'jsonwebtoken\|lettre' packages/api-gateway/Cargo.toml packages/notifications-service/Cargo.toml
grep -n 'cargo' .github/dependabot.yml || echo 'sin cargo en Dependabot'
grep -n 'npm audit' .github/workflows/ci.yml

# Conexiones
grep -rn 'max_connections' packages/*/src/db.rs
```

### Limitaciones de esta auditoría

- No se ejecutó el stack completo ni el E2E (`npm run demo`) por coste de build
  de 16 imágenes Rust; el E2E está configurado en `e2e.yml` y se ejecuta en
  release.
- No se hizo pentest nuevo; el informe de staging de 2026-10-24 se cita como
  antecedente y puede estar desactualizado.
- La cobertura real se midió en 2 de 16 servicios (api-gateway 85,03/77,00;
  analytics 67,74/58,03); para el resto se usan los umbrales de CI como mínimo,
  no como cobertura efectiva.
- Las pruebas de RLS, restore y outbox se hicieron sobre PostgreSQL
  16.4/PostGIS efímero, no sobre la base de producción; los fallos son
  reproducibles con el código y los compose del repositorio.
- Los avisos OSV se obtuvieron de los 16 `Cargo.lock`; parte puede corresponder
  a dependencias no compiladas. Se marca explícitamente cuáles son alcanzables
  por el árbol de dependencias.
