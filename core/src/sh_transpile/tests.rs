//! Tests for shell-to-Luau transpilation and shell runtime behavior.

use super::*;

fn transpile(input: &str) -> String {
    transpile_sh(input).unwrap().luau_source
}

#[test]
fn test_echo_simple() {
    let result = transpile("echo hello");
    assert!(result.contains("sh.echo(\"hello\")"), "got: {}", result);
}

#[test]
fn test_echo_quoted() {
    let result = transpile("echo \"hello world\"");
    assert!(
        result.contains("sh.echo(\"hello world\")"),
        "got: {}",
        result
    );
}

#[test]
fn test_ls_path() {
    let result = transpile("ls /workspace");
    assert!(
        result.contains("sh.ls({[1]=\"/workspace\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_ls_no_args() {
    let result = transpile("ls");
    assert!(result.contains("sh.ls()"), "got: {}", result);
}

#[test]
fn test_pipe_two() {
    let result = transpile("ls /workspace | grep foo");
    assert!(result.contains("sh.pipe("), "got: {}", result);
    assert!(
        result.contains("sh.ls({[1]=\"/workspace\"})"),
        "got: {}",
        result
    );
    assert!(
        result.contains("sh.grep({input=_in, [1]=\"foo\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_pipe_three() {
    let result = transpile("cat /workspace/data.csv | grep pattern | head -5");
    assert!(result.contains("sh.pipe("), "got: {}", result);
    assert!(
        result.contains("sh.cat(\"/workspace/data.csv\")"),
        "got: {}",
        result
    );
    assert!(
        result.contains("sh.grep({input=_in, [1]=\"pattern\"})"),
        "got: {}",
        result
    );
    assert!(
        result.contains("sh.head({input=_in, n=5})"),
        "got: {}",
        result
    );
}

#[test]
fn test_variable_assignment() {
    let result = transpile("NAME=\"world\"");
    assert!(result.contains("local NAME = \"world\""), "got: {}", result);
}

#[test]
fn test_variable_expansion() {
    let result = transpile("NAME=\"world\"\necho \"hello $NAME\"");
    assert!(result.contains("local NAME = \"world\""), "got: {}", result);
    assert!(
        result.contains("sh.echo(\"hello \" .. NAME)"),
        "got: {}",
        result
    );
}

#[test]
fn test_pwd() {
    let result = transpile("pwd");
    assert!(result.contains("sh.pwd()"), "got: {}", result);
}

#[test]
fn test_unknown_command() {
    // Unknown command with args: first arg is treated as method, rest as table args
    let result = transpile("mycmd arg1 arg2");
    assert!(
        result.contains("sh.run(\"mycmd\", \"arg1\", {[1]=\"arg2\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_unknown_command_bare() {
    // Bare unknown command: sh.run("mycmd", nil, nil)
    let result = transpile("mycmd");
    assert!(
        result.contains("sh.run(\"mycmd\", nil, nil)"),
        "got: {}",
        result
    );
}

#[test]
fn test_redirect_write() {
    let result = transpile("echo hello > /workspace/out.txt");
    assert!(result.contains("sh.redirect_write("), "got: {}", result);
    assert!(
        result.contains("function() return"),
        "should wrap in function, got: {}",
        result
    );
}

#[test]
fn test_single_quoted_string() {
    let result = transpile("echo 'hello world'");
    assert!(
        result.contains("sh.echo(\"hello world\")"),
        "got: {}",
        result
    );
}

#[test]
fn test_mkdir() {
    let result = transpile("mkdir -p /workspace/subdir");
    assert!(
        result.contains("sh.mkdir({p=true, [1]=\"/workspace/subdir\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_cd() {
    let result = transpile("cd /workspace/subdir");
    assert!(
        result.contains("sh.cd(\"/workspace/subdir\")"),
        "got: {}",
        result
    );
}

#[test]
fn test_head_n_flag() {
    let result = transpile("head -n 3 /workspace/file.txt");
    assert!(result.contains("sh.head("), "got: {}", result);
    assert!(result.contains("3"), "should have n=3, got: {}", result);
}

#[test]
fn test_cp_mv() {
    let result = transpile("cp /workspace/a.txt /workspace/b.txt");
    assert!(
        result.contains("sh.cp({[1]=\"/workspace/a.txt\", [2]=\"/workspace/b.txt\"})"),
        "got: {}",
        result
    );
    let result = transpile("mv /workspace/a.txt /workspace/b.txt");
    assert!(
        result.contains("sh.mv(\"/workspace/a.txt\", \"/workspace/b.txt\")"),
        "got: {}",
        result
    );
}

#[test]
fn test_test_command() {
    let result = transpile("test -f /workspace/file.txt");
    assert!(
        result.contains("sh.test(\"-f\", \"/workspace/file.txt\")"),
        "got: {}",
        result
    );
}

// ── End-to-end tests (transpile + execute in sandbox) ─────────

const SHRT_SOURCE: &str = include_str!("../../../runtime/shrt.luau");

fn exec_sh(source: &str) -> String {
    let transpiled = transpile_sh(source).expect("transpile failed");
    let sandbox = crate::Sandbox::new().expect("sandbox creation failed");
    sandbox
        .register_module("shrt", SHRT_SOURCE)
        .expect("shrt load failed");
    sandbox.exec(&transpiled.luau_source).expect("exec failed")
}

#[test]
fn test_e2e_echo() {
    let output = exec_sh("echo hello world");
    assert_eq!(output, "hello world");
}

#[test]
fn test_e2e_echo_quoted() {
    let output = exec_sh("echo \"hello world\"");
    assert_eq!(output, "hello world");
}

#[test]
fn test_e2e_write_denied_errors_use_shell_message() {
    let output = exec_sh("touch /proc/newfile");
    assert!(
        output.contains("/proc is read-only"),
        "should explain that /proc is read-only, got: {}",
        output
    );
    assert!(
        !output.contains("function is not defined"),
        "should not leak Luau nil-function errors, got: {}",
        output
    );
}

#[test]
fn test_e2e_missing_file_errors_use_shell_message() {
    let output = exec_sh("cat /missing");
    assert!(
        output.contains("No such file or directory"),
        "should explain missing file, got: {}",
        output
    );
    assert!(
        !output.contains("function is not defined"),
        "should not leak Luau nil-function errors, got: {}",
        output
    );
}

#[test]
fn test_e2e_variable() {
    let output = exec_sh("NAME=\"world\"\necho \"hello $NAME\"");
    assert_eq!(output, "hello world");
}

#[test]
fn test_e2e_pwd() {
    let output = exec_sh("pwd");
    assert_eq!(output, "/");
}

#[test]
fn test_e2e_pipe_head() {
    // echo produces multiline output, pipe through head
    let output = exec_sh("echo \"line1\nline2\nline3\" | head -2");
    assert_eq!(output, "line1\nline2");
}

#[test]
fn test_e2e_pipe_grep() {
    let output = exec_sh("echo \"apple\nbanana\ncherry\" | grep banana");
    assert_eq!(output, "banana");
}

#[test]
fn test_e2e_pipe_sort() {
    let output = exec_sh("echo \"cherry\napple\nbanana\" | sort");
    assert_eq!(output, "apple\nbanana\ncherry");
}

#[test]
fn test_e2e_pipe_uniq() {
    let output = exec_sh("echo \"a\na\nb\nb\nc\" | uniq");
    assert_eq!(output, "a\nb\nc");
}

#[test]
fn test_e2e_wc_lines() {
    let output = exec_sh("echo \"a\nb\nc\" | wc -l");
    assert_eq!(output, "3");
}

#[test]
fn test_tree_d_flag() {
    let result = transpile("tree /workspace -d");
    assert!(
        result.contains("d=true"),
        "expected d=true in output, got: {}",
        result
    );
    assert!(
        result.contains("[1]=\"/workspace\""),
        "expected positional path, got: {}",
        result
    );
}

#[test]
fn test_tree_d_and_l_flags() {
    let result = transpile("tree /workspace -d -L 3");
    assert!(
        result.contains("d=true"),
        "expected d=true, got: {}",
        result
    );
    assert!(
        result.contains("L=\"3\""),
        "expected L=\"3\", got: {}",
        result
    );
    assert!(
        result.contains("[1]=\"/workspace\""),
        "expected positional path, got: {}",
        result
    );
}

// ── Phase 2 tests ─────────────────────────────────────────────

#[test]
fn test_e2e_env_set_get() {
    // sh.set/sh.get work via transpiled variable assignment
    let output = exec_sh("FOO=\"bar\"\necho $FOO");
    assert_eq!(output, "bar");
}

#[test]
fn test_e2e_variable_reassignment() {
    let output = exec_sh("X=\"first\"\nX=\"second\"\necho $X");
    assert_eq!(output, "second");
}

#[test]
fn test_e2e_variable_in_double_quotes() {
    let output = exec_sh("GREETING=\"hello\"\necho \"$GREETING world\"");
    assert_eq!(output, "hello world");
}

#[test]
fn test_e2e_single_quotes_no_expansion() {
    // Single quotes should not expand variables
    let output = exec_sh("NAME=\"world\"\necho 'hello $NAME'");
    assert_eq!(output, "hello $NAME");
}

#[test]
fn test_command_substitution() {
    let result = transpile("echo $(pwd)");
    assert!(result.contains("sh.echo(sh.capture("), "got: {}", result);
    assert!(result.contains("sh.pwd()"), "got: {}", result);
}

#[test]
fn test_e2e_command_substitution() {
    let output = exec_sh("echo $(pwd)");
    assert_eq!(output, "/");
}

#[test]
fn test_e2e_command_substitution_in_string() {
    let output = exec_sh("echo \"dir: $(pwd)\"");
    assert_eq!(output, "dir: /");
}

#[test]
fn test_parameter_default() {
    // ${VAR:-default} when unset
    let result = transpile("echo ${UNSET:-fallback}");
    assert!(
        result.contains("UNSET") && result.contains("fallback"),
        "got: {}",
        result
    );
}

#[test]
fn test_redirect_append() {
    let result = transpile("echo hello >> /workspace/out.txt");
    assert!(result.contains("sh.redirect_append("), "got: {}", result);
    assert!(
        result.contains("function() return"),
        "should wrap in function, got: {}",
        result
    );
}

#[test]
fn test_redirect_stderr_merge() {
    // 2>&1 should not cause a parse error — it's just ignored in our sandbox
    let result = transpile("echo hello 2>&1");
    assert!(result.contains("sh.echo"), "got: {}", result);
}

#[test]
fn test_input_redirect() {
    let result = transpile("cat < /workspace/data.txt");
    assert!(result.contains("fs.read("), "got: {}", result);
}

#[test]
fn test_glob_in_rm() {
    let result = transpile("rm *.txt");
    assert!(
        result.contains("sh.glob("),
        "should emit sh.glob(), got: {}",
        result
    );
    assert!(
        result.contains("*.txt"),
        "should have the pattern, got: {}",
        result
    );
}

#[test]
fn test_glob_detection() {
    let result = transpile("echo *.csv");
    assert!(
        result.contains("sh.glob("),
        "echo with glob should expand, got: {}",
        result
    );
}

#[test]
fn test_find_command() {
    let result = transpile("find /workspace -name '*.csv' -type f");
    assert!(result.contains("sh.find("), "got: {}", result);
    assert!(result.contains("name=\"*.csv\""), "got: {}", result);
    assert!(result.contains("type=\"f\""), "got: {}", result);
    assert!(result.contains("[1]=\"/workspace\""), "got: {}", result);
}

#[test]
fn test_tee_command() {
    let result = transpile("echo hello | tee /workspace/out.txt");
    assert!(result.contains("sh.tee(_in,"), "got: {}", result);
}

#[test]
fn test_tee_standalone() {
    let result = transpile("tee /workspace/out.txt");
    assert!(result.contains("sh.tee("), "got: {}", result);
}

// ── Phase 3a: if/elif/else/fi ────────────────────────────────

#[test]
fn test_if_simple() {
    let result = transpile("if [ -f /workspace/data.csv ]; then\n  echo found\nfi");
    assert!(
        result.contains("if sh.test(\"-f\", \"/workspace/data.csv\") then"),
        "got: {}",
        result
    );
    assert!(result.contains("sh.echo(\"found\")"), "got: {}", result);
    assert!(result.contains("end"), "got: {}", result);
}

#[test]
fn test_if_else() {
    let result = transpile("if [ -d /workspace ]; then\n  echo yes\nelse\n  echo no\nfi");
    assert!(
        result.contains("if sh.test(\"-d\", \"/workspace\") then"),
        "got: {}",
        result
    );
    assert!(result.contains("else"), "got: {}", result);
    assert!(result.contains("end"), "got: {}", result);
}

#[test]
fn test_if_elif_else() {
    let result = transpile(
        "if [ -f /a ]; then\n  echo file\nelif [ -d /b ]; then\n  echo dir\nelse\n  echo none\nfi",
    );
    assert!(
        result.contains("if sh.test(\"-f\", \"/a\") then"),
        "got: {}",
        result
    );
    assert!(
        result.contains("elseif sh.test(\"-d\", \"/b\") then"),
        "got: {}",
        result
    );
    assert!(result.contains("else"), "got: {}", result);
}

#[test]
fn test_e2e_if_true() {
    // PWD is always / in sandbox
    let output = exec_sh("if [ -n \"hello\" ]; then\n  echo yes\nelse\n  echo no\nfi");
    assert_eq!(output, "yes");
}

#[test]
fn test_e2e_if_false() {
    let output = exec_sh("if [ -z \"hello\" ]; then\n  echo yes\nelse\n  echo no\nfi");
    assert_eq!(output, "no");
}

#[test]
fn test_e2e_if_elif() {
    let output = exec_sh(
            "X=\"two\"\nif [ \"$X\" = \"one\" ]; then\n  echo 1\nelif [ \"$X\" = \"two\" ]; then\n  echo 2\nelse\n  echo other\nfi"
        );
    assert_eq!(output, "2");
}

// ── Phase 3b: sh.test() runtime ──────────────────────────────

#[test]
fn test_e2e_test_string_empty() {
    let output = exec_sh("if [ -z \"\" ]; then\n  echo empty\nelse\n  echo notempty\nfi");
    assert_eq!(output, "empty");
}

#[test]
fn test_e2e_test_string_nonempty() {
    let output = exec_sh("if [ -n \"hello\" ]; then\n  echo notempty\nelse\n  echo empty\nfi");
    assert_eq!(output, "notempty");
}

#[test]
fn test_e2e_test_string_equal() {
    let output = exec_sh("if [ \"abc\" = \"abc\" ]; then\n  echo match\nfi");
    assert_eq!(output, "match");
}

#[test]
fn test_e2e_test_string_not_equal() {
    let output = exec_sh("if [ \"abc\" != \"def\" ]; then\n  echo diff\nfi");
    assert_eq!(output, "diff");
}

#[test]
fn test_e2e_test_numeric_eq() {
    let output = exec_sh("if [ 5 -eq 5 ]; then\n  echo equal\nfi");
    assert_eq!(output, "equal");
}

#[test]
fn test_e2e_test_numeric_lt() {
    let output = exec_sh("if [ 3 -lt 5 ]; then\n  echo less\nfi");
    assert_eq!(output, "less");
}

#[test]
fn test_e2e_test_not() {
    let output = exec_sh("if [ ! -z \"hello\" ]; then\n  echo yes\nfi");
    assert_eq!(output, "yes");
}

// ── Phase 3c: for loops ──────────────────────────────────────

#[test]
fn test_for_list() {
    let result = transpile("for i in a b c; do\n  echo $i\ndone");
    assert!(
        result.contains("for _, i in ipairs({\"a\", \"b\", \"c\"}) do"),
        "got: {}",
        result
    );
    assert!(result.contains("sh.echo(i)"), "got: {}", result);
    assert!(result.contains("end"), "got: {}", result);
}

#[test]
fn test_for_glob() {
    let result = transpile("for f in *.csv; do\n  echo $f\ndone");
    assert!(
        result.contains("sh.glob("),
        "should use sh.glob for patterns, got: {}",
        result
    );
    assert!(result.contains("*.csv"), "got: {}", result);
}

#[test]
fn test_e2e_for_list() {
    let output = exec_sh("for i in a b c; do\n  echo $i\ndone");
    assert_eq!(output, "a\nb\nc");
}

#[test]
fn test_e2e_for_numbers() {
    let output = exec_sh("for n in 1 2 3; do\n  echo $n\ndone");
    assert_eq!(output, "1\n2\n3");
}

// ── Phase 3d: while loops ────────────────────────────────────

#[test]
fn test_while_transpile() {
    let result = transpile("while [ -n \"$X\" ]; do\n  echo loop\ndone");
    assert!(result.contains("while sh.test(\"-n\""), "got: {}", result);
    assert!(result.contains("do"), "got: {}", result);
    assert!(result.contains("sh.echo(\"loop\")"), "got: {}", result);
    assert!(result.contains("end"), "got: {}", result);
}

#[test]
fn test_while_read_transpile() {
    let result = transpile("while read line; do\n  echo $line\ndone < /workspace/input.txt");
    // while read line with input redirect should become a for-lines loop
    assert!(
        result.contains("sh.lines("),
        "should use sh.lines, got: {}",
        result
    );
    assert!(result.contains("line"), "got: {}", result);
}

// ── Phase 3e: case/esac ──────────────────────────────────────

#[test]
fn test_case_transpile() {
    let result = transpile(
        "case \"$ext\" in\n*.csv) echo CSV ;;\n*.json) echo JSON ;;\n*) echo Unknown ;;\nesac",
    );
    assert!(result.contains("sh.match(ext,"), "got: {}", result);
    assert!(result.contains("sh.echo(\"CSV\")"), "got: {}", result);
    assert!(result.contains("sh.echo(\"JSON\")"), "got: {}", result);
    assert!(result.contains("sh.echo(\"Unknown\")"), "got: {}", result);
}

#[test]
fn test_e2e_case() {
    let output = exec_sh(
            "ext=\"file.csv\"\ncase \"$ext\" in\n*.csv) echo CSV ;;\n*.json) echo JSON ;;\n*) echo Unknown ;;\nesac"
        );
    assert_eq!(output, "CSV");
}

#[test]
fn test_e2e_case_default() {
    let output = exec_sh(
            "ext=\"file.xml\"\ncase \"$ext\" in\n*.csv) echo CSV ;;\n*.json) echo JSON ;;\n*) echo Unknown ;;\nesac"
        );
    assert_eq!(output, "Unknown");
}

// ── Phase 3f: Functions ──────────────────────────────────────

#[test]
fn test_function_transpile() {
    let result = transpile("greet() { echo \"Hello, $1\"; }\ngreet world");
    assert!(result.contains("local function greet("), "got: {}", result);
    assert!(result.contains("local args = {...}"), "got: {}", result);
    assert!(result.contains("greet(\"world\")"), "got: {}", result);
}

#[test]
fn test_e2e_function() {
    let output = exec_sh("greet() { echo \"Hello, $1\"; }\ngreet world");
    assert_eq!(output, "Hello, world");
}

#[test]
fn test_e2e_function_multiple_args() {
    let output = exec_sh("add() { echo \"$1 and $2\"; }\nadd foo bar");
    assert_eq!(output, "foo and bar");
}

// ── Phase 3g: Exit codes and logical operators ───────────────

#[test]
fn test_and_transpile() {
    let result = transpile("[ -n \"hello\" ] && echo yes");
    assert!(result.contains("sh.test("), "got: {}", result);
    assert!(result.contains("sh.last_exit_code == 0"), "got: {}", result);
    assert!(result.contains("sh.echo(\"yes\")"), "got: {}", result);
}

#[test]
fn test_or_transpile() {
    let result = transpile("[ -z \"hello\" ] || echo fallback");
    assert!(result.contains("sh.test("), "got: {}", result);
    assert!(result.contains("sh.last_exit_code ~= 0"), "got: {}", result);
    assert!(result.contains("sh.echo(\"fallback\")"), "got: {}", result);
}

#[test]
fn test_e2e_and_true() {
    let output = exec_sh("[ -n \"hello\" ] && echo yes");
    assert_eq!(output, "yes");
}

#[test]
fn test_e2e_and_false() {
    // -z "hello" is false, so && should not execute
    let output = exec_sh("[ -z \"hello\" ] && echo yes");
    assert_eq!(output, "");
}

#[test]
fn test_e2e_or_true() {
    // -n "hello" is true, so || should not execute
    let output = exec_sh("[ -n \"hello\" ] || echo fallback");
    assert_eq!(output, "");
}

#[test]
fn test_e2e_or_false() {
    let output = exec_sh("[ -z \"hello\" ] || echo fallback");
    assert_eq!(output, "fallback");
}

#[test]
fn test_e2e_exit_code_var() {
    let output = exec_sh("[ -n \"hello\" ]\necho $?");
    assert_eq!(output, "0");
}

#[test]
fn test_e2e_exit_code_fail() {
    let output = exec_sh("[ -z \"hello\" ]\necho $?");
    assert_eq!(output, "1");
}

// ── Phase 3h: Here-documents ─────────────────────────────────

#[test]
fn test_heredoc_cat() {
    let result = transpile("cat <<EOF\nhello world\nEOF");
    assert!(result.contains("sh.cat_input("), "got: {}", result);
    assert!(result.contains("hello world"), "got: {}", result);
}

#[test]
fn test_e2e_heredoc_cat() {
    let output = exec_sh("cat <<EOF\nhello world\nEOF");
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn test_e2e_heredoc_with_variable() {
    let output = exec_sh("NAME=\"world\"\ncat <<EOF\nhello $NAME\nEOF");
    assert_eq!(output.trim(), "hello world");
}

// ── Phase 1: Module dispatch via sh.run() ─────────────────────

#[test]
fn test_module_http_get() {
    // Single positional arg → sh.run with table
    let result = transpile("http get \"http://example.com\"");
    assert!(
        result.contains("sh.run(\"http\", \"get\", {[1]=\"http://example.com\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_fs_read() {
    let result = transpile("fs read /data/file.txt");
    assert!(
        result.contains("sh.run(\"fs\", \"read\", {[1]=\"/data/file.txt\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_compress_zip() {
    let result = transpile("compress zip /src /dst.zip");
    assert!(
        result.contains("sh.run(\"compress\", \"zip\", {[1]=\"/src\", [2]=\"/dst.zip\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_flag_parsing() {
    let result = transpile("compress zip /src /dst.zip --level 9");
    assert!(
        result.contains(
            "sh.run(\"compress\", \"zip\", {[1]=\"/src\", [2]=\"/dst.zip\", level=\"9\"})"
        ),
        "got: {}",
        result
    );
}

#[test]
fn test_module_boolean_flag() {
    let result = transpile("compress zip /src /dst.zip --verbose");
    assert!(result.contains("verbose=true"), "got: {}", result);
    assert!(result.contains("[1]=\"/src\""), "got: {}", result);
}

#[test]
fn test_module_pipe_injection() {
    let result = transpile("echo \"text\" | compress zip /src /dst.zip");
    assert!(
        result
            .contains("sh.run(\"compress\", \"zip\", {input=_in, [1]=\"/src\", [2]=\"/dst.zip\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_bare_name_shows_help() {
    let result = transpile("compress");
    assert!(
        result.contains("sh.run(\"compress\", nil, nil)"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_method_no_args() {
    let result = transpile("http get");
    assert!(
        result.contains("sh.run(\"http\", \"get\", nil)"),
        "got: {}",
        result
    );
}

#[test]
fn test_module_multiple_flags() {
    let result = transpile("compress zip /src /dst.zip --level 9 --verbose");
    assert!(result.contains("sh.run(\"compress\", \"zip\", {[1]=\"/src\", [2]=\"/dst.zip\", level=\"9\", verbose=true})"), "got: {}", result);
}

#[test]
fn test_module_pipe_with_flags() {
    let result = transpile("echo \"text\" | compress zip /src /dst.zip --verbose");
    assert!(result.contains("sh.run(\"compress\", \"zip\", {input=_in, [1]=\"/src\", [2]=\"/dst.zip\", verbose=true})"), "got: {}", result);
}

// Any unknown command also goes through sh.run
#[test]
fn test_unknown_module_dispatch() {
    // `plot line --x "1,2" --y "3,4"` — plot wasn't in KNOWN_MODULES before, now works
    let result = transpile("plot line --x \"1,2\" --y \"3,4\"");
    assert!(
        result.contains("sh.run(\"plot\", \"line\", {x=\"1,2\", y=\"3,4\"})"),
        "got: {}",
        result
    );
}

// ── E2E module dispatch (shell → Luau → sandbox execution) ──

#[test]
fn test_e2e_module_fs_read() {
    let result = transpile_sh("fs read /data/file.txt");
    assert!(result.is_ok());
    let luau = result.unwrap().luau_source;
    assert!(
        luau.contains("sh.run(\"fs\", \"read\", {[1]=\"/data/file.txt\"})"),
        "got: {}",
        luau
    );
}

#[test]
fn test_e2e_module_compress_doc() {
    let output = exec_sh("compress");
    assert!(output.contains("compress — archive"), "got: {}", output);
}

// ── Named-param flag table: ls ──────────────────────────────

#[test]
fn test_ls_flags_la() {
    let result = transpile("ls -la /workspace");
    assert!(
        result.contains("l=true"),
        "should have l=true, got: {}",
        result
    );
    assert!(
        result.contains("a=true"),
        "should have a=true, got: {}",
        result
    );
    assert!(
        result.contains("[1]=\"/workspace\""),
        "should have positional path, got: {}",
        result
    );
}

#[test]
fn test_ls_flags_separate() {
    let result = transpile("ls -l -a /workspace");
    assert!(
        result.contains("l=true"),
        "should have l=true, got: {}",
        result
    );
    assert!(
        result.contains("a=true"),
        "should have a=true, got: {}",
        result
    );
    assert!(
        result.contains("[1]=\"/workspace\""),
        "should have positional path, got: {}",
        result
    );
}

#[test]
fn test_ls_flags_lah() {
    let result = transpile("ls -lah");
    assert!(
        result.contains("l=true"),
        "should have l=true, got: {}",
        result
    );
    assert!(
        result.contains("a=true"),
        "should have a=true, got: {}",
        result
    );
    assert!(
        result.contains("h=true"),
        "should have h=true, got: {}",
        result
    );
}

#[test]
fn test_ls_bare() {
    let result = transpile("ls");
    assert!(
        result.contains("sh.ls()"),
        "should be bare call, got: {}",
        result
    );
}

#[test]
fn test_e2e_ls_flags() {
    // ls -l should produce long format — verify transpilation emits correct table
    let result = transpile("ls -l /workspace");
    assert!(
        result.contains("sh.ls({l=true, [1]=\"/workspace\"})"),
        "got: {}",
        result
    );
}

#[test]
fn test_e2e_ls_long_flag() {
    let result = transpile("ls --long /workspace");
    assert!(
        result.contains("sh.ls({long=true, [1]=\"/workspace\"})"),
        "got: {}",
        result
    );
}

// ── E2E ls tests with mounted filesystem ─────────────────────

#[test]
fn test_e2e_ls_hides_dotfiles() {
    let dir = tempfile::TempDir::new().unwrap();
    // Create visible and hidden files
    std::fs::write(dir.path().join("README.md"), "hello").unwrap();
    std::fs::write(dir.path().join(".hidden"), "secret").unwrap();
    std::fs::write(dir.path().join("data.csv"), "a,b").unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("ls /workspace").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert!(
        !output.contains(".hidden"),
        "ls should hide dotfiles, got: {}",
        output
    );
    assert!(
        output.contains("README.md"),
        "ls should show visible files, got: {}",
        output
    );
    assert!(
        output.contains("data.csv"),
        "ls should show visible files, got: {}",
        output
    );
}

#[test]
fn test_e2e_ls_a_shows_dotfiles() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("README.md"), "hello").unwrap();
    std::fs::write(dir.path().join(".hidden"), "secret").unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("ls -a /workspace").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert!(
        output.contains(".hidden"),
        "ls -a should show dotfiles, got: {}",
        output
    );
    assert!(
        output.contains("README.md"),
        "ls -a should show visible files, got: {}",
        output
    );
}

#[test]
fn test_e2e_ls_l_shows_sizes() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("small.txt"), "hi").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("ls -l /workspace").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    // Should show file with size (2 bytes for "hi")
    assert!(
        output.contains("file"),
        "should have file type, got: {}",
        output
    );
    assert!(
        output.contains("2"),
        "should show size 2 for 'hi', got: {}",
        output
    );
    assert!(
        output.contains("small.txt"),
        "should show filename, got: {}",
        output
    );
    // Should show dir with "-" for size
    assert!(
        output.contains("dir"),
        "should have dir type, got: {}",
        output
    );
    assert!(
        output.contains("subdir"),
        "should show dirname, got: {}",
        output
    );
}

#[test]
fn test_e2e_ls_l_no_crash_on_binary() {
    let dir = tempfile::TempDir::new().unwrap();
    // Write binary content (invalid UTF-8)
    std::fs::write(
        dir.path().join("binary.bin"),
        &[0xFF, 0xFE, 0x00, 0x01, 0x80],
    )
    .unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("ls -l /workspace").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    // Should NOT crash — uses fs.size() instead of fs.read()
    assert!(
        output.contains("binary.bin"),
        "should show binary file, got: {}",
        output
    );
    assert!(output.contains("5"), "should show size 5, got: {}", output);
}

#[test]
fn test_e2e_ls_lh_human_readable() {
    let dir = tempfile::TempDir::new().unwrap();
    // Write a ~1.5KB file
    let content = "x".repeat(1536);
    std::fs::write(dir.path().join("medium.txt"), &content).unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("ls -lh /workspace").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    // Should show human-readable size like "1.5K"
    assert!(
        output.contains("1.5K"),
        "should show human-readable size, got: {}",
        output
    );
    assert!(
        output.contains("medium.txt"),
        "should show filename, got: {}",
        output
    );
}

// ── 1g: sh.run() integration tests ──────────────────────────

#[test]
fn test_e2e_shrun_fs_read_displays_string() {
    // `fs read /workspace/file.txt` via sh.run() should display the file contents
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();

    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("fs read /workspace/hello.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(output, "hello world");
}

#[test]
fn test_e2e_shrun_csv_parse_displays_json() {
    // `csv parse "a,b\n1,2"` via sh.run() should auto-serialize the table result to JSON
    let sandbox = crate::Sandbox::new().unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("csv parse \"a,b\n1,2\"").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    // csv.parse returns a table; sh.run() auto-serializes it to JSON
    assert!(
        output.contains("a") && output.contains("b"),
        "should contain column data, got: {}",
        output
    );
}

#[test]
fn test_e2e_shrun_json_decode_displays_json() {
    // `json decode '{"key":"value"}'` via sh.run() should display pretty JSON
    let sandbox = crate::Sandbox::new().unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("json decode '{\"key\":\"value\"}'").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert!(
        output.contains("key") && output.contains("value"),
        "should display JSON, got: {}",
        output
    );
}

#[test]
fn test_e2e_shrun_unknown_cmd_errors() {
    // Unknown command should error with "command not found"
    let sandbox = crate::Sandbox::new().unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("nonexistent_cmd arg1").unwrap();
    let result = sandbox.exec(&transpiled.luau_source);
    assert!(result.is_err(), "should error for unknown command");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("command not found"),
        "got: {}",
        err.message
    );
}

// ── E2E redirect tests with mounted filesystem ────────────────

#[test]
fn test_e2e_redirect_write_creates_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace:rw", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    // echo "test" > /workspace/out.txt should create the file with no stdout
    let transpiled = transpile_sh("echo test > /workspace/out.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(
        output, "",
        "redirect should suppress stdout, got: {}",
        output
    );
    let content = std::fs::read_to_string(dir.path().join("out.txt")).unwrap();
    assert_eq!(
        content, "test\n",
        "file should contain 'test\\n', got: {:?}",
        content
    );
}

#[test]
fn test_e2e_redirect_write_relative_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/:rw", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    // Relative path "foo.txt" should resolve via shell cwd (/) to /foo.txt
    let transpiled = transpile_sh("echo hello > foo.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(
        output, "",
        "redirect should suppress stdout, got: {}",
        output
    );
    let content = std::fs::read_to_string(dir.path().join("foo.txt")).unwrap();
    assert_eq!(
        content, "hello\n",
        "file should contain 'hello\\n', got: {:?}",
        content
    );
}

#[test]
fn test_e2e_redirect_append() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("log.txt"), "line1\n").unwrap();
    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace:rw", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    let transpiled = transpile_sh("echo line2 >> /workspace/log.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(
        output, "",
        "redirect should suppress stdout, got: {}",
        output
    );
    let content = std::fs::read_to_string(dir.path().join("log.txt")).unwrap();
    assert_eq!(
        content, "line1\nline2\n",
        "should append with newlines, got: {:?}",
        content
    );
}

#[test]
fn test_e2e_redirect_append_creates_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace:rw", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    // >> should create the file if it doesn't exist
    let transpiled = transpile_sh("echo first >> /workspace/new.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(
        output, "",
        "redirect should suppress stdout, got: {}",
        output
    );
    let content = std::fs::read_to_string(dir.path().join("new.txt")).unwrap();
    assert_eq!(
        content, "first\n",
        "should create file with trailing newline, got: {:?}",
        content
    );
}

#[test]
fn test_e2e_bare_redirect_creates_empty_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut table = crate::mount::MountTable::new();
    table
        .parse_and_add(&format!("{}:/workspace:rw", dir.path().display()))
        .unwrap();
    let sandbox = crate::Sandbox::with_mounts(table).unwrap();
    sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

    // > file with no command should create/truncate
    let transpiled = transpile_sh("> /workspace/empty.txt").unwrap();
    let output = sandbox.exec(&transpiled.luau_source).unwrap();
    assert_eq!(output, "");
    let content = std::fs::read_to_string(dir.path().join("empty.txt")).unwrap();
    assert_eq!(content, "", "bare redirect should create empty file");
}
