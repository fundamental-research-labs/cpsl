# Module API Design Guide

Every sandbox module must feel native in three languages simultaneously. If any mapping feels unnatural, the API is wrong.

## The Three-Language Test

```
Lua:    module.method(arg1, arg2, {opt="val"})
Python: module.method(arg1, arg2, opt="val")
Shell:  module method arg1 arg2 --opt val
```

Every function signature must pass all three forms before shipping.

## Hard Constraints

### 1. Option names must be valid Python identifiers

Never use Python reserved words as option names:

`from`, `in`, `is`, `not`, `and`, `or`, `class`, `for`, `if`, `import`, `return`, `with`, `as`, `del`, `elif`, `else`, `except`, `finally`, `global`, `lambda`, `nonlocal`, `pass`, `raise`, `try`, `while`, `yield`

This eliminates an entire class of cross-language bugs.

### 2. Option names use snake_case

```
Lua:    {input_format="markdown"}
Python: input_format="markdown"
Shell:  --input-format markdown  (auto-normalized to input_format)
```

One convention, zero translation between Lua and Python. Shell kebab-case is a trivial `s/-/_/` transformation handled by the transpiler.

### 3. Prefer positional args for required parameters

Options are for optional overrides. If a parameter is always required, make it positional.

```
-- Good: formats are always required for string conversion
pandoc.render(text, "markdown", "html")

-- Bad: formats as options when they're mandatory
pandoc.render(text, {input_format="markdown", output_format="html"})
```

### 4. POSIX shell builtins are syntactic sugar

```
cat file       ->  sh.cat()      ->  fs.read()
ls dir         ->  sh.ls()       ->  fs.list()
mkdir -p dir   ->  sh.mkdir()    ->  fs.mkdir()
cp src dst     ->  sh.cp()       ->  fs.read() + fs.write()
mv src dst     ->  sh.mv()       ->  fs.rename()
rm file        ->  sh.rm()       ->  fs.remove()
```

The `sh.*` runtime is a POSIX compatibility layer. The modules (`fs`, `http`, `compress`, `pandoc`) are the real API. Both paths work; neither is "wrong."

### 5. Shell dispatch: `module method args`

When the shell transpiler sees a command name matching a known module, it dispatches:

```
pandoc convert in.md out.html --standalone
```

Becomes:

```lua
pandoc.convert("in.md", "out.html", {standalone=true})
```

- `args[0]` = method name
- Remaining positional args passed directly
- `--flag value` pairs collected into a trailing Lua table
- `--flag` (no value) emits `flag=true`
- Kebab-case auto-normalizes to snake_case

No validation at transpile time. Runtime `__index` metatable catches unknown methods with helpful errors.

## Current Module Audit

### fs

All positional-only. Maps cleanly:

| Shell | Module |
|---|---|
| `fs read /path` | `fs.read("/path")` |
| `fs write /path "content"` | `fs.write("/path", "content")` |
| `fs list /path` | `fs.list("/path")` |
| `fs exists /path` | `fs.exists("/path")` |
| `fs mkdir /path` | `fs.mkdir("/path")` |
| `fs rename /a /b` | `fs.rename("/a", "/b")` |
| `fs remove /path` | `fs.remove("/path")` |

### http

Option names `headers`, `body` are Python-safe and snake_case.

| Shell | Module |
|---|---|
| `http get "url"` | `http.get("url")` |
| `http post "url" --body "data"` | `http.post("url", {body="data"})` |
| `http request POST "url" --body "data"` | `http.request("POST", "url", {body="data"})` |

### compress

All positional-only. Maps trivially:

| Shell | Module |
|---|---|
| `compress zip /src /dst.zip` | `compress.zip("/src", "/dst.zip")` |
| `compress unzip /archive /dest` | `compress.unzip("/archive", "/dest")` |

### pandoc

Mixed positional + optional opts table:

| Shell | Module |
|---|---|
| `pandoc convert in.md out.html` | `pandoc.convert("in.md", "out.html")` |
| `pandoc convert in.txt out.html --input-format markdown` | `pandoc.convert("in.txt", "out.html", {input_format="markdown"})` |
| `pandoc render "$text" markdown html` | `pandoc.render(text, "markdown", "html")` |
| `pandoc read /doc.docx` | `pandoc.read("/doc.docx")` |
| `pandoc formats` | `pandoc.formats()` |

## Good vs Bad API Design

### Good

