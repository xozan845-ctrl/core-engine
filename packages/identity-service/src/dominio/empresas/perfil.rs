use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    pub async fn actualizar_perfil(
        &self,
        usuario_id: &str,
        identificador: &str,
        cambios: &Value,
        es_global: bool,
    ) -> Result<Value, ApiError> {
        let (datos, _) = self
            .buscar_por_identificador(identificador)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Empresa", identificador))?;
        let empresa_id = texto(&datos, "id");
        if !es_global {
            self.autorizar_membresia(&empresa_id, usuario_id, Some("empresario"))
                .await?;
        }
        let region = self.regiones.de_empresa(&empresa_id).await?;
        let meta = self.normalizar_perfil(cambios, &datos, region.as_ref().map(|r| &r.region));
        self.validar_perfil(&meta)?;
        validar_background(&BackgroundInput::desde(cambios), &background_de(&meta))?;
        let ruc = texto(&meta, "ruc");
        if ruc != texto(&datos, "ruc") {
            self.validar_ruc_unico(&ruc, Some(&empresa_id)).await?;
        }
        let logo = meta.get("logo_objeto_id").and_then(Value::as_str);
        if logo != datos.get("logo_objeto_id").and_then(Value::as_str) {
            if let Some(logo) = logo {
                self.storage.validar_logo(&empresa_id, logo).await?;
            }
        }
        let mut media = meta
            .get("background_media_type")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(objeto) = meta.get("background_objeto_id").and_then(Value::as_str) {
            if Some(objeto) != datos.get("background_objeto_id").and_then(Value::as_str) {
                media = Some(
                    self.storage
                        .validar_media(&empresa_id, objeto, "fondo")
                        .await?,
                );
            }
        } else {
            media = None;
        }
        if let Some(poster) = meta
            .get("background_poster_objeto_id")
            .and_then(Value::as_str)
        {
            if Some(poster)
                != datos
                    .get("background_poster_objeto_id")
                    .and_then(Value::as_str)
            {
                self.storage
                    .validar_media(&empresa_id, poster, "poster")
                    .await?;
            }
        }
        let plantilla = meta
            .get("plantilla")
            .and_then(Value::as_str)
            .map(str::to_string);
        let tema = meta.get("tema").cloned().unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.empresas AS e SET
               ruc=$2, razon_social=$3, nombre_comercial=$4, descripcion=$5,
               telefono=$6, correo_contacto=$7, sitio_web=$8, direccion=$9,
               ciudad=$10, departamento=$11, pais=$12, moneda=$13, zona_horaria=$14,
               sector=$15, representante_legal=$16, horario_atencion=$17,
               logo_objeto_id=$18::uuid, background_objeto_id=$19::uuid,
               background_media_type=$20, background_poster_objeto_id=$21::uuid,
               background_alt=$22, background_overlay=$23, background_position=$24,
               background_activo=$25, plantilla_override=$26, tema_override=$27::jsonb,
               actualizado_en=NOW()
             WHERE e.id = $1::uuid
             RETURNING to_jsonb(e) AS datos",
        )
        .bind(&empresa_id)
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
        .bind(logo)
        .bind(meta.get("background_objeto_id").and_then(Value::as_str))
        .bind(media)
        .bind(
            meta.get("background_poster_objeto_id")
                .and_then(Value::as_str),
        )
        .bind(texto(&meta, "background_alt"))
        .bind(
            meta.get("background_overlay")
                .and_then(Value::as_i64)
                .unwrap_or(35),
        )
        .bind(texto(&meta, "background_position"))
        .bind(
            meta.get("background_activo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(plantilla)
        .bind(tema)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto("El RUC ya está registrado en otra empresa.")
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", &empresa_id))?;
        tx.commit().await.map_err(interno)?;
        let nuevos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(Self::con_urls(&nuevos))
    }

    pub(super) fn normalizar_perfil(
        &self,
        datos: &Value,
        actual: &Value,
        region: Option<&RegionTributaria>,
    ) -> Value {
        let pais_respaldo = region.map(|region| region.nombre.as_str()).unwrap_or("");
        let moneda_respaldo = region
            .map(|region| {
                if region.moneda_iso.is_empty() {
                    region.moneda.as_str()
                } else {
                    region.moneda_iso.as_str()
                }
            })
            .unwrap_or("");
        let zona_respaldo = region
            .map(|region| region.zona_horaria.as_str())
            .unwrap_or("UTC");
        let texto = |clave: &str, respaldo: &str| {
            datos
                .get(clave)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_else(|| {
                    actual
                        .get(clave)
                        .and_then(Value::as_str)
                        .unwrap_or(respaldo)
                })
                .to_string()
        };
        let mut perfil = Map::new();
        perfil.insert("ruc".into(), json!(texto("ruc", "")));
        perfil.insert("razon_social".into(), json!(texto("razon_social", "")));
        perfil.insert(
            "nombre_comercial".into(),
            json!(texto("nombre_comercial", "")),
        );
        perfil.insert("descripcion".into(), json!(texto("descripcion", "")));
        perfil.insert("telefono".into(), json!(texto("telefono", "")));
        perfil.insert(
            "correo_contacto".into(),
            json!(texto("correo_contacto", "")),
        );
        perfil.insert("sitio_web".into(), json!(texto("sitio_web", "")));
        perfil.insert("direccion".into(), json!(texto("direccion", "")));
        perfil.insert("ciudad".into(), json!(texto("ciudad", "")));
        perfil.insert("departamento".into(), json!(texto("departamento", "")));
        perfil.insert(
            "pais".into(),
            json!(texto("pais", pais_respaldo).if_empty(pais_respaldo)),
        );
        perfil.insert(
            "moneda".into(),
            json!(texto("moneda", moneda_respaldo)
                .if_empty(moneda_respaldo)
                .to_uppercase()),
        );
        perfil.insert(
            "zona_horaria".into(),
            json!(texto("zona_horaria", zona_respaldo).if_empty(zona_respaldo)),
        );
        perfil.insert("sector".into(), json!(texto("sector", "")));
        perfil.insert(
            "representante_legal".into(),
            json!(texto("representante_legal", "")),
        );
        perfil.insert(
            "horario_atencion".into(),
            json!(texto("horario_atencion", "")),
        );

        let logo = match datos.get("logo_objeto_id") {
            None => actual
                .get("logo_objeto_id")
                .and_then(Value::as_str)
                .map(|valor| json!(valor))
                .unwrap_or(Value::Null),
            Some(Value::Null) => Value::Null,
            Some(Value::String(valor)) => json!(valor.trim()),
            Some(_) => Value::Null,
        };
        perfil.insert("logo_objeto_id".into(), logo);

        let plantilla_actual = actual
            .get("plantilla_override")
            .and_then(Value::as_str)
            .filter(|valor| es_plantilla(valor))
            .map(|valor| json!(valor))
            .unwrap_or(Value::Null);
        let plantilla = match datos.get("plantilla") {
            None => plantilla_actual,
            Some(Value::Null) => Value::Null,
            Some(Value::String(valor)) if es_plantilla(valor) => json!(valor),
            Some(_) => plantilla_actual,
        };
        perfil.insert("plantilla".into(), plantilla);

        let actual_tema = actual
            .get("tema_override")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let tema = match datos.get("tema") {
            None => actual_tema,
            Some(Value::Null) => json!({}),
            Some(valor @ Value::Object(_)) => {
                let mut mezcla = actual_tema.as_object().cloned().unwrap_or_default();
                if let Some(parcial) = normalizar_tema_parcial(valor).as_object().cloned() {
                    for (clave, valor) in parcial {
                        mezcla.insert(clave, valor);
                    }
                }
                Value::Object(mezcla)
            }
            Some(_) => actual_tema,
        };
        perfil.insert("tema".into(), tema);

        let background =
            normalizar_background(&BackgroundInput::desde(datos), Some(&background_de(actual)));
        if let Some(objeto) = serde_json::to_value(background)
            .ok()
            .and_then(|valor| valor.as_object().cloned())
        {
            for (clave, valor) in objeto {
                perfil.insert(clave, valor);
            }
        }
        Value::Object(perfil)
    }

    pub(super) fn validar_perfil(&self, meta: &Value) -> Result<(), ApiError> {
        let limites = [
            ("ruc", 30),
            ("razon_social", 160),
            ("nombre_comercial", 160),
            ("descripcion", 1000),
            ("telefono", 40),
            ("sitio_web", 200),
            ("direccion", 300),
            ("ciudad", 80),
            ("departamento", 80),
            ("pais", 80),
            ("zona_horaria", 64),
            ("sector", 80),
            ("representante_legal", 160),
            ("horario_atencion", 120),
        ];
        for (campo, maximo) in limites {
            if texto(meta, campo).chars().count() > maximo {
                return Err(ApiError::dominio(
                    "PERFIL_INVALIDO",
                    format!("{campo} supera {maximo} caracteres."),
                ));
            }
        }
        let moneda = texto(meta, "moneda");
        if moneda.len() != 3 || !moneda.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "La moneda debe ser un código ISO de 3 letras.",
            ));
        }
        let correo = texto(meta, "correo_contacto");
        if !correo.is_empty() && !correo_valido(&correo) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "El correo de contacto no es válido.",
            ));
        }
        if let Some(logo) = meta.get("logo_objeto_id").and_then(Value::as_str) {
            if !es_uuid(logo) {
                return Err(ApiError::dominio(
                    "PERFIL_INVALIDO",
                    "El logo debe ser un objeto de storage válido.",
                ));
            }
        }
        let plantilla = meta.get("plantilla").and_then(Value::as_str);
        if plantilla.is_some_and(|valor| !PLANTILLAS.contains(&valor)) {
            return Err(ApiError::dominio(
                "PERFIL_INVALIDO",
                "La plantilla no es válida.",
            ));
        }
        validar_tema_input(meta.get("tema").unwrap_or(&json!({})))?;
        validar_tema(&mezclar_tema(&[meta.get("tema").unwrap_or(&json!({}))]))
    }

    pub(super) async fn validar_ruc_unico(
        &self,
        ruc: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if ruc.is_empty() {
            return Ok(());
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match excluir_id {
            Some(id) => sqlx::query(
                "SELECT 1 FROM identity.empresas
                  WHERE upper(ruc) = upper($1) AND id <> $2::uuid LIMIT 1",
            )
            .bind(ruc)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => {
                sqlx::query("SELECT 1 FROM identity.empresas WHERE upper(ruc) = upper($1) LIMIT 1")
                    .bind(ruc)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(interno)?
            }
        };
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(
                "El RUC ya está registrado en otra empresa.",
            ));
        }
        Ok(())
    }
}
