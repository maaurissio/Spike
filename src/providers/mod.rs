//! Capa de proveedores — desacopla `GameStateSource` del resto de la app.
//! Arquitectura: `GameStateSource` es la capability que el resto consume.
//! Fase 2B: interfaces + adaptadores local/mock sin red. Fase 2C: adaptador Riot autorizado.

pub mod capabilities;
pub mod local;
pub mod lockfile;
pub mod match_detail;
#[cfg(test)]
pub mod mock;
pub mod process;

pub use capabilities::ProviderError;
pub use capabilities::{GameStateSource, StateInfo};
pub use local::LocalClientSource;
pub(crate) use match_detail::MatchDetailSource;
#[cfg(test)]
pub use mock::MockGameStateSource;
pub use process::ProcessGameStateSource;

/// Resuelve el estado intentando el proveedor primario y usando fallback si es recuperable.
/// Mantiene último estado conocido en caso de fallo — útil para mostrar en TUI sin pantalla vacía.
pub fn resolve_with_fallback(
    primary: &dyn GameStateSource,
    fallback: &dyn GameStateSource,
) -> Result<StateInfo, ProviderError> {
    match primary.fetch() {
        Ok(info) => Ok(info),
        Err(e) if e.is_retryable() => fallback.fetch(),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::capabilities::GamePhase;

    #[test]
    fn fallback_on_retryable_error() {
        let primary =
            MockGameStateSource::with_results(vec![Err(ProviderError::Network("down".into()))]);
        let fallback = MockGameStateSource::new(vec![GamePhase::Idle]);
        let info = resolve_with_fallback(&primary, &fallback).unwrap();
        assert_eq!(info.phase, GamePhase::Idle);
        assert_eq!(info.source, "mock");
    }

    #[test]
    fn no_fallback_on_auth_error() {
        let primary = MockGameStateSource::with_results(vec![Err(ProviderError::Unauthorized(
            "bad key".into(),
        ))]);
        let fallback = MockGameStateSource::new(vec![GamePhase::Idle]);
        let err = resolve_with_fallback(&primary, &fallback).unwrap_err();
        assert!(err.is_auth_failure());
    }

    #[test]
    fn primary_success_no_fallback() {
        let primary = MockGameStateSource::new(vec![GamePhase::InMatch]);
        let fallback = MockGameStateSource::new(vec![GamePhase::Idle]);
        let info = resolve_with_fallback(&primary, &fallback).unwrap();
        assert_eq!(info.phase, GamePhase::InMatch);
    }
}
