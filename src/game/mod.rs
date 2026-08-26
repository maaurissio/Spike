use std::{env, fmt, process::Command};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameState {
    #[default]
    Unknown,
    ClientClosed,
    Idle,
    GameOpen,
}
impl GameState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Comprobando",
            Self::ClientClosed => "Cliente cerrado",
            Self::Idle => "Cliente disponible",
            Self::GameOpen => "Cliente y juego abiertos (modo no confirmado)",
        }
    }
}
impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
pub struct Observation {
    pub state: GameState,
    pub client_found: bool,
    pub game_found: bool,
    pub source: &'static str,
}

pub fn process_list() -> Result<String, String> {
    let output = if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
    } else {
        Command::new("ps").args(["-A", "-o", "comm="]).output()
    }
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn observation_from_process_list(list: &str) -> Observation {
    let list = list.to_lowercase();
    let game_found = list.contains("valorant-win64-shipping");
    let client_found =
        list.contains("riotclientservices") || list.contains("valorant") || game_found;
    let state = if game_found {
        GameState::GameOpen
    } else if client_found {
        GameState::Idle
    } else {
        GameState::ClientClosed
    };
    Observation {
        state,
        client_found,
        game_found,
        source: "procesos locales",
    }
}

pub fn detect() -> Observation {
    if let Ok(value) = env::var("VTRACKER_STATE") {
        let state = match value.to_lowercase().as_str() {
            "closed" => GameState::ClientClosed,
            "idle" => GameState::Idle,
            "game" => GameState::GameOpen,
            _ => GameState::Unknown,
        };
        return Observation {
            state,
            client_found: matches!(state, GameState::Idle | GameState::GameOpen),
            game_found: state == GameState::GameOpen,
            source: "variable de entorno",
        };
    }
    let Ok(list) = process_list() else {
        return Observation {
            state: GameState::Unknown,
            client_found: false,
            game_found: false,
            source: "procesos no disponibles",
        };
    };
    observation_from_process_list(&list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_state_labels_are_correct() {
        assert_eq!(GameState::Unknown.label(), "Comprobando");
        assert_eq!(GameState::ClientClosed.label(), "Cliente cerrado");
        assert_eq!(GameState::Idle.label(), "Cliente disponible");
        assert_eq!(
            GameState::GameOpen.label(),
            "Cliente y juego abiertos (modo no confirmado)"
        );
    }

    #[test]
    fn observation_from_list_detects_client_closed() {
        let obs = observation_from_process_list("explorer.exe\nchrome.exe");
        assert_eq!(obs.state, GameState::ClientClosed);
        assert!(!obs.client_found);
        assert!(!obs.game_found);
    }

    #[test]
    fn observation_from_list_detects_idle_via_riot_client() {
        let obs = observation_from_process_list("\"RiotClientServices.exe\",\"123\"");
        assert_eq!(obs.state, GameState::Idle);
        assert!(obs.client_found);
        assert!(!obs.game_found);
    }

    #[test]
    fn observation_from_list_detects_idle_via_valorant_string() {
        let obs = observation_from_process_list("VALORANT.exe");
        assert_eq!(obs.state, GameState::Idle);
        assert!(obs.client_found);
    }

    #[test]
    fn observation_from_list_detects_game_open() {
        let obs = observation_from_process_list("VALORANT-Win64-Shipping.exe");
        assert_eq!(obs.state, GameState::GameOpen);
        assert!(obs.client_found);
        assert!(obs.game_found);
    }

    #[test]
    fn observation_from_list_is_case_insensitive() {
        let obs = observation_from_process_list("riotclientservices");
        assert_eq!(obs.state, GameState::Idle);
        let obs = observation_from_process_list("VaLoRaNt-WiN64-ShIpPiNg");
        assert_eq!(obs.state, GameState::GameOpen);
    }

    #[test]
    fn detect_respects_vtracker_state_env() {
        let _guard = crate::test_support::env_lock();
        let original = std::env::var_os("VTRACKER_STATE");
        unsafe { std::env::set_var("VTRACKER_STATE", "closed") };
        assert_eq!(detect().state, GameState::ClientClosed);
        unsafe { std::env::set_var("VTRACKER_STATE", "idle") };
        assert_eq!(detect().state, GameState::Idle);
        unsafe { std::env::set_var("VTRACKER_STATE", "game") };
        assert_eq!(detect().state, GameState::GameOpen);
        unsafe { std::env::set_var("VTRACKER_STATE", "unknown_value") };
        assert_eq!(detect().state, GameState::Unknown);
        if let Some(val) = original {
            unsafe { std::env::set_var("VTRACKER_STATE", val) };
        } else {
            unsafe { std::env::remove_var("VTRACKER_STATE") };
        }
    }
}
