use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub rule_id: String,
    pub text: String,
    pub doc_id: String,
    pub section: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DocMeta {
    pub doc_id: String,
    pub title: String,
    #[serde(default)] pub meaning: String,
    #[serde(default)] pub keywords: Vec<String>,
    #[serde(default)] pub questions_answered: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyEntry {
    pub policy_file: String,
    pub entry_id: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusManifest {
    pub version: String,
    pub expected_docs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CorpusData {
    pub rules: Vec<Rule>,
    pub doc_metas: Vec<DocMeta>,
    pub policies: Vec<PolicyEntry>,
    pub manifest: Option<CorpusManifest>,
    pub plans: Vec<serde_json::Value>,
    pub plan_items: Vec<(String, serde_json::Value)>,
    pub file_hashes: BTreeMap<String, String>,
}

fn sha_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

pub fn load_all(corpus_root: &Path) -> Result<CorpusData> {
    let mut data = CorpusData::default();

    let rules_dir = corpus_root.join("docs/build-rules");
    let rule_re = regex::Regex::new(r"^\|\s*(R\d+\.\d+)\s*\|\s*(.+?)\s*\|\s*$")?;
    let section_re = regex::Regex::new(r"^##\s+(§[^\s—]+)")?;
    let doc_id_re = regex::Regex::new(r"Doc id:\*\*\s*(\S+)")?;

    if rules_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(&rules_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let text = std::fs::read_to_string(&path)?;
            let doc_id = doc_id_re.captures(&text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into());

            let mut current_section: Option<String> = None;
            for line in text.lines() {
                if let Some(cap) = section_re.captures(line) {
                    current_section = Some(cap[1].to_string());
                    continue;
                }
                if let Some(cap) = rule_re.captures(line) {
                    let rule_id = cap[1].to_string();
                    let rule_text = cap[2].to_string();
                    if rule_text.len() > 3 {
                        data.rules.push(Rule {
                            rule_id,
                            text: rule_text,
                            doc_id: doc_id.clone(),
                            section: current_section.clone(),
                        });
                    }
                }
            }

            let rel = format!("docs/build-rules/{}", path.file_name().unwrap().to_string_lossy());
            data.file_hashes.insert(rel, sha_hex(text.as_bytes()));
        }
    }

    let data_dir = corpus_root.join("data");
    if data_dir.exists() {
        let mut dirs: Vec<_> = std::fs::read_dir(&data_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let path = entry.path();
            let meta_path = path.join("meta.toml");
            if !meta_path.exists() { continue; }
            let text = std::fs::read_to_string(&meta_path)?;
            match toml::from_str::<DocMeta>(&text) {
                Ok(m) => data.doc_metas.push(m),
                Err(e) => eprintln!("[warn] {}: {}", meta_path.display(), e),
            }
            let rel = format!("data/{}/meta.toml", path.file_name().unwrap().to_string_lossy());
            data.file_hashes.insert(rel, sha_hex(text.as_bytes()));
        }
    }

    let manifest_path = data_dir.join("MANIFEST.toml");
    if manifest_path.exists() {
        let text = std::fs::read_to_string(&manifest_path)?;
        let val: toml::Value = toml::from_str(&text)?;
        let version = val.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let expected_docs: Vec<String> = val.get("expected_docs")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        data.manifest = Some(CorpusManifest { version, expected_docs });
        data.file_hashes.insert("data/MANIFEST.toml".into(), sha_hex(text.as_bytes()));
    }

    let policy_names = ["AVOID", "ACCESS", "DANGER", "FORKS", "HUMANS", "PREFER", "SCOPES"];
    for policy_name in &policy_names {
        let path = data_dir.join(format!("{}.toml", policy_name));
        if !path.exists() { continue; }
        let text = std::fs::read_to_string(&path)?;
        let val: toml::Value = toml::from_str(&text)?;
        if let Some(table) = val.as_table() {
            for (key, value) in table {
                if key == "meta" { continue; }
                if let Some(arr) = value.as_array() {
                    for item in arr {
                        let entry_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        let json = serde_json::to_value(item)?;
                        data.policies.push(PolicyEntry {
                            policy_file: policy_name.to_string(),
                            entry_id,
                            raw: json,
                        });
                    }
                }
            }
        }
        data.file_hashes.insert(format!("data/{}.toml", policy_name), sha_hex(text.as_bytes()));
    }

    let plans_dir = corpus_root.join("plans");
    if plans_dir.exists() {
        let mut dirs: Vec<_> = std::fs::read_dir(&plans_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let path = entry.path();
            let plan_path = path.join("plan.toml");
            if !plan_path.exists() { continue; }
            let text = std::fs::read_to_string(&plan_path)?;
            let val: toml::Value = toml::from_str(&text)?;
            let plan_obj = match val.get("plan") {
                Some(p) => p,
                None => continue,
            };
            let plan_id = plan_obj.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            data.plans.push(serde_json::to_value(plan_obj)?);
            for arr_key in ["donot", "avoid", "prefer", "intent"] {
                if let Some(arr) = plan_obj.get(arr_key).and_then(|v| v.as_array()) {
                    for item in arr {
                        data.plan_items.push((plan_id.clone(), serde_json::to_value(item)?));
                    }
                }
            }
            let rel = format!("plans/{}/plan.toml", plan_id);
            data.file_hashes.insert(rel, sha_hex(text.as_bytes()));
        }
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_real_corpus() {
        let root = PathBuf::from(
            "/run/media/notbob/storage/projects/k-build-rules-data-set/rules_build_rules_mob_rules"
        );
        let data = load_all(&root).unwrap();
        println!("rules:           {}", data.rules.len());
        println!("doc_metas:       {}", data.doc_metas.len());
        println!("policies:        {}", data.policies.len());
        println!("manifest:        {:?}", data.manifest.as_ref().map(|m| m.expected_docs.len()));
        println!("plans:           {}", data.plans.len());
        println!("plan_items:      {}", data.plan_items.len());
        println!("file_hashes:     {}", data.file_hashes.len());

        assert_eq!(data.doc_metas.len(), 34);
        assert!(data.rules.len() >= 3000, "got {}", data.rules.len());
        assert!(data.policies.len() >= 10, "got {}", data.policies.len());
        assert!(data.manifest.is_some());
        assert_eq!(data.manifest.as_ref().unwrap().expected_docs.len(), 34);
        assert!(!data.plans.is_empty());
        assert!(!data.plan_items.is_empty());
    }
}
