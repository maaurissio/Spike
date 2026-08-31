use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use serde_json::{Value, json};

const PROFILE_NAME: &str = "VTRACKER";
const SCHEME_NAME: &str = "VTRACKER Gruvbox Dark";
const FONT_NAME: &str = "Fira Mono";
const PROFILE_GUID: &str = "{fae68b8f-fb8c-4e21-aec6-0d6fb610f080}";

#[derive(Debug)]
struct InstallationPaths {
    fragment: PathBuf,
    executable: PathBuf,
    font: PathBuf,
    settings: PathBuf,
    settings_backup: PathBuf,
}

fn paths() -> io::Result<InstallationPaths> {
    let local = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA no está disponible")
        })?;
    Ok(InstallationPaths {
        fragment: local
            .join("Microsoft")
            .join("Windows Terminal")
            .join("Fragments")
            .join("VTracker")
            .join("vtracker.json"),
        executable: local.join("VTracker").join("vtracker.exe"),
        font: local
            .join("Microsoft")
            .join("Windows")
            .join("Fonts")
            .join("FiraMono-Regular.ttf"),
        settings: local
            .join("Packages")
            .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.json"),
        settings_backup: local
            .join("Packages")
            .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.vtracker-backup.json"),
    })
}

fn terminal_executable() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where.exe").arg("wt.exe").output()
        && output.status.success()
        && let Some(path) = String::from_utf8_lossy(&output.stdout).lines().next()
    {
        return Some(PathBuf::from(path.trim()));
    }

    if let Some(local) = env::var_os("LOCALAPPDATA") {
        let alias = PathBuf::from(local)
            .join("Microsoft")
            .join("WindowsApps")
            .join("wt.exe");
        if alias.is_file() {
            return Some(alias);
        }
    }

    let windows_apps = env::var_os("ProgramFiles")
        .map(PathBuf::from)?
        .join("WindowsApps");
    let mut candidates = fs::read_dir(windows_apps)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("Microsoft.WindowsTerminal_")
                        && name.ends_with("__8wekyb3d8bbwe")
                })
        })
        .map(|path| path.join("wt.exe"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop()
}

fn profile(executable: &str) -> Value {
    json!({
            "guid": PROFILE_GUID,
            "name": PROFILE_NAME,
            "commandline": executable,
            "colorScheme": SCHEME_NAME,
            "font": {
                "face": FONT_NAME,
                "size": 12,
                "weight": "normal"
            },
            "antialiasingMode": "grayscale",
            "padding": "8",
            "scrollbarState": "hidden",
            "tabTitle": PROFILE_NAME,
            "suppressApplicationTitle": true,
            "cursorShape": "vintage"
    })
}

fn scheme() -> Value {
    json!({
            "name": SCHEME_NAME,
            "background": "#282828",
            "foreground": "#EBDBB2",
            "cursorColor": "#FABD2F",
            "selectionBackground": "#504945",
            "black": "#282828",
            "red": "#CC241D",
            "green": "#98971A",
            "yellow": "#D79921",
            "blue": "#458588",
            "purple": "#B16286",
            "cyan": "#689D6A",
            "white": "#A89984",
            "brightBlack": "#928374",
            "brightRed": "#FB4934",
            "brightGreen": "#B8BB26",
            "brightYellow": "#FABD2F",
            "brightBlue": "#83A598",
            "brightPurple": "#D3869B",
            "brightCyan": "#8EC07C",
            "brightWhite": "#EBDBB2"
    })
}

fn profile_fragment(executable: &str) -> Value {
    json!({
        "profiles": [profile(executable)],
        "schemes": [scheme()]
    })
}

fn write_fragment(path: &Path, executable: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("ruta de perfil inválida"))?;
    fs::create_dir_all(parent)?;
    let executable = executable.to_string_lossy();
    let content = serde_json::to_string_pretty(&profile_fragment(&executable))?;
    fs::write(path, format!("{content}\n"))
}

fn refresh_terminal_settings() {
    let Some(local) = env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return;
    };
    for settings in [
        local
            .join("Packages")
            .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.json"),
        local
            .join("Packages")
            .join("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.json"),
        local
            .join("Microsoft")
            .join("Windows Terminal")
            .join("settings.json"),
    ] {
        if let Ok(file) = fs::OpenOptions::new().write(true).open(settings) {
            let _ = file.set_modified(SystemTime::now());
        }
    }
}

fn merge_settings(paths: &InstallationPaths, executable: &Path) -> io::Result<()> {
    if !paths.settings.is_file() {
        return Ok(());
    }
    if !paths.settings_backup.exists() {
        fs::copy(&paths.settings, &paths.settings_backup)?;
    }
    let mut settings: Value = serde_json::from_str(&fs::read_to_string(&paths.settings)?)?;
    let profiles = settings
        .pointer_mut("/profiles/list")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| io::Error::other("settings.json no contiene profiles.list"))?;
    profiles
        .retain(|candidate| candidate.get("guid").and_then(Value::as_str) != Some(PROFILE_GUID));
    profiles.push(profile(&executable.to_string_lossy()));

    let schemes = settings
        .get_mut("schemes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| io::Error::other("settings.json no contiene schemes"))?;
    schemes.retain(|candidate| candidate.get("name").and_then(Value::as_str) != Some(SCHEME_NAME));
    schemes.push(scheme());
    fs::write(
        &paths.settings,
        format!("{}\n", serde_json::to_string_pretty(&settings)?),
    )
}

