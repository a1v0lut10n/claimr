// SPDX-License-Identifier: Apache-2.0

//! CLM-0010: machine-readable evaluation output — one JSON document
//! per query on stdout (NDJSON), structured diagnostics on stderr.
//! Hand-rolled writer rather than a serde dependency: the shapes are
//! three fixed objects over strings, and the dependency tree stays
//! lean. The format is documented in `docs/reference/json-output.md`
//! and pinned by `tests/json_output.rs`; consumers dispatch on
//! `outcome` without parsing prose.
//!
//! Shapes:
//! - per query: `{"query": "?- ….", "outcome": "solutions" | "none",
//!   "answers": [{"equations": […], "disequations": […],
//!   "constraints": […]}, …]}` — a proof with no bindings is one
//!   answer with three empty lists (the human mode's `true`).
//! - a query that fails at runtime: `{"query": "?- ….",
//!   "outcome": "error", "error": {"message": "…"}}`.
//! - a file that never runs (read, parse, compile): on stderr,
//!   `{"error": {"file": "…", "line": N?, "column": N?,
//!   "message": "…"}}` — line/column only when known.

use claimr::{Answer, FailureTree, ProofNode, Reason};

/// JSON string escaping per RFC 8259: `"`, `\`, and control
/// characters; the common controls get their short forms.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn string_array(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| format!("\"{}\"", escape(s))).collect();
    format!("[{}]", inner.join(","))
}

fn answer_object(answer: &Answer) -> String {
    let mut out = format!(
        "{{\"equations\":{},\"disequations\":{},\"constraints\":{}",
        string_array(&answer.equations),
        string_array(&answer.disequations),
        string_array(&answer.constraints),
    );
    // CLM-0023: present only when explanations were asked for, so a
    // document produced without `--explain` is unchanged.
    if let Some(roots) = &answer.explanation {
        out.push_str(",\"explanation\":");
        out.push_str(&proof_array(roots));
    }
    out.push('}');
    out
}

/// A proof tree as nested objects: `{"goal": …, "by": …, "line"?, "column"?,
/// "clause"?, "subgoals": […]}`. Iterative — a derivation can be deep.
fn proof_array(roots: &[ProofNode]) -> String {
    // A frame per open node: the node and its children rendered so far.
    struct Open<'a> {
        node: &'a ProofNode,
        next_child: usize,
        children: Vec<String>,
    }
    let mut rendered_roots: Vec<String> = Vec::new();
    let mut stack: Vec<Open<'_>> = Vec::new();
    let mut pending_roots = roots.iter();
    loop {
        // Descend into the next child of the innermost open node, or open
        // the next root.
        let next = match stack.last_mut() {
            Some(open) => {
                let child = open.node.subgoals.get(open.next_child);
                open.next_child += 1;
                child
            }
            None => pending_roots.next(),
        };
        match next {
            Some(node) => stack.push(Open {
                node,
                next_child: 0,
                children: Vec::new(),
            }),
            None => {
                let Some(done) = stack.pop() else {
                    break;
                };
                let text = proof_object(done.node, &done.children);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(text),
                    None => rendered_roots.push(text),
                }
            }
        }
    }
    format!("[{}]", rendered_roots.join(","))
}

fn proof_object(node: &ProofNode, children: &[String]) -> String {
    let mut fields = vec![
        format!("\"goal\":\"{}\"", escape(&node.goal)),
        format!("\"by\":\"{}\"", node.by.as_str()),
    ];
    // CLM-0031: a failed node's reason first, its detail after the citation.
    if let Some(failure) = &node.failure {
        fields.push(format!("\"reason\":\"{}\"", failure.reason.as_str()));
    }
    if let Some(line) = node.line {
        fields.push(format!("\"line\":{line}"));
    }
    if let Some(column) = node.column {
        fields.push(format!("\"column\":{column}"));
    }
    if let Some(clause) = &node.clause {
        fields.push(format!("\"clause\":\"{}\"", escape(clause)));
    }
    if let Some(failure) = &node.failure {
        if failure.reason == Reason::Unsatisfiable {
            fields.push(format!("\"conflict\":{}", proof_array(&failure.conflict)));
        }
        if let Some(note) = &failure.note {
            fields.push(format!("\"note\":\"{}\"", escape(note)));
        }
        if let Some(n) = failure.clauses {
            fields.push(format!("\"clauses\":{n}"));
        }
        if let Some(n) = failure.candidates {
            fields.push(format!("\"candidates\":{n}"));
        }
        if let Some(n) = failure.answers {
            fields.push(format!("\"answers\":{n}"));
        }
    }
    fields.push(format!("\"subgoals\":[{}]", children.join(",")));
    format!("{{{}}}", fields.join(","))
}

