pub mod adapters;
pub mod assets;
pub mod config;
pub mod install;
pub mod rotation;
pub mod storage;

use anyhow::{Context, Result};
use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
}
impl Paths {
    pub fn discover() -> Result<Self> {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .context("Home directory unavailable")?
            .into();
        Ok(Self::for_home(home))
    }
    pub fn for_home(home: PathBuf) -> Self {
        let dir = |key: &str, fallback: &str| {
            env::var_os(key)
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(fallback))
                .join("paintings")
        };
        Self {
            config: dir("XDG_CONFIG_HOME", ".config"),
            state: dir("XDG_STATE_HOME", ".local/state"),
            cache: dir("XDG_CACHE_HOME", ".cache"),
            home,
        }
    }
    pub fn isolated(home: PathBuf) -> Self {
        Self {
            config: home.join("config"),
            state: home.join("state"),
            cache: home.join("cache"),
            home,
        }
    }
}
