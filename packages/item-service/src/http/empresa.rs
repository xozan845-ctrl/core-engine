//! Selección de la empresa activa para la superficie de catálogo.

use crate::auth::UsuarioContexto;
use crate::errors::ApiError;

/// Empresario o rol configurable: atado a su tenant. Admin global: `empresa_id`
/// explícito.
pub fn de_tenant(
    usuario: &UsuarioContexto,
    solicitada: Option<&str>,
    mensaje_otra: &str,
) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio("EMPRESA_NO_AUTORIZADA", mensaje_otra));
            }
        }
        return Ok(tenant.to_string());
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

/// Variante de operaciones: solo `empresario` se ata al tenant; el resto usa
/// `empresa_id` o, si falta, el tenant del contexto.
pub fn de_operaciones(
    usuario: &UsuarioContexto,
    solicitada: Option<&str>,
) -> Result<String, ApiError> {
    if usuario.rol == "empresario" {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::no_encontrado("Empresa", "activa"))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio(
                    "EMPRESA_NO_AUTORIZADA",
                    "El empresario solo puede operar el catálogo de su empresa.",
                ));
            }
        }
        return Ok(tenant.to_string());
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .or_else(|| usuario.tenant_id.clone())
        .ok_or_else(|| {
            ApiError::dominio(
                "EMPRESA_REQUERIDA",
                "Indica empresa_id para operar el catálogo.",
            )
        })
}
