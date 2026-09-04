//! config.toml ↔ MachineConfig

use kiddos_kernel::{Lang, MachineConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FileConfig {
    pub lang: String,
    pub kid_name: String,
    pub crt: Option<bool>,
    pub font: String,
    pub hostname: String,
    /// Start windowed instead of fullscreen (for development).
    pub windowed: bool,
}

pub fn load(path: &Path) -> (MachineConfig, bool) {
    let fc: FileConfig = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    let d = MachineConfig::default();
    (
        MachineConfig {
            lang: Lang::from_code(&fc.lang).unwrap_or(d.lang),
            kid_name: fc.kid_name,
            crt: fc.crt.unwrap_or(d.crt),
            font: if fc.font.is_empty() { d.font } else { fc.font },
            hostname: if fc.hostname.is_empty() {
                d.hostname
            } else {
                fc.hostname
            },
        },
        fc.windowed,
    )
}

pub fn save(path: &Path, cfg: &MachineConfig) -> Result<(), String> {
    // keep `windowed` from the existing file
    let windowed = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<FileConfig>(&s).ok())
        .map(|f| f.windowed)
        .unwrap_or(false);
    let fc = FileConfig {
        lang: cfg.lang.code().to_string(),
        kid_name: cfg.kid_name.clone(),
        crt: Some(cfg.crt),
        font: cfg.font.clone(),
        hostname: cfg.hostname.clone(),
        windowed,
    };
    let text = toml::to_string_pretty(&fc).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}
