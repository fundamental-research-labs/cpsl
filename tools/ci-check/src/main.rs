//! Strict CI checks for Rust source size and module documentation.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

const DEFAULT_MAX_FILE_LINES: usize = 1000;
const DEFAULT_MAX_FUNCTION_LINES: usize = 350;
const DEFAULT_MIN_DOC_CHARS: usize = 20;
const DEFAULT_MAX_DOC_CHARS: usize = 700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    All,
    FileLength,
    FunctionLength,
    Docstring,
}

#[derive(Debug)]
struct Config {
    rule: Rule,
    paths: Vec<PathBuf>,
    max_file_lines: usize,
    max_function_lines: usize,
    min_doc_chars: usize,
    max_doc_chars: usize,
}

#[derive(Debug)]
struct Violation {
    path: PathBuf,
    line: usize,
    rule: &'static str,
    message: String,
}

impl Violation {
    fn render(&self) -> String {
        format!(
            "{}:{}: {}: {}",
            self.path.display(),
            self.line,
            self.rule,
            self.message
        )
    }
}

fn main() {
    match run() {
        Ok(0) => {}
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    }
}

fn run() -> Result<i32, String> {
    let config = parse_args(env::args().skip(1).collect())?;
    let files = rust_files(&config.paths).map_err(|err| err.to_string())?;
    let mut violations = Vec::new();

    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let lines: Vec<&str> = source.lines().collect();

        if matches!(config.rule, Rule::All | Rule::FileLength) {
            check_file_length(&path, lines.len(), config.max_file_lines, &mut violations);
        }
        if matches!(config.rule, Rule::All | Rule::FunctionLength) {
            check_function_length(&path, &lines, config.max_function_lines, &mut violations);
        }
        if matches!(config.rule, Rule::All | Rule::Docstring) {
            check_docstring(
                &path,
                &lines,
                config.min_doc_chars,
                config.max_doc_chars,
                &mut violations,
            );
        }
    }

    if violations.is_empty() {
        return Ok(0);
    }

    for violation in violations {
        println!("{}", violation.render());
    }
    Ok(1)
}

fn parse_args(args: Vec<String>) -> Result<Config, String> {
    if args.is_empty() {
        return Err(usage());
    }

    let rule = match args[0].as_str() {
        "all" => Rule::All,
        "file-length" => Rule::FileLength,
        "function-length" => Rule::FunctionLength,
        "docstring" => Rule::Docstring,
        _ => return Err(usage()),
    };

    let mut config = Config {
        rule,
        paths: Vec::new(),
        max_file_lines: DEFAULT_MAX_FILE_LINES,
        max_function_lines: DEFAULT_MAX_FUNCTION_LINES,
        min_doc_chars: DEFAULT_MIN_DOC_CHARS,
        max_doc_chars: DEFAULT_MAX_DOC_CHARS,
    };

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--max-file-lines" => {
                config.max_file_lines = parse_value(&args, &mut index, "--max-file-lines")?;
            }
            "--max-function-lines" => {
                config.max_function_lines = parse_value(&args, &mut index, "--max-function-lines")?;
            }
            "--min-doc-chars" => {
                config.min_doc_chars = parse_value(&args, &mut index, "--min-doc-chars")?;
            }
            "--max-doc-chars" => {
                config.max_doc_chars = parse_value(&args, &mut index, "--max-doc-chars")?;
            }
            "-h" | "--help" => return Err(usage()),
            path if path.starts_with('-') => return Err(format!("unknown flag: {path}")),
            path => config.paths.push(PathBuf::from(path)),
        }
        index += 1;
    }

    if config.paths.is_empty() {
        config.paths.push(PathBuf::from("."));
    }

    Ok(config)
}

fn parse_value(args: &[String], index: &mut usize, flag: &str) -> Result<usize, String> {
    *index += 1;
    let value = args
        .get(*index)
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse()
        .map_err(|_| format!("{flag} requires a positive integer"))
}

fn usage() -> String {
    "usage: ci-check <all|file-length|function-length|docstring> [flags] [paths...]".into()
}

fn rust_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        collect_rust_files(path, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if should_skip(&path) {
            continue;
        }
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target"))
}

fn check_file_length(path: &Path, lines: usize, max_lines: usize, violations: &mut Vec<Violation>) {
    if lines > max_lines {
        violations.push(Violation {
            path: path.to_path_buf(),
            line: 1,
            rule: "file-length",
            message: format!("file has {lines} lines (max {max_lines})"),
        });
    }
}

fn check_function_length(
    path: &Path,
    lines: &[&str],
    max_lines: usize,
    violations: &mut Vec<Violation>,
) {
    let mut scanner = BraceScanner::default();
    let mut pending: Option<FunctionState> = None;

    for (line_index, raw_line) in lines.iter().enumerate() {
        let line_number = line_index + 1;
        let code = scanner.strip(raw_line);

        if pending.is_none() {
            if let Some(name) = parse_fn_name(&code) {
                pending = Some(FunctionState {
                    name,
                    start_line: line_number,
                    brace_depth: 0,
                    opened: false,
                });
            }
        }

        let Some(function) = pending.as_mut() else {
            continue;
        };

        function.brace_depth += brace_delta(&code);
        if code.contains('{') {
            function.opened = true;
        }

        if function.opened && function.brace_depth <= 0 {
            let function = pending.take().expect("function state exists");
            let length = line_number - function.start_line + 1;
            if length > max_lines {
                violations.push(Violation {
                    path: path.to_path_buf(),
                    line: function.start_line,
                    rule: "function-length",
                    message: format!(
                        "function {} has {length} lines (max {max_lines})",
                        function.name
                    ),
                });
            }
        }
    }
}

