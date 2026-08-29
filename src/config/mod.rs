use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub interval: Duration,
    pub log_transitions: bool,
    pub theme: Theme,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
    Mono,
}

impl Theme {
    pub fn previous(self) -> Self {
        match self {
            Self::System => Self::Mono,
            Self::Dark => Self::System,
            Self::Light => Self::Dark,
            Self::Mono => Self::Light,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::Mono,
            Self::Mono => Self::System,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Dark => "Noche",
            Self::Light => "Claro",
            Self::Mono => "Sin color",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Mono => "mono",
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3),
            log_transitions: false,
            theme: Theme::System,
        }
    }
}

impl Config {
    pub fn parse(contents: &str) -> Result<Self, String> {
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
                "theme" => {
                    config.theme = match value.trim() {
                        "\"system\"" => Theme::System,
                        "\"dark\"" => Theme::Dark,
                        "\"light\"" => Theme::Light,
                        "\"mono\"" => Theme::Mono,
                        _ => return Err(format!("línea {}: tema inválido", number + 1)),
                    };
                }
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
                    })?
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

    pub fn load() -> Result<Self, String> {
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

    pub fn effective() -> (Self, Option<String>) {
        match Self::load() {
            Ok(config) => (config, None),
            Err(error) => (Self::default(), Some(error)),
        }
    }

    fn encode(&self) -> String {
        format!(
            "# Generado por vtracker. No guardes secretos aquí.\ninterval_seconds = {}\nlog_transitions = {}\ntheme = \"{}\"\n",
            self.interval.as_secs(),
            self.log_transitions,
            self.theme.key(),
        )
    }
}

pub fn show() -> Result<String, String> {
    let path = config_path().ok_or_else(|| "APPDATA no está disponible".to_string())?;
    let configured = path.exists();
    let config = Config::load()?;
    Ok(format_config(&config, &path, configured))
}

pub fn validate() -> Result<String, String> {
    let path = config_path().ok_or_else(|| "APPDATA no está disponible".to_string())?;
    if !path.exists() {
        return Ok(format!(
            "Configuración no encontrada: {}\nLos valores por defecto son válidos.",
            path.display()
        ));
    }
    Config::load()?;
    Ok(format!("Configuración válida: {}", path.display()))
}

pub fn edit(interval_secs: Option<u64>, log_transitions: Option<bool>) -> Result<String, String> {
    let path = config_path().ok_or_else(|| "APPDATA no está disponible".to_string())?;
    let mut config = Config::load()?;
    if let Some(seconds) = interval_secs {
        if !(1..=60).contains(&seconds) {
            return Err("interval_seconds debe estar entre 1 y 60".into());
        }
        config.interval = Duration::from_secs(seconds);
    }
    if let Some(enabled) = log_transitions {
        config.log_transitions = enabled;
    }
    save_atomic(&path, &config)?;
    Ok(format!("Configuración guardada: {}", path.display()))
}

fn save_atomic(path: &Path, config: &Config) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "ruta de configuración inválida".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, config.encode()).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

/// Guardado explícito del borrador completo de la TUI, fuera del render.
pub fn save(config: &Config) -> Result<(), String> {
    Config::parse(&config.encode())?;
    let path = config_path().ok_or_else(|| "APPDATA no está disponible".to_string())?;
    save_atomic(&path, config)
}

fn format_config(config: &Config, path: &std::path::Path, configured: bool) -> String {
    format!(
        "Configuración {}\nOrigen          {}\nIntervalo       {} s\nLog transiciones {}\nTema            {}\nSecretos        no se muestran aquí",
        if configured {
            "efectiva"
        } else {
            "por defecto"
        },
        path.display(),
        config.interval.as_secs(),
        if config.log_transitions {
            "activo"
        } else {
            "desactivado"
        },
        config.theme.label(),
    )
}

