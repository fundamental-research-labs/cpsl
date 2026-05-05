//! Shell-to-Luau transpiler.
//!
//! Uses `conch-parser` to parse POSIX shell into an AST,
//! then walks it emitting Luau code that calls `shrt.luau` runtime functions.

use conch_parser::ast;
use conch_parser::lexer::Lexer;
use conch_parser::parse::DefaultParser;
use std::collections::HashMap;

use crate::transpile::TranspileResult;

/// Transpile shell source to Luau.
pub fn transpile_sh(source: &str) -> Result<TranspileResult, String> {
    let lex = Lexer::new(source.chars());
    let parser = DefaultParser::new(lex);

    let mut t = ShTranspiler::new();
    t.emit_line("local sh = require(\"shrt\")");

    for cmd in parser {
        let cmd = cmd.map_err(|e| format!("{}", e))?;
        t.visit_top_level(&cmd);
    }

    Ok(TranspileResult {
        luau_source: t.lines.join("\n"),
        source_map: t.source_map,
        warnings: t.warnings,
    })
}

// ── Type aliases for the default conch-parser AST ──────────────────

type ShTopLevel = ast::TopLevelCommand<String>;
type ShWord = ast::TopLevelWord<String>;
type ShSimpleWord = ast::SimpleWord<String, ast::Parameter<String>, Box<ShParamSubst>>;
type ShWordFragment = ast::Word<String, ShSimpleWord>;
type ShComplexWord = ast::ComplexWord<ShWordFragment>;
type ShRedirect = ast::Redirect<ShWord>;
type ShParamSubst = ast::ParameterSubstitution<
    ast::Parameter<String>,
    ShWord,
    ShTopLevel,
    ast::Arithmetic<String>,
>;
type ShSimpleCommand = ast::SimpleCommand<String, ShWord, ShRedirect>;
type ShCompoundCommand = ast::CompoundCommand<ast::CompoundCommandKind<String, ShWord, ShTopLevel>, ShRedirect>;
type ShPipeable = ast::PipeableCommand<String, Box<ShSimpleCommand>, Box<ShCompoundCommand>, std::rc::Rc<ShCompoundCommand>>;
type ShListable = ast::ListableCommand<ShPipeable>;
type ShAndOrList = ast::AndOrList<ShListable>;

// ── Transpiler ─────────────────────────────────────────────────────

struct ShTranspiler {
    lines: Vec<String>,
    source_map: HashMap<usize, usize>,
    warnings: Vec<String>,
    indent: usize,
    declared_vars: std::collections::HashSet<String>,
    declared_fns: std::collections::HashSet<String>,
}

