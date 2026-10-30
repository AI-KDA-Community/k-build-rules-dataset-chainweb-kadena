use crate::config::Rocks;
use crate::extract::Record;
use anyhow::{Context, Result};
use rocksdb::{BlockBasedOptions, DBCompressionType, Options, WriteBatch, DB};
use std::path::Path;

pub fn write(records: &[Record], dir: &Path, cfg: &Rocks) -> Result<usize> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_compression_type(match cfg.compression.as_str() {
        "zstd" => DBCompressionType::Zstd,
        "snappy" => DBCompressionType::Snappy,
        "none" => DBCompressionType::None,
        _ => DBCompressionType::Zstd,
    });
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_bloom_filter(cfg.bloom_bits, false);
    opts.set_block_based_table_factory(&block_opts);
    opts.set_write_buffer_size(cfg.write_buffer_mb * 1024 * 1024);
    opts.set_max_write_buffer_number(cfg.max_write_buffers);

    let db = DB::open(&opts, dir)
        .with_context(|| format!("opening rocksdb at {}", dir.display()))?;

    let mut batch = WriteBatch::default();
    for r in records {
        let mod_ = r.module_name.as_deref().unwrap_or("_");
        let name = r.name.as_deref().unwrap_or("_");
        let sha16 = &r.content_sha[..16.min(r.content_sha.len())];

        batch.put(format!("c/{}", r.content_sha).as_bytes(), r.content.as_bytes());

        let body = rmp_serde::to_vec_named(r)?;
        batch.put(
            format!("f/{}/{}/{}/{}", r.form_type, mod_, name, sha16).as_bytes(),
            &body,
        );

        let idx = rmp_serde::to_vec_named(&serde_json::json!({
            "form_type": r.form_type,
            "name": r.name,
            "file_path": r.file_path,
        }))?;
        batch.put(format!("m/{}/{}", mod_, sha16).as_bytes(), &idx);
    }
    db.write(batch)?;
    db.flush()?;

    let n = db.property_int_value("rocksdb.estimate-num-keys")?.unwrap_or(0) as usize;
    drop(db);
    Ok(n)
}

pub fn verify(dir: &Path, sqlite_path: &Path, expected_keys: usize) -> Result<()> {
    let opts = Options::default();
    let db = DB::open_for_read_only(&opts, dir, false)
        .with_context(|| format!("opening rocksdb read-only at {}", dir.display()))?;

    let n = db.property_int_value("rocksdb.estimate-num-keys")?.unwrap_or(0) as usize;
    if expected_keys > 0 && n > 0 {
        anyhow::ensure!(
            (n as i64 - expected_keys as i64).abs() < 100,
            "key count sanity: {} vs manifest {}", n, expected_keys
        );
    }

    let con = rusqlite::Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let (sha, content): (String, String) = con.query_row(
        "SELECT content_sha, content FROM forms LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let blob = db.get(format!("c/{}", sha).as_bytes())?
        .with_context(|| format!("c/{} missing", sha))?;
    anyhow::ensure!(blob == content.as_bytes(), "content_sha cross-ref mismatch");
    Ok(())
}
