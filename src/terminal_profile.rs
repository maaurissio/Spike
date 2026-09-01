use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime},
};

use serde_json::{Value, json};

const PROFILE_NAME: &str = "SPIKE";
const SCHEME_NAME: &str = "SPIKE Gruvbox Dark";
const FONT_NAME: &str = "Fira Mono";
const FONT_REGISTRY_NAME: &str = "Fira Mono Regular (TrueType)";
const PROFILE_GUID: &str = "{fae68b8f-fb8c-4e21-aec6-0d6fb610f080}";
const TERMINAL_PACKAGE_ID: &str = "Microsoft.WindowsTerminal";
const TERMINAL_RELEASES_API: &str =
    "https://api.github.com/repos/microsoft/terminal/releases/latest";
const MAX_TERMINAL_DOWNLOAD_BYTES: u64 = 150 * 1024 * 1024;
const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/FiraMono-Regular.ttf");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DashboardBootstrap {
    Continue,
    Relaunched,
}

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
            .join("Spike")
            .join("spike.json"),
        executable: local.join("Spike").join("spike.exe"),
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
            .join("settings.spike-backup.json"),
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

    if let Some(path) = terminal_from_appx_package() {
        return Some(path);
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

fn terminal_from_appx_package() -> Option<PathBuf> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-AppxPackage -Name Microsoft.WindowsTerminal | Select-Object -First 1 -ExpandProperty InstallLocation)",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let location = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if location.is_empty() {
        return None;
    }
    ["wt.exe", "WindowsTerminal.exe"]
        .into_iter()
        .map(|name| PathBuf::from(&location).join(name))
        .find(|path| path.is_file())
}

fn executable_on_path(name: &str) -> Option<PathBuf> {
    let output = Command::new("where.exe").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .next()
}

fn wait_for_terminal() -> Option<PathBuf> {
    for _ in 0..20 {
        if let Some(terminal) = terminal_executable() {
            return Some(terminal);
        }
        thread::sleep(Duration::from_millis(250));
    }
    None
}

fn install_terminal_with_winget(winget: &Path) -> io::Result<()> {
    println!("Windows Terminal no está instalado. Instalando con WinGet...");
    let status = Command::new(winget)
        .args([
            "install",
            "--id",
            TERMINAL_PACKAGE_ID,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--disable-interactivity",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "WinGet no pudo instalar Windows Terminal (código {})",
            status.code().unwrap_or(-1)
        )))
    }
}

fn terminal_release_asset() -> io::Result<(String, String)> {
    let response = reqwest::blocking::Client::new()
        .get(TERMINAL_RELEASES_API)
        .header(
            reqwest::header::USER_AGENT,
            format!("Spike/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| io::Error::other(format!("no se pudo consultar GitHub: {error}")))?;
    let release: Value = response
        .json()
        .map_err(|error| io::Error::other(format!("respuesta de GitHub inválida: {error}")))?;
    select_terminal_release_asset(&release)
        .ok_or_else(|| io::Error::other("la versión estable no incluye un paquete msixbundle"))
}

fn select_terminal_release_asset(release: &Value) -> Option<(String, String)> {
    release["assets"].as_array().and_then(|assets| {
        assets.iter().find_map(|asset| {
            let name = asset["name"].as_str()?;
            let url = asset["browser_download_url"].as_str()?;
            (name.starts_with("Microsoft.WindowsTerminal_")
                && name.ends_with("_8wekyb3d8bbwe.msixbundle"))
            .then(|| (name.to_owned(), url.to_owned()))
        })
    })
}

fn install_terminal_from_official_release() -> io::Result<()> {
    println!("WinGet no está disponible. Descargando Windows Terminal desde Microsoft...");
    let (name, url) = terminal_release_asset()?;
    let package = env::temp_dir().join(format!("spike-{}-{name}", std::process::id()));
    let result = (|| {
        let response = reqwest::blocking::Client::new()
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                format!("Spike/{}", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|error| {
                io::Error::other(format!("no se pudo descargar Windows Terminal: {error}"))
            })?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_TERMINAL_DOWNLOAD_BYTES)
        {
            return Err(io::Error::other(
                "el paquete de Windows Terminal supera el límite permitido",
            ));
        }
        let mut source = response.take(MAX_TERMINAL_DOWNLOAD_BYTES + 1);
        let mut destination = fs::File::create(&package)?;
        let copied = io::copy(&mut source, &mut destination)?;
        if copied > MAX_TERMINAL_DOWNLOAD_BYTES {
            return Err(io::Error::other(
                "el paquete de Windows Terminal supera el límite permitido",
            ));
        }

        let escaped = package.to_string_lossy().replace('\'', "''");
        let script = format!("Add-AppxPackage -LiteralPath '{escaped}'");
        let status = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Windows no pudo instalar el paquete oficial (código {})",
                status.code().unwrap_or(-1)
            )))
        }
    })();
    let _ = fs::remove_file(package);
    result
}

fn ensure_windows_terminal() -> io::Result<()> {
    if terminal_executable().is_some() {
        return Ok(());
    }
    if let Some(winget) = executable_on_path("winget.exe") {
        if let Err(error) = install_terminal_with_winget(&winget) {
            eprintln!("WinGet no completó la instalación ({error}). Probando descarga oficial...");
            install_terminal_from_official_release()?;
        }
    } else {
        install_terminal_from_official_release()?;
    }
    wait_for_terminal().map(|_| ()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Windows Terminal se instaló, pero wt.exe todavía no está disponible; inicia sesión de nuevo y ejecuta Spike",
        )
    })
}

