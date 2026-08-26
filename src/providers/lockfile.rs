use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockfileProtocol {
    Http,
    Https,
}

impl LockfileProtocol {
    fn parse(value: &str) -> Result<Self, LockfileError> {
        match value {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            _ => Err(LockfileError::Parse(format!("protocolo inválido: {value}"))),
        }
    }
}

impl fmt::Display for LockfileProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => f.write_str("http"),
            Self::Https => f.write_str("https"),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Lockfile {
    pub name: String,
    pub pid: u32,
    pub port: u16,
    password: String,
    pub protocol: LockfileProtocol,
}

impl Lockfile {
    pub fn parse(input: &str) -> Result<Self, LockfileError> {
        let trimmed = input.trim();
        let parts = trimmed.split(':').collect::<Vec<_>>();
        if parts.len() != 5 {
            return Err(LockfileError::Parse(
                "se esperaban 5 campos: name:pid:port:password:protocol".into(),
            ));
        }

        let name = parts[0];
        let pid = parse_positive_u32(parts[1], "pid")?;
        let port = parse_positive_u16(parts[2], "port")?;
        let password = parts[3];
        let protocol = LockfileProtocol::parse(parts[4])?;

        if name.is_empty() {
            return Err(LockfileError::Parse("name vacío".into()));
        }
        if password.is_empty() {
            return Err(LockfileError::Parse("password vacío".into()));
        }

        Ok(Self {
            name: name.into(),
            pid,
            port,
            password: password.into(),
            protocol,
        })
    }

    pub fn has_password(&self) -> bool {
        !self.password.is_empty()
    }
}

impl fmt::Debug for Lockfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lockfile")
            .field("name", &self.name)
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("password", &"<redacted>")
            .field("protocol", &self.protocol)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LockfileError {
    MissingLocalAppData,
    NotFound(PathBuf),
    Read(String),
    Parse(String),
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLocalAppData => f.write_str("LOCALAPPDATA no está disponible"),
            Self::NotFound(path) => write!(f, "no encontrado: {}", path.display()),
            Self::Read(error) => write!(f, "no se pudo leer: {error}"),
            Self::Parse(error) => write!(f, "formato inválido: {error}"),
        }
    }
}

impl std::error::Error for LockfileError {}

pub fn default_lockfile_path() -> Result<PathBuf, LockfileError> {
    let base = env::var_os("LOCALAPPDATA").ok_or(LockfileError::MissingLocalAppData)?;
    Ok(PathBuf::from(base)
        .join("Riot Games")
        .join("Riot Client")
        .join("Config")
        .join("lockfile"))
}

pub fn read(path: &Path) -> Result<Lockfile, LockfileError> {
    if !path.exists() {
        return Err(LockfileError::NotFound(path.to_path_buf()));
    }
    let contents =
        fs::read_to_string(path).map_err(|error| LockfileError::Read(error.to_string()))?;
    Lockfile::parse(&contents)
}

pub fn read_default() -> Result<Lockfile, LockfileError> {
    let path = default_lockfile_path()?;
    read(&path)
}

fn parse_positive_u32(value: &str, field: &str) -> Result<u32, LockfileError> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| LockfileError::Parse(format!("{field} no es numérico")))?;
    if parsed == 0 {
        return Err(LockfileError::Parse(format!(
            "{field} debe ser mayor que 0"
        )));
    }
    Ok(parsed)
}

fn parse_positive_u16(value: &str, field: &str) -> Result<u16, LockfileError> {
    let parsed = parse_positive_u32(value, field)?;
    u16::try_from(parsed).map_err(|_| LockfileError::Parse(format!("{field} fuera de rango")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_lockfile() {
        let lockfile = Lockfile::parse("riot:12345:56789:secret:https\n").unwrap();
        assert_eq!(lockfile.name, "riot");
        assert_eq!(lockfile.pid, 12345);
        assert_eq!(lockfile.port, 56789);
        assert_eq!(lockfile.password, "secret");
        assert_eq!(lockfile.protocol, LockfileProtocol::Https);
    }

    #[test]
    fn rejects_wrong_field_count() {
        let error = Lockfile::parse("riot:123:456:https").unwrap_err();
        assert!(error.to_string().contains("5 campos"));
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert!(Lockfile::parse("riot:abc:456:secret:https").is_err());
        assert!(Lockfile::parse("riot:123:0:secret:https").is_err());
        assert!(Lockfile::parse("riot:123:99999:secret:https").is_err());
    }

    #[test]
    fn rejects_empty_auth_fields() {
        assert!(Lockfile::parse(":123:456:secret:https").is_err());
        assert!(Lockfile::parse("riot:123:456::https").is_err());
    }

    #[test]
    fn rejects_unknown_protocol() {
        let error = Lockfile::parse("riot:123:456:secret:ws").unwrap_err();
        assert!(error.to_string().contains("protocolo"));
    }

    #[test]
    fn debug_redacts_password() {
        let lockfile = Lockfile::parse("riot:123:456:secret:https").unwrap();
        let debug = format!("{lockfile:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret"));
    }
}
