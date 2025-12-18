#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax

// TODO: Fix clippy warnings for better code quality

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::AppConfig;

pub fn load_app_config_file(path: &Path) -> Option<AppConfig> {
    fs::read_to_string(path).ok().and_then(|s| {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
        {
            Some(ext) if ext == "yml" || ext == "yaml" => {
                serde_yaml::from_str::<AppConfig>(&s).ok()
            }
            Some(ext) if ext == "json" => serde_json::from_str::<AppConfig>(&s).ok(),
            _ => toml::from_str::<AppConfig>(&s).ok(),
        }
    })
}

#[allow(dead_code)]
pub fn load_global_config() -> Option<AppConfig> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".dx").join("config.toml"))
        .and_then(|p| load_app_config_file(&p))
}

#[allow(dead_code)]
pub fn load_local_config() -> Option<AppConfig> {
    let project_root = crate::exec::find_project_root();
    load_app_config_file(&project_root.join("config.toml"))
}

pub fn save_app_config(path: &Path, cfg: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let s = toml::to_string_pretty(cfg)?;
    fs::write(path, s)?;
    Ok(())
}

pub fn validate_app_config_file(path: &Path) -> Option<(Vec<String>, Vec<String>)> {
    let s = fs::read_to_string(path).ok()?;
    let parsed: Result<AppConfig, String> = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        Some(ext) if ext == "yml" || ext == "yaml" => {
            serde_yaml::from_str::<AppConfig>(&s).map_err(|e| e.to_string())
        }
        Some(ext) if ext == "json" => {
            serde_json::from_str::<AppConfig>(&s).map_err(|e| e.to_string())
        }
        _ => toml::from_str::<AppConfig>(&s).map_err(|e| e.to_string()),
    };
    match parsed {
        Ok(cfg) => Some(validate_app_config(&cfg)),
        Err(e) => Some((vec![e], Vec::new())),
    }
}

pub fn validate_app_config(cfg: &AppConfig) -> (Vec<String>, Vec<String>) {
    let errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Accept arbitrary theme names; only warn if theme_file has unexpected extension
    if let Some(path) = &cfg.theme_file {
        let ok_ext = path.to_ascii_lowercase().ends_with(".dx-theme");
        if !ok_ext {
            warnings.push(format!("theme_file '{path}' does not end with .dx-theme"));
        }
    }

    if let Some(tel) = &cfg.telemetry {
        if tel.enabled && tel.endpoint.as_deref().unwrap_or("").trim().is_empty() {
            warnings.push("telemetry.enabled=true but telemetry.endpoint is empty".to_string());
        }
    }

    if let Some(a) = &cfg.asciinema {
        let mode = a.stream_mode.to_ascii_lowercase();
        if mode != "remote" && mode != "local" {
            warnings.push(format!(
                "asciinema.stream_mode='{}' not in ['remote','local']",
                a.stream_mode
            ));
        }
        if a.stream && mode == "remote" && a.remote.as_deref().unwrap_or("").trim().is_empty() {
            warnings.push(
                "asciinema.stream=true with 'remote' mode but 'remote' id/url missing".to_string(),
            );
        }
    }

    if let Some(up) = &cfg.update {
        if up.on_start && up.build_cmd.trim().is_empty() {
            warnings.push("update.on_start=true but update.build_cmd is empty".to_string());
        }
    }

    (errors, warnings)
}

use crate::{AsciinemaConfig, TelemetryConfig, UpdateConfig, default_stream_mode};

#[derive(Debug, Clone)]
pub struct ConfigState {
    pub path: PathBuf,
    pub is_global: bool,
    pub cfg: AppConfig,
    pub message: Option<String>,
}

pub fn open_config_state() -> ConfigState {
    // Prefer YAML, then TOML
    let project_root = crate::exec::find_project_root();
    let local_yaml = project_root.join("config.yaml");
    let local_yml = project_root.join("config.yml");
    let local_toml = project_root.join("config.toml");
    let (path, is_global) = if local_yaml.exists() {
        (local_yaml, false)
    } else if local_yml.exists() {
        (local_yml, false)
    } else if local_toml.exists() {
        (local_toml, false)
    } else {
        let home = std::env::var("HOME").ok();
        let cfg_dir = home
            .map(|h| PathBuf::from(h).join(".dx"))
            .unwrap_or_else(|| PathBuf::from("~/.dx"));
        let gyaml = cfg_dir.join("config.yaml");
        let gyml = cfg_dir.join("config.yml");
        let gtoml = cfg_dir.join("config.toml");
        if gyaml.exists() {
            (gyaml, true)
        } else if gyml.exists() {
            (gyml, true)
        } else {
            (gtoml, true)
        }
    };
    let cfg = load_app_config_file(&path).unwrap_or(AppConfig {
        status: None,
        allow_project_override: true,
        motd_wrap: Some(true),
        motd_color: None,
        markdown_enabled: Some(true),
        output_dim: Some(true),
        theme: Some("dark".to_string()),
        theme_file: None,
        theme_overrides: None,
        theme_dir: None,
        telemetry: Some(TelemetryConfig {
            enabled: false,
            endpoint: None,
        }),
        update: Some(UpdateConfig {
            on_start: false,
            build_cmd: crate::default_build_cmd(),
            relaunch_path: None,
            preserve_args: true,
        }),
        asciinema: Some(AsciinemaConfig {
            enabled: false,
            external: false,
            on_relaunch: false,
            dir: None,
            file_prefix: Some("dx".to_string()),
            title: None,
            quiet: false,
            overwrite: false,
            stream: false,
            stream_mode: default_stream_mode(),
            local_addr: None,
            remote: None,
        }),
        show_fps: Some(true),
    });
    ConfigState {
        path,
        is_global,
        cfg,
        message: None,
    }
}

