//! EXP-06: worker output belongs to the current source.
//!
//! Edit and delete during reasoning: the old job is cancelled at its next
//! slice, and an old result cannot replace the new generation's, whether it
//! was cancelled in time or raced to completion. Runs on the host and on
//! wasm32.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ontomark_reason::axiom::Axiom;
use ontomark_reason::session::{Generation, Job, Rejected, Session, Step};
use ontomark_reason::{Options, Stopped};

fn chain(n: usize, prefix: &str) -> Vec<Axiom> {
    (0..n)
        .map(|i| Axiom::sub_class(format!("urn:{prefix}{i}"), format!("urn:{prefix}{}", i + 1)))
        .collect()
}

fn entails(outcome: &ontomark_reason::Classification, a: &str, b: &str) -> bool {
    outcome
        .subsumptions
        .binary_search(&(a.to_owned(), b.to_owned()))
        .is_ok()
}

#[test]
fn an_edit_during_reasoning_cancels_the_old_job_at_its_next_slice() {
    let mut session = Session::new();
    let old_axioms = chain(300, "a");
    let mut old = Job::new(session.begin(), &old_axioms, Options::default());
    assert!(matches!(old.step(50), Step::Pending { .. }));
    // The page changes: a new generation begins.
    let new_axioms = chain(5, "b");
    let ticket = session.begin();
    match old.step(50) {
        Step::Stopped(Stopped::Cancelled { steps }) => assert_eq!(steps, 50),
        other => panic!("old job must stop: {other:?}"),
    }
    // Stopping is sticky.
    assert!(matches!(
        old.step(50),
        Step::Stopped(Stopped::Cancelled { .. })
    ));
    let outcome = Job::new(ticket, &new_axioms, Options::default())
        .run(64)
        .expect("new job finishes");
    assert_eq!(outcome.generation, Generation(2));
    let accepted = session.accept(outcome).expect("current generation");
    assert!(entails(accepted, "urn:b0", "urn:b5"));
}

#[test]
fn an_old_result_that_raced_to_completion_cannot_replace_the_new_one() {
    let mut session = Session::new();
    let first = session.begin();
    let old_axioms = chain(3, "a");
    // The old job finishes before it notices anything.
    let old_outcome = Job::new(first, &old_axioms, Options::default())
        .run(u64::MAX)
        .expect("finishes");
    let second = session.begin();
    let new_axioms = chain(3, "b");
    let new_outcome = Job::new(second, &new_axioms, Options::default())
        .run(u64::MAX)
        .expect("finishes");
    session.accept(new_outcome.clone()).expect("current");
    // The old outcome arrives late.
    assert_eq!(
        session.accept(old_outcome).unwrap_err(),
        Rejected::Stale {
            outcome: Generation(1),
            current: Generation(2)
        }
    );
    assert_eq!(session.current(), Some(&new_outcome));
}

#[test]
fn a_late_old_result_is_rejected_even_before_the_new_one_arrives() {
    let mut session = Session::new();
    let first = session.begin();
    let old_outcome = Job::new(first, &chain(3, "a"), Options::default())
        .run(u64::MAX)
        .expect("finishes");
    let _second = session.begin();
    assert!(session.accept(old_outcome).is_err());
    assert!(
        session.current().is_none(),
        "no result beats a stale result"
    );
}

#[test]
fn deleting_a_page_removes_its_class_from_the_accepted_result() {
    let mut session = Session::new();
    let mut axioms = chain(4, "a");
    axioms.push(Axiom::sub_class("urn:doomed", "urn:a0"));
    let before = Job::new(session.begin(), &axioms, Options::default())
        .run(u64::MAX)
        .expect("finishes");
    let accepted = session.accept(before).expect("current").clone();
    assert!(entails(&accepted, "urn:doomed", "urn:a4"));
    // The page for urn:doomed is deleted.
    axioms.pop();
    let after = Job::new(session.begin(), &axioms, Options::default())
        .run(u64::MAX)
        .expect("finishes");
    let accepted = session.accept(after).expect("current");
    assert!(accepted
        .subsumptions
        .iter()
        .all(|(a, b)| a != "urn:doomed" && b != "urn:doomed"));
    assert_eq!(session.current().map(|o| o.generation), Some(Generation(2)));
}

#[test]
fn a_finished_job_is_spent() {
    let mut session = Session::new();
    let mut job = Job::new(session.begin(), &chain(2, "a"), Options::default());
    assert!(matches!(job.step(u64::MAX), Step::Done(_)));
    assert!(matches!(job.step(1), Step::Spent));
}

#[test]
fn a_cancel_from_the_host_stops_the_job() {
    let mut session = Session::new();
    let ticket = session.begin();
    let cancel = ticket.cancel.clone();
    let mut job = Job::new(ticket, &chain(200, "a"), Options::default());
    assert!(matches!(job.step(10), Step::Pending { .. }));
    cancel.cancel();
    assert!(matches!(
        job.step(10),
        Step::Stopped(Stopped::Cancelled { steps: 10 })
    ));
}

#[test]
fn a_step_budget_stops_the_job_instead_of_running_on() {
    let mut session = Session::new();
    let options = Options {
        max_steps: Some(25),
        ..Options::default()
    };
    let result = Job::new(session.begin(), &chain(200, "a"), options).run(10);
    assert_eq!(result, Err(Stopped::OverBudget { steps: 25 }));
    assert_eq!(
        ontomark_reason::classify_with(&chain(200, "a"), options),
        Err(Stopped::OverBudget { steps: 25 })
    );
}

#[test]
fn sliced_jobs_equal_one_shot_classification() {
    let axioms: Vec<Axiom> = chain(40, "a").into_iter().chain(chain(40, "b")).collect();
    let whole = ontomark_reason::classify(&axioms);
    for slice in [1, 3, 1000] {
        let mut session = Session::new();
        let outcome = Job::new(session.begin(), &axioms, Options::default())
            .run(slice)
            .expect("finishes");
        let mut a = outcome.classification;
        let mut b = whole.clone();
        a.steps = 0;
        b.steps = 0;
        assert_eq!(a, b, "slice {slice}");
    }
}

/// Cross-vendor audit counterexample: an outcome from another session with
/// the same generation number must not be accepted.
#[test]
fn an_outcome_from_another_session_is_rejected() {
    let mut first = Session::new();
    let old = Job::new(first.begin(), &chain(2, "a"), Options::default())
        .run(u64::MAX)
        .expect("finishes");
    let mut second = Session::new();
    let _ticket = second.begin();
    assert_eq!(old.generation, second.generation());
    assert!(matches!(
        second.accept(old),
        Err(Rejected::ForeignSession { .. })
    ));
    assert!(second.current().is_none());
}

#[test]
fn a_zero_step_budget_allows_an_already_finished_empty_job() {
    let options = Options {
        max_steps: Some(0),
        ..Options::default()
    };
    let synchronous = ontomark_reason::classify_with(&[], options).expect("no work needed");
    let mut session = Session::new();
    let outcome = Job::new(session.begin(), &[], options)
        .run(1)
        .expect("no work needed");
    assert_eq!(outcome.classification, synchronous);
    assert_eq!(
        Job::new(session.begin(), &chain(1, "a"), options).run(1),
        Err(Stopped::OverBudget { steps: 0 })
    );
}
