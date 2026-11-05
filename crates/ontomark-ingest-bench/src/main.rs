//! `ontomark-ingest-bench`: score the labelled cases and print a JSON report.
//!
//! ```text
//! ontomark-ingest-bench [--extractor oracle|model] [--jev] [--out FILE]
//!                       [--endpoint URL] [--model NAME]
//! ```
//!
//! * `--extractor oracle` (default) proposes each case's own fact: the
//!   deterministic engine alone. `model` asks the chat endpoint at
//!   `--endpoint` (an OpenAI-compatible `/chat/completions` URL; a bearer key
//!   is read from `ONTOMARK_BENCH_KEY` if set).
//! * `--jev` also asks the Jev judge, configured from the environment
//!   exactly as `ontomark ingest` is (`ONTOMARK_JEV=1`, endpoint resolution,
//!   local-only policy), and adds the selective-risk curve.

use std::process::ExitCode;

use ontomark_ingest_bench::{
    breakdown, dataset, risk_curve, run_case, CaseResult, ChatModel, Extractor, Oracle, SupportFn,
};
use ontomark_jev::{JevClient, JevConfig, JevFactJudge, UreqTransport};
use serde_json::json;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let data = dataset();
    let extractor: Box<dyn Extractor> = match arg(&args, "--extractor").as_deref() {
        None | Some("oracle") => Box::new(Oracle),
        Some("model") => {
            let Some(endpoint) = arg(&args, "--endpoint") else {
                eprintln!("--extractor model needs --endpoint");
                return ExitCode::from(2);
            };
            let url = if endpoint.ends_with("/chat/completions") {
                endpoint
            } else {
                format!("{}/chat/completions", endpoint.trim_end_matches('/'))
            };
            Box::new(ChatModel {
                url,
                model: arg(&args, "--model").unwrap_or_default(),
                bearer: std::env::var("ONTOMARK_BENCH_KEY").ok(),
            })
        }
        Some(other) => {
            eprintln!("unknown extractor {other}");
            return ExitCode::from(2);
        }
    };
    let jev = args.iter().any(|a| a == "--jev").then(|| {
        let env = |k: &str| std::env::var(k).ok();
        JevFactJudge(JevClient::new(JevConfig::from_env(&env), UreqTransport))
    });
    let support_fn = jev.as_ref().map(|j| {
        move |claims: &[ontomark_ingest::FactClaim]| j.support(claims).map_err(|u| u.reason)
    });
    let support: Option<&SupportFn<'_>> = support_fn.as_ref().map(|f| f as &SupportFn<'_>);

    let results: Vec<CaseResult> = data
        .cases
        .iter()
        .map(|case| {
            let r = run_case(&data.vocabulary, case, extractor.as_ref(), support);
            eprintln!(
                "{:<16} supported={:<5} accepted={:<5} p={:<6} {}",
                r.id,
                r.supported,
                r.accepted,
                r.support.map_or("-".into(), |p| format!("{p:.3}")),
                r.error.as_deref().unwrap_or("")
            );
            r
        })
        .collect();

    let arm = format!(
        "{}{}",
        extractor.name(),
        if jev.is_some() { "+jev" } else { "" }
    );
    let report = json!({
        "benchmark": "ontomark-ingest-bench",
        "dataset_version": data.version,
        "cases": results.len(),
        "positives": results.iter().filter(|r| r.supported).count(),
        "arm": arm,
        "endpoint": arg(&args, "--endpoint"),
        "model": arg(&args, "--model"),
        "accepted": breakdown(&results, |r| r.accepted),
        "extra_accepted": results.iter().map(|r| r.extra_accepted).sum::<usize>(),
        "risk_curve": jev.is_some().then(|| risk_curve(&results)),
        "results": results,
    });
    let text = serde_json::to_string_pretty(&report).expect("report serialises");
    match arg(&args, "--out") {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, text + "\n") {
                eprintln!("{path}: {e}");
                return ExitCode::from(1);
            }
        }
        None => println!("{text}"),
    }
    ExitCode::SUCCESS
}
