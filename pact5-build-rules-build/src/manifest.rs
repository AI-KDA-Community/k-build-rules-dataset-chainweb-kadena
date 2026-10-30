use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub corpus: String,
    pub dataset_family: String,
    pub dataset_version: String,
    pub source_repo: String,
    pub source_commit: String,
    pub built_at: String,
    pub builder: Builder,
    pub sqlite: Sqlite,
    pub rocksdb: RocksDb,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Builder {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sqlite {
    pub file: String,
    pub schema_version: u32,
    pub rows: usize,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RocksDb {
    pub dir: String,
    pub keys: usize,
    pub key_prefixes: Vec<String>,
}

pub fn file_sha256(p: &Path) -> Result<String> {
    let mut h = Sha256::new();
    h.update(fs::read(p).with_context(|| format!("reading {}", p.display()))?);
    Ok(format!("{:x}", h.finalize()))
}

pub fn now_iso() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

impl Manifest {
    pub fn build(
        commit: &str,
        sqlite_path: &Path,
        _rocks_path: &Path,
        rows: usize,
        keys: usize,
        counts: BTreeMap<String, usize>,
    ) -> Result<Self> {
        Ok(Manifest {
            corpus: "pact5".into(),
            dataset_family: "build-rules".into(),
            dataset_version: "0.1.0".into(),
            source_repo: "AI-KDA-Community/build-rules".into(),
            source_commit: commit.into(),
            built_at: now_iso(),
            builder: Builder {
                name: "pact5-build-rules-build".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            sqlite: Sqlite {
                file: sqlite_path.file_name().unwrap().to_string_lossy().into(),
                schema_version: 2,
                rows,
                sha256: file_sha256(sqlite_path)?,
            },
            rocksdb: RocksDb {
                dir: "pact5-build-rules.rocks".into(),
                keys,
                key_prefixes: vec!["c/".into(), "f/".into(), "m/".into()],
            },
            counts,
        })
    }
}

pub fn write(m: &Manifest, path: &Path) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(m)?)?;
    Ok(())
}

pub fn read(path: &Path) -> Result<Manifest> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
