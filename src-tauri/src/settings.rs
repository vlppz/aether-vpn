use aether_core::CoreSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Ru,
}

/// Mirrors `Settings` in ui/src/lib/types.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub core: CoreSettings,
    pub theme: Theme,
    pub accent: String,
    pub language: Language,
    pub launch_at_login: bool,
    pub start_minimized: bool,
    pub auto_connect: bool,
    pub set_system_proxy: bool,
    pub close_to_tray: bool,
    pub user_agent: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            core: CoreSettings::default(),
            theme: Theme::System,
            accent: "#2F5FD0".into(),
            language: Language::En,
            launch_at_login: false,
            start_minimized: false,
            auto_connect: false,
            set_system_proxy: true,
            close_to_tray: true,
            user_agent: None,
        }
    }
}
