//! FFF-backed content search module for the Luau sandbox.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use crate::{MountError, MountTable};
use fff_grep::{LineTerminator, Match, Matcher, NoError, SearcherBuilder, Sink, SinkMatch};
use mlua::{Lua, MultiValue, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FFF_GREP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "pattern",
        typ: "string",
        required: true,
        description: "Literal byte pattern to search for",
    },
    FieldDoc {
        name: "path",
        typ: "string",
        required: true,
        description: "File or directory to search",
    },
    FieldDoc {
        name: "max_count",
        typ: "number",
        required: false,
        description: "Maximum number of matching lines to return",
    },
];

pub(crate) static FFF_DOC: ModuleDoc = ModuleDoc {
    name: "fff",
    summary: "Fast literal content search powered by fff-grep",
    functions: &[FnDoc {
        name: "grep",
        description: "Search a file or directory for a literal pattern. Returns matching lines.",
        params: &[Param {
            name: "opts",
            short: None,
            typ: ParamType::Table,
            required: true,
            fields: Some(FFF_GREP_OPTS_FIELDS),
        }],
        returns: ReturnType::Table,
        example: Some(r#"fff.grep({pattern="TODO", path="/workspace", max_count=20})"#),
    }],
};

#[derive(Clone)]
struct LiteralMatcher {
    needle: Vec<u8>,
}

impl LiteralMatcher {
    fn new(needle: Vec<u8>) -> Self {
        Self { needle }
    }
}

impl Matcher for LiteralMatcher {
    type Error = NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, Self::Error> {
        if at > haystack.len() {
            return Ok(None);
        }

        Ok(
            memchr::memmem::find(&haystack[at..], &self.needle).map(|start| {
                let start = start + at;
                Match::new(start, start + self.needle.len())
            }),
        )
    }

    fn line_terminator(&self) -> Option<LineTerminator> {
        if self.needle.contains(&b'\n') {
            None
        } else {
            Some(LineTerminator::byte(b'\n'))
        }
    }
}

struct LineMatch {
    line_number: u64,
    column: usize,
    line: String,
    match_text: String,
}

struct CollectSink {
    needle: Vec<u8>,
    max_count: Option<usize>,
    matches: Vec<LineMatch>,
}

impl CollectSink {
    fn new(needle: Vec<u8>, max_count: Option<usize>) -> Self {
        Self {
            needle,
            max_count,
            matches: Vec::new(),
        }
    }

    fn is_full(&self) -> bool {
        self.max_count
            .map(|max| self.matches.len() >= max)
            .unwrap_or(false)
    }
}

impl Sink for CollectSink {
    type Error = std::io::Error;

    fn begin(&mut self, _searcher: &fff_grep::Searcher) -> Result<bool, Self::Error> {
        Ok(!self.is_full())
    }

    fn matched(
        &mut self,
        _searcher: &fff_grep::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        if self.is_full() {
            return Ok(false);
        }

        let line_bytes = trim_line_end(mat.bytes());
        let column = memchr::memmem::find(line_bytes, &self.needle)
            .map(|idx| idx + 1)
            .unwrap_or(1);
        let match_text = line_bytes
            .get((column - 1)..(column - 1 + self.needle.len()))
            .map(String::from_utf8_lossy)
            .map(|s| s.into_owned())
            .unwrap_or_default();

        self.matches.push(LineMatch {
            line_number: mat.line_number().unwrap_or(0),
            column,
            line: String::from_utf8_lossy(line_bytes).into_owned(),
            match_text,
        });

        Ok(!self.is_full())
    }
}

pub(crate) fn register_fff_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let fff_table = lua.create_table()?;

    let grep_mounts = mounts.clone();
    fff_table.set(
        "grep",
        lua.create_function(move |lua, args: MultiValue| {
            let validated = validate_args(&args, FFF_DOC.params("grep"), "fff.grep")?;
            let opts = match &validated[0] {
                Value::Table(t) => t.clone(),
                _ => unreachable!("validate_args ensures table"),
            };

            let pattern = table_string(&opts, "pattern", "fff.grep")?;
            if pattern.is_empty() {
                return Err(mlua::Error::external(
                    "fff.grep: field 'pattern' must not be empty",
                ));
            }

            let sandbox_path = table_string(&opts, "path", "fff.grep")?;
            let max_count = table_usize(&opts, "max_count");
            let host_path = grep_mounts
                .resolve_read(&sandbox_path)
                .map_err(mlua::Error::external)?;
            let files = collect_files(&host_path, &sandbox_path)?;
            let result_table = lua.create_table()?;
            let needle = pattern.into_bytes();
            let mut result_idx = 1;
            let mut match_count = 0;

            for (host_file, virtual_file) in files {
                if max_count.map(|max| match_count >= max).unwrap_or(false) {
                    break;
                }

                let bytes = match std::fs::read(&host_file) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let remaining = max_count.map(|max| max.saturating_sub(match_count));
                let mut sink = CollectSink::new(needle.clone(), remaining);
                let searcher = SearcherBuilder::new().line_number(true).build();

                searcher
                    .search_slice(LiteralMatcher::new(needle.clone()), &bytes, &mut sink)
                    .map_err(|e| mlua::Error::external(format!("fff.grep: {e}")))?;

                for line_match in sink.matches {
                    let entry = lua.create_table()?;
                    entry.set("file", virtual_file.as_str())?;
                    entry.set("line_number", line_match.line_number)?;
                    entry.set("column", line_match.column)?;
                    entry.set("line", line_match.line)?;
                    entry.set("match_text", line_match.match_text)?;
                    result_table.set(result_idx, entry)?;
                    result_idx += 1;
                    match_count += 1;
                }
            }

            Ok(result_table)
        })?,
    )?;

    register_help_functions(lua, &fff_table, &FFF_DOC)?;
    lua.globals().set("fff", fff_table)?;
    wrap_module_with_help_hints(lua, "fff")?;

    Ok(())
}

