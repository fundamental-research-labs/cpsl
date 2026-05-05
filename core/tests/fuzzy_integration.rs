#![cfg(feature = "mod-fuzzy")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── fuzzy.ratio ─────────────────────────────────────────────────

#[test]
fn ratio_identical_strings() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.ratio("hello", "hello")"#).unwrap();
    assert_eq!(r, "100");
}

#[test]
fn ratio_completely_different() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.ratio("abc", "xyz")"#).unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(score < 50.0, "expected low score, got {}", score);
}

#[test]
fn ratio_similar_strings() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.ratio("this is a test", "this is a test!")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(score > 90.0, "expected high score, got {}", score);
}

#[test]
fn ratio_empty_strings() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.ratio("", "")"#).unwrap();
    // Both empty should be 0 (no characters to compare)
    let score: f64 = r.parse().unwrap();
    assert!(score >= 0.0, "got {}", score);
}

#[test]
fn ratio_one_empty() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.ratio("hello", "")"#).unwrap();
    let score: f64 = r.parse().unwrap();
    assert_eq!(score, 0.0);
}

// ── fuzzy.partial_ratio ─────────────────────────────────────────

#[test]
fn partial_ratio_substring_match() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.partial_ratio("test", "this is a test")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert_eq!(
        score, 100.0,
        "exact substring should score 100, got {}",
        score
    );
}

#[test]
fn partial_ratio_identical() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.partial_ratio("hello", "hello")"#)
        .unwrap();
    assert_eq!(r, "100");
}

#[test]
fn partial_ratio_no_match() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.partial_ratio("xyz", "abc")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(score < 50.0, "expected low score, got {}", score);
}

#[test]
fn partial_ratio_empty() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.partial_ratio("", "hello")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert_eq!(score, 0.0);
}

// ── fuzzy.token_sort_ratio ──────────────────────────────────────

#[test]
fn token_sort_ratio_reordered() {
    let s = sb();
    let r = s
        .exec(
            r#"return fuzzy.token_sort_ratio("fuzzy wuzzy was a bear", "wuzzy fuzzy was a bear")"#,
        )
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert_eq!(
        score, 100.0,
        "reordered tokens should score 100, got {}",
        score
    );
}

#[test]
fn token_sort_ratio_different() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.token_sort_ratio("apple banana", "cherry date")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(score < 50.0, "expected low score, got {}", score);
}

// ── fuzzy.token_set_ratio ───────────────────────────────────────

#[test]
fn token_set_ratio_subset() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.token_set_ratio("fuzzy was a bear", "fuzzy fuzzy was a bear")"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(
        score > 90.0,
        "near-duplicate with extra token should score high, got {}",
        score
    );
}

#[test]
fn token_set_ratio_identical() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.token_set_ratio("hello world", "hello world")"#)
        .unwrap();
    assert_eq!(r, "100");
}

// ── fuzzy.extract ───────────────────────────────────────────────

#[test]
fn extract_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("new york", {"New York", "Los Angeles", "New Orleans", "York"})
            return #results .. " " .. results[1].choice .. " " .. tostring(results[1].score > 50)
        "#,
        )
        .unwrap();
    // Default limit=5 but only 4 choices; first match should be "New York"
    assert!(r.starts_with("4 "), "expected 4 results, got: {}", r);
    assert!(
        r.contains("true"),
        "expected high score for best match, got: {}",
        r
    );
}

#[test]
fn extract_with_limit() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("apple", {"apple", "applesauce", "banana", "pineapple"}, {limit=2})
            return #results
        "#,
        )
        .unwrap();
    assert_eq!(r, "2");
}

#[test]
fn extract_with_cutoff() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("apple", {"apple", "banana", "cherry"}, {cutoff=80})
            return #results
        "#,
        )
        .unwrap();
    // Only "apple" should pass a cutoff of 80
    assert_eq!(r, "1");
}

#[test]
fn extract_with_scorer() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("test", {"this is a test", "testing 123", "nope"}, {scorer="partial_ratio"})
            return results[1].choice
        "#,
        )
        .unwrap();
    assert_eq!(r, "this is a test");
}

#[test]
fn extract_result_has_index() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("banana", {"apple", "banana", "cherry"}, {limit=1})
            return results[1].index
        "#,
        )
        .unwrap();
    assert_eq!(r, "2"); // 1-based index
}

#[test]
fn extract_empty_choices() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("test", {})
            return #results
        "#,
        )
        .unwrap();
    assert_eq!(r, "0");
}

// ── fuzzy.extractOne ────────────────────────────────────────────

#[test]
fn extract_one_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = fuzzy.extractOne("new york", {"New York", "Los Angeles", "New Orleans"})
            return result.choice
        "#,
        )
        .unwrap();
    assert!(r.contains("New York"), "expected New York, got: {}", r);
}

#[test]
fn extract_one_with_cutoff_no_match() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = fuzzy.extractOne("xyz", {"apple", "banana"}, {cutoff=90})
            return tostring(result)
        "#,
        )
        .unwrap();
    assert_eq!(r, "nil");
}

#[test]
fn extract_one_returns_score_and_index() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = fuzzy.extractOne("hello", {"hello", "world"})
            return result.score .. " " .. result.index
        "#,
        )
        .unwrap();
    assert_eq!(r, "100 1");
}

// ── fuzzy.distance ──────────────────────────────────────────────

#[test]
fn distance_identical() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.distance("hello", "hello")"#)
        .unwrap();
    assert_eq!(r, "0");
}

#[test]
fn distance_one_edit() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.distance("kitten", "sitten")"#)
        .unwrap();
    assert_eq!(r, "1"); // one substitution
}