```lua
-- Positional for required, opts for optional
pandoc.convert(input, output, {input_format="markdown"})
http.post(url, {body="data", headers={["Content-Type"]="application/json"}})
```

### Bad

```lua
-- Python reserved word as option name
pandoc.convert(input, output, {from="markdown"})  -- WRONG: `from` is a Python keyword

-- CamelCase option name
http.post(url, {contentType="json"})  -- WRONG: should be content_type

-- Required params as options
pandoc.render({text="...", from="md", to="html"})  -- WRONG: all three are required, make positional

-- Overly complex option nesting
fs.read({path="/data/file.txt", encoding="utf8"})  -- WRONG: path is required, make positional
```

## Dual Signature

Every Lua API function must accept **two calling conventions**:

1. **Ordered args**: `fn(arg1, arg2, {opt="val"})` — natural for Lua/Python callers
2. **Single table with named+positional keys**: `fn({[1]=arg1, [2]=arg2, opt="val"})` — natural for shell dispatch via `sh.run()`

Shell translates `module method arg1 --opt val` into `module.method({[1]="arg1", opt="val"})`. If the Lua function only accepts ordered args, shell gets a type error.

### Rust pattern for argument validation

All module functions use `validate_args` from `sandbox.rs`. It takes a `MultiValue`, a `&[Param]` slice (from `ModuleDoc`), and the function name. It handles both calling conventions automatically (positional args and single-table form) and returns a `Vec<Value>` aligned with the params — one entry per param, `Nil` for missing optionals. Errors are human-readable: `"module.fn: missing required argument 'name' (type)"`.

```rust
// Define params via ModuleDoc (typically a static):
static MY_DOC: ModuleDoc = ModuleDoc {
    name: "mymod",
    summary: "Example module",
    functions: &[FnDoc {
        name: "process",
        description: "Process a file.",
        params: &[
            Param { name: "path", short: None, typ: ParamType::String, required: true },
            Param { name: "mode", short: Some('m'), typ: ParamType::String, required: false },
        ],
        returns: ReturnType::String,
    }],
};

// In the function implementation:
lua.create_function(|_, args: MultiValue| {
    let validated = validate_args(&args, MY_DOC.functions[0].params, "mymod.process")?;
    let path = match &validated[0] {
        Value::String(s) => s.to_string_lossy().to_string(),
        _ => unreachable!("validate_args ensures string"),
    };
    let mode = match &validated[1] {
        Value::String(s) => Some(s.to_string_lossy().to_string()),
        Value::Nil => None, // optional param not provided
        _ => unreachable!("validate_args ensures string"),
    };
    // ... use path, mode
})
```

This single pattern replaces all ad-hoc argument extraction. The `Param` metadata serves double duty: `validate_args` uses it for runtime validation, and the help system uses it to generate signatures and shell flag documentation.

### Rust pattern for functions where first arg is a table (ambiguous)

When the first arg IS a table (e.g., `xml.query(doc, path)`, `plot.line(x, y, opts?)`), disambiguate by checking for the second argument:

```rust
lua.create_function(|lua, (first, second_opt): (mlua::Table, Option<String>)| {
    match second_opt {
        Some(s) => { /* ordered form: fn(table_arg, string_arg) */ }
        None => { /* table form: fn({[1]=table_arg, [2]="string_arg"}) */ }
    }
})
```

For functions where the first arg is data (histogram data, heatmap matrix) vs named-params table, check for the `output` key:

```rust
Value::Table(t) if has_output_key(t) => { /* table form */ }
_ => { /* ordered form: first arg is data */ }
```

### Special cases: encode-style functions

Functions where the first arg IS a table value (json.encode, yaml.encode, xml.encode, xml.text) stay **ordered-only**. The table is the value, not named params. From shell, these work because the shell passes the string representation in `[1]`, which the function receives as text.

## Short Aliases

Every option must have a short alias for natural shell flag usage (`-d ","` → `d=","`):

```rust
// In option extraction:
let delimiter = t.get::<String>("delimiter")
    .or_else(|_| t.get::<String>("d"))?;    // short alias

let header = t.get::<bool>("header")
    .or_else(|_| t.get::<bool>("h"))?;      // short alias
```

### Common short aliases

| Long | Short | Used by |
|------|-------|---------|
| `delimiter` | `d` | csv |
| `quote` | `q` | csv |
| `header` | `h` | csv |
| `output` | `o` | plot |
| `title` | `t` | plot |
| `xlabel` | `xl` | plot |
| `ylabel` | `yl` | plot |
| `width` | `w` | plot |
| `height` | `h` | plot |
| `legend` | `l` | plot |
| `grid` | `g` | plot |