/// The per-query document: `outcome` is `"solutions"` when any answer
/// exists, `"none"` otherwise. CLM-0031: with a failure tree (an
/// outcome-none query under `--explain`) the document gains `failure`,
/// `{"branches": [[node, …], …], "omitted": N}`; without one it is unchanged.
pub fn query_document(
    query_text: &str,
    answers: &[Answer],
    failure: Option<&FailureTree>,
) -> String {
    let outcome = if answers.is_empty() {
        "none"
    } else {
        "solutions"
    };
    let rendered: Vec<String> = answers.iter().map(answer_object).collect();
    let mut out = format!(
        "{{\"query\":\"{}\",\"outcome\":\"{outcome}\",\"answers\":[{}]",
        escape(query_text),
        rendered.join(","),
    );
    if let Some(tree) = failure {
        let branches: Vec<String> = tree.branches.iter().map(|b| proof_array(b)).collect();
        out.push_str(&format!(
            ",\"failure\":{{\"branches\":[{}],\"omitted\":{}}}",
            branches.join(","),
            tree.omitted
        ));
    }
    out.push('}');
    out
}

/// The runtime-failure document for one query.
pub fn query_error_document(query_text: &str, message: &str) -> String {
    format!(
        "{{\"query\":\"{}\",\"outcome\":\"error\",\"error\":{{\"message\":\"{}\"}}}}",
        escape(query_text),
        escape(message),
    )
}

/// The file-level diagnostic (read, parse, or compile failure), for
/// stderr. `line`/`column` appear only when known.
pub fn error_document(
    file: &str,
    line: Option<usize>,
    column: Option<usize>,
    message: &str,
) -> String {
    let mut fields = vec![format!("\"file\":\"{}\"", escape(file))];
    if let Some(line) = line {
        fields.push(format!("\"line\":{line}"));
    }
    if let Some(column) = column {
        fields.push(format!("\"column\":{column}"));
    }
    fields.push(format!("\"message\":\"{}\"", escape(message)));
    format!("{{\"error\":{{{}}}}}", fields.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_covers_quotes_backslashes_and_controls() {
        assert_eq!(escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape("a\nb\tc\r"), "a\\nb\\tc\\r");
        assert_eq!(escape("\u{01}"), "\\u0001");
        assert_eq!(escape("père = 37/2"), "père = 37/2");
    }

    #[test]
    fn documents_take_their_documented_shapes() {
        let truth = Answer {
            equations: vec![],
            disequations: vec![],
            constraints: vec![],
            explanation: None,
        };
        assert_eq!(
            query_document("?- ok(s1).", &[truth], None),
            r#"{"query":"?- ok(s1).","outcome":"solutions","answers":[{"equations":[],"disequations":[],"constraints":[]}]}"#
        );
        assert_eq!(
            query_document("?- ok(s2).", &[], None),
            r#"{"query":"?- ok(s2).","outcome":"none","answers":[]}"#
        );
        let bound = Answer {
            equations: vec!["X = 3".to_string()],
            disequations: vec!["X != Y".to_string()],
            constraints: vec!["age(alice) = 30".to_string()],
            explanation: None,
        };
        assert_eq!(
            query_document("?- q(X).", &[bound], None),
            r#"{"query":"?- q(X).","outcome":"solutions","answers":[{"equations":["X = 3"],"disequations":["X != Y"],"constraints":["age(alice) = 30"]}]}"#
        );
        // CLM-0023: the explanation, when present, nests under `subgoals`.
        let explained = Answer {
            equations: vec!["C = c17".to_string()],
            disequations: vec![],
            constraints: vec![],
            explanation: Some(vec![ProofNode {
                goal: "violation(c17)".to_string(),
                by: claimr::Resolution::Rule,
                line: Some(2),
                column: Some(1),
                clause: "violation(C) :- k(C, K), { K > 3 }.".to_string().into(),
                subgoals: vec![
                    ProofNode {
                        goal: "k(c17, 5)".to_string(),
                        by: claimr::Resolution::Fact,
                        line: Some(1),
                        column: Some(1),
                        clause: Some("k(c17, 5).".to_string()),
                        subgoals: vec![],
                        failure: None,
                    },
                    ProofNode {
                        goal: "{ 5 > 3 }".to_string(),
                        by: claimr::Resolution::Constraint,
                        line: Some(2),
                        column: Some(1),
                        clause: None,
                        subgoals: vec![],
                        failure: None,
                    },
                ],
                failure: None,
            }]),
        };
        assert_eq!(
            query_document("?- violation(C).", &[explained], None),
            r#"{"query":"?- violation(C).","outcome":"solutions","answers":[{"equations":["C = c17"],"disequations":[],"constraints":[],"explanation":[{"goal":"violation(c17)","by":"rule","line":2,"column":1,"clause":"violation(C) :- k(C, K), { K > 3 }.","subgoals":[{"goal":"k(c17, 5)","by":"fact","line":1,"column":1,"clause":"k(c17, 5).","subgoals":[]},{"goal":"{ 5 > 3 }","by":"constraint","line":2,"column":1,"subgoals":[]}]}]}]}"#
        );
        assert_eq!(
            query_error_document("?- q(X).", "boom"),
            r#"{"query":"?- q(X).","outcome":"error","error":{"message":"boom"}}"#
        );
        assert_eq!(
            error_document("f.claimr", Some(2), Some(1), "Expected Dot."),
            r#"{"error":{"file":"f.claimr","line":2,"column":1,"message":"Expected Dot."}}"#
        );
        assert_eq!(
            error_document("f.claimr", None, None, "cannot read"),
            r#"{"error":{"file":"f.claimr","message":"cannot read"}}"#
        );
    }
}