fn table_string(table: &mlua::Table, field: &str, fn_name: &str) -> Result<String, mlua::Error> {
    table
        .get::<mlua::String>(field)
        .map(|s| s.to_string_lossy().to_string())
        .map_err(|_| {
            mlua::Error::external(format!(
                "{fn_name}: missing required field '{field}' (string)"
            ))
        })
}

fn table_usize(table: &mlua::Table, field: &str) -> Option<usize> {
    table
        .get::<Value>(field)
        .ok()
        .and_then(|value| match value {
            Value::Integer(n) => Some(n.max(0) as usize),
            Value::Number(n) => Some((n as i64).max(0) as usize),
            _ => None,
        })
}

fn collect_files(
    host_path: &Path,
    sandbox_path: &str,
) -> Result<Vec<(PathBuf, String)>, mlua::Error> {
    if host_path.is_file() {
        return Ok(vec![(host_path.to_path_buf(), sandbox_path.to_string())]);
    }

    if !host_path.is_dir() {
        return Err(mlua::Error::external(MountError::NotFound(
            sandbox_path.to_string(),
        )));
    }

    let mut files = Vec::new();
    let mut builder = ignore::WalkBuilder::new(host_path);
    builder.hidden(false);

    for entry in builder.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = match path.strip_prefix(host_path) {
            Ok(relative) if !relative.as_os_str().is_empty() => relative,
            _ => continue,
        };
        let rel_str = relative.to_string_lossy();
        let virtual_path = if sandbox_path.ends_with('/') {
            format!("{sandbox_path}{rel_str}")
        } else {
            format!("{sandbox_path}/{rel_str}")
        };

        files.push((path.to_path_buf(), virtual_path));
    }

    Ok(files)
}

fn trim_line_end(mut line: &[u8]) -> &[u8] {
    if let Some(stripped) = line.strip_suffix(b"\n") {
        line = stripped;
    }
    if let Some(stripped) = line.strip_suffix(b"\r") {
        line = stripped;
    }
    line
}
