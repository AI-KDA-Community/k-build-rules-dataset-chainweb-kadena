use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub corpus: CorpusCfg,
    pub output: Output,
    pub rocks: Rocks,
}

#[derive(Debug, Deserialize)]
pub struct CorpusCfg { pub root: String }

#[derive(Debug, Deserialize)]
pub struct Output {
    pub dir: String,
    pub sqlite_file: String,
    pub rocks_dir: String,
    pub manifest_file: String,
}

#[derive(Debug, Deserialize)]
pub struct Rocks {
    #[serde(default = "default_compression")] pub compression: String,
    #[serde(default = "default_bloom")] pub bloom_bits: f64,
    #[serde(default = "default_wb_mb")] pub write_buffer_mb: usize,
    #[serde(default = "default_max_wb")] pub max_write_buffers: i32,
}

fn default_compression() -> String { "zstd".into() }
fn default_bloom() -> f64 { 10.0 }
fn default_wb_mb() -> usize { 256 }
fn default_max_wb() -> i32 { 4 }

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&s).with_context(|| format!("parsing {}", path.display()))
    }
}
