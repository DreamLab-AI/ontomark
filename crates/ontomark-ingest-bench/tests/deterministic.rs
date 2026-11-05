//! The deterministic arm is pinned: grounding alone accepts every labelled
//! fact, so its precision is the dataset's prevalence of supported facts.
//! A change here means the engine started (or stopped) refusing a grounded
//! fact; update the pin only with the reason in the commit.

use ontomark_ingest_bench::{breakdown, dataset, run_case, Oracle, Score};

#[test]
fn every_case_is_groundable_and_the_oracle_arm_is_pinned() {
    let data = dataset();
    let results: Vec<_> = data
        .cases
        .iter()
        .map(|c| run_case(&data.vocabulary, c, &Oracle, None))
        .collect();
    for r in &results {
        assert!(r.error.is_none(), "{}: {:?}", r.id, r.error);
        assert!(r.accepted, "{} was not grounded: fix the case", r.id);
        assert_eq!(r.extra_accepted, 0, "{}", r.id);
    }
    let s = Score::of(&results, |r| r.accepted);
    assert_eq!((s.tp, s.fp, s.r#fn, s.tn, s.errors), (19, 41, 0, 0, 0));
    let report = breakdown(&results, |r| r.accepted);
    assert_eq!(report["by_category"]["negated"]["false_acceptance"], 1.0);
}

#[test]
fn ids_are_unique_and_quotes_occur_in_their_notes() {
    let data = dataset();
    let mut ids = std::collections::BTreeSet::new();
    for c in &data.cases {
        assert!(ids.insert(c.id.clone()), "duplicate id {}", c.id);
        assert!(c.body.contains(&c.quote), "{}", c.id);
    }
}
