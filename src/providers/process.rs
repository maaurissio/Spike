use super::capabilities::{Confidence, GamePhase, GameStateSource, ProviderError, StateInfo};
use crate::game;

/// Adaptador que usa la detección local por procesos (`src/game/mod.rs`).
/// No puede distinguir fases finas; retorna `GamePhase::GameOpen` cuando detecta el ejecutable.
pub struct ProcessGameStateSource;

impl ProcessGameStateSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessGameStateSource {
    fn default() -> Self {
        Self::new()
    }
}

impl GameStateSource for ProcessGameStateSource {
    fn name(&self) -> &'static str {
        "process-local"
    }

    fn fetch(&self) -> Result<StateInfo, ProviderError> {
        let obs = game::detect();
        let phase = GamePhase::from_coarse(obs.state);
        let confidence = match obs.state {
            game::GameState::Unknown => Confidence::Unknown,
            _ => Confidence::Low,
        };
        Ok(StateInfo::new(
            phase,
            obs.state,
            confidence,
            obs.source,
            obs.client_found,
            obs.game_found,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    #[test]
    fn process_source_name() {
        assert_eq!(ProcessGameStateSource::new().name(), "process-local");
    }

    #[test]
    fn process_source_maps_game_state_to_phase() {
        // Usa VTRACKER_STATE para simular sin depender de procesos reales.
        let _guard = crate::test_support::env_lock();
        let original = std::env::var_os("VTRACKER_STATE");

        unsafe { std::env::set_var("VTRACKER_STATE", "idle") };
        let info = ProcessGameStateSource::new().fetch().unwrap();
        assert_eq!(info.phase, GamePhase::Idle);
        assert_eq!(info.coarse, GameState::Idle);
        assert_eq!(info.confidence, Confidence::Low);
        assert!(info.client_found);

        unsafe { std::env::set_var("VTRACKER_STATE", "game") };
        let info = ProcessGameStateSource::new().fetch().unwrap();
        assert_eq!(info.phase, GamePhase::GameOpen);
        assert_eq!(info.confidence, Confidence::Low);
        assert!(info.game_found);

        unsafe { std::env::set_var("VTRACKER_STATE", "closed") };
        let info = ProcessGameStateSource::new().fetch().unwrap();
        assert_eq!(info.phase, GamePhase::ClientClosed);

        if let Some(v) = original {
            unsafe { std::env::set_var("VTRACKER_STATE", v) };
        } else {
            unsafe { std::env::remove_var("VTRACKER_STATE") };
        }
    }

    #[test]
    fn process_source_never_returns_fine_grained_without_provider() {
        let _guard = crate::test_support::env_lock();
        let original = std::env::var_os("VTRACKER_STATE");
        for val in ["closed", "idle", "game", "unknown_value"] {
            unsafe { std::env::set_var("VTRACKER_STATE", val) };
            let info = ProcessGameStateSource::new().fetch().unwrap();
            assert!(
                !info.phase.is_fine_grained(),
                "phase {:?} no debe ser fina",
                info.phase
            );
        }
        if let Some(v) = original {
            unsafe { std::env::set_var("VTRACKER_STATE", v) };
        } else {
            unsafe { std::env::remove_var("VTRACKER_STATE") };
        }
    }
}