impl ShTranspiler {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            source_map: HashMap::new(),
            warnings: Vec::new(),
            indent: 0,
            declared_vars: std::collections::HashSet::new(),
            declared_fns: std::collections::HashSet::new(),
        }
    }

    fn emit_line(&mut self, s: &str) {
        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}{}", indent, s));
    }

    fn visit_top_level(&mut self, cmd: &ShTopLevel) {
        use ast::Command;
        match &**cmd {
            Command::Job(list) | Command::List(list) => {
                self.visit_and_or_list(list);
            }
        }
    }

    fn visit_and_or_list(&mut self, list: &ShAndOrList) {
        self.visit_listable(&list.first);
        for and_or in &list.rest {
            match and_or {
                ast::AndOr::And(cmd) => {
                    self.emit_line("if sh.last_exit_code == 0 then");
                    self.indent += 1;
                    self.visit_listable(cmd);
                    self.indent -= 1;
                    self.emit_line("end");
                }
                ast::AndOr::Or(cmd) => {
                    self.emit_line("if sh.last_exit_code ~= 0 then");
                    self.indent += 1;
                    self.visit_listable(cmd);
                    self.indent -= 1;
                    self.emit_line("end");
                }
            }
        }
    }

    fn visit_listable(&mut self, cmd: &ShListable) {
        match cmd {
            ast::ListableCommand::Pipe(_, cmds) => self.visit_pipeline(cmds),
            ast::ListableCommand::Single(cmd) => self.visit_pipeable(cmd),
        }
    }

    fn visit_pipeline(&mut self, cmds: &[ShPipeable]) {
        if cmds.len() == 1 {
            self.visit_pipeable(&cmds[0]);
            return;
        }
        // All commands wrapped in lambdas so pipe() can manage pipe_depth
        // before any command runs (suppressing intermediate print output).
        let mut parts = Vec::new();
        for (i, cmd) in cmds.iter().enumerate() {
            if i == 0 {
                // First command: no stdin, just wrap in a thunk
                parts.push(format!("function() return {} end", self.pipeable_to_expr(cmd)));
            } else {
                parts.push(format!("function(_in) return {} end", self.pipeable_to_pipe_expr(cmd)));
            }
        }
        self.emit_line(&format!("sh.pipe({})", parts.join(", ")));
    }

    fn visit_pipeable(&mut self, cmd: &ShPipeable) {
        match cmd {
            ast::PipeableCommand::Simple(simple) => self.visit_simple_command(simple),
            ast::PipeableCommand::Compound(compound) => {
                self.visit_compound_command(&compound.kind, &compound.io);
            }
            ast::PipeableCommand::FunctionDef(name, body) => {
                self.visit_function_def(name, body);
            }
        }
    }

    fn pipeable_to_expr(&self, cmd: &ShPipeable) -> String {
        match cmd {
            ast::PipeableCommand::Simple(simple) => self.simple_command_to_expr(simple),
            _ => "nil --[[ unsupported compound in pipe ]]".to_string(),
        }
    }

    /// Like pipeable_to_expr but for piped commands: uses `_in` as stdin.
    fn pipeable_to_pipe_expr(&self, cmd: &ShPipeable) -> String {
        match cmd {
            ast::PipeableCommand::Simple(simple) => self.simple_command_to_pipe_expr(simple),
            _ => "_in --[[ unsupported compound in pipe ]]".to_string(),
        }
    }

    fn visit_simple_command(&mut self, cmd: &ShSimpleCommand) {
        let has_cmd = cmd.redirects_or_cmd_words.iter().any(|item| {
            matches!(item, ast::RedirectOrCmdWord::CmdWord(_))
        });

        if !has_cmd {
            // Pure assignment: FOO="bar"
            for item in &cmd.redirects_or_env_vars {
                if let ast::RedirectOrEnvVar::EnvVar(name, value) = item {
                    let val_expr = match value {
                        Some(w) => self.word_to_expr(w),
                        None => "\"\"".to_string(),
                    };
                    if self.declared_vars.contains(name) {
                        self.emit_line(&format!("{} = {}", name, val_expr));
                    } else {
                        self.declared_vars.insert(name.clone());
                        self.emit_line(&format!("local {} = {}", name, val_expr));
                    }
                }
            }
            // Handle bare redirects: > foo.txt creates/truncates, >> foo.txt creates
            // Check both locations where redirects can appear in the AST
            let all_redirects: Vec<&ShRedirect> = cmd.redirects_or_cmd_words.iter().filter_map(|item| {
                if let ast::RedirectOrCmdWord::Redirect(r) = item { Some(r) } else { None }
            }).chain(cmd.redirects_or_env_vars.iter().filter_map(|item| {
                if let ast::RedirectOrEnvVar::Redirect(r) = item { Some(r) } else { None }
            })).collect();
            for redir in all_redirects {
                match redir {
                    ast::Redirect::Write(_, ref path) => {
                        let path_expr = self.word_to_expr(path);
                        self.emit_line(&format!(
                            "sh.redirect_write({}, function() return \"\" end)",
                            path_expr
                        ));
                    }
                    ast::Redirect::Append(_, ref path) => {
                        let path_expr = self.word_to_expr(path);
                        self.emit_line(&format!(
                            "sh.redirect_append({}, function() return \"\" end)",
                            path_expr
                        ));
                    }
                    _ => {}
                }
            }
            return;
        }

        let expr = self.simple_command_to_expr(cmd);

        // Handle output redirections
        let mut redirected = false;
        for item in &cmd.redirects_or_cmd_words {
            if let ast::RedirectOrCmdWord::Redirect(redir) = item {
                match redir {
                    ast::Redirect::Write(_, ref path) => {
                        let path_expr = self.word_to_expr(path);
                        self.emit_line(&format!(
                            "sh.redirect_write({}, function() return {} end)",
                            path_expr, expr
                        ));
                        redirected = true;
                    }
                    ast::Redirect::Append(_, ref path) => {
                        let path_expr = self.word_to_expr(path);
                        self.emit_line(&format!(
                            "sh.redirect_append({}, function() return {} end)",
                            path_expr, expr
                        ));
                        redirected = true;
                    }
                    ast::Redirect::Heredoc(_, _) => {
                        // Heredoc input is handled in simple_command_to_expr via input_expr
                    }
                    ast::Redirect::DupRead(_, _) |
                    ast::Redirect::DupWrite(_, _) => {
                        // 2>&1, 1>&2, etc. — no real stderr in sandbox, ignore
                    }
                    _ => {}
                }
            }
        }

        if !redirected {
            self.emit_line(&expr);
        }
    }

    fn simple_command_to_expr(&self, cmd: &ShSimpleCommand) -> String {
        // Collect command words as both plain strings (for name matching) and AST refs (for expression generation)
        let mut cmd_words: Vec<&ShWord> = Vec::new();
        for item in &cmd.redirects_or_cmd_words {
            if let ast::RedirectOrCmdWord::CmdWord(w) = item {
                cmd_words.push(w);
            }
        }

        // Check for input redirection or heredoc
        let mut input_expr: Option<String> = None;
        for item in &cmd.redirects_or_cmd_words {
            match item {
                ast::RedirectOrCmdWord::Redirect(ast::Redirect::Read(_, ref path)) => {
                    input_expr = Some(format!("fs.read({})", self.word_to_expr(path)));
                }
                ast::RedirectOrCmdWord::Redirect(ast::Redirect::Heredoc(_, ref body)) => {
                    input_expr = Some(self.word_to_expr(body));
                }
                _ => {}
            }
        }
        for item in &cmd.redirects_or_env_vars {
            match item {
                ast::RedirectOrEnvVar::Redirect(ast::Redirect::Read(_, ref path)) => {
                    input_expr = Some(format!("fs.read({})", self.word_to_expr(path)));
                }
                ast::RedirectOrEnvVar::Redirect(ast::Redirect::Heredoc(_, ref body)) => {
                    input_expr = Some(self.word_to_expr(body));
                }
                _ => {}
            }
        }

        if cmd_words.is_empty() {
            return "nil".to_string();
        }

        let cmd_name = self.word_to_string(cmd_words[0]);
        // For arguments: use plain strings for flag parsing, AST expressions for output
        let args: Vec<String> = cmd_words[1..].iter().map(|w| self.word_to_string(w)).collect();
        let arg_exprs: Vec<String> = cmd_words[1..].iter().map(|w| self.word_to_expr(w)).collect();
        // Glob-aware expressions: if an arg has glob chars, wrap in sh.glob()
        let arg_glob_exprs: Vec<String> = cmd_words[1..].iter().map(|w| self.word_to_expr_or_glob(w)).collect();

        match cmd_name.as_str() {
            "echo" => {
                if arg_glob_exprs.is_empty() { "sh.echo()".to_string() }
                else { format!("sh.echo({})", arg_glob_exprs.join(", ")) }
            }
            "printf" => format!("sh.printf({})", arg_exprs.join(", ")),
            "ls" => {
                if args.is_empty() {
                    "sh.ls()".to_string()
                } else {
                    let table = self.build_flag_table(&args, &arg_exprs, &[], None, false);
                    if table.is_empty() { "sh.ls()".to_string() }
                    else { format!("sh.ls({})", table) }
                }
            }
            "cat" => {
                if let Some(ref input) = input_expr { format!("sh.cat({})", input) }
                else if arg_exprs.is_empty() { "sh.cat()".to_string() }
                else { format!("sh.cat({})", arg_exprs.join(", ")) }
            }
            "pwd" => "sh.pwd()".to_string(),
            "cd" => {
                if arg_exprs.is_empty() { "sh.cd()".to_string() }
                else { format!("sh.cd({})", arg_exprs[0]) }
            }
            "head" | "tail" => {
                // Value flag: -n
                let table = self.build_flag_table(&args, &arg_exprs, &["n"], input_expr.as_deref(), true);
                if table.is_empty() { format!("sh.{}()", cmd_name) }
                else { format!("sh.{}({})", cmd_name, table) }
            }
            "grep" => {
                // Bool flags: -i, -v, -c, -n. Positional: pattern, file
                let table = self.build_flag_table(&args, &arg_exprs, &[], input_expr.as_deref(), false);
                if table.is_empty() { "sh.grep()".to_string() }
                else { format!("sh.grep({})", table) }
            }
            "wc" => {
                // Bool flags: -l, -w, -c
                let table = self.build_flag_table(&args, &arg_exprs, &[], input_expr.as_deref(), true);
                if table.is_empty() { "sh.wc()".to_string() }
                else { format!("sh.wc({})", table) }
            }
            "sort" => {
                // Bool flags: -r, -n, -u
                let table = self.build_flag_table(&args, &arg_exprs, &[], input_expr.as_deref(), true);
                if table.is_empty() { "sh.sort()".to_string() }
                else { format!("sh.sort({})", table) }
            }
            "uniq" => {
                let table = self.build_flag_table(&args, &arg_exprs, &[], input_expr.as_deref(), true);
                if table.is_empty() { "sh.uniq()".to_string() }
                else { format!("sh.uniq({})", table) }
            }
            "mkdir" => {
                // Bool flags: -p. Positional: path
                let table = self.build_flag_table(&args, &arg_exprs, &[], None, false);
                if table.is_empty() { "sh.mkdir()".to_string() }
                else { format!("sh.mkdir({})", table) }
            }
            "touch" => {
                if arg_exprs.is_empty() { "sh.touch()".to_string() }
                else { format!("sh.touch({})", arg_exprs[0]) }
            }
            "cp" => {
                // Bool flags: -r, -f. Positional: src, dst
                let table = self.build_flag_table(&args, &arg_exprs, &[], None, false);
                if table.is_empty() { "sh.cp()".to_string() }
                else { format!("sh.cp({})", table) }
            }
            "mv" => {
                if arg_exprs.len() >= 2 { format!("sh.mv({}, {})", arg_exprs[0], arg_exprs[1]) }
                else { "sh.mv() -- missing args".to_string() }
            }
            "rm" => {
                // Bool flags: -r, -f, -rf. Positional: paths (glob-aware)
                let table = self.build_flag_table(&args, &arg_glob_exprs, &[], None, false);
                if table.is_empty() { "sh.rm()".to_string() }
                else { format!("sh.rm({})", table) }
            }
            "test" | "[" => {
                let filtered: Vec<usize> = args.iter().enumerate().filter(|(_, a)| a.as_str() != "]").map(|(i, _)| i).collect();
                let exprs: Vec<String> = filtered.iter().map(|&i| arg_exprs[i].clone()).collect();
                format!("sh.test({})", exprs.join(", "))
            }
            "find" => {
                // Value flags: -name, -type, -maxdepth (multi-char single-dash)
                if args.is_empty() {
                    "sh.find()".to_string()
                } else {
                    let table = self.build_flag_table(&args, &arg_exprs, &["name", "type", "maxdepth"], None, false);
                    if table.is_empty() { "sh.find()".to_string() }
                    else { format!("sh.find({})", table) }
                }
            }
            "tree" => {
                // tree [path] [-d] [-L depth]
                // Maps to sh.tree({path, d=true, L=N})
                let table = self.build_flag_table(&args, &arg_exprs, &["L"], None, false);
                if table.is_empty() { "sh.tree()".to_string() }
                else { format!("sh.tree({})", table) }
            }
            "tee" => {
                // tee file — write stdin to file AND stdout
                if arg_exprs.is_empty() { "sh.tee()".to_string() }
                else { format!("sh.tee(nil, {})", arg_exprs[0]) }
            }
            "base64" => {
                // Linux base64: encode by default, decode with -d flag.
                // `base64` bare → module help; `base64 -d "text"` → decode;
                // `base64 "text"` → encode; `base64 encode/decode` → module dispatch.
                if args.is_empty() {
                    // Bare `base64` → module dispatch (shows help)
                    self.runtime_dispatch_expr(&cmd_name, &args, &arg_exprs)
                } else if args[0] == "-d" || args[0] == "-D" || args[0] == "--decode" {
                    // `base64 -d "text"` → decode, print result
                    let data_expr = arg_exprs.get(1);
                    match data_expr {
                        Some(e) => format!("print(base64.decode({}))", e),
                        None => "print(base64.decode(\"\"))".to_string(),
                    }
                } else if args[0].starts_with('-') {
                    // Other flags (e.g. --help) → module dispatch
                    self.runtime_dispatch_expr(&cmd_name, &args, &arg_exprs)
                } else if args[0] == "encode" || args[0] == "decode" || args[0] == "b64encode" || args[0] == "b64decode" || args[0] == "help" {
                    // `base64 encode/decode/help` → module dispatch
                    self.runtime_dispatch_expr(&cmd_name, &args, &arg_exprs)
                } else {
                    // `base64 "text"` → encode, print result
                    format!("print(base64.encode({}))", arg_exprs[0])
                }
            }
            "help" => "sh.shell_help()".to_string(),
            "true" => "sh.true_cmd()".to_string(),
            "false" => "sh.false_cmd()".to_string(),
            // Informational commands
            "whoami" => "sh.whoami()".to_string(),
            "hostname" => "sh.hostname_cmd()".to_string(),
            "id" => "sh.id()".to_string(),
            "uname" => {
                if arg_exprs.is_empty() { "sh.uname()".to_string() }
                else { format!("sh.uname({})", arg_exprs[0]) }
            }
            "date" => {
                if arg_exprs.is_empty() { "sh.date()".to_string() }
                else { format!("sh.date({})", arg_exprs[0]) }
            }
            "env" => "sh.env_cmd()".to_string(),
            "export" => {
                if arg_exprs.is_empty() { "sh.env_cmd()".to_string() }
                else { format!("sh.export({})", arg_exprs.join(", ")) }
            }
            "which" => {
                if arg_exprs.is_empty() { "sh.which()".to_string() }
                else { format!("sh.which({})", arg_exprs[0]) }
            }
            "type" => {
                if arg_exprs.is_empty() { "sh.type_cmd()".to_string() }
                else { format!("sh.type_cmd({})", arg_exprs[0]) }
            }
            // Graceful stubs for unsupported commands
            "ps" => "sh.ps()".to_string(),
            "kill" | "top" | "bg" | "fg" | "jobs" => {
                format!("sh.stub_no_process(\"{}\")", cmd_name)
            }
            "sleep" => "sh.sleep_cmd()".to_string(),
            "ssh" | "curl" | "wget" => {
                format!("sh.stub_no_network(\"{}\")", cmd_name)
            }
            "sudo" => "sh.sudo()".to_string(),
            "apt" | "apt-get" | "brew" | "pip" | "pip3" | "npm" | "yarn" => {
                format!("sh.stub_no_pkg(\"{}\")", cmd_name)
            }
            "exit" => {
                if arg_exprs.is_empty() { "sh.exit(0)".to_string() }
                else { format!("sh.exit({})", arg_exprs[0]) }
            }
            _ => {
                // Check if it's a user-defined function call
                if self.declared_fns.contains(&cmd_name) {
                    if arg_exprs.is_empty() {
                        format!("{}()", cmd_name)
                    } else {
                        format!("{}({})", cmd_name, arg_exprs.join(", "))
                    }
                } else {
                    // Runtime module dispatch: sh.run("cmd", "method", {args})
                    // If args[0] looks like a method name, split into module + method + table
                    // Otherwise emit sh.run("cmd", nil, {args}) for bare commands
                    self.runtime_dispatch_expr(&cmd_name, &args, &arg_exprs)
                }
            }
        }
    }

    /// Like simple_command_to_expr, but for a command receiving piped input.
    /// Injects `_in` as the input source for commands that accept stdin.
    fn simple_command_to_pipe_expr(&self, cmd: &ShSimpleCommand) -> String {
        let mut cmd_words: Vec<&ShWord> = Vec::new();
        for item in &cmd.redirects_or_cmd_words {
            if let ast::RedirectOrCmdWord::CmdWord(w) = item {
                cmd_words.push(w);
            }
        }
        if cmd_words.is_empty() { return "_in".to_string(); }

        let cmd_name = self.word_to_string(cmd_words[0]);
        let arg_exprs: Vec<String> = cmd_words[1..].iter().map(|w| self.word_to_expr(w)).collect();
        let args: Vec<String> = cmd_words[1..].iter().map(|w| self.word_to_string(w)).collect();

        // For piped commands, inject _in as the input via the flag table
        match cmd_name.as_str() {
            "head" | "tail" => {
                let table = self.build_flag_table(&args, &arg_exprs, &["n"], Some("_in"), false);
                format!("sh.{}({})", cmd_name, table)
            }
            "grep" => {
                let table = self.build_flag_table(&args, &arg_exprs, &[], Some("_in"), false);
                format!("sh.grep({})", table)
            }
            "sort" => {
                let table = self.build_flag_table(&args, &arg_exprs, &[], Some("_in"), false);
                if table.is_empty() { "sh.sort({input=_in})".to_string() }
                else { format!("sh.sort({})", table) }
            }
            "uniq" => {
                let table = self.build_flag_table(&args, &arg_exprs, &[], Some("_in"), false);
                if table.is_empty() { "sh.uniq({input=_in})".to_string() }
                else { format!("sh.uniq({})", table) }
            }
            "wc" => {
                let table = self.build_flag_table(&args, &arg_exprs, &[], Some("_in"), false);
                if table.is_empty() { "sh.wc({input=_in})".to_string() }
                else { format!("sh.wc({})", table) }
            }
            "tr" => {
                // tr: sh.tr(_in, set1, set2) or sh.tr(_in, "-d", chars) or sh.tr(_in, "-s", chars)
                let mut call_args = vec!["_in".to_string()];
                for a in &arg_exprs { call_args.push(a.clone()); }
                format!("sh.tr({})", call_args.join(", "))
            }
            "cut" => {
                // cut: sh.cut(_in, "-d", delim, "-f", field)
                // Handles both `-d ','` (separate words) and `-d,` / `-f2` (attached)
                let mut call_args = vec!["_in".to_string()];
                let mut i = 0;
                while i < args.len() {
                    if (args[i] == "-d" || args[i] == "-f") && i + 1 < args.len() {
                        // Separate: -d ','
                        call_args.push(format!("\"{}\"", args[i]));
                        call_args.push(arg_exprs[i + 1].clone());
                        i += 2;
                    } else if args[i].starts_with("-d") && args[i].len() > 2 {
                        // Attached: -d,
                        call_args.push("\"-d\"".to_string());
                        call_args.push(format!("\"{}\"", &args[i][2..]));
                        i += 1;
                    } else if args[i].starts_with("-f") && args[i].len() > 2 {
                        // Attached: -f2
                        call_args.push("\"-f\"".to_string());
                        call_args.push(format!("\"{}\"", &args[i][2..]));
                        i += 1;
                    } else { i += 1; }
                }
                format!("sh.cut({})", call_args.join(", "))
            }
            "base64" => {
                // Linux base64: encode by default, decode with -d flag
                if args.iter().any(|a| a == "-d" || a == "-D" || a == "--decode") {
                    "base64.decode(_in)".to_string()
                } else {
                    "base64.encode(_in)".to_string()
                }
            }
            "cat" => "sh.cat(_in)".to_string(),
            "tee" => {
                if arg_exprs.is_empty() { "sh.tee(_in)".to_string() }
                else { format!("sh.tee(_in, {})", arg_exprs[0]) }
            }
            // Runtime module dispatch for piped commands
            _ => self.runtime_pipe_dispatch_expr(&cmd_name, &args, &arg_exprs),
        }
    }

    /// Runtime dispatch: emits `sh.run("cmd", "method", {args})` for unknown commands.
    ///
    /// If the command has sub-args, `args[0]` is treated as the method name and
    /// the rest go into a flag table. If no args, it's a bare command.
    /// At runtime `sh.run()` checks `_G[cmd]` — if it's a table (module), dispatches
    /// to module.method(); otherwise errors with "command not found".
    fn runtime_dispatch_expr(&self, cmd: &str, args: &[String], arg_exprs: &[String]) -> String {
        if args.is_empty() {
            // Bare command: `plot` → sh.run("plot", nil, nil)
            return format!("sh.run(\"{}\", nil, nil)", cmd);
        }
        let method = &args[0];
        let remaining = &args[1..];
        let remaining_exprs = &arg_exprs[1..];

        let table = self.build_flag_table_inner(remaining, remaining_exprs, &[], None, false, true);
        if table.is_empty() {
            format!("sh.run(\"{}\", \"{}\", nil)", cmd, method)
        } else {
            format!("sh.run(\"{}\", \"{}\", {})", cmd, method, table)
        }
    }

    /// Like `runtime_dispatch_expr` but injects `input=_in` for piped commands.
    fn runtime_pipe_dispatch_expr(&self, cmd: &str, args: &[String], arg_exprs: &[String]) -> String {
        if args.is_empty() {
            return format!("sh.run(\"{}\", nil, {{input=_in}})", cmd);
        }
        let method = &args[0];
        let remaining = &args[1..];
        let remaining_exprs = &arg_exprs[1..];

        let table = self.build_flag_table_inner(remaining, remaining_exprs, &[], Some("_in"), false, true);
        format!("sh.run(\"{}\", \"{}\", {})", cmd, method, table)
    }

    /// Generic flag parser that returns a Lua table expression.
    ///
    /// Parses shell-style flags and positional args into a Lua table:
    /// - Compound single-char flags are expanded: `-lah` → `l=true, a=true, h=true`
    /// - Single-char value flags consume the next arg: `-n 5` → `n=5`
    /// - Long flags: `--long` → `long=true`, `--name value` → `name="value"`
    /// - `-N` shorthand (e.g. `head -5`): emits `n=5`
    /// - Positional args get numeric keys: `[1]="/workspace"`, `[2]="pattern"`
    /// - If `input_expr` is set, it's injected as `input=<expr>`
    /// - If `auto_read` is true and positional args look like file paths,
    ///   they're wrapped in `fs.read()` (for commands like `head file.txt`)
    /// - If `long_flags_take_values` is true, `--flag next` treats `next` as the value
    ///   (unless `next` is also a flag). Used by module calls. When false, `--flag` is
    ///   always boolean (used by shell commands where the Lua function resolves meaning).
    ///
    /// Returns `{l=true, a=true, [1]="/workspace"}` or empty `nil` if no flags/args.
    fn build_flag_table(
        &self,
        args: &[String],
        arg_exprs: &[String],
        value_flags: &[&str],
        input_expr: Option<&str>,
        auto_read_positionals: bool,
    ) -> String {
        self.build_flag_table_inner(args, arg_exprs, value_flags, input_expr, auto_read_positionals, false)
    }

    fn build_flag_table_inner(
        &self,
        args: &[String],
        arg_exprs: &[String],
        value_flags: &[&str],
        input_expr: Option<&str>,
        auto_read_positionals: bool,
        long_flags_take_values: bool,
    ) -> String {
        let mut entries: Vec<String> = Vec::new();
        let mut positional_idx = 1usize;
        let mut i = 0;

        while i < args.len() {
            if args[i].starts_with("--") && args[i].len() > 2 {
                let key = args[i][2..].replace('-', "_");
                if long_flags_take_values
                    && i + 1 < args.len()
                    && !args[i + 1].starts_with('-')
                {
                    // --flag value (module mode)
                    entries.push(format!("{}={}", key, arg_exprs[i + 1]));
                    i += 2;
                } else {
                    // --flag (boolean)
                    entries.push(format!("{}=true", key));
                    i += 1;
                }
            } else if args[i].starts_with('-') && args[i].len() > 1 {
                let flag_str = &args[i][1..];
                // Check if this is a known multi-char value flag (e.g. -name, -type)
                let is_multi_value = value_flags.iter().any(|f| f.len() > 1 && *f == flag_str);
                if is_multi_value && i + 1 < args.len() {
                    let key = flag_str.replace('-', "_");
                    entries.push(format!("{}={}", key, arg_exprs[i + 1]));
                    i += 2;
                } else if let Ok(n) = flag_str.parse::<i64>() {
                    // -N shorthand (e.g. head -5, tail -20)
                    entries.push(format!("n={}", n));
                    i += 1;
                } else if flag_str.len() == 1 {
                    // Single char flag: -n 5 or -l
                    let ch_str = flag_str;
                    if value_flags.contains(&ch_str) && i + 1 < args.len() {
                        entries.push(format!("{}={}", ch_str, arg_exprs[i + 1]));
                        i += 2;
                    } else {
                        entries.push(format!("{}=true", ch_str));
                        i += 1;
                    }
                } else {
                    // Compound flags: -lah → l=true, a=true, h=true
                    for ch in flag_str.chars() {
                        entries.push(format!("{}=true", ch));
                    }
                    i += 1;
                }
            } else {
                // Positional arg
                let expr = if auto_read_positionals {
                    format!("fs.read({})", arg_exprs[i])
                } else {
                    arg_exprs[i].clone()
                };
                entries.push(format!("[{}]={}", positional_idx, expr));
                positional_idx += 1;
                i += 1;
            }
        }

        // Inject input if present
        if let Some(input) = input_expr {
            entries.insert(0, format!("input={}", input));
        }

        if entries.is_empty() {
            return String::new();
        }
        format!("{{{}}}", entries.join(", "))
    }

    /// Check if a guard is a `read VAR` command and return the variable name.
    fn is_read_guard(&self, guard: &[ShTopLevel]) -> Option<String> {
        if guard.len() != 1 { return None; }
        use ast::Command;
        match &**&guard[0] {
            Command::List(list) | Command::Job(list) => {
                if !list.rest.is_empty() { return None; }
                match &list.first {
                    ast::ListableCommand::Single(ast::PipeableCommand::Simple(simple)) => {
                        let cmd_words: Vec<&ShWord> = simple.redirects_or_cmd_words.iter().filter_map(|item| {
                            if let ast::RedirectOrCmdWord::CmdWord(w) = item { Some(w) } else { None }
                        }).collect();
                        if cmd_words.len() == 2 && self.word_to_string(cmd_words[0]) == "read" {
                            Some(self.word_to_string(cmd_words[1]))
                        } else { None }
                    }
                    _ => None,
                }
            }
        }
    }

    /// Extract input redirect file path from compound command IO redirects.
    fn find_input_redirect(&self, io: &[ShRedirect]) -> Option<String> {
        for redir in io {
            if let ast::Redirect::Read(_, ref path) = redir {
                return Some(self.word_to_expr(path));
            }
        }
        None
    }

    fn visit_compound_command(&mut self, kind: &ast::CompoundCommandKind<String, ShWord, ShTopLevel>, _io: &[ShRedirect]) {
        match kind {
            ast::CompoundCommandKind::If { conditionals, else_branch } => {
                for (i, pair) in conditionals.iter().enumerate() {
                    let keyword = if i == 0 { "if" } else { "elseif" };
                    let guard_expr = self.guard_to_expr(&pair.guard);
                    self.emit_line(&format!("{} {} then", keyword, guard_expr));
                    self.indent += 1;
                    for cmd in &pair.body {
                        self.visit_top_level(cmd);
                    }
                    self.indent -= 1;
                }
                if let Some(ref else_body) = *else_branch {
                    self.emit_line("else");
                    self.indent += 1;
                    for cmd in else_body {
                        self.visit_top_level(cmd);
                    }
                    self.indent -= 1;
                }
                self.emit_line("end");
            }
            ast::CompoundCommandKind::For { var, words, body } => {
                let items = match words {
                    Some(words) if words.len() == 1 && self.word_has_glob(&words[0]) => {
                        // Single glob word: iterate directly over sh.glob() result
                        let pattern = self.word_to_glob_pattern(&words[0]);
                        format!("sh.glob(\"{}\")", escape_luau_string(&pattern))
                    }
                    Some(words) => {
                        let exprs: Vec<String> = words.iter().map(|w| self.word_to_expr_or_glob(w)).collect();
                        format!("{{{}}}", exprs.join(", "))
                    }
                    None => "arg".to_string(),
                };
                self.emit_line(&format!("for _, {} in ipairs({}) do", var, items));
                self.indent += 1;
                for cmd in body { self.visit_top_level(cmd); }
                self.indent -= 1;
                self.emit_line("end");
            }
            ast::CompoundCommandKind::While(pair) => {
                // Detect `while read VAR; do ... done < file` pattern
                if let Some(var) = self.is_read_guard(&pair.guard) {
                    let input = self.find_input_redirect(_io)
                        .unwrap_or_else(|| "\"\"".to_string());
                    self.emit_line(&format!("for {} in sh.lines(fs.read({})) do", var, input));
                    self.indent += 1;
                    for cmd in &pair.body { self.visit_top_level(cmd); }
                    self.indent -= 1;
                    self.emit_line("end");
                } else {
                    let guard_expr = self.guard_to_expr(&pair.guard);
                    self.emit_line(&format!("while {} do", guard_expr));
                    self.indent += 1;
                    for cmd in &pair.body { self.visit_top_level(cmd); }
                    self.indent -= 1;
                    self.emit_line("end");
                }
            }
            ast::CompoundCommandKind::Until(pair) => {
                let guard_expr = self.guard_to_expr(&pair.guard);
                self.emit_line(&format!("while not ({}) do", guard_expr));
                self.indent += 1;
                for cmd in &pair.body { self.visit_top_level(cmd); }
                self.indent -= 1;
                self.emit_line("end");
            }
            ast::CompoundCommandKind::Case { word, arms } => {
                let word_expr = self.word_to_expr(word);
                for (i, pair) in arms.iter().enumerate() {
                    let keyword = if i == 0 { "if" } else { "elseif" };
                    let pats: Vec<String> = pair.patterns.iter().map(|p| {
                        format!("sh.match({}, {})", word_expr, self.word_to_expr(p))
                    }).collect();
                    self.emit_line(&format!("{} {} then", keyword, pats.join(" or ")));
                    self.indent += 1;
                    for cmd in &pair.body { self.visit_top_level(cmd); }
                    self.indent -= 1;
                }
                self.emit_line("end");
            }
            ast::CompoundCommandKind::Brace(cmds) |
            ast::CompoundCommandKind::Subshell(cmds) => {
                for cmd in cmds { self.visit_top_level(cmd); }
            }
        }
    }

    fn visit_function_def(&mut self, name: &str, body: &std::rc::Rc<ShCompoundCommand>) {
        self.declared_fns.insert(name.to_string());
        self.emit_line(&format!("local function {}(...)", name));
        self.indent += 1;
        self.emit_line("local args = {...}");
        self.visit_compound_command(&body.kind, &body.io);
        self.indent -= 1;
        self.emit_line("end");
    }

    fn guard_to_expr(&self, guard: &[ShTopLevel]) -> String {
        if guard.len() == 1 {
            if let Some(expr) = self.top_level_to_expr(&guard[0]) {
                return expr;
            }
        }
        let exprs: Vec<String> = guard.iter().filter_map(|c| self.top_level_to_expr(c)).collect();
        if exprs.is_empty() { "true".to_string() } else { exprs.join(" and ") }
    }

    fn top_level_to_expr(&self, cmd: &ShTopLevel) -> Option<String> {
        use ast::Command;
        match &**cmd {
            Command::List(list) | Command::Job(list) => {
                let first = self.listable_to_expr(&list.first)?;
                if list.rest.is_empty() {
                    return Some(first);
                }
                // Handle && / || in expression context
                let mut expr = first;
                for and_or in &list.rest {
                    match and_or {
                        ast::AndOr::And(cmd) => {
                            let right = self.listable_to_expr(cmd)?;
                            expr = format!("{} and {}", expr, right);
                        }
                        ast::AndOr::Or(cmd) => {
                            let right = self.listable_to_expr(cmd)?;
                            expr = format!("{} or {}", expr, right);
                        }
                    }
                }
                Some(expr)
            }
        }
    }

    fn listable_to_expr(&self, cmd: &ShListable) -> Option<String> {
        match cmd {
            ast::ListableCommand::Single(pipeable) => Some(self.pipeable_to_expr(pipeable)),
            ast::ListableCommand::Pipe(_, cmds) => {
                let mut parts = Vec::new();
                for (i, cmd) in cmds.iter().enumerate() {
                    if i == 0 {
                        parts.push(format!("function() return {} end", self.pipeable_to_expr(cmd)));
                    } else {
                        parts.push(format!("function(_in) return {} end", self.pipeable_to_pipe_expr(cmd)));
                    }
                }
                Some(format!("sh.pipe({})", parts.join(", ")))
            }
        }
    }

    // ── Glob detection ────────────────────────────────────────────

    /// Check if a word contains glob metacharacters (*, ?, [)
    fn word_has_glob(&self, word: &ShWord) -> bool {
        self.complex_word_has_glob(&*word)
    }

    fn complex_word_has_glob(&self, cw: &ShComplexWord) -> bool {
        match cw {
            ast::ComplexWord::Single(w) => self.fragment_has_glob(w),
            ast::ComplexWord::Concat(parts) => parts.iter().any(|w| self.fragment_has_glob(w)),
        }
    }

    fn fragment_has_glob(&self, w: &ShWordFragment) -> bool {
        match w {
            ast::Word::Simple(sw) => matches!(sw,
                ast::SimpleWord::Star | ast::SimpleWord::Question | ast::SimpleWord::SquareOpen
            ),
            ast::Word::DoubleQuoted(_) => false, // quoted — no glob expansion
            ast::Word::SingleQuoted(_) => false,
        }
    }

    /// Convert a word to a glob pattern string (for sh.glob() call)
    fn word_to_glob_pattern(&self, word: &ShWord) -> String {
        self.complex_word_to_glob_pattern(&*word)
    }

    fn complex_word_to_glob_pattern(&self, cw: &ShComplexWord) -> String {
        match cw {
            ast::ComplexWord::Single(w) => self.fragment_to_glob_string(w),
            ast::ComplexWord::Concat(parts) => {
                parts.iter().map(|w| self.fragment_to_glob_string(w)).collect::<Vec<_>>().join("")
            }
        }
    }

    fn fragment_to_glob_string(&self, w: &ShWordFragment) -> String {
        match w {
            ast::Word::Simple(sw) => match sw {
                ast::SimpleWord::Literal(s) | ast::SimpleWord::Escaped(s) => s.clone(),
                ast::SimpleWord::Star => "*".to_string(),
                ast::SimpleWord::Question => "?".to_string(),
                ast::SimpleWord::SquareOpen => "[".to_string(),
                ast::SimpleWord::SquareClose => "]".to_string(),
                ast::SimpleWord::Tilde => "~".to_string(),
                _ => self.simple_word_to_string(sw),
            },
            ast::Word::DoubleQuoted(parts) => {
                parts.iter().map(|sw| self.simple_word_to_string(sw)).collect::<Vec<_>>().join("")
            }
            ast::Word::SingleQuoted(lit) => lit.clone(),
        }
    }

    /// Return a glob expression if the word contains glob chars, otherwise the normal expression
    fn word_to_expr_or_glob(&self, word: &ShWord) -> String {
        if self.word_has_glob(word) {
            let pattern = self.word_to_glob_pattern(word);
            format!("sh.glob(\"{}\")", escape_luau_string(&pattern))
        } else {
            self.word_to_expr(word)
        }
    }

    // ── Word handling ──────────────────────────────────────────────

    fn word_to_expr(&self, word: &ShWord) -> String {
        self.complex_word_to_expr(&*word)
    }

    fn complex_word_to_expr(&self, cw: &ShComplexWord) -> String {
        match cw {
            ast::ComplexWord::Single(w) => self.word_fragment_to_expr(w),
            ast::ComplexWord::Concat(parts) => {
                let exprs: Vec<String> = parts.iter().map(|w| self.word_fragment_to_expr(w)).collect();
                if exprs.len() == 1 { exprs[0].clone() } else { exprs.join(" .. ") }
            }
        }
    }

    fn word_fragment_to_expr(&self, w: &ShWordFragment) -> String {
        match w {
            ast::Word::Simple(sw) => self.simple_word_to_expr(sw),
            ast::Word::DoubleQuoted(parts) => {
                if parts.is_empty() { return "\"\"".to_string(); }
                let exprs: Vec<String> = parts.iter().map(|sw| self.simple_word_to_expr(sw)).collect();
                if exprs.len() == 1 { exprs[0].clone() } else { exprs.join(" .. ") }
            }
            ast::Word::SingleQuoted(lit) => format!("\"{}\"", escape_luau_string(lit)),
        }
    }

    fn simple_word_to_expr(&self, sw: &ShSimpleWord) -> String {
        match sw {
            ast::SimpleWord::Literal(s) => format!("\"{}\"", escape_luau_string(s)),
            ast::SimpleWord::Escaped(s) => format!("\"{}\"", escape_luau_string(s)),
            ast::SimpleWord::Param(param) => self.param_to_expr(param),
            ast::SimpleWord::Subst(subst) => self.param_subst_to_expr(subst),
            ast::SimpleWord::Star => "\"*\"".to_string(),
            ast::SimpleWord::Question => "\"?\"".to_string(),
            ast::SimpleWord::SquareOpen => "\"[\"".to_string(),
            ast::SimpleWord::SquareClose => "\"]\"".to_string(),
            ast::SimpleWord::Tilde => "sh.home()".to_string(),
            ast::SimpleWord::Colon => "\":\"".to_string(),
        }
    }

    fn param_to_expr(&self, param: &ast::Parameter<String>) -> String {
        match param {
            ast::Parameter::Var(name) => name.clone(),
            ast::Parameter::Positional(n) => format!("(args and args[{}] or \"\")", n),
            ast::Parameter::Question => "sh.last_exit_code".to_string(),
            ast::Parameter::Pound => "(args and #args or 0)".to_string(),
            ast::Parameter::Dollar => "\"$$\"".to_string(),
            ast::Parameter::At | ast::Parameter::Star => "table.concat(args or {}, \" \")".to_string(),
            ast::Parameter::Dash | ast::Parameter::Bang => "\"\"".to_string(),
        }
    }

    fn param_subst_to_expr(&self, subst: &ShParamSubst) -> String {
        match subst {
            ast::ParameterSubstitution::Command(cmds) => {
                // Wrap in sh.capture() to suppress printing during substitution
                if cmds.len() == 1 {
                    if let Some(expr) = self.top_level_to_expr(&cmds[0]) {
                        return format!("sh.capture(function() return {} end)", expr);
                    }
                }
                // Multi-command substitution
                if !cmds.is_empty() {
                    let mut inner = ShTranspiler::new();
                    for cmd in cmds {
                        inner.visit_top_level(cmd);
                    }
                    let body = inner.lines.join("; ");
                    return format!("sh.capture(function() {} end)", body);
                }
                "\"\"".to_string()
            }
            ast::ParameterSubstitution::Len(param) => {
                format!("#tostring({})", self.param_to_expr(param))
            }
            // ${var:-default} — use default if var is unset/empty
            ast::ParameterSubstitution::Default(_, param, default) => {
                let var = self.param_to_expr(param);
                let def = default.as_ref().map(|w| self.word_to_expr(w)).unwrap_or_else(|| "\"\"".to_string());
                format!("({v} ~= nil and {v} ~= \"\" and {v} or {d})", v = var, d = def)
            }
            // ${var:+alternate} — use alternate if var IS set
            ast::ParameterSubstitution::Alternative(_, param, alt) => {
                let var = self.param_to_expr(param);
                let alt_val = alt.as_ref().map(|w| self.word_to_expr(w)).unwrap_or_else(|| "\"\"".to_string());
                format!("({v} ~= nil and {v} ~= \"\" and {a} or \"\")", v = var, a = alt_val)
            }
            // ${var:=default} — assign default if unset (same as Default for our purposes)
            ast::ParameterSubstitution::Assign(_, param, default) => {
                let var = self.param_to_expr(param);
                let def = default.as_ref().map(|w| self.word_to_expr(w)).unwrap_or_else(|| "\"\"".to_string());
                format!("({v} ~= nil and {v} ~= \"\" and {v} or {d})", v = var, d = def)
            }
            ast::ParameterSubstitution::Arith(Some(arith)) => {
                format!("tostring({})", self.arith_to_expr(arith))
            }
            ast::ParameterSubstitution::Arith(None) => "\"0\"".to_string(),
            _ => "\"\" --[[ unsupported parameter substitution ]]".to_string(),
        }
    }

    /// Convert a bash arithmetic expression `$((expr))` to a Luau numeric expression.
    fn arith_to_expr(&self, arith: &ast::Arithmetic<String>) -> String {
        use ast::Arithmetic::*;
        match arith {
            Var(name) => format!("(tonumber({}) or 0)", name),
            Literal(n) => format!("{}", n),
            Add(l, r) => format!("({} + {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Sub(l, r) => format!("({} - {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Mult(l, r) => format!("({} * {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Div(l, r) => format!("math.floor({} / {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Modulo(l, r) => format!("({} % {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Pow(l, r) => format!("({} ^ {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            Less(l, r) => format!("(({} < {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            LessEq(l, r) => format!("(({} <= {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            Great(l, r) => format!("(({} > {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            GreatEq(l, r) => format!("(({} >= {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            Eq(l, r) => format!("(({} == {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            NotEq(l, r) => format!("(({} ~= {}) and 1 or 0)", self.arith_to_expr(l), self.arith_to_expr(r)),
            UnaryPlus(e) => self.arith_to_expr(e),
            UnaryMinus(e) => format!("(-{})", self.arith_to_expr(e)),
            LogicalNot(e) => format!("(({} == 0) and 1 or 0)", self.arith_to_expr(e)),
            BitwiseNot(e) => format!("bit32.bnot({})", self.arith_to_expr(e)),
            BitwiseAnd(l, r) => format!("bit32.band({}, {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            BitwiseOr(l, r) => format!("bit32.bor({}, {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            BitwiseXor(l, r) => format!("bit32.bxor({}, {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            ShiftLeft(l, r) => format!("bit32.lshift({}, {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            ShiftRight(l, r) => format!("bit32.rshift({}, {})", self.arith_to_expr(l), self.arith_to_expr(r)),
            LogicalAnd(l, r) => format!("((({l} ~= 0) and ({r} ~= 0)) and 1 or 0)", l = self.arith_to_expr(l), r = self.arith_to_expr(r)),
            LogicalOr(l, r) => format!("((({l} ~= 0) or ({r} ~= 0)) and 1 or 0)", l = self.arith_to_expr(l), r = self.arith_to_expr(r)),
            Sequence(exprs) => {
                // Comma operator: evaluate all, return last
                if exprs.is_empty() { "0".to_string() }
                else { self.arith_to_expr(exprs.last().unwrap()) }
            }
            // Assignment: var = expr
            Assign(name, expr) => {
                let val = self.arith_to_expr(expr);
                format!("(function() {n} = tostring({v}); return tonumber({n}) or 0 end)()", n = name, v = val)
            }
            // Increment/decrement
            PostIncr(name) => format!("(function() local __v = tonumber({n}) or 0; {n} = tostring(__v + 1); return __v end)()", n = name),
            PostDecr(name) => format!("(function() local __v = tonumber({n}) or 0; {n} = tostring(__v - 1); return __v end)()", n = name),
            PreIncr(name) => format!("(function() local __v = (tonumber({n}) or 0) + 1; {n} = tostring(__v); return __v end)()", n = name),
            PreDecr(name) => format!("(function() local __v = (tonumber({n}) or 0) - 1; {n} = tostring(__v); return __v end)()", n = name),
            Ternary(cond, then_val, else_val) => {
                format!("(({} ~= 0) and {} or {})",
                    self.arith_to_expr(cond),
                    self.arith_to_expr(then_val),
                    self.arith_to_expr(else_val))
            }
            // All variants are exhaustively handled above.
            // Sequence with empty is already handled, no remaining variants.
        }
    }

    fn word_to_string(&self, word: &ShWord) -> String {
        self.complex_word_to_string(&*word)
    }

    fn complex_word_to_string(&self, cw: &ShComplexWord) -> String {
        match cw {
            ast::ComplexWord::Single(w) => self.word_fragment_to_string(w),
            ast::ComplexWord::Concat(parts) => {
                parts.iter().map(|w| self.word_fragment_to_string(w)).collect::<Vec<_>>().join("")
            }
        }
    }

    fn word_fragment_to_string(&self, w: &ShWordFragment) -> String {
        match w {
            ast::Word::Simple(sw) => self.simple_word_to_string(sw),
            ast::Word::DoubleQuoted(parts) => {
                parts.iter().map(|sw| self.simple_word_to_string(sw)).collect::<Vec<_>>().join("")
            }
            ast::Word::SingleQuoted(lit) => lit.clone(),
        }
    }

    fn simple_word_to_string(&self, sw: &ShSimpleWord) -> String {
        match sw {
            ast::SimpleWord::Literal(s) | ast::SimpleWord::Escaped(s) => s.clone(),
            ast::SimpleWord::Param(param) => format!("${}", match param {
                ast::Parameter::Var(name) => name.clone(),
                ast::Parameter::Positional(n) => n.to_string(),
                ast::Parameter::Question => "?".to_string(),
                _ => String::new(),
            }),
            ast::SimpleWord::Star => "*".to_string(),
            ast::SimpleWord::Question => "?".to_string(),
            ast::SimpleWord::SquareOpen => "[".to_string(),
            ast::SimpleWord::SquareClose => "]".to_string(),
            ast::SimpleWord::Tilde => "~".to_string(),
            ast::SimpleWord::Colon => ":".to_string(),
            _ => String::new(),
        }
    }

}

fn escape_luau_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
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
        assert!(result.contains("sh.echo(\"hello world\")"), "got: {}", result);
    }

    #[test]
    fn test_ls_path() {
        let result = transpile("ls /workspace");
        assert!(result.contains("sh.ls({[1]=\"/workspace\"})"), "got: {}", result);
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
        assert!(result.contains("sh.ls({[1]=\"/workspace\"})"), "got: {}", result);
        assert!(result.contains("sh.grep({input=_in, [1]=\"foo\"})"), "got: {}", result);
    }

    #[test]
    fn test_pipe_three() {
        let result = transpile("cat /workspace/data.csv | grep pattern | head -5");
        assert!(result.contains("sh.pipe("), "got: {}", result);
        assert!(result.contains("sh.cat(\"/workspace/data.csv\")"), "got: {}", result);
        assert!(result.contains("sh.grep({input=_in, [1]=\"pattern\"})"), "got: {}", result);
        assert!(result.contains("sh.head({input=_in, n=5})"), "got: {}", result);
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
        assert!(result.contains("sh.echo(\"hello \" .. NAME)"), "got: {}", result);
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
        assert!(result.contains("sh.run(\"mycmd\", \"arg1\", {[1]=\"arg2\"})"), "got: {}", result);
    }

    #[test]
    fn test_unknown_command_bare() {
        // Bare unknown command: sh.run("mycmd", nil, nil)
        let result = transpile("mycmd");
        assert!(result.contains("sh.run(\"mycmd\", nil, nil)"), "got: {}", result);
    }

    #[test]
    fn test_redirect_write() {
        let result = transpile("echo hello > /workspace/out.txt");
        assert!(result.contains("sh.redirect_write("), "got: {}", result);
        assert!(result.contains("function() return"), "should wrap in function, got: {}", result);
    }

    #[test]
    fn test_single_quoted_string() {
        let result = transpile("echo 'hello world'");
        assert!(result.contains("sh.echo(\"hello world\")"), "got: {}", result);
    }

    #[test]
    fn test_mkdir() {
        let result = transpile("mkdir -p /workspace/subdir");
        assert!(result.contains("sh.mkdir({p=true, [1]=\"/workspace/subdir\"})"), "got: {}", result);
    }

    #[test]
    fn test_cd() {
        let result = transpile("cd /workspace/subdir");
        assert!(result.contains("sh.cd(\"/workspace/subdir\")"), "got: {}", result);
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
        assert!(result.contains("sh.cp({[1]=\"/workspace/a.txt\", [2]=\"/workspace/b.txt\"})"), "got: {}", result);
        let result = transpile("mv /workspace/a.txt /workspace/b.txt");
        assert!(result.contains("sh.mv(\"/workspace/a.txt\", \"/workspace/b.txt\")"), "got: {}", result);
    }

    #[test]
    fn test_test_command() {
        let result = transpile("test -f /workspace/file.txt");
        assert!(result.contains("sh.test(\"-f\", \"/workspace/file.txt\")"), "got: {}", result);
    }

    // ── End-to-end tests (transpile + execute in sandbox) ─────────

    const SHRT_SOURCE: &str = include_str!("../../shrt.luau");

    fn exec_sh(source: &str) -> String {
        let transpiled = transpile_sh(source).expect("transpile failed");
        let sandbox = crate::Sandbox::new().expect("sandbox creation failed");
        sandbox.register_module("shrt", SHRT_SOURCE).expect("shrt load failed");
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
        assert!(result.contains("d=true"), "expected d=true in output, got: {}", result);
        assert!(result.contains("[1]=\"/workspace\""), "expected positional path, got: {}", result);
    }

    #[test]
    fn test_tree_d_and_L_flags() {
        let result = transpile("tree /workspace -d -L 3");
        assert!(result.contains("d=true"), "expected d=true, got: {}", result);
        assert!(result.contains("L=\"3\""), "expected L=\"3\", got: {}", result);
        assert!(result.contains("[1]=\"/workspace\""), "expected positional path, got: {}", result);
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
        assert!(result.contains("UNSET") && result.contains("fallback"), "got: {}", result);
    }

    #[test]
    fn test_redirect_append() {
        let result = transpile("echo hello >> /workspace/out.txt");
        assert!(result.contains("sh.redirect_append("), "got: {}", result);
        assert!(result.contains("function() return"), "should wrap in function, got: {}", result);
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
        assert!(result.contains("sh.glob("), "should emit sh.glob(), got: {}", result);
        assert!(result.contains("*.txt"), "should have the pattern, got: {}", result);
    }

    #[test]
    fn test_glob_detection() {
        let result = transpile("echo *.csv");
        assert!(result.contains("sh.glob("), "echo with glob should expand, got: {}", result);
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
        assert!(result.contains("if sh.test(\"-f\", \"/workspace/data.csv\") then"), "got: {}", result);
        assert!(result.contains("sh.echo(\"found\")"), "got: {}", result);
        assert!(result.contains("end"), "got: {}", result);
    }

    #[test]
    fn test_if_else() {
        let result = transpile("if [ -d /workspace ]; then\n  echo yes\nelse\n  echo no\nfi");
        assert!(result.contains("if sh.test(\"-d\", \"/workspace\") then"), "got: {}", result);
        assert!(result.contains("else"), "got: {}", result);
        assert!(result.contains("end"), "got: {}", result);
    }

    #[test]
    fn test_if_elif_else() {
        let result = transpile(
            "if [ -f /a ]; then\n  echo file\nelif [ -d /b ]; then\n  echo dir\nelse\n  echo none\nfi"
        );
        assert!(result.contains("if sh.test(\"-f\", \"/a\") then"), "got: {}", result);
        assert!(result.contains("elseif sh.test(\"-d\", \"/b\") then"), "got: {}", result);
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
        assert!(result.contains("for _, i in ipairs({\"a\", \"b\", \"c\"}) do"), "got: {}", result);
        assert!(result.contains("sh.echo(i)"), "got: {}", result);
        assert!(result.contains("end"), "got: {}", result);
    }

    #[test]
    fn test_for_glob() {
        let result = transpile("for f in *.csv; do\n  echo $f\ndone");
        assert!(result.contains("sh.glob("), "should use sh.glob for patterns, got: {}", result);
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
        assert!(result.contains("sh.lines("), "should use sh.lines, got: {}", result);
        assert!(result.contains("line"), "got: {}", result);
    }

    // ── Phase 3e: case/esac ──────────────────────────────────────

    #[test]
    fn test_case_transpile() {
        let result = transpile("case \"$ext\" in\n*.csv) echo CSV ;;\n*.json) echo JSON ;;\n*) echo Unknown ;;\nesac");
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
        assert!(result.contains("sh.cat("), "got: {}", result);
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
        assert!(result.contains("sh.run(\"http\", \"get\", {[1]=\"http://example.com\"})"), "got: {}", result);
    }

    #[test]
    fn test_module_fs_read() {
        let result = transpile("fs read /data/file.txt");
        assert!(result.contains("sh.run(\"fs\", \"read\", {[1]=\"/data/file.txt\"})"), "got: {}", result);
    }

    #[test]
    fn test_module_compress_zip() {
        let result = transpile("compress zip /src /dst.zip");
        assert!(result.contains("sh.run(\"compress\", \"zip\", {[1]=\"/src\", [2]=\"/dst.zip\"})"), "got: {}", result);
    }

    #[test]
    fn test_module_flag_parsing() {
        let result = transpile("compress zip /src /dst.zip --level 9");
        assert!(result.contains("sh.run(\"compress\", \"zip\", {[1]=\"/src\", [2]=\"/dst.zip\", level=\"9\"})"), "got: {}", result);
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
        assert!(result.contains("sh.run(\"compress\", \"zip\", {input=_in, [1]=\"/src\", [2]=\"/dst.zip\"})"), "got: {}", result);
    }

    #[test]
    fn test_module_bare_name_shows_help() {
        let result = transpile("compress");
        assert!(result.contains("sh.run(\"compress\", nil, nil)"), "got: {}", result);
    }

    #[test]
    fn test_module_method_no_args() {
        let result = transpile("http get");
        assert!(result.contains("sh.run(\"http\", \"get\", nil)"), "got: {}", result);
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
        assert!(result.contains("sh.run(\"plot\", \"line\", {x=\"1,2\", y=\"3,4\"})"), "got: {}", result);
    }

    // ── E2E module dispatch (shell → Luau → sandbox execution) ──

    #[test]
    fn test_e2e_module_fs_read() {
        let result = transpile_sh("fs read /data/file.txt");
        assert!(result.is_ok());
        let luau = result.unwrap().luau_source;
        assert!(luau.contains("sh.run(\"fs\", \"read\", {[1]=\"/data/file.txt\"})"), "got: {}", luau);
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
        assert!(result.contains("l=true"), "should have l=true, got: {}", result);
        assert!(result.contains("a=true"), "should have a=true, got: {}", result);
        assert!(result.contains("[1]=\"/workspace\""), "should have positional path, got: {}", result);
    }

    #[test]
    fn test_ls_flags_separate() {
        let result = transpile("ls -l -a /workspace");
        assert!(result.contains("l=true"), "should have l=true, got: {}", result);
        assert!(result.contains("a=true"), "should have a=true, got: {}", result);
        assert!(result.contains("[1]=\"/workspace\""), "should have positional path, got: {}", result);
    }

    #[test]
    fn test_ls_flags_lah() {
        let result = transpile("ls -lah");
        assert!(result.contains("l=true"), "should have l=true, got: {}", result);
        assert!(result.contains("a=true"), "should have a=true, got: {}", result);
        assert!(result.contains("h=true"), "should have h=true, got: {}", result);
    }

    #[test]
    fn test_ls_bare() {
        let result = transpile("ls");
        assert!(result.contains("sh.ls()"), "should be bare call, got: {}", result);
    }

    #[test]
    fn test_e2e_ls_flags() {
        // ls -l should produce long format — verify transpilation emits correct table
        let result = transpile("ls -l /workspace");
        assert!(result.contains("sh.ls({l=true, [1]=\"/workspace\"})"), "got: {}", result);
    }

    #[test]
    fn test_e2e_ls_long_flag() {
        let result = transpile("ls --long /workspace");
        assert!(result.contains("sh.ls({long=true, [1]=\"/workspace\"})"), "got: {}", result);
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
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("ls /workspace").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert!(!output.contains(".hidden"), "ls should hide dotfiles, got: {}", output);
        assert!(output.contains("README.md"), "ls should show visible files, got: {}", output);
        assert!(output.contains("data.csv"), "ls should show visible files, got: {}", output);
    }

    #[test]
    fn test_e2e_ls_a_shows_dotfiles() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "hello").unwrap();
        std::fs::write(dir.path().join(".hidden"), "secret").unwrap();

        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("ls -a /workspace").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert!(output.contains(".hidden"), "ls -a should show dotfiles, got: {}", output);
        assert!(output.contains("README.md"), "ls -a should show visible files, got: {}", output);
    }

    #[test]
    fn test_e2e_ls_l_shows_sizes() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("small.txt"), "hi").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("ls -l /workspace").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        // Should show file with size (2 bytes for "hi")
        assert!(output.contains("file"), "should have file type, got: {}", output);
        assert!(output.contains("2"), "should show size 2 for 'hi', got: {}", output);
        assert!(output.contains("small.txt"), "should show filename, got: {}", output);
        // Should show dir with "-" for size
        assert!(output.contains("dir"), "should have dir type, got: {}", output);
        assert!(output.contains("subdir"), "should show dirname, got: {}", output);
    }

    #[test]
    fn test_e2e_ls_l_no_crash_on_binary() {
        let dir = tempfile::TempDir::new().unwrap();
        // Write binary content (invalid UTF-8)
        std::fs::write(dir.path().join("binary.bin"), &[0xFF, 0xFE, 0x00, 0x01, 0x80]).unwrap();

        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("ls -l /workspace").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        // Should NOT crash — uses fs.size() instead of fs.read()
        assert!(output.contains("binary.bin"), "should show binary file, got: {}", output);
        assert!(output.contains("5"), "should show size 5, got: {}", output);
    }

    #[test]
    fn test_e2e_ls_lh_human_readable() {
        let dir = tempfile::TempDir::new().unwrap();
        // Write a ~1.5KB file
        let content = "x".repeat(1536);
        std::fs::write(dir.path().join("medium.txt"), &content).unwrap();

        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("ls -lh /workspace").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        // Should show human-readable size like "1.5K"
        assert!(output.contains("1.5K"), "should show human-readable size, got: {}", output);
        assert!(output.contains("medium.txt"), "should show filename, got: {}", output);
    }

    // ── 1g: sh.run() integration tests ──────────────────────────

    #[test]
    fn test_e2e_shrun_fs_read_displays_string() {
        // `fs read /workspace/file.txt` via sh.run() should display the file contents
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();

        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace", dir.path().display())).unwrap();
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
        assert!(output.contains("a") && output.contains("b"), "should contain column data, got: {}", output);
    }

    #[test]
    fn test_e2e_shrun_json_decode_displays_json() {
        // `json decode '{"key":"value"}'` via sh.run() should display pretty JSON
        let sandbox = crate::Sandbox::new().unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("json decode '{\"key\":\"value\"}'").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert!(output.contains("key") && output.contains("value"), "should display JSON, got: {}", output);
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
        assert!(err.message.contains("command not found"), "got: {}", err.message);
    }

    // ── E2E redirect tests with mounted filesystem ────────────────

    #[test]
    fn test_e2e_redirect_write_creates_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace:rw", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        // echo "test" > /workspace/out.txt should create the file with no stdout
        let transpiled = transpile_sh("echo test > /workspace/out.txt").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert_eq!(output, "", "redirect should suppress stdout, got: {}", output);
        let content = std::fs::read_to_string(dir.path().join("out.txt")).unwrap();
        assert_eq!(content, "test\n", "file should contain 'test\\n', got: {:?}", content);
    }

    #[test]
    fn test_e2e_redirect_write_relative_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/:rw", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        // Relative path "foo.txt" should resolve via shell cwd (/) to /foo.txt
        let transpiled = transpile_sh("echo hello > foo.txt").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert_eq!(output, "", "redirect should suppress stdout, got: {}", output);
        let content = std::fs::read_to_string(dir.path().join("foo.txt")).unwrap();
        assert_eq!(content, "hello\n", "file should contain 'hello\\n', got: {:?}", content);
    }

    #[test]
    fn test_e2e_redirect_append() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("log.txt"), "line1\n").unwrap();
        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace:rw", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        let transpiled = transpile_sh("echo line2 >> /workspace/log.txt").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert_eq!(output, "", "redirect should suppress stdout, got: {}", output);
        let content = std::fs::read_to_string(dir.path().join("log.txt")).unwrap();
        assert_eq!(content, "line1\nline2\n", "should append with newlines, got: {:?}", content);
    }

    #[test]
    fn test_e2e_redirect_append_creates_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace:rw", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        // >> should create the file if it doesn't exist
        let transpiled = transpile_sh("echo first >> /workspace/new.txt").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert_eq!(output, "", "redirect should suppress stdout, got: {}", output);
        let content = std::fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert_eq!(content, "first\n", "should create file with trailing newline, got: {:?}", content);
    }

    #[test]
    fn test_e2e_bare_redirect_creates_empty_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut table = crate::mount::MountTable::new();
        table.parse_and_add(&format!("{}:/workspace:rw", dir.path().display())).unwrap();
        let sandbox = crate::Sandbox::with_mounts(table).unwrap();
        sandbox.register_module("shrt", SHRT_SOURCE).unwrap();

        // > file with no command should create/truncate
        let transpiled = transpile_sh("> /workspace/empty.txt").unwrap();
        let output = sandbox.exec(&transpiled.luau_source).unwrap();
        assert_eq!(output, "");
        let content = std::fs::read_to_string(dir.path().join("empty.txt")).unwrap();
        assert_eq!(content, "", "bare redirect should create empty file");
    }
}
