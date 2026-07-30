use miette::{Diagnostic, NamedSource, SourceSpan};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    pub code: String,
    pub message: String,
    pub help: Option<String>,
    pub path: String,
    pub instance_path: String,
    pub source_path: Option<PathBuf>,
    pub source: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Error, Debug, Diagnostic, Clone)]
#[error("{message}")]
#[diagnostic(code = "{code}")]
pub struct ValidationDiagnostic {
    pub message: String,
    #[source_code]
    src: NamedSource<String>,

    #[label("at {path}")]
    span: SourceSpan,

    code: String,
    help: Option<String>,

    path: String,
    instance_path: String,
    source_path: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl ValidationDiagnostic {
    pub fn from_input(input: DiagnosticInput) -> Self {
        let (src, span) = build_source_span(input.source.clone(), &input.instance_path);
        let (line, column) = input.line.zip(input.column).map_or_else(
            || position_from_source(&src.inner(), span.offset()),
            |(l, c)| (Some(l), Some(c)),
        );

        Self {
            message: input.message,
            src,
            span,
            code: input.code,
            help: input.help,
            path: input.path,
            instance_path: input.instance_path,
            source_path: input.source_path,
            line,
            column,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn instance_path(&self) -> &str {
        &self.instance_path
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

fn position_from_source(source: &str, offset: usize) -> (Option<usize>, Option<usize>) {
    let clamped = offset.min(source.len());
    let prefix = &source[..clamped];
    let line = prefix.lines().count().saturating_sub(1);
    let column = prefix.lines().last().map(|l| l.len()).unwrap_or(0);
    (Some(line), Some(column))
}

fn build_source_span(
    source: Option<String>,
    instance_path: &str,
) -> (NamedSource<String>, SourceSpan) {
    let empty = NamedSource::new("<unknown>", String::new());

    let source = match source {
        Some(s) => s,
        None => return (empty, (0, 0).into()),
    };

    let offset = approximate_offset(&source, instance_path);
    let label_start = offset.min(source.len());
    let label_len = find_token_len(&source, label_start).min(source.len().saturating_sub(label_start));

    let span: SourceSpan = (label_start, label_len).into();

    (
        NamedSource::new("<spec>", source),
        span,
    )
}

fn approximate_offset(source: &str, instance_path: &str) -> usize {
    if instance_path.is_empty() || instance_path == "/" {
        return 0;
    }

    let parts: Vec<&str> = instance_path
        .trim_start_matches('/')
        .split('/')
        .collect();

    let mut offset = 0usize;
    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx == parts.len() - 1;
        if let Some(found) = find_key_offset(source, offset, part) {
            offset = found;
            if !is_last {
                offset = move_past_key(source, offset);
            }
        } else {
            break;
        }
    }

    offset
}

fn find_key_offset(source: &str, start: usize, key: &str) -> Option<usize> {
    let mut search_start = start;
    loop {
        if search_start >= source.len() {
            return None;
        }
        let rest = &source[search_start..];
        let mut found = None;
        for (pos, _) in rest.match_indices(key) {
            let absolute: usize = pos + search_start;
            if is_key_at(source, absolute, key) {
                found = Some(absolute);
                break;
            }
        }
        if let Some(pos) = found {
            return Some(pos);
        }
        search_start += 1;
    }
}

fn is_key_at(source: &str, pos: usize, key: &str) -> bool {
    if pos.saturating_add(key.len()) > source.len() {
        return false;
    }
    if source[pos..pos + key.len()] != *key {
        return false;
    }

    let after = pos + key.len();
    let after_colon = source[after..].trim_start();
    let before = source[..pos].trim_end();
    let before_ok = before.is_empty()
        || before.ends_with('\n')
        || before.ends_with('{')
        || before.ends_with(',')
        || before.ends_with('-');
    let after_ok = after_colon.starts_with(':') || after_colon.starts_with('=');
    before_ok && after_ok
}

fn move_past_key(source: &str, pos: usize) -> usize {
    source[pos..]
        .find(':')
        .map(|i| pos + i + 1)
        .unwrap_or(pos)
}

fn find_token_len(source: &str, start: usize) -> usize {
    let rest = &source[start..];
    let rest = rest.trim_start();
    let start = start + (rest.len() - rest.trim_start().len());
    if start >= source.len() {
        return 0;
    }
    let mut chars = source[start..].chars().peekable();
    let first = chars.peek().copied().unwrap_or('\0');

    if first == '\n' {
        return 1;
    }

    if first == '"' || first == '\'' {
        chars.next();
        let mut len = 1;
        let mut escaped = false;
        for c in chars {
            len += 1;
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == first {
                break;
            }
        }
        return len.min(source.len() - start);
    }

    let mut len = 0;
    for c in source[start..].chars() {
        if c.is_whitespace() || c == ',' || c == '}' || c == ']' {
            break;
        }
        len += c.len_utf8();
    }
    len.max(1)
}