#[derive(Debug)]
struct FunctionState {
    name: String,
    start_line: usize,
    brace_depth: i32,
    opened: bool,
}

fn parse_fn_name(line: &str) -> Option<String> {
    let mut rest = line.trim_start();
    loop {
        let next = rest
            .strip_prefix("pub(crate) ")
            .or_else(|| rest.strip_prefix("pub(super) "))
            .or_else(|| rest.strip_prefix("pub(self) "))
            .or_else(|| rest.strip_prefix("pub "))
            .or_else(|| rest.strip_prefix("async "))
            .or_else(|| rest.strip_prefix("const "))
            .or_else(|| rest.strip_prefix("unsafe "));

        if let Some(next) = next {
            rest = next;
            continue;
        }

        if let Some(after_pub) = rest.strip_prefix("pub(") {
            let end = after_pub.find(')')?;
            rest = after_pub[end + 1..].trim_start();
            continue;
        }
        break;
    }

    let rest = rest.strip_prefix("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

#[derive(Default)]
struct BraceScanner {
    in_block_comment: bool,
    in_string: bool,
    in_char: bool,
    escaped: bool,
}

impl BraceScanner {
    fn strip(&mut self, line: &str) -> String {
        let mut out = String::new();
        let mut chars = line.chars().peekable();

        while let Some(ch) = chars.next() {
            let next = chars.peek().copied();
            if self.in_block_comment {
                if ch == '*' && next == Some('/') {
                    self.in_block_comment = false;
                    chars.next();
                }
                continue;
            }
            if self.in_string {
                if !self.escaped && ch == '"' {
                    self.in_string = false;
                }
                self.escaped = ch == '\\' && !self.escaped;
                if ch != '\\' {
                    self.escaped = false;
                }
                continue;
            }
            if self.in_char {
                if !self.escaped && ch == '\'' {
                    self.in_char = false;
                }
                self.escaped = ch == '\\' && !self.escaped;
                if ch != '\\' {
                    self.escaped = false;
                }
                continue;
            }

            match (ch, next) {
                ('/', Some('/')) => break,
                ('/', Some('*')) => {
                    self.in_block_comment = true;
                    chars.next();
                }
                ('"', _) => self.in_string = true,
                ('\'', Some(next)) if !next.is_ascii_alphabetic() && next != '_' => {
                    self.in_char = true;
                }
                ('\'', _) => out.push(ch),
                _ => out.push(ch),
            }
        }

        out
    }
}

fn check_docstring(
    path: &Path,
    lines: &[&str],
    min_chars: usize,
    max_chars: usize,
    violations: &mut Vec<Violation>,
) {
    let (line, doc) = module_doc(lines);
    let chars = doc.trim().chars().count();
    if chars < min_chars {
        violations.push(Violation {
            path: path.to_path_buf(),
            line,
            rule: "docstring",
            message: format!("module doc comment has {chars} chars (min {min_chars})"),
        });
    } else if chars > max_chars {
        violations.push(Violation {
            path: path.to_path_buf(),
            line,
            rule: "docstring",
            message: format!("module doc comment has {chars} chars (max {max_chars})"),
        });
    }
}

fn module_doc(lines: &[&str]) -> (usize, String) {
    let mut doc = String::new();
    let mut first_doc_line = 1;
    let mut seen_doc = false;
    let mut in_block_doc = false;

    for (line_index, raw_line) in lines.iter().enumerate() {
        let line_number = line_index + 1;
        let trimmed = raw_line.trim();

        if in_block_doc {
            if let Some((before, _after)) = trimmed.split_once("*/") {
                doc.push_str(before.trim());
                break;
            }
            doc.push_str(trimmed);
            doc.push(' ');
            continue;
        }

        if trimmed.is_empty() {
            if seen_doc {
                break;
            }
            continue;
        }
        if trimmed.starts_with("#![") {
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("//!") {
            if !seen_doc {
                first_doc_line = line_number;
            }
            seen_doc = true;
            doc.push_str(text.trim());
            doc.push(' ');
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("/*!") {
            if !seen_doc {
                first_doc_line = line_number;
            }
            seen_doc = true;
            if let Some((before, _after)) = text.split_once("*/") {
                doc.push_str(before.trim());
                break;
            }
            doc.push_str(text.trim());
            doc.push(' ');
            in_block_doc = true;
            continue;
        }
        break;
    }

    (first_doc_line, doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_function_forms() {
        assert_eq!(parse_fn_name("fn plain() {}").as_deref(), Some("plain"));
        assert_eq!(
            parse_fn_name("pub(crate) async fn run() {}").as_deref(),
            Some("run")
        );
        assert_eq!(
            parse_fn_name("pub(in crate::x) unsafe fn raw() {}").as_deref(),
            Some("raw")
        );
    }

    #[test]
    fn collects_line_module_docs() {
        let lines = [
            "#![allow(dead_code)]",
            "//! hello",
            "//! world",
            "",
            "fn x() {}",
        ];
        assert_eq!(module_doc(&lines), (2, "hello world ".to_string()));
    }

    #[test]
    fn strips_braces_in_strings_and_comments() {
        let mut scanner = BraceScanner::default();
        assert_eq!(
            scanner.strip(r#"fn x() { println!("{"); } // }"#),
            "fn x() { println!(); } "
        );
    }

    #[test]
    fn does_not_treat_lifetimes_as_char_literals() {
        let mut scanner = BraceScanner::default();
        assert_eq!(
            scanner.strip("fn params() -> &'static [Param] {"),
            "fn params() -> &'static [Param] {"
        );
    }
}
