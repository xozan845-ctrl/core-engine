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
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "description": "Empresa cuyo catálogo se consulta (ADR-27). Obligatoria para lecturas anónimas; con sesión se usa la empresa del token.",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
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
        "summary": "Listar productos de una empresa (público, filtros y paginación)",
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
        "summary": "Crear producto con variantes e imágenes (admin, TC-01)",
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
          },
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "description": "Empresa a la que debe pertenecer el producto (ADR-27). Obligatoria para lecturas anónimas.",
            "schema": {
              "format": "uuid",
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
            "description": "Producto inexistente o de otra empresa (NO_ENCONTRADO)"
          }
        },
        "summary": "Detalle de producto de una empresa (público, incluye variantes)",
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
    "/api/v1/catalog/productos/{id}/variantes": {
      "post": {
        "operationId": "ProductosController_crearVariante",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto padre",
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
                "$ref": "#/components/schemas/VarianteProductoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Variante creada",
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
        "summary": "Agregar una variante al producto (ADR-28)",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/catalog/productos/{id}/variantes/{varianteId}": {
      "patch": {
        "operationId": "ProductosController_actualizarVariante",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto padre",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "varianteId",
            "required": true,
            "in": "path",
            "description": "Id de la variante",
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
            "description": "Variante actualizada",
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
        "summary": "Editar una variante (precio/stock/atributos)",
        "tags": [
          "Catálogo"
        ]
      },
      "delete": {
        "operationId": "ProductosController_eliminarVariante",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto padre",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "varianteId",
            "required": true,
            "in": "path",
            "description": "Id de la variante",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "204": {
            "description": "Variante eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una variante",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/catalog/productos/{id}/imagenes": {
      "post": {
        "operationId": "ProductosController_agregarImagen",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto o variante",
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
                "$ref": "#/components/schemas/ImagenProductoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Imagen agregada",
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
        "summary": "Agregar una imagen a un producto o variante (máx. 3)",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/catalog/productos/{id}/imagenes/{imagenId}": {
      "delete": {
        "operationId": "ProductosController_eliminarImagen",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del producto o variante",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "imagenId",
            "required": true,
            "in": "path",
            "description": "Id de la relación producto-imagen",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "204": {
            "description": "Imagen eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una imagen de un producto o variante",
        "tags": [
          "Catálogo"
        ]
      }
    },
    "/api/v1/admin/inventario": {
      "get": {
        "operationId": "InventarioController_listar",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "description": "Filtra el inventario de una empresa (ADR-27)",
            "schema": {
              "type": "string"
            }
          }
        ],
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
    "/api/v1/catalog/servicios": {
      "get": {
        "operationId": "ServiciosController_listar",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
          {
            "name": "q",
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
              "type": "string",
              "enum": [
                "activo",
                "inactivo"
              ]
            }
          },
          {
            "name": "categoria",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "maximum": 100,
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Página de servicios",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaServiciosResponseDto"
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
        "summary": "Listar servicios de una empresa",
        "tags": [
          "Catálogo · Servicios"
        ]
      },
      "post": {
        "operationId": "ServiciosController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearServicioRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Servicio creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ServicioResponseDto"
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
        "summary": "Crear un servicio",
        "tags": [
          "Catálogo · Servicios"
        ]
      }
    },
    "/api/v1/catalog/servicios/{id}": {
      "get": {
        "operationId": "ServiciosController_detalle",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del servicio",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Servicio encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ServicioResponseDto"
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
        "summary": "Detalle de un servicio",
        "tags": [
          "Catálogo · Servicios"
        ]
      },
      "patch": {
        "operationId": "ServiciosController_actualizar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del servicio",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarServicioRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Servicio actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ServicioResponseDto"
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
        "summary": "Actualizar un servicio",
        "tags": [
          "Catálogo · Servicios"
        ]
      },
      "delete": {
        "operationId": "ServiciosController_eliminar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del servicio",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "responses": {
          "204": {
            "description": "Servicio dado de baja"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Dar de baja un servicio (baja lógica)",
        "tags": [
          "Catálogo · Servicios"
        ]
      }
    },
    "/api/v1/catalog/configuracion-inventario": {
      "get": {
        "operationId": "OperacionesController_configuracion",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ConfiguracionInventarioResponseDto"
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
        "summary": "Obtener la configuración de inventario de la empresa",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "put": {
        "operationId": "OperacionesController_guardarConfiguracion",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarConfiguracionInventarioRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ConfiguracionInventarioResponseDto"
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
        "summary": "Guardar la configuración de inventario",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/productos/importar": {
      "post": {
        "operationId": "OperacionesController_importar",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ImportarProductosRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ImportarProductosResponseDto"
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
        "summary": "Importar productos desde filas validadas (máx. 200)",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/escanear": {
      "get": {
        "operationId": "OperacionesController_escanear",
        "parameters": [
          {
            "name": "codigo",
            "required": true,
            "in": "query",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EscaneoResponseDto"
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
        "summary": "Buscar un producto o servicio por código/barras",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/combos": {
      "get": {
        "operationId": "OperacionesController_listarCombos",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
          {
            "name": "q",
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
              "type": "string",
              "enum": [
                "activo",
                "inactivo"
              ]
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "maximum": 100,
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaCombosResponseDto"
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
        "summary": "Listar combos de la empresa",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "post": {
        "operationId": "OperacionesController_crearCombo",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearComboRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ComboResponseDto"
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
        "summary": "Crear un combo",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/combos/{id}": {
      "get": {
        "operationId": "OperacionesController_detalleCombo",
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
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ComboResponseDto"
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
        "summary": "Detalle de un combo",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "patch": {
        "operationId": "OperacionesController_actualizarCombo",
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
                "$ref": "#/components/schemas/ActualizarComboRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ComboResponseDto"
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
        "summary": "Actualizar un combo",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "delete": {
        "operationId": "OperacionesController_eliminarCombo",
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
          "204": {
            "description": "Combo dado de baja"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Dar de baja un combo",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/promociones": {
      "get": {
        "operationId": "OperacionesController_listarPromociones",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
          {
            "name": "estado",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string",
              "enum": [
                "activo",
                "inactivo"
              ]
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "maximum": 100,
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaPromocionesResponseDto"
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
        "summary": "Listar promociones de la empresa",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "post": {
        "operationId": "OperacionesController_crearPromocion",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearPromocionRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PromocionResponseDto"
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
        "summary": "Crear una promoción",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/promociones/{id}": {
      "get": {
        "operationId": "OperacionesController_detallePromocion",
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
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PromocionResponseDto"
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
        "summary": "Detalle de una promoción",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "patch": {
        "operationId": "OperacionesController_actualizarPromocion",
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
                "$ref": "#/components/schemas/ActualizarPromocionRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PromocionResponseDto"
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
        "summary": "Actualizar una promoción",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "delete": {
        "operationId": "OperacionesController_eliminarPromocion",
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
          "204": {
            "description": "Promoción dada de baja"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Dar de baja una promoción",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/catalog/inventario/movimientos": {
      "get": {
        "operationId": "OperacionesController_listarMovimientos",
        "parameters": [
          {
            "name": "empresa_id",
            "required": false,
            "in": "query",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
          {
            "name": "tipo",
            "required": false,
            "in": "query",
            "schema": {
              "type": "string",
              "enum": [
                "entrada",
                "salida",
                "ajuste",
                "traslado"
              ]
            }
          },
          {
            "name": "pagina",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "type": "number"
            }
          },
          {
            "name": "limite",
            "required": false,
            "in": "query",
            "schema": {
              "minimum": 1,
              "maximum": 100,
              "type": "number"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaMovimientosResponseDto"
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
        "summary": "Listar movimientos y traslados de inventario",
        "tags": [
          "Catálogo · Operaciones"
        ]
      },
      "post": {
        "operationId": "OperacionesController_registrarMovimiento",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearMovimientoInventarioRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MovimientoInventarioResponseDto"
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
        "summary": "Registrar entrada, salida, ajuste o traslado",
        "tags": [
          "Catálogo · Operaciones"
        ]
      }
    },
    "/api/v1/vendedores/me/liquidaciones": {
      "get": {
        "operationId": "LiquidacionesController_mias",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Liquidaciones del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LiquidacionResponseDto"
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
        "summary": "Cortes/liquidaciones del vendedor autenticado",
        "tags": [
          "Comisiones · Liquidaciones"
        ]
      }
    },
    "/api/v1/empresarios/me/liquidaciones": {
      "get": {
        "operationId": "LiquidacionesController_deMiEmpresa",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Liquidaciones del empresario",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LiquidacionEmpresaResponseDto"
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
        "summary": "Liquidaciones de la empresa autenticada",
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
            "description": "Liquidaciones generadas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LiquidacionResponseDto"
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
            "description": "Liquidación pagada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/LiquidacionResponseDto"
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
            "description": "Estado de cuenta del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoCuentaResponseDto"
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
        "summary": "Estado de cuenta de ventas/comisiones del vendedor",
        "tags": [
          "Comisiones · Ventas"
        ]
      }
    },
    "/api/v1/empresarios/me/ventas": {
      "get": {
        "operationId": "VentasEmpresaController_estado",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Estado de cuenta de la empresa",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoCuentaEmpresaResponseDto"
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
        "summary": "Estado de cuenta de ventas/comisiones de la empresa",
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
            "description": "KPIs de ventas e inventario",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ReporteKpiResponseDto"
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
            "description": "Personal del tenant",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/PersonalResponseDto"
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
            "description": "Personal creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PersonalResponseDto"
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
            "description": "Personal encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PersonalResponseDto"
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
            "description": "Personal actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PersonalResponseDto"
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
            "description": "Personal eliminado"
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
            "description": "Ubicación del personal",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/UbicacionResponseDto"
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
            "description": "Ubicación actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PersonalResponseDto"
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
            "description": "Clientes de campo",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ClienteResponseDto"
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
            "description": "Cliente creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ClienteResponseDto"
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
            "description": "Cliente encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ClienteResponseDto"
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
            "description": "Cliente actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ClienteResponseDto"
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
            "description": "Cliente eliminado"
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
            "description": "Vehículos del tenant",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/VehiculoResponseDto"
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
            "description": "Vehículo creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VehiculoResponseDto"
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
            "description": "Vehículo encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VehiculoResponseDto"
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
            "description": "Vehículo actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VehiculoResponseDto"
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
            "description": "Vehículo eliminado"
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
            "description": "Rutas del tenant",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/RutaResponseDto"
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
            "description": "Ruta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RutaResponseDto"
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
            "description": "Ruta encontrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RutaResponseDto"
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
            "description": "Ruta actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RutaResponseDto"
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
            "description": "Ruta eliminada"
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
            "description": "Parada creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ParadaResponseDto"
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
            "description": "Parada actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ParadaResponseDto"
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
            "description": "Ruta asignada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RutaResponseDto"
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
            "description": "Pedidos de campo",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/PedidoResponseDto"
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
            "description": "Pedido creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PedidoResponseDto"
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
            "description": "Pedido encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PedidoResponseDto"
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
            "description": "Pedido actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PedidoResponseDto"
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
            "description": "Pedido eliminado"
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
            "description": "Pedido actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PedidoResponseDto"
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
            "description": "Registros de asistencia",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/AsistenciaResponseDto"
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
            "description": "Asistencia registrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AsistenciaResponseDto"
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
            "description": "Incidencias",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/IncidenciaResponseDto"
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
            "description": "Incidencia creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/IncidenciaResponseDto"
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
            "description": "Incidencia encontrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/IncidenciaResponseDto"
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
            "description": "Incidencia actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/IncidenciaResponseDto"
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
            "description": "Incidencia eliminada"
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
            "description": "Registros de tracking",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TrackingResponseDto"
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
            "description": "Tracking registrado",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TrackingResponseDto"
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
            "description": "Visitas registradas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/VisitaResponseDto"
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
            "description": "Visita registrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VisitaResponseDto"
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
            "description": "Cumplimiento de la ruta",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CumplimientoResponseDto"
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
            "description": "Cumplimiento guardado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CumplimientoResponseDto"
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
            "description": "Resultado de la sincronización",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/SyncResultadoResponseDto"
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
            "description": "Plan de cuentas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CuentaResponseDto"
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
            "description": "Cuenta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaResponseDto"
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
            "description": "Cuenta actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaResponseDto"
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
            "description": "Asientos del periodo",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/AsientoResponseDto"
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
            "description": "Asiento registrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AsientoResponseDto"
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
            "description": "Asiento anulado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AsientoResponseDto"
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
            "description": "Libro mayor de la cuenta",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/LibroMayorResponseDto"
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
            "description": "Libro de ventas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LibroIvaResponseDto"
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
            "description": "Libro de compras",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/LibroIvaResponseDto"
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
            "description": "Comprobantes fiscales",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ComprobanteResponseDto"
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
            "description": "Comprobante emitido",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ComprobanteResponseDto"
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
            "description": "Series de comprobantes",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/SerieComprobanteResponseDto"
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
            "description": "Comprobante anulado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ComprobanteResponseDto"
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
            "description": "Jurisdicciones",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/JurisdiccionResponseDto"
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
            "description": "Jurisdicción creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/JurisdiccionResponseDto"
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
            "description": "Regímenes fiscales",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/RegimenFiscalResponseDto"
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
            "description": "Régimen creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RegimenFiscalResponseDto"
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
            "description": "Sujetos tributarios",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/SujetoTributarioResponseDto"
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
            "description": "Sujeto registrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SujetoTributarioResponseDto"
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
            "description": "Sujeto dado de baja",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SujetoTributarioResponseDto"
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
            "description": "Declaraciones",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/DeclaracionResponseDto"
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
            "description": "Declaraciones generadas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/DeclaracionResponseDto"
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
            "description": "Declaración presentada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/DeclaracionResponseDto"
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
            "description": "Declaración pagada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/DeclaracionResponseDto"
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
            "description": "Proyección creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProyeccionResponseDto"
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
            "description": "Proyecciones",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ProyeccionResponseDto"
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
            "description": "Punto de equilibrio",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PuntoEquilibrioResponseDto"
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
            "description": "KPIs financieros",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/KpisFinancierosResponseDto"
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
            "description": "Estudios de sensibilidad",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SensibilidadEstudiosResponseDto"
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
            "description": "Plan a 24 meses",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PlanBienalResponseDto"
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
            "description": "Proyección encontrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProyeccionResponseDto"
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
            "description": "Tablero financiero",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TableroResponseDto"
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
            "description": "Situación fiscal del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VendedorFiscalResponseDto"
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
        "summary": "Situación fiscal del vendedor (cap. 4.4)",
        "tags": [
          "Finanzas · Vendedor"
        ]
      }
    },
    "/api/v1/comercio/bancos-cuentas": {
      "get": {
        "operationId": "ComercioController_listarBancos",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Cuentas bancarias",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CuentaBancoResponseDto"
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
        "summary": "Listar cuentas bancarias del workspace",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearBanco",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCuentaBancoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Cuenta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaBancoResponseDto"
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
        "summary": "Registrar una cuenta bancaria",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/bancos-cuentas/{id}": {
      "patch": {
        "operationId": "ComercioController_actualizarBanco",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la cuenta",
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
                "$ref": "#/components/schemas/ActualizarCuentaBancoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Cuenta actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaBancoResponseDto"
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
        "summary": "Actualizar una cuenta bancaria",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "delete": {
        "operationId": "ComercioController_eliminarBanco",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la cuenta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Cuenta eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una cuenta bancaria",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/creditos": {
      "get": {
        "operationId": "ComercioController_listarCreditos",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Créditos registrados",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CreditoResponseDto"
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
        "summary": "Listar créditos del workspace",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCredito",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCreditoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Crédito creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CreditoResponseDto"
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
        "summary": "Registrar un crédito",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/creditos/{id}/pagar": {
      "post": {
        "operationId": "ComercioController_pagarCredito",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del crédito",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Crédito pagado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CreditoResponseDto"
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
        "summary": "Marcar un crédito como pagado",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/creditos/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarCredito",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del crédito",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Crédito eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un crédito",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/gastos": {
      "get": {
        "operationId": "ComercioController_listarGastos",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Gastos registrados",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/GastoResponseDto"
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
        "summary": "Listar gastos de la empresa",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearGasto",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearGastoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Gasto creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/GastoResponseDto"
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
        "summary": "Registrar un gasto",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/gastos/{id}": {
      "patch": {
        "operationId": "ComercioController_actualizarGasto",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del gasto",
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
                "$ref": "#/components/schemas/ActualizarGastoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Gasto actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/GastoResponseDto"
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
        "summary": "Actualizar un gasto",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "delete": {
        "operationId": "ComercioController_eliminarGasto",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del gasto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Gasto eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un gasto",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/gastos/{id}/pagar": {
      "post": {
        "operationId": "ComercioController_pagarGasto",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del gasto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Gasto pagado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/GastoResponseDto"
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
        "summary": "Marcar un gasto como pagado",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/gastos/{id}/cancelar": {
      "post": {
        "operationId": "ComercioController_cancelarGasto",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del gasto",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Gasto cancelado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/GastoResponseDto"
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
        "summary": "Cancelar un gasto",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/cuentas-por-pagar": {
      "get": {
        "operationId": "ComercioController_listarCuentasPorPagar",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Cuentas por pagar",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CuentaPorPagarResponseDto"
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
        "summary": "Listar cuentas por pagar del workspace",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCuentaPorPagar",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCuentaPorPagarRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Cuenta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaPorPagarResponseDto"
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
        "summary": "Registrar una cuenta por pagar",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/cuentas-por-pagar/{id}/pagar": {
      "post": {
        "operationId": "ComercioController_pagarCuentaPorPagar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la cuenta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Cuenta pagada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CuentaPorPagarResponseDto"
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
        "summary": "Marcar una cuenta por pagar como pagada",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/cuentas-por-pagar/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarCuentaPorPagar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la cuenta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Cuenta eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una cuenta por pagar",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/proveedores": {
      "get": {
        "operationId": "ComercioController_listarProveedores",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Proveedores",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ProveedorResponseDto"
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
        "summary": "Listar proveedores de la empresa",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearProveedor",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearProveedorRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Proveedor creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ProveedorResponseDto"
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
        "summary": "Registrar un proveedor",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/proveedores/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarProveedor",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del proveedor",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Proveedor eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un proveedor",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/compras": {
      "get": {
        "operationId": "ComercioController_listarCompras",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Compras registradas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CompraResponseDto"
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
        "summary": "Listar compras de la empresa",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCompra",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCompraRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Compra creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CompraResponseDto"
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
        "summary": "Registrar una compra por líneas de producto",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/compras/directas": {
      "post": {
        "operationId": "ComercioController_crearCompraDirecta",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCompraDirectaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Compra directa creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CompraResponseDto"
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
        "summary": "Registrar una compra directa (gasto no inventariable)",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/compras/producto-nuevo": {
      "post": {
        "operationId": "ComercioController_crearCompraProductoNuevo",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCompraProductoNuevoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Compra creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CompraResponseDto"
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
        "summary": "Registrar la compra de un producto nuevo",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/compras/{id}/anular": {
      "post": {
        "operationId": "ComercioController_anularCompra",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la compra",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Compra anulada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CompraResponseDto"
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
        "summary": "Anular una compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/compras/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarCompra",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la compra",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Compra eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/ordenes-compra": {
      "get": {
        "operationId": "ComercioController_listarOrdenesCompra",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Órdenes de compra",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrdenCompraResponseDto"
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
        "summary": "Listar órdenes de compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearOrdenCompra",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearOrdenCompraRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Orden creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrdenCompraResponseDto"
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
        "summary": "Crear una orden de compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/ordenes-compra/{id}/convertir": {
      "post": {
        "operationId": "ComercioController_convertirOrdenCompra",
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
            "description": "Orden convertida",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrdenCompraResponseDto"
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
        "summary": "Convertir una orden pendiente en compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/ordenes-compra/{id}/anular": {
      "post": {
        "operationId": "ComercioController_anularOrdenCompra",
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
            "description": "Orden anulada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrdenCompraResponseDto"
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
        "summary": "Anular una orden de compra",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/configuracion-documentos": {
      "get": {
        "operationId": "ComercioController_obtenerConfiguracionDocumentos",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Configuración vigente",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ConfiguracionDocumentosResponseDto"
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
        "summary": "Obtener las preferencias de documentos imprimibles",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "put": {
        "operationId": "ComercioController_actualizarConfiguracionDocumentos",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ActualizarConfiguracionDocumentosRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Configuración guardada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ConfiguracionDocumentosResponseDto"
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
        "summary": "Guardar las preferencias de documentos imprimibles",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/metodos-pago": {
      "get": {
        "operationId": "ComercioController_listarMetodosPago",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Métodos de pago",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/MetodoPagoResponseDto"
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
        "summary": "Listar métodos de pago de la caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearMetodoPago",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearMetodoPagoRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Método creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MetodoPagoResponseDto"
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
        "summary": "Agregar un método de pago",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/metodos-pago/{id}": {
      "patch": {
        "operationId": "ComercioController_actualizarMetodoPago",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del método",
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
                "$ref": "#/components/schemas/ActualizarMetodoPagoRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Método actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MetodoPagoResponseDto"
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
        "summary": "Actualizar un método de pago",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "delete": {
        "operationId": "ComercioController_eliminarMetodoPago",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del método",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Método eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un método de pago sin movimientos",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/movimientos": {
      "get": {
        "operationId": "ComercioController_listarMovimientosCaja",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Movimientos de caja",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/MovimientoCajaResponseDto"
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
        "summary": "Listar movimientos de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearMovimientoCaja",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearMovimientoCajaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Movimiento creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MovimientoCajaResponseDto"
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
        "summary": "Registrar un movimiento de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/movimientos/{id}/anular": {
      "post": {
        "operationId": "ComercioController_anularMovimientoCaja",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del movimiento",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Movimiento anulado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MovimientoCajaResponseDto"
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
        "summary": "Anular un movimiento de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/movimientos/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarMovimientoCaja",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del movimiento",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Movimiento eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un movimiento de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/cierres": {
      "get": {
        "operationId": "ComercioController_listarCierres",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Cierres registrados",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CierreCajaResponseDto"
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
        "summary": "Listar cierres de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCierre",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCierreCajaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Cierre registrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CierreCajaResponseDto"
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
        "summary": "Registrar el cierre diario de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/cierres/preview": {
      "get": {
        "operationId": "ComercioController_previewCierre",
        "parameters": [
          {
            "name": "fecha",
            "required": false,
            "in": "query",
            "description": "Fecha local yyyy-mm-dd",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Corte calculado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CierreCajaPreviewResponseDto"
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
        "summary": "Previsualizar el corte del día",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/chica/fondo": {
      "get": {
        "operationId": "ComercioController_obtenerFondoCajaChica",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Fondo vigente",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/FondoCajaChicaResponseDto"
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
        "summary": "Obtener el fondo de caja chica",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "put": {
        "operationId": "ComercioController_guardarFondoCajaChica",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearFondoCajaChicaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Fondo actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/FondoCajaChicaResponseDto"
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
        "summary": "Actualizar el fondo de caja chica",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/chica/movimientos": {
      "get": {
        "operationId": "ComercioController_listarMovimientosCajaChica",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Movimientos de caja chica",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/MovimientoCajaChicaResponseDto"
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
        "summary": "Listar movimientos de caja chica",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearMovimientoCajaChica",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearMovimientoCajaChicaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Movimiento creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MovimientoCajaChicaResponseDto"
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
        "summary": "Registrar un movimiento de caja chica",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/transferencias": {
      "get": {
        "operationId": "ComercioController_listarTransferencias",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Transferencias",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TransferenciaCajaResponseDto"
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
        "summary": "Listar transferencias internas de caja",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearTransferencia",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearTransferenciaCajaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Transferencia creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TransferenciaCajaResponseDto"
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
        "summary": "Registrar una transferencia entre caja y bancos",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/caja/transferencias/{id}/anular": {
      "post": {
        "operationId": "ComercioController_anularTransferencia",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la transferencia",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "201": {
            "description": "Transferencia anulada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TransferenciaCajaResponseDto"
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
        "summary": "Anular una transferencia interna",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/clientes": {
      "get": {
        "operationId": "ComercioController_listarClientes",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Clientes registrados",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ClienteComercioResponseDto"
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
        "summary": "Listar la cartera de clientes del workspace",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCliente",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearClienteComercioRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Cliente creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ClienteComercioResponseDto"
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
        "summary": "Registrar un cliente manual",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/clientes/importar": {
      "post": {
        "operationId": "ComercioController_importarClientes",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/ImportarClientesRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Resultado de la importación",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ImportarClientesResponseDto"
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
        "summary": "Importar una cartera de clientes",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/clientes/{id}": {
      "patch": {
        "operationId": "ComercioController_actualizarCliente",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del cliente",
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
                "$ref": "#/components/schemas/ActualizarClienteComercioRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Cliente actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ClienteComercioResponseDto"
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
        "summary": "Actualizar un cliente",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "delete": {
        "operationId": "ComercioController_eliminarCliente",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del cliente",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Cliente eliminado"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar un cliente",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/codigos-barras": {
      "get": {
        "operationId": "ComercioController_listarCodigosBarras",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Etiquetas registradas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CodigoBarrasResponseDto"
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
        "summary": "Listar etiquetas de códigos de barras",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      },
      "post": {
        "operationId": "ComercioController_crearCodigoBarras",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearCodigoBarrasRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Etiqueta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CodigoBarrasResponseDto"
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
        "summary": "Generar una etiqueta de código de barras",
        "tags": [
          "Comercio · Operación de tienda"
        ]
      }
    },
    "/api/v1/comercio/codigos-barras/{id}": {
      "delete": {
        "operationId": "ComercioController_eliminarCodigoBarras",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id de la etiqueta",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Etiqueta eliminada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Eliminar una etiqueta de código de barras",
        "tags": [
          "Comercio · Operación de tienda"
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
            "description": "Sesión creada (JWT + refresh)",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SesionResponseDto"
                }
              }
            }
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
            "description": "Sesión iniciada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SesionResponseDto"
                }
              }
            }
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
            "description": "Sesión renovada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SesionResponseDto"
                }
              }
            }
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
          "201": {
            "description": "Sesión cerrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OkResponseDto"
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
            "description": "Usuario creado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/UsuarioResponseDto"
                }
              }
            }
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
            "description": "Contraseña cambiada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OkResponseDto"
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
            "description": "Solicitud aceptada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OkResponseDto"
                }
              }
            }
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
            "description": "Contexto del usuario autenticado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/UsuarioContextoResponseDto"
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
            "description": "Personal vinculado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OkResponseDto"
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
            "description": "Usuarios registrados",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/UsuarioResponseDto"
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
          "200": {
            "description": "Usuario encontrado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/UsuarioResponseDto"
                }
              }
            }
          },
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
    "/api/v1/empresas": {
      "post": {
        "operationId": "EmpresasController_crear",
        "parameters": [],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearEmpresaRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Empresa creada con membresía del empresario",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EmpresaResponseDto"
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
        "summary": "Crear empresa y elegir modalidad logística",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{id}": {
      "get": {
        "operationId": "EmpresasController_obtener",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "UUID de la empresa o su slug público actual/anterior. Un slug anterior responde 301.",
            "schema": {
              "example": "distribuidora-piloto",
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Empresa visible para el miembro o administrador global",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EmpresaResponseDto"
                }
              }
            }
          },
          "301": {
            "description": "El slug consultado es anterior; Location apunta al slug actual",
            "headers": {
              "Location": {
                "description": "Ruta con el slug actual",
                "schema": {
                  "type": "string"
                }
              }
            }
          },
          "404": {
            "description": "Empresa inexistente o no visible para el usuario"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Obtener una empresa por UUID o slug (solo miembros o administradores)",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{id}/slug": {
      "patch": {
        "operationId": "EmpresasController_actualizarSlug",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "UUID de la empresa o su slug actual/anterior",
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
                "$ref": "#/components/schemas/ActualizarSlugRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Slug actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EmpresaResponseDto"
                }
              }
            }
          },
          "409": {
            "description": "El slug ya está en uso (incluye slugs históricos)"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Cambiar el slug público; los anteriores redirigen 301",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{id}/logistica": {
      "patch": {
        "operationId": "EmpresasController_actualizarLogistica",
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
                "$ref": "#/components/schemas/ActualizarModalidadLogisticaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Modalidad logística actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EmpresaResponseDto"
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
        "summary": "Cambiar la modalidad logística para envíos futuros",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{id}/tipo": {
      "patch": {
        "operationId": "EmpresasController_actualizarTipo",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "UUID de la empresa",
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
                "$ref": "#/components/schemas/ActualizarTipoEmpresaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Tipo de empresa actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EmpresaResponseDto"
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
        "summary": "Clasificar la empresa (comercial, logística o plataforma)",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{id}/vendedores": {
      "post": {
        "operationId": "EmpresasController_vendedor",
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
                "$ref": "#/components/schemas/CrearVendedorRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Vendedor creado y asociado a la empresa",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/VendedorEmpresaResponseDto"
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
        "summary": "Crear vendedor con participación predeterminada",
        "tags": [
          "Empresas"
        ]
      }
    },
    "/api/v1/empresas/{empresaId}/invitaciones/vendedores": {
      "post": {
        "operationId": "InvitacionesEmpresaController_crear",
        "parameters": [
          {
            "name": "empresaId",
            "required": true,
            "in": "path",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/CrearInvitacionRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Invitación generada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/InvitacionCreadaResponseDto"
                }
              }
            }
          },
          "409": {
            "description": "Tienda asignada, invitación duplicada o correo ya miembro"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Generar una invitación para un futuro vendedor",
        "tags": [
          "Invitaciones"
        ]
      }
    },
    "/api/v1/empresas/{empresaId}/vendedores": {
      "get": {
        "operationId": "InvitacionesEmpresaController_listar",
        "parameters": [
          {
            "name": "empresaId",
            "required": true,
            "in": "path",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Vendedores e invitaciones de la empresa",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ListarVendedoresResponseDto"
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
        "summary": "Listar vendedores activos e invitaciones de la empresa",
        "tags": [
          "Invitaciones"
        ]
      }
    },
    "/api/v1/empresas/{empresaId}/invitaciones/vendedores/{invitacionId}": {
      "delete": {
        "operationId": "InvitacionesEmpresaController_revocar",
        "parameters": [
          {
            "name": "empresaId",
            "required": true,
            "in": "path",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          },
          {
            "name": "invitacionId",
            "required": true,
            "in": "path",
            "schema": {
              "format": "uuid",
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Invitación revocada"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Revocar una invitación pendiente",
        "tags": [
          "Invitaciones"
        ]
      }
    },
    "/api/v1/invitaciones/vendedores/{token}": {
      "get": {
        "operationId": "InvitacionesPublicasController_consultar",
        "parameters": [
          {
            "name": "token",
            "required": true,
            "in": "path",
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Invitación válida",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/InvitacionPublicaResponseDto"
                }
              }
            }
          },
          "410": {
            "description": "Invitación inexistente, expirada o utilizada"
          }
        },
        "summary": "Consultar una invitación por token (público)",
        "tags": [
          "Invitaciones"
        ]
      }
    },
    "/api/v1/invitaciones/vendedores/{token}/aceptar": {
      "post": {
        "operationId": "InvitacionesPublicasController_aceptar",
        "parameters": [
          {
            "name": "token",
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
                "$ref": "#/components/schemas/AceptarInvitacionRequestDto"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Vendedor creado y sesión iniciada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AceptarInvitacionResponseDto"
                }
              }
            }
          },
          "409": {
            "description": "El correo ya tiene una cuenta o la tienda ya fue asignada"
          },
          "410": {
            "description": "Invitación inexistente, expirada o utilizada"
          }
        },
        "summary": "Aceptar una invitación y crear la cuenta del vendedor (público)",
        "tags": [
          "Invitaciones"
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
        "summary": "Avanzar el estado de una orden (logística o empresario de la empresa)",
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
    "/api/v1/empresarios/me/envios": {
      "get": {
        "operationId": "EnviosEmpresaController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/EnvioResponseDto"
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
        "summary": "Listar envíos de la empresa activa",
        "tags": [
          "Logística empresarial"
        ]
      }
    },
    "/api/v1/empresarios/me/envios/{id}/asignacion": {
      "post": {
        "operationId": "EnviosEmpresaController_asignar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del envío",
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
                "$ref": "#/components/schemas/AsignarProveedorRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EnvioResponseDto"
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
        "summary": "Elegir flota para un envío de modalidad híbrida",
        "tags": [
          "Logística empresarial"
        ]
      }
    },
    "/api/v1/envios/{id}/estado": {
      "patch": {
        "operationId": "EnviosFlotaController_avanzar",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "Id del envío",
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
                "$ref": "#/components/schemas/AvanzarEnvioRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EnvioResponseDto"
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
        "summary": "Avanzar un envío asignado a la flota activa",
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
            "description": "Puntos del mapa de calor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MapaCalorResponseDto"
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
            "description": "Puntos del mapa de calor del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/MapaCalorResponseDto"
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
            "description": "Rendimiento por vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaRendimientoResponseDto"
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
            "description": "Rendimiento del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaRendimientoResponseDto"
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
            "description": "Cobertura por celda",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/CoberturaZonaResponseDto"
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
            "description": "Demanda por producto",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaDemandaResponseDto"
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
            "description": "Tendencias temporales",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TendenciaResponseDto"
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
            "description": "Resumen ejecutivo",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ResumenInteligenciaResponseDto"
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
            "description": "Venta registrada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/RegistrarVentaResponseDto"
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
            "description": "Pronóstico de demanda",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ForecastDemandaResponseDto"
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
            "description": "Anomalías detectadas",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AnomaliasResponseDto"
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
            "description": "Score del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ScoreVendedorResponseDto"
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
            "description": "Score del vendedor autenticado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ScoreVendedorResponseDto"
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
            "description": "Calidad del pipeline",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CalidadDatosResponseDto"
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
            "description": "Orden creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrderViewResponseDto"
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
            "description": "Órdenes del comprador",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrderViewResponseDto"
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
            "description": "Órdenes registradas",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrderViewResponseDto"
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
            "description": "Ventas del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrderViewResponseDto"
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
        "summary": "Órdenes del vendedor autenticado (mis ventas)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/vendedor/comisiones": {
      "get": {
        "deprecated": true,
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
            "description": "Comisiones por periodo",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/ComisionVistaResponseDto"
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
        "summary": "Comisiones por periodo (vendedor/admin) — obsoleto: usar /vendedores/me/ventas",
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
            "description": "Órdenes de la tienda",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrderViewResponseDto"
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
        "summary": "Órdenes de una tienda (vendedor dueño, empresario de la empresa o admin)",
        "tags": [
          "Pedidos"
        ]
      }
    },
    "/api/v1/orders/empresa": {
      "get": {
        "operationId": "PedidosController_porEmpresa",
        "parameters": [
          {
            "name": "empresaId",
            "required": false,
            "in": "query",
            "description": "Solo admin/super_admin; el empresario usa su empresa activa",
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "tiendaId",
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
            "description": "Órdenes de la empresa",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/OrderViewResponseDto"
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
        "summary": "Órdenes de una empresa (empresario de la empresa o admin)",
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
            "description": "Detalle de la orden",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OrderViewResponseDto"
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
            "description": "Timeline de la orden",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TimelineEventoResponseDto"
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
            "description": "Carrito del comprador",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CarritoResponseDto"
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
            "description": "Carrito vacío",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CarritoResponseDto"
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
            "description": "Item agregado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CarritoResponseDto"
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
            "description": "Carrito actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CarritoResponseDto"
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
            "description": "Carrito actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/CarritoResponseDto"
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
            "description": "Página de tenants",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaBucketsResponseDto"
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
            "description": "Estado del tenant",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoBucketResponseDto"
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
            "description": "Tenant actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoBucketResponseDto"
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
            "description": "Uso recalculado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoBucketResponseDto"
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
            "description": "Estado del bucket del tenant",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoBucketResponseDto"
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
          "201": {
            "description": "Bucket asegurado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/EstadoBucketResponseDto"
                }
              }
            }
          },
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
          "201": {
            "description": "URL de subida creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SolicitudSubidaResponseDto"
                }
              }
            }
          },
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
            "description": "Objeto activado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ObjetoStorageResponseDto"
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
            "description": "Página de objetos",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaObjetosResponseDto"
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
            "description": "Metadata del objeto",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ObjetoStorageResponseDto"
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
            "description": "Objeto eliminado"
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
            "description": "URL de descarga",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/UrlDescargaResponseDto"
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
        "summary": "URL prefirmada de descarga (GET) con validez limitada",
        "tags": [
          "Almacenamiento"
        ]
      }
    },
    "/api/v1/storage/public/objetos/{id}": {
      "get": {
        "operationId": "PublicStorageController_servir",
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
            "description": "Imagen (binario)",
            "content": {
              "image/*": {
                "schema": {
                  "type": "string",
                  "format": "binary"
                }
              }
            }
          },
          "404": {
            "description": "Objeto inexistente, privado, eliminado o no imagen"
          }
        },
        "summary": "Servir una imagen pública (cacheable, sin auth)",
        "tags": [
          "Almacenamiento público"
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
            "description": "Tienda creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TiendaResponseDto"
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
            "description": "Tienda del vendedor",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TiendaResponseDto"
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
        "summary": "Tienda del vendedor autenticado",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/empresarios/me/tiendas": {
      "get": {
        "operationId": "TiendasEmpresaController_listar",
        "parameters": [],
        "responses": {
          "200": {
            "description": "Tiendas de la empresa",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/TiendaResponseDto"
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
        "summary": "Tiendas de la empresa autenticada",
        "tags": [
          "Tiendas"
        ]
      }
    },
    "/api/v1/tiendas/{id}/slug": {
      "patch": {
        "operationId": "TiendasSlugController_actualizarSlug",
        "parameters": [
          {
            "name": "id",
            "required": true,
            "in": "path",
            "description": "UUID de la tienda o su slug actual/anterior",
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
                "$ref": "#/components/schemas/ActualizarSlugTiendaRequestDto"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Slug actualizado",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TiendaResponseDto"
                }
              }
            }
          },
          "403": {
            "description": "El usuario no es propietario de la tienda"
          },
          "409": {
            "description": "El slug ya está en uso (incluye slugs históricos)"
          }
        },
        "security": [
          {
            "bearer": []
          }
        ],
        "summary": "Cambiar el slug público; los anteriores redirigen 301",
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
            "description": "Oferta creada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OfertaResponseDto"
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
            "description": "Oferta actualizada",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/OfertaResponseDto"
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
            "description": "Página de ofertas",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PaginaOfertasResponseDto"
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
            "description": "UUID de la tienda o su slug público actual/anterior. Un slug anterior responde 301.",
            "schema": {
              "example": "mi-tienda",
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Tienda con sus ofertas",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/TiendaPublicaResponseDto"
                }
              }
            }
          },
          "301": {
            "description": "Slug anterior; Location apunta al slug actual",
            "headers": {
              "Location": {
                "description": "Ruta con el slug actual",
                "schema": {
                  "type": "string"
                }
              }
            }
          },
          "404": {
            "description": "Tienda inexistente (NO_ENCONTRADO)"
          }
        },
        "summary": "Tienda pública con sus ofertas (UUID o slug; ruta pública)",
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
          "empresa_id": {
            "type": "string",
            "description": "Empresa dueña del producto",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto (único por empresa)",
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
      "ImagenProductoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la relación producto-imagen",
            "format": "uuid"
          },
          "objeto_id": {
            "type": "string",
            "description": "Id del objeto en storage-service",
            "format": "uuid"
          },
          "url": {
            "type": "string",
            "description": "Ruta pública relativa servida por storage-service",
            "example": "/api/v1/storage/public/objetos/8f0c..."
          },
          "orden": {
            "type": "number",
            "description": "Orden de la imagen (0-2)",
            "minimum": 0,
            "maximum": 2
          },
          "es_principal": {
            "type": "boolean",
            "description": "Imagen principal del producto/variante"
          }
        },
        "required": [
          "id",
          "objeto_id",
          "url",
          "orden",
          "es_principal"
        ]
      },
      "VarianteProductoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la variante",
            "format": "uuid"
          },
          "producto_padre_id": {
            "type": "string",
            "description": "Id del producto padre",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU de la variante",
            "example": "SKU-001-ROJO-M"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre visible de la variante"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos de la variante (color, talla, etc.)",
            "additionalProperties": true
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base en córdobas con dos decimales",
            "example": "1500.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock disponible",
            "example": 10
          },
          "estado": {
            "type": "string",
            "description": "Estado de la variante",
            "enum": [
              "disponible",
              "agotado"
            ]
          },
          "imagenes": {
            "description": "Imágenes de la variante (máx. 3)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ImagenProductoResponseDto"
            }
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "producto_padre_id",
          "sku",
          "nombre",
          "atributos",
          "precio_base",
          "stock",
          "estado",
          "imagenes",
          "creado_en"
        ]
      },
      "ProductoResponseDto": {
        "type": "object",
        "properties": {
          "empresa_id": {
            "type": "string",
            "description": "Empresa propietaria",
            "format": "uuid"
          },
          "id": {
            "type": "string",
            "description": "Id del producto",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto (único por empresa)",
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
          "estado_operativo": {
            "type": "string",
            "description": "Estado comercial del producto",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "costo": {
            "type": "string",
            "description": "Costo en córdobas con dos decimales",
            "example": "150.00"
          },
          "marca": {
            "type": "string",
            "description": "Marca"
          },
          "codigo_barras": {
            "type": "string",
            "description": "Código de barras único por empresa"
          },
          "unidad": {
            "type": "string",
            "description": "Unidad de venta",
            "example": "Unidad (u)"
          },
          "permite_fracciones": {
            "type": "boolean",
            "description": "Permite fracciones"
          },
          "unidad_compra": {
            "type": "string",
            "description": "Unidad de compra"
          },
          "contenido_cantidad": {
            "type": "number",
            "description": "Contenido por unidad"
          },
          "contenido_unidad": {
            "type": "string",
            "description": "Unidad del contenido"
          },
          "tipo_precio": {
            "type": "string",
            "description": "Tipo de precio",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "tipo_costo": {
            "type": "string",
            "description": "Tipo de costo",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "stock_minimo": {
            "type": "number",
            "description": "Stock mínimo para alertas"
          },
          "stock_maximo": {
            "type": "object",
            "description": "Stock máximo",
            "nullable": true
          },
          "garantia_meses": {
            "type": "object",
            "description": "Garantía en meses",
            "nullable": true
          },
          "es_materia_prima": {
            "type": "boolean",
            "description": "Es materia prima"
          },
          "ubicacion": {
            "type": "string",
            "description": "Ubicación física"
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "producto_padre_id": {
            "type": "object",
            "description": "Producto padre si esta fila es una variante",
            "nullable": true,
            "format": "uuid"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos de la variante (color, talla, etc.)",
            "additionalProperties": true
          },
          "tiene_variantes": {
            "type": "boolean",
            "description": "El producto tiene variantes vendibles"
          },
          "imagenes": {
            "description": "Imágenes del producto (máx. 3)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ImagenProductoResponseDto"
            }
          },
          "variantes": {
            "description": "Variantes (solo en el detalle del padre)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/VarianteProductoResponseDto"
            }
          },
          "precio_desde": {
            "type": "string",
            "description": "Precio mínimo mostrable (\"desde\")",
            "example": "1000.00"
          },
          "stock_total": {
            "type": "number",
            "description": "Stock agregado de las variantes o el propio",
            "example": 25
          }
        },
        "required": [
          "empresa_id",
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
          "empresa_id": {
            "type": "string",
            "description": "Empresa dueña del producto (ADR-27)",
            "format": "uuid"
          },
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
      "ImagenProductoRequestDto": {
        "type": "object",
        "properties": {
          "objeto_id": {
            "type": "string",
            "description": "Id del objeto confirmado en storage-service",
            "format": "uuid"
          },
          "orden": {
            "type": "number",
            "description": "Orden de la imagen (0-2); si se omite se asigna el siguiente",
            "minimum": 0,
            "maximum": 2
          },
          "es_principal": {
            "type": "boolean",
            "description": "Marcar como imagen principal",
            "default": false
          }
        },
        "required": [
          "objeto_id"
        ]
      },
      "VarianteProductoRequestDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU de la variante",
            "pattern": "^[A-Za-z0-9-]{2,32}$",
            "example": "SKU-001-ROJO-M"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre visible de la variante"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos de la variante (color, talla, etc.)"
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base decimal (ej: 1000.00)",
            "example": "1500.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock inicial de la variante",
            "minimum": 0,
            "example": 5
          },
          "imagenes": {
            "description": "Imágenes de la variante (máx. 3)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ImagenProductoRequestDto"
            }
          }
        },
        "required": [
          "sku",
          "precio_base",
          "stock"
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
          "empresa_id": {
            "type": "string",
            "description": "Empresa destino. Obligatoria para super_admin/admin; el empresario usa su empresa activa.",
            "format": "uuid"
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
          },
          "costo": {
            "type": "string",
            "description": "Costo decimal en córdobas (ej: 150.00)",
            "example": "150.00"
          },
          "marca": {
            "type": "string",
            "description": "Marca"
          },
          "codigo_barras": {
            "type": "string",
            "description": "Código de barras único por empresa"
          },
          "unidad": {
            "type": "string",
            "description": "Unidad de venta",
            "example": "Unidad (u)"
          },
          "permite_fracciones": {
            "type": "boolean",
            "description": "Permite cantidades fraccionarias",
            "default": false
          },
          "unidad_compra": {
            "type": "string",
            "description": "Unidad de compra",
            "example": "Igual que la de venta"
          },
          "contenido_cantidad": {
            "type": "number",
            "description": "Contenido por unidad",
            "minimum": 0,
            "default": 0
          },
          "contenido_unidad": {
            "type": "string",
            "description": "Unidad del contenido",
            "example": "Sin contenido"
          },
          "tipo_precio": {
            "type": "string",
            "description": "Tipo de precio",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "tipo_costo": {
            "type": "string",
            "description": "Tipo de costo",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "stock_minimo": {
            "type": "number",
            "description": "Stock mínimo para alertas",
            "minimum": 0,
            "default": 0
          },
          "stock_maximo": {
            "type": "object",
            "description": "Stock máximo",
            "nullable": true,
            "minimum": 0
          },
          "garantia_meses": {
            "type": "object",
            "description": "Garantía en meses",
            "nullable": true,
            "minimum": 0
          },
          "es_materia_prima": {
            "type": "boolean",
            "description": "Es materia prima",
            "default": false
          },
          "ubicacion": {
            "type": "string",
            "description": "Ubicación física"
          },
          "estado_operativo": {
            "type": "string",
            "description": "Estado comercial",
            "enum": [
              "activo",
              "inactivo"
            ],
            "default": "activo"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos del producto (si aplica)"
          },
          "imagenes": {
            "description": "Imágenes del producto (máx. 3)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ImagenProductoRequestDto"
            }
          },
          "variantes": {
            "description": "Variantes vendibles del producto (máx. 50)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/VarianteProductoRequestDto"
            }
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
          "costo": {
            "type": "string",
            "description": "Costo decimal en córdobas (ej: 150.00)",
            "example": "150.00"
          },
          "marca": {
            "type": "string",
            "description": "Marca"
          },
          "codigo_barras": {
            "type": "string",
            "description": "Código de barras único por empresa"
          },
          "unidad": {
            "type": "string",
            "description": "Unidad de venta",
            "example": "Unidad (u)"
          },
          "permite_fracciones": {
            "type": "boolean",
            "description": "Permite cantidades fraccionarias"
          },
          "unidad_compra": {
            "type": "string",
            "description": "Unidad de compra",
            "example": "Igual que la de venta"
          },
          "contenido_cantidad": {
            "type": "number",
            "description": "Contenido por unidad",
            "minimum": 0,
            "default": 0
          },
          "contenido_unidad": {
            "type": "string",
            "description": "Unidad del contenido",
            "example": "Sin contenido"
          },
          "tipo_precio": {
            "type": "string",
            "description": "Tipo de precio",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "tipo_costo": {
            "type": "string",
            "description": "Tipo de costo",
            "enum": [
              "fijo",
              "escala"
            ]
          },
          "stock_minimo": {
            "type": "number",
            "description": "Stock mínimo para alertas",
            "minimum": 0,
            "default": 0
          },
          "stock_maximo": {
            "type": "object",
            "description": "Stock máximo",
            "nullable": true,
            "minimum": 0
          },
          "garantia_meses": {
            "type": "object",
            "description": "Garantía en meses",
            "nullable": true,
            "minimum": 0
          },
          "es_materia_prima": {
            "type": "boolean",
            "description": "Es materia prima"
          },
          "ubicacion": {
            "type": "string",
            "description": "Ubicación física"
          },
          "estado_operativo": {
            "type": "string",
            "description": "Estado comercial",
            "enum": [
              "activo",
              "inactivo"
            ],
            "default": "activo"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos de la variante"
          },
          "motivo": {
            "type": "string",
            "description": "Motivo del cambio (RN-08 histórico)"
          }
        }
      },
      "ServicioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "codigo": {
            "type": "string",
            "example": "SERV-001"
          },
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "categoria": {
            "type": "string"
          },
          "marca": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string",
            "example": "1500.00"
          },
          "costo": {
            "type": "string",
            "example": "500.00"
          },
          "unidad": {
            "type": "string",
            "example": "Hora (h)"
          },
          "duracion_minutos": {
            "type": "number",
            "example": 60
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "creado_en": {
            "type": "string",
            "format": "date-time"
          },
          "actualizado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "codigo",
          "nombre",
          "descripcion",
          "categoria",
          "marca",
          "codigo_barras",
          "precio",
          "costo",
          "unidad",
          "duracion_minutos",
          "estado",
          "creado_en",
          "actualizado_en"
        ]
      },
      "PaginaServiciosResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ServicioResponseDto"
            }
          },
          "total": {
            "type": "number"
          },
          "pagina": {
            "type": "number"
          },
          "limite": {
            "type": "number"
          },
          "paginas": {
            "type": "number"
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
      "CrearServicioRequestDto": {
        "type": "object",
        "properties": {
          "codigo": {
            "type": "string",
            "description": "Código interno del servicio",
            "example": "SERV-001"
          },
          "empresa_id": {
            "type": "string",
            "description": "Empresa destino (solo admin/super_admin)",
            "format": "uuid"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del servicio",
            "minLength": 2,
            "maxLength": 200
          },
          "descripcion": {
            "type": "string"
          },
          "categoria": {
            "type": "string"
          },
          "marca": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string",
            "description": "Precio decimal",
            "example": "1500.00"
          },
          "costo": {
            "type": "string",
            "description": "Costo decimal",
            "example": "500.00"
          },
          "unidad": {
            "type": "string",
            "example": "Hora (h)"
          },
          "duracion_minutos": {
            "type": "number",
            "description": "Duración en minutos (1-1440)",
            "minimum": 1,
            "maximum": 1440
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          }
        },
        "required": [
          "codigo",
          "nombre",
          "precio",
          "duracion_minutos"
        ]
      },
      "ActualizarServicioRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "minLength": 2
          },
          "descripcion": {
            "type": "string"
          },
          "categoria": {
            "type": "string"
          },
          "marca": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string"
          },
          "costo": {
            "type": "string"
          },
          "unidad": {
            "type": "string"
          },
          "duracion_minutos": {
            "type": "number",
            "minimum": 1,
            "maximum": 1440
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          }
        }
      },
      "ConfiguracionInventarioResponseDto": {
        "type": "object",
        "properties": {
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "metodo_costeo": {
            "type": "string",
            "enum": [
              "promedio",
              "fifo",
              "ueps"
            ]
          },
          "permitir_stock_negativo": {
            "type": "boolean"
          },
          "alerta_stock_bajo": {
            "type": "boolean"
          },
          "actualizar_costo_en_compra": {
            "type": "boolean"
          },
          "unidad_defecto": {
            "type": "string"
          },
          "actualizado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "empresa_id",
          "metodo_costeo",
          "permitir_stock_negativo",
          "alerta_stock_bajo",
          "actualizar_costo_en_compra",
          "unidad_defecto",
          "actualizado_en"
        ]
      },
      "ActualizarConfiguracionInventarioRequestDto": {
        "type": "object",
        "properties": {
          "metodo_costeo": {
            "type": "string",
            "enum": [
              "promedio",
              "fifo",
              "ueps"
            ]
          },
          "permitir_stock_negativo": {
            "type": "boolean"
          },
          "alerta_stock_bajo": {
            "type": "boolean"
          },
          "actualizar_costo_en_compra": {
            "type": "boolean"
          },
          "unidad_defecto": {
            "type": "string"
          }
        }
      },
      "ImportarProductoFilaDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string"
          },
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "categoria": {
            "type": "string"
          },
          "precio_base": {
            "type": "string",
            "example": "1000.00"
          },
          "stock": {
            "type": "number",
            "minimum": 0
          },
          "costo": {
            "type": "string"
          },
          "marca": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "unidad": {
            "type": "string"
          },
          "stock_minimo": {
            "type": "number",
            "minimum": 0
          },
          "stock_maximo": {
            "type": "object",
            "nullable": true,
            "minimum": 0
          },
          "es_materia_prima": {
            "type": "boolean"
          },
          "ubicacion": {
            "type": "string"
          }
        },
        "required": [
          "sku",
          "nombre",
          "precio_base",
          "stock"
        ]
      },
      "ImportarProductosRequestDto": {
        "type": "object",
        "properties": {
          "filas": {
            "description": "Máximo 200 filas",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ImportarProductoFilaDto"
            }
          }
        },
        "required": [
          "filas"
        ]
      },
      "ImportarProductosResponseDto": {
        "type": "object",
        "properties": {
          "importados": {
            "type": "number"
          },
          "omitidos": {
            "type": "number"
          },
          "errores": {
            "type": "array",
            "items": {
              "type": "string"
            }
          }
        },
        "required": [
          "importados",
          "omitidos",
          "errores"
        ]
      },
      "EscaneoResponseDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "enum": [
              "producto",
              "servicio"
            ]
          },
          "producto": {
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/ProductoResponseDto"
              }
            ]
          },
          "servicio": {
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/ServicioResponseDto"
              }
            ]
          }
        },
        "required": [
          "tipo"
        ]
      },
      "ComboComponenteResponseDto": {
        "type": "object",
        "properties": {
          "producto_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string"
          },
          "nombre": {
            "type": "string"
          },
          "cantidad": {
            "type": "number"
          },
          "costo_unitario": {
            "type": "string"
          },
          "subtotal": {
            "type": "string"
          }
        },
        "required": [
          "producto_id",
          "sku",
          "nombre",
          "cantidad",
          "costo_unitario",
          "subtotal"
        ]
      },
      "ComboResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string"
          },
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string"
          },
          "costo_total": {
            "type": "string"
          },
          "margen": {
            "type": "number"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "componentes": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ComboComponenteResponseDto"
            }
          },
          "creado_en": {
            "type": "string",
            "format": "date-time"
          },
          "actualizado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "sku",
          "nombre",
          "descripcion",
          "codigo_barras",
          "precio",
          "costo_total",
          "margen",
          "estado",
          "componentes",
          "creado_en",
          "actualizado_en"
        ]
      },
      "PaginaCombosResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ComboResponseDto"
            }
          },
          "total": {
            "type": "number"
          },
          "pagina": {
            "type": "number"
          },
          "limite": {
            "type": "number"
          },
          "paginas": {
            "type": "number"
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
      "ComboComponenteRequestDto": {
        "type": "object",
        "properties": {
          "producto_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto componente"
          },
          "cantidad": {
            "type": "number",
            "minimum": 1
          }
        },
        "required": [
          "sku",
          "cantidad"
        ]
      },
      "CrearComboRequestDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string"
          },
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string",
            "example": "2500.00"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "componentes": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ComboComponenteRequestDto"
            }
          }
        },
        "required": [
          "sku",
          "nombre",
          "precio",
          "componentes"
        ]
      },
      "ActualizarComboRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "codigo_barras": {
            "type": "string"
          },
          "precio": {
            "type": "string"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "componentes": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ComboComponenteRequestDto"
            }
          }
        }
      },
      "PromocionResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "2x1",
              "descuento_cantidad",
              "descuento_porcentaje",
              "regalo"
            ]
          },
          "producto_id": {
            "type": "object",
            "nullable": true
          },
          "sku": {
            "type": "string"
          },
          "cantidad_requerida": {
            "type": "object",
            "nullable": true
          },
          "cantidad_gratis": {
            "type": "object",
            "nullable": true
          },
          "descuento_porcentaje": {
            "type": "object",
            "nullable": true
          },
          "fecha_desde": {
            "type": "string",
            "format": "date"
          },
          "fecha_hasta": {
            "type": "string",
            "format": "date"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          },
          "creado_en": {
            "type": "string",
            "format": "date-time"
          },
          "actualizado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "nombre",
          "tipo",
          "producto_id",
          "sku",
          "cantidad_requerida",
          "cantidad_gratis",
          "descuento_porcentaje",
          "fecha_desde",
          "fecha_hasta",
          "estado",
          "creado_en",
          "actualizado_en"
        ]
      },
      "PaginaPromocionesResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/PromocionResponseDto"
            }
          },
          "total": {
            "type": "number"
          },
          "pagina": {
            "type": "number"
          },
          "limite": {
            "type": "number"
          },
          "paginas": {
            "type": "number"
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
      "CrearPromocionRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "2x1",
              "descuento_cantidad",
              "descuento_porcentaje",
              "regalo"
            ]
          },
          "producto_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string"
          },
          "cantidad_requerida": {
            "type": "object",
            "minimum": 1
          },
          "cantidad_gratis": {
            "type": "object",
            "minimum": 1
          },
          "descuento_porcentaje": {
            "type": "object",
            "minimum": 1,
            "maximum": 100
          },
          "fecha_desde": {
            "type": "string",
            "format": "date"
          },
          "fecha_hasta": {
            "type": "string",
            "format": "date"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          }
        },
        "required": [
          "nombre",
          "tipo",
          "fecha_desde",
          "fecha_hasta"
        ]
      },
      "ActualizarPromocionRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "2x1",
              "descuento_cantidad",
              "descuento_porcentaje",
              "regalo"
            ]
          },
          "producto_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string"
          },
          "cantidad_requerida": {
            "type": "object",
            "minimum": 1
          },
          "cantidad_gratis": {
            "type": "object",
            "minimum": 1
          },
          "descuento_porcentaje": {
            "type": "object",
            "minimum": 1,
            "maximum": 100
          },
          "fecha_desde": {
            "type": "string",
            "format": "date"
          },
          "fecha_hasta": {
            "type": "string",
            "format": "date"
          },
          "estado": {
            "type": "string",
            "enum": [
              "activo",
              "inactivo"
            ]
          }
        }
      },
      "MovimientoInventarioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "tipo": {
            "type": "string",
            "enum": [
              "entrada",
              "salida",
              "ajuste",
              "traslado"
            ]
          },
          "motivo": {
            "type": "string"
          },
          "referencia": {
            "type": "string"
          },
          "ubicacion_origen": {
            "type": "string"
          },
          "ubicacion_destino": {
            "type": "string"
          },
          "lineas": {
            "type": "array",
            "items": {
              "type": "object"
            }
          },
          "creado_por": {
            "type": "object",
            "nullable": true
          },
          "creado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "tipo",
          "motivo",
          "referencia",
          "ubicacion_origen",
          "ubicacion_destino",
          "lineas",
          "creado_por",
          "creado_en"
        ]
      },
      "PaginaMovimientosResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/MovimientoInventarioResponseDto"
            }
          },
          "total": {
            "type": "number"
          },
          "pagina": {
            "type": "number"
          },
          "limite": {
            "type": "number"
          },
          "paginas": {
            "type": "number"
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
      "MovimientoLineaRequestDto": {
        "type": "object",
        "properties": {
          "producto_id": {
            "type": "string",
            "format": "uuid"
          },
          "sku": {
            "type": "string"
          },
          "cantidad": {
            "type": "number",
            "minimum": 0
          }
        },
        "required": [
          "sku",
          "cantidad"
        ]
      },
      "CrearMovimientoInventarioRequestDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string",
            "enum": [
              "entrada",
              "salida",
              "ajuste",
              "traslado"
            ]
          },
          "motivo": {
            "type": "string"
          },
          "referencia": {
            "type": "string"
          },
          "ubicacion_origen": {
            "type": "string"
          },
          "ubicacion_destino": {
            "type": "string"
          },
          "lineas": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/MovimientoLineaRequestDto"
            }
          }
        },
        "required": [
          "tipo",
          "lineas"
        ]
      },
      "LiquidacionResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la liquidación",
            "format": "uuid"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "periodo_inicio": {
            "type": "string",
            "description": "Inicio del periodo (ISO 8601)",
            "format": "date"
          },
          "periodo_fin": {
            "type": "string",
            "description": "Fin del periodo (ISO 8601)",
            "format": "date"
          },
          "monto_cents": {
            "type": "number",
            "description": "Monto liquidado en centavos",
            "example": 13800
          },
          "estado": {
            "type": "string",
            "description": "Estado de la liquidación",
            "enum": [
              "aprobada",
              "pagada"
            ],
            "example": "aprobada"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "vendedor_id",
          "periodo_inicio",
          "periodo_fin",
          "monto_cents",
          "estado",
          "creado_en"
        ]
      },
      "LiquidacionEmpresaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la liquidación",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "description": "Id de la empresa beneficiaria",
            "format": "uuid"
          },
          "periodo_inicio": {
            "type": "string",
            "description": "Inicio del periodo (ISO 8601)",
            "format": "date"
          },
          "periodo_fin": {
            "type": "string",
            "description": "Fin del periodo (ISO 8601)",
            "format": "date"
          },
          "monto_cents": {
            "type": "number",
            "description": "Monto liquidado en centavos",
            "example": 4400
          },
          "estado": {
            "type": "string",
            "description": "Estado de la liquidación",
            "enum": [
              "aprobada",
              "pagada"
            ]
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "periodo_inicio",
          "periodo_fin",
          "monto_cents",
          "estado",
          "creado_en"
        ]
      },
      "VentaVendedorResponseDto": {
        "type": "object",
        "properties": {
          "order_id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "venta": {
            "type": "string",
            "description": "Monto de la venta en córdobas con dos decimales",
            "example": "1150.00"
          },
          "comision": {
            "type": "string",
            "description": "Comisión en córdobas con dos decimales",
            "example": "138.00"
          },
          "monto_vendedor": {
            "type": "string",
            "description": "Monto neto para el vendedor",
            "example": "1012.00"
          },
          "estado": {
            "type": "string",
            "description": "Estado de la comisión",
            "example": "acreditada"
          },
          "fecha": {
            "type": "string",
            "description": "Fecha de la venta (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "order_id",
          "venta",
          "comision",
          "monto_vendedor",
          "estado",
          "fecha"
        ]
      },
      "LiquidacionConMontoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la liquidación",
            "format": "uuid"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "periodo_inicio": {
            "type": "string",
            "description": "Inicio del periodo (ISO 8601)",
            "format": "date"
          },
          "periodo_fin": {
            "type": "string",
            "description": "Fin del periodo (ISO 8601)",
            "format": "date"
          },
          "monto_cents": {
            "type": "number",
            "description": "Monto liquidado en centavos",
            "example": 13800
          },
          "estado": {
            "type": "string",
            "description": "Estado de la liquidación",
            "enum": [
              "aprobada",
              "pagada"
            ],
            "example": "aprobada"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "monto": {
            "type": "string",
            "description": "Monto liquidado en córdobas con dos decimales",
            "example": "138.00"
          }
        },
        "required": [
          "id",
          "vendedor_id",
          "periodo_inicio",
          "periodo_fin",
          "monto_cents",
          "estado",
          "creado_en",
          "monto"
        ]
      },
      "EstadoCuentaResponseDto": {
        "type": "object",
        "properties": {
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "ventas": {
            "description": "Ventas del vendedor",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/VentaVendedorResponseDto"
            }
          },
          "comisiones_pendientes": {
            "type": "string",
            "description": "Comisiones pendientes en córdobas",
            "example": "138.00"
          },
          "liquidaciones": {
            "description": "Liquidaciones del vendedor",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LiquidacionConMontoResponseDto"
            }
          }
        },
        "required": [
          "vendedor_id",
          "ventas",
          "comisiones_pendientes",
          "liquidaciones"
        ]
      },
      "VentaEmpresaResponseDto": {
        "type": "object",
        "properties": {
          "order_id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "venta": {
            "type": "string",
            "description": "Monto de la venta en córdobas con dos decimales",
            "example": "1150.00"
          },
          "comision": {
            "type": "string",
            "description": "Comisión total en córdobas con dos decimales",
            "example": "138.00"
          },
          "monto_empresario": {
            "type": "string",
            "description": "Monto neto para la empresa",
            "example": "126.00"
          },
          "estado": {
            "type": "string",
            "description": "Estado de la comisión",
            "example": "acreditada"
          },
          "fecha": {
            "type": "string",
            "description": "Fecha de la venta (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "order_id",
          "venta",
          "comision",
          "monto_empresario",
          "estado",
          "fecha"
        ]
      },
      "LiquidacionEmpresaConMontoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la liquidación",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "description": "Id de la empresa beneficiaria",
            "format": "uuid"
          },
          "periodo_inicio": {
            "type": "string",
            "description": "Inicio del periodo (ISO 8601)",
            "format": "date"
          },
          "periodo_fin": {
            "type": "string",
            "description": "Fin del periodo (ISO 8601)",
            "format": "date"
          },
          "monto_cents": {
            "type": "number",
            "description": "Monto liquidado en centavos",
            "example": 4400
          },
          "estado": {
            "type": "string",
            "description": "Estado de la liquidación",
            "enum": [
              "aprobada",
              "pagada"
            ]
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "monto": {
            "type": "string",
            "description": "Monto liquidado en córdobas con dos decimales",
            "example": "4400.00"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "periodo_inicio",
          "periodo_fin",
          "monto_cents",
          "estado",
          "creado_en",
          "monto"
        ]
      },
      "EstadoCuentaEmpresaResponseDto": {
        "type": "object",
        "properties": {
          "empresa_id": {
            "type": "string",
            "description": "Id de la empresa",
            "format": "uuid"
          },
          "ventas": {
            "description": "Ventas de la empresa",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/VentaEmpresaResponseDto"
            }
          },
          "comisiones_pendientes": {
            "type": "string",
            "description": "Comisiones pendientes en córdobas",
            "example": "126.00"
          },
          "liquidaciones": {
            "description": "Liquidaciones de la empresa",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LiquidacionEmpresaConMontoResponseDto"
            }
          }
        },
        "required": [
          "empresa_id",
          "ventas",
          "comisiones_pendientes",
          "liquidaciones"
        ]
      },
      "ReporteKpiResponseDto": {
        "type": "object",
        "properties": {
          "gmv_mes_cents": {
            "type": "number",
            "description": "GMV del mes en centavos",
            "example": 1150000
          },
          "gmv_mes": {
            "type": "string",
            "description": "GMV del mes en córdobas con dos decimales",
            "example": "11500.00"
          },
          "pedidos_mes": {
            "type": "number",
            "description": "Pedidos del mes",
            "example": 42
          },
          "comisiones_mes_cents": {
            "type": "number",
            "description": "Comisiones del mes en centavos",
            "example": 138000
          },
          "comisiones_mes": {
            "type": "string",
            "description": "Comisiones del mes en córdobas con dos decimales",
            "example": "1380.00"
          },
          "vendedores_activos": {
            "type": "number",
            "description": "Vendedores activos",
            "example": 12
          },
          "productos": {
            "type": "number",
            "description": "Productos en catálogo",
            "example": 20
          },
          "stock_total": {
            "type": "number",
            "description": "Unidades en stock",
            "example": 120
          },
          "agotados": {
            "type": "number",
            "description": "Productos agotados",
            "example": 2
          },
          "devoluciones_mes": {
            "type": "number",
            "description": "Devoluciones del mes",
            "example": 1
          }
        },
        "required": [
          "gmv_mes_cents",
          "gmv_mes",
          "pedidos_mes",
          "comisiones_mes_cents",
          "comisiones_mes",
          "vendedores_activos",
          "productos",
          "stock_total",
          "agotados",
          "devoluciones_mes"
        ]
      },
      "PersonalResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "apellido": {
            "type": "string"
          },
          "cargo": {
            "type": "string"
          },
          "estado": {
            "type": "string"
          },
          "telefono": {
            "type": "object",
            "nullable": true
          },
          "email": {
            "type": "object",
            "nullable": true
          },
          "ruta_asignada_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "vehiculo_asignado_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "hora_check_in": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "hora_check_out": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "ubicacion_lat": {
            "type": "object",
            "nullable": true
          },
          "ubicacion_lng": {
            "type": "object",
            "nullable": true
          },
          "ubicacion_precision": {
            "type": "object",
            "nullable": true
          },
          "ubicacion_ts": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "nombre",
          "apellido",
          "cargo",
          "estado",
          "synced",
          "created_at",
          "updated_at"
        ]
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
      "UbicacionResponseDto": {
        "type": "object",
        "properties": {
          "personalId": {
            "type": "string",
            "format": "uuid"
          },
          "latitud": {
            "type": "object",
            "nullable": true
          },
          "longitud": {
            "type": "object",
            "nullable": true
          },
          "precision": {
            "type": "object",
            "nullable": true
          },
          "timestamp": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          }
        },
        "required": [
          "personalId",
          "latitud",
          "longitud",
          "precision",
          "timestamp"
        ]
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
      "ClienteResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre_completo": {
            "type": "string"
          },
          "tipo_documento": {
            "type": "object",
            "nullable": true
          },
          "numero_documento": {
            "type": "object",
            "nullable": true
          },
          "tipo_cliente": {
            "type": "string"
          },
          "email": {
            "type": "object",
            "nullable": true
          },
          "telefono": {
            "type": "object",
            "nullable": true
          },
          "telefono_alternativo": {
            "type": "object",
            "nullable": true
          },
          "direccion_principal": {
            "type": "object",
            "nullable": true
          },
          "direccion_secundaria": {
            "type": "object",
            "nullable": true
          },
          "referencias_direccion": {
            "type": "object",
            "nullable": true
          },
          "lat": {
            "type": "object",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "nullable": true
          },
          "notas_adicionales": {
            "type": "object",
            "nullable": true
          },
          "url_google_maps": {
            "type": "object",
            "nullable": true
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "nombre_completo",
          "tipo_cliente",
          "synced",
          "created_at",
          "updated_at"
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
      "VehiculoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "placa": {
            "type": "string"
          },
          "tipo": {
            "type": "string"
          },
          "marca": {
            "type": "object",
            "nullable": true
          },
          "modelo": {
            "type": "object",
            "nullable": true
          },
          "anio": {
            "type": "object",
            "nullable": true
          },
          "estado": {
            "type": "string"
          },
          "color": {
            "type": "object",
            "nullable": true
          },
          "capacidad_carga_kg": {
            "type": "object",
            "nullable": true
          },
          "numero_chasis": {
            "type": "object",
            "nullable": true
          },
          "numero_motor": {
            "type": "object",
            "nullable": true
          },
          "tipo_combustible": {
            "type": "object",
            "nullable": true
          },
          "vencimiento_seguro": {
            "type": "object",
            "nullable": true,
            "format": "date"
          },
          "vencimiento_circulacion": {
            "type": "object",
            "nullable": true,
            "format": "date"
          },
          "notas_adicionales": {
            "type": "object",
            "nullable": true
          },
          "conductor_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "ruta_activa_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "placa",
          "tipo",
          "estado",
          "synced",
          "created_at",
          "updated_at"
        ]
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
      "ParadaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "ruta_id": {
            "type": "string",
            "format": "uuid"
          },
          "orden": {
            "type": "number"
          },
          "nombre": {
            "type": "string"
          },
          "direccion": {
            "type": "object",
            "nullable": true
          },
          "lat": {
            "type": "object",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "nullable": true
          },
          "completada": {
            "type": "boolean"
          },
          "pedido_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "tipo": {
            "type": "string"
          },
          "cliente_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "telefono": {
            "type": "object",
            "nullable": true
          },
          "referencias": {
            "type": "object",
            "nullable": true
          },
          "horario_atencion": {
            "type": "object",
            "nullable": true
          },
          "notas": {
            "type": "object",
            "nullable": true
          },
          "tipo_combustible": {
            "type": "object",
            "nullable": true
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "ruta_id",
          "orden",
          "nombre",
          "completada",
          "tipo",
          "synced",
          "created_at",
          "updated_at"
        ]
      },
      "RutaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "descripcion": {
            "type": "object",
            "nullable": true
          },
          "estado": {
            "type": "string"
          },
          "personal_asignado_ids": {
            "type": "array",
            "items": {
              "type": "string"
            }
          },
          "vehiculo_asignado_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "fecha_inicio": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "fecha_fin": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          },
          "paradas": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ParadaResponseDto"
            }
          }
        },
        "required": [
          "id",
          "tenant_id",
          "nombre",
          "estado",
          "personal_asignado_ids",
          "synced",
          "created_at",
          "updated_at"
        ]
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
      "PedidoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "cliente": {
            "type": "string"
          },
          "cliente_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "direccion_entrega": {
            "type": "object",
            "nullable": true
          },
          "lat": {
            "type": "object",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "nullable": true
          },
          "estado": {
            "type": "string"
          },
          "ruta_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "notas": {
            "type": "object",
            "nullable": true
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "cliente",
          "estado",
          "synced",
          "created_at",
          "updated_at"
        ]
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
      "AsistenciaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "format": "uuid"
          },
          "tipo": {
            "type": "string"
          },
          "registrado_en": {
            "type": "string",
            "format": "date-time"
          },
          "lat": {
            "type": "object",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "nullable": true
          },
          "estado_puntualidad": {
            "type": "string"
          },
          "en_sede": {
            "type": "boolean"
          },
          "hora_real_llegada": {
            "type": "object",
            "nullable": true
          },
          "minutos_retraso_real": {
            "type": "object",
            "nullable": true
          },
          "monitoreo_activo": {
            "type": "boolean"
          },
          "notas": {
            "type": "object",
            "nullable": true
          },
          "justificacion": {
            "type": "object",
            "nullable": true
          },
          "fotos_justificacion": {
            "type": "array",
            "items": {
              "type": "object"
            }
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "personal_id",
          "tipo",
          "registrado_en",
          "estado_puntualidad",
          "en_sede",
          "monitoreo_activo",
          "fotos_justificacion",
          "synced",
          "created_at",
          "updated_at"
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
      "IncidenciaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "tipo": {
            "type": "string"
          },
          "estado": {
            "type": "string"
          },
          "descripcion": {
            "type": "string"
          },
          "personal_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "vehiculo_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "ruta_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "resolucion": {
            "type": "object",
            "nullable": true
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "tipo",
          "estado",
          "descripcion",
          "synced",
          "created_at",
          "updated_at"
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
      "TrackingResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "format": "uuid"
          },
          "latitud": {
            "type": "number"
          },
          "longitud": {
            "type": "number"
          },
          "precision": {
            "type": "object",
            "nullable": true
          },
          "velocidad": {
            "type": "object",
            "nullable": true
          },
          "rumbo": {
            "type": "object",
            "nullable": true
          },
          "registrado_en": {
            "type": "string",
            "format": "date-time"
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "personal_id",
          "latitud",
          "longitud",
          "registrado_en",
          "synced",
          "created_at",
          "updated_at"
        ]
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
      "VisitaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "format": "uuid"
          },
          "cliente_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "ruta_id": {
            "type": "object",
            "nullable": true,
            "format": "uuid"
          },
          "tipo_actividad": {
            "type": "string"
          },
          "resultado": {
            "type": "string"
          },
          "lat": {
            "type": "object",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "nullable": true
          },
          "distancia_al_cliente": {
            "type": "object",
            "nullable": true
          },
          "hora_llegada": {
            "type": "object",
            "nullable": true
          },
          "hora_salida": {
            "type": "object",
            "nullable": true
          },
          "duracion_minutos": {
            "type": "object",
            "nullable": true
          },
          "notas": {
            "type": "object",
            "nullable": true
          },
          "synced": {
            "type": "boolean"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "personal_id",
          "tipo_actividad",
          "resultado",
          "synced",
          "created_at",
          "updated_at"
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
      "CumplimientoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "format": "uuid"
          },
          "ruta_id": {
            "type": "string",
            "format": "uuid"
          },
          "fecha": {
            "type": "string",
            "format": "date"
          },
          "metricas": {
            "type": "object"
          },
          "deleted_at": {
            "type": "object",
            "nullable": true,
            "format": "date-time"
          },
          "created_at": {
            "type": "string",
            "format": "date-time"
          },
          "updated_at": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "ruta_id",
          "fecha",
          "metricas",
          "created_at",
          "updated_at"
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
      "SyncResultadoResponseDto": {
        "type": "object",
        "properties": {
          "tipo": {
            "type": "string"
          },
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "ok": {
            "type": "boolean"
          },
          "error": {
            "type": "string"
          }
        },
        "required": [
          "tipo",
          "ok"
        ]
      },
      "SujetoTributarioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del sujeto",
            "format": "uuid"
          },
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción",
            "example": "NI"
          },
          "regimen_id": {
            "type": "string",
            "description": "Id del régimen",
            "format": "uuid"
          },
          "regimen_nombre": {
            "type": "string",
            "description": "Nombre del régimen"
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
            "description": "¿Es la plataforma?",
            "example": false
          },
          "estado": {
            "type": "string",
            "description": "Estado",
            "enum": [
              "ACTIVO",
              "BAJA"
            ],
            "example": "ACTIVO"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "jurisdiccion",
          "razon_social",
          "es_plataforma",
          "estado",
          "creado_en"
        ]
      },
      "DeclaracionResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la declaración",
            "format": "uuid"
          },
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción",
            "example": "NI"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo",
            "enum": [
              "IVA",
              "IR",
              "CUOTA_FIJA"
            ],
            "example": "IVA"
          },
          "periodo_inicio": {
            "type": "string",
            "description": "Inicio del periodo",
            "format": "date"
          },
          "periodo_fin": {
            "type": "string",
            "description": "Fin del periodo",
            "format": "date"
          },
          "base_gravada_cents": {
            "type": "number",
            "description": "Base gravada en centavos",
            "example": 1150000
          },
          "iva_debitado_cents": {
            "type": "number",
            "description": "IVA debitado en centavos",
            "example": 172500
          },
          "iva_credito_cents": {
            "type": "number",
            "description": "IVA crédito en centavos",
            "example": 0
          },
          "iva_a_pagar_cents": {
            "type": "number",
            "description": "IVA a pagar en centavos",
            "example": 172500
          },
          "renta_bruta_cents": {
            "type": "number",
            "description": "Renta bruta en centavos",
            "example": 1150000
          },
          "renta_gravable_cents": {
            "type": "number",
            "description": "Renta gravable en centavos",
            "example": 575000
          },
          "ir_a_pagar_cents": {
            "type": "number",
            "description": "IR a pagar en centavos",
            "example": 172500
          },
          "cuota_cents": {
            "type": "number",
            "description": "Cuota fija en centavos",
            "example": 5000
          },
          "estado": {
            "type": "string",
            "description": "Estado",
            "enum": [
              "PENDIENTE",
              "GENERADA",
              "PRESENTADA",
              "PAGADA",
              "CANCELADA"
            ],
            "example": "GENERADA"
          },
          "detalle": {
            "type": "object",
            "description": "Detalle de la declaración",
            "nullable": true
          },
          "generada_en": {
            "type": "string",
            "description": "Fecha de generación",
            "format": "date-time"
          },
          "presentada_en": {
            "type": "string",
            "description": "Fecha de presentación",
            "format": "date-time"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "jurisdiccion",
          "tipo",
          "periodo_inicio",
          "periodo_fin",
          "base_gravada_cents",
          "iva_debitado_cents",
          "iva_credito_cents",
          "iva_a_pagar_cents",
          "renta_bruta_cents",
          "renta_gravable_cents",
          "ir_a_pagar_cents",
          "cuota_cents",
          "estado",
          "creado_en"
        ]
      },
      "CuentaResponseDto": {
        "type": "object",
        "properties": {
          "codigo": {
            "type": "string",
            "description": "Código de la cuenta",
            "example": "1101"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre de la cuenta",
            "example": "Caja"
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
            ],
            "example": "ACTIVO"
          },
          "naturaleza": {
            "type": "string",
            "description": "Naturaleza",
            "enum": [
              "DEUDORA",
              "ACREEDORA"
            ],
            "example": "DEUDORA"
          },
          "nivel": {
            "type": "number",
            "description": "Nivel jerárquico",
            "example": 1
          },
          "estado": {
            "type": "string",
            "description": "Estado",
            "enum": [
              "activa",
              "inactiva"
            ],
            "example": "activa"
          }
        },
        "required": [
          "codigo",
          "nombre",
          "tipo",
          "naturaleza",
          "nivel",
          "estado"
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
      "DetalleAsientoResponseDto": {
        "type": "object",
        "properties": {
          "cuenta_codigo": {
            "type": "string",
            "description": "Código de cuenta contable"
          },
          "debe_cents": {
            "type": "number",
            "description": "Débito en centavos",
            "example": 115000
          },
          "haber_cents": {
            "type": "number",
            "description": "Crédito en centavos",
            "example": 115000
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
      "AsientoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del asiento",
            "format": "uuid"
          },
          "fecha": {
            "type": "string",
            "description": "Fecha del asiento",
            "format": "date-time"
          },
          "concepto": {
            "type": "string",
            "description": "Concepto del asiento"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo de asiento",
            "example": "MANUAL"
          },
          "referencia_tipo": {
            "type": "string",
            "description": "Tipo de referencia"
          },
          "referencia_id": {
            "type": "string",
            "description": "Id de referencia"
          },
          "moneda": {
            "type": "string",
            "description": "Moneda",
            "example": "C$"
          },
          "estado": {
            "type": "string",
            "description": "Estado del asiento",
            "example": "registrado"
          },
          "creado_por": {
            "type": "string",
            "description": "Usuario que creó el asiento"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "debe_cents": {
            "type": "number",
            "description": "Total débito en centavos",
            "example": 115000
          },
          "haber_cents": {
            "type": "number",
            "description": "Total crédito en centavos",
            "example": 115000
          },
          "detalles": {
            "description": "Detalles del asiento",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/DetalleAsientoResponseDto"
            }
          }
        },
        "required": [
          "id",
          "fecha",
          "concepto",
          "tipo",
          "moneda",
          "estado",
          "creado_en",
          "debe_cents",
          "haber_cents",
          "detalles"
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
      "LibroMayorResponseDto": {
        "type": "object",
        "properties": {
          "cuenta": {
            "description": "Cuenta contable",
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/CuentaResponseDto"
              }
            ]
          },
          "saldo_inicial_cents": {
            "type": "number",
            "description": "Saldo inicial en centavos",
            "example": 100000
          },
          "saldo_final_cents": {
            "type": "number",
            "description": "Saldo final en centavos",
            "example": 215000
          },
          "movimientos": {
            "description": "Movimientos del periodo",
            "type": "array",
            "items": {
              "type": "object"
            }
          }
        },
        "required": [
          "cuenta",
          "saldo_inicial_cents",
          "saldo_final_cents",
          "movimientos"
        ]
      },
      "LibroIvaResponseDto": {
        "type": "object",
        "properties": {
          "periodo": {
            "type": "string",
            "description": "Periodo (YYYY-MM)",
            "example": "2026-10"
          },
          "comprobantes": {
            "type": "number",
            "description": "Comprobantes del periodo",
            "example": 12
          },
          "base_gravada_cents": {
            "type": "number",
            "description": "Base gravada en centavos",
            "example": 1150000
          },
          "iva_cents": {
            "type": "number",
            "description": "IVA en centavos",
            "example": 172500
          },
          "exento_cents": {
            "type": "number",
            "description": "Monto exento en centavos",
            "example": 0
          },
          "total_cents": {
            "type": "number",
            "description": "Total en centavos",
            "example": 1322500
          }
        },
        "required": [
          "periodo",
          "comprobantes",
          "base_gravada_cents",
          "iva_cents",
          "exento_cents",
          "total_cents"
        ]
      },
      "ComprobanteResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del comprobante",
            "format": "uuid"
          },
          "serie": {
            "type": "string",
            "description": "Serie",
            "example": "A"
          },
          "numero": {
            "type": "string",
            "description": "Número",
            "example": "000001"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo",
            "enum": [
              "FACTURA",
              "NOTA_CREDITO"
            ],
            "example": "FACTURA"
          },
          "orden_id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "cliente_id": {
            "type": "string",
            "description": "Id del cliente"
          },
          "base_gravada_cents": {
            "type": "number",
            "description": "Base gravada en centavos",
            "example": 1150000
          },
          "iva_cents": {
            "type": "number",
            "description": "IVA en centavos",
            "example": 172500
          },
          "exento_cents": {
            "type": "number",
            "description": "Monto exento en centavos",
            "example": 0
          },
          "total_cents": {
            "type": "number",
            "description": "Total en centavos",
            "example": 1322500
          },
          "moneda": {
            "type": "string",
            "description": "Moneda",
            "example": "C$"
          },
          "estado": {
            "type": "string",
            "description": "Estado",
            "enum": [
              "BORRADOR",
              "EMITIDO",
              "ANULADO"
            ],
            "example": "EMITIDO"
          },
          "datos_cliente": {
            "type": "object",
            "description": "Datos fiscales del cliente",
            "nullable": true
          },
          "emitido_en": {
            "type": "string",
            "description": "Fecha de emisión",
            "format": "date-time"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "serie",
          "numero",
          "tipo",
          "base_gravada_cents",
          "iva_cents",
          "exento_cents",
          "total_cents",
          "moneda",
          "estado",
          "creado_en"
        ]
      },
      "SerieComprobanteResponseDto": {
        "type": "object",
        "properties": {
          "serie": {
            "type": "string",
            "description": "Serie",
            "example": "A"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo",
            "enum": [
              "FACTURA",
              "NOTA_CREDITO"
            ],
            "example": "FACTURA"
          },
          "prefijo": {
            "type": "string",
            "description": "Prefijo",
            "example": "FAC"
          },
          "secuencial_actual": {
            "type": "number",
            "description": "Secuencial actual",
            "example": 12
          },
          "ruc_emisor": {
            "type": "string",
            "description": "RUC del emisor",
            "example": "J0310000000000"
          },
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción",
            "example": "NI"
          },
          "activa": {
            "type": "boolean",
            "description": "Serie activa",
            "example": true
          }
        },
        "required": [
          "serie",
          "tipo",
          "prefijo",
          "secuencial_actual",
          "ruc_emisor",
          "jurisdiccion",
          "activa"
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
      "JurisdiccionResponseDto": {
        "type": "object",
        "properties": {
          "codigo_pais": {
            "type": "string",
            "description": "Código ISO del país",
            "example": "NI"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre de la jurisdicción",
            "example": "Nicaragua"
          },
          "moneda": {
            "type": "string",
            "description": "Moneda",
            "example": "NIO"
          },
          "simbolo_moneda": {
            "type": "string",
            "description": "Símbolo de la moneda",
            "example": "C$"
          },
          "tasa_iva": {
            "type": "number",
            "description": "Tasa de IVA (0–1)",
            "example": 0.15
          },
          "tasa_ir": {
            "type": "number",
            "description": "Tasa de IR (0–1)",
            "example": 0.3
          },
          "periodicidad_declaracion": {
            "type": "string",
            "description": "Periodicidad de declaración",
            "example": "MENSUAL"
          },
          "leyes": {
            "type": "object",
            "description": "Leyes aplicables (clave→texto)",
            "additionalProperties": {
              "type": "string"
            }
          },
          "activa": {
            "type": "boolean",
            "description": "Jurisdicción activa",
            "example": true
          }
        },
        "required": [
          "codigo_pais",
          "nombre",
          "moneda",
          "simbolo_moneda",
          "tasa_iva",
          "tasa_ir",
          "periodicidad_declaracion",
          "leyes",
          "activa"
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
      "RegimenFiscalResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del régimen",
            "format": "uuid"
          },
          "jurisdiccion": {
            "type": "string",
            "description": "Jurisdicción",
            "example": "NI"
          },
          "codigo": {
            "type": "string",
            "description": "Código del régimen",
            "example": "CUOTA_FIJA"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del régimen",
            "example": "Cuota Fija"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción"
          },
          "periodicidad": {
            "type": "string",
            "description": "Periodicidad",
            "example": "MENSUAL"
          },
          "condicion_ingresos_anuales_cents": {
            "type": "number",
            "description": "Condición de ingresos anuales en centavos"
          },
          "estado": {
            "type": "string",
            "description": "Estado",
            "enum": [
              "activo",
              "inactivo"
            ],
            "example": "activo"
          }
        },
        "required": [
          "id",
          "jurisdiccion",
          "codigo",
          "nombre",
          "descripcion",
          "periodicidad",
          "estado"
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
      "SupuestosProyeccionResponseDto": {
        "type": "object",
        "properties": {
          "horizonte_meses": {
            "type": "number",
            "example": 12
          },
          "vendedores_iniciales": {
            "type": "number",
            "example": 20
          },
          "entrada_vendedores_mes": {
            "type": "number",
            "example": 5
          },
          "churn_tasa": {
            "type": "number",
            "example": 0
          },
          "pedidos_por_vendedor": {
            "example": [
              25,
              27,
              29,
              31,
              33,
              35,
              37,
              38,
              39,
              40,
              41,
              42
            ],
            "type": "array",
            "items": {
              "type": "number"
            }
          },
          "ticket_promedio_cents": {
            "type": "number",
            "example": 45000
          },
          "comision_tasa": {
            "type": "number",
            "example": 0.12
          },
          "costos_fijos_cents": {
            "type": "number",
            "example": 200000
          },
          "costos_fijos_desde_mes7_cents": {
            "type": "number",
            "example": 300000
          },
          "inversion_inicial_cents": {
            "type": "number",
            "example": 5850000
          },
          "tasa_descuento_mensual": {
            "type": "number",
            "example": 0.015
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
      "FilaProyeccionResponseDto": {
        "type": "object",
        "properties": {
          "mes": {
            "type": "number"
          },
          "vendedores": {
            "type": "number"
          },
          "pedidos": {
            "type": "number"
          },
          "gmv_cents": {
            "type": "number"
          },
          "ingresos_cents": {
            "type": "number"
          },
          "costos_cents": {
            "type": "number"
          },
          "flujo_cents": {
            "type": "number"
          },
          "flujo_acumulado_cents": {
            "type": "number"
          },
          "costo_fijo_por_pedido_cents": {
            "type": "number"
          }
        },
        "required": [
          "mes",
          "vendedores",
          "pedidos",
          "gmv_cents",
          "ingresos_cents",
          "costos_cents",
          "flujo_cents",
          "flujo_acumulado_cents",
          "costo_fijo_por_pedido_cents"
        ]
      },
      "TotalesProyeccionResponseDto": {
        "type": "object",
        "properties": {
          "gmv_cents": {
            "type": "number"
          },
          "ingresos_cents": {
            "type": "number"
          },
          "costos_cents": {
            "type": "number"
          },
          "flujo_cents": {
            "type": "number"
          }
        },
        "required": [
          "gmv_cents",
          "ingresos_cents",
          "costos_cents",
          "flujo_cents"
        ]
      },
      "EscenarioEquilibrioResponseDto": {
        "type": "object",
        "properties": {
          "escenario": {
            "type": "string",
            "enum": [
              "bajo",
              "medio",
              "alto"
            ]
          },
          "costo_variable_por_pedido_cents": {
            "type": "number"
          },
          "contribucion_por_pedido_cents": {
            "type": "number"
          },
          "pedidos_equilibrio": {
            "type": "number"
          },
          "gmv_equilibrio_cents": {
            "type": "number"
          }
        },
        "required": [
          "escenario",
          "costo_variable_por_pedido_cents",
          "contribucion_por_pedido_cents",
          "pedidos_equilibrio",
          "gmv_equilibrio_cents"
        ]
      },
      "MargenSeguridadResponseDto": {
        "type": "object",
        "properties": {
          "mes": {
            "type": "number"
          },
          "gmv_cents": {
            "type": "number"
          },
          "margen_seguridad": {
            "type": "number"
          }
        },
        "required": [
          "mes",
          "gmv_cents",
          "margen_seguridad"
        ]
      },
      "CoberturaPorVendedorResponseDto": {
        "type": "object",
        "properties": {
          "pedidos_por_vendedor_mes": {
            "type": "number"
          },
          "comision_por_pedido_cents": {
            "type": "number"
          },
          "comision_bruta_vendedor_cents": {
            "type": "number"
          },
          "contribucion_media_cents": {
            "type": "number"
          },
          "vendedores_para_cubrir_costos": {
            "type": "number"
          }
        },
        "required": [
          "pedidos_por_vendedor_mes",
          "comision_por_pedido_cents",
          "comision_bruta_vendedor_cents",
          "contribucion_media_cents",
          "vendedores_para_cubrir_costos"
        ]
      },
      "EconomiaDeEscalaResponseDto": {
        "type": "object",
        "properties": {
          "costo_fijo_por_pedido_inicio_cents": {
            "type": "number"
          },
          "costo_fijo_por_pedido_mes6_cents": {
            "type": "number"
          },
          "costo_fijo_por_pedido_fin_cents": {
            "type": "number"
          },
          "multiplicador_escala": {
            "type": "number"
          }
        },
        "required": [
          "costo_fijo_por_pedido_inicio_cents",
          "costo_fijo_por_pedido_mes6_cents",
          "costo_fijo_por_pedido_fin_cents",
          "multiplicador_escala"
        ]
      },
      "IndicadoresProyeccionResponseDto": {
        "type": "object",
        "properties": {
          "payback_mes": {
            "type": "object",
            "nullable": true
          },
          "van_cents": {
            "type": "number"
          },
          "tir_mensual": {
            "type": "object",
            "nullable": true
          },
          "tir_anual": {
            "type": "object",
            "nullable": true
          },
          "tir_no_estable": {
            "type": "boolean"
          },
          "roi": {
            "type": "number"
          },
          "margen_neto": {
            "type": "number"
          },
          "punto_equilibrio_caja_cents": {
            "type": "number"
          },
          "pedidos_equilibrio_caja": {
            "type": "number"
          },
          "escenarios_equilibrio": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/EscenarioEquilibrioResponseDto"
            }
          },
          "margen_seguridad_por_mes": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/MargenSeguridadResponseDto"
            }
          },
          "cobertura_por_vendedor": {
            "$ref": "#/components/schemas/CoberturaPorVendedorResponseDto"
          },
          "economia_de_escala": {
            "$ref": "#/components/schemas/EconomiaDeEscalaResponseDto"
          }
        },
        "required": [
          "payback_mes",
          "van_cents",
          "tir_mensual",
          "tir_anual",
          "tir_no_estable",
          "roi",
          "margen_neto",
          "punto_equilibrio_caja_cents",
          "pedidos_equilibrio_caja",
          "escenarios_equilibrio",
          "margen_seguridad_por_mes",
          "cobertura_por_vendedor",
          "economia_de_escala"
        ]
      },
      "ProyeccionResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "horizonte_meses": {
            "type": "number"
          },
          "supuestos": {
            "$ref": "#/components/schemas/SupuestosProyeccionResponseDto"
          },
          "filas": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/FilaProyeccionResponseDto"
            }
          },
          "totales": {
            "$ref": "#/components/schemas/TotalesProyeccionResponseDto"
          },
          "inversion_inicial_cents": {
            "type": "number"
          },
          "indicadores": {
            "$ref": "#/components/schemas/IndicadoresProyeccionResponseDto"
          },
          "creado_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "nombre",
          "horizonte_meses",
          "supuestos",
          "filas",
          "totales",
          "inversion_inicial_cents",
          "indicadores",
          "creado_en"
        ]
      },
      "PuntoEquilibrioResponseDto": {
        "type": "object",
        "properties": {
          "punto_equilibrio_caja_cents": {
            "type": "number"
          },
          "pedidos_equilibrio_caja": {
            "type": "number"
          },
          "escenarios_equilibrio": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/EscenarioEquilibrioResponseDto"
            }
          },
          "margen_seguridad_por_mes": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/MargenSeguridadResponseDto"
            }
          }
        },
        "required": [
          "punto_equilibrio_caja_cents",
          "pedidos_equilibrio_caja",
          "escenarios_equilibrio",
          "margen_seguridad_por_mes"
        ]
      },
      "KpisFinancierosResponseDto": {
        "type": "object",
        "properties": {
          "periodo": {
            "type": "string"
          },
          "gmv_cents": {
            "type": "number"
          },
          "gmv": {
            "type": "string"
          },
          "pedidos_pagados": {
            "type": "number"
          },
          "pedidos_entregados": {
            "type": "number"
          },
          "ingresos_comisiones_cents": {
            "type": "number"
          },
          "ingresos_comisiones": {
            "type": "string"
          },
          "devoluciones": {
            "type": "number"
          },
          "vendedores_activos": {
            "type": "number"
          },
          "pedidos_por_vendedor": {
            "type": "number"
          },
          "activacion_primeros_15_dias": {
            "type": "object",
            "nullable": true
          },
          "tasa_devoluciones": {
            "type": "number"
          },
          "churn_mensual": {
            "type": "number"
          },
          "conversion_carrito_checkout": {
            "type": "object",
            "nullable": true
          },
          "liquidacion_a_tiempo": {
            "type": "object",
            "nullable": true
          }
        },
        "required": [
          "periodo",
          "gmv_cents",
          "gmv",
          "pedidos_pagados",
          "pedidos_entregados",
          "ingresos_comisiones_cents",
          "ingresos_comisiones",
          "devoluciones",
          "vendedores_activos",
          "pedidos_por_vendedor",
          "activacion_primeros_15_dias",
          "tasa_devoluciones",
          "churn_mensual",
          "conversion_carrito_checkout",
          "liquidacion_a_tiempo"
        ]
      },
      "FlujoVariacionGmvResponseDto": {
        "type": "object",
        "properties": {
          "variacion_gmv": {
            "type": "number"
          },
          "flujo_cents": {
            "type": "number"
          }
        },
        "required": [
          "variacion_gmv",
          "flujo_cents"
        ]
      },
      "SensibilidadComisionGmvResponseDto": {
        "type": "object",
        "properties": {
          "tasa_comision": {
            "type": "number"
          },
          "flujo_mes12_cents": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/FlujoVariacionGmvResponseDto"
            }
          }
        },
        "required": [
          "tasa_comision",
          "flujo_mes12_cents"
        ]
      },
      "SensibilidadVanResponseDto": {
        "type": "object",
        "properties": {
          "tasa_anual": {
            "type": "number"
          },
          "tasa_mensual": {
            "type": "number"
          },
          "van_cents": {
            "type": "number"
          },
          "relacion_van_inversion": {
            "type": "number"
          }
        },
        "required": [
          "tasa_anual",
          "tasa_mensual",
          "van_cents",
          "relacion_van_inversion"
        ]
      },
      "PaybackPorEscenarioResponseDto": {
        "type": "object",
        "properties": {
          "escenario": {
            "type": "string",
            "enum": [
              "base",
              "pesimista",
              "peor_combinado"
            ]
          },
          "descripcion": {
            "type": "string"
          },
          "flujo_acumulado_cents": {
            "type": "number"
          },
          "payback_mes": {
            "type": "object",
            "nullable": true
          }
        },
        "required": [
          "escenario",
          "descripcion",
          "flujo_acumulado_cents",
          "payback_mes"
        ]
      },
      "SensibilidadEstudiosResponseDto": {
        "type": "object",
        "properties": {
          "supuestos": {
            "$ref": "#/components/schemas/SupuestosProyeccionResponseDto"
          },
          "matriz_comision_gmv": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/SensibilidadComisionGmvResponseDto"
            }
          },
          "van_por_tasa": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/SensibilidadVanResponseDto"
            }
          },
          "payback_por_escenario": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/PaybackPorEscenarioResponseDto"
            }
          },
          "cobertura_por_vendedor": {
            "$ref": "#/components/schemas/CoberturaPorVendedorResponseDto"
          }
        },
        "required": [
          "supuestos",
          "matriz_comision_gmv",
          "van_por_tasa",
          "payback_por_escenario",
          "cobertura_por_vendedor"
        ]
      },
      "ResumenAnualResponseDto": {
        "type": "object",
        "properties": {
          "anio": {
            "type": "number"
          },
          "gmv_cents": {
            "type": "number"
          },
          "ingresos_cents": {
            "type": "number"
          },
          "costos_cents": {
            "type": "number"
          },
          "flujo_cents": {
            "type": "number"
          },
          "margen_neto": {
            "type": "number"
          }
        },
        "required": [
          "anio",
          "gmv_cents",
          "ingresos_cents",
          "costos_cents",
          "flujo_cents",
          "margen_neto"
        ]
      },
      "Acumulado24MesesResponseDto": {
        "type": "object",
        "properties": {
          "gmv_cents": {
            "type": "number"
          },
          "ingresos_cents": {
            "type": "number"
          },
          "costos_cents": {
            "type": "number"
          },
          "flujo_cents": {
            "type": "number"
          }
        },
        "required": [
          "gmv_cents",
          "ingresos_cents",
          "costos_cents",
          "flujo_cents"
        ]
      },
      "Mes24ResponseDto": {
        "type": "object",
        "properties": {
          "gmv_cents": {
            "type": "number"
          },
          "incremento_vs_mes12": {
            "type": "number"
          }
        },
        "required": [
          "gmv_cents",
          "incremento_vs_mes12"
        ]
      },
      "PlanBienalResponseDto": {
        "type": "object",
        "properties": {
          "inversion_inicial_cents": {
            "type": "number"
          },
          "anios": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ResumenAnualResponseDto"
            }
          },
          "filas": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/FilaProyeccionResponseDto"
            }
          },
          "acumulado_24_meses": {
            "$ref": "#/components/schemas/Acumulado24MesesResponseDto"
          },
          "roi": {
            "type": "number"
          },
          "mes_24": {
            "$ref": "#/components/schemas/Mes24ResponseDto"
          },
          "indicadores": {
            "$ref": "#/components/schemas/IndicadoresProyeccionResponseDto"
          }
        },
        "required": [
          "inversion_inicial_cents",
          "anios",
          "filas",
          "acumulado_24_meses",
          "roi",
          "mes_24",
          "indicadores"
        ]
      },
      "TableroAlertaResponseDto": {
        "type": "object",
        "properties": {
          "capa": {
            "type": "string"
          },
          "frecuencia": {
            "type": "string"
          },
          "indicador": {
            "type": "string"
          },
          "estado": {
            "type": "string",
            "enum": [
              "OK",
              "ALERTA",
              "SIN_DATO"
            ]
          },
          "umbral": {
            "type": "string"
          },
          "valor": {
            "type": "object",
            "nullable": true
          },
          "accion": {
            "type": "string"
          },
          "detalle": {
            "type": "string"
          },
          "fecha_esperada": {
            "type": "string"
          }
        },
        "required": [
          "capa",
          "frecuencia",
          "indicador",
          "estado",
          "umbral",
          "valor",
          "accion",
          "fecha_esperada"
        ]
      },
      "TableroResponseDto": {
        "type": "object",
        "properties": {
          "periodo": {
            "type": "string"
          },
          "kpis": {
            "$ref": "#/components/schemas/KpisFinancierosResponseDto"
          },
          "alertas": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/TableroAlertaResponseDto"
            }
          }
        },
        "required": [
          "periodo",
          "kpis",
          "alertas"
        ]
      },
      "ResumenVendedorResponseDto": {
        "type": "object",
        "properties": {
          "vendedor_id": {
            "type": "string",
            "format": "uuid"
          },
          "periodo": {
            "type": "string"
          },
          "ventas": {
            "type": "number"
          },
          "monto_cents": {
            "type": "number"
          },
          "comision_cents": {
            "type": "number"
          },
          "devoluciones": {
            "type": "number"
          }
        },
        "required": [
          "vendedor_id",
          "periodo",
          "ventas",
          "monto_cents",
          "comision_cents",
          "devoluciones"
        ]
      },
      "VendedorFiscalResponseDto": {
        "type": "object",
        "properties": {
          "sujeto_fiscal": {
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/SujetoTributarioResponseDto"
              }
            ]
          },
          "resumen_comisiones": {
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/ResumenVendedorResponseDto"
              }
            ]
          },
          "nota": {
            "type": "string"
          }
        },
        "required": [
          "sujeto_fiscal",
          "resumen_comisiones",
          "nota"
        ]
      },
      "CuentaBancoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "bank": {
            "type": "string"
          },
          "alias": {
            "type": "string"
          },
          "accountNumber": {
            "type": "string"
          },
          "type": {
            "type": "string",
            "enum": [
              "ahorro",
              "corriente",
              "wallet"
            ]
          },
          "balanceCents": {
            "type": "number",
            "description": "Saldo en centavos"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "bank",
          "alias",
          "accountNumber",
          "type",
          "balanceCents"
        ]
      },
      "CrearCuentaBancoRequestDto": {
        "type": "object",
        "properties": {
          "bank": {
            "type": "string",
            "description": "Banco",
            "example": "BAC"
          },
          "alias": {
            "type": "string",
            "description": "Alias visible de la cuenta",
            "example": "Operativa"
          },
          "accountNumber": {
            "type": "string",
            "description": "Número de cuenta"
          },
          "type": {
            "type": "string",
            "description": "Tipo de cuenta",
            "enum": [
              "ahorro",
              "corriente",
              "wallet"
            ]
          },
          "balanceCents": {
            "type": "number",
            "description": "Saldo inicial en centavos",
            "minimum": 0
          }
        },
        "required": [
          "bank",
          "alias",
          "accountNumber",
          "type",
          "balanceCents"
        ]
      },
      "ActualizarCuentaBancoRequestDto": {
        "type": "object",
        "properties": {
          "bank": {
            "type": "string",
            "description": "Banco",
            "example": "BAC"
          },
          "alias": {
            "type": "string",
            "description": "Alias visible de la cuenta",
            "example": "Operativa"
          },
          "accountNumber": {
            "type": "string",
            "description": "Número de cuenta"
          },
          "type": {
            "type": "string",
            "description": "Tipo de cuenta",
            "enum": [
              "ahorro",
              "corriente",
              "wallet"
            ]
          },
          "balanceCents": {
            "type": "number",
            "description": "Saldo inicial en centavos",
            "minimum": 0
          }
        }
      },
      "CreditoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "customer": {
            "type": "string"
          },
          "concept": {
            "type": "string"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "dueDate": {
            "type": "string",
            "format": "date"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe original en centavos"
          },
          "balanceCents": {
            "type": "number",
            "description": "Saldo pendiente en centavos"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "customer",
          "concept",
          "date",
          "dueDate",
          "amountCents",
          "balanceCents"
        ]
      },
      "CrearCreditoRequestDto": {
        "type": "object",
        "properties": {
          "customer": {
            "type": "string",
            "description": "Cliente que recibe el crédito"
          },
          "concept": {
            "type": "string",
            "description": "Concepto del crédito"
          },
          "date": {
            "type": "string",
            "description": "Fecha de emisión (yyyy-mm-dd)"
          },
          "dueDate": {
            "type": "string",
            "description": "Fecha de vencimiento (yyyy-mm-dd)"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          }
        },
        "required": [
          "customer",
          "concept",
          "dueDate",
          "amountCents"
        ]
      },
      "GastoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "concept": {
            "type": "string"
          },
          "category": {
            "type": "string"
          },
          "type": {
            "type": "string",
            "enum": [
              "inmediato",
              "programado",
              "recurrente"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos"
          },
          "paymentMethod": {
            "type": "string"
          },
          "status": {
            "type": "string",
            "enum": [
              "pendiente",
              "pagado",
              "cancelado"
            ]
          },
          "includesVat": {
            "type": "boolean"
          },
          "irRetentionCents": {
            "type": "number",
            "description": "Retención de IR en centavos"
          },
          "beneficiary": {
            "type": "string"
          },
          "notes": {
            "type": "string"
          },
          "scheduledFor": {
            "type": "string",
            "format": "date"
          },
          "recurrence": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "date",
          "concept",
          "category",
          "type",
          "amountCents",
          "paymentMethod",
          "status",
          "includesVat",
          "irRetentionCents",
          "beneficiary",
          "notes",
          "scheduledFor",
          "recurrence"
        ]
      },
      "CrearGastoRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha del gasto (yyyy-mm-dd)"
          },
          "concept": {
            "type": "string",
            "description": "Concepto del gasto"
          },
          "category": {
            "type": "string",
            "description": "Categoría visible (puede ser personalizada)"
          },
          "type": {
            "type": "string",
            "description": "Tipo de gasto",
            "enum": [
              "inmediato",
              "programado",
              "recurrente"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago",
            "enum": [
              "efectivo",
              "tarjeta",
              "transferencia",
              "credito",
              "otro"
            ]
          },
          "includesVat": {
            "type": "boolean",
            "description": "Incluye IVA",
            "default": false
          },
          "irRetentionCents": {
            "type": "number",
            "description": "Retención de IR en centavos",
            "minimum": 0,
            "default": 0
          },
          "beneficiary": {
            "type": "string",
            "description": "Proveedor, empleado o beneficiario"
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          },
          "scheduledFor": {
            "type": "string",
            "description": "Fecha programada (yyyy-mm-dd)"
          },
          "recurrence": {
            "type": "string",
            "description": "Etiqueta de recurrencia"
          }
        },
        "required": [
          "date",
          "concept",
          "category",
          "type",
          "amountCents",
          "paymentMethod"
        ]
      },
      "ActualizarGastoRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha del gasto (yyyy-mm-dd)"
          },
          "concept": {
            "type": "string",
            "description": "Concepto del gasto"
          },
          "category": {
            "type": "string",
            "description": "Categoría visible (puede ser personalizada)"
          },
          "type": {
            "type": "string",
            "description": "Tipo de gasto",
            "enum": [
              "inmediato",
              "programado",
              "recurrente"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago",
            "enum": [
              "efectivo",
              "tarjeta",
              "transferencia",
              "credito",
              "otro"
            ]
          },
          "includesVat": {
            "type": "boolean",
            "description": "Incluye IVA",
            "default": false
          },
          "irRetentionCents": {
            "type": "number",
            "description": "Retención de IR en centavos",
            "minimum": 0,
            "default": 0
          },
          "beneficiary": {
            "type": "string",
            "description": "Proveedor, empleado o beneficiario"
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          },
          "scheduledFor": {
            "type": "string",
            "description": "Fecha programada (yyyy-mm-dd)"
          },
          "recurrence": {
            "type": "string",
            "description": "Etiqueta de recurrencia"
          }
        }
      },
      "CuentaPorPagarResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "supplier": {
            "type": "string"
          },
          "concept": {
            "type": "string"
          },
          "dueDate": {
            "type": "string",
            "format": "date"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos"
          },
          "status": {
            "type": "string",
            "enum": [
              "pendiente",
              "pagada"
            ]
          },
          "paidAt": {
            "type": "string",
            "format": "date"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "supplier",
          "concept",
          "dueDate",
          "amountCents",
          "status",
          "paidAt"
        ]
      },
      "CrearCuentaPorPagarRequestDto": {
        "type": "object",
        "properties": {
          "supplier": {
            "type": "string",
            "description": "Proveedor"
          },
          "concept": {
            "type": "string",
            "description": "Concepto de la obligación"
          },
          "dueDate": {
            "type": "string",
            "description": "Fecha de vencimiento (yyyy-mm-dd)"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          }
        },
        "required": [
          "supplier",
          "concept",
          "dueDate",
          "amountCents"
        ]
      },
      "ProveedorResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "name": {
            "type": "string"
          },
          "ruc": {
            "type": "string"
          },
          "phone": {
            "type": "string"
          },
          "email": {
            "type": "string"
          },
          "address": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "name",
          "ruc",
          "phone",
          "email",
          "address"
        ]
      },
      "CrearProveedorRequestDto": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Nombre del proveedor"
          },
          "ruc": {
            "type": "string",
            "description": "RUC"
          },
          "phone": {
            "type": "string",
            "description": "Teléfono"
          },
          "email": {
            "type": "string",
            "description": "Correo"
          },
          "address": {
            "type": "string",
            "description": "Dirección"
          }
        },
        "required": [
          "name"
        ]
      },
      "LineaCompraResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string"
          },
          "productId": {
            "type": "object",
            "nullable": true
          },
          "name": {
            "type": "string"
          },
          "sku": {
            "type": "string"
          },
          "quantity": {
            "type": "number"
          },
          "unitCostCents": {
            "type": "number",
            "description": "Costo unitario en centavos"
          },
          "salePriceCents": {
            "type": "object",
            "description": "Precio de venta en centavos",
            "nullable": true
          }
        },
        "required": [
          "id",
          "productId",
          "name",
          "sku",
          "quantity",
          "unitCostCents",
          "salePriceCents"
        ]
      },
      "CompraResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "number": {
            "type": "string"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "supplierId": {
            "type": "object",
            "nullable": true
          },
          "supplierName": {
            "type": "string"
          },
          "paymentMethod": {
            "type": "string"
          },
          "status": {
            "type": "string",
            "enum": [
              "recibida",
              "pendiente",
              "anulada"
            ]
          },
          "origin": {
            "type": "string",
            "enum": [
              "productos",
              "directa"
            ]
          },
          "costType": {
            "type": "string"
          },
          "lines": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LineaCompraResponseDto"
            }
          },
          "subtotalCents": {
            "type": "number",
            "description": "Subtotal en centavos"
          },
          "retentionCents": {
            "type": "number",
            "description": "Retención en centavos"
          },
          "totalCents": {
            "type": "number",
            "description": "Total en centavos"
          },
          "notes": {
            "type": "string"
          },
          "userName": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "number",
          "date",
          "supplierId",
          "supplierName",
          "paymentMethod",
          "status",
          "origin",
          "costType",
          "lines",
          "subtotalCents",
          "retentionCents",
          "totalCents",
          "notes",
          "userName"
        ]
      },
      "LineaCompraRequestDto": {
        "type": "object",
        "properties": {
          "productId": {
            "type": "object",
            "description": "Id del producto del catálogo",
            "nullable": true
          },
          "name": {
            "type": "string",
            "description": "Nombre visible de la línea"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto"
          },
          "quantity": {
            "type": "number",
            "description": "Cantidad comprada",
            "minimum": 1
          },
          "unitCostCents": {
            "type": "number",
            "description": "Costo unitario en centavos",
            "minimum": 0
          },
          "salePriceCents": {
            "type": "object",
            "description": "Precio de venta en centavos",
            "nullable": true,
            "minimum": 0
          }
        },
        "required": [
          "name",
          "quantity",
          "unitCostCents"
        ]
      },
      "CrearCompraRequestDto": {
        "type": "object",
        "properties": {
          "number": {
            "type": "string",
            "description": "Número del documento; si se omite se genera"
          },
          "date": {
            "type": "string",
            "description": "Fecha de la compra (yyyy-mm-dd)"
          },
          "supplierId": {
            "type": "object",
            "description": "Id del proveedor",
            "nullable": true
          },
          "supplierName": {
            "type": "string",
            "description": "Nombre del proveedor"
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago",
            "enum": [
              "efectivo",
              "tarjeta",
              "transferencia",
              "credito",
              "otro"
            ]
          },
          "status": {
            "type": "string",
            "description": "Estado del documento",
            "enum": [
              "recibida",
              "pendiente",
              "anulada"
            ]
          },
          "origin": {
            "type": "string",
            "description": "Origen de la compra",
            "enum": [
              "productos",
              "directa"
            ]
          },
          "costType": {
            "type": "string",
            "description": "Tipo de costo",
            "enum": [
              "inventario",
              "servicio",
              "gasto",
              "otro"
            ]
          },
          "lines": {
            "description": "Líneas de la compra",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LineaCompraRequestDto"
            }
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          },
          "retentionCents": {
            "type": "number",
            "description": "Retención informativa en centavos",
            "minimum": 0
          },
          "userName": {
            "type": "string",
            "description": "Usuario que registra la compra"
          }
        },
        "required": [
          "date",
          "paymentMethod",
          "status",
          "origin",
          "costType",
          "lines"
        ]
      },
      "CrearCompraDirectaRequestDto": {
        "type": "object",
        "properties": {
          "number": {
            "type": "string",
            "description": "Número del documento; si se omite se genera"
          },
          "date": {
            "type": "string",
            "description": "Fecha de la compra (yyyy-mm-dd)"
          },
          "supplierId": {
            "type": "object",
            "description": "Id del proveedor",
            "nullable": true
          },
          "supplierName": {
            "type": "string",
            "description": "Nombre del proveedor"
          },
          "concept": {
            "type": "string",
            "description": "Concepto del gasto directo"
          },
          "costType": {
            "type": "string",
            "description": "Tipo de costo",
            "enum": [
              "inventario",
              "servicio",
              "gasto",
              "otro"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago",
            "enum": [
              "efectivo",
              "tarjeta",
              "transferencia",
              "credito",
              "otro"
            ]
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          },
          "retentionCents": {
            "type": "number",
            "description": "Retención informativa en centavos",
            "minimum": 0
          }
        },
        "required": [
          "date",
          "concept",
          "costType",
          "amountCents",
          "paymentMethod"
        ]
      },
      "CrearCompraProductoNuevoRequestDto": {
        "type": "object",
        "properties": {
          "number": {
            "type": "string",
            "description": "Número del documento; si se omite se genera"
          },
          "date": {
            "type": "string",
            "description": "Fecha de la compra (yyyy-mm-dd)"
          },
          "supplierId": {
            "type": "object",
            "description": "Id del proveedor",
            "nullable": true
          },
          "supplierName": {
            "type": "string",
            "description": "Nombre del proveedor"
          },
          "productName": {
            "type": "string",
            "description": "Nombre del producto nuevo"
          },
          "category": {
            "type": "string",
            "description": "Categoría"
          },
          "sku": {
            "type": "string",
            "description": "SKU"
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago",
            "enum": [
              "efectivo",
              "tarjeta",
              "transferencia",
              "credito",
              "otro"
            ]
          },
          "unitCostCents": {
            "type": "number",
            "description": "Costo unitario en centavos",
            "minimum": 0
          },
          "salePriceCents": {
            "type": "number",
            "description": "Precio de venta en centavos",
            "minimum": 0
          },
          "quantity": {
            "type": "number",
            "description": "Cantidad",
            "minimum": 1
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          }
        },
        "required": [
          "date",
          "productName",
          "paymentMethod",
          "unitCostCents",
          "salePriceCents",
          "quantity"
        ]
      },
      "OrdenCompraResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "number": {
            "type": "string"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "supplierId": {
            "type": "object",
            "nullable": true
          },
          "supplierName": {
            "type": "string"
          },
          "lines": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LineaCompraResponseDto"
            }
          },
          "totalCents": {
            "type": "number",
            "description": "Total en centavos"
          },
          "status": {
            "type": "string",
            "enum": [
              "pendiente",
              "convertida",
              "anulada"
            ]
          },
          "notes": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "number",
          "date",
          "supplierId",
          "supplierName",
          "lines",
          "totalCents",
          "status",
          "notes"
        ]
      },
      "CrearOrdenCompraRequestDto": {
        "type": "object",
        "properties": {
          "number": {
            "type": "string",
            "description": "Número de la orden; si se omite se genera"
          },
          "date": {
            "type": "string",
            "description": "Fecha de la orden (yyyy-mm-dd)"
          },
          "supplierId": {
            "type": "object",
            "description": "Id del proveedor",
            "nullable": true
          },
          "supplierName": {
            "type": "string",
            "description": "Nombre del proveedor"
          },
          "lines": {
            "description": "Líneas de la orden",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/LineaCompraRequestDto"
            }
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          }
        },
        "required": [
          "date",
          "lines"
        ]
      },
      "ConfiguracionDocumentosResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "logoSize": {
            "type": "string",
            "enum": [
              "pequeno",
              "mediano",
              "grande"
            ]
          },
          "headerColor": {
            "type": "string"
          },
          "matchTableColor": {
            "type": "boolean"
          },
          "footerMessage": {
            "type": "string"
          },
          "showRuc": {
            "type": "boolean"
          },
          "showAddress": {
            "type": "boolean"
          },
          "showPhone": {
            "type": "boolean"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "logoSize",
          "headerColor",
          "matchTableColor",
          "footerMessage",
          "showRuc",
          "showAddress",
          "showPhone"
        ]
      },
      "ActualizarConfiguracionDocumentosRequestDto": {
        "type": "object",
        "properties": {
          "logoSize": {
            "type": "string",
            "description": "Tamaño del logo",
            "enum": [
              "pequeno",
              "mediano",
              "grande"
            ]
          },
          "headerColor": {
            "type": "string",
            "description": "Color de cabecera",
            "example": "#0f172a"
          },
          "matchTableColor": {
            "type": "boolean",
            "description": "Igualar color de tabla al de cabecera"
          },
          "footerMessage": {
            "type": "string",
            "description": "Mensaje del pie de página"
          },
          "showRuc": {
            "type": "boolean",
            "description": "Mostrar RUC"
          },
          "showAddress": {
            "type": "boolean",
            "description": "Mostrar dirección"
          },
          "showPhone": {
            "type": "boolean",
            "description": "Mostrar teléfono"
          }
        },
        "required": [
          "logoSize",
          "headerColor",
          "matchTableColor",
          "footerMessage",
          "showRuc",
          "showAddress",
          "showPhone"
        ]
      },
      "MetodoPagoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "name": {
            "type": "string"
          },
          "kind": {
            "type": "string"
          },
          "icon": {
            "type": "string"
          },
          "active": {
            "type": "boolean"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "name",
          "kind",
          "icon",
          "active"
        ]
      },
      "CrearMetodoPagoRequestDto": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Nombre visible del método"
          },
          "kind": {
            "type": "string",
            "description": "Tipo funcional (efectivo, tarjeta, transferencia…)"
          },
          "icon": {
            "type": "string",
            "description": "Icono"
          },
          "active": {
            "type": "boolean",
            "description": "Activo",
            "default": true
          }
        },
        "required": [
          "name",
          "kind"
        ]
      },
      "ActualizarMetodoPagoRequestDto": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Nombre visible del método"
          },
          "kind": {
            "type": "string",
            "description": "Tipo funcional (efectivo, tarjeta, transferencia…)"
          },
          "icon": {
            "type": "string",
            "description": "Icono"
          },
          "active": {
            "type": "boolean",
            "description": "Activo",
            "default": true
          }
        }
      },
      "MovimientoCajaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "type": {
            "type": "string",
            "enum": [
              "ingreso",
              "egreso"
            ]
          },
          "concept": {
            "type": "string"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos"
          },
          "currency": {
            "type": "string",
            "enum": [
              "NIO",
              "USD"
            ]
          },
          "originalAmountCents": {
            "type": "number",
            "description": "Importe original en centavos"
          },
          "exchangeRate": {
            "type": "number"
          },
          "paymentMethod": {
            "type": "string"
          },
          "category": {
            "type": "string"
          },
          "source": {
            "type": "string"
          },
          "status": {
            "type": "string",
            "enum": [
              "completado",
              "anulado"
            ]
          },
          "reference": {
            "type": "string"
          },
          "observations": {
            "type": "string"
          },
          "transferId": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "date",
          "type",
          "concept",
          "amountCents",
          "currency",
          "originalAmountCents",
          "exchangeRate",
          "paymentMethod",
          "category",
          "source",
          "status",
          "reference",
          "observations"
        ]
      },
      "CrearMovimientoCajaRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha local (yyyy-mm-dd)"
          },
          "type": {
            "type": "string",
            "description": "Dirección del movimiento",
            "enum": [
              "ingreso",
              "egreso"
            ]
          },
          "concept": {
            "type": "string",
            "description": "Concepto"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          },
          "paymentMethod": {
            "type": "string",
            "description": "Método de pago (id o etiqueta)"
          },
          "category": {
            "type": "string",
            "description": "Categoría funcional"
          },
          "currency": {
            "type": "string",
            "description": "Moneda",
            "enum": [
              "NIO",
              "USD"
            ]
          },
          "originalAmountCents": {
            "type": "number",
            "description": "Importe original capturado en centavos",
            "minimum": 0
          },
          "exchangeRate": {
            "type": "number",
            "description": "Tipo de cambio aplicado",
            "minimum": 0
          },
          "reference": {
            "type": "string",
            "description": "Referencia visible del documento"
          },
          "observations": {
            "type": "string",
            "description": "Observaciones"
          },
          "source": {
            "type": "string",
            "description": "Origen del movimiento"
          }
        },
        "required": [
          "date",
          "type",
          "concept",
          "amountCents",
          "paymentMethod"
        ]
      },
      "CierreCajaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "openingBalanceCents": {
            "type": "number",
            "description": "Saldo inicial en centavos"
          },
          "incomeCents": {
            "type": "number",
            "description": "Ingresos del día en centavos"
          },
          "expenseCents": {
            "type": "number",
            "description": "Egresos del día en centavos"
          },
          "expectedBalanceCents": {
            "type": "number",
            "description": "Saldo esperado en centavos"
          },
          "countedBalanceCents": {
            "type": "number",
            "description": "Saldo contado en centavos"
          },
          "differenceCents": {
            "type": "number",
            "description": "Diferencia en centavos"
          },
          "movementsCount": {
            "type": "number"
          },
          "notes": {
            "type": "string"
          },
          "closedAt": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "date",
          "openingBalanceCents",
          "incomeCents",
          "expenseCents",
          "expectedBalanceCents",
          "countedBalanceCents",
          "differenceCents",
          "movementsCount",
          "notes",
          "closedAt"
        ]
      },
      "CierreCajaPreviewResponseDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "format": "date"
          },
          "openingBalanceCents": {
            "type": "number",
            "description": "Saldo inicial en centavos"
          },
          "incomeCents": {
            "type": "number",
            "description": "Ingresos del día en centavos"
          },
          "expenseCents": {
            "type": "number",
            "description": "Egresos del día en centavos"
          },
          "expectedBalanceCents": {
            "type": "number",
            "description": "Saldo esperado en centavos"
          },
          "movementsCount": {
            "type": "number"
          },
          "existing": {
            "nullable": true,
            "type": "object",
            "allOf": [
              {
                "$ref": "#/components/schemas/CierreCajaResponseDto"
              }
            ]
          }
        },
        "required": [
          "date",
          "openingBalanceCents",
          "incomeCents",
          "expenseCents",
          "expectedBalanceCents",
          "movementsCount",
          "existing"
        ]
      },
      "CrearCierreCajaRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha del corte (yyyy-mm-dd)"
          },
          "countedBalanceCents": {
            "type": "number",
            "description": "Saldo contado en centavos",
            "minimum": 0
          },
          "notes": {
            "type": "string",
            "description": "Notas del cierre"
          }
        },
        "required": [
          "date",
          "countedBalanceCents"
        ]
      },
      "FondoCajaChicaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "amountCents": {
            "type": "number",
            "description": "Monto del fondo en centavos"
          },
          "responsible": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "amountCents",
          "responsible"
        ]
      },
      "CrearFondoCajaChicaRequestDto": {
        "type": "object",
        "properties": {
          "amountCents": {
            "type": "number",
            "description": "Monto del fondo en centavos",
            "minimum": 0
          },
          "responsible": {
            "type": "string",
            "description": "Responsable del fondo"
          }
        },
        "required": [
          "amountCents",
          "responsible"
        ]
      },
      "MovimientoCajaChicaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "type": {
            "type": "string",
            "enum": [
              "reposicion",
              "gasto"
            ]
          },
          "concept": {
            "type": "string"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "date",
          "type",
          "concept",
          "amountCents"
        ]
      },
      "CrearMovimientoCajaChicaRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha local (yyyy-mm-dd)"
          },
          "type": {
            "type": "string",
            "description": "Tipo de movimiento",
            "enum": [
              "reposicion",
              "gasto"
            ]
          },
          "concept": {
            "type": "string",
            "description": "Concepto"
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          }
        },
        "required": [
          "date",
          "type",
          "concept",
          "amountCents"
        ]
      },
      "TransferenciaCajaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "fromId": {
            "type": "string"
          },
          "fromLabel": {
            "type": "string"
          },
          "fromKind": {
            "type": "string",
            "enum": [
              "cash",
              "bank"
            ]
          },
          "toId": {
            "type": "string"
          },
          "toLabel": {
            "type": "string"
          },
          "toKind": {
            "type": "string",
            "enum": [
              "cash",
              "bank"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos"
          },
          "concept": {
            "type": "string"
          },
          "notes": {
            "type": "string"
          },
          "status": {
            "type": "string",
            "enum": [
              "completado",
              "anulado"
            ]
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "date",
          "fromId",
          "fromLabel",
          "fromKind",
          "toId",
          "toLabel",
          "toKind",
          "amountCents",
          "concept",
          "notes",
          "status"
        ]
      },
      "CrearTransferenciaCajaRequestDto": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "description": "Fecha local (yyyy-mm-dd)"
          },
          "fromId": {
            "type": "string",
            "description": "Id de la cuenta origen"
          },
          "fromLabel": {
            "type": "string",
            "description": "Etiqueta de la cuenta origen"
          },
          "fromKind": {
            "type": "string",
            "description": "Tipo de cuenta origen",
            "enum": [
              "cash",
              "bank"
            ]
          },
          "toId": {
            "type": "string",
            "description": "Id de la cuenta destino"
          },
          "toLabel": {
            "type": "string",
            "description": "Etiqueta de la cuenta destino"
          },
          "toKind": {
            "type": "string",
            "description": "Tipo de cuenta destino",
            "enum": [
              "cash",
              "bank"
            ]
          },
          "amountCents": {
            "type": "number",
            "description": "Importe en centavos",
            "minimum": 1
          },
          "concept": {
            "type": "string",
            "description": "Concepto de la transferencia"
          },
          "notes": {
            "type": "string",
            "description": "Notas"
          }
        },
        "required": [
          "date",
          "fromId",
          "fromLabel",
          "fromKind",
          "toId",
          "toLabel",
          "toKind",
          "amountCents",
          "concept"
        ]
      },
      "ClienteComercioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "name": {
            "type": "string"
          },
          "phone": {
            "type": "string"
          },
          "email": {
            "type": "string"
          },
          "company": {
            "type": "string"
          },
          "buyerId": {
            "type": "string"
          },
          "source": {
            "type": "string",
            "enum": [
              "order",
              "manual",
              "import"
            ]
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "name",
          "phone",
          "email",
          "company",
          "source"
        ]
      },
      "CrearClienteComercioRequestDto": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Nombre del cliente"
          },
          "phone": {
            "type": "string",
            "description": "Teléfono"
          },
          "email": {
            "type": "string",
            "description": "Correo"
          },
          "company": {
            "type": "string",
            "description": "Empresa del cliente"
          },
          "buyerId": {
            "type": "string",
            "description": "Id del comprador en Core Engine"
          },
          "source": {
            "type": "string",
            "description": "Origen",
            "enum": [
              "manual",
              "import"
            ]
          }
        },
        "required": [
          "name"
        ]
      },
      "ImportarClientesRequestDto": {
        "type": "object",
        "properties": {
          "clientes": {
            "description": "Filas a importar (máximo 500)",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/CrearClienteComercioRequestDto"
            }
          }
        },
        "required": [
          "clientes"
        ]
      },
      "ImportarClientesResponseDto": {
        "type": "object",
        "properties": {
          "imported": {
            "type": "number"
          },
          "skipped": {
            "type": "number"
          },
          "errors": {
            "type": "array",
            "items": {
              "type": "string"
            }
          }
        },
        "required": [
          "imported",
          "skipped",
          "errors"
        ]
      },
      "ActualizarClienteComercioRequestDto": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Nombre del cliente"
          },
          "phone": {
            "type": "string",
            "description": "Teléfono"
          },
          "email": {
            "type": "string",
            "description": "Correo"
          },
          "company": {
            "type": "string",
            "description": "Empresa del cliente"
          },
          "buyerId": {
            "type": "string",
            "description": "Id del comprador en Core Engine"
          },
          "source": {
            "type": "string",
            "description": "Origen",
            "enum": [
              "manual",
              "import"
            ]
          }
        }
      },
      "CodigoBarrasResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "createdAt": {
            "type": "string",
            "format": "date-time"
          },
          "updatedAt": {
            "type": "string",
            "format": "date-time"
          },
          "productName": {
            "type": "string"
          },
          "sku": {
            "type": "string"
          },
          "code": {
            "type": "string"
          },
          "symbology": {
            "type": "string",
            "enum": [
              "EAN13",
              "CODE128"
            ]
          },
          "copies": {
            "type": "number"
          }
        },
        "required": [
          "id",
          "createdAt",
          "updatedAt",
          "productName",
          "sku",
          "code",
          "symbology",
          "copies"
        ]
      },
      "CrearCodigoBarrasRequestDto": {
        "type": "object",
        "properties": {
          "productName": {
            "type": "string",
            "description": "Producto al que pertenece la etiqueta"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto"
          },
          "code": {
            "type": "string",
            "description": "Código; vacío = se genera según la simbología"
          },
          "symbology": {
            "type": "string",
            "description": "Simbología",
            "enum": [
              "EAN13",
              "CODE128"
            ]
          },
          "copies": {
            "type": "number",
            "description": "Copias a imprimir (1-100)",
            "minimum": 1,
            "maximum": 100
          }
        },
        "required": [
          "productName",
          "sku",
          "symbology",
          "copies"
        ]
      },
      "UsuarioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del usuario",
            "format": "uuid"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del usuario"
          },
          "correo": {
            "type": "string",
            "description": "Correo del usuario",
            "format": "email"
          },
          "rol": {
            "type": "string",
            "description": "Rol del usuario",
            "enum": [
              "admin",
              "vendedor",
              "comprador",
              "logistica",
              "coordinador",
              "supervisor",
              "operativo"
            ],
            "example": "vendedor"
          },
          "tenant_id": {
            "type": "string",
            "description": "Tenant del usuario",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "description": "Ficha de personal vinculada",
            "format": "uuid"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "nombre",
          "correo",
          "rol",
          "creado_en"
        ]
      },
      "ContarVendedoresResponseDto": {
        "type": "object",
        "properties": {
          "vendedores": {
            "type": "number",
            "description": "Cantidad de usuarios con rol vendedor",
            "example": 12
          }
        },
        "required": [
          "vendedores"
        ]
      },
      "EmpresaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "slug": {
            "type": "string",
            "description": "Slug público actual",
            "example": "distribuidora-piloto"
          },
          "modalidad_logistica": {
            "type": "string",
            "enum": [
              "plataforma",
              "propia",
              "hibrida"
            ]
          },
          "tipo_empresa": {
            "type": "string",
            "enum": [
              "comercial",
              "logistica",
              "plataforma"
            ],
            "description": "Tipo de empresa; solo las comerciales tienen catálogo propio (ADR-27)"
          },
          "estado": {
            "type": "string"
          },
          "creado_por": {
            "type": "string",
            "format": "uuid"
          },
          "creado_en": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "nombre",
          "modalidad_logistica",
          "estado",
          "creado_por",
          "creado_en"
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
              "empresario",
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
      "SesionUsuarioResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del usuario",
            "format": "uuid"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del usuario"
          },
          "correo": {
            "type": "string",
            "description": "Correo del usuario",
            "format": "email"
          },
          "rol": {
            "type": "string",
            "description": "Rol del usuario",
            "enum": [
              "admin",
              "vendedor",
              "comprador",
              "logistica",
              "coordinador",
              "supervisor",
              "operativo"
            ],
            "example": "comprador"
          },
          "tenant_id": {
            "type": "string",
            "description": "Tenant del usuario",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "description": "Ficha de personal vinculada",
            "format": "uuid"
          }
        },
        "required": [
          "id",
          "nombre",
          "correo",
          "rol"
        ]
      },
      "SesionResponseDto": {
        "type": "object",
        "properties": {
          "access_token": {
            "type": "string",
            "description": "Access token JWT (HS256)",
            "example": "eyJhbGciOiJIUzI1NiIs..."
          },
          "refresh_token": {
            "type": "string",
            "description": "Refresh token JWT",
            "example": "eyJhbGciOiJIUzI1NiIs..."
          },
          "expira_en": {
            "type": "number",
            "description": "Vigencia del access token en segundos",
            "example": 900
          },
          "usuario": {
            "description": "Datos del usuario autenticado",
            "allOf": [
              {
                "$ref": "#/components/schemas/SesionUsuarioResponseDto"
              }
            ]
          }
        },
        "required": [
          "access_token",
          "refresh_token",
          "expira_en",
          "usuario"
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
      "OkResponseDto": {
        "type": "object",
        "properties": {
          "ok": {
            "type": "boolean",
            "description": "Operación completada",
            "example": true
          }
        },
        "required": [
          "ok"
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
              "empresario",
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
      "UsuarioContextoResponseDto": {
        "type": "object",
        "properties": {
          "user_id": {
            "type": "string",
            "description": "Id del usuario autenticado",
            "format": "uuid"
          },
          "email": {
            "type": "string",
            "description": "Correo del usuario",
            "format": "email"
          },
          "rol": {
            "type": "string",
            "description": "Rol del usuario",
            "enum": [
              "admin",
              "vendedor",
              "comprador",
              "logistica",
              "coordinador",
              "supervisor",
              "operativo"
            ],
            "example": "admin"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del usuario"
          },
          "tenant_id": {
            "type": "string",
            "description": "Tenant del usuario",
            "format": "uuid"
          },
          "personal_id": {
            "type": "string",
            "description": "Ficha de personal vinculada",
            "format": "uuid"
          }
        },
        "required": [
          "user_id",
          "email",
          "rol"
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
      "CrearEmpresaRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "minLength": 2,
            "maxLength": 160
          },
          "slug": {
            "type": "string",
            "minLength": 2,
            "maxLength": 80,
            "description": "Slug público. Si se omite se deriva del nombre y se resuelven colisiones con sufijos.",
            "example": "distribuidora-piloto"
          },
          "modalidad_logistica": {
            "type": "string",
            "enum": [
              "plataforma",
              "propia",
              "hibrida"
            ]
          }
        },
        "required": [
          "nombre",
          "modalidad_logistica"
        ]
      },
      "ActualizarSlugRequestDto": {
        "type": "object",
        "properties": {
          "slug": {
            "type": "string",
            "minLength": 2,
            "maxLength": 80,
            "example": "distribuidora-piloto"
          }
        },
        "required": [
          "slug"
        ]
      },
      "ActualizarModalidadLogisticaRequestDto": {
        "type": "object",
        "properties": {
          "modalidad_logistica": {
            "type": "string",
            "enum": [
              "plataforma",
              "propia",
              "hibrida"
            ]
          }
        },
        "required": [
          "modalidad_logistica"
        ]
      },
      "ActualizarTipoEmpresaRequestDto": {
        "type": "object",
        "properties": {
          "tipo_empresa": {
            "type": "string",
            "enum": [
              "comercial",
              "logistica",
              "plataforma"
            ]
          }
        },
        "required": [
          "tipo_empresa"
        ]
      },
      "CrearVendedorRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string"
          },
          "correo": {
            "type": "string"
          },
          "contrasena": {
            "type": "string",
            "minLength": 8
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100
          }
        },
        "required": [
          "nombre",
          "correo",
          "contrasena",
          "participacion_porcentaje"
        ]
      },
      "VendedorEmpresaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "nombre": {
            "type": "string"
          },
          "correo": {
            "type": "string"
          },
          "rol": {
            "type": "string",
            "enum": [
              "vendedor"
            ]
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100
          }
        },
        "required": [
          "id",
          "nombre",
          "correo",
          "rol",
          "participacion_porcentaje"
        ]
      },
      "CrearInvitacionRequestDto": {
        "type": "object",
        "properties": {
          "correo": {
            "type": "string",
            "format": "email",
            "example": "vendedor@correo.com"
          },
          "nombre": {
            "type": "string",
            "minLength": 2,
            "maxLength": 120
          },
          "tienda_id": {
            "type": "string",
            "format": "uuid"
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100,
            "example": 25
          }
        },
        "required": [
          "correo",
          "tienda_id",
          "participacion_porcentaje"
        ]
      },
      "InvitacionCreadaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "format": "uuid"
          },
          "tienda_id": {
            "type": "string",
            "format": "uuid"
          },
          "tienda_nombre": {
            "type": "string"
          },
          "correo": {
            "type": "string"
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100
          },
          "estado": {
            "type": "string"
          },
          "expira_en": {
            "type": "string",
            "format": "date-time"
          },
          "url_invitacion": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "tienda_id",
          "tienda_nombre",
          "correo",
          "participacion_porcentaje",
          "estado",
          "expira_en",
          "url_invitacion"
        ]
      },
      "InvitacionResumenResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "format": "uuid"
          },
          "correo": {
            "type": "string"
          },
          "tienda_id": {
            "type": "string",
            "format": "uuid"
          },
          "tienda_nombre": {
            "type": "string"
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100
          },
          "estado": {
            "type": "string"
          },
          "expira_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "correo",
          "tienda_id",
          "tienda_nombre",
          "participacion_porcentaje",
          "estado",
          "expira_en"
        ]
      },
      "ListarVendedoresResponseDto": {
        "type": "object",
        "properties": {
          "vendedores": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/VendedorEmpresaResponseDto"
            }
          },
          "invitaciones": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/InvitacionResumenResponseDto"
            }
          }
        },
        "required": [
          "vendedores",
          "invitaciones"
        ]
      },
      "InvitacionPublicaResponseDto": {
        "type": "object",
        "properties": {
          "valida": {
            "type": "boolean"
          },
          "correo": {
            "type": "string"
          },
          "empresa_nombre": {
            "type": "string"
          },
          "tienda_nombre": {
            "type": "string"
          },
          "participacion_porcentaje": {
            "type": "number",
            "minimum": 0,
            "maximum": 100
          },
          "expira_en": {
            "type": "string",
            "format": "date-time"
          }
        },
        "required": [
          "valida",
          "correo",
          "empresa_nombre",
          "tienda_nombre",
          "participacion_porcentaje",
          "expira_en"
        ]
      },
      "AceptarInvitacionRequestDto": {
        "type": "object",
        "properties": {
          "nombre": {
            "type": "string",
            "minLength": 2,
            "maxLength": 120,
            "example": "Ana Pérez"
          },
          "contrasena": {
            "type": "string",
            "minLength": 8,
            "example": "UnaClaveSegura123"
          }
        },
        "required": [
          "nombre",
          "contrasena"
        ]
      },
      "AceptarInvitacionResponseDto": {
        "type": "object",
        "properties": {
          "access_token": {
            "type": "string"
          },
          "refresh_token": {
            "type": "string"
          },
          "expira_en": {
            "type": "number"
          },
          "usuario": {
            "type": "object",
            "description": "Usuario vendedor con tienda y participación asignadas"
          }
        },
        "required": [
          "access_token",
          "refresh_token",
          "expira_en",
          "usuario"
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
          "empresa_id": {
            "type": "string",
            "description": "Empresa propietaria de las líneas despachadas",
            "format": "uuid"
          },
          "modalidad_logistica": {
            "type": "string",
            "enum": [
              "plataforma",
              "propia",
              "hibrida"
            ]
          },
          "proveedor_tipo": {
            "type": "string",
            "enum": [
              "plataforma",
              "empresa"
            ],
            "nullable": true
          },
          "proveedor_tenant_id": {
            "type": "object",
            "format": "uuid",
            "nullable": true
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
          "empresa_id",
          "modalidad_logistica",
          "proveedor_tipo",
          "proveedor_tenant_id",
          "estado",
          "creado_en",
          "monto"
        ]
      },
      "EnvioResponseDto": {
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
          "empresa_id": {
            "type": "string",
            "description": "Empresa propietaria de las líneas despachadas",
            "format": "uuid"
          },
          "modalidad_logistica": {
            "type": "string",
            "enum": [
              "plataforma",
              "propia",
              "hibrida"
            ]
          },
          "proveedor_tipo": {
            "type": "string",
            "enum": [
              "plataforma",
              "empresa"
            ],
            "nullable": true
          },
          "proveedor_tenant_id": {
            "type": "object",
            "format": "uuid",
            "nullable": true
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
          }
        },
        "required": [
          "id",
          "guia",
          "order_id",
          "empresa_id",
          "modalidad_logistica",
          "proveedor_tipo",
          "proveedor_tenant_id",
          "estado",
          "creado_en"
        ]
      },
      "AsignarProveedorRequestDto": {
        "type": "object",
        "properties": {
          "proveedor_tipo": {
            "type": "string",
            "enum": [
              "plataforma",
              "empresa"
            ],
            "description": "Flota que atenderá el envío híbrido"
          }
        },
        "required": [
          "proveedor_tipo"
        ]
      },
      "AvanzarEnvioRequestDto": {
        "type": "object",
        "properties": {
          "estado": {
            "type": "string",
            "enum": [
              "enviada",
              "entregada",
              "cancelada"
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
      "PuntoCalorDto": {
        "type": "object",
        "properties": {
          "lat": {
            "type": "number"
          },
          "lng": {
            "type": "number"
          },
          "peso": {
            "type": "number"
          },
          "tipo": {
            "type": "string"
          }
        },
        "required": [
          "lat",
          "lng",
          "peso",
          "tipo"
        ]
      },
      "MapaCalorResponseDto": {
        "type": "object",
        "properties": {
          "puntos": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/PuntoCalorDto"
            }
          },
          "total_puntos": {
            "type": "number"
          }
        },
        "required": [
          "puntos",
          "total_puntos"
        ]
      },
      "RendimientoVendedorResponseDto": {
        "type": "object",
        "properties": {
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "total_ventas": {
            "type": "number",
            "description": "Total de ventas",
            "example": 42
          },
          "total_monto_cents": {
            "type": "number",
            "description": "Monto total vendido en centavos",
            "example": 4830000
          },
          "ticket_promedio_cents": {
            "type": "number",
            "description": "Ticket promedio en centavos",
            "example": 115000
          },
          "pedidos_entregados": {
            "type": "number",
            "description": "Pedidos entregados",
            "example": 40
          },
          "tasa_efectividad": {
            "type": "number",
            "description": "Tasa de efectividad (0–1)",
            "example": 0.95
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "vendedor_id",
          "total_ventas",
          "total_monto_cents",
          "ticket_promedio_cents",
          "pedidos_entregados",
          "tasa_efectividad",
          "actualizado_en"
        ]
      },
      "PaginaRendimientoResponseDto": {
        "type": "object",
        "properties": {
          "data": {
            "description": "Rendimiento por vendedor",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/RendimientoVendedorResponseDto"
            }
          },
          "next_cursor": {
            "type": "object",
            "description": "Cursor de la siguiente página",
            "nullable": true
          }
        },
        "required": [
          "data"
        ]
      },
      "CoberturaZonaResponseDto": {
        "type": "object",
        "properties": {
          "lat_cell": {
            "type": "number",
            "description": "Latitud de la celda",
            "example": 12.1364
          },
          "lng_cell": {
            "type": "number",
            "description": "Longitud de la celda",
            "example": -86.2514
          },
          "total_ventas": {
            "type": "number",
            "description": "Ventas en la celda",
            "example": 18
          },
          "total_monto_cents": {
            "type": "number",
            "description": "Monto vendido en centavos",
            "example": 2070000
          },
          "total_vendedores": {
            "type": "number",
            "description": "Vendedores activos en la celda",
            "example": 5
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          },
          "geojson": {
            "type": "string",
            "description": "Bounding box en GeoJSON",
            "example": "{\"type\":\"Polygon\",\"coordinates\":[]}"
          }
        },
        "required": [
          "lat_cell",
          "lng_cell",
          "total_ventas",
          "total_monto_cents",
          "total_vendedores",
          "actualizado_en",
          "geojson"
        ]
      },
      "DemandaProductoResponseDto": {
        "type": "object",
        "properties": {
          "empresa_id": {
            "type": "string",
            "description": "Empresa dueña del producto; bucket de mercado si la venta no la identifica",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre del producto"
          },
          "total_vendido": {
            "type": "number",
            "description": "Unidades vendidas",
            "example": 120
          },
          "total_monto_cents": {
            "type": "number",
            "description": "Monto vendido en centavos",
            "example": 13800000
          },
          "stock_actual": {
            "type": "number",
            "description": "Stock actual",
            "example": 10
          },
          "ultima_venta_en": {
            "type": "object",
            "description": "Última venta (ISO 8601 UTC)",
            "format": "date-time",
            "nullable": true
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "sku",
          "nombre",
          "total_vendido",
          "total_monto_cents",
          "stock_actual",
          "actualizado_en"
        ]
      },
      "PaginaDemandaResponseDto": {
        "type": "object",
        "properties": {
          "data": {
            "description": "Demanda por producto",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/DemandaProductoResponseDto"
            }
          },
          "next_cursor": {
            "type": "object",
            "description": "Cursor de la siguiente página",
            "nullable": true
          }
        },
        "required": [
          "data"
        ]
      },
      "TendenciaResponseDto": {
        "type": "object",
        "properties": {
          "fecha": {
            "type": "string",
            "description": "Fecha de la tendencia",
            "format": "date"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "total_ventas": {
            "type": "number",
            "description": "Unidades vendidas",
            "example": 12
          },
          "total_monto_cents": {
            "type": "number",
            "description": "Monto vendido en centavos",
            "example": 1380000
          },
          "dia_semana": {
            "type": "number",
            "description": "Día de la semana",
            "example": 3
          }
        },
        "required": [
          "fecha",
          "sku",
          "vendedor_id",
          "total_ventas",
          "total_monto_cents",
          "dia_semana"
        ]
      },
      "ResumenInteligenciaResponseDto": {
        "type": "object",
        "properties": {
          "total_pedidos": {
            "type": "number",
            "description": "Total de pedidos",
            "example": 320
          },
          "ingresos_totales_cents": {
            "type": "number",
            "description": "Ingresos totales en centavos",
            "example": 36800000
          },
          "vendedores_activos": {
            "type": "number",
            "description": "Vendedores activos",
            "example": 20
          },
          "calculado_en": {
            "type": "string",
            "description": "Fecha de cálculo (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "total_pedidos",
          "ingresos_totales_cents",
          "vendedores_activos",
          "calculado_en"
        ]
      },
      "RegistrarVentaResponseDto": {
        "type": "object",
        "properties": {
          "codigo": {
            "type": "string",
            "description": "Código de resultado",
            "example": "OK"
          },
          "mensaje": {
            "type": "string",
            "description": "Mensaje de resultado",
            "example": "Venta registrada en inteligencia"
          }
        },
        "required": [
          "codigo",
          "mensaje"
        ]
      },
      "ForecastDemandaResponseDto": {
        "type": "object",
        "properties": {
          "sku": {
            "type": "string",
            "description": "SKU analizado",
            "example": "SKU-001"
          },
          "dias_proyectados": {
            "type": "number",
            "description": "Días proyectados",
            "example": 7
          },
          "ventas_proyectadas": {
            "type": "number",
            "description": "Ventas proyectadas",
            "example": 84
          },
          "intervalo_inferior": {
            "type": "number",
            "description": "Límite inferior del intervalo",
            "example": 60
          },
          "intervalo_superior": {
            "type": "number",
            "description": "Límite superior del intervalo",
            "example": 108
          },
          "avg_diario_base": {
            "type": "number",
            "description": "Media diaria base",
            "example": 12
          },
          "std_diario_base": {
            "type": "number",
            "description": "Desviación estándar diaria base",
            "example": 3.2
          },
          "features_computadas_en": {
            "type": "object",
            "description": "Fecha de cómputo de features",
            "nullable": true,
            "format": "date-time"
          },
          "metodologia": {
            "type": "string",
            "description": "Metodología aplicada",
            "example": "media_movil_ponderada_7d"
          }
        },
        "required": [
          "sku",
          "dias_proyectados",
          "ventas_proyectadas",
          "intervalo_inferior",
          "intervalo_superior",
          "avg_diario_base",
          "std_diario_base",
          "metodologia"
        ]
      },
      "AnomaliaResponseDto": {
        "type": "object",
        "properties": {
          "order_id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "monto_cents": {
            "type": "number",
            "description": "Monto de la venta en centavos",
            "example": 115000
          },
          "lat": {
            "type": "object",
            "description": "Latitud",
            "nullable": true
          },
          "lng": {
            "type": "object",
            "description": "Longitud",
            "nullable": true
          },
          "ocurrido_en": {
            "type": "string",
            "description": "Fecha de la venta (ISO 8601 UTC)",
            "format": "date-time"
          },
          "media_monto": {
            "type": "number",
            "description": "Media histórica del monto",
            "example": 115000
          },
          "std_monto": {
            "type": "number",
            "description": "Desviación estándar histórica",
            "example": 15000
          },
          "z_score": {
            "type": "number",
            "description": "Z-score de la anomalía",
            "example": 3.4
          }
        },
        "required": [
          "order_id",
          "sku",
          "vendedor_id",
          "monto_cents",
          "ocurrido_en",
          "media_monto",
          "std_monto",
          "z_score"
        ]
      },
      "AnomaliasResponseDto": {
        "type": "object",
        "properties": {
          "total": {
            "type": "number",
            "description": "Total de anomalías",
            "example": 3
          },
          "anomalias": {
            "description": "Anomalías detectadas",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/AnomaliaResponseDto"
            }
          }
        },
        "required": [
          "total",
          "anomalias"
        ]
      },
      "ScoreVendedorResponseDto": {
        "type": "object",
        "properties": {
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "tasa_efectividad": {
            "type": "object",
            "description": "Tasa de efectividad",
            "nullable": true
          },
          "churn_risk_score": {
            "type": "number",
            "description": "Score de riesgo de churn"
          },
          "churn_risk_nivel": {
            "type": "string",
            "description": "Nivel de riesgo",
            "enum": [
              "ALTO",
              "MEDIO",
              "BAJO"
            ]
          },
          "features_computadas_en": {
            "type": "string",
            "description": "Fecha de cómputo de features",
            "format": "date-time"
          },
          "mensaje": {
            "type": "string",
            "description": "Mensaje cuando no hay datos computados"
          }
        }
      },
      "ErrorFrecuenteResponseDto": {
        "type": "object",
        "properties": {
          "event_tipo": {
            "type": "string",
            "description": "Tipo de evento",
            "example": "order.created"
          },
          "error": {
            "type": "string",
            "description": "Error detectado",
            "example": "campo requerido"
          },
          "total": {
            "type": "number",
            "description": "Cantidad de ocurrencias",
            "example": 4
          }
        },
        "required": [
          "event_tipo",
          "error",
          "total"
        ]
      },
      "CalidadDatosResponseDto": {
        "type": "object",
        "properties": {
          "ventana_horas": {
            "type": "number",
            "description": "Ventana analizada en horas",
            "example": 24
          },
          "eventos_procesados": {
            "type": "number",
            "description": "Eventos procesados",
            "example": 1200
          },
          "eventos_invalidos": {
            "type": "number",
            "description": "Eventos inválidos",
            "example": 12
          },
          "tasa_rechazo_pct": {
            "type": "number",
            "description": "Tasa de rechazo (%)",
            "example": 0.99
          },
          "errores_frecuentes": {
            "description": "Errores más frecuentes",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ErrorFrecuenteResponseDto"
            }
          }
        },
        "required": [
          "ventana_horas",
          "eventos_procesados",
          "eventos_invalidos",
          "tasa_rechazo_pct",
          "errores_frecuentes"
        ]
      },
      "ItemOrdenResponseDto": {
        "type": "object",
        "properties": {
          "oferta_id": {
            "type": "string",
            "description": "Id de la oferta",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "producto_nombre": {
            "type": "string",
            "description": "Nombre del producto"
          },
          "cantidad": {
            "type": "number",
            "description": "Cantidad pedida",
            "example": 2
          },
          "precio_unitario_cents": {
            "type": "number",
            "description": "Precio unitario de venta en centavos",
            "example": 115000
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "tienda_id": {
            "type": "string",
            "description": "Id de la tienda",
            "format": "uuid"
          }
        },
        "required": [
          "oferta_id",
          "sku",
          "producto_nombre",
          "cantidad",
          "precio_unitario_cents",
          "vendedor_id",
          "tienda_id"
        ]
      },
      "OrderViewResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la orden",
            "format": "uuid"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "comprador_id": {
            "type": "string",
            "description": "Id del comprador",
            "format": "uuid"
          },
          "tienda_id": {
            "type": "string",
            "description": "Id de la tienda",
            "format": "uuid"
          },
          "estado": {
            "type": "string",
            "description": "Estado de la orden",
            "example": "pagada"
          },
          "total_cents": {
            "type": "number",
            "description": "Total en centavos",
            "example": 230000
          },
          "comision_cents": {
            "type": "number",
            "description": "Comisión en centavos",
            "example": 27600
          },
          "moneda": {
            "type": "string",
            "description": "Moneda",
            "example": "C$"
          },
          "items": {
            "description": "Líneas de la orden",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ItemOrdenResponseDto"
            }
          },
          "direccion_envio": {
            "type": "object",
            "description": "Dirección de envío",
            "nullable": true
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          },
          "pagada_en": {
            "type": "object",
            "description": "Fecha de pago (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          },
          "enviada_en": {
            "type": "object",
            "description": "Fecha de envío (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          },
          "entregada_en": {
            "type": "object",
            "description": "Fecha de entrega (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          },
          "cancelada_en": {
            "type": "object",
            "description": "Fecha de cancelación (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          },
          "devuelta_en": {
            "type": "object",
            "description": "Fecha de devolución (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "vendedor_id",
          "comprador_id",
          "tienda_id",
          "estado",
          "total_cents",
          "comision_cents",
          "moneda",
          "items",
          "creado_en",
          "actualizado_en"
        ]
      },
      "HistoriaEventoResponseDto": {
        "type": "object",
        "properties": {
          "event_id": {
            "type": "string",
            "description": "Id del evento",
            "format": "uuid"
          },
          "aggregate_id": {
            "type": "string",
            "description": "Id del agregado (orden)",
            "format": "uuid"
          },
          "tipo": {
            "type": "string",
            "description": "Tipo de evento",
            "example": "OrderCreatedEvent"
          },
          "payload": {
            "type": "object",
            "description": "Payload del evento"
          },
          "version": {
            "type": "number",
            "description": "Versión del agregado",
            "example": 1
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha del evento (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "event_id",
          "aggregate_id",
          "tipo",
          "payload",
          "version",
          "creado_en"
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
      "ComisionVistaResponseDto": {
        "type": "object",
        "properties": {
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "periodo": {
            "type": "string",
            "description": "Periodo (YYYY-MM)",
            "example": "2026-10"
          },
          "ordenes": {
            "type": "number",
            "description": "Órdenes del periodo",
            "example": 12
          },
          "total_ventas_cents": {
            "type": "number",
            "description": "Ventas del periodo en centavos",
            "example": 1380000
          },
          "total_comision_cents": {
            "type": "number",
            "description": "Comisiones del periodo en centavos",
            "example": 165600
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "vendedor_id",
          "periodo",
          "ordenes",
          "total_ventas_cents",
          "total_comision_cents",
          "actualizado_en"
        ]
      },
      "TimelineEventoResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "number",
            "description": "Secuencia del evento",
            "example": 3
          },
          "tipo": {
            "type": "string",
            "description": "Tipo de evento",
            "example": "OrderStatusUpdatedEvent"
          },
          "payload": {
            "type": "object",
            "description": "Payload del evento"
          },
          "version": {
            "type": "number",
            "description": "Versión del agregado",
            "example": 3
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha del evento (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tipo",
          "payload",
          "version",
          "creado_en"
        ]
      },
      "CarritoItemResponseDto": {
        "type": "object",
        "properties": {
          "oferta_id": {
            "type": "string",
            "description": "Id de la oferta",
            "format": "uuid"
          },
          "cantidad": {
            "type": "number",
            "description": "Cantidad",
            "example": 2
          },
          "id": {
            "type": "string",
            "description": "Id de la oferta (alias enriquecido)",
            "format": "uuid"
          },
          "tienda_id": {
            "type": "string",
            "description": "Id de la tienda",
            "format": "uuid"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU del producto",
            "example": "SKU-001"
          },
          "producto_nombre": {
            "type": "string",
            "description": "Nombre del producto"
          },
          "precio_venta": {
            "type": "string",
            "description": "Precio de venta en córdobas con dos decimales",
            "example": "1150.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock disponible",
            "example": 10
          },
          "estado": {
            "type": "string",
            "description": "Estado de la oferta",
            "example": "activa"
          }
        },
        "required": [
          "oferta_id",
          "cantidad",
          "id",
          "tienda_id",
          "vendedor_id",
          "sku",
          "producto_nombre",
          "precio_venta",
          "stock",
          "estado"
        ]
      },
      "CarritoResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Items del carrito",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/CarritoItemResponseDto"
            }
          },
          "total": {
            "type": "string",
            "description": "Total del carrito en córdobas con dos decimales",
            "example": "2300.00"
          },
          "expira_en": {
            "type": "object",
            "description": "Expiración del carrito (ISO 8601 UTC); null si está vacío",
            "nullable": true,
            "format": "date-time"
          }
        },
        "required": [
          "items",
          "total"
        ]
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
      "EstadoBucketResponseDto": {
        "type": "object",
        "properties": {
          "tenant_id": {
            "type": "string",
            "description": "Id del tenant",
            "format": "uuid"
          },
          "bucket": {
            "type": "string",
            "description": "Nombre del bucket S3",
            "example": "t-00000000-0000-0000-0000-000000000000"
          },
          "cuota_bytes": {
            "type": "number",
            "description": "Cuota asignada en bytes",
            "example": 5368709120
          },
          "usado_bytes": {
            "type": "number",
            "description": "Bytes usados",
            "example": 204800
          },
          "disponible_bytes": {
            "type": "number",
            "description": "Bytes disponibles",
            "example": 5368504320
          },
          "objetos": {
            "type": "number",
            "description": "Objetos activos",
            "example": 12
          },
          "estado": {
            "type": "string",
            "description": "Estado del almacenamiento",
            "enum": [
              "activo",
              "suspendido"
            ],
            "example": "activo"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "actualizado_en": {
            "type": "string",
            "description": "Última actualización (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "tenant_id",
          "bucket",
          "cuota_bytes",
          "usado_bytes",
          "disponible_bytes",
          "objetos",
          "estado",
          "creado_en",
          "actualizado_en"
        ]
      },
      "PaginaBucketsResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Buckets de la página",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/EstadoBucketResponseDto"
            }
          },
          "total": {
            "type": "number",
            "description": "Total de tenants con almacenamiento",
            "example": 8
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
            "example": 1
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
      "ObjetoStorageResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id del objeto",
            "format": "uuid"
          },
          "tenant_id": {
            "type": "string",
            "description": "Id del tenant dueño",
            "format": "uuid"
          },
          "bucket": {
            "type": "string",
            "description": "Bucket S3",
            "example": "t-00000000-0000-0000-0000-000000000000"
          },
          "objeto": {
            "type": "string",
            "description": "Clave del objeto en el bucket",
            "example": "productos/foto.jpg"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre original del archivo",
            "example": "foto-producto.jpg"
          },
          "content_type": {
            "type": "string",
            "description": "Content-Type",
            "example": "image/jpeg"
          },
          "carpeta": {
            "type": "string",
            "description": "Carpeta lógica",
            "example": "productos"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción libre"
          },
          "etiquetas": {
            "type": "object",
            "description": "Etiquetas clave-valor",
            "additionalProperties": {
              "type": "string"
            }
          },
          "tamano_bytes": {
            "type": "number",
            "description": "Tamaño en bytes",
            "example": 204800
          },
          "etag": {
            "type": "object",
            "description": "ETag devuelto por MinIO",
            "nullable": true
          },
          "estado": {
            "type": "string",
            "description": "Estado del objeto",
            "enum": [
              "pendiente",
              "activo",
              "eliminado"
            ],
            "example": "activo"
          },
          "publico": {
            "type": "boolean",
            "description": "Servido por el endpoint estático público (ADR-28)",
            "example": true
          },
          "subido_por": {
            "type": "object",
            "description": "Usuario que subió el objeto",
            "nullable": true
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          },
          "confirmado_en": {
            "type": "object",
            "description": "Fecha de confirmación (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          },
          "eliminado_en": {
            "type": "object",
            "description": "Fecha de eliminación lógica (ISO 8601 UTC)",
            "nullable": true,
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tenant_id",
          "bucket",
          "objeto",
          "nombre",
          "content_type",
          "carpeta",
          "descripcion",
          "etiquetas",
          "tamano_bytes",
          "estado",
          "creado_en"
        ]
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
          },
          "publico": {
            "type": "boolean",
            "description": "Servir el objeto como imagen estática pública (ADR-28)",
            "default": false
          }
        },
        "required": [
          "nombre",
          "content_type",
          "tamano_bytes"
        ]
      },
      "SolicitudSubidaResponseDto": {
        "type": "object",
        "properties": {
          "objeto_id": {
            "type": "string",
            "description": "Id del objeto pendiente",
            "format": "uuid"
          },
          "bucket": {
            "type": "string",
            "description": "Bucket S3"
          },
          "objeto": {
            "type": "string",
            "description": "Clave del objeto en el bucket"
          },
          "url_subida": {
            "type": "string",
            "description": "URL prefirmada para el PUT directo",
            "format": "uri"
          },
          "metodo": {
            "type": "string",
            "description": "Método HTTP de subida",
            "enum": [
              "PUT"
            ],
            "example": "PUT"
          },
          "headers": {
            "type": "object",
            "description": "Cabeceras que debe enviar el cliente",
            "additionalProperties": {
              "type": "string"
            }
          },
          "expira_en": {
            "type": "string",
            "description": "Expiración de la URL (ISO 8601 UTC)",
            "format": "date-time"
          },
          "max_bytes": {
            "type": "number",
            "description": "Tamaño máximo permitido en bytes",
            "example": 204800
          }
        },
        "required": [
          "objeto_id",
          "bucket",
          "objeto",
          "url_subida",
          "metodo",
          "headers",
          "expira_en",
          "max_bytes"
        ]
      },
      "PaginaObjetosResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Objetos de la página",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/ObjetoStorageResponseDto"
            }
          },
          "total": {
            "type": "number",
            "description": "Total de objetos",
            "example": 12
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
            "example": 1
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
      "UrlDescargaResponseDto": {
        "type": "object",
        "properties": {
          "url_descarga": {
            "type": "string",
            "description": "URL prefirmada de descarga",
            "format": "uri"
          },
          "expira_en": {
            "type": "string",
            "description": "Expiración de la URL (ISO 8601 UTC)",
            "format": "date-time"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre original del archivo"
          },
          "content_type": {
            "type": "string",
            "description": "Content-Type",
            "example": "image/jpeg"
          },
          "tamano_bytes": {
            "type": "number",
            "description": "Tamaño en bytes",
            "example": 204800
          }
        },
        "required": [
          "url_descarga",
          "expira_en",
          "nombre",
          "content_type",
          "tamano_bytes"
        ]
      },
      "OfertaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la oferta",
            "format": "uuid"
          },
          "tienda_id": {
            "type": "string",
            "description": "Id de la tienda",
            "format": "uuid"
          },
          "vendedor_id": {
            "type": "string",
            "description": "Id del vendedor",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "description": "Empresa propietaria",
            "format": "uuid"
          },
          "participacion_porcentaje": {
            "type": "number",
            "description": "Participación congelable del vendedor",
            "minimum": 0,
            "maximum": 100
          },
          "producto_id": {
            "type": "string",
            "description": "Id del producto o variante vendible",
            "format": "uuid"
          },
          "producto_padre_id": {
            "type": "object",
            "description": "Id del producto padre (si es variante)",
            "nullable": true,
            "format": "uuid"
          },
          "sku": {
            "type": "string",
            "description": "SKU de la variante o producto",
            "example": "SKU-001"
          },
          "producto_nombre": {
            "type": "string",
            "description": "Nombre del producto/variante"
          },
          "atributos": {
            "type": "object",
            "description": "Atributos de la variante (color, talla, etc.)",
            "additionalProperties": true
          },
          "imagen_objeto_id": {
            "type": "object",
            "description": "Id del objeto de la imagen principal",
            "nullable": true,
            "format": "uuid"
          },
          "imagen_url": {
            "type": "object",
            "description": "URL pública de la imagen principal",
            "nullable": true
          },
          "margen": {
            "type": "number",
            "description": "Margen aplicado (0–90)",
            "example": 15
          },
          "precio_base": {
            "type": "string",
            "description": "Precio base en córdobas con dos decimales",
            "example": "1000.00"
          },
          "precio_venta": {
            "type": "string",
            "description": "Precio de venta en córdobas con dos decimales",
            "example": "1150.00"
          },
          "stock": {
            "type": "number",
            "description": "Stock disponible",
            "example": 10
          },
          "estado": {
            "type": "string",
            "description": "Estado de la oferta",
            "example": "activa"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "tienda_id",
          "vendedor_id",
          "empresa_id",
          "participacion_porcentaje",
          "producto_id",
          "sku",
          "producto_nombre",
          "margen",
          "precio_base",
          "precio_venta",
          "stock",
          "estado",
          "creado_en"
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
          },
          "slug": {
            "type": "string",
            "description": "Slug público. Si se omite se deriva del nombre y se resuelven colisiones con sufijos.",
            "minLength": 2,
            "maxLength": 80,
            "example": "tienda-de-la-vendedora-piloto"
          }
        },
        "required": [
          "nombre"
        ]
      },
      "TiendaResponseDto": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "Id de la tienda",
            "format": "uuid"
          },
          "empresa_id": {
            "type": "string",
            "description": "Empresa propietaria",
            "format": "uuid"
          },
          "propietario_usuario_id": {
            "type": "string",
            "description": "Usuario propietario operativo",
            "format": "uuid"
          },
          "participacion_porcentaje": {
            "type": "number",
            "nullable": true,
            "minimum": 0,
            "maximum": 100
          },
          "vendedor_id": {
            "type": "string",
            "nullable": true,
            "description": "Id del vendedor asignado; null si la tienda está disponible",
            "format": "uuid"
          },
          "nombre": {
            "type": "string",
            "description": "Nombre de la tienda",
            "example": "Mi tienda"
          },
          "slug": {
            "type": "string",
            "description": "Slug público actual",
            "example": "mi-tienda"
          },
          "descripcion": {
            "type": "string",
            "description": "Descripción de la tienda"
          },
          "creado_en": {
            "type": "string",
            "description": "Fecha de creación (ISO 8601 UTC)",
            "format": "date-time"
          }
        },
        "required": [
          "id",
          "empresa_id",
          "propietario_usuario_id",
          "nombre",
          "descripcion",
          "creado_en"
        ]
      },
      "ActualizarSlugTiendaRequestDto": {
        "type": "object",
        "properties": {
          "slug": {
            "type": "string",
            "minLength": 2,
            "maxLength": 80,
            "example": "tienda-de-la-vendedora"
          }
        },
        "required": [
          "slug"
        ]
      },
      "PublicarProductoRequestDto": {
        "type": "object",
        "properties": {
          "tienda_id": {
            "type": "string",
            "description": "Tienda donde se publicará (UUID o slug). Si se omite, se usa la tienda del vendedor autenticado.",
            "example": "mi-tienda"
          },
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
      },
      "PaginaOfertasResponseDto": {
        "type": "object",
        "properties": {
          "items": {
            "description": "Ofertas de la página",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/OfertaResponseDto"
            }
          },
          "total": {
            "type": "number",
            "description": "Total de ofertas",
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
      "TiendaPublicaResponseDto": {
        "type": "object",
        "properties": {
          "tienda": {
            "description": "Datos de la tienda",
            "allOf": [
              {
                "$ref": "#/components/schemas/TiendaResponseDto"
              }
            ]
          },
          "ofertas": {
            "description": "Ofertas de la tienda",
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/OfertaResponseDto"
            }
          }
        },
        "required": [
          "tienda",
          "ofertas"
        ]
      }
    }
  }
} as unknown as OpenAPIObject;
