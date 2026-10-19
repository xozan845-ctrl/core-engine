/**
 * GENERADO por scripts/tools/swagger-aggregate.mjs — NO editar a mano.
 * Fuente: docs/openapi/*.json (npm run swagger:export). Verifica: gate G-8.
 */
import type { OpenAPIObject } from '@nestjs/swagger';

export const OPENAPI_AGREGADO = {
  "openapi": "3.0.0",
  "info": {
    "title": "Core Engine · API pública (agregada)",
    "description": "Contrato de la superficie pública (/api/v1/**) expuesta por el API Gateway. No incluye las rutas /internal/**, que son exclusivas del tráfico servicio→servicio.",
    "version": "v1"
  },
  "tags": [],
  "paths": {
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
            "description": "Página de productos",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaProductosResponseDto"
                }
              }
            }
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
            "description": "Producto creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProductoResponseDto"
                }
              }
            }
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
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
          "200": {
            "description": "Producto encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProductoResponseDto"
                }
              }
            }
          },
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
            "description": "Producto actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProductoResponseDto"
                }
              }
            }
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
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
            "description": "Líneas de inventario",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LineaInventarioResponseDto"
                  }
                }
              }
            }
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Sincronizar operaciones offline",
        "tags": [
          "Field (logística de campo)"
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Listar proyecciones",
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Plan a 24 meses (8.6 + 8.9)",
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Detalle de una proyección",
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Situación fiscal del vendedor (cap. 4.4)",
        "tags": [
          "Finanzas · Vendedor"
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
    "/api/v1/auth/logout": {
      "post": {
        "operationId": "AuthController_logout",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/LogoutRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Sesión cerrada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Cerrar sesión (revoca el refresh token)",
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
            "description": "Orden con el nuevo estado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrdenExternaResponseDto"
                }
              }
            }
          },
          "400": {
            "description": "Estado objetivo inválido (ESTADO_INVALIDO)"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
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
            "description": "Guías de despacho con monto serializado",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/EnvioConMontoResponseDto"
                  }
                }
              }
            }
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Quitar un item del carrito",
        "tags": [
          "Carrito"
        ]
      }
    },
    "/api/v1/admin/storage/tenants": {
      "get": {
        "operationId": "AdminStorageController_listar",
        "parameters": [
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "description": "Pagina (1-based)",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "description": "Tamano de pagina (max 100)",
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Listar tenants con almacenamiento (cuota y uso)",
        "tags": [
          "Almacenamiento (admin)"
        ]
      }
    },
    "/api/v1/admin/storage/tenants/{tenantId}": {
      "get": {
        "operationId": "AdminStorageController_estado",
        "parameters": [
          {
            "name": "tenantId",
            "required": true,
            "in": "path",
            "description": "Id del tenant (organizacion)",
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
        "summary": "Estado de almacenamiento de un tenant",
        "tags": [
          "Almacenamiento (admin)"
        ]
      },
      "patch": {
        "operationId": "AdminStorageController_actualizar",
        "parameters": [
          {
            "name": "tenantId",
            "required": true,
            "in": "path",
            "description": "Id del tenant (organizacion)",
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
                "$ref": "#/components/schemas/ActualizarTenantRequestDto"
              }
            }
          }
        },
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
        "summary": "Ajustar cuota y/o suspender el almacenamiento del tenant",
        "tags": [
          "Almacenamiento (admin)"
        ]
      }
    },
    "/api/v1/admin/storage/tenants/{tenantId}/recalcular": {
      "post": {
        "operationId": "AdminStorageController_recalcular",
        "parameters": [
          {
            "name": "tenantId",
            "required": true,
            "in": "path",
            "description": "Id del tenant (organizacion)",
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Recalcular el uso del tenant desde la metadata (idempotente)",
        "tags": [
          "Almacenamiento (admin)"
        ]
      }
    },
    "/api/v1/storage/bucket": {
      "get": {
        "description": "Crea el bucket del tenant de forma perezosa la primera vez.",
        "operationId": "BucketsController_estado",
        "parameters": [],
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
        "summary": "Estado del almacenamiento del tenant (bucket, cuota, uso)",
        "tags": [
          "Almacenamiento"
        ]
      },
      "post": {
        "operationId": "BucketsController_configurar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ConfigurarBucketRequestDto"
              }
            }
          }
        },
        "responses": {
          "400": {
            "description": "Cuota invalida (CUOTA_INVALIDA/CUOTA_MENOR_AL_USO)"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Asegurar el bucket del tenant y fijar su cuota (admin)",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/subidas": {
      "post": {
        "description": "Valida tipo, tamano y cuota del tenant; registra el objeto en estado pendiente y devuelve la URL prefirmada.",
        "operationId": "ObjetosController_solicitarSubida",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/SolicitarSubidaRequestDto"
              }
            }
          }
        },
        "responses": {
          "400": {
            "description": "Tipo o tamano invalido (VALIDACION)"
          },
          "409": {
            "description": "Cuota excedida (CUOTA_EXCEDIDA)"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Solicitar URL prefirmada de subida (PUT directo a MinIO)",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/objetos/{id}/confirmar": {
      "post": {
        "operationId": "ObjetosController_confirmar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del objeto",
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
        "summary": "Confirmar la subida y activar el objeto (contabiliza cuota)",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/objetos": {
      "get": {
        "operationId": "ObjetosController_listar",
        "parameters": [
          {
            "name": "q",
            "required": false,
            "in": "query",
            "description": "Busqueda por nombre",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "carpeta",
            "required": false,
            "in": "query",
            "description": "Filtrar por carpeta",
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
              "type": "string",
              "enum": [
                "pendiente",
                "activo",
                "eliminado"
              ]
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "description": "Pagina (1-based)",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "description": "Tamano de pagina (max 100)",
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
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Listar objetos del tenant (paginado, sin eliminados por defecto)",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/objetos/{id}": {
      "get": {
        "operationId": "ObjetosController_detalle",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del objeto",
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
        "summary": "Detalle (metadata) de un objeto del tenant",
        "tags": [
          "Almacenamiento"
        ]
      },
      "delete": {
        "operationId": "ObjetosController_eliminar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del objeto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "204": {
            "description": ""
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un objeto (baja logica + borrado en MinIO)",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/objetos/{id}/descarga": {
      "get": {
        "operationId": "ObjetosController_descarga",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del objeto",
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
        "summary": "URL prefirmada de descarga (GET) con validez limitada",
        "tags": [
          "Almacenamiento"
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
        "security": [
          {
            "bearer": []
          }
        ],
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
      },
      "internalKey": {
        "type": "apiKey",
        "in": "header",
        "name": "x-internal-key"
      }
    },
    "schemas": {
      "LoteProductoResponseDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "stock": {
            "type": "number",
            "description": "Stock disponible",
            "example": 10
          },
          "estado": {
            "type": "string",
            "description": "Estado del producto",
            "enum": [
              "disponible",
              "agotado"
            ],
            "example": "disponible"
          }
        },
        "required": [
          "sku",
          "stock",
          "estado"
        ]
      },
      "ProductoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del producto",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU único del producto",
            "example": "SKU-001"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del producto"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción del producto"
          },
          "categoria": {
            "type": "string",
            "description": "Categoría del producto"
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base en córdobas con dos decimales",
            "example": "1000.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock disponible (neto de reservas)",
            "example": 10
          },
          "estado": {
            "type": "string",
            "description": "Estado del producto",
            "enum": [
              "disponible",
              "agotado"
            ],
            "example": "disponible"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "sku",
          "nombre",
          "descripcion",
          "categoria",
          "precio_base",
          "stock",
          "estado",
          "creado_en"
        ]
      },
      "ResumenInventarioResponseDto": {
        "type": "object",
        "properties": {
          "productos": {
            "type": "number",
            "description": "Total de productos",
            "example": 20
          },
          "agotados": {
            "type": "number",
            "description": "Productos agotados",
            "example": 2
          },
          "stock_total": {
            "type": "number",
            "description": "Unidades en stock",
            "example": 120
          },
          "valor_inventario_cents": {
            "type": "number",
            "description": "Valor del inventario en centavos",
            "example": 12000000
          }
        },
        "required": [
          "productos",
          "agotados",
          "stock_total",
          "valor_inventario_cents"
        ]
      },
      "LineaInventarioResponseDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del producto"
          },
          "stock": {
            "type": "number",
            "description": "Stock físico en bodega",
            "example": 12
          },
          "disponible": {
            "type": "number",
            "description": "Stock disponible (neto de reservas)",
            "example": 10
          },
          "reservado": {
            "type": "number",
            "description": "Unidades reservadas por órdenes activas",
            "example": 2
          },
          "estado": {
            "type": "string",
            "description": "Estado del producto",
            "example": "disponible"
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base en córdobas con dos decimales",
            "example": "1000.00"
          },
          "reservas_activas": {
            "type": "number",
            "description": "Alias de reservado para consumidores internos",
            "example": 2
          },
          "valor_disponible_cents": {
            "type": "number",
            "description": "Valor del stock disponible en centavos",
            "example": 1000000
          },
          "valor_total_cents": {
            "type": "number",
            "description": "Valor del stock físico total en centavos",
            "example": 1200000
          }
        },
        "required": [
          "sku",
          "nombre",
          "stock",
          "disponible",
          "reservado",
          "estado",
          "precio_base",
          "reservas_activas",
          "valor_disponible_cents",
          "valor_total_cents"
        ]
      },
      "PaginaProductosResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Productos de la página",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ProductoResponseDto"
            }
          },
          "total": {
            "type": "number",
            "description": "Total de productos que cumplen el filtro",
            "example": 42
          },
          "pagina": {
            "type": "number",
            "description": "Página actual (1-based)",
            "example": 1
          },
          "limite": {
            "type": "number",
            "description": "Tamaño de página",
            "example": 20
          },
          "paginas": {
            "type": "number",
            "description": "Total de páginas",
            "example": 3
          }
        },
        "required": [
          "items",
          "total",
          "pagina",
          "limite",
          "paginas"
        ]
      },
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
      "LogoutRequestDto": {
        "type": "object",
        "properties": {
          "refresh_token": {
            "type": "string",
            "description": "Refresh token de la sesión a cerrar; si se omite, cierra todas"
          }
        }
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
      "ItemOrdenExternaResponseDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "cantidad": {
            "type": "number",
            "description": "Cantidad pedida",
            "example": 2
          },
          "producto_nombre": {
            "type": "string",
            "description": "Nombre del producto"
          }
        },
        "required": [
          "sku",
          "cantidad",
          "producto_nombre"
        ]
      },
      "OrdenExternaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "cliente_id": {
            "type": "string",
            "description": "Id del comprador",
            "format": "uuid"
          },
          "items": {
            "description": "Líneas de la orden",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ItemOrdenExternaResponseDto"
            }
          },
          "total": {
            "type": "string",
            "description": "Total en córdobas con dos decimales",
            "example": "1150.00"
          },
          "total_cents": {
            "type": "number",
            "description": "Total en centavos",
            "example": 115000
          },
          "estado": {
            "type": "string",
            "description": "Estado actual de la orden",
            "example": "enviada"
          }
        },
        "required": [
          "id",
          "cliente_id",
          "items",
          "total",
          "total_cents",
          "estado"
        ]
      },
      "EnvioConMontoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la guía de despacho",
            "format": "uuid"
          },
          "guia": {
            "type": "string",
            "description": "Número de guía",
            "example": "BGH-ABC123-DEF456"
          },
          "order_id": {
            "type": "string",
            "description": "Id de la orden asociada",
            "format": "uuid"
          },
          "estado": {
            "type": "string",
            "description": "Estado del envío",
            "example": "en_preparacion"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "monto": {
            "type": "string",
            "description": "Monto del envío en córdobas con dos decimales",
            "example": "1150.00"
          }
        },
        "required": [
          "id",
          "guia",
          "order_id",
          "estado",
          "creado_en",
          "monto"
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
      "ActualizarTenantRequestDto": {
        "type": "object",
        "properties": {
          "cuota_bytes": {
            "type": "number",
            "description": "Nueva cuota en bytes"
          },
          "estado": {
            "type": "string",
            "description": "Estado del almacenamiento del tenant",
            "enum": [
              "activo",
              "suspendido"
            ]
          }
        }
      },
      "ConfigurarBucketRequestDto": {
        "type": "object",
        "properties": {
          "cuota_bytes": {
            "type": "number",
            "description": "Cuota del tenant en bytes (por defecto la del entorno)",
            "example": 5368709120
          }
        }
      },
      "SolicitarSubidaRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "description": "Nombre del archivo (sin rutas)",
            "example": "foto-producto.jpg"
          },
          "content_type": {
            "type": "string",
            "description": "Content-Type del archivo (whitelist)",
            "example": "image/jpeg"
          },
          "tamano_bytes": {
            "type": "number",
            "description": "Tamano en bytes que el cliente subira",
            "example": 204800
          },
          "carpeta": {
            "type": "string",
            "description": "Carpeta logica dentro del bucket",
            "example": "productos"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripcion libre"
          },
          "etiquetas": {
            "type": "object",
            "description": "Etiquetas clave-valor (max 20)"
          }
        },
        "required": [
          "nombre",
          "content_type",
          "tamano_bytes"
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
