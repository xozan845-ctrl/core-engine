export function validarCierre({
  empresas,
  migraciones,
  packs,
  capacidades,
  packCapacidades,
  empresaCapacidades,
  packRegiones = [],
  jurisdicciones = [],
}) {
  const problemas = [];
  const packsActivos = new Set(packs.filter((pack) => pack.activo).map((pack) => `${pack.codigo}@${pack.version}`));
  const capacidadesConocidas = new Set(capacidades.map((capacidad) => capacidad.codigo));
  const regionesExistentes = new Set(jurisdicciones.map((region) => region.codigo_pais));
  const regionesActivas = new Set(
    jurisdicciones.filter((region) => region.activa).map((region) => region.codigo_pais),
  );

  for (const empresa of empresas) {
    if (!empresa.pack_codigo || !empresa.pack_version) {
      problemas.push(`empresa ${empresa.id}: sin pack asignado`);
    } else {
      if (!packsActivos.has(`${empresa.pack_codigo}@${empresa.pack_version}`)) {
        problemas.push(`empresa ${empresa.id}: pack inexistente o inactivo ${empresa.pack_codigo}@${empresa.pack_version}`);
      }
      if (!empresa.region_tributaria) {
        problemas.push(`empresa ${empresa.id}: sin region tributaria`);
      } else if (
        jurisdicciones.length > 0 &&
        !regionesExistentes.has(empresa.region_tributaria)
      ) {
        problemas.push(`empresa ${empresa.id}: region tributaria inexistente ${empresa.region_tributaria}`);
      } else {
        const delPack = packRegiones
          .filter(
            (region) =>
              region.pack_codigo === empresa.pack_codigo &&
              region.pack_version === empresa.pack_version &&
              region.activo,
          )
          .map((region) => region.region_codigo);
        if (delPack.length > 0 && !delPack.includes(empresa.region_tributaria)) {
          problemas.push(
            `empresa ${empresa.id}: region ${empresa.region_tributaria} no declarada por su pack`,
          );
        }
      }
    }
  }

  for (const clave of packsActivos) {
    const regiones = packRegiones.filter(
      (region) => `${region.pack_codigo}@${region.pack_version}` === clave && region.activo,
    );
    const activas = regiones.filter((region) => regionesActivas.has(region.region_codigo));
    if (activas.length === 0) {
      problemas.push(`pack ${clave}: sin region tributaria activa`);
      continue;
    }
    const predeterminadas = activas.filter((region) => region.predeterminada);
    if (predeterminadas.length !== 1) {
      problemas.push(`pack ${clave}: debe tener exactamente una region predeterminada`);
    }
    for (const region of regiones) {
      if (jurisdicciones.length > 0 && !regionesExistentes.has(region.region_codigo)) {
        problemas.push(`pack ${clave}: region inexistente ${region.region_codigo}`);
      }
    }
  }

  for (const migracion of migraciones) {
    if (migracion.estado !== 'aplicado') {
      problemas.push(`empresa ${migracion.empresa_id}: migracion ${migracion.estado}`);
    }
  }

  for (const capacidad of packCapacidades) {
    if (!capacidadesConocidas.has(capacidad.capacidad_codigo)) {
      problemas.push(`pack ${capacidad.pack_codigo}@${capacidad.pack_version}: capacidad desconocida ${capacidad.capacidad_codigo}`);
    }
  }

  const empresasConPack = new Set(empresas.map((empresa) => empresa.id));
  for (const capacidad of empresaCapacidades) {
    if (!empresasConPack.has(capacidad.empresa_id)) {
      problemas.push(`capacidad ${capacidad.capacidad_codigo}: empresa inexistente ${capacidad.empresa_id}`);
    }
  }

  return {
    ok: problemas.length === 0,
    problemas,
    resumen: {
      empresas: empresas.length,
      packs_activos: packsActivos.size,
      migraciones_pendientes: migraciones.filter((migracion) => migracion.estado !== 'aplicado').length,
      regiones_activas: regionesActivas.size,
    },
  };
}

async function ejecutar() {
  const { default: pg } = await import('pg');
  const connectionString = process.env.DATABASE_URL ?? process.env.TEST_DATABASE_URL;
  if (!connectionString) throw new Error('DATABASE_URL o TEST_DATABASE_URL requerida');
  const client = new pg.Client({ connectionString });
  await client.connect();
  try {
    const empresas = await client.query(
      'SELECT id, pack_codigo, pack_version, region_tributaria FROM identity.empresas',
    );
    const migraciones = await client.query('SELECT empresa_id, estado FROM identity.pack_migraciones');
    const packs = await client.query('SELECT codigo, version, activo FROM identity.packs');
    const capacidades = await client.query('SELECT codigo FROM identity.capacidades');
    const packCapacidades = await client.query('SELECT pack_codigo, pack_version, capacidad_codigo FROM identity.pack_capacidades');
    const empresaCapacidades = await client.query('SELECT empresa_id, capacidad_codigo FROM identity.empresa_capacidades');
    const packRegiones = await client.query(
      'SELECT pack_codigo, pack_version, region_codigo, predeterminada, activo FROM identity.pack_regiones',
    );
    const jurisdicciones = await client.query('SELECT codigo_pais, activa FROM finance.jurisdicciones');
    const resultado = validarCierre({
      empresas: empresas.rows,
      migraciones: migraciones.rows,
      packs: packs.rows,
      capacidades: capacidades.rows,
      packCapacidades: packCapacidades.rows,
      empresaCapacidades: empresaCapacidades.rows,
      packRegiones: packRegiones.rows,
      jurisdicciones: jurisdicciones.rows,
    });
    console.log(JSON.stringify(resultado));
    if (!resultado.ok) process.exitCode = 1;
  } finally {
    await client.end();
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  ejecutar().catch((error) => {
    console.error(JSON.stringify({ nivel: 'error', msg: error.message }));
    process.exitCode = 1;
  });
}