fn remove_from_settings(paths: &InstallationPaths) -> io::Result<()> {
    if !paths.settings.is_file() {
        return Ok(());
    }
    let mut settings: Value = serde_json::from_str(&fs::read_to_string(&paths.settings)?)?;
    if let Some(profiles) = settings
        .pointer_mut("/profiles/list")
        .and_then(Value::as_array_mut)
    {
        profiles.retain(|candidate| {
            candidate.get("guid").and_then(Value::as_str) != Some(PROFILE_GUID)
        });
    }
    if let Some(schemes) = settings.get_mut("schemes").and_then(Value::as_array_mut) {
        schemes
            .retain(|candidate| candidate.get("name").and_then(Value::as_str) != Some(SCHEME_NAME));
    }
    fs::write(
        &paths.settings,
        format!("{}\n", serde_json::to_string_pretty(&settings)?),
    )
}

fn settings_profile_installed(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|settings| settings.pointer("/profiles/list")?.as_array().cloned())
        .is_some_and(|profiles| {
            profiles.iter().any(|candidate| {
                candidate.get("guid").and_then(Value::as_str) == Some(PROFILE_GUID)
            })
        })
}

pub fn install() -> io::Result<String> {
    if terminal_executable().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows Terminal no está instalado o wt.exe no está disponible",
        ));
    }
    let paths = paths()?;
    if !paths.font.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Fira Mono no está instalada para este usuario",
        ));
    }
    let current = env::current_exe()?;
    if current != paths.executable {
        let parent = paths
            .executable
            .parent()
            .ok_or_else(|| io::Error::other("ruta de instalación inválida"))?;
        fs::create_dir_all(parent)?;
        fs::copy(current, &paths.executable)?;
    }
    write_fragment(&paths.fragment, &paths.executable)?;
    merge_settings(&paths, &paths.executable)?;
    refresh_terminal_settings();
    Ok(format!(
        "Perfil {PROFILE_NAME} instalado.\nFuente: {FONT_NAME} 12 pt\nEjecutable: {}\nPerfil: {}",
        paths.executable.display(),
        paths.fragment.display()
    ))
}

pub fn status() -> io::Result<String> {
    let paths = paths()?;
    Ok(format!(
        "Windows Terminal: {}\nFira Mono: {}\nPerfil VTRACKER: {}\nEjecutable instalado: {}",
        available(terminal_executable().is_some()),
        available(paths.font.is_file()),
        available(paths.fragment.is_file() && settings_profile_installed(&paths.settings)),
        available(paths.executable.is_file())
    ))
}

pub fn launch() -> io::Result<String> {
    let paths = paths()?;
    if !paths.fragment.is_file() || !paths.executable.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "el perfil no está instalado; ejecuta `vtracker terminal install`",
        ));
    }
    let terminal = terminal_executable().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Windows Terminal no está instalado o wt.exe no está disponible",
        )
    })?;
    Command::new(terminal)
        .args(["-w", "new", "new-tab", "-p", PROFILE_GUID])
        .spawn()?;
    Ok("Abriendo VTRACKER en su perfil de Windows Terminal.".into())
}

pub fn uninstall() -> io::Result<String> {
    let paths = paths()?;
    if paths.fragment.is_file() {
        fs::remove_file(&paths.fragment)?;
    }
    remove_from_settings(&paths)?;
    if paths.executable.is_file() && env::current_exe()? != paths.executable {
        fs::remove_file(&paths.executable)?;
    }
    if let Some(directory) = paths.fragment.parent() {
        let _ = fs::remove_dir(directory);
    }
    Ok("Perfil VTRACKER eliminado. Windows Terminal y Fira Mono se conservaron.".into())
}

fn available(value: bool) -> &'static str {
    if value { "disponible" } else { "no disponible" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_is_isolated_and_uses_gruvbox_with_fira_mono() {
        let fragment = profile_fragment(r"C:\\Program Files\\VTracker\\vtracker.exe");
        let profile = &fragment["profiles"][0];
        assert_eq!(profile["name"], PROFILE_NAME);
        assert_eq!(profile["guid"], PROFILE_GUID);
        assert_eq!(profile["font"]["face"], FONT_NAME);
        assert_eq!(profile["font"]["size"], 12);
        assert_eq!(profile["scrollbarState"], "hidden");
        assert_eq!(fragment["schemes"][0]["background"], "#282828");
        assert_eq!(fragment["schemes"][0]["brightYellow"], "#FABD2F");
    }
}