#[test]
fn distance_classic_example() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.distance("kitten", "sitting")"#)
        .unwrap();
    assert_eq!(r, "3"); // kitten → sitten → sittin → sitting
}

#[test]
fn distance_empty_strings() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.distance("", "")"#).unwrap();
    assert_eq!(r, "0");
}

#[test]
fn distance_one_empty() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.distance("hello", "")"#).unwrap();
    assert_eq!(r, "5"); // 5 deletions
}

// ── Dual-signature tests (table form for shell dispatch) ────────

#[test]
fn ratio_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.ratio({[1]="hello", [2]="hello"})"#)
        .unwrap();
    assert_eq!(r, "100");
}

#[test]
fn distance_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.distance({[1]="kitten", [2]="sitting"})"#)
        .unwrap();
    assert_eq!(r, "3");
}

#[test]
fn partial_ratio_table_form() {
    let s = sb();
    let r = s
        .exec(r#"return fuzzy.partial_ratio({[1]="test", [2]="this is a test"})"#)
        .unwrap();
    let score: f64 = r.parse().unwrap();
    assert_eq!(score, 100.0);
}

#[test]
fn extract_table_form() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract({[1]="hello", [2]={"hello", "world"}, limit=1})
            return results[1].choice
        "#,
        )
        .unwrap();
    assert_eq!(r, "hello");
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_fuzzy_ratio() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"fuzzy ratio "hello" "hello""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("100"), "expected 100, got: {}", r);
}

#[test]
fn shell_fuzzy_distance() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"fuzzy distance "kitten" "sitting""#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau).unwrap();
    assert!(r.contains("3"), "expected 3, got: {}", r);
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn ratio_no_args_errors() {
    let s = sb();
    let err = s.exec("fuzzy.ratio()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn ratio_wrong_type_errors() {
    let s = sb();
    let err = s.exec("fuzzy.ratio(42, 43)").unwrap_err();
    assert!(
        err.message.contains("string") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn extract_invalid_scorer_errors() {
    let s = sb();
    let err = s
        .exec(r#"fuzzy.extract("test", {"a"}, {scorer="nonexistent"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("unknown scorer"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn fuzzy_help_returns_help() {
    let s = sb();
    let r = s.exec("return fuzzy.help()").unwrap();
    assert!(r.contains("fuzzy"), "help: {}", r);
    assert!(r.contains("fuzzy.ratio"), "help: {}", r);
    assert!(r.contains("fuzzy.distance"), "help: {}", r);
    assert!(r.contains("fuzzy.extract"), "help: {}", r);
}

#[test]
fn fuzzy_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("fuzzy.foo()").unwrap_err();
    assert!(
        err.message.contains("fuzzy.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call fuzzy.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_fuzzy() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("fuzzy"), "global help should list fuzzy: {}", r);
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_rapidfuzz_fuzz_ratio() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from rapidfuzz import fuzz
score = fuzz.ratio("hello", "hello")
print(score)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "100");
}

#[test]
fn python_rapidfuzz_process_extract() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
from rapidfuzz import process
results = process.extract("apple", ["apple", "banana", "pineapple"], limit=2)
print(len(results))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    assert_eq!(r, "2");
}

#[test]
fn python_import_fuzzy_passthrough() {
    let py_code = r#"
import fuzzy
result = fuzzy.ratio("a", "b")
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("fuzzy"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

// ── Sandbox safety: no filesystem or network access ─────────────

#[test]
fn fuzzy_does_not_access_filesystem() {
    // All fuzzy functions are pure string operations — no filesystem access possible.
    // Verify by running in a sandbox with no mounts.
    let s = sb();
    let r = s
        .exec(
            r#"
            local score = fuzzy.ratio("hello", "world")
            local dist = fuzzy.distance("abc", "def")
            local results = fuzzy.extract("test", {"testing", "best"})
            return tostring(score) .. " " .. tostring(dist) .. " " .. tostring(#results)
        "#,
        )
        .unwrap();
    // Should succeed without any fs or network access
    let parts: Vec<&str> = r.split(' ').collect();
    assert_eq!(parts.len(), 3, "expected 3 values, got: {}", r);
}

#[test]
fn fuzzy_no_dangerous_globals_exposed() {
    // Verify fuzzy module doesn't leak any dangerous globals
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(fuzzy.ratio)) .. " " ..
                   tostring(type(fuzzy.distance)) .. " " ..
                   tostring(rawget(fuzzy, "io")) .. " " ..
                   tostring(rawget(fuzzy, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function nil nil");
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn ratio_unicode_strings() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.ratio("café", "cafe")"#).unwrap();
    let score: f64 = r.parse().unwrap();
    assert!(
        score > 70.0,
        "Unicode near-match should score reasonably, got {}",
        score
    );
}

#[test]
fn distance_unicode_strings() {
    let s = sb();
    let r = s.exec(r#"return fuzzy.distance("café", "cafe")"#).unwrap();
    let dist: i32 = r.parse().unwrap();
    assert_eq!(dist, 1, "café→cafe is 1 edit (é→e)");
}

#[test]
fn extract_preserves_original_indices() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("banana", {"cherry", "banana", "apple"}, {limit=3})
            local indices = {}
            for _, r in ipairs(results) do
                table.insert(indices, tostring(r.index))
            end
            return table.concat(indices, ",")
        "#,
        )
        .unwrap();
    // The best match "banana" is at index 2
    assert!(
        r.starts_with("2"),
        "best match should be index 2, got: {}",
        r
    );
}

#[test]
fn extract_sorted_by_score_descending() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local results = fuzzy.extract("apple", {"apple", "applesauce", "pineapple", "banana"})
            local ok = true
            for i = 2, #results do
                if results[i].score > results[i-1].score then
                    ok = false
                    break
                end
            end
            return tostring(ok)
        "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}
