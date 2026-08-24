use std::{env, fs, path::PathBuf, time::Duration};

#[derive(Debug)]
pub struct Config {
    pub interval: Duration,
    pub log_transitions: bool,
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
    fn rejects_invalid_configuration() {
        assert!(Config::parse("interval_seconds = 0").is_err());
        assert!(Config::parse("unknown = true").is_err());
    }
}
