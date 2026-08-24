//! MVP seguro de `vtracker watch`: observa procesos locales, nunca el juego.

use std::{
    env, fmt, fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum GameState {
    #[default]
    Unknown,
    ClientClosed,
    Idle,
    GameOpen,
}
impl GameState {
    fn label(self) -> &'static str {
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

#[derive(Debug)]
struct Observation {
    state: GameState,
    client_found: bool,
    game_found: bool,
    source: &'static str,
}
#[derive(Debug)]
struct Transition {
    from: GameState,
    to: GameState,
    at: SystemTime,
}
#[derive(Default)]
struct Watcher {
    state: GameState,
    transitions: Vec<Transition>,
}
impl Watcher {
    fn observe(&mut self, observation: Observation) -> Option<Transition> {
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

#[derive(Debug)]
struct Config {
    interval: Duration,
    log_transitions: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3),
            log_transitions: false,
        }
    }
}
impl Config {
    fn parse(contents: &str) -> Result<Self, String> {
        let mut config = Self::default();
        for (number, line) in contents.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("línea {}: se esperaba clave = valor", number + 1))?;
            match key.trim() {
                "interval_seconds" => {
                    let seconds = value.trim().parse::<u64>().map_err(|_| {
                        format!("línea {}: interval_seconds debe ser un número", number + 1)
                    })?;
                    if !(1..=60).contains(&seconds) {
                        return Err(format!(
                            "línea {}: interval_seconds debe estar entre 1 y 60",
                            number + 1
                        ));
                    }
                    config.interval = Duration::from_secs(seconds);
                }
                "log_transitions" => {
                    config.log_transitions = value.trim().parse::<bool>().map_err(|_| {
                        format!(
                            "línea {}: log_transitions debe ser true o false",
                            number + 1
                        )
                    })?;
                }
                key => {
                    return Err(format!(
                        "línea {}: clave desconocida `{}`",
                        number + 1,
                        key.trim()
                    ));
                }
            }
        }
        Ok(config)
    }

    fn load() -> Result<Self, String> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
        Self::parse(&contents).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn effective() -> (Self, Option<String>) {
        match Self::load() {
            Ok(config) => (config, None),
            Err(error) => (Self::default(), Some(error)),
        }
    }
}
fn config_path() -> Option<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("vtracker").join("config.toml"))
}

