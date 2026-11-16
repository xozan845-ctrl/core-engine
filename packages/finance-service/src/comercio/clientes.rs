use super::helpers::*;
use super::*;

impl Comercio {
    pub async fn crear_cliente(
        &self,
        dto: ClienteComercio,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.name, "name", 1)?;
        if let Some(email) = &dto.email {
            validar_email(email, "email")?;
        }
        self.crear("clientes", json!(dto), usuario).await
    }

    pub async fn importar_clientes(
        &self,
        dto: ImportarClientes,
        usuario: &UsuarioContexto,
    ) -> Result<ImportacionClientes, ApiError> {
        if dto.clientes.len() > 500 {
            return Err(ApiError::dominio(
                "IMPORTACION_DEMASIADO_GRANDE",
                "El máximo es 500 clientes por importación.",
            ));
        }
        let existentes = self.listar("clientes", usuario, None).await?;
        let mut claves: HashSet<String> = existentes.iter().map(clave_cliente).collect();
        let mut imported = 0i64;
        let mut skipped = 0i64;
        let mut errors = Vec::new();
        for (indice, fila) in dto.clientes.into_iter().enumerate() {
            let nombre = fila.name.trim().to_string();
            if nombre.is_empty() {
                skipped += 1;
                errors.push(format!("Fila {}: nombre vacío.", indice + 1));
                continue;
            }
            let fila_clave = json!({"name": fila.name, "phone": fila.phone.clone()});
            let clave = clave_cliente(fila_clave.as_object().unwrap_or(&Map::new()));
            if claves.contains(&clave) {
                skipped += 1;
                continue;
            }
            let mut datos = json!(fila);
            datos["name"] = json!(nombre);
            datos["source"] = json!("import");
            match self.crear("clientes", datos, usuario).await {
                Ok(_) => {
                    claves.insert(clave);
                    imported += 1;
                }
                Err(error) => errors.push(format!("Fila {}: {}", indice + 1, error.mensaje)),
            }
        }
        Ok(ImportacionClientes {
            imported,
            skipped,
            errors,
        })
    }

    pub async fn crear_codigo_barras(
        &self,
        dto: CodigoBarras,
        usuario: &UsuarioContexto,
    ) -> Result<Registro, ApiError> {
        validar_texto(&dto.product_name, "productName", 1)?;
        validar_texto(&dto.sku, "sku", 1)?;
        validar_in(&dto.symbology, "symbology", &SIMBOLOGIAS)?;
        validar_rango(dto.copies as f64, "copies", 1.0, 100.0)?;
        self.crear("codigos-barras", json!(dto), usuario).await
    }

    // ── helpers ────────────────────────────────────────────────────────────

    pub fn ambito(
        recurso: &str,
        usuario: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Ambito, ApiError> {
        if !ROLES_COMERCIO.contains(&usuario.rol.as_str()) {
            return Err(ApiError::dominio(
                "ACCESO_DENEGADO",
                "El rol no tiene acceso a la operación de comercio.",
            ));
        }
        if empresa_id.is_none()
            && usuario.rol == "vendedor"
            && RECURSOS_SOLO_EMPRESA.contains(&recurso)
        {
            return Err(ApiError::dominio(
                "ACCESO_DENEGADO",
                "Este recurso pertenece al workspace de empresa.",
            ));
        }
        if usuario.rol == "vendedor" {
            return Ok(Ambito {
                tenant_id: None,
                propietario_id: Some(usuario.user_id.clone()),
            });
        }
        if usuario.rol == "empresario" {
            let tenant = usuario.tenant_id.clone().ok_or_else(|| {
                ApiError::dominio(
                    "TENANT_REQUERIDO",
                    "El empresario no tiene una empresa activa.",
                )
            })?;
            return Ok(Ambito {
                tenant_id: Some(tenant),
                propietario_id: None,
            });
        }
        Ok(Ambito {
            tenant_id: empresa_id
                .map(str::to_string)
                .or_else(|| usuario.tenant_id.clone()),
            propietario_id: None,
        })
    }
}
