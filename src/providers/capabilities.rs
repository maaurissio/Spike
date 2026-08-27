use std::fmt;

use crate::game::GameState;

/// Fase enriquecida del cliente/juego.
/// Solo un `GameStateSource` autorizado debe retornar `Lobby`/`PreGame`/`AgentSelect`/`InMatch`/`PostMatch`.
/// `GameOpen` significa "juego detectado pero fase no confirmada" (compatibilidad honesta con detección por procesos).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GamePhase {
    #[default]
    Unknown,
    ClientClosed,
    Idle,
    Lobby,
    PreGame,
    AgentSelect,
    InMatch,
    PostMatch,
    GameOpen,
}

impl GamePhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Desconocido",
            Self::ClientClosed => "Cliente cerrado",
            Self::Idle => "Cliente disponible",
            Self::Lobby => "Lobby",
            Self::PreGame => "Pre-partida",
            Self::AgentSelect => "Selección de agente",
            Self::InMatch => "En partida",
            Self::PostMatch => "Post-partida",
            Self::GameOpen => "Juego abierto (fase no confirmada)",
        }
    }

    pub fn is_fine_grained(self) -> bool {
        matches!(
            self,
            Self::Lobby | Self::PreGame | Self::AgentSelect | Self::InMatch | Self::PostMatch
        )
    }

    pub fn from_coarse(state: GameState) -> Self {
        match state {
            GameState::Unknown => Self::Unknown,
            GameState::ClientClosed => Self::ClientClosed,
            GameState::Idle => Self::Idle,
            GameState::GameOpen => Self::GameOpen,
        }
    }
}

pub const FINE_GRAINED_PHASES: &[GamePhase] = &[
    GamePhase::Lobby,
    GamePhase::PreGame,
    GamePhase::AgentSelect,
    GamePhase::InMatch,
    GamePhase::PostMatch,
];

impl fmt::Display for GamePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

pub const CONFIDENCE_LEVELS: &[Confidence] = &[
    Confidence::High,
    Confidence::Medium,
    Confidence::Low,
    Confidence::Unknown,
];

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::High => "alta",
            Self::Medium => "media",
            Self::Low => "baja",
            Self::Unknown => "desconocida",
        };
        f.write_str(s)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    NotConfigured(String),
    Unavailable(String),
    EndpointUnavailable { endpoint: String, status: u16 },
    Unauthorized(String),
    RateLimited(String),
    Network(String),
    Parse(String),
    Timeout,
    Unknown(String),
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::RateLimited(_) | Self::Timeout | Self::Unavailable(_)
        )
    }

    #[cfg(test)]
    pub fn is_auth_failure(&self) -> bool {
        matches!(self, Self::Unauthorized(_))
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured(msg) => write!(f, "no configurado: {msg}"),
            Self::Unavailable(msg) => write!(f, "no disponible: {msg}"),
            Self::EndpointUnavailable { endpoint, status } => {
                write!(
                    f,
                    "endpoint local no disponible: {endpoint} (HTTP {status})"
                )
            }
            Self::Unauthorized(msg) => write!(f, "no autorizado: {msg}"),
            Self::RateLimited(msg) => write!(f, "rate limited: {msg}"),
            Self::Network(msg) => write!(f, "error de red: {msg}"),
            Self::Parse(msg) => write!(f, "error de formato: {msg}"),
            Self::Timeout => write!(f, "timeout"),
            Self::Unknown(msg) => write!(f, "desconocido: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}

#[derive(Clone, Debug)]
pub struct StateInfo {
    pub phase: GamePhase,
    pub coarse: GameState,
    pub confidence: Confidence,
    pub source: &'static str,
    pub client_found: bool,
    pub game_found: bool,
}

impl StateInfo {
    pub fn new(
        phase: GamePhase,
        coarse: GameState,
        confidence: Confidence,
        source: &'static str,
        client_found: bool,
        game_found: bool,
    ) -> Self {
        Self {
            phase,
            coarse,
            confidence,
            source,
            client_found,
            game_found,
        }
    }

    pub fn unknown(source: &'static str) -> Self {
        Self::new(
            GamePhase::Unknown,
            GameState::Unknown,
            Confidence::Unknown,
            source,
            false,
            false,
        )
    }
}

/// Capability principal para obtener el estado del cliente/juego.
/// El resto de la app consulta esta trait, no implementaciones concretas.
pub trait GameStateSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn fetch(&self) -> Result<StateInfo, ProviderError>;
    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    #[test]
    fn phase_labels_are_correct() {
        assert_eq!(GamePhase::Lobby.label(), "Lobby");
        assert_eq!(
            GamePhase::GameOpen.label(),
            "Juego abierto (fase no confirmada)"
        );
    }

    #[test]
    fn phase_fine_grained_detection() {
        assert!(GamePhase::InMatch.is_fine_grained());
        assert!(!GamePhase::Idle.is_fine_grained());
        assert!(!GamePhase::GameOpen.is_fine_grained());
        assert!(!GamePhase::Unknown.is_fine_grained());
    }

    #[test]
    fn phase_from_coarse_maps_correctly() {
        assert_eq!(
            GamePhase::from_coarse(GameState::ClientClosed),
            GamePhase::ClientClosed
        );
        assert_eq!(GamePhase::from_coarse(GameState::Idle), GamePhase::Idle);
        assert_eq!(
            GamePhase::from_coarse(GameState::GameOpen),
            GamePhase::GameOpen
        );
        assert_eq!(
            GamePhase::from_coarse(GameState::Unknown),
            GamePhase::Unknown
        );
    }

    #[test]
    fn provider_error_retryable() {
        assert!(ProviderError::Network("x".into()).is_retryable());
        assert!(ProviderError::RateLimited("x".into()).is_retryable());
        assert!(!ProviderError::Unauthorized("x".into()).is_retryable());
        assert!(
            !ProviderError::EndpointUnavailable {
                endpoint: "/session".into(),
                status: 404,
            }
            .is_retryable()
        );
        assert!(!ProviderError::Parse("x".into()).is_retryable());
    }

    #[test]
    fn confidence_display() {
        assert_eq!(Confidence::High.to_string(), "alta");
        assert_eq!(Confidence::Low.to_string(), "baja");
    }
}
