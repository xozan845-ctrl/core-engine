export const VentaGeolocalizadaSchema = {
  $schema: 'http://json-schema.org/draft-07/schema#',
  type: 'object',
  required: ['order_id', 'vendedor_id', 'skus', 'monto_cents'],
  properties: {
    order_id: { type: 'string' },
    vendedor_id: { type: 'string' },
    skus: { 
      type: 'array', 
      items: { type: 'string' }, 
      minItems: 1 
    },
    monto_cents: { type: 'integer', minimum: 0 },
    lat: { type: 'number', minimum: -90, maximum: 90 },
    lng: { type: 'number', minimum: -180, maximum: 180 },
    precision: { type: 'number', minimum: 0 },
    velocidad: { type: 'number', minimum: 0 },
    rumbo: { type: 'number', minimum: 0, maximum: 360 },
    tipo_actividad: { type: 'string' },
    resultado_visita: { type: 'string' },
    distancia_cliente_metros: { type: 'number', minimum: 0 },
    rango_edad: { type: 'string' },
    genero: { type: 'string', enum: ['M', 'F', 'NS'] }
  },
  additionalProperties: false,
};
