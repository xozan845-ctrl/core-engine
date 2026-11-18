# ADR-0031 — Facturación electrónica y PDF configurables por negocio

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0014](./0014-finance-service-en-rust.md),
  [ADR-0026](./0026-fiscalidad-por-tenant.md)

## Contexto

El comprobante era interno: no había envío a un proveedor electrónico ni PDF.
DGI es específico de Nicaragua; hardcodearlo rompería el principio agnóstico.
El negocio debe poder activar la función y elegir proveedor/ambiente.

## Decisión

1. **Capacidad `facturacion_electronica`.** Se siembra en identity y se asigna
   a las verticales; el pack/negocio decide activarla. Añade los features
   `finanzas.facturacion_electronica` y `finanzas.facturacion_pdf`.
2. **Configuración por tenant.** `finance.configuracion_fiscal` incorpora
   `facturacion_electronica`, `formato_pdf`, `proveedor_electronico`
   (`simulado|http`) y `ambiente_electronico` (`pruebas|produccion`).
3. **ACL de proveedor.** `ProveedorFacturacionElectronica` abstrae el envío.
   `FacturacionSimulada` permite desarrollo/pruebas; `FacturacionHttp` habla con
   un proveedor genérico (`POST /comprobantes`). Un adaptador DGI concreto se
   implementa fuera del dominio o detrás de esa URL.
4. **Emisión tolerante.** Si el proveedor falla, el comprobante se conserva y
   queda `electronica_estado='error'`; no se pierde la numeración ni el asiento.
5. **PDF bajo demanda.** `GET /finanzas/comprobantes/{id}/pdf` genera un PDF
   mínimo (Helvetica, una página) con serie, número, base, IVA y total. Solo
   responde si `formato_pdf=true` y la capacidad está activa.
6. **Sin DGI hardcodeada.** No se incluyen XML/certificados DGI en el núcleo;
   el adaptador HTTP permite conectar el proveedor autorizado de cada país.

## Consecuencias

- **Positivas:** cada negocio activa la función cuando la necesita; el mismo
  núcleo sirve para países/proveedores distintos.
- **Contrato:** configuración fiscal extendida, campos electrónicos en el
  comprobante y ruta PDF.
- **Datos:** migración `99_facturacion_electronica.sql`; RLS existente.
- **Pendiente:** adaptador DGI concreto, firma/certificados, XML UBL y envío
  automático de notas de crédito.

## Criterios de aceptación

- Sin capacidad/flag, el comprobante sigue siendo interno.
- Con `facturacion_electronica=true`, el comprobante guarda estado y referencia
  del proveedor.
- Con `formato_pdf=true`, el PDF descarga con `application/pdf`.
- Sin `formato_pdf`, el PDF responde `PDF_NO_ACTIVADO`.
- El proveedor HTTP se configura por entorno y no por código de país.
