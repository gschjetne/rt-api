//! The differ: turns a revised document into add, revise and withdraw operations against
//! the decisions a session already has.

use crate::doc::{DecisionContent, Document};

/// A decision as the session knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveDecision {
    pub id: String,
    pub number: u32,
    pub version: u32,
    pub content: DecisionContent,
    pub withdrawn: bool,
}

/// What a revised document does to the session. Numbers for new decisions are assigned by
/// the session when it applies the operations, in the order given.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add {
        id: String,
        content: DecisionContent,
    },
    Revise {
        id: String,
        number: u32,
        content: DecisionContent,
        fields: Vec<&'static str>,
        reinstated: bool,
    },
    Withdraw {
        id: String,
        number: u32,
        reason: String,
    },
}

/// Reason recorded when a revised document simply leaves a decision out.
pub const REMOVED_REASON: &str = "removed from the document";

pub fn diff(live: &[LiveDecision], doc: &Document) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    for spec in &doc.decisions {
        let existing = live.iter().find(|d| d.id == spec.id);
        match (&spec.withdrawn, existing) {
            (None, None) => ops.push(Op::Add {
                id: spec.id.clone(),
                content: spec.content.clone(),
            }),
            (None, Some(d)) => {
                let fields = d.content.changed_fields(&spec.content);
                if d.withdrawn || !fields.is_empty() {
                    ops.push(Op::Revise {
                        id: spec.id.clone(),
                        number: d.number,
                        content: spec.content.clone(),
                        fields,
                        reinstated: d.withdrawn,
                    });
                }
            }
            (Some(reason), Some(d)) => {
                if !d.withdrawn {
                    let reason = if reason.trim().is_empty() {
                        "withdrawn"
                    } else {
                        reason.trim()
                    };
                    ops.push(Op::Withdraw {
                        id: spec.id.clone(),
                        number: d.number,
                        reason: reason.to_owned(),
                    });
                }
            }
            (Some(_), None) => {
                return Err(format!(
                    "line {}: decision `{}` is marked withdrawn but is not in this session",
                    spec.line, spec.id
                ));
            }
        }
    }
    for d in live {
        if !d.withdrawn && !doc.decisions.iter().any(|s| s.id == d.id) {
            ops.push(Op::Withdraw {
                id: d.id.clone(),
                number: d.number,
                reason: REMOVED_REASON.to_owned(),
            });
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

    fn live(doc: &str) -> Vec<LiveDecision> {
        parse(doc)
            .unwrap()
            .decisions
            .into_iter()
            .enumerate()
            .map(|(i, d)| LiveDecision {
                id: d.id,
                number: i as u32 + 1,
                version: 1,
                content: d.content,
                withdrawn: d.withdrawn.is_some(),
            })
            .collect()
    }

    const V1: &str = "## Decision: Strategy\n```decision\nid: strategy\nkind: choice\noptions:\n  - last-writer-wins\n  - per-field merge\nrecommend: per-field merge\n```\nWhy it matters.\n\n## Decision: Ship?\n```decision\nid: ship\nkind: go-nogo\n```\n\n## Decision: Name\n```decision\nid: name\nkind: text\n```\n";

    #[test]
    fn unchanged_document_has_no_ops() {
        assert_eq!(diff(&live(V1), &parse(V1).unwrap()).unwrap(), []);
    }

    #[test]
    fn opening_adds_everything_in_order() {
        let ops = diff(&[], &parse(V1).unwrap()).unwrap();
        let ids: Vec<_> = ops
            .iter()
            .map(|op| match op {
                Op::Add { id, .. } => id.as_str(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(ids, ["strategy", "ship", "name"]);
    }

    #[test]
    fn revise_add_and_withdraw() {
        // The plan's scenario: an answer in chat changes one question, makes another
        // moot, and raises a new one.
        let v2 = "## Decision: Strategy\n```decision\nid: strategy\nkind: choice\noptions:\n  - last-writer-wins\n  - per-field merge\n  - CRDT\nrecommend: CRDT\n```\nWhy it matters, revised.\n\n## Decision: Ship?\n```decision\nid: ship\nkind: go-nogo\nwithdrawn: depends on the CRDT spike\n```\n\n## Decision: CRDT library\n```decision\nid: crdt-lib\nkind: choice\noptions: [automerge, yrs]\n```\n";
        let ops = diff(&live(V1), &parse(v2).unwrap()).unwrap();
        assert_eq!(ops.len(), 4, "{ops:#?}");
        match &ops[0] {
            Op::Revise {
                id,
                number,
                fields,
                reinstated,
                ..
            } => {
                assert_eq!((id.as_str(), *number, *reinstated), ("strategy", 1, false));
                assert_eq!(fields, &["options", "recommendation", "text"]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            ops[1],
            Op::Withdraw {
                id: "ship".into(),
                number: 2,
                reason: "depends on the CRDT spike".into()
            }
        );
        assert!(matches!(&ops[2], Op::Add { id, .. } if id == "crdt-lib"));
        assert_eq!(
            ops[3],
            Op::Withdraw {
                id: "name".into(),
                number: 3,
                reason: REMOVED_REASON.into()
            }
        );
    }

    #[test]
    fn reappearing_decision_is_reinstated() {
        let mut current = live(V1);
        current[1].withdrawn = true;
        let ops = diff(&current, &parse(V1).unwrap()).unwrap();
        assert!(
            matches!(&ops[..], [Op::Revise { number: 2, reinstated: true, fields, .. }] if fields.is_empty())
        );
    }

    #[test]
    fn already_withdrawn_stays_quiet() {
        let mut current = live(V1);
        current[2].withdrawn = true;
        let doc = V1.replace(
            "id: name\nkind: text\n",
            "id: name\nkind: text\nwithdrawn: no longer needed\n",
        );
        assert_eq!(diff(&current, &parse(&doc).unwrap()).unwrap(), []);
        let without = &V1[..V1.find("## Decision: Name").unwrap()];
        assert_eq!(diff(&current, &parse(without).unwrap()).unwrap(), []);
    }

    #[test]
    fn withdrawing_unknown_decision_is_an_error() {
        let doc = "## X\n```decision\nid: x\nkind: go-nogo\nwithdrawn: never mind\n```\n";
        assert!(
            diff(&[], &parse(doc).unwrap())
                .unwrap_err()
                .contains("not in this session")
        );
    }
}
