//! Shared `fs.grep` API layer for grep-like providers.

use crate::sandbox::{validate_args, Param};
use crate::{MountError, MountTable};
use mlua::{Lua, MultiValue, Value};
#[cfg(feature = "mod-ripgrep")]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrepRequest {
    pub pattern: String,
    pub path: String,
    pub mode: GrepMode,
    pub glob: Option<String>,
    pub max_count: Option<usize>,
    pub files_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrepMode {
    Regex,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrepMatch {
    pub file: String,
    pub line_number: u64,
    pub line: String,
    pub match_text: String,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GrepResult {
    Match(GrepMatch),
    File(String),
}

pub(crate) trait GrepProvider: Clone + Send + Sync + 'static {
    fn search(&self, request: &GrepRequest) -> Result<Vec<GrepResult>, GrepError>;
}

#[derive(Debug)]
pub(crate) enum GrepError {
    InvalidGlob(String),
    InvalidPattern(String),
    #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
    Usage(String),
    Mount(MountError),
    #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
    Search(String),
}

impl GrepError {
    pub(crate) fn into_lua(self, fn_name: &str) -> mlua::Error {
        match self {
            GrepError::InvalidGlob(message) => {
                mlua::Error::external(format!("{fn_name}: invalid glob: {message}"))
            }
            GrepError::InvalidPattern(message) => {
                mlua::Error::external(format!("{fn_name}: invalid pattern: {message}"))
            }
            #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
            GrepError::Usage(message) => mlua::Error::external(format!("{fn_name}: {message}")),
            GrepError::Mount(error) => mlua::Error::external(error),
            #[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
            GrepError::Search(message) => mlua::Error::external(format!("{fn_name}: {message}")),
        }
    }
}

#[cfg(feature = "mod-fs")]
pub(crate) fn register_fs_grep<P>(
    lua: &Lua,
    fs: &mlua::Table,
    provider: P,
) -> Result<(), mlua::Error>
where
    P: GrepProvider,
{
    fs.set(
        "grep",
        lua.create_function(move |lua, args: MultiValue| {
            let request =
                parse_grep_request(&args, crate::sandbox::FS_DOC.params("grep"), "fs.grep")?;
            let results = provider
                .search(&request)
                .map_err(|error| error.into_lua("fs.grep"))?;
            grep_results_to_lua(lua, results, false)
        })?,
    )?;
    Ok(())
}

pub(crate) fn parse_grep_request(
    args: &MultiValue,
    params: &[Param],
    fn_name: &str,
) -> Result<GrepRequest, mlua::Error> {
    let validated = validate_args(args, params, fn_name)?;
    let opts = match &validated[0] {
        Value::Table(t) => t.clone(),
        _ => unreachable!("validate_args ensures table"),
    };

    let pattern = required_table_string(&opts, "pattern", fn_name)?;
    let path = required_table_string(&opts, "path", fn_name)?;
    let mode = optional_grep_mode(&opts, fn_name)?;
    let glob = optional_table_string(&opts, "glob");
    let max_count = optional_table_usize(&opts, "max_count");
    let files_only = optional_table_bool(&opts, "files_only").unwrap_or(false);

    Ok(GrepRequest {
        pattern,
        path,
        mode,
        glob,
        max_count,
        files_only,
    })
}

pub(crate) fn grep_results_to_lua(
    lua: &Lua,
    results: Vec<GrepResult>,
    include_column: bool,
) -> Result<mlua::Table, mlua::Error> {
    let result_table = lua.create_table()?;
    for (idx, result) in results.into_iter().enumerate() {
        match result {
            GrepResult::File(file) => result_table.set(idx + 1, file)?,
            GrepResult::Match(grep_match) => {
                let entry = lua.create_table()?;
                entry.set("file", grep_match.file)?;
                entry.set("line_number", grep_match.line_number)?;
                entry.set("line", grep_match.line)?;
                entry.set("match_text", grep_match.match_text)?;
                if include_column {
                    if let Some(column) = grep_match.column {
                        entry.set("column", column)?;
                    }
                }
                result_table.set(idx + 1, entry)?;
            }
        }
    }
    Ok(result_table)
}

fn required_table_string(
    table: &mlua::Table,
    field: &str,
    fn_name: &str,
) -> Result<String, mlua::Error> {
    table
        .get::<mlua::LuaString>(field)
        .map(|s| s.to_string_lossy().to_string())
        .map_err(|_| {
            mlua::Error::external(format!(
                "{fn_name}: missing required field '{field}' (string)"
            ))
        })
}

fn optional_table_string(table: &mlua::Table, field: &str) -> Option<String> {
    table
        .get::<Value>(field)
        .ok()
        .and_then(|value| match value {
            Value::String(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
}

fn optional_table_usize(table: &mlua::Table, field: &str) -> Option<usize> {
    table
        .get::<Value>(field)
        .ok()
        .and_then(|value| match value {
            Value::Integer(n) => Some(n.max(0) as usize),
            Value::Number(n) => Some((n as i64).max(0) as usize),
            _ => None,
        })
}

fn optional_table_bool(table: &mlua::Table, field: &str) -> Option<bool> {
    table
        .get::<Value>(field)
        .ok()
        .and_then(|value| match value {
            Value::Boolean(value) => Some(value),
            _ => None,
        })
}

fn optional_grep_mode(table: &mlua::Table, fn_name: &str) -> Result<GrepMode, mlua::Error> {
    match table.get::<Value>("mode").unwrap_or(Value::Nil) {
        Value::Nil => Ok(GrepMode::Regex),
        Value::String(mode) => match mode.to_string_lossy().as_ref() {
            "regex" => Ok(GrepMode::Regex),
            "plain" => Ok(GrepMode::Plain),
            other => Err(mlua::Error::external(format!(
                "{fn_name}: unsupported mode '{other}' (supported: regex, plain)"
            ))),
        },
        value => Err(mlua::Error::external(format!(
            "{fn_name}: field 'mode' expected string, got {}",
            value.type_name()
        ))),
    }
}

fn collect_files(
    mounts: &MountTable,
    request: &GrepRequest,
) -> Result<Vec<(PathBuf, String)>, GrepError> {
    let compiled_glob = request.glob.as_deref().map(compile_glob).transpose()?;
    let host_path = mounts
        .resolve_read(&request.path)
        .map_err(GrepError::Mount)?;

    if host_path.is_file() {
        return Ok(vec![(host_path, request.path.clone())]);
    }

    if !host_path.is_dir() {
        return Err(GrepError::Mount(MountError::NotFound(request.path.clone())));
    }

    collect_directory_files(&host_path, &request.path, compiled_glob.as_ref())
}

fn collect_directory_files(
    host_path: &Path,
    sandbox_path: &str,
    compiled_glob: Option<&globset::GlobSet>,
) -> Result<Vec<(PathBuf, String)>, GrepError> {
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

        if let Some(glob_set) = compiled_glob {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                if !glob_set.is_match(name) {
                    continue;
                }
            } else {
                continue;
            }
        }

        let virtual_path = if let Ok(relative) = path.strip_prefix(host_path) {
            if relative.as_os_str().is_empty() {
                sandbox_path.to_string()
            } else {
                let rel_str = relative.to_string_lossy();
                if sandbox_path.ends_with('/') {
                    format!("{sandbox_path}{rel_str}")
                } else {
                    format!("{sandbox_path}/{rel_str}")
                }
            }
        } else {
            continue;
        };

        files.push((path.to_path_buf(), virtual_path));
    }

    Ok(files)
}

fn compile_glob(pattern: &str) -> Result<globset::GlobSet, GrepError> {
    let glob =
        globset::Glob::new(pattern).map_err(|error| GrepError::InvalidGlob(error.to_string()))?;
    let mut builder = globset::GlobSetBuilder::new();
    builder.add(glob);
    builder
        .build()
        .map_err(|error| GrepError::InvalidGlob(error.to_string()))
}

#[cfg(feature = "mod-ripgrep")]
#[derive(Clone)]
pub(crate) struct RipgrepProvider {
    mounts: Arc<MountTable>,
}

#[cfg(feature = "mod-ripgrep")]
impl RipgrepProvider {
    pub(crate) fn new(mounts: Arc<MountTable>) -> Self {
        Self { mounts }
    }
}

#[cfg(feature = "mod-ripgrep")]
impl GrepProvider for RipgrepProvider {
    fn search(&self, request: &GrepRequest) -> Result<Vec<GrepResult>, GrepError> {
        let matcher = build_regex_matcher(&request.pattern, request.mode)?;
        let files_to_search = collect_files(&self.mounts, request)?;

        let mut results = Vec::new();
        let mut match_count: usize = 0;
        let mut seen_files: HashSet<String> = HashSet::new();

        'outer: for (file_host_path, file_virtual_path) in files_to_search {
            if request
                .max_count
                .map(|max| match_count >= max)
                .unwrap_or(false)
            {
                break;
            }

            let mut searcher = grep_searcher::Searcher::new();
            let mut file_matches: Vec<(u64, String, String)> = Vec::new();

            let search_result = searcher.search_path(
                &matcher,
                &file_host_path,
                grep_searcher::sinks::UTF8(|line_num, line_content| {
                    let mut match_text = String::new();
                    use grep_matcher::Matcher;
                    if let Ok(Some(regex_match)) = matcher.find(line_content.as_bytes()) {
                        match_text =
                            line_content[regex_match.start()..regex_match.end()].to_string();
                    }
                    let line_str = line_content
                        .trim_end_matches('\n')
                        .trim_end_matches('\r')
                        .to_string();
                    file_matches.push((line_num, line_str, match_text));
                    Ok(true)
                }),
            );

            if search_result.is_err() {
                continue;
            }

            if request.files_only {
                if !file_matches.is_empty() && !seen_files.contains(&file_virtual_path) {
                    seen_files.insert(file_virtual_path.clone());
                    results.push(GrepResult::File(file_virtual_path));
                    match_count += 1;
                    if request
                        .max_count
                        .map(|max| match_count >= max)
                        .unwrap_or(false)
                    {
                        break 'outer;
                    }
                }
            } else {
                for (line_number, line, match_text) in file_matches {
                    results.push(GrepResult::Match(GrepMatch {
                        file: file_virtual_path.clone(),
                        line_number,
                        line,
                        match_text,
                        column: None,
                    }));
                    match_count += 1;
                    if request
                        .max_count
                        .map(|max| match_count >= max)
                        .unwrap_or(false)
                    {
                        break 'outer;
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(any(feature = "mod-ripgrep", feature = "mod-fff"))]
fn build_regex_matcher(
    pattern: &str,
    mode: GrepMode,
) -> Result<grep_regex::RegexMatcher, GrepError> {
    let mut builder = grep_regex::RegexMatcherBuilder::new();
    if mode == GrepMode::Plain {
        builder.fixed_strings(true);
    }
    builder
        .build(pattern)
        .map_err(|error| GrepError::InvalidPattern(error.to_string()))
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[derive(Clone)]
pub(crate) struct FffGrepProvider {
    mounts: Arc<MountTable>,
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl FffGrepProvider {
    pub(crate) fn fs_compatible(mounts: Arc<MountTable>) -> Self {
        Self { mounts }
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl GrepProvider for FffGrepProvider {
    fn search(&self, request: &GrepRequest) -> Result<Vec<GrepResult>, GrepError> {
        if request.pattern.is_empty() {
            return Err(GrepError::Usage(
                "field 'pattern' must not be empty".to_string(),
            ));
        }

        let files_to_search = collect_files(&self.mounts, request)?;
        let plain_needle = request.pattern.as_bytes().to_vec();
        let regex_matcher = if request.mode == GrepMode::Regex {
            Some(build_regex_matcher(&request.pattern, request.mode)?)
        } else {
            None
        };
        let mut results = Vec::new();
        let mut result_count = 0usize;

        'outer: for (host_file, virtual_file) in files_to_search {
            if request
                .max_count
                .map(|max| result_count >= max)
                .unwrap_or(false)
            {
                break;
            }

            let bytes = match std::fs::read(&host_file) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let remaining = if request.files_only {
                Some(1)
            } else {
                request
                    .max_count
                    .map(|max| max.saturating_sub(result_count))
            };
            let searcher = fff_grep::SearcherBuilder::new().line_number(true).build();
            let sink = match request.mode {
                GrepMode::Plain => {
                    let needle = plain_needle.clone();
                    let mut sink =
                        CollectSink::new(MatchExtractor::Plain(needle.clone()), remaining);
                    searcher
                        .search_slice(LiteralMatcher::new(needle), &bytes, &mut sink)
                        .map_err(|error| GrepError::Search(error.to_string()))?;
                    sink
                }
                GrepMode::Regex => {
                    let matcher = regex_matcher
                        .as_ref()
                        .expect("regex matcher compiled for regex mode")
                        .clone();
                    let mut sink =
                        CollectSink::new(MatchExtractor::Regex(matcher.clone()), remaining);
                    searcher
                        .search_slice(FffRegexMatcher::new(matcher), &bytes, &mut sink)
                        .map_err(|error| GrepError::Search(error.to_string()))?;
                    sink
                }
            };

            if request.files_only {
                if !sink.matches.is_empty() {
                    results.push(GrepResult::File(virtual_file));
                    result_count += 1;
                    if request
                        .max_count
                        .map(|max| result_count >= max)
                        .unwrap_or(false)
                    {
                        break 'outer;
                    }
                }
            } else {
                for line_match in sink.matches {
                    results.push(GrepResult::Match(GrepMatch {
                        file: virtual_file.clone(),
                        line_number: line_match.line_number,
                        line: line_match.line,
                        match_text: line_match.match_text,
                        column: Some(line_match.column),
                    }));
                    result_count += 1;
                    if request
                        .max_count
                        .map(|max| result_count >= max)
                        .unwrap_or(false)
                    {
                        break 'outer;
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[derive(Clone)]
struct LiteralMatcher {
    needle: Vec<u8>,
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl LiteralMatcher {
    fn new(needle: Vec<u8>) -> Self {
        Self { needle }
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl fff_grep::Matcher for LiteralMatcher {
    type Error = fff_grep::NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<fff_grep::Match>, Self::Error> {
        if at > haystack.len() {
            return Ok(None);
        }

        Ok(
            memchr::memmem::find(&haystack[at..], &self.needle).map(|start| {
                let start = start + at;
                fff_grep::Match::new(start, start + self.needle.len())
            }),
        )
    }

    fn line_terminator(&self) -> Option<fff_grep::LineTerminator> {
        if self.needle.contains(&b'\n') {
            None
        } else {
            Some(fff_grep::LineTerminator::byte(b'\n'))
        }
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
#[derive(Clone)]
struct FffRegexMatcher {
    matcher: grep_regex::RegexMatcher,
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl FffRegexMatcher {
    fn new(matcher: grep_regex::RegexMatcher) -> Self {
        Self { matcher }
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl fff_grep::Matcher for FffRegexMatcher {
    type Error = grep_matcher::NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<fff_grep::Match>, Self::Error> {
        use grep_matcher::Matcher;

        self.matcher
            .find_at(haystack, at)
            .map(|match_| match_.map(|match_| fff_grep::Match::new(match_.start(), match_.end())))
    }

    fn line_terminator(&self) -> Option<fff_grep::LineTerminator> {
        use grep_matcher::Matcher;

        self.matcher
            .line_terminator()
            .map(|line_terminator| fff_grep::LineTerminator::byte(line_terminator.as_byte()))
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
struct LineMatch {
    line_number: u64,
    column: usize,
    line: String,
    match_text: String,
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
enum MatchExtractor {
    Plain(Vec<u8>),
    Regex(grep_regex::RegexMatcher),
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl MatchExtractor {
    fn find(&self, line_bytes: &[u8]) -> Option<(usize, usize)> {
        match self {
            MatchExtractor::Plain(needle) => {
                memchr::memmem::find(line_bytes, needle).map(|start| (start, start + needle.len()))
            }
            MatchExtractor::Regex(matcher) => {
                use grep_matcher::Matcher;

                matcher
                    .find(line_bytes)
                    .ok()
                    .flatten()
                    .map(|match_| (match_.start(), match_.end()))
            }
        }
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
struct CollectSink {
    extractor: MatchExtractor,
    max_count: Option<usize>,
    matches: Vec<LineMatch>,
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl CollectSink {
    fn new(extractor: MatchExtractor, max_count: Option<usize>) -> Self {
        Self {
            extractor,
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

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
impl fff_grep::Sink for CollectSink {
    type Error = std::io::Error;

    fn begin(&mut self, _searcher: &fff_grep::Searcher) -> Result<bool, Self::Error> {
        Ok(!self.is_full())
    }

    fn matched(
        &mut self,
        _searcher: &fff_grep::Searcher,
        mat: &fff_grep::SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        if self.is_full() {
            return Ok(false);
        }

        let line_bytes = trim_line_end(mat.bytes());
        let Some((match_start, match_end)) = self.extractor.find(line_bytes) else {
            return Ok(true);
        };
        let column = match_start + 1;
        let match_bytes = line_bytes.get(match_start..match_end);

        let line = match std::str::from_utf8(line_bytes) {
            Ok(line) => line.to_string(),
            Err(_) => return Ok(true),
        };
        let match_text = match match_bytes.and_then(|bytes| std::str::from_utf8(bytes).ok()) {
            Some(match_text) => match_text.to_string(),
            None => return Ok(true),
        };

        self.matches.push(LineMatch {
            line_number: mat.line_number().unwrap_or(0),
            column,
            line,
            match_text,
        });

        Ok(!self.is_full())
    }
}

#[cfg(all(feature = "mod-fff", not(feature = "mod-ripgrep")))]
fn trim_line_end(mut line: &[u8]) -> &[u8] {
    if let Some(stripped) = line.strip_suffix(b"\n") {
        line = stripped;
    }
    if let Some(stripped) = line.strip_suffix(b"\r") {
        line = stripped;
    }
    line
}
