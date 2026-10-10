//! The user's choices, saved between runs by the shell.

use std::collections::BTreeMap;

use artcraft_launcher_core::PackageKind;
use serde::{Deserialize, Serialize};

use crate::theme::ThemeChoice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Grid,
    Table,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filter {
    #[default]
    All,
    Installed,
    Updates,
    NotInstalled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    /// Suite order: classic apps, then new.
    #[default]
    Suite,
    Name,
    /// The product an app is inspired by.
    InspiredBy,
    Status,
    /// Installed version.
    Installed,
    /// Latest released version.
    Latest,
    /// Release date.
    Released,
}

impl SortKey {
    /// The direction a first click sorts in: newest and highest first for dates and versions,
    /// A to Z for text.
    pub fn starts_descending(self) -> bool {
        matches!(self, Self::Installed | Self::Latest | Self::Released)
    }
}

/// How often to look for updates in the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckEvery {
    Hour,
    FourHours,
    Day,
    Never,
}

impl CheckEvery {
    pub const ALL: [Self; 4] = [Self::Hour, Self::FourHours, Self::Day, Self::Never];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hour => "Every hour",
            Self::FourHours => "Every 4 hours",
            Self::Day => "Once a day",
            Self::Never => "Only when I ask",
        }
    }

    pub fn seconds(self) -> Option<i64> {
        match self {
            Self::Hour => Some(3600),
            Self::FourHours => Some(4 * 3600),
            Self::Day => Some(24 * 3600),
            Self::Never => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub view: ViewMode,
    pub filter: Filter,
    pub sort: SortKey,
    pub sort_descending: bool,
    pub theme: ThemeChoice,
    /// Closing the window keeps the launcher in the tray (menu bar) instead of quitting.
    pub close_to_tray: bool,
    /// macOS: showing the window brings it to the current desktop (Space) instead of switching to
    /// the desktop it was on.
    pub show_on_current_desktop: bool,
    pub check_every: CheckEvery,
    /// Show a system notification when an installed app has an update.
    pub notify: bool,
    pub prereleases: bool,
    /// Linux: the package format to prefer for new installs (empty: the system's own).
    pub preferred_kinds: Vec<PackageKind>,
    /// Updates already announced, so each version is announced once: app id → version.
    pub notified: BTreeMap<String, String>,
    /// Unix seconds of the last finished check.
    pub last_check: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view: ViewMode::Grid,
            filter: Filter::All,
            sort: SortKey::Suite,
            sort_descending: false,
            theme: ThemeChoice::System,
            close_to_tray: true,
            show_on_current_desktop: true,
            check_every: CheckEvery::Hour,
            notify: true,
            prereleases: false,
            preferred_kinds: Vec::new(),
            notified: BTreeMap::new(),
            last_check: None,
        }
    }
}
