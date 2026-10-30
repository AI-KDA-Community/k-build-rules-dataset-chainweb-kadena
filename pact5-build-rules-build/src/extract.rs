use crate::client::{CorpusData, DocMeta, PolicyEntry, Rule};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub content: String,
    pub module_name: Option<String>,
    pub form_type: String,
    pub name: Option<String>,
    pub function_name: Option<String>,
    pub capability_name: Option<String>,
    pub pact_version: Option<String>,
    pub source_repo: String,
    pub file_path: String,
    pub content_sha: String,
    pub semantic_sha: Option<String>,
    pub task_tags: Vec<String>,
}

const SOURCE_REPO: &str = "AI-KDA-Community/build-rules";

pub fn extract_all(data: &CorpusData) -> Vec<Record> {
    let mut out = Vec::new();

    for r in &data.rules {
        let content = serde_json::to_string(r).unwrap();
        out.push(make_record(
            "rule",
            Some(&r.rule_id),
            Some(&r.doc_id),
            &format!("docs/build-rules/{}.md", r.doc_id),
            content,
        ));
    }

    for m in &data.doc_metas {
        let content = serde_json::to_string(m).unwrap();
        out.push(make_record(
            "doc_meta",
            Some(&m.doc_id),
            Some("data"),
            &format!("data/{}/meta.toml", m.doc_id),
            content,
        ));
    }

    if let Some(m) = &data.manifest {
        let content = serde_json::to_string(m).unwrap();
        out.push(make_record(
            "corpus_manifest",
            Some("MANIFEST"),
            Some("data"),
            "data/MANIFEST.toml",
            content,
        ));
    }

    for p in &data.policies {
        let content = serde_json::to_string(&p.raw).unwrap();
        let name = format!("{}#{}", p.policy_file, p.entry_id);
        out.push(make_record(
            "policy",
            Some(&name),
            Some(&p.policy_file),
            &format!("data/{}.toml", p.policy_file),
            content,
        ));
    }

    for plan in &data.plans {
        let id = plan.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let content = serde_json::to_string(plan).unwrap();
        out.push(make_record(
            "plan",
            Some(id),
            Some("plans"),
            &format!("plans/{}/plan.toml", id),
            content,
        ));
    }

    for (plan_id, item) in &data.plan_items {
        let item_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let name = format!("{}#{}", plan_id, item_id);
        let content = serde_json::to_string(item).unwrap();
        out.push(make_record(
            "plan_item",
            Some(&name),
            Some(plan_id),
            &format!("plans/{}/plan.toml", plan_id),
            content,
        ));
    }

    out
}

fn make_record(
    form_type: &str,
    name: Option<&str>,
    module_name: Option<&str>,
    file_path: &str,
    content: String,
) -> Record {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    let sha = format!("{:x}", h.finalize());
    Record {
        content,
        module_name: module_name.map(|s| s.to_string()),
        form_type: form_type.to_string(),
        name: name.map(|s| s.to_string()),
        function_name: None,
        capability_name: None,
        pact_version: None,
        source_repo: SOURCE_REPO.into(),
        file_path: file_path.to_string(),
        content_sha: sha,
        semantic_sha: None,
        task_tags: vec![],
    }
}

pub fn count_by_type(records: &[Record]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for r in records {
        *m.entry(r.form_type.clone()).or_insert(0) += 1;
    }
    m
}

pub fn _unused(_r: Rule, _d: DocMeta, _p: PolicyEntry) {}
