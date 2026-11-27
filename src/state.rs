//! What the app remembers between runs by itself, as opposed to the settings the user
//! writes in `config.toml`: `state.toml`, in the same folder. VS Code keeps the two
//! apart the same way (its UI state never goes into `settings.json`).
//!
//! ```toml
//! [panels]          # the dragged panel widths, in logical pixels
//! sidebar = 312.0
//! changes = 280.0
//!
//! [sidebar]
//! archived = ['C:\repos\old-project']   # archived repos, by their folder's full path
//! archived_open = false                 # whether the Archived section is open
//!
//! [sidebar.repo_colors]                 # badge colors picked in a repo's menu
//! 'C:\repos\web-app' = "Teal"
//!
//! [theme]           # the three picked colors; every other shade is worked out from them
//! background = "#18191c"
//! text = "#e6e6e9"
//! accent = "Mauve"  # a Catppuccin accent by name, or a hex code
//! ```
//!
//! The app owns this file and rewrites it whenever a divider is dropped, a repo is
//! archived or the theme changes. Nobody needs to edit it, so a missing or broken one
//! just means the defaults (the next save replaces it), and unknown keys are ignored
//! rather than rejected. A bad `[theme]` value only resets the theme, and says so.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize};

use crate::app::PanelWidths;
use crate::config::{self, Config, ConfigError};
use crate::theme::palette::{Accent, ThemeSettings};

const FILE_NAME: &str = "state.toml";

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub panels: PanelWidths,
    pub sidebar: SidebarState,
    pub theme: ThemeSettings,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarState {
    /// Archived repos, by their folder's full path rather than their name, so another
    /// root with a repo of the same name isn't affected. Paths that aren't under the
    /// current root are kept, for when that root is chosen again.
    pub archived: BTreeSet<PathBuf>,
    /// Whether the Archived section is open; closed until opened.
    pub archived_open: bool,
    /// The repo colors picked in their menus, by folder like `archived`. A repo with no
    /// entry gets one worked out from its folder (see `sidebar::repo_color`). An entry
    /// that isn't a color name is dropped, so it costs only that repo's pick.
    #[serde(deserialize_with = "known_colors")]
    pub repo_colors: BTreeMap<PathBuf, Accent>,
}

fn known_colors<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<PathBuf, Accent>, D::Error> {
    let entries = BTreeMap::<PathBuf, toml::Value>::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .filter_map(|(repo, color)| Some((repo, color.try_into().ok()?)))
        .collect())
}

impl State {
    fn path() -> Result<PathBuf, ConfigError> {
        Ok(Config::dir()?.join(FILE_NAME))
    }

    /// Reads the saved state. Anything missing or unreadable gives the defaults. The
    /// second value is set when `[theme]` had a bad value, which is then reset to the
    /// default theme: unlike the rest, its values are typed in by hand (hex codes), so
    /// a typo is told, and it never costs the other sections.
    pub fn load() -> (State, Option<String>) {
        State::path()
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| State::parse(&text).ok())
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Result<(State, Option<String>), toml::de::Error> {
        let mut table: toml::Table = toml::from_str(text)?;
        let theme = table.remove("theme");
        let mut state: State = toml::Value::Table(table).try_into()?;

        let warning = theme.and_then(|theme| match theme.try_into() {
            Ok(theme) => {
                state.theme = theme;
                None
            }
            Err(err) => Some(format!(
                "The theme was reset to the default: {FILE_NAME} had a bad value in \
                 [theme] ({})",
                err.message()
            )),
        });
        Ok((state, warning))
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        config::write_toml(&State::path()?, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette::{AccentChoice, Rgb};

    /// Parses a file that must have no bad theme value.
    fn parse(text: &str) -> State {
        let (state, warning) = State::parse(text).unwrap();
        assert_eq!(warning, None);
        state
    }

    #[test]
    fn test_parse_full() {
        let state = parse("[panels]\nsidebar = 312.5\nchanges = 300.0\n");
        assert_eq!(state.panels.sidebar, 312.5);
        assert_eq!(state.panels.changes, 300.0);
    }

    #[test]
    fn test_parse_fills_in_missing_keys() {
        assert_eq!(parse(""), State::default());

        let state = parse("[panels]\nsidebar = 312.0\n");
        assert_eq!(state.panels.sidebar, 312.0);
        assert_eq!(state.panels.changes, PanelWidths::default().changes);
    }

    #[test]
    fn test_parse_ignores_unknown_keys() {
        assert_eq!(
            parse("future = 1\n[panels]\nother = 2\n[theme]\nother = 3\n"),
            State::default()
        );
    }

    #[test]
    fn test_parse_sidebar() {
        let state = parse("[sidebar]\narchived = ['C:\\repos\\old']\narchived_open = true\n");
        assert!(
            state
                .sidebar
                .archived
                .contains(&PathBuf::from(r"C:\repos\old"))
        );
        assert!(state.sidebar.archived_open);
        // A file from before archiving existed: nothing archived, section closed.
        assert_eq!(
            parse("[panels]\nsidebar = 312.0\n").sidebar,
            SidebarState::default()
        );
    }

    #[test]
    fn test_parse_theme() {
        let state = parse("[theme]\nbackground = '#eff1f5'\naccent = 'Teal'\n");
        assert_eq!(state.theme.background, Rgb(0xef, 0xf1, 0xf5));
        assert_eq!(state.theme.text, ThemeSettings::default().text);
    }

    #[test]
    fn test_bad_theme_value_resets_only_the_theme() {
        for theme in [
            "background = '#eff1f'",
            "text = 3",
            "accent = 'Purple'",
            "accent = '#zzzzzz'",
        ] {
            let text = format!("[sidebar]\narchived = ['C:\\repos\\old']\n[theme]\n{theme}\n");
            let (state, warning) = State::parse(&text).unwrap();

            assert_eq!(state.theme, ThemeSettings::default(), "{theme}");
            assert_eq!(state.sidebar.archived.len(), 1, "{theme}");
            let warning = warning.expect(theme);
            assert!(warning.starts_with("The theme was reset"), "{warning}");
        }
    }

    #[test]
    fn test_bad_repo_color_drops_only_that_entry() {
        let state = parse(
            "[sidebar]\narchived = ['C:\\repos\\old']\n\
             [sidebar.repo_colors]\n'C:\\a' = 'Teal'\n'C:\\b' = 'Teall'\n'C:\\c' = 3\n",
        );
        assert_eq!(state.sidebar.archived.len(), 1);
        assert_eq!(
            state.sidebar.repo_colors.into_iter().collect::<Vec<_>>(),
            vec![(PathBuf::from(r"C:\a"), Accent::Teal)]
        );
    }

    #[test]
    fn test_round_trip() {
        let mut state = State::default();
        state.panels.sidebar = 333.25;
        state
            .sidebar
            .archived
            .insert(PathBuf::from(r"C:\repos\old"));
        state.sidebar.archived_open = true;
        state
            .sidebar
            .repo_colors
            .insert(PathBuf::from(r"C:\repos\web-app"), Accent::Teal);
        state.theme.accent = AccentChoice::Custom(Rgb(0xff, 0x88, 0x00));

        let text = toml::to_string_pretty(&state).unwrap();
        assert_eq!(parse(&text), state);
    }
}
