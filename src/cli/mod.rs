#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Dashboard { demo: bool },
    Help,
    Doctor,
    Config(ConfigCommand),
    History(HistoryArgs),
    Player,
    Stats(StatsArgs),
    Terminal(TerminalCommand),
    Watch(WatchArgs),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCommand {
    Install,
    Status,
    Launch,
    Uninstall,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigCommand {
    Show,
    Validate,
    Edit(ConfigEditArgs),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ConfigEditArgs {
    pub interval_secs: Option<u64>,
    pub log_transitions: Option<bool>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WatchArgs {
    pub once: bool,
    pub interval_secs: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HistoryArgs {
    pub limit: u8,
}

impl Default for HistoryArgs {
    fn default() -> Self {
        Self { limit: 5 }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StatsArgs {
    pub limit: u8,
}

impl Default for StatsArgs {
    fn default() -> Self {
        Self { limit: 5 }
    }
}

/// Parsea los argumentos de CLI sin efectos secundarios (sin `process::exit`).
/// `args` corresponde a `env::args().skip(1)` ya colectado.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut iter = args.iter().map(|s| s.as_str());
    let Some(command) = iter.next() else {
        return Ok(Command::Dashboard { demo: false });
    };

    if matches!(command, "-h" | "--help" | "help") {
        return Ok(Command::Help);
    }
    if command == "doctor" {
        if let Some(option) = iter.next() {
            return Err(format!("Opción desconocida: {option}"));
        }
        return Ok(Command::Doctor);
    }
    if matches!(command, "dashboard" | "tui") {
        let mut demo = false;
        for option in iter {
            match option {
                "--demo" if !demo => demo = true,
                "--help" | "-h" => return Ok(Command::Help),
                _ => return Err(format!("Opción desconocida: {option}")),
            }
        }
        return Ok(Command::Dashboard { demo });
    }
    if command == "config" {
        let Some(subcommand) = iter.next() else {
            return Err("Uso: vtracker config show|validate".into());
        };
        return match subcommand {
            "show" => parse_config_readonly(&mut iter, ConfigCommand::Show),
            "validate" => parse_config_readonly(&mut iter, ConfigCommand::Validate),
            "edit" => parse_config_edit(&mut iter),
            _ => Err("Uso: vtracker config show|validate".into()),
        };
    }
    if command == "history" {
        return parse_history(&mut iter);
    }
    if command == "player" {
        if let Some(option) = iter.next() {
            return Err(format!("Opción desconocida: {option}"));
        }
        return Ok(Command::Player);
    }
    if command == "stats" {
        return parse_stats(&mut iter);
    }
    if command == "terminal" {
        let Some(subcommand) = iter.next() else {
            return Err("Uso: vtracker terminal install|status|launch|uninstall".into());
        };
        let terminal_command = match subcommand {
            "install" => TerminalCommand::Install,
            "status" => TerminalCommand::Status,
            "launch" => TerminalCommand::Launch,
            "uninstall" => TerminalCommand::Uninstall,
            _ => return Err("Uso: vtracker terminal install|status|launch|uninstall".into()),
        };
        if let Some(option) = iter.next() {
            return Err(format!("Opción desconocida: {option}"));
        }
        return Ok(Command::Terminal(terminal_command));
    }
    if command != "watch" {
        return Err(format!(
            "Comando no disponible: {command}\nUsa `vtracker` o `vtracker --help`."
        ));
    }

    let mut once = false;
    let mut interval_secs = None;
    while let Some(arg) = iter.next() {
        match arg {
            "--once" => once = true,
            "--interval" => {
                let Some(value) = iter.next() else {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                };
                let Ok(seconds) = value.parse::<u64>() else {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                };
                if !(1..=60).contains(&seconds) {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                }
                interval_secs = Some(seconds);
            }
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(format!("Opción desconocida: {arg}")),
        }
    }
    Ok(Command::Watch(WatchArgs {
        once,
        interval_secs,
    }))
}

fn parse_stats<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<Command, String> {
    let mut args = StatsArgs::default();
    while let Some(arg) = iter.next() {
        match arg {
            "--limit" => {
                let Some(value) = iter.next() else {
                    return Err("--limit debe estar entre 1 y 5.".into());
                };
                let Ok(limit) = value.parse::<u8>() else {
                    return Err("--limit debe estar entre 1 y 5.".into());
                };
                if !(1..=5).contains(&limit) {
                    return Err("--limit debe estar entre 1 y 5.".into());
                }
                args.limit = limit;
            }
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(format!("Opción desconocida: {arg}")),
        }
    }
    Ok(Command::Stats(args))
}

fn parse_history<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<Command, String> {
    let mut args = HistoryArgs::default();
    while let Some(arg) = iter.next() {
        match arg {
            "--limit" => {
                let Some(value) = iter.next() else {
                    return Err("--limit debe estar entre 1 y 20.".into());
                };
                let Ok(limit) = value.parse::<u8>() else {
                    return Err("--limit debe estar entre 1 y 20.".into());
                };
                if !(1..=20).contains(&limit) {
                    return Err("--limit debe estar entre 1 y 20.".into());
                }
                args.limit = limit;
            }
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(format!("Opción desconocida: {arg}")),
        }
    }
    Ok(Command::History(args))
}

fn parse_config_readonly<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    command: ConfigCommand,
) -> Result<Command, String> {
    if let Some(option) = iter.next() {
        return Err(format!("Opción desconocida: {option}"));
    }
    Ok(Command::Config(command))
}

fn parse_config_edit<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<Command, String> {
    let mut edit = ConfigEditArgs::default();
    while let Some(arg) = iter.next() {
        match arg {
            "--interval" => {
                let Some(value) = iter.next() else {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                };
                let Ok(seconds) = value.parse::<u64>() else {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                };
                if !(1..=60).contains(&seconds) {
                    return Err("--interval debe estar entre 1 y 60 segundos.".into());
                }
                edit.interval_secs = Some(seconds);
            }
            "--log-transitions" => {
                let Some(value) = iter.next() else {
                    return Err("--log-transitions debe ser true o false.".into());
                };
                edit.log_transitions = Some(
                    value
                        .parse::<bool>()
                        .map_err(|_| "--log-transitions debe ser true o false.")?,
                );
            }
            _ => return Err(format!("Opción desconocida: {arg}")),
        }
    }
    if edit.interval_secs.is_none() && edit.log_transitions.is_none() {
        return Err(
            "Uso: vtracker config edit --interval SEGUNDOS|--log-transitions true|false".into(),
        );
    }
    Ok(Command::Config(ConfigCommand::Edit(edit)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn defaults_to_dashboard_when_no_args() {
        assert_eq!(parse(&s(&[])), Ok(Command::Dashboard { demo: false }));
    }

    #[test]
    fn parses_help_variants() {
        assert_eq!(parse(&s(&["-h"])), Ok(Command::Help));
        assert_eq!(parse(&s(&["--help"])), Ok(Command::Help));
        assert_eq!(parse(&s(&["help"])), Ok(Command::Help));
    }

    #[test]
    fn parses_doctor_with_no_extra_args() {
        assert_eq!(parse(&s(&["doctor"])), Ok(Command::Doctor));
    }

    #[test]
    fn parses_dashboard_aliases() {
        assert_eq!(
            parse(&["dashboard".into()]),
            Ok(Command::Dashboard { demo: false })
        );
        assert_eq!(
            parse(&["tui".into()]),
            Ok(Command::Dashboard { demo: false })
        );
        assert_eq!(
            parse(&s(&["dashboard", "--demo"])),
            Ok(Command::Dashboard { demo: true })
        );
        assert!(parse(&s(&["dashboard", "--demo", "--demo"])).is_err());
        assert!(parse(&s(&["dashboard", "--once"])).is_err());
    }

    #[test]
    fn rejects_doctor_with_extra_args() {
        assert_eq!(
            parse(&s(&["doctor", "--once"])),
            Err("Opción desconocida: --once".into())
        );
    }

    #[test]
    fn parses_config_subcommands() {
        assert_eq!(
            parse(&s(&["config", "show"])),
            Ok(Command::Config(ConfigCommand::Show))
        );
        assert_eq!(
            parse(&s(&["config", "validate"])),
            Ok(Command::Config(ConfigCommand::Validate))
        );
    }

    #[test]
    fn parses_history_with_safe_limit() {
        assert_eq!(
            parse(&s(&["history"])),
            Ok(Command::History(HistoryArgs { limit: 5 }))
        );
        assert_eq!(
            parse(&s(&["history", "--limit", "10"])),
            Ok(Command::History(HistoryArgs { limit: 10 }))
        );
    }

    #[test]
    fn rejects_invalid_history_limit() {
        assert_eq!(
            parse(&s(&["history", "--limit", "0"])),
            Err("--limit debe estar entre 1 y 20.".into())
        );
        assert_eq!(
            parse(&s(&["history", "--limit", "21"])),
            Err("--limit debe estar entre 1 y 20.".into())
        );
    }

    #[test]
    fn parses_player_without_options() {
        assert_eq!(parse(&s(&["player"])), Ok(Command::Player));
        assert_eq!(
            parse(&s(&["player", "--all"])),
            Err("Opción desconocida: --all".into())
        );
    }

    #[test]
    fn parses_stats_with_bounded_limit() {
        assert_eq!(
            parse(&s(&["stats", "--limit", "3"])),
            Ok(Command::Stats(StatsArgs { limit: 3 }))
        );
        assert_eq!(
            parse(&s(&["stats", "--limit", "6"])),
            Err("--limit debe estar entre 1 y 5.".into())
        );
    }

    #[test]
    fn parses_terminal_profile_commands() {
        assert_eq!(
            parse(&s(&["terminal", "install"])),
            Ok(Command::Terminal(TerminalCommand::Install))
        );
        assert_eq!(
            parse(&s(&["terminal", "status"])),
            Ok(Command::Terminal(TerminalCommand::Status))
        );
        assert_eq!(
            parse(&s(&["terminal", "launch"])),
            Ok(Command::Terminal(TerminalCommand::Launch))
        );
        assert_eq!(
            parse(&s(&["terminal", "uninstall"])),
            Ok(Command::Terminal(TerminalCommand::Uninstall))
        );
        assert!(parse(&s(&["terminal"])).is_err());
        assert!(parse(&s(&["terminal", "install", "--force"])).is_err());
    }

    #[test]
    fn rejects_invalid_config_subcommands() {
        assert_eq!(
            parse(&s(&["config"])),
            Err("Uso: vtracker config show|validate".into())
        );
        assert_eq!(
            parse(&s(&["config", "edit"])),
            Err(
                "Uso: vtracker config edit --interval SEGUNDOS|--log-transitions true|false".into()
            )
        );
    }

    #[test]
    fn parses_config_edit_values() {
        assert_eq!(
            parse(&s(&[
                "config",
                "edit",
                "--interval",
                "5",
                "--log-transitions",
                "true"
            ])),
            Ok(Command::Config(ConfigCommand::Edit(ConfigEditArgs {
                interval_secs: Some(5),
                log_transitions: Some(true),
            })))
        );
    }

    #[test]
    fn rejects_unknown_command() {
        let err = parse(&s(&["rank"])).unwrap_err();
        assert!(err.contains("Comando no disponible"));
    }

    #[test]
    fn parses_watch_defaults() {
        assert_eq!(
            parse(&s(&["watch"])),
            Ok(Command::Watch(WatchArgs {
                once: false,
                interval_secs: None
            }))
        );
    }

    #[test]
    fn parses_watch_once() {
        assert_eq!(
            parse(&s(&["watch", "--once"])),
            Ok(Command::Watch(WatchArgs {
                once: true,
                interval_secs: None
            }))
        );
    }

    #[test]
    fn parses_watch_interval() {
        assert_eq!(
            parse(&s(&["watch", "--interval", "5"])),
            Ok(Command::Watch(WatchArgs {
                once: false,
                interval_secs: Some(5)
            }))
        );
    }

    #[test]
    fn parses_watch_once_and_interval_combined() {
        assert_eq!(
            parse(&s(&["watch", "--once", "--interval", "10"])),
            Ok(Command::Watch(WatchArgs {
                once: true,
                interval_secs: Some(10)
            }))
        );
        assert_eq!(
            parse(&s(&["watch", "--interval", "7", "--once"])),
            Ok(Command::Watch(WatchArgs {
                once: true,
                interval_secs: Some(7)
            }))
        );
    }

    #[test]
    fn watch_help_flag_returns_help() {
        assert_eq!(parse(&s(&["watch", "--help"])), Ok(Command::Help));
        assert_eq!(parse(&s(&["watch", "-h"])), Ok(Command::Help));
    }

    #[test]
    fn rejects_unknown_watch_option() {
        assert_eq!(
            parse(&s(&["watch", "--verbose"])),
            Err("Opción desconocida: --verbose".into())
        );
    }

    #[test]
    fn rejects_interval_without_value() {
        assert_eq!(
            parse(&s(&["watch", "--interval"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
    }

    #[test]
    fn rejects_interval_out_of_range() {
        assert_eq!(
            parse(&s(&["watch", "--interval", "0"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
        assert_eq!(
            parse(&s(&["watch", "--interval", "61"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
        assert_eq!(
            parse(&s(&["watch", "--interval", "abc"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
    }

    #[test]
    fn rejects_interval_with_negative_or_float() {
        assert_eq!(
            parse(&s(&["watch", "--interval", "-1"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
        assert_eq!(
            parse(&s(&["watch", "--interval", "3.5"])),
            Err("--interval debe estar entre 1 y 60 segundos.".into())
        );
    }
}
