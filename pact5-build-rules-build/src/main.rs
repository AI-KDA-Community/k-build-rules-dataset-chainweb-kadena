mod client;
mod config;
mod extract;
mod manifest;
mod store_rocks;
mod store_sqlite;
mod verify;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "pact5-build-rules-build", version)]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    Build  { #[arg(long, default_value = "pact5-build-rules.toml")] config: PathBuf },
    Verify { dir: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Build  { config } => cmd_build(&config),
        Cmd::Verify { dir }    => verify::verify_artifacts(&dir),
    }
}

fn cmd_build(config_path: &Path) -> Result<()> {
    let cfg = config::Config::load(config_path)?;
    let out = PathBuf::from(&cfg.output.dir);
    std::fs::create_dir_all(&out)?;

    let data = client::load_all(Path::new(&cfg.corpus.root))?;
    println!("[1] loaded: rules={} doc_metas={} policies={} plans={}",
             data.rules.len(), data.doc_metas.len(),
             data.policies.len(), data.plans.len());

    let mut records = extract::extract_all(&data);
    println!("[2] extracted {} records", records.len());

    records.sort_by(|a, b| {
        a.file_path.cmp(&b.file_path)
            .then_with(|| a.content_sha.cmp(&b.content_sha))
            .then_with(|| a.form_type.cmp(&b.form_type))
    });

    let commit = String::from_utf8(
        std::process::Command::new("git")
            .args(["-C", &cfg.corpus.root, "rev-parse", "HEAD"])
            .output()?.stdout
    )?.trim().to_string();
    println!("[3] source commit: {}", &commit[..commit.len().min(12)]);

    let sql_path = out.join(&cfg.output.sqlite_file);
    let n_sql = store_sqlite::write(&records, &sql_path, &commit)?;
    println!("[4] sqlite: {} rows", n_sql);

    let rocks_path = out.join(&cfg.output.rocks_dir);
    let n_rocks = store_rocks::write(&records, &rocks_path, &cfg.rocks)?;
    println!("[5] rocksdb: {} keys", n_rocks);

    let counts = extract::count_by_type(&records);
    let m = manifest::Manifest::build(&commit, &sql_path, &rocks_path, n_sql, n_rocks, counts)?;
    manifest::write(&m, &out.join(&cfg.output.manifest_file))?;
    println!("[6] manifest written");
    Ok(())
}
