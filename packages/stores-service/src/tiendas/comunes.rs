use super::*;

impl Tiendas {
    pub(super) async fn con_urls(&self, fila: &Value) -> Result<Value, ApiError> {
        let overrides = overrides_de(fila);
        let empresa_cruda = self.temas.obtener_empresa(&texto(fila, "empresa_id")).await;
        let plataforma_cruda = self.temas.obtener_plataforma().await;
        let empresa = TemaHerencia {
            plantilla: plantilla_de(&empresa_cruda.plantilla_override),
            tema: tema_parcial_de(&empresa_cruda.tema_override),
        };
        let plataforma = TemaHerencia {
            plantilla: plantilla_de(&plataforma_cruda.plantilla_override),
            tema: tema_parcial_de(&plataforma_cruda.tema_override),
        };
        let overrides_tema = TemaHerencia {
            plantilla: overrides.plantilla.clone(),
            tema: overrides.tema.clone(),
        };
        let resuelta = resolver_personalizacion(&overrides_tema, &empresa, &plataforma);
        let background = background_de(fila);
        let movil = background_movil_de(fila);
        let contenido = contenido_de(fila);

        // El orden de claves replica el spread de objetos de `conUrls`: se
        // parte de la fila (columnas en el orden del SELECT), se reemplazan los
        // campos normalizados en su sitio y las claves nuevas se añaden al
        // final en el mismo orden que TypeScript.
        let mut objeto = fila.as_object().cloned().unwrap_or_default();
        objeto.remove("plantilla_override");
        objeto.remove("tema_override");
        objeto.remove("es_actual");

        insertar(
            &mut objeto,
            "background_objeto_id",
            opt(background.background_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_media_type",
            opt(background.background_media_type.clone()),
        );
        insertar(
            &mut objeto,
            "background_poster_objeto_id",
            opt(background.background_poster_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_alt",
            Value::String(background.background_alt.clone()),
        );
        insertar(
            &mut objeto,
            "background_overlay",
            Value::from(background.background_overlay),
        );
        insertar(
            &mut objeto,
            "background_position",
            Value::String(background.background_position.clone()),
        );
        insertar(
            &mut objeto,
            "background_activo",
            Value::Bool(background.background_activo),
        );
        insertar(
            &mut objeto,
            "background_movil_objeto_id",
            opt(movil.background_movil_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_media_type",
            opt(movil.background_movil_media_type.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_poster_objeto_id",
            opt(movil.background_movil_poster_objeto_id.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_alt",
            Value::String(movil.background_movil_alt.clone()),
        );
        insertar(
            &mut objeto,
            "background_movil_activo",
            Value::Bool(movil.background_movil_activo),
        );
        for (clave, valor) in personalizacion_json(&overrides, &resuelta) {
            if clave == "plantilla" || clave == "tema" {
                continue;
            }
            insertar(&mut objeto, &clave, valor);
        }
        if let Some(campos) = contenido_json(&contenido).as_object().cloned() {
            for (clave, valor) in campos {
                insertar(&mut objeto, &clave, valor);
            }
        }
        if texto_opcional(fila, "logo_objeto_id").is_some() {
            insertar(
                &mut objeto,
                "logo_url",
                Value::String(format!(
                    "/api/v2/storage/public/objetos/{}",
                    texto(fila, "logo_objeto_id")
                )),
            );
        }
        for (clave, valor) in background_urls(&background)
            .as_object()
            .cloned()
            .unwrap_or_default()
        {
            insertar(&mut objeto, &clave, valor);
        }
        for (clave, valor) in background_movil_urls(&movil)
            .as_object()
            .cloned()
            .unwrap_or_default()
        {
            insertar(&mut objeto, &clave, valor);
        }
        insertar(
            &mut objeto,
            "plantilla",
            Value::String(resuelta.plantilla.clone()),
        );
        insertar(
            &mut objeto,
            "tema",
            serde_json::to_value(&resuelta.tema).map_err(crate::db::interno)?,
        );
        insertar(
            &mut objeto,
            "seo_imagen_url",
            opt(con_seo_imagen_url(&overrides.seo_imagen_objeto_id)),
        );
        insertar(
            &mut objeto,
            "origen_personalizacion",
            serde_json::json!({
                "plantilla": resuelta.origen_plantilla,
                "tema": resuelta.origen_tema,
            }),
        );
        Ok(Value::Object(objeto))
    }
}
