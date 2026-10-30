use crate::extract::Record;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = OFF;
PRAGMA page_size    = 4096;

CREATE TABLE IF NOT EXISTS forms (
    id              INTEGER PRIMARY KEY,
    content         TEXT    NOT NULL,
    module_name     TEXT,
    form_type       TEXT    NOT NULL,
    name            TEXT,
    function_name   TEXT,
    capability_name TEXT,
    pact_version    TEXT,
    source_repo     TEXT    NOT NULL,
    file_path       TEXT    NOT NULL,
    content_sha     TEXT    NOT NULL,
    semantic_sha    TEXT,
    task_tags       TEXT,
    UNIQUE(module_name, content_sha)
);
CREATE INDEX IF NOT EXISTS idx_forms_type ON forms(form_type);
CREATE INDEX IF NOT EXISTS idx_forms_name ON forms(name);

CREATE VIRTUAL TABLE IF NOT EXISTS forms_fts USING fts5(
    content, module_name, name,
    content='forms', content_rowid='id',
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

pub fn write(records: &[Record], path: &Path, commit: &str) -> Result<usize> {
    let mut con = Connection::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    con.execute_batch(SCHEMA)?;

    let tx = con.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO forms
             (content, module_name, form_type, name, function_name,
              capability_name, pact_version, source_repo, file_path,
              content_sha, semantic_sha, task_tags)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        )?;
        for r in records {
            let tags = serde_json::to_string(&r.task_tags)?;
            stmt.execute(params![
                r.content, r.module_name, r.form_type, r.name,
                r.function_name, r.capability_name, r.pact_version,
                r.source_repo, r.file_path, r.content_sha,
                r.semantic_sha, tags,
            ])?;
        }
    }
    tx.commit()?;

    con.execute("INSERT INTO forms_fts(forms_fts) VALUES('rebuild')", [])?;
    con.execute(
        "INSERT OR REPLACE INTO meta VALUES ('source_commit', ?1)",
        params![commit],
    )?;
    con.execute("INSERT OR REPLACE INTO meta VALUES ('schema_version', '2')", [])?;
    con.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")?;
    con.execute("PRAGMA optimize", [])?;
    con.execute_batch("VACUUM;")?;

    let n: i64 = con.query_row("SELECT COUNT(*) FROM forms", [], |r| r.get(0))?;
    Ok(n as usize)
}

pub fn verify(path: &Path, expected_rows: usize) -> Result<()> {
    let con = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let status: String = con.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    anyhow::ensure!(status == "ok", "integrity_check: {}", status);
    let n: i64 = con.query_row("SELECT COUNT(*) FROM forms", [], |r| r.get(0))?;
    anyhow::ensure!(n as usize == expected_rows, "row count mismatch: {} vs {}", n, expected_rows);
    Ok(())
}
