//! Reasoning that belongs to a generation of the source (EXP-06).
//!
//! A host (an Obsidian worker, an editor, a watcher) owns one [`Session`].
//! Every edit, deletion or rename that changes the axioms calls
//! [`Session::begin`], which numbers a new generation and cancels the job
//! running for the old one. Work runs in a [`Job`], in step-bounded slices, so
//! a single-threaded host can interleave reasoning with its message loop and
//! notice cancellation between slices. A finished job yields an [`Outcome`]
//! stamped with its generation; [`Session::accept`] takes it only if no newer
//! generation has begun. A result computed from old source therefore cannot
//! replace the result for new source, whether the old job was cancelled in
//! time or raced to completion.
//!
//! The guard is against staleness, not malice: an [`Outcome`]'s stamps are
//! plain data (it must cross a worker boundary as JSON), so code that edits
//! them, or runs a job on axioms other than the session's current source,
//! defeats it. The host is trusted to build each job from the source it
//! called [`Session::begin`] for.
//!
//! ```
//! use ontomark_reason::axiom::Axiom;
//! use ontomark_reason::session::{Job, Session, Step};
//! use ontomark_reason::Options;
//!
//! let mut session = Session::new();
//! let old = session.begin();
//! let mut stale_job = Job::new(old, &[Axiom::sub_class("urn:A", "urn:B")], Options::default());
//! // The source changes before the old job finishes.
//! let new = session.begin();
//! assert!(matches!(stale_job.step(1_000), Step::Stopped(_)));
//! let mut job = Job::new(new, &[Axiom::sub_class("urn:A", "urn:C")], Options::default());
//! let outcome = loop {
//!     match job.step(1_000) {
//!         Step::Pending { .. } => continue,
//!         Step::Done(outcome) => break outcome,
//!         Step::Stopped(why) => panic!("{why}"),
//!         Step::Spent => unreachable!(),
//!     }
//! };
//! assert!(session.accept(outcome).is_ok());
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use whelk_el::Progress;

use crate::axiom::Axiom;
use crate::classify::{Classification, Options, Prepared, Stopped};

/// Identifies one [`Session`] within the process, so an outcome cannot be
/// accepted by a session other than the one that issued its ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// A monotonically increasing source generation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct Generation(pub u64);

/// A shared cancellation flag. Cloning shares the flag; it is `Send` and
/// `Sync`, so a host can cancel from another thread.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A fresh, uncancelled token.
    #[must_use]
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Cancel every job holding this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// `true` once [`Self::cancel`] has been called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// The right to run one job for one generation. `Send`, so it can be handed
/// to wherever the job runs; not `Clone`, so one ticket yields one job. Clone
/// [`Ticket::cancel`] to cancel from elsewhere.
#[derive(Debug)]
pub struct Ticket {
    /// The session that issued the ticket.
    pub session: SessionId,
    /// The generation the job reasons for.
    pub generation: Generation,
    /// Cancelled when a newer generation begins.
    pub cancel: CancelToken,
}

/// A finished classification, stamped with the generation it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// The session whose ticket the job ran under.
    pub session: SessionId,
    /// The generation the input belonged to.
    pub generation: Generation,
    /// The result.
    pub classification: Classification,
}

/// Why [`Session::accept`] refused an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejected {
    /// A newer generation has begun since the outcome's job started.
    Stale {
        /// The outcome's generation.
        outcome: Generation,
        /// The session's current generation.
        current: Generation,
    },
    /// The outcome was produced under another session's ticket.
    ForeignSession {
        /// The session that issued the outcome's ticket.
        outcome: SessionId,
        /// This session.
        session: SessionId,
    },
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejected::Stale { outcome, current } => write!(
                f,
                "result for generation {} is stale: generation {} has begun",
                outcome.0, current.0
            ),
            Rejected::ForeignSession { outcome, session } => write!(
                f,
                "result belongs to session {}, not session {}",
                outcome.0, session.0
            ),
        }
    }
}

impl std::error::Error for Rejected {}

/// The host's view of the current source and its accepted classification.
#[derive(Debug)]
pub struct Session {
    id: SessionId,
    generation: Generation,
    live: Option<CancelToken>,
    current: Option<Outcome>,
}

