/**
 * GENERADO por scripts/swagger-aggregate.mjs — NO editar a mano.
 * Fuente: docs/openapi/*.json (npm run swagger:export). Verifica: gate G-8.
 */
import type { OpenAPIObject } from '@nestjs/swagger';

export const OPENAPI_AGREGADO = {
  "openapi": "3.0.0",
  "info": {
    "title": "Core Engine · API (agregada)",
    "description": "Documento agregado de todos los microservicios, servido por el API Gateway. Cada operacion se enruta a traves del gateway (prefijo /api/v1).",
    "version": "v1"
  },
  "tags": [],
  "paths": {
    "/internal/productos/lote": {
      "get": {
        "operationId": "InternalController_lote",
        "parameters": [
          {
            "name": "skus",
            "required": false,
            "in": "query",
            "description": "SKUs separados por coma",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Lote de productos por SKUs (servicio→servicio)",
        "tags": [
          "Catálogo (interno)"
        ]
      }
    },
    "/internal/productos/sku/{sku}": {
      "get": {
        "operationId": "InternalController_porSku",
        "parameters": [
          {
            "name": "sku",
            "required": true,
            "in": "path",
            "description": "SKU del producto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Producto por SKU (servicio→servicio)",
        "tags": [
          "Catálogo (interno)"
        ]
      }
    },
    "/internal/productos/{id}": {
      "get": {
        "operationId": "InternalController_porId",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Producto por id (servicio→servicio)",
        "tags": [
          "Catálogo (interno)"
        ]
      }
    },
    "/internal/inventario/resumen": {
      "get": {
        "operationId": "InternalController_resumen",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Resumen de inventario (servicio→servicio)",
        "tags": [
          "Catálogo (interno)"
        ]
      }
    },
    "/internal/inventario/lineas": {
      "get": {
        "operationId": "InternalController_lineas",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Líneas de inventario (servicio→servicio)",
        "tags": [
          "Catálogo (interno)"
        ]
      }
    },
    "/api/v1/catalog/productos": {
      "get": {
        "operationId": "ProductosController_listar",
        "parameters": [
          {
            "name": "q",
            "required": false,
            "in": "query",
            "description": "Búsqueda por texto",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "description": "Filtrar por estado",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "categoria",
            "required": false,
            "in": "query",
            "description": "Filtrar por categoría",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "description": "Página (1-based)",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "description": "Tamaño de página (máx 100)",
            "schema": {
              "minimum": 1,
              "maximum": 100,
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar productos (público, filtros y paginación)",
        "tags": [
          "Catálogo"
        ]
      },
      "post": {
        "operationId": "ProductosController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearProductoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear producto (admin, TC-01)",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/catalog/productos/{id}": {
      "get": {
        "operationId": "ProductosController_detalle",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "404": {
            "description": "Producto inexistente (NO_ENCONTRADO)"
          }
        },
        "summary": "Detalle de producto (público)",
        "tags": [
          "Catálogo"
        ]
      },
      "patch": {
        "operationId": "ProductosController_actualizar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarProductoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Editar producto (admin; RN-08 histórico)",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/admin/inventario": {
      "get": {
        "operationId": "InventarioController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Niveles de stock (admin; Tabla 21)",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/vendedores/me/liquidaciones": {
      "get": {
        "operationId": "LiquidacionesController_mias",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Cortes/liquidaciones del vendedor autenticado",
        "tags": [
          "Comisiones · Liquidaciones"
        ]
      }
    },
    "/api/v1/admin/liquidaciones/corte": {
      "post": {
        "operationId": "LiquidacionesController_corteManual",
        "parameters": [
          {
            "name": "inicio",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "fin",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Ejecutar un corte de liquidaciones (admin)",
        "tags": [
          "Comisiones · Liquidaciones"
        ]
      }
    },
    "/api/v1/admin/liquidaciones/{id}/pagar": {
      "post": {
        "operationId": "LiquidacionesController_pagar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la liquidación",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Marcar una liquidación como pagada (RN-07)",
        "tags": [
          "Comisiones · Liquidaciones"
        ]
      }
    },
    "/api/v1/vendedores/me/ventas": {
      "get": {
        "operationId": "VentasController_estado",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Estado de cuenta de ventas/comisiones del vendedor",
        "tags": [
          "Comisiones · Ventas"
        ]
      }
    },
    "/api/v1/admin/reportes": {
      "get": {
        "operationId": "ReportesController_kpis",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "KPIs de ventas e inventario (admin)",
        "tags": [
          "Comisiones · Reportes"
        ]
      }
    },
    "/api/v1/field/personal": {
      "get": {
        "operationId": "FieldController_listarPersonal",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar personal",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearPersonal",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearPersonalRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear personal",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/personal/{id}": {
      "get": {
        "operationId": "FieldController_obtenerPersonal",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener personal por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarPersonal",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarPersonalRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar personal",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarPersonal",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar personal",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/personal/{id}/ubicacion": {
      "get": {
        "operationId": "FieldController_ubicacionPersonal",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Ubicación del personal",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarUbicacion",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/UbicacionRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar ubicación del personal",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/clientes": {
      "get": {
        "operationId": "FieldController_listarClientes",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar clientes de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearCliente",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearClienteRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear cliente de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/clientes/{id}": {
      "get": {
        "operationId": "FieldController_obtenerCliente",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener cliente por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarCliente",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarClienteRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar cliente de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarCliente",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar cliente de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/vehiculos": {
      "get": {
        "operationId": "FieldController_listarVehiculos",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar vehículos",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearVehiculo",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearVehiculoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear vehículo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/vehiculos/{id}": {
      "get": {
        "operationId": "FieldController_obtenerVehiculo",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener vehículo por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarVehiculo",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarVehiculoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar vehículo",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarVehiculo",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar vehículo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/rutas": {
      "get": {
        "operationId": "FieldController_listarRutas",
        "parameters": [
          {
            "name": "personalId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar rutas",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearRuta",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearRutaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/rutas/{id}": {
      "get": {
        "operationId": "FieldController_obtenerRuta",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener ruta por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarRuta",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarRutaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarRuta",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/rutas/{id}/paradas": {
      "post": {
        "operationId": "FieldController_agregarParada",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ParadaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Agregar parada a la ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/rutas/{id}/paradas/{paradaId}": {
      "patch": {
        "operationId": "FieldController_actualizarParada",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "paradaId",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ParadaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar parada de la ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/rutas/{id}/asignar": {
      "post": {
        "operationId": "FieldController_asignarRuta",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AsignarRutaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Asignar personal/vehículo a la ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/pedidos": {
      "get": {
        "operationId": "FieldController_listarPedidos",
        "parameters": [
          {
            "name": "estado",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "rutaId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "clienteId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar pedidos de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearPedido",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearPedidoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear pedido de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/pedidos/{id}": {
      "get": {
        "operationId": "FieldController_obtenerPedido",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener pedido por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarPedido",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarPedidoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar pedido de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarPedido",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar pedido de campo",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/pedidos/{id}/estado": {
      "patch": {
        "operationId": "FieldController_cambiarEstadoPedido",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CambiarEstadoPedidoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Cambiar estado del pedido",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/asistencia": {
      "get": {
        "operationId": "FieldController_listarAsistencia",
        "parameters": [
          {
            "name": "personalId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "fecha",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar asistencia",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_registrarAsistencia",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearAsistenciaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar asistencia",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/incidencias": {
      "get": {
        "operationId": "FieldController_listarIncidencias",
        "parameters": [
          {
            "name": "estado",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "rutaId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar incidencias",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_crearIncidencia",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearIncidenciaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear incidencia",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/incidencias/{id}": {
      "get": {
        "operationId": "FieldController_obtenerIncidencia",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener incidencia por id",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "patch": {
        "operationId": "FieldController_actualizarIncidencia",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarIncidenciaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar incidencia",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "delete": {
        "operationId": "FieldController_eliminarIncidencia",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Eliminar incidencia",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/tracking": {
      "get": {
        "operationId": "FieldController_listarTracking",
        "parameters": [
          {
            "name": "personalId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "desde",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar tracking GPS",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_registrarTracking",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/TrackingBulkRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar tracking GPS (lote)",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/visitas": {
      "get": {
        "operationId": "FieldController_listarVisitas",
        "parameters": [
          {
            "name": "personalId",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "fecha",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar visitas (telemetría)",
        "tags": [
          "Field (logística de campo)"
        ]
      },
      "post": {
        "operationId": "FieldController_registrarVisita",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearVisitaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar visita (telemetría)",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/cumplimiento/{rutaId}/{fecha}": {
      "get": {
        "operationId": "FieldController_obtenerCumplimiento",
        "parameters": [
          {
            "name": "rutaId",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "fecha",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Obtener cumplimiento de ruta/fecha",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/cumplimiento": {
      "post": {
        "operationId": "FieldController_guardarCumplimiento",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/GuardarCumplimientoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Guardar cumplimiento de ruta",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/api/v1/field/sync": {
      "post": {
        "operationId": "FieldController_sincronizar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/SyncRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Sincronizar operaciones offline",
        "tags": [
          "Field (logística de campo)"
        ]
      }
    },
    "/internal/finance/sujetos/{usuarioId}": {
      "get": {
        "operationId": "InternoController_sujeto",
        "parameters": [
          {
            "name": "usuarioId",
            "required": true,
            "in": "path",
            "description": "Id del usuario",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Situación fiscal de un usuario (para identity)",
        "tags": [
          "Finanzas (interno)"
        ]
      }
    },
    "/internal/finance/declaraciones": {
      "get": {
        "operationId": "InternoController_declaraciones",
        "parameters": [
          {
            "name": "tipo",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "periodo_inicio",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Declaraciones del periodo (reportes)",
        "tags": [
          "Finanzas (interno)"
        ]
      }
    },
    "/api/v1/finanzas/cuentas": {
      "get": {
        "operationId": "ContabilidadController_cuentas",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Plan de cuentas",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      },
      "post": {
        "operationId": "ContabilidadController_crearCuenta",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCuentaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear una cuenta contable",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/cuentas/{codigo}/estado": {
      "post": {
        "operationId": "ContabilidadController_estadoCuenta",
        "parameters": [
          {
            "name": "codigo",
            "required": true,
            "in": "path",
            "description": "Código de la cuenta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/EstadoCuentaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Cambiar el estado de una cuenta",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/asientos": {
      "get": {
        "operationId": "ContabilidadController_libroDiario",
        "parameters": [
          {
            "name": "desde",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "limite",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Libro diario (asientos)",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      },
      "post": {
        "operationId": "ContabilidadController_registrar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearAsientoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar un asiento manual",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/asientos/{id}/anular": {
      "post": {
        "operationId": "ContabilidadController_anular",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del asiento",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Anular un asiento",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/libro-mayor/{cuenta}": {
      "get": {
        "operationId": "ContabilidadController_libroMayor",
        "parameters": [
          {
            "name": "cuenta",
            "required": true,
            "in": "path",
            "description": "Código de la cuenta",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "desde",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Libro mayor por cuenta",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/libro-ventas": {
      "get": {
        "operationId": "ContabilidadController_libroVentas",
        "parameters": [
          {
            "name": "desde",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Libro de ventas (DGI)",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/libro-compras": {
      "get": {
        "operationId": "ContabilidadController_libroCompras",
        "parameters": [
          {
            "name": "desde",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Libro de compras (DGI)",
        "tags": [
          "Finanzas · Contabilidad"
        ]
      }
    },
    "/api/v1/finanzas/comprobantes": {
      "get": {
        "operationId": "FacturacionController_listar",
        "parameters": [
          {
            "name": "desde",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar comprobantes fiscales",
        "tags": [
          "Finanzas · Facturación"
        ]
      },
      "post": {
        "operationId": "FacturacionController_emitir",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/EmitirComprobanteRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Emitir un comprobante (factura o nota de crédito)",
        "tags": [
          "Finanzas · Facturación"
        ]
      }
    },
    "/api/v1/finanzas/comprobantes/series": {
      "get": {
        "operationId": "FacturacionController_series",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Series de comprobantes",
        "tags": [
          "Finanzas · Facturación"
        ]
      }
    },
    "/api/v1/finanzas/comprobantes/{id}/anular": {
      "post": {
        "operationId": "FacturacionController_anular",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del comprobante",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Anular un comprobante",
        "tags": [
          "Finanzas · Facturación"
        ]
      }
    },
    "/api/v1/finanzas/jurisdicciones": {
      "get": {
        "operationId": "TributacionController_jurisdicciones",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar jurisdicciones",
        "tags": [
          "Finanzas · Tributación"
        ]
      },
      "post": {
        "operationId": "TributacionController_crearJurisdiccion",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearJurisdiccionRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear una jurisdicción",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/regimenes": {
      "get": {
        "operationId": "TributacionController_regimenes",
        "parameters": [
          {
            "name": "jurisdiccion",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar regímenes fiscales",
        "tags": [
          "Finanzas · Tributación"
        ]
      },
      "post": {
        "operationId": "TributacionController_crearRegimen",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearRegimenRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear un régimen fiscal",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/sujetos": {
      "get": {
        "operationId": "TributacionController_sujetos",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar sujetos tributarios",
        "tags": [
          "Finanzas · Tributación"
        ]
      },
      "post": {
        "operationId": "TributacionController_registrarSujeto",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/RegistrarSujetoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar un sujeto tributario",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/sujetos/{id}/baja": {
      "post": {
        "operationId": "TributacionController_darDeBaja",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del sujeto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Dar de baja un sujeto tributario",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/declaraciones": {
      "get": {
        "operationId": "TributacionController_declaraciones",
        "parameters": [
          {
            "name": "tipo",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "periodo_inicio",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar declaraciones",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/declaraciones/generar": {
      "post": {
        "operationId": "TributacionController_generar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/GenerarDeclaracionesRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Generar declaraciones del periodo",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/declaraciones/{id}/presentar": {
      "post": {
        "operationId": "TributacionController_presentar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la declaración",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Presentar una declaración",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/declaraciones/{id}/pagar": {
      "post": {
        "operationId": "TributacionController_pagar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la declaración",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Marcar una declaración como pagada",
        "tags": [
          "Finanzas · Tributación"
        ]
      }
    },
    "/api/v1/finanzas/proyecciones": {
      "post": {
        "operationId": "FinanzasController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearProyeccionRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear una proyección financiera",
        "tags": [
          "Finanzas · Modelo"
        ]
      },
      "get": {
        "operationId": "FinanzasController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar proyecciones",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/proyecciones/{id}": {
      "get": {
        "operationId": "FinanzasController_detalle",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la proyección",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Detalle de una proyección",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/punto-equilibrio": {
      "get": {
        "operationId": "FinanzasController_puntoEquilibrio",
        "parameters": [
          {
            "name": "costos_fijos_cents",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "ticket_promedio_cents",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "comision_tasa",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "horizonte_meses",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Punto de equilibrio (tablas 8.12/8.11)",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/kpis": {
      "get": {
        "operationId": "FinanzasController_kpisDelMes",
        "parameters": [
          {
            "name": "mes",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "KPIs financieros del periodo (tabla 8.8)",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/proyecciones/sensibilidad": {
      "get": {
        "operationId": "FinanzasController_sensibilidad",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Estudios de sensibilidad (8.7, 8.10, 8.11, 8.12)",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/proyecciones/plan-bienal": {
      "get": {
        "operationId": "FinanzasController_planBienal",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Plan a 24 meses (8.6 + 8.9)",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/finanzas/tablero": {
      "get": {
        "operationId": "FinanzasController_tablero",
        "parameters": [
          {
            "name": "mes",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "costo_entrega_por_pedido_cents",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Tablero financiero (KPIs + alertas, tabla 8.10)",
        "tags": [
          "Finanzas · Modelo"
        ]
      }
    },
    "/api/v1/vendedores/me/fiscal": {
      "get": {
        "operationId": "VendedorFiscalController_situacion",
        "parameters": [
          {
            "name": "mes",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Situación fiscal del vendedor (cap. 4.4)",
        "tags": [
          "Finanzas · Vendedor"
        ]
      }
    },
    "/internal/usuarios/{id}": {
      "get": {
        "operationId": "InternalController_usuario",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del usuario",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Usuario por id (servicio→servicio)",
        "tags": [
          "Usuarios (interno)"
        ]
      }
    },
    "/internal/vendedores": {
      "get": {
        "operationId": "InternalController_vendedores",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar vendedores (servicio→servicio)",
        "tags": [
          "Usuarios (interno)"
        ]
      }
    },
    "/internal/contar-vendedores": {
      "get": {
        "operationId": "InternalController_contarVendedores",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Contar vendedores (servicio→servicio)",
        "tags": [
          "Usuarios (interno)"
        ]
      }
    },
    "/api/v1/auth/registro": {
      "post": {
        "operationId": "AuthController_registrar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/RegistroRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Sesión creada (JWT + refresh)"
          }
        },
        "summary": "Registrar vendedor o comprador",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/login": {
      "post": {
        "operationId": "AuthController_login",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/LoginRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Sesión iniciada"
          },
          "401": {
            "description": "Credenciales inválidas"
          }
        },
        "summary": "Iniciar sesión (JWT + refresh)",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/refresh": {
      "post": {
        "operationId": "AuthController_refresh",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "refresh_token"
                ],
                "properties": {
                  "refresh_token": {
                    "type": "string"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Renovar la sesión con refresh token",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/crear-usuario": {
      "post": {
        "operationId": "AuthController_crearUsuario",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearUsuarioRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Usuario creado"
          },
          "403": {
            "description": "Solo un administrador puede crear usuarios"
          }
        },
        "summary": "Crear usuario (solo admin)",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/cambiar-contrasena": {
      "post": {
        "operationId": "AuthController_cambiarContrasena",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CambiarContrasenaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Cambiar la contraseña (autenticado)",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/restablecer-contrasena": {
      "post": {
        "operationId": "AuthController_restablecerContrasena",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/RestablecerContrasenaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Solicitar restablecimiento de contraseña (público)",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/me": {
      "get": {
        "operationId": "AuthController_yo",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Perfil de la sesión actual",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/auth/vincular-personal": {
      "post": {
        "operationId": "AuthController_vincularPersonal",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/VincularPersonalRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Vincular la ficha de personal del usuario autenticado",
        "tags": [
          "Auth"
        ]
      }
    },
    "/api/v1/usuarios": {
      "get": {
        "operationId": "UsuariosController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar usuarios (admin)",
        "tags": [
          "Usuarios"
        ]
      }
    },
    "/api/v1/usuarios/{id}": {
      "get": {
        "operationId": "UsuariosController_obtener",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del usuario",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "404": {
            "description": "Usuario inexistente o sin permiso"
          }
        },
        "summary": "Perfil de un usuario (admin o el propio)",
        "tags": [
          "Usuarios"
        ]
      }
    },
    "/api/v1/orders/{id}/estado": {
      "patch": {
        "operationId": "EstadoOrdenController_avanzar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AvanzarEstadoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Orden con el nuevo estado"
          },
          "400": {
            "description": "Estado objetivo inválido (ESTADO_INVALIDO)"
          }
        },
        "summary": "Avanzar el estado de una orden (logística)",
        "tags": [
          "Logística"
        ]
      }
    },
    "/api/v1/admin/envios": {
      "get": {
        "operationId": "EnviosAdminController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Guías de despacho con monto serializado"
          }
        },
        "summary": "Listar guías de despacho (admin)",
        "tags": [
          "Logística"
        ]
      }
    },
    "/api/v1/inteligencia/mapa-calor": {
      "get": {
        "operationId": "MetricasController_getMapaCalor",
        "parameters": [
          {
            "name": "desde",
            "required": false,
            "in": "query",
            "description": "Fecha desde (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": false,
            "in": "query",
            "description": "Fecha hasta (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "sku",
            "required": false,
            "in": "query",
            "description": "SKU",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "vendedor_id",
            "required": false,
            "in": "query",
            "description": "Id del vendedor (UUID)",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Mapa de calor de ventas (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/me/mapa-calor": {
      "get": {
        "operationId": "MetricasController_getMiMapaCalor",
        "parameters": [
          {
            "name": "x-user-personal",
            "required": false,
            "in": "header",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "desde",
            "required": false,
            "in": "query",
            "description": "Fecha desde (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": false,
            "in": "query",
            "description": "Fecha hasta (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "sku",
            "required": false,
            "in": "query",
            "description": "SKU",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "vendedor_id",
            "required": false,
            "in": "query",
            "description": "Id del vendedor (UUID)",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Mi mapa de calor (vendedor)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/rendimiento": {
      "get": {
        "operationId": "MetricasController_getRendimiento",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Rendimiento de vendedores (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/me/rendimiento": {
      "get": {
        "operationId": "MetricasController_getMiRendimiento",
        "parameters": [
          {
            "name": "x-user-personal",
            "required": false,
            "in": "header",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Mi rendimiento como vendedor",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/cobertura": {
      "get": {
        "operationId": "MetricasController_getCobertura",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Cobertura por zona (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/demanda": {
      "get": {
        "operationId": "MetricasController_getDemanda",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Demanda de productos (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/tendencias": {
      "get": {
        "operationId": "MetricasController_getTendencias",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Tendencias (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/resumen": {
      "get": {
        "operationId": "MetricasController_getResumen",
        "parameters": [
          {
            "name": "desde",
            "required": false,
            "in": "query",
            "description": "Fecha desde (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "hasta",
            "required": false,
            "in": "query",
            "description": "Fecha hasta (ISO 8601)",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "sku",
            "required": false,
            "in": "query",
            "description": "SKU",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "vendedor_id",
            "required": false,
            "in": "query",
            "description": "Id del vendedor (UUID)",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Resumen de inteligencia de mercado (admin)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/ventas/registrar": {
      "post": {
        "operationId": "MetricasController_registrarVenta",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Registrar una venta en inteligencia (ingesta manual)",
        "tags": [
          "Inteligencia — Métricas"
        ]
      }
    },
    "/api/v1/inteligencia/forecast/demanda": {
      "get": {
        "operationId": "PrediccionController_forecastDemanda",
        "parameters": [
          {
            "name": "sku",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "dias",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Forecast de demanda por SKU (media móvil 7d)",
        "tags": [
          "Inteligencia — Predicción"
        ]
      }
    },
    "/api/v1/inteligencia/anomalias": {
      "get": {
        "operationId": "PrediccionController_anomalias",
        "parameters": [
          {
            "name": "sku",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "vendedor_id",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "limite",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Detección de ventas anómalas por Z-score",
        "tags": [
          "Inteligencia — Predicción"
        ]
      }
    },
    "/api/v1/inteligencia/vendedores/{id}/score": {
      "get": {
        "operationId": "PrediccionController_scoreVendedor",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Score predictivo de un vendedor (churn, efectividad)",
        "tags": [
          "Inteligencia — Predicción"
        ]
      }
    },
    "/api/v1/inteligencia/me/score": {
      "get": {
        "operationId": "PrediccionController_miScore",
        "parameters": [
          {
            "name": "x-user-id",
            "required": true,
            "in": "header",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Mi score predictivo como vendedor",
        "tags": [
          "Inteligencia — Predicción"
        ]
      }
    },
    "/api/v1/inteligencia/calidad": {
      "get": {
        "operationId": "PrediccionController_calidadDatos",
        "parameters": [
          {
            "name": "horas",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Dashboard de calidad de datos del pipeline (últimas N horas)",
        "tags": [
          "Inteligencia — Predicción"
        ]
      }
    },
    "/internal/orders/{id}/transicion": {
      "post": {
        "operationId": "InternoController_transicion",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": true,
            "in": "query",
            "description": "Estado objetivo",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "motivo",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Avanzar el ciclo de vida de la orden (logística→orders)",
        "tags": [
          "Pedidos (interno)"
        ]
      }
    },
    "/internal/orders/{id}": {
      "get": {
        "operationId": "InternoController_orden",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Lectura completa de una orden (reportes/comisiones)",
        "tags": [
          "Pedidos (interno)"
        ]
      }
    },
    "/internal/orders": {
      "get": {
        "operationId": "InternoController_listar",
        "parameters": [
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar órdenes (servicio→servicio)",
        "tags": [
          "Pedidos (interno)"
        ]
      }
    },
    "/internal/orders/{id}/reproyectar": {
      "post": {
        "operationId": "InternoController_reproyectar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Reproyectar una orden desde su historia (Event Sourcing)",
        "tags": [
          "Pedidos (interno)"
        ]
      }
    },
    "/internal/orders/{id}/historia": {
      "get": {
        "operationId": "InternoController_historia",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Historia inmutable de eventos de una orden",
        "tags": [
          "Pedidos (interno)"
        ]
      }
    },
    "/api/v1/orders": {
      "post": {
        "operationId": "PedidosController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CreateOrderCommand"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear una orden desde el carrito (comprador)",
        "tags": [
          "Pedidos"
        ]
      },
      "get": {
        "operationId": "PedidosController_misOrdenes",
        "parameters": [
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Historial de órdenes del comprador autenticado",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/admin/todas": {
      "get": {
        "operationId": "PedidosController_todas",
        "parameters": [
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "schema": {
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar todas las órdenes (admin)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/vendedor/mis-ventas": {
      "get": {
        "operationId": "PedidosController_misVentas",
        "parameters": [
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Órdenes del vendedor autenticado (mis ventas)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/vendedor/comisiones": {
      "get": {
        "operationId": "PedidosController_comisiones",
        "parameters": [
          {
            "name": "periodo",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "vendedorId",
            "required": false,
            "in": "query",
            "description": "Solo admin",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Comisiones por periodo (vendedor/admin)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/tienda/{tiendaId}": {
      "get": {
        "operationId": "PedidosController_porTienda",
        "parameters": [
          {
            "name": "tiendaId",
            "required": true,
            "in": "path",
            "description": "Id de la tienda",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Órdenes de una tienda (vendedor dueño o admin)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/{id}": {
      "get": {
        "operationId": "PedidosController_detalle",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Estado/detalle de una orden (dueño o admin)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/{id}/timeline": {
      "get": {
        "operationId": "PedidosController_timeline",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la orden",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Timeline de eventos de la orden (Event Sourcing)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/carrito": {
      "get": {
        "operationId": "CarritoController_ver",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Ver el carrito del comprador (RN-05)",
        "tags": [
          "Carrito"
        ]
      },
      "delete": {
        "operationId": "CarritoController_vaciar",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Vaciar el carrito",
        "tags": [
          "Carrito"
        ]
      }
    },
    "/api/v1/carrito/items": {
      "post": {
        "operationId": "CarritoController_agregar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AgregarItemRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Agregar un item al carrito",
        "tags": [
          "Carrito"
        ]
      }
    },
    "/api/v1/carrito/items/{ofertaId}": {
      "patch": {
        "operationId": "CarritoController_actualizar",
        "parameters": [
          {
            "name": "ofertaId",
            "required": true,
            "in": "path",
            "description": "Id de la oferta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarCantidadRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Actualizar la cantidad de un item (0 lo elimina)",
        "tags": [
          "Carrito"
        ]
      },
      "delete": {
        "operationId": "CarritoController_quitar",
        "parameters": [
          {
            "name": "ofertaId",
            "required": true,
            "in": "path",
            "description": "Id de la oferta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Quitar un item del carrito",
        "tags": [
          "Carrito"
        ]
      }
    },
    "/internal/ofertas": {
      "get": {
        "operationId": "InternalController_porIds",
        "parameters": [
          {
            "name": "ids",
            "required": false,
            "in": "query",
            "description": "Ids de oferta separados por coma",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Lote de ofertas por ids (servicio→servicio)",
        "tags": [
          "Tiendas (interno)"
        ]
      }
    },
    "/api/v1/vendedores/tienda": {
      "post": {
        "operationId": "TiendasController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearTiendaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": ""
          }
        },
        "summary": "Crear la tienda del vendedor autenticado",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/vendedores/me/tienda": {
      "get": {
        "operationId": "TiendasController_miTienda",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Tienda del vendedor autenticado",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/vendedores/productos": {
      "post": {
        "operationId": "OfertasController_publicar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/PublicarProductoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Oferta creada"
          }
        },
        "summary": "Publicar un producto con margen (vendedor)",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/vendedores/productos/{id}": {
      "patch": {
        "operationId": "OfertasController_cambiarMargen",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CambiarMargenRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Cambiar el margen de una oferta (vendedor)",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/vendedores/me/ofertas": {
      "get": {
        "operationId": "OfertasController_misOfertas",
        "parameters": [],
        "responses": {
          "200": {
            "description": ""
          }
        },
        "summary": "Listar las ofertas del vendedor autenticado",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/tiendas/{id}": {
      "get": {
        "operationId": "OfertasController_tiendaPublica",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "404": {
            "description": "Tienda inexistente (NO_ENCONTRADO)"
          }
        },
        "summary": "Tienda pública con sus ofertas (ruta pública)",
        "tags": [
          "Tiendas"
        ]
      }
    }
  },
  "components": {
    "securitySchemes": {
      "bearer": {
        "scheme": "bearer",
        "bearerFormat": "JWT",
        "type": "http"
      }
    },
    "schemas": {
      "CrearProductoRequestDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "pattern": "^[A-Za-z0-9-]{2,32}$",
            "example": "SKU-001"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del producto",
            "minLength": 2,
            "maxLength": 200
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción"
          },
          "categoria": {
            "type": "string",
            "description": "Categoría"
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base decimal (ej: 1000.00) en córdobas; se guarda en centavos",
            "example": "1000.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock inicial",
            "minimum": 0,
            "example": 10
          }
        },
        "required": [
          "sku",
          "nombre",
          "precio_base",
          "stock"
        ]
      },
      "ActualizarProductoRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre del producto",
            "minLength": 2,
            "maxLength": 200
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción"
          },
          "categoria": {
            "type": "string",
            "description": "Categoría"
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base decimal (ej: 1000.00)"
          },
          "stock": {
            "type": "number",
            "description": "Stock",
            "minimum": 0
          },
          "motivo": {
            "type": "string",
            "description": "Motivo del cambio (RN-08 histórico)"
          }
        }
      },
      "CrearPersonalRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "apellido": {
            "type": "string"
          },
          "cargo": {
            "type": "string",
            "enum": [
              "conductor",
              "auxiliar",
              "supervisor",
              "coordinador"
            ]
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "en_ruta",
              "descansando",
              "inactivo"
            ]
          },
          "telefono": {
            "type": "string"
          },
          "email": {
            "type": "string"
          },
          "rutaAsignadaId": {
            "type": "string"
          },
          "vehiculoAsignadoId": {
            "type": "string"
          },
          "horaCheckIn": {
            "type": "string"
          },
          "horaCheckOut": {
            "type": "string"
          },
          "ubicacionLat": {
            "type": "number"
          },
          "ubicacionLng": {
            "type": "number"
          },
          "ubicacionPrecision": {
            "type": "number"
          },
          "ubicacionTs": {
            "type": "string"
          }
        },
        "required": [
          "nombre"
        ]
      },
      "ActualizarPersonalRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "apellido": {
            "type": "string"
          },
          "cargo": {
            "type": "string",
            "enum": [
              "conductor",
              "auxiliar",
              "supervisor",
              "coordinador"
            ]
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "en_ruta",
              "descansando",
              "inactivo"
            ]
          },
          "telefono": {
            "type": "string"
          },
          "email": {
            "type": "string"
          },
          "rutaAsignadaId": {
            "type": "string"
          },
          "vehiculoAsignadoId": {
            "type": "string"
          },
          "horaCheckIn": {
            "type": "string"
          },
          "horaCheckOut": {
            "type": "string"
          },
          "ubicacionLat": {
            "type": "number"
          },
          "ubicacionLng": {
            "type": "number"
          },
          "ubicacionPrecision": {
            "type": "number"
          },
          "ubicacionTs": {
            "type": "string"
          }
        }
      },
      "UbicacionRequestDto": {
        "type": "object",
        "properties": {
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "precision": {
            "type": "number"
          },
          "timestamp": {
            "type": "string"
          }
        },
        "required": [
          "lat",
          "lng"
        ]
      },
      "CrearClienteRequestDto": {
        "type": "object",
        "properties": {
          "nombreCompleto": {
            "type": "string"
          },
          "tipoDocumento": {
            "type": "string"
          },
          "numeroDocumento": {
            "type": "string"
          },
          "tipoCliente": {
            "type": "string",
            "enum": [
              "particular",
              "minorista",
              "mayorista",
              "corporativo"
            ]
          },
          "email": {
            "type": "string"
          },
          "telefono": {
            "type": "string"
          },
          "telefonoAlternativo": {
            "type": "string"
          },
          "direccionPrincipal": {
            "type": "string"
          },
          "direccionSecundaria": {
            "type": "string"
          },
          "referenciasDireccion": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "notasAdicionales": {
            "type": "string"
          },
          "urlGoogleMaps": {
            "type": "string"
          }
        },
        "required": [
          "nombreCompleto"
        ]
      },
      "ActualizarClienteRequestDto": {
        "type": "object",
        "properties": {
          "nombreCompleto": {
            "type": "string"
          },
          "tipoDocumento": {
            "type": "string"
          },
          "numeroDocumento": {
            "type": "string"
          },
          "tipoCliente": {
            "type": "string",
            "enum": [
              "particular",
              "minorista",
              "mayorista",
              "corporativo"
            ]
          },
          "email": {
            "type": "string"
          },
          "telefono": {
            "type": "string"
          },
          "telefonoAlternativo": {
            "type": "string"
          },
          "direccionPrincipal": {
            "type": "string"
          },
          "direccionSecundaria": {
            "type": "string"
          },
          "referenciasDireccion": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "notasAdicionales": {
            "type": "string"
          },
          "urlGoogleMaps": {
            "type": "string"
          }
        }
      },
      "CrearVehiculoRequestDto": {
        "type": "object",
        "properties": {
          "placa": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "camioneta",
              "furgon",
              "camion",
              "moto",
              "otro"
            ]
          },
          "marca": {
            "type": "string"
          },
          "modelo": {
            "type": "string"
          },
          "anio": {
            "type": "number",
            "minimum": 1900
          },
          "estado": {
            "type": "string",
            "enum": [
              "disponible",
              "en_ruta",
              "mantenimiento",
              "fuera_de_servicio"
            ]
          },
          "color": {
            "type": "string"
          },
          "capacidadCargaKg": {
            "type": "number"
          },
          "numeroChasis": {
            "type": "string"
          },
          "numeroMotor": {
            "type": "string"
          },
          "tipoCombustible": {
            "type": "string"
          },
          "vencimientoSeguro": {
            "type": "string"
          },
          "vencimientoCirculacion": {
            "type": "string"
          },
          "notasAdicionales": {
            "type": "string"
          },
          "conductorId": {
            "type": "string"
          },
          "rutaActivaId": {
            "type": "string"
          }
        },
        "required": [
          "placa"
        ]
      },
      "ActualizarVehiculoRequestDto": {
        "type": "object",
        "properties": {
          "placa": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "camioneta",
              "furgon",
              "camion",
              "moto",
              "otro"
            ]
          },
          "marca": {
            "type": "string"
          },
          "modelo": {
            "type": "string"
          },
          "anio": {
            "type": "number",
            "minimum": 1900
          },
          "estado": {
            "type": "string",
            "enum": [
              "disponible",
              "en_ruta",
              "mantenimiento",
              "fuera_de_servicio"
            ]
          },
          "color": {
            "type": "string"
          },
          "capacidadCargaKg": {
            "type": "number"
          },
          "numeroChasis": {
            "type": "string"
          },
          "numeroMotor": {
            "type": "string"
          },
          "tipoCombustible": {
            "type": "string"
          },
          "vencimientoSeguro": {
            "type": "string"
          },
          "vencimientoCirculacion": {
            "type": "string"
          },
          "notasAdicionales": {
            "type": "string"
          },
          "conductorId": {
            "type": "string"
          },
          "rutaActivaId": {
            "type": "string"
          }
        }
      },
      "CrearRutaRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "estado": {
            "type": "string",
            "enum": [
              "pendiente",
              "en_curso",
              "completada",
              "cancelada"
            ]
          },
          "personalIds": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "vehiculoAsignadoId": {
            "type": "string"
          },
          "fechaInicio": {
            "type": "string"
          },
          "fechaFin": {
            "type": "string"
          }
        },
        "required": [
          "nombre"
        ]
      },
      "ActualizarRutaRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "estado": {
            "type": "string",
            "enum": [
              "pendiente",
              "en_curso",
              "completada",
              "cancelada"
            ]
          },
          "personalIds": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "vehiculoAsignadoId": {
            "type": "string"
          },
          "fechaInicio": {
            "type": "string"
          },
          "fechaFin": {
            "type": "string"
          }
        }
      },
      "ParadaRequestDto": {
        "type": "object",
        "properties": {
          "rutaId": {
            "type": "string"
          },
          "orden": {
            "type": "number",
            "minimum": 0
          },
          "nombre": {
            "type": "string"
          },
          "direccion": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "completada": {
            "type": "boolean"
          },
          "pedidoId": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "cliente",
              "logistica"
            ]
          },
          "clienteId": {
            "type": "string"
          },
          "telefono": {
            "type": "string"
          },
          "referencias": {
            "type": "string"
          },
          "horarioAtencion": {
            "type": "string"
          },
          "notas": {
            "type": "string"
          },
          "tipoCombustible": {
            "type": "string"
          }
        },
        "required": [
          "nombre"
        ]
      },
      "AsignarRutaRequestDto": {
        "type": "object",
        "properties": {
          "personalIds": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "vehiculoId": {
            "type": "string"
          }
        }
      },
      "CrearPedidoRequestDto": {
        "type": "object",
        "properties": {
          "cliente": {
            "type": "string"
          },
          "clienteId": {
            "type": "string"
          },
          "direccionEntrega": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "estado": {
            "type": "string",
            "enum": [
              "pendiente",
              "asignado",
              "en_camino",
              "entregado",
              "fallido",
              "cancelado"
            ]
          },
          "rutaId": {
            "type": "string"
          },
          "notas": {
            "type": "string"
          }
        }
      },
      "ActualizarPedidoRequestDto": {
        "type": "object",
        "properties": {
          "cliente": {
            "type": "string"
          },
          "clienteId": {
            "type": "string"
          },
          "direccionEntrega": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "estado": {
            "type": "string",
            "enum": [
              "pendiente",
              "asignado",
              "en_camino",
              "entregado",
              "fallido",
              "cancelado"
            ]
          },
          "rutaId": {
            "type": "string"
          },
          "notas": {
            "type": "string"
          }
        }
      },
      "CambiarEstadoPedidoRequestDto": {
        "type": "object",
        "properties": {
          "estado": {
            "type": "string",
            "enum": [
              "pendiente",
              "asignado",
              "en_camino",
              "entregado",
              "fallido",
              "cancelado"
            ]
          },
          "motivo": {
            "type": "string"
          }
        },
        "required": [
          "estado"
        ]
      },
      "CrearAsistenciaRequestDto": {
        "type": "object",
        "properties": {
          "personalId": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "entrada",
              "salida"
            ]
          },
          "timestamp": {
            "type": "string"
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "estadoPuntualidad": {
            "type": "string",
            "enum": [
              "puntual",
              "tardanza",
              "salida_anticipada",
              "salida_tardia"
            ]
          },
          "enSede": {
            "type": "boolean"
          },
          "horaRealLlegada": {
            "type": "string"
          },
          "minutosRetrasoReal": {
            "type": "number"
          },
          "monitoreoActivo": {
            "type": "boolean"
          },
          "notas": {
            "type": "string"
          },
          "justificacion": {
            "type": "string"
          },
          "fotosJustificacion": {
            "type": "array",
            "items": {
              "type": "string"
            }
          }
        },
        "required": [
          "personalId",
          "tipo",
          "estadoPuntualidad"
        ]
      },
      "CrearIncidenciaRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "enum": [
              "mecanica",
              "accidente",
              "salud",
              "clima",
              "otro"
            ]
          },
          "estado": {
            "type": "string",
            "enum": [
              "abierta",
              "en_progreso",
              "resuelta"
            ]
          },
          "descripcion": {
            "type": "string"
          },
          "personalId": {
            "type": "string"
          },
          "vehiculoId": {
            "type": "string"
          },
          "rutaId": {
            "type": "string"
          },
          "resolucion": {
            "type": "string"
          }
        },
        "required": [
          "tipo",
          "descripcion"
        ]
      },
      "ActualizarIncidenciaRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "enum": [
              "mecanica",
              "accidente",
              "salud",
              "clima",
              "otro"
            ]
          },
          "estado": {
            "type": "string",
            "enum": [
              "abierta",
              "en_progreso",
              "resuelta"
            ]
          },
          "descripcion": {
            "type": "string"
          },
          "personalId": {
            "type": "string"
          },
          "vehiculoId": {
            "type": "string"
          },
          "rutaId": {
            "type": "string"
          },
          "resolucion": {
            "type": "string"
          }
        }
      },
      "CrearTrackingRequestDto": {
        "type": "object",
        "properties": {
          "personalId": {
            "type": "string"
          },
          "latitud": {
            "type": "number"
          },
          "longitud": {
            "type": "number"
          },
          "precision": {
            "type": "number"
          },
          "velocidad": {
            "type": "number"
          },
          "rumbo": {
            "type": "number"
          },
          "timestamp": {
            "type": "string"
          }
        },
        "required": [
          "personalId",
          "latitud",
          "longitud"
        ]
      },
      "TrackingBulkRequestDto": {
        "type": "object",
        "properties": {
          "registros": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/CrearTrackingRequestDto"
            }
          }
        },
        "required": [
          "registros"
        ]
      },
      "CrearVisitaRequestDto": {
        "type": "object",
        "properties": {
          "personalId": {
            "type": "string"
          },
          "clienteId": {
            "type": "string"
          },
          "rutaId": {
            "type": "string"
          },
          "tipoActividad": {
            "type": "string",
            "enum": [
              "impulsacion",
              "venta",
              "entrega",
              "reparto"
            ]
          },
          "resultado": {
            "type": "string",
            "enum": [
              "visitado",
              "cerca_no_visitado",
              "no_visitado"
            ]
          },
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "distanciaAlCliente": {
            "type": "number"
          },
          "horaLlegada": {
            "type": "string"
          },
          "horaSalida": {
            "type": "string"
          },
          "duracionMinutos": {
            "type": "number"
          },
          "notas": {
            "type": "string"
          }
        },
        "required": [
          "personalId",
          "tipoActividad"
        ]
      },
      "GuardarCumplimientoRequestDto": {
        "type": "object",
        "properties": {
          "rutaId": {
            "type": "string"
          },
          "fecha": {
            "type": "string"
          },
          "metricas": {
            "type": "object"
          }
        },
        "required": [
          "rutaId",
          "fecha",
          "metricas"
        ]
      },
      "SyncOperacionRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string"
          },
          "payload": {
            "type": "object"
          }
        },
        "required": [
          "tipo",
          "payload"
        ]
      },
      "SyncRequestDto": {
        "type": "object",
        "properties": {
          "operaciones": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/SyncOperacionRequestDto"
            }
          }
        },
        "required": [
          "operaciones"
        ]
      },
      "CrearCuentaRequestDto": {
        "type": "object",
        "properties": {
          "codigo": {
            "type": "string",
            "description": "Código de la cuenta"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre de la cuenta",
            "minLength": 3
          },
          "tipo": {
            "type": "string",
            "description": "Tipo de cuenta",
            "enum": [
              "ACTIVO",
              "PASIVO",
              "CAPITAL",
              "INGRESO",
              "COSTO",
              "GASTO"
            ]
          },
          "naturaleza": {
            "type": "string",
            "description": "Naturaleza de la cuenta",
            "enum": [
              "DEUDORA",
              "ACREEDORA"
            ]
          },
          "nivel": {
            "type": "number",
            "description": "Nivel jerárquico"
          }
        },
        "required": [
          "codigo",
          "nombre",
          "tipo",
          "naturaleza"
        ]
      },
      "EstadoCuentaRequestDto": {
        "type": "object",
        "properties": {
          "estado": {
            "type": "string",
            "description": "Estado de la cuenta",
            "enum": [
              "activa",
              "inactiva"
            ]
          }
        },
        "required": [
          "estado"
        ]
      },
      "DetalleAsientoRequestDto": {
        "type": "object",
        "properties": {
          "cuenta_codigo": {
            "type": "string",
            "description": "Código de cuenta contable"
          },
          "debe_cents": {
            "type": "number",
            "description": "Débito en centavos",
            "minimum": 0
          },
          "haber_cents": {
            "type": "number",
            "description": "Crédito en centavos",
            "minimum": 0
          },
          "concepto": {
            "type": "string",
            "description": "Concepto del detalle"
          },
          "orden": {
            "type": "number",
            "description": "Orden del detalle"
          }
        },
        "required": [
          "cuenta_codigo",
          "debe_cents",
          "haber_cents"
        ]
      },
      "CrearAsientoRequestDto": {
        "type": "object",
        "properties": {
          "concepto": {
            "type": "string",
            "description": "Concepto del asiento",
            "minLength": 3
          },
          "fecha": {
            "type": "string",
            "description": "Fecha (ISO 8601)"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo de asiento",
            "enum": [
              "INGRESO",
              "EGRESO",
              "AJUSTE",
              "CIERRE",
              "APERTURA",
              "MANUAL"
            ]
          },
          "referencia_tipo": {
            "type": "string",
            "description": "Tipo de referencia"
          },
          "referencia_id": {
            "type": "string",
            "description": "Id de referencia"
          },
          "detalles": {
            "description": "Detalles del asiento (mínimo 2)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/DetalleAsientoRequestDto"
            }
          }
        },
        "required": [
          "concepto",
          "detalles"
        ]
      },
      "EmitirComprobanteRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "description": "Tipo de comprobante",
            "enum": [
              "FACTURA",
              "NOTA_CREDITO"
            ]
          },
          "orden_id": {
            "type": "string",
            "description": "Id de la orden (UUID)"
          },
          "cliente_id": {
            "type": "string",
            "description": "Id del cliente"
          },
          "razon_social": {
            "type": "string",
            "description": "Razón social"
          },
          "ruc": {
            "type": "string",
            "description": "RUC"
          }
        },
        "required": [
          "tipo",
          "orden_id"
        ]
      },
      "CrearJurisdiccionRequestDto": {
        "type": "object",
        "properties": {
          "codigo_pais": {
            "type": "string",
            "description": "Código de país (ISO)",
            "minLength": 2
          },
          "nombre": {
            "type": "string",
            "description": "Nombre de la jurisdicción"
          },
          "moneda": {
            "type": "string",
            "description": "Moneda"
          },
          "simbolo_moneda": {
            "type": "string",
            "description": "Símbolo de la moneda"
          },
          "tasa_iva_por_mil": {
            "type": "number",
            "description": "Tasa de IVA por mil",
            "minimum": 0
          },
          "tasa_ir_por_mil": {
            "type": "number",
            "description": "Tasa de IR por mil",
            "minimum": 0
          },
          "periodicidad_declaracion": {
            "type": "string",
            "description": "Periodicidad de declaración"
          },
          "leyes": {
            "type": "object",
            "description": "Leyes aplicables (clave→texto)"
          }
        },
        "required": [
          "codigo_pais",
          "nombre",
          "moneda",
          "simbolo_moneda",
          "tasa_iva_por_mil",
          "tasa_ir_por_mil"
        ]
      },
      "CrearRegimenRequestDto": {
        "type": "object",
        "properties": {
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción",
            "minLength": 2
          },
          "codigo": {
            "type": "string",
            "description": "Código del régimen"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del régimen"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción"
          },
          "periodicidad": {
            "type": "string",
            "description": "Periodicidad"
          },
          "condicion_ingresos_anuales_cents": {
            "type": "number",
            "description": "Condición de ingresos anuales (centavos)"
          }
        },
        "required": [
          "jurisdiccion",
          "codigo",
          "nombre"
        ]
      },
      "RegistrarSujetoRequestDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del sujeto (UUID)"
          },
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción"
          },
          "regimen_id": {
            "type": "string",
            "description": "Id del régimen (UUID)"
          },
          "razon_social": {
            "type": "string",
            "description": "Razón social"
          },
          "ruc": {
            "type": "string",
            "description": "RUC"
          },
          "es_plataforma": {
            "type": "boolean",
            "description": "¿Es la plataforma?"
          }
        },
        "required": [
          "id",
          "razon_social"
        ]
      },
      "GenerarDeclaracionesRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "description": "Tipo de declaración",
            "enum": [
              "IVA",
              "IR",
              "CUOTA_FIJA"
            ]
          },
          "inicio": {
            "type": "string",
            "description": "Inicio del periodo"
          },
          "fin": {
            "type": "string",
            "description": "Fin del periodo"
          }
        }
      },
      "SupuestosRequestDto": {
        "type": "object",
        "properties": {
          "horizonte_meses": {
            "type": "number",
            "description": "Horizonte en meses (1-60)",
            "minimum": 1,
            "maximum": 60
          },
          "vendedores_iniciales": {
            "type": "number",
            "description": "Vendedores iniciales",
            "minimum": 1
          },
          "entrada_vendedores_mes": {
            "type": "number",
            "description": "Entrada de vendedores por mes",
            "minimum": 0
          },
          "churn_tasa": {
            "type": "number",
            "description": "Tasa de churn (0-1)",
            "minimum": 0,
            "maximum": 1
          },
          "pedidos_por_vendedor": {
            "description": "Pedidos por vendedor por mes",
            "type": "array",
            "items": {
              "type": "number"
            }
          },
          "ticket_promedio_cents": {
            "type": "number",
            "description": "Ticket promedio en centavos",
            "minimum": 1
          },
          "comision_tasa": {
            "type": "number",
            "description": "Tasa de comisión (0-1)",
            "minimum": 0,
            "maximum": 1
          },
          "costos_fijos_cents": {
            "type": "number",
            "description": "Costos fijos en centavos",
            "minimum": 0
          },
          "costos_fijos_desde_mes7_cents": {
            "type": "number",
            "description": "Costos fijos desde el mes 7 en centavos",
            "minimum": 0
          },
          "inversion_inicial_cents": {
            "type": "number",
            "description": "Inversión inicial en centavos",
            "minimum": 0
          },
          "tasa_descuento_mensual": {
            "type": "number",
            "description": "Tasa de descuento mensual",
            "minimum": 0
          }
        },
        "required": [
          "horizonte_meses",
          "vendedores_iniciales",
          "entrada_vendedores_mes",
          "churn_tasa",
          "pedidos_por_vendedor",
          "ticket_promedio_cents",
          "comision_tasa",
          "costos_fijos_cents",
          "inversion_inicial_cents"
        ]
      },
      "CrearProyeccionRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre de la proyección",
            "minLength": 3
          },
          "supuestos": {
            "description": "Supuestos del modelo",
            "allOf": [
              {
                "$ref": "#/components/schemas/SupuestosRequestDto"
              }
            ]
          }
        },
        "required": [
          "nombre",
          "supuestos"
        ]
      },
      "RegistroRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre completo",
            "minLength": 2,
            "maxLength": 120,
            "example": "Ana Pérez"
          },
          "correo": {
            "type": "string",
            "description": "Correo electrónico",
            "example": "ana@tienda.test"
          },
          "contrasena": {
            "type": "string",
            "description": "Contraseña (mínimo 8 caracteres)",
            "minLength": 8
          },
          "rol": {
            "type": "string",
            "description": "Rol registrable",
            "enum": [
              "vendedor",
              "comprador"
            ],
            "example": "comprador"
          }
        },
        "required": [
          "nombre",
          "correo",
          "contrasena",
          "rol"
        ]
      },
      "LoginRequestDto": {
        "type": "object",
        "properties": {
          "correo": {
            "type": "string",
            "description": "Correo electrónico",
            "example": "ana@tienda.test"
          },
          "contrasena": {
            "type": "string",
            "description": "Contraseña",
            "example": "secreto123"
          }
        },
        "required": [
          "correo",
          "contrasena"
        ]
      },
      "CrearUsuarioRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre completo",
            "minLength": 2,
            "maxLength": 120
          },
          "correo": {
            "type": "string",
            "description": "Correo electrónico"
          },
          "contrasena": {
            "type": "string",
            "description": "Contraseña (mínimo 8 caracteres)",
            "minLength": 8
          },
          "rol": {
            "type": "string",
            "description": "Rol del usuario",
            "enum": [
              "vendedor",
              "comprador",
              "admin",
              "logistica",
              "coordinador",
              "supervisor",
              "operativo"
            ]
          },
          "tenant_id": {
            "type": "string",
            "description": "Tenant (organización); obligatorio para roles de logística"
          },
          "personal_id": {
            "type": "string",
            "description": "Id de la ficha de personal en field-service"
          }
        },
        "required": [
          "nombre",
          "correo",
          "contrasena",
          "rol"
        ]
      },
      "CambiarContrasenaRequestDto": {
        "type": "object",
        "properties": {
          "actual": {
            "type": "string",
            "description": "Contraseña actual"
          },
          "nueva": {
            "type": "string",
            "description": "Contraseña nueva (mínimo 8 caracteres)",
            "minLength": 8
          }
        },
        "required": [
          "actual",
          "nueva"
        ]
      },
      "RestablecerContrasenaRequestDto": {
        "type": "object",
        "properties": {
          "correo": {
            "type": "string",
            "description": "Correo electrónico de la cuenta"
          }
        },
        "required": [
          "correo"
        ]
      },
      "VincularPersonalRequestDto": {
        "type": "object",
        "properties": {
          "personal_id": {
            "type": "string",
            "description": "Id de la ficha de personal en field-service",
            "minLength": 1,
            "maxLength": 100
          },
          "nombre": {
            "type": "string",
            "description": "Nombre a reflejar en el perfil",
            "maxLength": 120
          }
        },
        "required": [
          "personal_id"
        ]
      },
      "AvanzarEstadoRequestDto": {
        "type": "object",
        "properties": {
          "estado": {
            "type": "string",
            "description": "Estado objetivo del ciclo de vida de la orden",
            "enum": [
              "en_preparacion",
              "enviada",
              "entregada",
              "cancelada",
              "devuelta"
            ],
            "example": "enviada"
          },
          "motivo": {
            "type": "string",
            "description": "Motivo del cambio de estado",
            "example": "despacho a ruta 3"
          }
        },
        "required": [
          "estado"
        ]
      },
      "ItemOrdenRequestDto": {
        "type": "object",
        "properties": {
          "oferta_id": {
            "type": "string",
            "description": "Id de la oferta (vendedor-producto)"
          },
          "cantidad": {
            "type": "number",
            "description": "Cantidad (1-99)",
            "minimum": 1,
            "maximum": 99,
            "example": 1
          }
        },
        "required": [
          "oferta_id",
          "cantidad"
        ]
      },
      "CreateOrderCommand": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Artículos de la orden",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ItemOrdenRequestDto"
            }
          },
          "usar_carrito": {
            "type": "boolean",
            "description": "Crear la orden desde el carrito (RN-05)",
            "example": false
          }
        }
      },
      "AgregarItemRequestDto": {
        "type": "object",
        "properties": {
          "oferta_id": {
            "type": "string",
            "description": "Id de la oferta (vendedor-producto)",
            "example": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"
          },
          "cantidad": {
            "type": "number",
            "description": "Cantidad (1-99)",
            "minimum": 1,
            "maximum": 99,
            "example": 2
          }
        },
        "required": [
          "oferta_id",
          "cantidad"
        ]
      },
      "ActualizarCantidadRequestDto": {
        "type": "object",
        "properties": {
          "cantidad": {
            "type": "number",
            "description": "Cantidad (0 elimina el item; 1-99)",
            "minimum": 0,
            "maximum": 99,
            "example": 3
          }
        },
        "required": [
          "cantidad"
        ]
      },
      "CrearTiendaRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre de la tienda",
            "minLength": 2,
            "maxLength": 100,
            "example": "Mi tienda"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción de la tienda",
            "example": "Ropa y accesorios"
          }
        },
        "required": [
          "nombre"
        ]
      },
      "PublicarProductoRequestDto": {
        "type": "object",
        "properties": {
          "producto_id": {
            "type": "string",
            "description": "Id del producto a publicar",
            "example": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"
          },
          "margen": {
            "type": "number",
            "description": "Margen en enteros (15 = 15 %). RN-01: 0 a 90",
            "minimum": 0,
            "maximum": 90,
            "example": 15
          }
        },
        "required": [
          "producto_id",
          "margen"
        ]
      },
      "CambiarMargenRequestDto": {
        "type": "object",
        "properties": {
          "margen": {
            "type": "number",
            "description": "Nuevo margen en enteros (0 a 90)",
            "minimum": 0,
            "maximum": 90,
            "example": 20
          }
        },
        "required": [
          "margen"
        ]
      }
    }
  }
} as unknown as OpenAPIObject;
