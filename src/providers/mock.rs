use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::game::GameState;

use super::capabilities::{Confidence, GamePhase, GameStateSource, ProviderError, StateInfo};

/// Fuente simulada para pruebas y para validar la TUI sin depender de procesos o red.
/// Permite pre-programar una secuencia de fases y errores.
pub struct MockGameStateSource {
    name: &'static str,
    queue: Arc<Mutex<VecDeque<Result<GamePhase, ProviderError>>>>,
    default_confidence: Confidence,
}

impl MockGameStateSource {
    pub fn new(phases: Vec<GamePhase>) -> Self {
        let queue = phases.into_iter().map(Ok).collect();
        Self {
            name: "mock",
            queue: Arc::new(Mutex::new(queue)),
            default_confidence: Confidence::High,
        }
    }

    pub fn with_results(results: Vec<Result<GamePhase, ProviderError>>) -> Self {
        Self {
            name: "mock",
            queue: Arc::new(Mutex::new(results.into())),
            default_confidence: Confidence::High,
        }
    }

    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.default_confidence = confidence;
        self
    }

    /// Crea un mock que cicla por Idle -> PreGame -> AgentSelect -> InMatch -> PostMatch
    pub fn full_lifecycle() -> Self {
        Self::new(vec![
            GamePhase::Idle,
            GamePhase::Lobby,
            GamePhase::PreGame,
            GamePhase::AgentSelect,
            GamePhase::InMatch,
            GamePhase::PostMatch,
            GamePhase::Idle,
        ])
    }
}

impl GameStateSource for MockGameStateSource {
    fn name(&self) -> &'static str {
        self.name
    }

    fn fetch(&self) -> Result<StateInfo, ProviderError> {
        let mut guard = self.queue.lock().unwrap();
        let next = guard.pop_front().unwrap_or(Ok(GamePhase::Unknown));
        match next {
            Ok(phase) => {
                let coarse = match phase {
                    GamePhase::ClientClosed => GameState::ClientClosed,
                    GamePhase::Idle
                    | GamePhase::Lobby
                    | GamePhase::PreGame
                    | GamePhase::AgentSelect => GameState::Idle,
                    GamePhase::InMatch | GamePhase::PostMatch | GamePhase::GameOpen => {
                        GameState::GameOpen
                    }
                    GamePhase::Unknown => GameState::Unknown,
                };
                let (client_found, game_found) = match phase {
                    GamePhase::ClientClosed | GamePhase::Unknown => (false, false),
                    GamePhase::Idle
                    | GamePhase::Lobby
                    | GamePhase::PreGame
                    | GamePhase::AgentSelect => (true, false),
                    GamePhase::InMatch | GamePhase::PostMatch | GamePhase::GameOpen => (true, true),
                };
                Ok(StateInfo {
                    phase,
                    coarse,
                    confidence: self.default_confidence,
                    source: self.name,
                    client_found,
                    game_found,
                    context_revision: None,
                })
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_programmed_phases_in_order() {
        let mock = MockGameStateSource::new(vec![
            GamePhase::Idle,
            GamePhase::InMatch,
            GamePhase::PostMatch,
        ]);
        assert_eq!(mock.fetch().unwrap().phase, GamePhase::Idle);
        assert_eq!(mock.fetch().unwrap().phase, GamePhase::InMatch);
        assert_eq!(mock.fetch().unwrap().phase, GamePhase::PostMatch);
        // Agotada la cola, retorna Unknown
        assert_eq!(mock.fetch().unwrap().phase, GamePhase::Unknown);
    }

    #[test]
    fn mock_can_simulate_errors_and_fallback() {
        let mock = MockGameStateSource::with_results(vec![
            Ok(GamePhase::Idle),
            Err(ProviderError::Network("timeout".into())),
            Ok(GamePhase::InMatch),
            Err(ProviderError::RateLimited("too many".into())),
        ]);
        assert!(mock.fetch().is_ok());
        assert!(mock.fetch().unwrap_err().is_retryable());
        assert_eq!(mock.fetch().unwrap().phase, GamePhase::InMatch);
        assert!(mock.fetch().unwrap_err().is_retryable());
    }

    #[test]
    fn mock_full_lifecycle_covers_experience_flow() {
        let mock = MockGameStateSource::full_lifecycle();
        let phases: Vec<_> = (0..7).map(|_| mock.fetch().unwrap().phase).collect();
        assert_eq!(phases[0], GamePhase::Idle); // perfil propio
        assert_eq!(phases[3], GamePhase::AgentSelect); // equipo
        assert_eq!(phases[4], GamePhase::InMatch); // partida
    }

    #[test]
    fn mock_confidence_and_source() {
        let mock = MockGameStateSource::new(vec![GamePhase::Lobby])
            .with_name("mock-ui")
            .with_confidence(Confidence::Medium);
        let info = mock.fetch().unwrap();
        assert_eq!(info.confidence, Confidence::Medium);
        assert_eq!(info.source, "mock-ui");
    }
}
