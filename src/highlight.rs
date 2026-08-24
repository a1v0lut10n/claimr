//! CLM-0009: claimr's token vocabulary, for editors.
//!
//! An error-tolerant, line-based lexer over the grammar's terminals
//! (`src/parser/claimr.rustemo`). Line-based and stateless on purpose:
//! an editor's buffer is mid-edit most of the time — a full LR parse
//! fails there; lexing never does. Ranges are byte ranges within the
//! line. The parser proper stays the source of truth for *meaning*;
//! this module only names what each span *is*.

use std::ops::Range;

/// What a span is, in claimr's own vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// `% …` to end of line.
    Comment,
    /// Uppercase-initial: a logic variable (`X`, `Total`).
    Variable,
    /// Lowercase-initial identifier applied to arguments: `parent(…)`.
    Predicate,
    /// Any other lowercase-initial identifier (an atom).
    Ident,
    /// Exact-rational literal (`42`, `18.5`).
    Number,
    /// `:-  ?-  =>  !=  <=  >=  =  <  >  +  -  *  /`.
    Operator,
    /// `. , ( ) { }`.
    Punctuation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub range: Range<usize>,
    pub kind: TokenKind,
}

/// Tokenize one line. Unrecognised bytes are skipped, never an error.
pub fn line_tokens(line: &str) -> Vec<Token> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Comment: to end of line.
        if c == b'%' {
            out.push(Token {
                range: i..line.len(),
                kind: TokenKind::Comment,
            });
            break;
        }
        // Identifiers and variables.
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let kind = if c.is_ascii_uppercase() {
                TokenKind::Variable
            } else {
                // A predicate when applied: `name(` (whitespace allowed).
                let mut j = i;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                    j += 1;
                }
                if j < b.len() && b[j] == b'(' {
                    TokenKind::Predicate
                } else {
                    TokenKind::Ident
                }
            };
            out.push(Token {
                range: start..i,
                kind,
            });
            continue;
        }
        // Numbers (exact rationals).
        if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push(Token {
                range: start..i,
                kind: TokenKind::Number,
            });
            continue;
        }
        // Operators, longest first (the grammar's exact set).
        const OPS: [&str; 13] = [
            ":-", "?-", "=>", "!=", "<=", ">=", "=", "<", ">", "+", "-", "*", "/",
        ];
        if let Some(op) = OPS.iter().find(|op| line[i..].starts_with(**op)) {
            out.push(Token {
                range: i..i + op.len(),
                kind: TokenKind::Operator,
            });
            i += op.len();
            continue;
        }
        if matches!(c, b'.' | b',' | b'(' | b')' | b'{' | b'}') {
            out.push(Token {
                range: i..i + 1,
                kind: TokenKind::Punctuation,
            });
            i += 1;
            continue;
        }
        // Whitespace or anything the grammar doesn't know: skip a char.
        i += line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(String, TokenKind)> {
        line_tokens(line)
            .into_iter()
            .map(|t| (line[t.range.clone()].to_string(), t.kind))
            .collect()
    }

    #[test]
    fn a_clause_tokenizes_by_role() {
        use TokenKind::*;
        assert_eq!(
            kinds("grandparent(X, Z) :- parent(X, Y), parent(Y, Z). % family"),
            vec![
                ("grandparent".into(), Predicate),
                ("(".into(), Punctuation),
                ("X".into(), Variable),
                (",".into(), Punctuation),
                ("Z".into(), Variable),
                (")".into(), Punctuation),
                (":-".into(), Operator),
                ("parent".into(), Predicate),
                ("(".into(), Punctuation),
                ("X".into(), Variable),
                (",".into(), Punctuation),
                ("Y".into(), Variable),
                (")".into(), Punctuation),
                (",".into(), Punctuation),
                ("parent".into(), Predicate),
                ("(".into(), Punctuation),
                ("Y".into(), Variable),
                (",".into(), Punctuation),
                ("Z".into(), Variable),
                (")".into(), Punctuation),
                (".".into(), Punctuation),
                ("% family".into(), Comment),
            ]
        );
    }

    #[test]
    fn queries_numbers_and_relops_lex() {
        use TokenKind::*;
        assert_eq!(
            kinds("?- total(T), T >= 18.5."),
            vec![
                ("?-".into(), Operator),
                ("total".into(), Predicate),
                ("(".into(), Punctuation),
                ("T".into(), Variable),
                (")".into(), Punctuation),
                (",".into(), Punctuation),
                ("T".into(), Variable),
                (">=".into(), Operator),
                ("18.5".into(), Number),
                (".".into(), Punctuation),
            ]
        );
    }

    #[test]
    fn mid_edit_junk_is_skipped_not_fatal() {
        assert!(!line_tokens("p(X@# :-").is_empty());
        assert!(line_tokens("").is_empty());
    }

    /// Drift alarm: every terminal the grammar declares is one this
    /// lexer knows how to produce.
    #[test]
    fn lexer_covers_the_grammars_terminal_set() {
        let grammar = include_str!("parser/claimr.rustemo");
        let terminals_block = grammar.split("terminals").nth(1).expect("terminals block");
        for line in terminals_block.lines() {
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() || name.starts_with("//") || matches!(name, "WS" | "Comment") {
                continue;
            }
            // A literal terminal must lex to exactly one token.
            if let Some(lit) = rest.trim().strip_suffix(';').and_then(|r| {
                let r = r.trim();
                r.strip_prefix('\'').and_then(|x| x.strip_suffix('\''))
            }) {
                let toks = line_tokens(lit);
                assert_eq!(toks.len(), 1, "terminal {name} ({lit:?}) must lex");
                assert_eq!(&lit[toks[0].range.clone()], lit);
            }
        }
        // The regex terminals, by example.
        assert_eq!(line_tokens("abc")[0].kind, TokenKind::Ident);
        assert_eq!(line_tokens("Abc")[0].kind, TokenKind::Variable);
        assert_eq!(line_tokens("12.5")[0].kind, TokenKind::Number);
        assert_eq!(line_tokens("% x")[0].kind, TokenKind::Comment);
    }
}
