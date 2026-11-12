# Blueprint · Tecnología y reparación

> **Esto es configuración, no código.** No crea módulos ni servicios nuevos:
> compone las piezas existentes (items, inventario, agenda, cotizaciones,
> logística por capacidades, crédito y registros) para un negocio de venta de
> tecnología + reparación de celulares.

## 1. Crear la vertical

```http
POST /api/v2/packs/verticales
Content-Type: application/json

{
  "codigo": "tecnologia_reparacion",
  "nombre": "Tecnología y reparación",
  "modelo_operacion": "mixto"
}
```

`mixto` ya existe y significa: productos + servicios + alquileres + logística.

## 2. Crear el pack

Usar `docs/blueprints/tecnologia-reparacion.pack.json` como cuerpo de:

```http
POST /api/v2/packs
```

Capacidades incluidas y qué habilitan:

| Capacidad | Uso en este negocio |
|---|---|
| `catalogo_items` | Productos, servicios de reparación y alquileres |
| `inventario_stock` | Stock de accesorios, repuestos y equipos |
| `codigo_barras` | Etiquetas de mostrador y almacén |
| `inventario_lotes` | Repuestos con vencimiento o número de lote |
| `agenda_recursos` | Taller, técnicos y citas de recepción |
| `cotizaciones` | Presupuesto de reparación → aceptación → orden |
| `facturacion` / `liquidaciones` | Facturas y comisiones de técnicos |
| `fulfillment_delivery` | Entrega con flota de plataforma |
| `fulfillment_pickup` | Retiro en tienda |
| `fulfillment_onsite` | Servicio a domicilio |
| `logistica_rutas` | Flota propia |
| `ventas_credito` | Ventas fiadas con cupo y abonos |
| `registros` | Diagnósticos, notas e historial operativo (no clínico) |

`historia_clinica` y `recetas` **no** se incluyen: son específicas de salud.
`registros` es la capacidad genérica para cualquier histórico por cliente.

## 3. Crear la empresa

```http
POST /api/v2/empresas
Content-Type: application/json

{
  "nombre": "TechFix",
  "slug": "techfix",
  "vertical_id": "tecnologia_reparacion",
  "pack_codigo": "pack_tecnologia_reparacion",
  "pack_version": 1
}
```

## 4. Logística: propia, global o híbrida

La flota se deriva de las capacidades activas, sin configuración extra:

| Capacidades del pack | Modalidad resultante |
|---|---|
| `logistica_rutas` | **Propia** (flota de la empresa) |
| `fulfillment_delivery` | **Global** (flota de plataforma) |
| ambas | **Híbrida**: cada envío nace sin proveedor y el empresario elige |

En híbrida:

```http
POST /api/v2/empresarios/me/envios/:id/asignacion
{ "proveedor_tipo": "empresa" | "plataforma" }
```

## 5. Operaciones típicas

- **Venta de accesorios**: producto con stock; la orden descuenta inventario.
- **Reparación**: servicio (`tipo_orden: "orden_servicio"`) + repuestos como
  productos en la misma orden. La mano de obra no descuenta stock; los
  repuestos sí.
- **Reparación con composición**: el servicio puede tener componentes
  (`/api/v2/catalog/items/:id/componentes`); al venderlo se descuentan los
  repuestos hoja, sin descontar el servicio.
- **Diagnóstico/historial**: registro operativo ligado a la `parte` (cliente)
  con `tipo`, `datos` JSON y adjuntos.
- **Presupuesto**: cotización → aceptar → orden de compra.
- **Crédito**: `PUT /api/v2/comercio/credito-cuentas/:parteId` fija el cupo y
  el checkout con `metodo_pago: "credito"` registra la cuenta por cobrar.

## 6. Ajustes posteriores

- Cambiar de pack: `PATCH /api/v2/empresas/:id/pack`.
- Nuevas versiones del pack: crear `version: 2` y actualizar la empresa.
- El pack es por empresa; para combinaciones nuevas se crea otro pack con la
  unión de capacidades (no se requiere código).
