use std::{
    fs,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use crate::{errors::ConfigError, playback::Playlist};
use serde::{Deserialize, Serialize};

use tracing::{debug, error, info, warn};

#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Playlists {
    playlists: Vec<Playlist>,
}

impl Deref for Playlists {
    type Target = Vec<Playlist>;
    fn deref(&self) -> &Self::Target {
        &self.playlists
    }
}

impl DerefMut for Playlists {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.playlists
    }
}

impl Default for Playlists {
    fn default() -> Self {
        // Try to get the user's default "Music" folder if it exists on the OS, windows and gnome create one by default.
        let path = if cfg!(windows) {
            let userprofile = std::env::var("USERPROFILE")
                .expect("Every windows system should have the USERPROFILE variable");
            let mut path = PathBuf::from(userprofile);
            path.push("Music");
            path
        } else {
            let home =
                std::env::var("HOME").expect("Every unix system should have the HOME variable");
            let mut path = PathBuf::from(home);
            path.push("Music");
            path
        };
        let playlists = match Playlist::new(path, "Music".into(), None) {
            Ok(playlist) => vec![playlist],
            Err(_) => Vec::new(),
        };
        Self { playlists }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerConfig {
    pub buffer_size: usize,
    pub volume: f64,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            buffer_size: 2048,
            volume: 0.5,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InnerConfig {
    #[serde(flatten)]
    pub player: PlayerConfig,
    #[serde(rename = "playlist")]
    #[serde(default)]
    pub playlists: Playlists,
}

pub struct Config {
    pub path: PathBuf,
    inner: InnerConfig,
}

impl Deref for Config {
    type Target = InnerConfig;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Config {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Default for Config {
    fn default() -> Self {
        let path = Self::default_path();
        Self {
            path,
            inner: InnerConfig::default(),
        }
    }
}

impl Config {
    /// Try to write the config to the file system.
    ///
    /// Fails if the config has invalid data (serialization error), or the write failed.
    pub fn write(&self) -> Result<(), ConfigError> {
        Ok(fs::write(&self.path, toml::to_string_pretty(&self.inner)?)?)
    }

    /// Gets the default config path (`~/.config/amuseing/` on unix systems, `%APPDATA%/amuseing/` on windows).
    pub fn default_path() -> PathBuf {
        let mut path = if cfg!(windows) {
            let appdata = std::env::var("APPDATA")
                .expect("Every windows system should have the %APPDATA% variable");
            PathBuf::from(appdata)
        } else {
            let home =
                std::env::var("HOME").expect("Every unix system should have a HOME variable");
            let mut path = PathBuf::from(home);
            path.push(".config");
            path
        };
        path.push("amuseing");
        path
    }

    /// Get the config from the [`default_path]`.
    ///
    /// Use `Result::unwrap_or_default` to get the default config, and optionally write it with [`write`].
    ///
    /// [`default_path`]: Self::default_path
    /// [`write`]: Self::write
    pub fn from_default_path() -> Result<Self, ConfigError> {
        let path = Self::default_path();
        Self::from_path(path)
    }

    /// Get config from given `path`
    ///
    /// Fails if the `path` is invalid or it could not be parsed
    fn from_path(mut path: PathBuf) -> Result<Self, ConfigError> {
        path.push("config.toml");
        let toml_str = fs::read_to_string(&path)?;
        let inner: InnerConfig =
            toml::from_str(&toml_str).inspect_err(|e| error!("Error parsing config file: {e}"))?;
        Ok(Self { path, inner })
    }
}
