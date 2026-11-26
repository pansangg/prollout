use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// holds all Artifacts
#[derive(Debug, Clone)]
pub struct Config {
    pub artifacts: Vec<Artifact>,
}

/// a file, script, plugin etc. that user (probably) wants to roll out
#[derive(Debug, Clone)]
pub struct Artifact {
    pub name: String,
    pub source: PathBuf,
    pub targets: Vec<Target>,
}

/// where the Artifact should be placed
#[derive(Debug, Clone)]
pub struct Target {
    pub path: PathBuf,
    pub location: Location,
}

/// destination of Target
#[derive(Debug, Clone)]
pub enum Location {
    Local {
        name: String,
        path: PathBuf,
    },
    Remote {
        name: String,
        path: PathBuf,
        host: RemoteHost,
    },
}

impl Location {
    pub fn name(&self) -> &str {
        match self {
            Location::Local { name, .. } => name,
            Location::Remote { name, .. } => name,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Location::Local { path, .. } => path,
            Location::Remote { path, .. } => path,
        }
    }
}

/// SFTP host details
#[derive(Debug, Clone, Deserialize)]
pub struct RemoteHost {
    pub name: String,
    pub address: String,
    pub port: Option<u16>,
    pub login: String,
    pub pass: String,
}

fn default_ssh_port() -> u16 {
    22
}

/// Config with raw values
#[derive(Debug, Clone, Deserialize)]
pub struct RawConfig {
    pub artifacts: Vec<RawArtifact>,
    pub locations: Vec<RawLocation>,
    pub hosts: Vec<RemoteHost>,
}

/// Artifact with raw values
#[derive(Debug, Clone, Deserialize)]
pub struct RawArtifact {
    pub name: String,
    pub source: String,
    pub targets: Vec<RawTarget>,
}

/// target of Artifact with raw path (may have its own or inherit from Location)
#[derive(Debug, Clone, Deserialize)]
pub struct RawTarget {
    pub path: Option<String>,
    pub location: String,
}

/// Location with raw values
#[derive(Debug, Clone, Deserialize)]
pub struct RawLocation {
    pub kind: String,
    pub name: String,
    pub path: String,
    pub host: Option<String>,
}

/// loads the config and turns it into a Config instance
pub fn load_config(path: &Path) -> Result<Config> {
    let string_config = fs::read_to_string(path)
        .with_context(|| format!("failed to read from {}", path.display()))?;
    let raw_config: RawConfig =
        toml::from_str(&string_config).with_context(|| "failed to parse TOML")?;
    let mut locations: HashMap<String, Location> =
        HashMap::with_capacity(raw_config.locations.len());

    for raw_loc in raw_config.locations {
        locations.insert(raw_loc.name.clone(), resolve_location(raw_loc)?);
    }

    let mut config = Config {
        artifacts: Vec::new(),
    };
    for raw_artifact in raw_config.artifacts {
        let mut targets: Vec<Target> = Vec::with_capacity(raw_artifact.targets.len());
        for raw_target in raw_artifact.targets {
            let loc_name = &raw_target.location;
            let loc: Location = locations.get(loc_name).cloned().with_context(|| {
                format!(
                    "location \"{}\" for target of artifact {} not found",
                    loc_name, raw_artifact.name
                )
            })?;

            targets.push(resolve_target(raw_target, loc));
        }

        config.artifacts.push(Artifact {
            name: raw_artifact.name,
            source: PathBuf::from(raw_artifact.source),
            targets,
        });
    }

    Ok(config)
}

// fn resolve_host(raw_host: RawRemoteHost) -> RemoteHost {
//     RemoteHost {
//         name: raw_host.name,
//         address: raw_host.address,
//         port: raw_host.port,
//         login: raw_host.login,
//         pass: raw_host.pass,
//     }
// }

/// resolves RawLocation -> Location
fn resolve_location(raw_loc: RawLocation) -> Result<Location> {
    match raw_loc.kind.as_str() {
        "local" => {
            if raw_loc.host.is_some() {
                bail!(
                    "location \"{}\" of type local can't have field \"host\"",
                    raw_loc.name
                )
            }
            Ok(Location::Local {
                name: raw_loc.name,
                path: PathBuf::from(raw_loc.path),
            })
        }
        "remote" => bail!(
            "location {} is of type remote; remote type not supported yet",
            raw_loc.name
        ),
        // "remote" => Location::Remote {
        //     name: raw_loc.name,
        //     path: raw_loc.path.map(PathBuf::from),
        //     host: raw_loc.host.map(), TODO!
        // },
        _ => bail!(
            "location {} of an incorrect type \"{}\"",
            raw_loc.name,
            raw_loc.kind
        ),
    }
}

/// resolves RawTarget -> Target
fn resolve_target(raw_target: RawTarget, location: Location) -> Target {
    let mut path = location.path().to_path_buf().clone();
    if let Some(str_path) = raw_target.path {
        path.push(PathBuf::from(str_path.trim_start_matches("/")))
    }
    Target { path, location }
}