pub fn config_path() -> Option<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("vtracker").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_valid_configuration() {
        let config = Config::parse("interval_seconds = 5\nlog_transitions = true").unwrap();
        assert_eq!(config.interval, Duration::from_secs(5));
        assert!(config.log_transitions);
    }
    #[test]
    fn themes_roundtrip_and_legacy_configs_use_terminal_defaults() {
        assert_eq!(
            Config::parse("interval_seconds = 4").unwrap().theme,
            Theme::System
        );
        for theme in [Theme::System, Theme::Dark, Theme::Light, Theme::Mono] {
            let config = Config {
                theme,
                ..Config::default()
            };
            assert_eq!(Config::parse(&config.encode()).unwrap(), config);
        }
        assert!(Config::parse("theme = dark").is_err());
        assert!(Config::parse("theme = \"invalid\"").is_err());
    }
    #[test]
    fn rejects_invalid_configuration() {
        assert!(Config::parse("interval_seconds = 0").is_err());
        assert!(Config::parse("unknown = true").is_err());
    }
    #[test]
    fn ignores_comments_and_blank_lines() {
        let config = Config::parse(
            "# comentario\n\ninterval_seconds = 10 # inline\n\nlog_transitions = false\n",
        )
        .unwrap();
        assert_eq!(config.interval, Duration::from_secs(10));
        assert!(!config.log_transitions);
    }
    #[test]
    fn handles_whitespace_around_keys_and_values() {
        let config =
            Config::parse("  interval_seconds  =  7  \n  log_transitions  =  true  ").unwrap();
        assert_eq!(config.interval, Duration::from_secs(7));
        assert!(config.log_transitions);
    }
    #[test]
    fn accepts_boundary_interval_values() {
        assert_eq!(
            Config::parse("interval_seconds = 1").unwrap().interval,
            Duration::from_secs(1)
        );
        assert_eq!(
            Config::parse("interval_seconds = 60").unwrap().interval,
            Duration::from_secs(60)
        );
    }
    #[test]
    fn rejects_out_of_range_interval() {
        assert!(Config::parse("interval_seconds = 0").is_err());
        assert!(Config::parse("interval_seconds = 61").is_err());
        assert!(Config::parse("interval_seconds = 999").is_err());
    }
    #[test]
    fn rejects_non_numeric_interval() {
        assert!(Config::parse("interval_seconds = abc").is_err());
        assert!(Config::parse("interval_seconds = 3.5").is_err());
        assert!(Config::parse("interval_seconds = true").is_err());
    }
    #[test]
    fn rejects_invalid_bool_for_log_transitions() {
        assert!(Config::parse("log_transitions = yes").is_err());
        assert!(Config::parse("log_transitions = 1").is_err());
        assert!(Config::parse("log_transitions = Truee").is_err());
    }
    #[test]
    fn rejects_missing_equals_sign() {
        assert!(Config::parse("interval_seconds 5").is_err());
        assert!(Config::parse("log_transitions true").is_err());
    }
    #[test]
    fn rejects_unknown_keys() {
        assert!(Config::parse("unknown = true").is_err());
        assert!(Config::parse("foo = bar").is_err());
        assert!(Config::parse("interval_seconds = 5\nunknown = true").is_err());
    }
    #[test]
    fn defaults_are_applied_when_empty() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.interval, Duration::from_secs(3));
        assert!(!config.log_transitions);
        let config = Config::parse("# solo comentarios\n\n").unwrap();
        assert_eq!(config.interval, Duration::from_secs(3));
    }
    #[test]
    fn last_value_wins_on_duplicate_keys() {
        let config = Config::parse("interval_seconds = 5\ninterval_seconds = 10").unwrap();
        assert_eq!(config.interval, Duration::from_secs(10));
    }

    #[test]
    fn config_format_never_includes_secrets() {
        let formatted = format_config(
            &Config {
                interval: Duration::from_secs(5),
                log_transitions: true,
                ..Config::default()
            },
            std::path::Path::new("config.toml"),
            true,
        );
        assert!(formatted.contains("Intervalo       5 s"));
        assert!(formatted.contains("Secretos        no se muestran aquí"));
    }

    #[test]
    fn saves_configuration_atomically() {
        let path = std::env::temp_dir().join("vtracker-config-atomic-test.toml");
        let config = Config {
            interval: Duration::from_secs(9),
            log_transitions: true,
            theme: Theme::Dark,
        };
        save_atomic(&path, &config).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(
            Config::parse(&content).unwrap().interval,
            Duration::from_secs(9)
        );
        assert!(Config::parse(&content).unwrap().log_transitions);
        assert!(!path.with_extension("toml.tmp").exists());
        // La TUI guarda sobre una configuración existente, también en Windows.
        let updated = Config {
            interval: Duration::from_secs(4),
            log_transitions: false,
            theme: Theme::Light,
        };
        save_atomic(&path, &updated).unwrap();
        assert_eq!(
            Config::parse(&fs::read_to_string(&path).unwrap()).unwrap(),
            updated
        );
        fs::remove_file(path).unwrap();
    }
}
