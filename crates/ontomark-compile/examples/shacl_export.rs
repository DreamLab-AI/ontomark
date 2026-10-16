//! Write the SHACL shapes and the frontmatter data graph of a repository as
//! Turtle and JSON-LD, for an independent engine to validate.
//!
//! ```text
//! cargo run -p ontomark-compile --example shacl_export -- \
//!     --repo conformance/fixtures/estate-small \
//!     --profile profiles/estate/profile.yaml \
//!     --out /tmp/shacl-out
//! ```
//!
//! For each vault (`knowledge`, `working`) that exists under `--repo` it writes
//! `<out>/<vault>.data.ttl` and `<out>/<vault>.data.jsonld`; it also writes
//! `<out>/shapes.ttl` and `<out>/shapes.jsonld`, and, when the knowledge vault
//! passes the publication projection, the asserted ontology as
//! `<out>/ontology.ttl` and `<out>/ontology.jsonld`. Nothing is written into
//! the repository.

use std::path::PathBuf;

use ontomark_compile::build::{okf, turtle};
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::ir::{model::Corpus, projection};
use ontomark_compile::jsonld::to_jsonld_string;
use ontomark_compile::profile::Profile;
use ontomark_compile::shacl::{frontmatter_graph, shapes};

fn main() -> anyhow::Result<()> {
    let mut repo = None;
    let mut profile_path = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--repo" => &mut repo,
            "--profile" => &mut profile_path,
            "--out" => &mut out,
            other => anyhow::bail!("unknown argument {other}"),
        };
        *slot = Some(PathBuf::from(
            args.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))?,
        ));
    }
    let repo = repo.ok_or_else(|| anyhow::anyhow!("--repo is required"))?;
    let out = out.ok_or_else(|| anyhow::anyhow!("--out is required"))?;
    let profile = match profile_path {
        Some(path) => Profile::from_yaml(&std::fs::read_to_string(path)?)?,
        None => Profile::generic(),
    };
    let vocab = Vocabulary::load(repo.join("ontology/vocabulary.yaml"))?;
    std::fs::create_dir_all(&out)?;

    let context = okf::context(&profile, &vocab);
    let shape_graph = shapes(&profile, &vocab);
    std::fs::write(
        out.join("shapes.ttl"),
        turtle::serialise(&profile, &shape_graph),
    )?;
    std::fs::write(
        out.join("shapes.jsonld"),
        to_jsonld_string(&shape_graph, &context),
    )?;

    for (name, kind) in [
        ("knowledge", VaultKind::Knowledge),
        ("working", VaultKind::Working),
    ] {
        let root = repo.join(name);
        if !root.is_dir() {
            continue;
        }
        let vault = Vault::load(&root, kind)?;
        if kind == VaultKind::Knowledge {
            let raw = Corpus::build(&vault, &vocab);
            match projection::project(&profile, &raw, &Default::default()) {
                Ok(corpus) => {
                    let roster = ontomark_compile::build::attribution::Roster::discover(&repo);
                    let ontology =
                        turtle::build_graph_attributed(&profile, &corpus, &vocab, true, &roster);
                    eprintln!("ontology: {} triples", ontology.len());
                    std::fs::write(
                        out.join("ontology.ttl"),
                        turtle::serialise(&profile, &ontology),
                    )?;
                    std::fs::write(
                        out.join("ontology.jsonld"),
                        to_jsonld_string(&ontology, &context),
                    )?;
                }
                Err(blocked) => eprintln!("no ontology written: {blocked}"),
            }
        }
        let graph = frontmatter_graph(&vault, name, &profile, &vocab);
        eprintln!(
            "{name}: {} pages, {} triples",
            vault.pages.len(),
            graph.len()
        );
        std::fs::write(
            out.join(format!("{name}.data.ttl")),
            turtle::serialise(&profile, &graph),
        )?;
        std::fs::write(
            out.join(format!("{name}.data.jsonld")),
            to_jsonld_string(&graph, &context),
        )?;
    }
    Ok(())
}
