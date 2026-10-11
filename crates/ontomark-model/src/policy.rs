//! Corpus-policy checks over the model. Policy findings are not logical
//! inconsistencies; the profile decides their severity.

use std::collections::BTreeMap;

use crate::iri::Iri;
use crate::model::{Disposition, Model, PolicyFinding, Resolution, StatementForm};
use crate::profile::CyclePolicy;

/// Strongly connected components of the asserted, supported `SubClassOf`
/// graph that contain a cycle (including a self-loop).
pub(crate) fn subclass_cycles(model: &Model, policy: CyclePolicy) -> Vec<PolicyFinding> {
    let Some(severity) = policy.severity() else {
        return Vec::new();
    };
    let iris: Vec<&Iri> = model.entities.keys().collect();
    let index: BTreeMap<&Iri, usize> = iris.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); iris.len()];
    let mut self_loop = vec![false; iris.len()];
    for (i, e) in model.entities.values().enumerate() {
        for s in &e.statements {
            if s.form != StatementForm::SubClassOf || s.disposition != Disposition::Supported {
                continue;
            }
            if let Some(Resolution::Resolved(t)) = s.object.resolution() {
                if let Some(&j) = index.get(t) {
                    adj[i].push(j);
                    if i == j {
                        self_loop[i] = true;
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for comp in tarjan(&adj) {
        if comp.len() > 1 || self_loop[comp[0]] {
            let mut members: Vec<Iri> = comp.iter().map(|&i| iris[i].clone()).collect();
            members.sort();
            out.push(PolicyFinding {
                code: "SUBCLASS_CYCLE".into(),
                severity,
                members,
            });
        }
    }
    out.sort_by(|a, b| a.members.cmp(&b.members));
    out
}

/// Iterative Tarjan SCC.
fn tarjan(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on = vec![false; n];
    let mut stack = Vec::new();
    let mut out = Vec::new();
    let mut next = 0;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&mut (v, ref mut ei)) = work.last_mut() {
            if *ei == 0 && index[v] == usize::MAX {
                index[v] = next;
                low[v] = next;
                next += 1;
                stack.push(v);
                on[v] = true;
            }
            if let Some(&w) = adj[v].get(*ei) {
                *ei += 1;
                if index[w] == usize::MAX {
                    work.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                out.push(comp);
            }
        }
    }
    out
}
