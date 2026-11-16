use super::*;

#[async_trait::async_trait]
pub trait AgendaPort: Send + Sync {
    async fn crear_recurso(&self, empresa_id: &str, dto: &CrearRecurso) -> Result<Value, ApiError>;
    async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError>;
    async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError>;
    async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError>;
    async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError>;
    async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError>;
    async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError>;
    async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError>;
    async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError>;
    async fn crear_desde_reserva(&self, orden: &OrdenReserva) -> Result<Value, ApiError>;
    async fn procesar_recordatorios(&self) -> Result<usize, ApiError>;
    async fn verificar_disponibilidad(&self, dto: &VerificarDisponibilidad)
        -> Result<(), ApiError>;
}

#[async_trait::async_trait]
impl AgendaPort for Agenda {
    async fn crear_recurso(&self, empresa_id: &str, dto: &CrearRecurso) -> Result<Value, ApiError> {
        Agenda::crear_recurso(self, empresa_id, dto).await
    }

    async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_recursos(self, empresa_id).await
    }

    async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        Agenda::crear_disponibilidad(self, empresa_id, dto).await
    }

    async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_disponibilidad(self, empresa_id, recurso_id).await
    }

    async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        Agenda::crear_bloqueo(self, empresa_id, actor_id, dto).await
    }

    async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError> {
        Agenda::crear_cita(self, empresa_id, dto).await
    }

    async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_citas(self, empresa_id, desde, hasta).await
    }

    async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError> {
        Agenda::cancelar_cita(self, empresa_id, cita_id, motivo).await
    }

    async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_recordatorios(self, empresa_id).await
    }

    async fn crear_desde_reserva(&self, orden: &OrdenReserva) -> Result<Value, ApiError> {
        Agenda::crear_desde_reserva(self, orden).await
    }

    async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
        Agenda::procesar_recordatorios(self).await
    }

    async fn verificar_disponibilidad(
        &self,
        dto: &VerificarDisponibilidad,
    ) -> Result<(), ApiError> {
        Agenda::verificar_disponibilidad(self, dto).await
    }
}

/// Implementación para tests/arranque sin base de datos.
pub struct AgendaNoDisponible;

#[async_trait::async_trait]
impl AgendaPort for AgendaNoDisponible {
    async fn crear_recurso(&self, _: &str, _: &CrearRecurso) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_recursos(&self, _: &str) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_disponibilidad(
        &self,
        _: &str,
        _: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_disponibilidad(&self, _: &str, _: &str) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_bloqueo(
        &self,
        _: &str,
        _: Option<&str>,
        _: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_cita(&self, _: &str, _: &CrearCita) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_citas(
        &self,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn cancelar_cita(&self, _: &str, _: &str, _: Option<&str>) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_recordatorios(&self, _: &str) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_desde_reserva(&self, _: &OrdenReserva) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn verificar_disponibilidad(&self, _: &VerificarDisponibilidad) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
}
