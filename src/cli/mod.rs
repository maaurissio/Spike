#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Doctor,
    Watch(WatchArgs),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WatchArgs {
    pub once: bool,
    pub interval_secs: Option<u64>,
}

/// Parsea los argumentos de CLI sin efectos secundarios (sin `process::exit`).
/// `args` corresponde a `env::args().skip(1)` ya colectado.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut iter = args.iter().map(|s| s.as_str());
    let Some(command) = iter.next() else {
        return Ok(Command::Watch(WatchArgs::default()));
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
    if command != "watch" {
        return Err(format!(
            "Comando no disponible en el MVP: {command}\nUsa `vtracker watch`."
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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn defaults_to_watch_when_no_args() {
        assert_eq!(parse(&s(&[])), Ok(Command::Watch(WatchArgs::default())));
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
    fn rejects_doctor_with_extra_args() {
        assert_eq!(
            parse(&s(&["doctor", "--once"])),
            Err("Opción desconocida: --once".into())
        );
    }

    #[test]
    fn rejects_unknown_command() {
        let err = parse(&s(&["player"])).unwrap_err();
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