fn process_list() -> Result<String, String> {
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

fn detect() -> Observation {
    // Para ensayar la UI/CI sin VALORANT: VTRACKER_STATE=closed|idle|game.
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

fn doctor() {
    println!("VTRACKER DOCTOR  ·  v{VERSION}\n────────────────────────────────────────");
    println!("Sistema         {}", env::consts::OS);
    println!("Detector        procesos locales (sin acceso a memoria)");

    match config_path() {
        Some(path) if path.exists() => match Config::load() {
            Ok(_) => println!("Configuración   válida: {}", path.display()),
            Err(error) => println!("Configuración   inválida: {error}"),
        },
        Some(path) => println!(
            "Configuración   no encontrada (opcional): {}",
            path.display()
        ),
        None => println!("Configuración   APPDATA no está disponible"),
    }
    let (config, _) = Config::effective();
    println!("Intervalo       {} s", config.interval.as_secs());
    println!(
        "Log transiciones {}",
        if config.log_transitions {
            "activo"
        } else {
            "desactivado"
        }
    );

    let process_query_ok = match process_list() {
        Ok(processes) => {
            let matches: Vec<_> = processes
                .lines()
                .filter(|line| {
                    let line = line.to_lowercase();
                    line.contains("valorant")
                        || line.contains("riotclient")
                        || line.contains("riot client")
                })
                .take(8)
                .collect();
            println!("Consulta        correcta");
            if matches.is_empty() {
                println!("Procesos Riot   no detectados");
            } else {
                println!("Procesos Riot   detectados:");
                for process in matches {
                    println!("  - {}", process.trim_matches('"'));
                }
            }
            true
        }
        Err(error) => {
            println!("Consulta        falló: {error}");
            false
        }
    };

    if env::var_os("VTRACKER_STATE").is_some() {
        println!("Simulación      activa mediante VTRACKER_STATE (el estado mostrado no es real)");
    }
    let observation = detect();
    println!("Estado actual   {}", observation.state);
    println!("────────────────────────────────────────");
    if process_query_ok {
        println!(
            "Resultado: el detector de procesos está listo. No puede distinguir lobby, selección o partida real; esa capacidad requiere una fuente autorizada adicional."
        );
    } else {
        println!(
            "Resultado: no fue posible consultar procesos. Ejecuta el comando en una consola normal y revisa sus permisos."
        );
    }
}

fn transition_log_path() -> Option<PathBuf> {
    config_path().map(|path| path.with_file_name("watch.log"))
}

fn log_transition(transition: &Transition) -> Result<(), String> {
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

fn timestamp(time: SystemTime) -> String {
    match time.elapsed() {
        Ok(age) => format!("hace {} s", age.as_secs()),
        Err(_) => "ahora".into(),
    }
}

fn clear_terminal() {
    // Algunas consolas de Windows no habilitan ANSI aunque sean interactivas.
    // `cls` es el mecanismo nativo y funciona también cuando se inicia desde PowerShell.
    if cfg!(windows) {
        let _ = Command::new("cmd").args(["/C", "cls"]).status();
    } else {
        print!("\x1B[2J\x1B[H");
    }
}

fn draw(watcher: &Watcher, observation: &Observation, started: Instant, interactive: bool) {
    if interactive {
        clear_terminal();
    }
    println!("VTRACKER WATCH  ·  MVP  ·  v{VERSION}\n────────────────────────────────────────");
    println!(
        "Estado          {}\nFuente          {}\nCliente         {}\nJuego           {}\nSesión          {} s\nTransiciones    {}",
        watcher.state,
        observation.source,
        if observation.client_found {
            "detectado"
        } else {
            "no detectado"
        },
        if observation.game_found {
            "en ejecución"
        } else {
            "no detectado"
        },
        started.elapsed().as_secs(),
        watcher.transitions.len()
    );
    if let Some(last) = watcher.transitions.last() {
        println!(
            "Último cambio   {} → {} ({})",
            last.from,
            last.to,
            timestamp(last.at)
        );
    }
    println!(
        "────────────────────────────────────────\nSolo se observan procesos del sistema; no se accede a memoria ni se automatiza el juego."
    );
    if interactive {
        println!("Actualización automática. Ctrl+C para salir.");
    }
    let _ = io::stdout().flush();
}
fn help() {
    println!(
        "vtracker {VERSION}\n\nUSO:\n  vtracker watch [--once] [--interval SEGUNDOS]\n  vtracker doctor\n\nVARIABLES:\n  VTRACKER_STATE=closed|idle|game  Simula un estado para pruebas."
    );
}
fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "watch".into());
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        help();
        return;
    }
    if command == "doctor" {
        if let Some(option) = args.next() {
            eprintln!("Opción desconocida: {option}");
            std::process::exit(2);
        }
        doctor();
        return;
    }
    if command != "watch" {
        eprintln!("Comando no disponible en el MVP: {command}\nUsa `vtracker watch`.");
        std::process::exit(2);
    }
    let mut once = false;
    let (mut config, config_warning) = Config::effective();
    if let Some(warning) = config_warning {
        eprintln!("Advertencia: configuración ignorada ({warning}). Se usan valores por defecto.");
    }
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--once" => once = true,
            "--interval" => match args.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(seconds) if (1..=60).contains(&seconds) => {
                    config.interval = Duration::from_secs(seconds)
                }
                _ => {
                    eprintln!("--interval debe estar entre 1 y 60 segundos.");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                help();
                return;
            }
            _ => {
                eprintln!("Opción desconocida: {arg}");
                std::process::exit(2);
            }
        }
    }
    let interactive = io::stdout().is_terminal();
    let started = Instant::now();
    let mut watcher = Watcher::default();
    loop {
        let observation = detect();
        let transition = watcher.observe(Observation {
            state: observation.state,
            client_found: observation.client_found,
            game_found: observation.game_found,
            source: observation.source,
        });
        if config.log_transitions
            && let Some(transition) = transition
            && let Err(error) = log_transition(&transition)
        {
            eprintln!("Advertencia: no se pudo guardar el log: {error}");
        }
        draw(&watcher, &observation, started, interactive);
        if once || !interactive {
            break;
        }
        thread::sleep(config.interval);
    }
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
        for i in 0..12 {
            watcher.observe(observation(if i % 2 == 0 {
                GameState::Idle
            } else {
                GameState::GameOpen
            }));
        }
        assert_eq!(watcher.transitions.len(), 8);
    }
    #[test]
    fn parses_valid_configuration() {
        let config = Config::parse("interval_seconds = 5\nlog_transitions = true").unwrap();
        assert_eq!(config.interval, Duration::from_secs(5));
        assert!(config.log_transitions);
    }
    #[test]
    fn rejects_invalid_configuration() {
        assert!(Config::parse("interval_seconds = 0").is_err());
        assert!(Config::parse("unknown = true").is_err());
    }
}
