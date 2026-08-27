use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config::config_path,
    game::GameState,
    providers::{StateInfo, capabilities::GamePhase},
};

#[derive(Debug)]
pub struct Transition {
    pub from: GamePhase,
    pub to: GamePhase,
    pub at: SystemTime,
}
#[derive(Default)]
pub struct Watcher {
    pub state: GameState,
    pub phase: GamePhase,
    pub transitions: Vec<Transition>,
}
impl Watcher {
    /// Registra cambios del estado grueso o de una fase local confirmada.
    /// Así `AgentSelect → PostMatch` no se pierde por pertenecer ambos al
    /// cliente abierto.
    pub fn observe(&mut self, info: &StateInfo) -> Option<Transition> {
        if info.coarse == self.state && info.phase == self.phase {
            return None;
        }
        let transition = Transition {
            from: self.phase,
            to: info.phase,
            at: SystemTime::now(),
        };
        self.state = info.coarse;
        self.phase = info.phase;
        self.transitions.push(Transition {
            from: transition.from,
            to: transition.to,
            at: transition.at,
        });
        if self.transitions.len() > 8 {
            self.transitions.remove(0);
        }
        Some(transition)
    }
}
fn transition_log_path() -> Option<PathBuf> {
    config_path().map(|path| path.with_file_name("watch.log"))
}
pub fn log_transition(transition: &Transition) -> Result<(), String> {
    let Some(path) = transition_log_path() else {
        return Err("APPDATA no está disponible".into());
    };
    let parent = path
        .parent()
        .ok_or_else(|| "ruta de log inválida".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let seconds = transition
        .at
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| error.to_string())?;
    writeln!(file, "{seconds} {} -> {}", transition.from, transition.to)
        .map_err(|error| error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::capabilities::Confidence;

    fn info(state: GameState, phase: GamePhase) -> StateInfo {
        StateInfo::new(phase, state, Confidence::High, "test", false, false)
    }

    #[test]
    fn only_records_actual_state_changes() {
        let mut watcher = Watcher::default();
        assert!(
            watcher
                .observe(&info(GameState::ClientClosed, GamePhase::ClientClosed))
                .is_some()
        );
        assert!(
            watcher
                .observe(&info(GameState::ClientClosed, GamePhase::ClientClosed))
                .is_none()
        );
        assert!(
            watcher
                .observe(&info(GameState::GameOpen, GamePhase::GameOpen))
                .is_some()
        );
        assert_eq!(watcher.transitions.len(), 2);
    }

    #[test]
    fn records_fine_grained_transition_without_coarse_change() {
        let mut watcher = Watcher::default();
        watcher.observe(&info(GameState::Idle, GamePhase::AgentSelect));

        let transition = watcher
            .observe(&info(GameState::Idle, GamePhase::PostMatch))
            .unwrap();

        assert_eq!(transition.from, GamePhase::AgentSelect);
        assert_eq!(transition.to, GamePhase::PostMatch);
        assert_eq!(watcher.transitions.len(), 2);
    }

    #[test]
    fn retains_only_recent_transitions() {
        let mut watcher = Watcher::default();
        for index in 0..12 {
            let (state, phase) = if index % 2 == 0 {
                (GameState::Idle, GamePhase::Idle)
            } else {
                (GameState::GameOpen, GamePhase::GameOpen)
            };
            watcher.observe(&info(state, phase));
        }
        assert_eq!(watcher.transitions.len(), 8);
    }
}
