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

use claimr::Answer;

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
    format!(
        "{{\"equations\":{},\"disequations\":{},\"constraints\":{}}}",
        string_array(&answer.equations),
        string_array(&answer.disequations),
        string_array(&answer.constraints),
    )
}

/// The per-query document: `outcome` is `"solutions"` when any answer
/// exists, `"none"` otherwise.
pub fn query_document(query_text: &str, answers: &[Answer]) -> String {
    let outcome = if answers.is_empty() {
        "none"
    } else {
        "solutions"
    };
    let rendered: Vec<String> = answers.iter().map(answer_object).collect();
    format!(
        "{{\"query\":\"{}\",\"outcome\":\"{outcome}\",\"answers\":[{}]}}",
        escape(query_text),
        rendered.join(","),
    )
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
        };
        assert_eq!(
            query_document("?- ok(s1).", &[truth]),
            r#"{"query":"?- ok(s1).","outcome":"solutions","answers":[{"equations":[],"disequations":[],"constraints":[]}]}"#
        );
        assert_eq!(
            query_document("?- ok(s2).", &[]),
            r#"{"query":"?- ok(s2).","outcome":"none","answers":[]}"#
        );
        let bound = Answer {
            equations: vec!["X = 3".to_string()],
            disequations: vec!["X != Y".to_string()],
            constraints: vec!["age(alice) = 30".to_string()],
        };
        assert_eq!(
            query_document("?- q(X).", &[bound]),
            r#"{"query":"?- q(X).","outcome":"solutions","answers":[{"equations":["X = 3"],"disequations":["X != Y"],"constraints":["age(alice) = 30"]}]}"#
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
