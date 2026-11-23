use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub name: String,
    pub source: PathBuf,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub path: PathBuf,
    pub loc: Location,
}

#[derive(Debug, Clone)]
pub enum Location {
    Local { name: String },
    Remote { name: String, host: RemoteHost },
}

#[derive(Debug, Clone)]
pub struct RemoteHost {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub login: String,
    pub pass: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

// --- Сырые структуры для десериализации TOML ---

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    pub hosts: HashMap<String, RawHost>,
    #[serde(default)]
    pub locations: HashMap<String, RawLocation>,
    #[serde(default)]
    pub artifacts: Vec<RawArtifact>,
}

#[derive(Debug, Deserialize)]
struct RawArtifact {
    pub name: String,
    pub source: PathBuf,
    #[serde(default)]
    pub targets: Vec<RawTarget>,
}

#[derive(Debug, Deserialize)]
struct RawTarget {
    pub path: Option<PathBuf>,
    pub loc: String,
}

#[derive(Debug, Deserialize)]
enum RawLocation {
    Local { path: Option<PathBuf> },
    Remote { host: String, path: Option<PathBuf> },
}

#[derive(Debug, Deserialize)]
struct RawHost {
    pub address: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub login: String,
    pub pass: Option<String>,
}

// --- Пути и загрузка ---

pub fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~") {
        // ~user/... is not supported (lazy ass)
        if rest.components().next().is_some() {
            return path.to_path_buf();
        }
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

pub fn resolve_config_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(expand_tilde(&path));
    }
    let mut candidates = vec![PathBuf::from("prollout.toml")];
    if let Some(cfg_dir) = dirs::config_dir() {
        candidates.push(cfg_dir.join("prollout/prollout.toml"));
    }
    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }
    bail!("config not found. specify with --config (-c), or put it at ./prollout.toml");
}

pub fn load_config(explicit_path: Option<PathBuf>) -> Result<Config> {
    let path = resolve_config_path(explicit_path)?;
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;

    let parsed: RawConfig = toml::from_str(&raw)
        .with_context(|| format!("TOML parsing error from {}", path.display()))?;

    let cfg = resolve_config(parsed)?;
    validate(&cfg)?;
    Ok(cfg)
}

fn resolve_host(host_name: &str, hosts: &HashMap<String, RawHost>) -> Result<RemoteHost> {
    let h = hosts
        .get(host_name)
        .with_context(|| format!("unknown host '{}' referenced in config", host_name))?;

    Ok(RemoteHost {
        name: host_name.to_string(),
        address: h.address.clone(),
        port: h.port,
        login: h.login.clone(),
        pass: h.pass.clone(),
    })
}

fn resolve_location(
    loc_name: &str,
    target_path: Option<PathBuf>,
    locations: &HashMap<String, RawLocation>,
    hosts: &HashMap<String, RawHost>,
) -> Result<(PathBuf, Location)> {
    let entry = locations
        .get(loc_name)
        .with_context(|| format!("unknown location '{}' referenced in config", loc_name))?;

    let name = loc_name.to_string();
    let (default_path, loc) = match entry {
        RawLocation::Local { path } => (path, Location::Local { name: name }),
        RawLocation::Remote { host, path } => {
            let h = resolve_host(host, hosts)?;
            (
                path,
                Location::Remote {
                    name: name,
                    host: h,
                },
            )
        }
    };

    let final_path = match target_path {
        Some(p) => p,
        None => default_path
            .clone()
            .with_context(|| format!("location '{loc_name}' has no path"))?,
    };
    Ok((expand_tilde(&final_path), loc))
}

fn resolve_config(raw: RawConfig) -> Result<Config> {
    let RawConfig {
        hosts,
        locations,
        artifacts,
    } = raw;

    let mut resolved = Vec::with_capacity(artifacts.len());
    for raw_art in artifacts {
        let mut targets = Vec::with_capacity(raw_art.targets.len());
        for raw_tgt in raw_art.targets {
            let (path, loc) = resolve_location(&raw_tgt.loc, raw_tgt.path, &locations, &hosts)?;
            targets.push(Target { path, loc });
        }
        resolved.push(Artifact {
            name: raw_art.name,
            source: expand_tilde(&raw_art.source),
            targets,
        });
    }
    Ok(Config {
        artifacts: resolved,
    })
}

fn validate(cfg: &Config) -> Result<()> {
    if cfg.artifacts.is_empty() {
        bail!("config does not have any artifacts ([[artifacts]])");
    }

    for artifact in &cfg.artifacts {
        if artifact.name.trim().is_empty() {
            bail!("some of the artifacts don't have a name or it's empty");
        }
        if artifact.targets.is_empty() {
            bail!("artifact '{}' has no targets", artifact.name);
        }

        for target in &artifact.targets {
            if let Location::Remote { host, .. } = &target.loc {
                if host.address.trim().is_empty() {
                    bail!("address of remote location of '{}' is empty", artifact.name);
                }
                if host.login.trim().is_empty() {
                    bail!("login of remote location of '{}' is empty", artifact.name);
                }
            }
        }
    }

    Ok(())
}
