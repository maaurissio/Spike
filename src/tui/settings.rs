use crate::config::Config;
use std::time::Duration;

/// Cambiar el borrador nunca modifica la configuración activa ni el disco.
pub(super) struct Settings {
    pub active: Config,
    pub draft: Config,
    pub selected: usize,
    pub saving: bool,
    message: &'static str,
}

impl Settings {
    const OPTION_COUNT: usize = 4;

    pub fn new(config: &Config) -> Self {
        Self {
            active: config.clone(),
            draft: config.clone(),
            selected: 0,
            saving: false,
            message: "",
        }
    }

    pub fn select(&mut self) {
        self.selected = (self.selected + 1) % Self::OPTION_COUNT;
    }

    pub fn previous(&mut self) {
        self.selected = (self.selected + Self::OPTION_COUNT - 1) % Self::OPTION_COUNT;
    }

    pub fn cycle_theme(&mut self) {
        if !self.saving {
            self.draft.theme = self.draft.theme.next();
            self.message = "";
        }
    }

    pub fn adjust(&mut self, increase: bool) {
        if self.saving {
            return;
        }
        if self.selected == 0 {
            self.draft.theme = if increase {
                self.draft.theme.next()
            } else {
                self.draft.theme.previous()
            };
        } else if self.selected == 1 {
            let seconds = self.draft.interval.as_secs();
            self.draft.interval = Duration::from_secs(if increase {
                seconds.saturating_add(1).min(60)
            } else {
                seconds.saturating_sub(1).max(1)
            });
        } else if self.selected == 2 {
            self.draft.log_transitions = !self.draft.log_transitions;
        }
        self.message = "";
    }

    pub fn toggle(&mut self) {
        if matches!(self.selected, 0 | 2) {
            self.adjust(true);
        }
    }

    pub fn discard(&mut self) {
        if !self.saving {
            self.draft = self.active.clone();
            self.message = "Cambios descartados.";
        }
    }

    pub fn to_save(&self) -> Option<Config> {
        (!self.saving && self.draft != self.active).then(|| self.draft.clone())
    }

    pub fn saved(&mut self, result: Result<Config, ()>) {
        self.saving = false;
        self.message = match result {
            Ok(config) => {
                self.active = config.clone();
                self.draft = config;
                "Guardado. Los ajustes ya están activos."
            }
            Err(()) => "No se pudo guardar. Se conservan los ajustes activos; s reintenta.",
        };
    }

    pub fn status(&self) -> &'static str {
        if self.saving {
            "Guardando…"
        } else if !self.message.is_empty() {
            self.message
        } else if self.draft != self.active {
            "Cambios sin guardar. s guarda; r descarta."
        } else {
            "Sin cambios pendientes."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_is_bounded_and_only_successful_save_applies_changes() {
        let config = Config::default();
        let mut settings = Settings::new(&config);
        settings.selected = 1;
        for _ in 0..100 {
            settings.adjust(false);
        }
        assert_eq!(settings.draft.interval.as_secs(), 1);
        for _ in 0..100 {
            settings.adjust(true);
        }
        assert_eq!(settings.draft.interval.as_secs(), 60);
        settings.select();
        settings.toggle();
        assert!(settings.draft.log_transitions);
        assert_eq!(settings.active, config);
        settings.saving = true;
        assert!(settings.to_save().is_none());
        settings.saved(Err(()));
        assert_eq!(settings.active, config);
        let requested = settings.to_save().unwrap();
        settings.saved(Ok(requested.clone()));
        assert_eq!(settings.active, requested);
        assert!(settings.to_save().is_none());
    }

    #[test]
    fn discard_does_not_enable_logging_or_change_active_interval() {
        let config = Config::default();
        let mut settings = Settings::new(&config);
        settings.selected = 1;
        settings.adjust(true);
        settings.select();
        settings.toggle();
        settings.discard();
        assert_eq!(settings.active, config);
        assert!(settings.to_save().is_none());
    }
    #[test]
    fn theme_preview_is_draft_until_saved_and_discard_restores_it() {
        let config = Config::default();
        let mut settings = Settings::new(&config);
        settings.cycle_theme();
        assert_ne!(settings.draft.theme, settings.active.theme);
        settings.discard();
        assert_eq!(settings.draft.theme, config.theme);
        settings.cycle_theme();
        let draft = settings.to_save().unwrap();
        settings.saving = true;
        settings.cycle_theme();
        assert_eq!(settings.draft, draft);
        settings.saved(Ok(draft.clone()));
        assert_eq!(settings.active, draft);
    }

    #[test]
    fn minus_reverses_theme_selection_and_wraps_backwards() {
        let mut settings = Settings::new(&Config::default());
        settings.adjust(true);
        settings.adjust(false);
        assert_eq!(settings.draft.theme, crate::config::Theme::Dark);
        settings.adjust(false);
        assert_eq!(settings.draft.theme, crate::config::Theme::System);
        assert_eq!(settings.active.theme, crate::config::Theme::Dark);
    }

    #[test]
    fn palette_button_is_a_fourth_non_mutating_option() {
        let config = Config::default();
        let mut settings = Settings::new(&config);
        settings.previous();
        assert_eq!(settings.selected, 3);
        settings.toggle();
        settings.adjust(true);
        assert_eq!(settings.draft, config);
        settings.select();
        assert_eq!(settings.selected, 0);
    }
}
