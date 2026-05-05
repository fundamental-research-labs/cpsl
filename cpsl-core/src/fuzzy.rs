//! Fuzzy string matching module for the Luau sandbox.
//!
//! Exposes `fuzzy.ratio`, `fuzzy.partial_ratio`, `fuzzy.token_sort_ratio`,
//! `fuzzy.token_set_ratio`, `fuzzy.extract`, `fuzzy.extractOne`, `fuzzy.distance`
//! as globals. Uses the `rapidfuzz` crate for core similarity/distance metrics.

use crate::lua_util::register_help_functions;
use crate::pyrt_compat::unwrap_py_seq;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue, Value};
use rapidfuzz::distance::levenshtein;
use rapidfuzz::fuzz;

const FUZZY_EXTRACT_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "limit", typ: "number", required: false, description: "Max results to return (default 5)" },
    FieldDoc { name: "cutoff", typ: "number", required: false, description: "Minimum score threshold (0-100, default 0)" },
    FieldDoc { name: "scorer", typ: "string", required: false, description: "Scoring function: ratio (default), partial_ratio, token_sort_ratio, token_set_ratio" },
];

const FUZZY_EXTRACT_ONE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "cutoff", typ: "number", required: false, description: "Minimum score threshold (0-100, default 0)" },
    FieldDoc { name: "scorer", typ: "string", required: false, description: "Scoring function: ratio (default), partial_ratio, token_sort_ratio, token_set_ratio" },
];

