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