## Description Writing Guidelines

Descriptions must be **runtime-agnostic**. They are displayed in Lua REPL, shell help, and potentially other contexts. The structured `ParamType` and `ReturnType` enums already encode type information and are rendered by the help formatter — descriptions should not duplicate or contradict them.

### Rules

1. **Never reference a specific runtime** in descriptions. No "Lua value", "Lua table", "Python dict", etc. Use neutral terms: "value", "native value", "collection", "mapping".

2. **Don't enumerate return types in prose** — that's what `ReturnType` is for. The formatter renders `-> value`, `-> table`, `-> string` automatically from the enum.

   ```rust
   // Bad: duplicates type info and uses Lua terms
   description: "Parse a JSON string into a Lua value (table, string, number, boolean, or nil)."

   // Good: describes behavior, lets ReturnType handle the rest
   description: "Parse a JSON string into a native value."
   ```

3. **Don't embed calling convention examples** in descriptions. The help formatter generates signatures from `Param` metadata. If you need to document option fields, list them as a flat schema:

   ```rust
   // Bad: Lua-specific syntax in description
   description: "Read a file.\n    Named: doc.read({path=\"/f.xlsx\"})\n    Positional: doc.read(\"/f.xlsx\")"

   // Good: just describe behavior and options
   description: "Read a document file and extract its text. Format auto-detected from extension."
   ```

4. **Use neutral collection syntax** when showing data shapes. Prefer `[1,2,3]` or prose ("list of strings") over `{1,2,3}` (Lua) or `(1,2,3)` (Python).

5. **Option schemas** can use `{key, key?, key?}` notation — this is a structural shorthand, not Lua syntax. Just don't mix in runtime-specific terms.

## Adding a New Module

1. Define the function signatures. Run them through the three-language test.
2. Verify all option names against the Python reserved words list.
3. Implement dual-signature support for every function (see patterns above).
4. Add short aliases for every option.
5. Create `core/src/{module}.rs` following the `csv_mod.rs` pattern.
6. Register in `Sandbox::build()`.
7. Add to `register_global_help()` in `sandbox.rs`.
8. Write integration tests in `core/tests/{module}_integration.rs` covering both calling conventions.
9. Write shell round-trip tests proving `module method args --flags` works end-to-end.
10. **Update the agent's `TOOL_DESCRIPTION`** in `backend/agent/tools/local_sandbox.py` to list the new module and its key functions. If the LLM doesn't know a module exists, it will fall back to Python/shell instead of using the native Lua API.
11. **Update the help known-modules lists** in both `sandbox.rs` (`register_global_help`) and `runtime/shrt.luau` (`sh.shell_help`) so that `help()` displays the new module.

**Note**: There is no `KNOWN_MODULES` list for shell dispatch. Shell dispatch is dynamic — `sh.run()` checks `_G[name]` at runtime. Any module registered as a global table is automatically available from shell. However, the help command and agent tool description use static lists that must be updated manually.

## Shell Parity Checklist

Use this checklist for every new module or when adding functions to existing modules:

- [ ] **Dual signature**: Every function accepts both ordered args and single-table form
- [ ] **Short aliases**: Every option has a 1-2 char short alias
- [ ] **Table form tests**: Integration tests for `module.fn({[1]=arg, opt=val})` calling convention
- [ ] **Shell dispatch test**: `module method arg --flag val` transpiles and executes correctly via `sh.run()`
- [ ] **Bare module test**: `module` with no args shows `help()` output
- [ ] **Return value serialization**: Table return values display correctly in shell (auto-JSON via `sh.run()`)
- [ ] **Error messages**: Missing/wrong args give helpful hints (not cryptic Lua type errors)
- [ ] **No transpiler leakage**: Error messages must never mention "transpile", "transpiler", "ShellSyntaxError", or other internal machinery. Users should feel like they're in shell/Python/Lua — not that their code is being translated. Use generic "syntax error:" prefixes and let the parser's message speak for itself.
- [ ] **Runtime-agnostic descriptions**: No "Lua value", "Lua table", calling convention examples, or type enumerations in description strings
- [ ] **Python reserved words**: No option names collide with Python keywords
- [ ] **Snake case options**: All option names use `snake_case` (shell `--kebab-case` auto-normalizes)