pub(crate) static FUZZY_DOC: ModuleDoc = ModuleDoc {
    name: "fuzzy",
    summary: "Fuzzy string matching & similarity",
    functions: &[
        FnDoc {
            name: "ratio",
            description: "Similarity ratio between two strings (0-100).",
            params: &[
                Param {
                    name: "s1",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "s2",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "partial_ratio",
            description: "Best substring match ratio (0-100). Finds the optimal alignment of the shorter string within the longer one.",
            params: &[
                Param {
                    name: "s1",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "s2",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "token_sort_ratio",
            description: "Token-sorted similarity ratio (0-100). Splits into tokens, sorts alphabetically, then computes ratio.",
            params: &[
                Param {
                    name: "s1",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "s2",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "token_set_ratio",
            description: "Token-set similarity ratio (0-100). Compares the intersection and differences of token sets.",
            params: &[
                Param {
                    name: "s1",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "s2",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "extract",
            description:
                "Top-N matches from choices. Returns list of {choice, score, index}.",
            params: &[
                Param {
                    name: "query",
                    short: Some('q'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "choices",
                    short: Some('c'),
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(FUZZY_EXTRACT_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Table,
            example: Some(r#"fuzzy.extract({query="apple", choices={"apple pie","pineapple","banana"}, limit=2})"#),
        },
        FnDoc {
            name: "extractOne",
            description:
                "Best match from choices. Returns {choice, score, index} or nil.",
            params: &[
                Param {
                    name: "query",
                    short: Some('q'),
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "choices",
                    short: Some('c'),
                    typ: ParamType::Table,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "opts",
                    short: None,
                    typ: ParamType::Table,
                    required: false,
                    fields: Some(FUZZY_EXTRACT_ONE_OPTS_FIELDS),
                },
            ],
            returns: ReturnType::Value,
            example: Some(r#"fuzzy.extractOne({query="apple", choices={"apple pie","pineapple","banana"}})"#),
        },
        FnDoc {
            name: "distance",
            description: "Levenshtein edit distance between two strings.",
            params: &[
                Param {
                    name: "s1",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "s2",
                    short: None,
                    typ: ParamType::String,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
    ],
};

/// Compute simple ratio (0-100) using rapidfuzz.
fn compute_ratio(s1: &str, s2: &str) -> f64 {
    fuzz::ratio(s1.chars(), s2.chars()) * 100.0
}

/// Compute partial ratio (0-100): best substring match of shorter in longer.
fn compute_partial_ratio(s1: &str, s2: &str) -> f64 {
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }
    if s1 == s2 {
        return 100.0;
    }

    let (shorter, longer) = if s1.chars().count() <= s2.chars().count() {
        (s1, s2)
    } else {
        (s2, s1)
    };

    let short_len = shorter.chars().count();
    let long_chars: Vec<char> = longer.chars().collect();
    let long_len = long_chars.len();

    if short_len == 0 {
        return 0.0;
    }

    let mut best = 0.0f64;
    for start in 0..=(long_len - short_len) {
        let substr: String = long_chars[start..start + short_len].iter().collect();
        let score = fuzz::ratio(shorter.chars(), substr.chars()) * 100.0;
        if score > best {
            best = score;
            if (best - 100.0).abs() < f64::EPSILON {
                break;
            }
        }
    }
    best
}

/// Compute token-sorted ratio (0-100): sort tokens alphabetically, then ratio.
fn compute_token_sort_ratio(s1: &str, s2: &str) -> f64 {
    let sorted1 = sort_tokens(s1);
    let sorted2 = sort_tokens(s2);
    compute_ratio(&sorted1, &sorted2)
}

fn sort_tokens(s: &str) -> String {
    let mut tokens: Vec<&str> = s.split_whitespace().collect();
    tokens.sort_unstable();
    tokens.join(" ")
}

/// Compute token-set ratio (0-100): compare intersection and differences of token sets.
fn compute_token_set_ratio(s1: &str, s2: &str) -> f64 {
    let tokens1: std::collections::BTreeSet<&str> = s1.split_whitespace().collect();
    let tokens2: std::collections::BTreeSet<&str> = s2.split_whitespace().collect();

    let intersection: Vec<&str> = tokens1.intersection(&tokens2).copied().collect();
    let diff1: Vec<&str> = tokens1.difference(&tokens2).copied().collect();
    let diff2: Vec<&str> = tokens2.difference(&tokens1).copied().collect();

    let sorted_sect = intersection.join(" ");
    let combined1 = if diff1.is_empty() {
        sorted_sect.clone()
    } else {
        format!("{} {}", sorted_sect, diff1.join(" "))
    };
    let combined2 = if diff2.is_empty() {
        sorted_sect.clone()
    } else {
        format!("{} {}", sorted_sect, diff2.join(" "))
    };

    // Compare all three pairs, return the best
    let r1 = compute_ratio(&sorted_sect, &combined1);
    let r2 = compute_ratio(&sorted_sect, &combined2);
    let r3 = compute_ratio(&combined1, &combined2);
    r1.max(r2).max(r3)
}

/// Scorer enum for extract/extractOne.
enum Scorer {
    Ratio,
    PartialRatio,
    TokenSortRatio,
    TokenSetRatio,
}

impl Scorer {
    fn from_str(s: &str) -> Result<Self, mlua::Error> {
        match s {
            "ratio" => Ok(Scorer::Ratio),
            "partial_ratio" => Ok(Scorer::PartialRatio),
            "token_sort_ratio" => Ok(Scorer::TokenSortRatio),
            "token_set_ratio" => Ok(Scorer::TokenSetRatio),
            _ => Err(mlua::Error::external(format!(
                "fuzzy: unknown scorer '{}'. Valid scorers: ratio, partial_ratio, token_sort_ratio, token_set_ratio",
                s
            ))),
        }
    }

    fn score(&self, s1: &str, s2: &str) -> f64 {
        match self {
            Scorer::Ratio => compute_ratio(s1, s2),
            Scorer::PartialRatio => compute_partial_ratio(s1, s2),
            Scorer::TokenSortRatio => compute_token_sort_ratio(s1, s2),
            Scorer::TokenSetRatio => compute_token_set_ratio(s1, s2),
        }
    }
}

/// Register `fuzzy.*` globals in the Lua VM.
pub fn register_fuzzy_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let fuzzy_table = lua.create_table()?;

    // fuzzy.ratio(s1, s2) -> number (0-100)
    fuzzy_table.set(
        "ratio",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FUZZY_DOC.params("ratio"), "fuzzy.ratio")?;
            let s1 = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let s2 = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(round_score(compute_ratio(&s1, &s2)))
        })?,
    )?;

    // fuzzy.partial_ratio(s1, s2) -> number (0-100)
    fuzzy_table.set(
        "partial_ratio",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("partial_ratio"), "fuzzy.partial_ratio")?;
            let s1 = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let s2 = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(round_score(compute_partial_ratio(&s1, &s2)))
        })?,
    )?;

    // fuzzy.token_sort_ratio(s1, s2) -> number (0-100)
    fuzzy_table.set(
        "token_sort_ratio",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("token_sort_ratio"), "fuzzy.token_sort_ratio")?;
            let s1 = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let s2 = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(round_score(compute_token_sort_ratio(&s1, &s2)))
        })?,
    )?;

    // fuzzy.token_set_ratio(s1, s2) -> number (0-100)
    fuzzy_table.set(
        "token_set_ratio",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("token_set_ratio"), "fuzzy.token_set_ratio")?;
            let s1 = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let s2 = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            Ok(round_score(compute_token_set_ratio(&s1, &s2)))
        })?,
    )?;

    // fuzzy.extract(query, choices, opts?) -> table of {choice, score, index}
    fuzzy_table.set(
        "extract",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("extract"), "fuzzy.extract")?;
            let query = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let choices_raw = match &validated[1] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };
            // Unwrap py.list/py.tuple if the choices come from Python transpiled code
            let choices_table = unwrap_py_seq(&choices_raw)?;
            let opts = match &validated[2] {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            };

            let limit = opts
                .as_ref()
                .and_then(|t| t.get::<i32>("limit").ok().or_else(|| t.get::<i32>("l").ok()))
                .unwrap_or(5) as usize;
            let cutoff = opts
                .as_ref()
                .and_then(|t| {
                    t.get::<f64>("cutoff")
                        .ok()
                        .or_else(|| t.get::<f64>("c").ok())
                })
                .unwrap_or(0.0);
            let scorer_name = opts
                .as_ref()
                .and_then(|t| {
                    t.get::<String>("scorer")
                        .ok()
                        .or_else(|| t.get::<String>("s").ok())
                });
            let scorer = match scorer_name {
                Some(ref name) => Scorer::from_str(name)?,
                None => Scorer::Ratio,
            };

            // Collect choices from the Lua table (1-based array)
            let len = choices_table.raw_len();
            let mut results: Vec<(String, f64, usize)> = Vec::new();
            for i in 1..=len {
                let choice: String = choices_table.get(i)?;
                let score = scorer.score(&query, &choice);
                if score >= cutoff {
                    results.push((choice, score, i));
                }
            }

            // Sort by score descending, then by index ascending for stability
            results.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.2.cmp(&b.2))
            });
            results.truncate(limit);

            let out = lua.create_table()?;
            for (idx, (choice, score, orig_idx)) in results.iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("choice", choice.as_str())?;
                entry.set("score", round_score(*score))?;
                entry.set("index", *orig_idx as i32)?;
                out.set(idx + 1, entry)?;
            }
            Ok(Value::Table(out))
        })?,
    )?;

    // fuzzy.extractOne(query, choices, opts?) -> {choice, score, index} or nil
    fuzzy_table.set(
        "extractOne",
        lua.create_function(|lua, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("extractOne"), "fuzzy.extractOne")?;
            let query = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let choices_raw = match &validated[1] {
                Value::Table(t) => t.clone(),
                _ => unreachable!(),
            };
            // Unwrap py.list/py.tuple if the choices come from Python transpiled code
            let choices_table = unwrap_py_seq(&choices_raw)?;
            let opts = match &validated[2] {
                Value::Table(t) => Some(t.clone()),
                _ => None,
            };

            let cutoff = opts
                .as_ref()
                .and_then(|t| {
                    t.get::<f64>("cutoff")
                        .ok()
                        .or_else(|| t.get::<f64>("c").ok())
                })
                .unwrap_or(0.0);
            let scorer_name = opts
                .as_ref()
                .and_then(|t| {
                    t.get::<String>("scorer")
                        .ok()
                        .or_else(|| t.get::<String>("s").ok())
                });
            let scorer = match scorer_name {
                Some(ref name) => Scorer::from_str(name)?,
                None => Scorer::Ratio,
            };

            let len = choices_table.raw_len();
            let mut best: Option<(String, f64, usize)> = None;
            for i in 1..=len {
                let choice: String = choices_table.get(i)?;
                let score = scorer.score(&query, &choice);
                if score >= cutoff {
                    if best.is_none() || score > best.as_ref().unwrap().1 {
                        best = Some((choice, score, i));
                    }
                }
            }

            match best {
                Some((choice, score, idx)) => {
                    let entry = lua.create_table()?;
                    entry.set("choice", choice)?;
                    entry.set("score", round_score(score))?;
                    entry.set("index", idx as i32)?;
                    Ok(Value::Table(entry))
                }
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // fuzzy.distance(s1, s2) -> number (Levenshtein edit distance)
    fuzzy_table.set(
        "distance",
        lua.create_function(|_, args: MultiValue| {
            let validated =
                validate_args(&args, FUZZY_DOC.params("distance"), "fuzzy.distance")?;
            let s1 = match &validated[0] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let s2 = match &validated[1] {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => unreachable!(),
            };
            let dist = levenshtein::distance(s1.chars(), s2.chars());
            Ok(dist as i32)
        })?,
    )?;

    register_help_functions(lua, &fuzzy_table, &FUZZY_DOC)?;

    lua.globals().set("fuzzy", fuzzy_table)?;
    wrap_module_with_help_hints(lua, "fuzzy")?;

    Ok(())
}

/// Round a score to 2 decimal places for clean output.
fn round_score(score: f64) -> f64 {
    (score * 100.0).round() / 100.0
}
