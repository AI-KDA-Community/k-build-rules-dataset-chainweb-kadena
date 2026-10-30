use crate::{manifest, store_rocks, store_sqlite};
use anyhow::{Context, Result};
use std::path::Path;

pub fn verify_artifacts(dir: &Path) -> Result<()> {
    let mut manifests: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("opening {}", dir.display()))?
    {
        let e = entry?;
        let p = e.path();
        if p.is_file() {
            if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                if n.ends_with(".manifest.json") {
                    manifests.push(p);
                }
            }
        }
    }
    anyhow::ensure!(
        manifests.len() == 1,
        "expected exactly one *.manifest.json in {}, found {}: {:?}",
        dir.display(), manifests.len(), manifests
    );
    let man_path = manifests.into_iter().next().unwrap();
    let m = manifest::read(&man_path)?;
    println!("verifying manifest at {}", man_path.display());

    let sql_path = dir.join(&m.sqlite.file);
    store_sqlite::verify(&sql_path, m.sqlite.rows)?;
    println!("[ok] sqlite integrity + row count");

    let sha = manifest::file_sha256(&sql_path)?;
    anyhow::ensure!(sha == m.sqlite.sha256, "sqlite sha mismatch");
    println!("[ok] sqlite sha256 matches manifest");

    let rocks_path = dir.join(&m.rocksdb.dir);
    store_rocks::verify(&rocks_path, &sql_path, m.rocksdb.keys)?;
    println!("[ok] rocksdb opens read-only, key count ok");

    Ok(())
}
