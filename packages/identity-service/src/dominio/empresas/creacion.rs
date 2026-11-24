use super::comunes::*;
use super::packs::resolver_pack;
use super::prelude::*;
use super::*;

impl Empresas {
    pub async fn crear(&self, input: CrearEmpresa) -> Result<Value, ApiError> {
        let nombre = input.nombre.trim().to_string();
        let vertical = (input.vertical_id.unwrap_or_default())
            .trim()
            .to_lowercase();
        if !es_codigo_segmento(&vertical) {
            return Err(ApiError::dominio(
                "VERTICAL_REQUERIDA",
                "Selecciona una vertical para la empresa.",
            ));
        }
        let Some(pack_codigo) = input
            .pack_codigo
            .map(|valor| valor.trim().to_lowercase())
            .filter(|valor| !valor.is_empty())
        else {
            return Err(ApiError::dominio(
                "PACK_REQUERIDO",
                "Selecciona un pack para la empresa.",
            ));
        };
        let mut perfil_datos = input.perfil.clone();
        if !perfil_datos.is_object() {
            perfil_datos = json!({});
        }
        if let Some(objeto) = perfil_datos.as_object_mut() {
            objeto.insert("logo_objeto_id".into(), Value::Null);
            objeto.insert("background_objeto_id".into(), Value::Null);
        }
        // Pack y región se resuelven antes de normalizar el perfil: país,
        // moneda y zona horaria heredan la región elegida en vez de un país
        // hardcodeado.
        let (pack, region) = {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let pack =
                resolver_pack(&mut tx, &pack_codigo, Some(&vertical), input.pack_version).await?;
            let region = resolver_en(
                &mut tx,
                &pack.codigo,
                pack.version,
                input.region_tributaria.as_deref(),
            )
            .await?;
            tx.commit().await.map_err(interno)?;
            (pack, region)
        };
        let meta = self.normalizar_perfil(&perfil_datos, &json!({}), Some(&region));
        self.validar_perfil(&meta)?;
        let ruc = texto(&meta, "ruc");
        self.validar_ruc_unico(&ruc, None).await?;

        let slug_explicito = match input.slug.as_deref() {
            Some(valor) => Some(validar_slug_solicitado(valor)?),
            None => None,
        };
        let max_intentos = if slug_explicito.is_some() { 1 } else { 3 };

        for intento in 1..=max_intentos {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let slug = elegir_slug(&mut tx, &nombre, slug_explicito.as_deref()).await?;
            let plantilla_override = meta.get("plantilla").and_then(Value::as_str);
            let tema_override = meta.get("tema").cloned().unwrap_or_else(|| json!({}));
            let insertado = sqlx::query(
                "INSERT INTO identity.empresas AS e
                   (nombre, slug, vertical_id, pack_codigo, pack_version, creado_por,
                    ruc, razon_social, nombre_comercial, descripcion, telefono,
                    correo_contacto, sitio_web, direccion, ciudad, departamento, pais,
                    moneda, zona_horaria, sector, representante_legal, horario_atencion,
                    plantilla_override, tema_override, logo_objeto_id,
                    region_tributaria, region_tributaria_desde)
                 VALUES ($1,$2,$3,$4,$5,$6::uuid,$7,$8,$9,$10,$11,$12,$13,$14,
                         $15,$16,$17,$18,$19,$20,$21,$22,$23,$24,NULL,$25,CURRENT_DATE)
                 RETURNING to_jsonb(e) AS datos",
            )
            .bind(&nombre)
            .bind(&slug)
            .bind(&pack.vertical_codigo)
            .bind(&pack.codigo)
            .bind(pack.version)
            .bind(&input.usuario_id)
            .bind(texto(&meta, "ruc"))
            .bind(texto(&meta, "razon_social"))
            .bind(texto(&meta, "nombre_comercial"))
            .bind(texto(&meta, "descripcion"))
            .bind(texto(&meta, "telefono"))
            .bind(texto(&meta, "correo_contacto"))
            .bind(texto(&meta, "sitio_web"))
            .bind(texto(&meta, "direccion"))
            .bind(texto(&meta, "ciudad"))
            .bind(texto(&meta, "departamento"))
            .bind(texto(&meta, "pais"))
            .bind(texto(&meta, "moneda"))
            .bind(texto(&meta, "zona_horaria"))
            .bind(texto(&meta, "sector"))
            .bind(texto(&meta, "representante_legal"))
            .bind(texto(&meta, "horario_atencion"))
            .bind(plantilla_override)
            .bind(tema_override)
            .bind(&region.codigo)
            .fetch_optional(&mut *tx)
            .await;

            let fila = match insertado {
                Ok(fila) => fila,
                Err(error) => {
                    if es_unico(&error) {
                        if slug_explicito.is_some() || intento == max_intentos {
                            return Err(ApiError::conflicto(format!(
                                "El slug \"{}\" ya está en uso.",
                                slug_explicito.as_deref().unwrap_or(&nombre)
                            )));
                        }
                        continue;
                    }
                    return Err(interno(error));
                }
            };
            let Some(fila) = fila else {
                return Err(ApiError::interno());
            };
            let datos: Value = fila.try_get("datos").map_err(interno)?;
            let empresa_id = texto(&datos, "id");
            sqlx::query("SELECT set_config('request.jwt.claims', $1, true)")
                .bind(
                    json!({
                        "sub": input.usuario_id,
                        "rol": "empresario",
                        "tenant_id": empresa_id,
                    })
                    .to_string(),
                )
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.negocios
                   (empresa_id, codigo, nombre, vertical_id, pack_codigo,
                    pack_version, es_predeterminado, creado_por)
                 VALUES ($1::uuid, 'principal', $2, $3, $4, $5, true, $6::uuid)
                 ON CONFLICT (empresa_id, codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(if nombre.chars().count() > 120 {
                nombre.chars().take(120).collect::<String>()
            } else {
                nombre.clone()
            })
            .bind(&pack.vertical_codigo)
            .bind(&pack.codigo)
            .bind(pack.version)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            auditoria::registrar(
                &mut tx,
                &input.usuario_id,
                &empresa_id,
                None,
                "empresa.creada",
                "empresa",
                Some(&empresa_id),
                json!({ "nombre": nombre, "slug": slug }),
            )
            .await?;
            sqlx::query(
                "INSERT INTO identity.roles_empresa
                   (empresa_id, negocio_id, codigo, nombre, permisos, creado_por)
                 SELECT $1::uuid, n.id, p.codigo, p.nombre, p.permisos, $2::uuid
                   FROM identity.negocios n
                   JOIN identity.plantillas_roles p ON p.vertical_id = n.vertical_id
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO inventory.ubicaciones
                   (empresa_id, negocio_id, codigo, nombre, tipo, predeterminada)
                 SELECT $1::uuid, n.id,
                        'principal_' || substr(replace(n.id::text, '-', ''), 1, 8),
                        'Principal', 'bodega', true
                   FROM identity.negocios n
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.roles_empresa
                   (empresa_id, codigo, nombre, permisos, creado_por)
                 VALUES ($1::uuid, 'empresario', 'Empresario', '{}'::jsonb, $2::uuid)
                 ON CONFLICT DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.asignaciones_personal
                   (empresa_id, usuario_id, rol_empresa_id, estado, creado_por)
                 SELECT $1::uuid, $2::uuid, r.id, 'activa', $2::uuid
                   FROM identity.roles_empresa r
                  WHERE r.empresa_id = $1::uuid AND r.codigo = 'empresario'",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "INSERT INTO identity.empresa_capacidades
                   (empresa_id, capacidad_codigo, config, activo)
                 SELECT $1::uuid, capacidad_codigo, config, activo
                   FROM identity.pack_capacidades
                  WHERE pack_codigo = $2 AND pack_version = $3
                 ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .bind(&pack.codigo)
            .bind(pack.version)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            for capacidad in &input.capacidades_extra {
                let codigo = capacidad.trim().to_lowercase();
                if codigo.is_empty() {
                    continue;
                }
                let disponible = sqlx::query(
                    "SELECT 1 FROM identity.vertical_capacidades vc
                       JOIN identity.capacidades c ON c.codigo = vc.capacidad_codigo
                      WHERE vc.vertical_codigo = $1 AND vc.capacidad_codigo = $2
                        AND c.activo = true",
                )
                .bind(&pack.vertical_codigo)
                .bind(&codigo)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
                if disponible.is_none() {
                    return Err(ApiError::dominio(
                        "CAPACIDAD_NO_DISPONIBLE",
                        format!("La capacidad \"{codigo}\" no esta disponible para la vertical."),
                    ));
                }
                sqlx::query(
                    "INSERT INTO identity.empresa_capacidades AS ec
                       (empresa_id, capacidad_codigo, config, activo)
                     VALUES ($1::uuid, $2, '{}'::jsonb, true)
                     ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
                       SET activo = true, actualizado_en = NOW()",
                )
                .bind(&empresa_id)
                .bind(&codigo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            sqlx::query(
                "INSERT INTO identity.negocio_capacidades
                   (negocio_id, capacidad_codigo, config, activo)
                 SELECT n.id, ec.capacidad_codigo, ec.config, ec.activo
                   FROM identity.negocios n
                   JOIN identity.empresa_capacidades ec ON ec.empresa_id = n.empresa_id
                  WHERE n.empresa_id = $1::uuid AND n.es_predeterminado
                 ON CONFLICT (negocio_id, capacidad_codigo) DO NOTHING",
            )
            .bind(&empresa_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            sqlx::query(
                "UPDATE identity.usuarios SET tenant_id = $1::uuid, rol = 'empresario'
                  WHERE id = $2::uuid",
            )
            .bind(&empresa_id)
            .bind(&input.usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos));
        }
        Err(ApiError::conflicto(
            "No se pudo asignar un slug disponible.",
        ))
    }

    /// Resuelve por UUID o slug (actual o histórico) y autoriza la membresía.
    pub async fn resolver_para(
        &self,
        usuario_id: &str,
        identificador: &str,
        es_global: bool,
        rol_miembro: Option<&str>,
    ) -> Result<(Value, Option<String>), ApiError> {
        let (datos, es_actual) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        if !es_global {
            self.autorizar_membresia(&empresa_id, usuario_id, rol_miembro)
                .await?;
        }
        let respuesta = Self::con_urls(&datos);
        let redireccion = if es_actual {
            None
        } else {
            Some(format!("/api/v2/empresas/{}", texto(&datos, "slug")))
        };
        Ok((respuesta, redireccion))
    }

    pub async fn actualizar_slug(
        &self,
        usuario_id: &str,
        identificador: &str,
        slug_solicitado: &str,
    ) -> Result<Value, ApiError> {
        let (datos, _) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        self.autorizar_membresia(&empresa_id, usuario_id, Some("empresario"))
            .await?;
        let slug = validar_slug_solicitado(slug_solicitado)?;
        if slug == texto(&datos, "slug") {
            return Ok(Self::con_urls(&datos));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ocupado = sqlx::query("SELECT 1 FROM identity.empresa_slugs WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        if ocupado.is_some() {
            return Err(ApiError::conflicto(format!(
                "El slug \"{slug}\" ya está en uso."
            )));
        }
        let fila = sqlx::query(
            "UPDATE identity.empresas AS e SET slug = $1
              WHERE e.id = $2::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(&slug)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto(format!("El slug \"{slug}\" ya está en uso."))
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", &empresa_id))?;
        tx.commit().await.map_err(interno)?;
        let nuevos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(Self::con_urls(&nuevos))
    }
}

async fn elegir_slug(
    tx: &mut Transaction<'_, Postgres>,
    nombre: &str,
    explicito: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(explicito) = explicito {
        if !slug_libre(tx, explicito).await? {
            return Err(ApiError::conflicto(format!(
                "El slug \"{explicito}\" ya está en uso."
            )));
        }
        return Ok(explicito.to_string());
    }
    let base = slug_desde_nombre(nombre, "empresa");
    for intento in 1..=MAX_INTENTOS_SLUG {
        let candidato = if intento == 1 {
            base.clone()
        } else {
            slug_con_sufijo(&base, intento)
        };
        if slug_libre(tx, &candidato).await? {
            return Ok(candidato);
        }
    }
    let sufijo = format!("-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let maximo = crate::dominio::slug::SLUG_MAX_LENGTH.saturating_sub(sufijo.len());
    let recorte = base
        .chars()
        .take(maximo)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();
    Ok(format!(
        "{}{sufijo}",
        if recorte.is_empty() {
            "empresa"
        } else {
            &recorte
        }
    ))
}

async fn slug_libre(tx: &mut Transaction<'_, Postgres>, slug: &str) -> Result<bool, ApiError> {
    let fila = sqlx::query("SELECT 1 FROM identity.empresa_slugs WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
    Ok(fila.is_none())
}

pub(super) fn es_codigo_segmento(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
