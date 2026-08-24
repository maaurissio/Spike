use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config::config_path,
    game::{GameState, Observation},
};

#[derive(Debug)]
pub struct Transition {
    pub from: GameState,
    pub to: GameState,
    pub at: SystemTime,
}
#[derive(Default)]
pub struct Watcher {
    pub state: GameState,
    pub transitions: Vec<Transition>,
}
impl Watcher {
    pub fn observe(&mut self, observation: Observation) -> Option<Transition> {
        if observation.state == self.state {
            return None;
        }
        let transition = Transition {
            from: self.state,
            to: observation.state,
            at: SystemTime::now(),
        };
        self.state = observation.state;
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
    fn observation(state: GameState) -> Observation {
        Observation {
            state,
            client_found: false,
            game_found: false,
            source: "test",
        }
    }
    #[test]
    fn only_records_actual_state_changes() {
        let mut watcher = Watcher::default();
        assert!(
            watcher
                .observe(observation(GameState::ClientClosed))
                .is_some()
        );
        assert!(
            watcher
                .observe(observation(GameState::ClientClosed))
                .is_none()
        );
        assert!(watcher.observe(observation(GameState::GameOpen)).is_some());
        assert_eq!(watcher.transitions.len(), 2);
    }
    #[test]
    fn retains_only_recent_transitions() {
        let mut watcher = Watcher::default();
        for index in 0..12 {
            watcher.observe(observation(if index % 2 == 0 {
                GameState::Idle
            } else {
                GameState::GameOpen
            }));
        }
        assert_eq!(watcher.transitions.len(), 8);
    }
}
