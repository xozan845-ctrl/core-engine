/**
 * Diff focalizado de OpenAPI (R-C-7): detecta cambios incompatibles sin
 * dependencias externas.
 *
 * Cubre lo que rompe consumidores: paths/métodos eliminados, códigos de
 * respuesta eliminados, propiedades de respuesta eliminadas, cambios de tipo y
 * propiedades de request que pasan a ser obligatorias.
 */

function deref(spec, schema) {
  let current = schema;
  const seen = new Set();

  while (current?.$ref && !seen.has(current.$ref)) {
    seen.add(current.$ref);
    const keys = current.$ref.replace(/^#\//, '').split('/');
    current = keys.reduce((node, key) => node?.[key], spec);
  }

  if (!current) return {};
  if (Array.isArray(current.allOf)) {
    const merged = { type: 'object', properties: {}, required: [] };
    for (const part of current.allOf) {
      const resolved = deref(spec, part);
      Object.assign(merged.properties, resolved.properties ?? {});
      merged.required.push(...(resolved.required ?? []));
    }
    Object.assign(merged.properties, current.properties ?? {});
    merged.required.push(...(current.required ?? []));
    return merged;
  }
  return current;
}

function compareSchema(oldSpec, newSpec, oldSchema, newSchema, context, problems) {
  const before = deref(oldSpec, oldSchema);
  const after = deref(newSpec, newSchema);

  if (before.type && after.type && before.type !== after.type) {
    problems.push(`${context}: tipo '${before.type}' → '${after.type}'`);
    return;
  }

  if (before.type === 'array' || after.type === 'array') {
    if (before.items && after.items) {
      compareSchema(oldSpec, newSpec, before.items, after.items, context, problems);
    }
    return;
  }

  const beforeProps = before.properties ?? {};
  const afterProps = after.properties ?? {};
  for (const name of Object.keys(beforeProps)) {
    if (!(name in afterProps)) {
      problems.push(`${context}.${name}: propiedad eliminada`);
      continue;
    }
    compareSchema(
      oldSpec,
      newSpec,
      beforeProps[name],
      afterProps[name],
      `${context}.${name}`,
      problems,
    );
  }

  const beforeRequired = new Set(before.required ?? []);
  for (const name of after.required ?? []) {
    if (!beforeRequired.has(name)) {
      problems.push(`${context}.${name}: propiedad nueva obligatoria`);
    }
  }
}

function responseSchema(operation) {
  const responses = operation.responses ?? {};
  for (const [status, response] of Object.entries(responses)) {
    const schema = response?.content?.['application/json']?.schema;
    if (schema) return { status, schema };
  }
  return null;
}

/**
 * @param {object} oldSpec
 * @param {object} newSpec
 * @returns {string[]}
 */
export function compareSpecs(oldSpec, newSpec) {
  const problems = [];
  const oldPaths = oldSpec.paths ?? {};
  const newPaths = newSpec.paths ?? {};

  for (const [path, oldPath] of Object.entries(oldPaths)) {
    const newPath = newPaths[path];
    if (!newPath) {
      problems.push(`${path}: path eliminado`);
      continue;
    }

    for (const [method, oldOperation] of Object.entries(oldPath)) {
      if (!['get', 'post', 'put', 'patch', 'delete'].includes(method)) continue;
      const newOperation = newPath[method];
      if (!newOperation) {
        problems.push(`${method.toUpperCase()} ${path}: operación eliminada`);
        continue;
      }

      for (const status of Object.keys(oldOperation.responses ?? {})) {
        if (!(status in (newOperation.responses ?? {}))) {
          problems.push(`${method.toUpperCase()} ${path}: respuesta ${status} eliminada`);
        }
      }

      const oldResponse = responseSchema(oldOperation);
      const newResponse = responseSchema(newOperation);
      if (oldResponse && newResponse) {
        compareSchema(
          oldSpec,
          newSpec,
          oldResponse.schema,
          newResponse.schema,
          `${method.toUpperCase()} ${path} response ${newResponse.status}`,
          problems,
        );
      }

      const oldRequest = oldOperation.requestBody?.content?.['application/json']?.schema;
      const newRequest = newOperation.requestBody?.content?.['application/json']?.schema;
      if (oldRequest && newRequest) {
        compareSchema(
          oldSpec,
          newSpec,
          oldRequest,
          newRequest,
          `${method.toUpperCase()} ${path} request`,
          problems,
        );
      }
    }
  }

  return problems;
}