impl Session {
    /// A session at generation 0 with no result and a process-unique id.
    #[must_use]
    pub fn new() -> Session {
        Session {
            id: SessionId(NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            generation: Generation::default(),
            live: None,
            current: None,
        }
    }

    /// This session's id.
    #[must_use]
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The source changed: begin a new generation, cancel the job for the
    /// previous one, and return the ticket for the new job.
    ///
    /// # Panics
    /// After `u64::MAX` generations, rather than reuse a generation number.
    pub fn begin(&mut self) -> Ticket {
        // A wrapped counter could make an old outcome current again; 2^64
        // generations cannot be reached in practice, so exhaustion is a panic
        // rather than a silent reuse, raised before any state changes.
        let next = Generation(
            self.generation
                .0
                .checked_add(1)
                .expect("generation counter exhausted"),
        );
        if let Some(previous) = self.live.take() {
            previous.cancel();
        }
        self.generation = next;
        let cancel = CancelToken::new();
        self.live = Some(cancel.clone());
        Ticket {
            session: self.id,
            generation: self.generation,
            cancel,
        }
    }

    /// The current generation.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Accept `outcome` as the current result if it belongs to the current
    /// generation.
    ///
    /// # Errors
    /// [`Rejected::ForeignSession`] when another session issued the outcome's
    /// ticket; [`Rejected::Stale`] when a newer generation has begun. The
    /// session's result is unchanged either way.
    pub fn accept(&mut self, outcome: Outcome) -> Result<&Classification, Rejected> {
        if outcome.session != self.id {
            return Err(Rejected::ForeignSession {
                outcome: outcome.session,
                session: self.id,
            });
        }
        if outcome.generation != self.generation {
            return Err(Rejected::Stale {
                outcome: outcome.generation,
                current: self.generation,
            });
        }
        self.live = None;
        Ok(&self.current.insert(outcome).classification)
    }

    /// The accepted result, if any. It may belong to an older generation than
    /// [`Self::generation`] while a newer job runs; compare the two to show
    /// staleness.
    #[must_use]
    pub fn current(&self) -> Option<&Outcome> {
        self.current.as_ref()
    }
}

impl Default for Session {
    fn default() -> Session {
        Session::new()
    }
}

/// What one [`Job::step`] achieved.
#[derive(Debug)]
pub enum Step {
    /// Work remains.
    Pending {
        /// Steps taken so far.
        steps: u64,
    },
    /// Finished.
    Done(Outcome),
    /// Cancelled or over budget. Every later call returns this again.
    Stopped(Stopped),
    /// The job already returned [`Step::Done`]; there is nothing left to run.
    Spent,
}

enum JobState {
    Running(Box<Prepared>),
    Stopped(Stopped),
    Finished,
}

/// One classification for one generation, run in slices.
///
/// Not `Send` (the saturation shares structure through `Rc`): build it where
/// it runs, from a [`Ticket`] and the axioms.
pub struct Job {
    ticket: Ticket,
    max_steps: Option<u64>,
    state: JobState,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("generation", &self.ticket.generation)
            .field("cancelled", &self.ticket.cancel.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl Job {
    /// Check and translate `axioms` and prepare the saturation.
    #[must_use]
    pub fn new(ticket: Ticket, axioms: &[Axiom], options: Options) -> Job {
        Job {
            ticket,
            max_steps: options.max_steps,
            state: JobState::Running(Box::new(Prepared::new(axioms, options))),
        }
    }

    /// The generation this job reasons for.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.ticket.generation
    }

    /// Run at most `budget` saturation steps, checking cancellation first.
    pub fn step(&mut self, budget: u64) -> Step {
        let prepared = match &mut self.state {
            JobState::Running(p) => p,
            JobState::Stopped(why) => return Step::Stopped(*why),
            JobState::Finished => return Step::Spent,
        };
        let steps = prepared.saturation.steps();
        if self.ticket.cancel.is_cancelled() {
            let why = Stopped::Cancelled { steps };
            self.state = JobState::Stopped(why);
            return Step::Stopped(why);
        }
        let budget = match self.max_steps {
            Some(max) if steps >= max && !prepared.saturation.is_done() => {
                let why = Stopped::OverBudget { steps };
                self.state = JobState::Stopped(why);
                return Step::Stopped(why);
            }
            Some(max) => budget.min(max - steps),
            None => budget,
        };
        match prepared.saturation.run(budget) {
            Progress::Pending => Step::Pending {
                steps: prepared.saturation.steps(),
            },
            Progress::Done => match std::mem::replace(&mut self.state, JobState::Finished) {
                JobState::Running(p) => Step::Done(Outcome {
                    session: self.ticket.session,
                    generation: self.ticket.generation,
                    classification: p.finish(),
                }),
                _ => unreachable!("state was Running"),
            },
        }
    }

    /// Run to the end in slices of `slice` steps.
    ///
    /// # Errors
    /// [`Stopped`] when cancelled or over budget.
    pub fn run(mut self, slice: u64) -> Result<Outcome, Stopped> {
        loop {
            match self.step(slice) {
                Step::Pending { .. } => {}
                Step::Done(outcome) => return Ok(outcome),
                Step::Stopped(why) => return Err(why),
                Step::Spent => unreachable!("run owns the job and returns on Done"),
            }
        }
    }
}
