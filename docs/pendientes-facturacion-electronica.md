# Pendientes — facturación electrónica certificada

- **Fecha:** 2026-11-25
- **Alcance:** [ADR-0049](./adr/0049-regiones-tributarias.md),
  `finance-service/src/facturacion/`
- **Estado:** adaptador de referencia implementado; **sin certificación ni
  validez fiscal**

Este documento es la lista de cierre. Un PDF interno o un XML generado por
`FacturacionDgi` **no sustituyen** un comprobante fiscal autorizado.

## 1. Lo que ya hace el código

- `FacturacionDgi` genera un XML de referencia y lo publica en
  `FACTURACION_PROVEEDOR_URL`.
- `FacturacionSimulada` ya **no** marca el comprobante como `aceptada`: lo
  marca como `no_fiscal`.
- Si un tenant activa `facturacion_electronica=true` con `simulado` y el
  piloto no aceptó operar sin comprobante, la emisión falla con
  `FACTURACION_NO_CERTIFICADA` antes de consumir el secuencial.
- Si el proveedor es `http`/`dgi` pero no está certificado, la emisión falla
  con `FACTURACION_NO_CERTIFICADA`.
- El PDF interno agrega la leyenda
  `*** COMPROBANTE NO FISCAL - NO VALIDO ANTE LA DGI ***` cuando no hay
  aceptación electrónica válida.
- `GET /api/v2/finanzas/configuracion-fiscal` publica:
  - `certificacion_estado`: `no_aplica` | `no_certificada` |
    `piloto_sin_comprobante` | `certificada`.
  - `emite_comprobante_fiscal`: booleano.

## 2. Decisión pendiente (una de las dos)

### Opción A — certificar el proveedor electrónico

Requisito para retail formal en Nicaragua. Implica:

1. Contratar/activar un proveedor autorizado o la DGI.
2. Obtener credenciales, certificado/firma y endpoint de producción.
3. Validar el esquema XML, la firma, el envío y la respuesta de aceptación.
4. Configurar `FACTURACION_PROVEEDOR_CERTIFICADO=true` y
   `EMITIR_COMPROBANTES_FISCALES=true`.
5. Probar factura, nota de crédito, contingencia y anulación.

### Opción B — piloto sin comprobante fiscal

Solo válida si se **acepta por escrito** que el piloto no emite comprobante
fiscal. Se refleja en el entorno con:

```dotenv
EMITIR_COMPROBANTES_FISCALES=false
FACTURACION_PERMITIR_SIN_CERTIFICADO=true
FACTURACION_PROVEEDOR_CERTIFICADO=false
```

Con esta opción el sistema sigue operando, pero:

- los comprobantes quedan `no_fiscal`;
- el PDF lleva la leyenda de no válido;
- no se debe presentar como factura legal;
- sigue siendo obligatorio certificar antes de retail formal.

## 3. Bloqueadores técnicos y operativos

| # | Pendiente | Por qué bloquea | Criterio de aceptación |
|---|---|---|---|
| 1 | **Proveedor/certificación** | El adaptador es de referencia; no hay autorización de la DGI ni de un proveedor certificado. | Un comprobante de prueba es aceptado por la DGI/proveedor y devuelve referencia válida. |
| 2 | **Credenciales y endpoint** | `FACTURACION_PROVEEDOR_URL/KEY` están vacíos y la URL de ejemplo no es real. | Endpoint y credenciales de sandbox/producción configurados y verificados. |
| 3 | **Esquema XML y firma** | El XML actual es un formato de referencia; no está validado contra el esquema oficial ni firmado. | XML validado contra XSD oficial y firma verificada por el receptor. |
| 4 | **Respuesta y estados** | Hoy se interpreta `aceptada`/`referencia`; faltan estados oficiales (aceptado, rechazado, contingencia, procesando). | Mapeo de estados del proveedor y persistencia de errores por código. |
| 5 | **Nota de crédito y anulación** | No se ha probado el ciclo fiscal completo. | Nota de crédito aceptada y referencia cruzada con la factura original. |
| 6 | **Contingencia** | No hay flujo para DGI caída ni comprobantes diferidos. | Cola de contingencia y reenvío controlado. |
| 7 | **Datos fiscales del emisor** | RUC, razón social, régimen y serie dependen de datos del tenant; falta validación contra la DGI. | Emisor validado y serie autorizada para la región. |
| 8 | **Numeración y secuenciales** | El secuencial se consume aunque el envío falle; hay que confirmar la regla fiscal aplicable. | Política de secuencial documentada y probada con rechazos. |
| 9 | **PDF como representación** | Un PDF interno no es el comprobante; debe representar el XML autorizado. | PDF incluye referencia/estado fiscal y no se emite como válido sin aceptación. |
| 10 | **Producción** | Las plantillas dejan `EMITIR_COMPROBANTES_FISCALES=false` y el proveedor simulado. | Variables de producción con certificación o waiver explícito, y `/ready` verde. |
| 11 | **Prueba E2E** | No se ha ejecutado contra DGI/proveedor real. | Factura + PDF + nota de crédito + contingencia con evidencia. |
| 12 | **Aceptación por escrito (si aplica)** | Sin certificación, el piloto no puede presentarse como retail formal. | Documento firmado por negocio/legal con alcance y fecha de cierre. |

## 4. Checklist de cierre

1. Elegir Opción A u Opción B y registrarla.
2. Si A: cargar credenciales/certificado, validar XML/firma y endpoint.
3. Emitir una factura de prueba y verificar aceptación + referencia.
4. Descargar el PDF y comprobar que representa el comprobante aceptado.
5. Emitir una nota de crédito y verificar el cruce con la factura.
6. Probar un rechazo y una contingencia.
7. Configurar producción y ejecutar el smoke fiscal.
8. Si B: archivar la aceptación escrita y mantener la leyenda NO FISCAL.

## 5. Dónde está el código

- Adaptadores: `packages/finance-service/src/facturacion/mod.rs`.
- Emisión y validación previa: `src/facturacion/emision.rs`.
- Configuración y estado de certificación:
  `src/tributacion/configuracion.rs`,
  `src/http/finanzas/facturacion.rs`.
- PDF y leyenda NO FISCAL: `src/pdf.rs`.
- Variables: `.env.production.example`, `.env.stage.example`, `.env.example`.
- Runbook: `docs/runbook-regiones-tributarias.md`.