fn font_registered(path: &Path) -> bool {
    let Ok(output) = Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts",
        ])
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains(&path.to_string_lossy().to_ascii_lowercase())
}

fn font_ready(path: &Path) -> bool {
    path.is_file() && fs::read(path).is_ok_and(|bytes| bytes == FONT_BYTES) && font_registered(path)
}

fn install_font(path: &Path) -> io::Result<()> {
    if font_ready(path) {
        return Ok(());
    }
    if !path.is_file() || !fs::read(path).is_ok_and(|bytes| bytes == FONT_BYTES) {
        println!("Instalando {FONT_NAME} para el usuario actual...");
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("ruta de fuente inválida"))?;
        fs::create_dir_all(parent)?;
        fs::write(path, FONT_BYTES)?;
    }
    let status = Command::new("reg.exe")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts",
            "/v",
            FONT_REGISTRY_NAME,
            "/t",
            "REG_SZ",
            "/d",
            &path.to_string_lossy(),
            "/f",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Windows no pudo registrar {FONT_NAME} (código {})",
            status.code().unwrap_or(-1)
        )))
    }
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
            // El marco de la TUI ya aporta su propio borde. Dejar padding en
            // Windows Terminal aparentaba una barra lateral aun con el scroll
            // oculto y quitaba columnas útiles a Ratatui.
            "padding": "0",
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
    ensure_windows_terminal()?;
    let paths = paths()?;
    install_font(&paths.font)?;
    let current = env::current_exe()?;
    if !same_executable(&current, &paths.executable) {
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
        "Windows Terminal: {}\nFira Mono: {}\nPerfil SPIKE: {}\nEjecutable instalado: {}",
        available(terminal_executable().is_some()),
        available(font_ready(&paths.font)),
        available(paths.fragment.is_file() || settings_profile_installed(&paths.settings)),
        available(paths.executable.is_file())
    ))
}

fn same_executable(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Prepara silenciosamente el entorno del dashboard en ejecuciones posteriores.
/// En el primer arranque instala los requisitos, crea el perfil y relanza Spike
/// dentro de Windows Terminal. El proceso relanzado continúa hacia la TUI.
pub fn bootstrap_dashboard() -> io::Result<DashboardBootstrap> {
    let paths = paths()?;
    let current = env::current_exe()?;
    let already_in_profile = env::var_os("WT_SESSION").is_some()
        && paths.executable.is_file()
        && same_executable(&current, &paths.executable);

    if already_in_profile {
        // Repara recursos eliminados sin abrir una segunda pestaña.
        ensure_windows_terminal()?;
        install_font(&paths.font)?;
        if !paths.fragment.is_file() {
            write_fragment(&paths.fragment, &paths.executable)?;
            refresh_terminal_settings();
        }
        return Ok(DashboardBootstrap::Continue);
    }

    install()?;
    launch()?;
    Ok(DashboardBootstrap::Relaunched)
}

pub fn launch() -> io::Result<String> {
    let paths = paths()?;
    if !paths.fragment.is_file() || !paths.executable.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "el perfil no está instalado; ejecuta `spike terminal install`",
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
    Ok("Abriendo SPIKE en su perfil de Windows Terminal.".into())
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
    Ok("Perfil SPIKE eliminado. Windows Terminal y Fira Mono se conservaron.".into())
}

fn available(value: bool) -> &'static str {
    if value { "disponible" } else { "no disponible" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_is_isolated_and_uses_gruvbox_with_fira_mono() {
        let fragment = profile_fragment(r"C:\\Program Files\\Spike\\spike.exe");
        let profile = &fragment["profiles"][0];
        assert_eq!(profile["name"], PROFILE_NAME);
        assert_eq!(profile["guid"], PROFILE_GUID);
        assert_eq!(profile["font"]["face"], FONT_NAME);
        assert_eq!(profile["font"]["size"], 12);
        assert_eq!(profile["padding"], "0");
        assert_eq!(profile["scrollbarState"], "hidden");
        assert_eq!(fragment["schemes"][0]["background"], "#282828");
        assert_eq!(fragment["schemes"][0]["brightYellow"], "#FABD2F");
        assert!(FONT_BYTES.starts_with(&[0, 1, 0, 0]));
    }

    #[test]
    fn selects_only_the_stable_terminal_bundle() {
        let release = json!({
            "assets": [
                {
                    "name": "Microsoft.WindowsTerminal_1.0_8wekyb3d8bbwe.msixbundle_Windows10_PreinstallKit.zip",
                    "browser_download_url": "https://example.invalid/preinstall.zip"
                },
                {
                    "name": "Microsoft.WindowsTerminal_1.0_8wekyb3d8bbwe.msixbundle",
                    "browser_download_url": "https://github.com/microsoft/terminal/releases/download/v1.0/terminal.msixbundle"
                }
            ]
        });
        assert_eq!(
            select_terminal_release_asset(&release),
            Some((
                "Microsoft.WindowsTerminal_1.0_8wekyb3d8bbwe.msixbundle".into(),
                "https://github.com/microsoft/terminal/releases/download/v1.0/terminal.msixbundle"
                    .into()
            ))
        );
    }
}
