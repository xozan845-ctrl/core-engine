# Seguridad ofensiva · staging `api.kbcoleccion.com`

Este directorio contiene el material reproducible de la revisión de seguridad
del entorno de staging. **Solo debe usarse contra activos propios o con
autorización escrita del propietario.**

## Alcance autorizado

| Elemento | Valor |
| --- | --- |
| Objetivo | `https://api.kbcoleccion.com` |
| IP | `179.236.225.19` (`srv2020744.hstgr.cloud`) |
| Ventana | 2026-10-24, revisión puntual |
| Modo | Caja gris (código del gateway disponible en este repo) |

Fuera de alcance: explotación destructiva, fuerza bruta de credenciales,
denegación de servicio sostenida, pruebas sobre otros dominios del VPS o
servicios de terceros.

## Herramientas MCP configuradas

La configuración vive en `opencode.jsonc` (raíz del repo):

- **`pentest`** — [`DMontgomery40/pentest-mcp`](https://github.com/DMontgomery40/pentest-mcp)
  (`pentest-mcp@0.9.0`), transporte stdio.
- **`k6`** — [`grafana/mcp-k6`](https://github.com/grafana/mcp-k6), ejecutado en
  Docker (`grafana/mcp-k6:latest`).

Herramientas instaladas en `~/.local/bin`:

| Herramienta | Uso |
| --- | --- |
| `nmap` (wrapper Docker `instrumentisto/nmap`) | puertos y servicios |
| `nuclei` + plantillas firmadas | vulnerabilidades y exposiciones |
| `subfinder` | subdominios pasivos |
| `httpx` (ProjectDiscovery) | fingerprint HTTP/TLS |
| `ffuf`, `gobuster` | descubrimiento de contenido |
| `nikto` (Perl local) | chequeos web clásicos |

Los tools destructivos del MCP (`hydraBruteforce`, `runJohnTheRipper`,
`runHashcat`, `privEscAudit`, `extractionSweep`) están **denegados** en
`opencode.jsonc`. Para habilitarlos hace falta autorización explícita y un
acuerdo de alcance.

Verificar conexión:

```sh
opencode mcp list
```

## Uso

```text
# Reconocimiento no intrusivo
pentest_httpxProbe / pentest_nmapScan / pentest_nucleiScan

# Carga ligera (k6)
k6_run_script con security/k6/health-smoke.js
```

Scripts:

- `k6/health-smoke.js`: 5 VUs, 20 s, solo `/health`. Baseline seguro.
- `k6/health-load.js`: rampa hasta 20 VUs. **No ejecutar sin aprobación** y sin
  monitorizar el VPS.

## Remediación aplicada

Los hallazgos de la revisión del 2026-10-24 se corrigieron en el repositorio:

- Gateway publicado solo en loopback (`127.0.0.1:8080:8080`) en stage y prod.
- `/ready` exige `x-internal-key`, tiene límite propio (10/min) y caché de 5 s.
- `trust proxy` configurable (`TRUST_PROXY`, por defecto `1`) para que el rate
  limit use la IP real detrás de Traefik/Dockploy.
- Staging exige `EDGE_BASIC_AUTH` (`REQUIRE_EDGE_AUTH=true`): el arranque falla
  sin ella. Un Bearer inválido ya no puede saltarse el Basic.
- Cabeceras `Permissions-Policy`, COOP, COEP, CORP y
  `X-Permitted-Cross-Domain-Policies` añadidas.
- Hardening de host en `security/hardening/` (SSH sin SHA-1 y Traefik sin CBC).

Variables nuevas: `READY_PUBLIC`, `READY_CACHE_MS`, `TRUST_PROXY`,
`REQUIRE_EDGE_AUTH`. Están documentadas en `.env.stage.example`,
`.env.production.example` y `.env.example`.

## Reglas de engagement

1. No atacar `/ready` con carga: hace fan-out a 8 microservicios.
2. No superar 20 VUs ni 2 minutos sin aprobación.
3. No probar credenciales reales ni datos de clientes.
4. Parar si aparecen errores 5xx sostenidos o latencia anómala.
5. Registrar hora, origen y comandos ejecutados.
