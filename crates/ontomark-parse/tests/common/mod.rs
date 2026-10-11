//! Shared checks for the EXP-03 suites.
#![allow(dead_code, missing_docs)]

use ontomark_parse::{Document, FrontmatterState, Patch, Span, Value};
use yaml_rust2::{Yaml, YamlLoader};

/// Convert a yaml-rust2 value to ours, the independent reading for the
/// differential check.
pub fn from_yaml(y: &Yaml) -> Value {
    match y {
        Yaml::Real(r) => Value::Float(r.clone()),
        Yaml::Integer(i) => Value::Int(*i),
        Yaml::String(s) if s == "Null" || s == "NULL" => Value::Null,
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Array(a) => Value::Seq(a.iter().map(from_yaml).collect()),
        Yaml::Hash(h) => Value::Map(h.iter().map(|(k, v)| (key_text(k), from_yaml(v))).collect()),
        Yaml::Null => Value::Null,
        other => panic!("unexpected yaml {other:?}"),
    }
}

fn key_text(k: &Yaml) -> String {
    match k {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Null => "null".into(),
        other => panic!("complex key {other:?}"),
    }
}

/// Every lossless invariant that holds for a parsed page, as a list of
/// failures (empty = pass).
pub fn check_lossless(src: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let doc = Document::parse(src);
    if doc.source() != src || doc.to_string() != src {
        fails.push("identity round-trip differs".into());
    }
    let Some(fm) = doc.frontmatter() else {
        return fails;
    };
    if fm.state != FrontmatterState::Parsed {
        fails.push(format!("state {:?}", fm.state));
        return fails;
    }
    // Entries and gaps tile the content exactly.
    let mut spans: Vec<Span> = fm
        .entries
        .iter()
        .map(|e| e.span)
        .chain(fm.gaps.iter().copied())
        .collect();
    spans.sort();
    let mut at = fm.content.start;
    for s in &spans {
        if s.start != at {
            fails.push(format!("tiling gap at {at}"));
            break;
        }
        at = s.end;
    }
    if at != fm.content.end {
        fails.push("tiling does not reach the closing fence".into());
    }
    // Differential: our per-key values == an independent whole-block load.
    match YamlLoader::load_from_str(fm.content.slice(src)) {
        Ok(docs) => {
            let theirs = docs
                .first()
                .map_or(Value::Map(Default::default()), from_yaml);
            let ours = Value::Map(
                fm.entries
                    .iter()
                    .map(|e| (e.key.clone(), e.value.clone()))
                    .collect(),
            );
            if theirs != ours {
                fails.push("value differs from yaml-rust2 whole-block load".into());
            }
        }
        Err(e) => fails.push(format!("independent load failed: {e}")),
    }
    fails
}

/// Apply `patch`, then check independently (not trusting the receipt) that
/// every entry not in `touched` kept its exact bytes and that the body is
/// unchanged unless `body_touched`.
pub fn check_patch(
    doc: &Document,
    patch: &Patch,
    touched: &[&str],
    body_touched: bool,
) -> Result<Document, String> {
    let out = doc.apply(patch).map_err(|e| e.to_string())?;
    let new = out.document;
    let (os, ns) = (doc.source(), new.source());
    let ofm = doc.frontmatter();
    let nfm = new.frontmatter().ok_or("frontmatter vanished")?;
    if let Some(ofm) = ofm {
        for e in &ofm.entries {
            if touched.contains(&e.key.as_str()) {
                continue;
            }
            let ne = nfm
                .get(&e.key)
                .ok_or_else(|| format!("{} vanished", e.key))?;
            if ne.raw(ns) != e.raw(os) {
                return Err(format!("{} bytes changed", e.key));
            }
        }
        let gaps = |d: &Document, s: &str| -> Vec<String> {
            d.frontmatter()
                .unwrap()
                .gaps
                .iter()
                .map(|g| g.slice(s).to_owned())
                .collect()
        };
        if gaps(doc, os) != gaps(&new, ns) {
            return Err("comments/blank lines changed".into());
        }
    }
    if !body_touched && doc.body() != new.body() {
        return Err("body changed".into());
    }
    let after = check_lossless(ns);
    if !after.is_empty() {
        return Err(format!("result not lossless: {after:?}"));
    }
    Ok(new)
}
