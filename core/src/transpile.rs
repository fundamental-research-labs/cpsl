//! Pure-Rust Python-to-Luau transpiler.
//!
//! Uses `rustpython-parser` to parse Python source into an AST,
//! then walks it in a single pass emitting Luau code + a source map.

use rustpython_parser::ast::*;
use rustpython_parser::text_size::TextRange;
use rustpython_parser::{self as rp, Mode};
use std::collections::{HashMap, HashSet};

/// Result of transpiling Python to Luau.
pub struct TranspileResult {
    pub luau_source: String,
    pub source_map: HashMap<usize, usize>, // luau_line → python_line
    pub warnings: Vec<String>,
}

/// Transpile Python source to Luau.
pub fn transpile(source: &str) -> Result<TranspileResult, String> {
    let ast =
        rp::parse(source, Mode::Module, "<input>").map_err(|e| format!("SyntaxError: {}", e))?;

    let module = match ast {
        Mod::Module(m) => m,
        _ => return Err("expected module".to_string()),
    };

    let mut t = Transpiler::new(source);
    t.emit_raw(r#"local py = require("pyrt")"#);
    for stmt in &module.body {
        t.visit_stmt(stmt);
    }

    Ok(TranspileResult {
        luau_source: t.lines.join("\n"),
        source_map: t.source_map,
        warnings: t.warnings,
    })
}

// ── Scope tracking ──────────────────────────────────────────────

struct ScopeFrame {
    declared: HashSet<String>,
    types: HashMap<String, ExprType>,
}

struct ScopeTracker {
    scopes: Vec<ScopeFrame>,
}

impl ScopeTracker {
    fn new() -> Self {
        Self {
            scopes: vec![ScopeFrame {
                declared: HashSet::new(),
                types: HashMap::new(),
            }],
        }
    }
    fn enter(&mut self) {
        self.scopes.push(ScopeFrame {
            declared: HashSet::new(),
            types: HashMap::new(),
        });
    }
    fn exit(&mut self) {
        self.scopes.pop();
    }
    fn declare(&mut self, name: &str) {
        if let Some(s) = self.scopes.last_mut() {
            s.declared.insert(name.to_string());
        }
    }
    fn is_declared(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.declared.contains(name))
    }

    /// Record the inferred type for a variable in the current scope.
    fn set_type(&mut self, name: &str, ty: ExprType) {
        if let Some(s) = self.scopes.last_mut() {
            s.types.insert(name.to_string(), ty);
        }
    }

    /// Look up the inferred type for a variable, searching from inner to outer scope.
    fn get_type(&self, name: &str) -> Option<ExprType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.types.get(name) {
                return Some(*ty);
            }
        }
        None
    }
}

mod maps;

use maps::{
    builtin_map, direct_method_map, is_passthrough_module, python_from_import_to_luau,
    python_module_to_luau,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExprType {
    Int,
    Float,
    Str,
    Bool,
    Unknown,
}

impl ExprType {
    /// True if this type is numeric (Int or Float).
    fn is_num(self) -> bool {
        matches!(self, ExprType::Int | ExprType::Float)
    }
}

// ── Helper to get identifier string ────────────────────────────

fn ident(id: &Identifier) -> &str {
    id.as_str()
}

// ── Transpiler ──────────────────────────────────────────────────

struct Transpiler<'a> {
    source: &'a str,
    lines: Vec<String>,
    source_map: HashMap<usize, usize>,
    scopes: ScopeTracker,
    indent: usize,
    current_line: usize,
    warnings: Vec<String>,
    temp_counter: usize,
    /// Maps import aliases to original module names (e.g. "m" → "math")
    import_aliases: HashMap<String, String>,
    /// Stack of break-flag variable names for for/else and while/else support.
    /// When non-empty, `break` statements emit `flag = true` before `break`.
    break_flag_stack: Vec<String>,
}

impl<'a> Transpiler<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            lines: Vec::new(),
            source_map: HashMap::new(),
            scopes: ScopeTracker::new(),
            indent: 0,
            current_line: 1,
            warnings: Vec::new(),
            temp_counter: 0,
            import_aliases: HashMap::new(),
            break_flag_stack: Vec::new(),
        }
    }

    fn temp_var(&mut self) -> String {
        self.temp_counter += 1;
        format!("__tmp{}", self.temp_counter)
    }

    fn py_line(&self, offset: usize) -> usize {
        let offset = offset.min(self.source.len());
        self.source[..offset].matches('\n').count() + 1
    }

    fn emit_raw(&mut self, code: &str) {
        self.lines.push(code.to_string());
        self.current_line += 1;
    }

    fn emit(&mut self, code: &str, range: TextRange) {
        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}{}", indent, code));
        self.source_map
            .insert(self.current_line, self.py_line(range.start().into()));
        self.current_line += 1;
    }

    #[allow(dead_code)]
    fn emit_no_map(&mut self, code: &str) {
        let indent = "    ".repeat(self.indent);
        self.lines.push(format!("{}{}", indent, code));
        self.current_line += 1;
    }

    fn warn(&mut self, range: TextRange, msg: &str) {
        self.warnings.push(format!(
            "line {}: {}",
            self.py_line(range.start().into()),
            msg
        ));
    }

    // ── Type inference (read-only AST walk) ─────────────────────

    fn expr_type(&self, node: &Expr) -> ExprType {
        match node {
            Expr::Constant(c) => match &c.value {
                Constant::Int(_) => ExprType::Int,
                Constant::Float(_) => ExprType::Float,
                Constant::Bool(_) => ExprType::Bool,
                Constant::Str(_) => ExprType::Str,
                _ => ExprType::Unknown,
            },
            Expr::Name(n) => match ident(&n.id) {
                "True" | "False" => ExprType::Bool,
                name => self.scopes.get_type(name).unwrap_or(ExprType::Unknown),
            },
            Expr::BinOp(b) => {
                let lt = self.expr_type(&b.left);
                let rt = self.expr_type(&b.right);
                match b.op {
                    Operator::Add
                    | Operator::Sub
                    | Operator::Mult
                    | Operator::Mod
                    | Operator::Pow => {
                        if lt.is_num() && rt.is_num() {
                            // Float + anything = Float (Python promotion)
                            if lt == ExprType::Float || rt == ExprType::Float {
                                ExprType::Float
                            } else {
                                ExprType::Int
                            }
                        } else if matches!(b.op, Operator::Add)
                            && lt == ExprType::Str
                            && rt == ExprType::Str
                        {
                            ExprType::Str
                        } else {
                            ExprType::Unknown
                        }
                    }
                    // / always returns float in Python
                    Operator::Div => {
                        if lt.is_num() && rt.is_num() {
                            ExprType::Float
                        } else {
                            ExprType::Unknown
                        }
                    }
                    // // returns int if both int, float if either is float
                    Operator::FloorDiv => {
                        if lt.is_num() && rt.is_num() {
                            if lt == ExprType::Float || rt == ExprType::Float {
                                ExprType::Float
                            } else {
                                ExprType::Int
                            }
                        } else {
                            ExprType::Unknown
                        }
                    }
                    _ => ExprType::Unknown,
                }
            }
            Expr::UnaryOp(u) => match u.op {
                UnaryOp::Not => ExprType::Bool,
                UnaryOp::USub | UnaryOp::UAdd => {
                    let t = self.expr_type(&u.operand);
                    if t.is_num() {
                        t
                    } else {
                        ExprType::Unknown
                    }
                }
                _ => ExprType::Unknown,
            },
            Expr::Compare(_) => ExprType::Bool,
            Expr::BoolOp(b) => {
                if b.values.iter().all(|v| self.expr_type(v) == ExprType::Bool) {
                    ExprType::Bool
                } else {
                    ExprType::Unknown
                }
            }
            Expr::Call(c) => {
                if let Expr::Name(name) = c.func.as_ref() {
                    match ident(&name.id) {
                        "len" | "int" => ExprType::Int,
                        "float" => ExprType::Float,
                        "abs" | "min" | "max" | "sum" => {
                            // Preserve float-ness from first arg if available
                            if let Some(first) = c.args.first() {
                                let t = self.expr_type(first);
                                if t.is_num() {
                                    t
                                } else {
                                    ExprType::Int
                                }
                            } else {
                                ExprType::Int
                            }
                        }
                        "str" => ExprType::Str,
                        "bool" | "isinstance" => ExprType::Bool,
                        _ => ExprType::Unknown,
                    }
                } else {
                    ExprType::Unknown
                }
            }
            _ => ExprType::Unknown,
        }
    }

    /// Emit an expression suitable for use as a boolean condition.
    /// Skips `py.bool()` wrapping when the expression is already boolean.
    fn emit_bool_expr(&mut self, node: &Expr) -> String {
        let ty = self.expr_type(node);
        let e = self.expr(node);
        if ty == ExprType::Bool {
            e
        } else {
            format!("py.bool({})", e)
        }
    }

    // ── Expressions → String ────────────────────────────────────

    fn expr(&mut self, node: &Expr) -> String {
        match node {
            Expr::Constant(c) => self.visit_constant(c),
            Expr::Name(n) => self.visit_name(n),
            Expr::BinOp(b) => self.visit_binop(b),
            Expr::UnaryOp(u) => self.visit_unaryop(u),
            Expr::BoolOp(b) => self.visit_boolop(b),
            Expr::Compare(c) => self.visit_compare(c),
            Expr::Call(c) => self.visit_call(c),
            Expr::Subscript(s) => self.visit_subscript(s),
            Expr::Attribute(a) => self.visit_attribute(a),
            Expr::List(l) => self.visit_list(l),
            Expr::Dict(d) => self.visit_dict(d),
            Expr::Tuple(t) => self.visit_tuple(t),
            Expr::JoinedStr(j) => self.visit_joinedstr(j),
            Expr::IfExp(i) => self.visit_ifexp(i),
            Expr::Lambda(l) => self.visit_lambda(l),
            Expr::ListComp(l) => self.visit_listcomp(l),
            Expr::DictComp(d) => self.visit_dictcomp(d),
            Expr::Slice(s) => {
                self.warn(s.range, "unexpected standalone slice");
                "nil".to_string()
            }
            Expr::FormattedValue(f) => self.expr(&f.value),
            Expr::NamedExpr(n) => {
                self.warn(n.range, "walrus operator := not supported");
                "\"<unsupported: walrus>\"".to_string()
            }
            Expr::Starred(s) => {
                self.warn(s.range, "starred expressions not supported");
                self.expr(&s.value)
            }
            Expr::GeneratorExp(g) => {
                self.warn(g.range, "generator expression treated as list comp");
                let tmp = self.temp_var();
                let elt = self.expr(&g.elt);
                let mut parts = vec![format!("(function() local {} = py.list({{}}); ", tmp)];
                self.emit_comp_generators(
                    &mut parts,
                    &g.generators,
                    &format!("py.append({}, {})", tmp, elt),
                );
                parts.push(format!(" return {} end)()", tmp));
                parts.join("")
            }
            _ => "nil --[[unsupported expr]]".to_string(),
        }
    }

    fn visit_constant(&self, node: &ExprConstant) -> String {
        match &node.value {
            Constant::None => "py.None".to_string(),
            Constant::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Constant::Int(i) => format!("{}", i),
            Constant::Float(f) => format!("{}", f),
            Constant::Str(s) => {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t");
                format!("\"{}\"", escaped)
            }
            Constant::Bytes(b) => {
                format!("\"{}\"", String::from_utf8_lossy(b))
            }
            Constant::Ellipsis => "nil --[[...]]".to_string(),
            _ => "nil".to_string(),
        }
    }

    fn visit_name(&self, node: &ExprName) -> String {
        match ident(&node.id) {
            "None" => "py.None".to_string(),
            "True" => "true".to_string(),
            "False" => "false".to_string(),
            // Map bare builtin references (e.g. key=len) to their py.* equivalents
            name => {
                if let Some(py_name) = builtin_map(name) {
                    // Only map if the name isn't shadowed by a local variable
                    if !self.scopes.is_declared(name) {
                        return py_name.to_string();
                    }
                }
                name.to_string()
            }
        }
    }

    fn visit_binop(&mut self, node: &ExprBinOp) -> String {
        let lt = self.expr_type(&node.left);
        let rt = self.expr_type(&node.right);
        let left = self.expr(&node.left);
        let right = self.expr(&node.right);
        let both_num = lt.is_num() && rt.is_num();
        let both_str = lt == ExprType::Str && rt == ExprType::Str;
        let either_bool = lt == ExprType::Bool || rt == ExprType::Bool;
        match node.op {
            Operator::Add => {
                if both_num {
                    format!("({} + {})", left, right)
                } else if both_str {
                    format!("({} .. {})", left, right)
                } else {
                    format!("py.add({}, {})", left, right)
                }
            }
            Operator::Sub => {
                if either_bool {
                    format!("py.sub({}, {})", left, right)
                } else {
                    format!("({} - {})", left, right)
                }
            }
            Operator::Mult => {
                if both_num {
                    format!("({} * {})", left, right)
                } else {
                    format!("py.mul({}, {})", left, right)
                }
            }
            Operator::Div => format!("py.truediv({}, {})", left, right),
            Operator::FloorDiv => format!("py.floordiv({}, {})", left, right),
            Operator::Mod => format!("py.mod({}, {})", left, right),
            Operator::Pow => {
                if both_num {
                    format!("({} ^ {})", left, right)
                } else {
                    format!("py.pow({}, {})", left, right)
                }
            }
            Operator::BitAnd => format!("bit32.band({}, {})", left, right),
            Operator::BitOr => format!("bit32.bor({}, {})", left, right),
            Operator::BitXor => format!("bit32.bxor({}, {})", left, right),
            Operator::LShift => format!("bit32.lshift({}, {})", left, right),
            Operator::RShift => format!("bit32.rshift({}, {})", left, right),
            Operator::MatMult => {
                self.warn(node.range, "matrix multiply not supported");
                format!("({} --[[@ unsupported]] {})", left, right)
            }
        }
    }

    fn visit_unaryop(&mut self, node: &ExprUnaryOp) -> String {
        let operand_type = self.expr_type(&node.operand);
        let operand = self.expr(&node.operand);
        match node.op {
            UnaryOp::USub => format!("(-{})", operand),
            UnaryOp::UAdd => operand, // Luau has no unary +, it's a no-op for numbers
            UnaryOp::Not => {
                if operand_type == ExprType::Bool {
                    format!("(not {})", operand)
                } else {
                    format!("(not py.bool({}))", operand)
                }
            }
            UnaryOp::Invert => format!("(-{} - 1)", operand),
        }
    }

    fn visit_boolop(&mut self, node: &ExprBoolOp) -> String {
        let values: Vec<String> = node.values.iter().map(|v| self.expr(v)).collect();
        match node.op {
            BoolOp::And => values.join(" and "),
            BoolOp::Or => values.join(" or "),
        }
    }

    fn visit_compare(&mut self, node: &ExprCompare) -> String {
        let mut parts = Vec::new();
        let mut left_type = self.expr_type(&node.left);
        let mut left = self.expr(&node.left);

        for (op, comparator) in node.ops.iter().zip(node.comparators.iter()) {
            let right_type = self.expr_type(comparator);
            let right = self.expr(comparator);
            let can_raw_eq = (left_type == right_type
                || (left_type.is_num() && right_type.is_num()))
                && matches!(
                    left_type,
                    ExprType::Int | ExprType::Float | ExprType::Str | ExprType::Bool
                );
            let part = match op {
                CmpOp::Eq => {
                    if can_raw_eq {
                        format!("({} == {})", left, right)
                    } else {
                        format!("py.eq({}, {})", left, right)
                    }
                }
                CmpOp::NotEq => {
                    if can_raw_eq {
                        format!("({} ~= {})", left, right)
                    } else {
                        format!("(not py.eq({}, {}))", left, right)
                    }
                }
                CmpOp::Lt => format!("({} < {})", left, right),
                CmpOp::LtE => format!("({} <= {})", left, right),
                CmpOp::Gt => format!("({} > {})", left, right),
                CmpOp::GtE => format!("({} >= {})", left, right),
                CmpOp::Is => format!("({} == {})", left, right),
                CmpOp::IsNot => format!("({} ~= {})", left, right),
                CmpOp::In => format!("py.contains({}, {})", right, left),
                CmpOp::NotIn => format!("(not py.contains({}, {}))", right, left),
            };
            parts.push(part);
            left = right;
            left_type = right_type;
        }

        if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            format!("({})", parts.join(" and "))
        }
    }

    fn visit_call(&mut self, node: &ExprCall) -> String {
        let args: Vec<String> = node.args.iter().map(|a| self.expr(a)).collect();
        let has_kwargs = !node.keywords.is_empty();

        // Method calls: obj.method(args)
        if let Expr::Attribute(attr) = node.func.as_ref() {
            let obj = self.expr(&attr.value);
            let method = ident(&attr.attr);

            // Passthrough modules: builtin modules (fs, math) and all import-ed modules
            // Python kwargs become a trailing Lua opts table:
            //   Python: module.method(arg1, arg2, opt="val")
            //     Lua:  module.method(arg1, arg2, {opt = "val"})
            if let Expr::Name(name) = attr.value.as_ref() {
                let var_name = ident(&name.id);
                let module_name = self
                    .import_aliases
                    .get(var_name)
                    .map(|s| s.as_str())
                    .unwrap_or(var_name);
                if is_passthrough_module(module_name) || self.import_aliases.contains_key(var_name)
                {
                    if has_kwargs {
                        let kwargs_table = self.build_kwargs_only_table(&node.keywords);
                        let mut all_args = args.clone();
                        all_args.push(kwargs_table);
                        return format!("{}.{}({})", obj, method, all_args.join(", "));
                    }
                    return format!("{}.{}({})", obj, method, args.join(", "));
                }
            }

            // Chained attribute access on passthrough modules: np.linalg.det(a), np.random.seed(42)
            // attr.value is Attribute(Name("np"), "linalg"), method is "det"
            if let Expr::Attribute(inner_attr) = attr.value.as_ref() {
                if let Expr::Name(name) = inner_attr.value.as_ref() {
                    let var_name = ident(&name.id);
                    let module_name = self
                        .import_aliases
                        .get(var_name)
                        .map(|s| s.as_str())
                        .unwrap_or(var_name);
                    if is_passthrough_module(module_name)
                        || self.import_aliases.contains_key(var_name)
                    {
                        if has_kwargs {
                            let kwargs_table = self.build_kwargs_only_table(&node.keywords);
                            let mut all_args = args.clone();
                            all_args.push(kwargs_table);
                            return format!("{}.{}({})", obj, method, all_args.join(", "));
                        }
                        return format!("{}.{}({})", obj, method, args.join(", "));
                    }
                }
            }

            if let Some(fn_name) = direct_method_map(method) {
                let mut all_args = vec![obj];
                all_args.extend(args);
                // For sort(), map key= and reverse= kwargs to positional args:
                // py.sort(list, key, reverse)
                if method == "sort" && has_kwargs {
                    let (key, reverse) = self.extract_sort_kwargs(&node.keywords);
                    // Ensure list is first arg, then key, then reverse
                    if all_args.len() == 1 {
                        all_args.push(key);
                        all_args.push(reverse);
                    }
                }
                return format!("{}({})", fn_name, all_args.join(", "));
            }

            let mut all_args = vec![obj, format!("\"{}\"", method)];
            all_args.extend(args);
            return format!("py.method_call({})", all_args.join(", "));
        }

        // Builtins
        if let Expr::Name(name) = node.func.as_ref() {
            let builtin_name = ident(&name.id);
            if let Some(fn_name) = builtin_map(builtin_name) {
                // For isinstance(), pass the type name as a string literal
                // (not the py.* function reference)
                if builtin_name == "isinstance" && node.args.len() == 2 {
                    let val = &args[0];
                    if let Expr::Name(type_name_node) = &node.args[1] {
                        let raw_name = ident(&type_name_node.id);
                        return format!("py.isinstance({}, \"{}\")", val, raw_name);
                    }
                    return format!("py.isinstance({}, {})", val, args[1]);
                }
                // For sorted(), map key= and reverse= kwargs to positional args:
                // py.sorted(iterable, key, reverse)
                if builtin_name == "sorted" && has_kwargs {
                    let (key, reverse) = self.extract_sort_kwargs(&node.keywords);
                    let mut all = args.clone();
                    if all.len() == 1 {
                        all.push(key);
                        all.push(reverse);
                    }
                    return format!("{}({})", fn_name, all.join(", "));
                }
                // For print() and str(), wrap Float-typed args in py.F() so
                // the runtime formats them as Python floats (e.g. "3.0" not "3").
                if builtin_name == "print" || builtin_name == "str" {
                    let wrapped: Vec<String> = node
                        .args
                        .iter()
                        .zip(args.iter())
                        .map(|(ast_arg, lua_arg)| {
                            if self.expr_type(ast_arg) == ExprType::Float {
                                format!("py.F({})", lua_arg)
                            } else {
                                lua_arg.clone()
                            }
                        })
                        .collect();
                    return format!("{}({})", fn_name, wrapped.join(", "));
                }
                return format!("{}({})", fn_name, args.join(", "));
            }
        }

        let func = self.expr(&node.func);
        format!("{}({})", func, args.join(", "))
    }

    /// Build a Lua table containing ONLY keyword arguments (no positional args).
    /// Used for passthrough modules where kwargs map to a trailing opts table:
    ///   Python: module.method(arg1, arg2, opt="val")
    ///     Lua:  module.method(arg1, arg2, {opt = "val"})
    fn build_kwargs_only_table(&mut self, keywords: &[Keyword]) -> String {
        let mut entries = Vec::new();
        for kw in keywords {
            if let Some(ref name) = kw.arg {
                let val = self.expr(&kw.value);
                entries.push(format!("{} = {}", ident(name), val));
            } else {
                self.warn(kw.range, "**kwargs expansion not supported");
            }
        }
        format!("{{{}}}", entries.join(", "))
    }

    /// Extract `key=` and `reverse=` kwargs for sort/sorted, returning
    /// (key_expr, reverse_expr) as Lua strings ("nil" when absent).
    fn extract_sort_kwargs(&mut self, keywords: &[Keyword]) -> (String, String) {
        let mut key = "nil".to_string();
        let mut reverse = "nil".to_string();
        for kw in keywords {
            if let Some(ref name) = kw.arg {
                match ident(name) {
                    "key" => key = self.expr(&kw.value),
                    "reverse" => reverse = self.expr(&kw.value),
                    _ => {}
                }
            }
        }
        (key, reverse)
    }

    fn visit_subscript(&mut self, node: &ExprSubscript) -> String {
        let obj = self.expr(&node.value);

        if let Expr::Slice(slice) = node.slice.as_ref() {
            let lower = slice
                .lower
                .as_ref()
                .map(|e| self.expr(e))
                .unwrap_or_else(|| "nil".to_string());
            let upper = slice
                .upper
                .as_ref()
                .map(|e| self.expr(e))
                .unwrap_or_else(|| "nil".to_string());
            let step = slice
                .step
                .as_ref()
                .map(|e| self.expr(e))
                .unwrap_or_else(|| "nil".to_string());
            return format!("py.slice({}, {}, {}, {})", obj, lower, upper, step);
        }

        let key = self.expr(&node.slice);
        format!("py.index({}, {})", obj, key)
    }

    fn visit_attribute(&mut self, node: &ExprAttribute) -> String {
        let obj = self.expr(&node.value);
        format!("{}.{}", obj, ident(&node.attr))
    }

    fn visit_list(&mut self, node: &ExprList) -> String {
        let elems: Vec<String> = node.elts.iter().map(|e| self.expr(e)).collect();
        format!("py.list({{{}}})", elems.join(", "))
    }

    fn visit_dict(&mut self, node: &ExprDict) -> String {
        // Emit as py.dict_from({key1, val1, key2, val2, ...}) to preserve insertion order.
        let mut kvs = Vec::new();
        for (key, value) in node.keys.iter().zip(node.values.iter()) {
            let val = self.expr(value);
            if let Some(k) = key {
                let key_str = self.expr(k);
                kvs.push(key_str);
                kvs.push(val);
            } else {
                self.warn(node.range, "dict unpacking (**) not supported");
            }
        }
        if kvs.is_empty() {
            "py.dict({})".to_string()
        } else {
            format!("py.dict_from({{{}}})", kvs.join(", "))
        }
    }

    fn visit_tuple(&mut self, node: &ExprTuple) -> String {
        let elems: Vec<String> = node.elts.iter().map(|e| self.expr(e)).collect();
        format!("py.tuple({{{}}})", elems.join(", "))
    }

    fn visit_joinedstr(&mut self, node: &ExprJoinedStr) -> String {
        let mut fmt_parts = Vec::new();
        let mut args = Vec::new();

        for val in &node.values {
            match val {
                Expr::Constant(c) => {
                    if let Constant::Str(s) = &c.value {
                        let escaped = s
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r")
                            .replace('\t', "\\t")
                            .replace('\0', "\\0")
                            .replace('%', "%%");
                        fmt_parts.push(escaped);
                    }
                }
                Expr::FormattedValue(fv) => {
                    let expr_str = self.expr(&fv.value);
                    let is_float = self.expr_type(&fv.value) == ExprType::Float;
                    if let Some(spec) = &fv.format_spec {
                        let spec_str = self.extract_format_spec(spec);
                        fmt_parts.push(format!("%{}", spec_str));
                        args.push(expr_str);
                    } else {
                        fmt_parts.push("%s".to_string());
                        if is_float {
                            args.push(format!("py.str(py.F({}))", expr_str));
                        } else {
                            args.push(format!("py.str({})", expr_str));
                        }
                    }
                }
                _ => {
                    let s = self.expr(val);
                    let is_float = self.expr_type(val) == ExprType::Float;
                    fmt_parts.push("%s".to_string());
                    if is_float {
                        args.push(format!("py.str(py.F({}))", s));
                    } else {
                        args.push(format!("py.str({})", s));
                    }
                }
            }
        }

        let fmt = fmt_parts.join("");
        if args.is_empty() {
            format!("\"{}\"", fmt)
        } else {
            format!("string.format(\"{}\", {})", fmt, args.join(", "))
        }
    }

    fn extract_format_spec(&mut self, spec: &Expr) -> String {
        if let Expr::JoinedStr(js) = spec {
            let mut parts = Vec::new();
            for v in &js.values {
                if let Expr::Constant(c) = v {
                    if let Constant::Str(s) = &c.value {
                        parts.push(s.clone());
                    }
                }
            }
            parts.join("")
        } else if let Expr::Constant(c) = spec {
            if let Constant::Str(s) = &c.value {
                s.clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    fn visit_ifexp(&mut self, node: &ExprIfExp) -> String {
        let cond = self.emit_bool_expr(&node.test);
        let body = self.expr(&node.body);
        let orelse = self.expr(&node.orelse);
        format!(
            "({} and (function() return {} end)() or {})",
            cond, body, orelse
        )
    }

    fn visit_lambda(&mut self, node: &ExprLambda) -> String {
        let params: Vec<&str> = node.args.args.iter().map(|a| ident(&a.def.arg)).collect();
        let body = self.expr(&node.body);
        format!("function({}) return {} end", params.join(", "), body)
    }

    fn visit_listcomp(&mut self, node: &ExprListComp) -> String {
        let tmp = self.temp_var();
        let elt = self.expr(&node.elt);
        let mut parts = vec![format!("(function() local {} = py.list({{}}); ", tmp)];
        self.emit_comp_generators(
            &mut parts,
            &node.generators,
            &format!("py.append({}, {})", tmp, elt),
        );
        parts.push(format!(" return {} end)()", tmp));
        parts.join("")
    }

    fn visit_dictcomp(&mut self, node: &ExprDictComp) -> String {
        let tmp = self.temp_var();
        let key = self.expr(&node.key);
        let val = self.expr(&node.value);
        let mut parts = vec![format!("(function() local {} = py.dict({{}}); ", tmp)];
        self.emit_comp_generators(
            &mut parts,
            &node.generators,
            &format!("py.setindex({}, {}, {})", tmp, key, val),
        );
        parts.push(format!(" return {} end)()", tmp));
        parts.join("")
    }

    fn emit_comp_generators(
        &mut self,
        parts: &mut Vec<String>,
        generators: &[Comprehension],
        body: &str,
    ) {
        let gen = &generators[0];
        let target = self.target_str(&gen.target);

        // Check for range() call
        if let Expr::Call(call) = &gen.iter {
            if let Expr::Name(name) = call.func.as_ref() {
                if ident(&name.id) == "range" {
                    let args: Vec<String> = call.args.iter().map(|a| self.expr(a)).collect();
                    parts.push(format!(
                        "for {} in py.range({}) do ",
                        target,
                        args.join(", ")
                    ));
                    for if_clause in &gen.ifs {
                        let cond = self.emit_bool_expr(if_clause);
                        parts.push(format!("if {} then ", cond));
                    }
                    if generators.len() > 1 {
                        self.emit_comp_generators(parts, &generators[1..], body);
                    } else {
                        parts.push(format!("{}; ", body));
                    }
                    for _ in &gen.ifs {
                        parts.push("end ".to_string());
                    }
                    parts.push("end; ".to_string());
                    return;
                }
            }
        }

        let iter_expr = self.expr(&gen.iter);
        parts.push(format!("for {} in py.iter({}) do ", target, iter_expr));
        for if_clause in &gen.ifs {
            let cond = self.emit_bool_expr(if_clause);
            parts.push(format!("if {} then ", cond));
        }
        if generators.len() > 1 {
            self.emit_comp_generators(parts, &generators[1..], body);
        } else {
            parts.push(format!("{}; ", body));
        }
        for _ in &gen.ifs {
            parts.push("end ".to_string());
        }
        parts.push("end; ".to_string());
    }

    fn target_str(&mut self, target: &Expr) -> String {
        match target {
            Expr::Name(n) => ident(&n.id).to_string(),
            Expr::Tuple(t) => {
                let parts: Vec<String> = t.elts.iter().map(|e| self.target_str(e)).collect();
                parts.join(", ")
            }
            Expr::List(l) => {
                let parts: Vec<String> = l.elts.iter().map(|e| self.target_str(e)).collect();
                parts.join(", ")
            }
            _ => "_".to_string(),
        }
    }

    // ── Statements ──────────────────────────────────────────────

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(s) => self.visit_assign(s),
            Stmt::AugAssign(s) => self.visit_augassign(s),
            Stmt::FunctionDef(s) => self.visit_functiondef(s),
            Stmt::AsyncFunctionDef(s) => {
                self.warn(s.range, "async functions not supported, treating as sync");
                self.visit_async_functiondef(s);
            }
            Stmt::Return(s) => self.visit_return(s),
            Stmt::If(s) => self.visit_if(s),
            Stmt::While(s) => self.visit_while(s),
            Stmt::For(s) => self.visit_for(s),
            Stmt::Break(s) => {
                if let Some(flag) = self.break_flag_stack.last() {
                    self.emit(&format!("{} = true", flag), s.range);
                }
                self.emit("break", s.range);
            }
            Stmt::Continue(s) => self.emit("continue", s.range),
            Stmt::Import(s) => self.visit_import(s),
            Stmt::ImportFrom(s) => self.visit_importfrom(s),
            Stmt::Try(s) => self.visit_try(s),
            Stmt::TryStar(s) => {
                self.warn(s.range, "try* (exception groups) not supported");
            }
            Stmt::Raise(s) => self.visit_raise(s),
            Stmt::Expr(s) => {
                let val = self.expr(&s.value);
                // Prefix with `;` when expression starts with `(` to avoid
                // Luau's "ambiguous syntax" error (e.g. IIFE from list comps).
                if val.starts_with('(') {
                    self.emit(&format!(";{}", val), s.range);
                } else {
                    self.emit(&val, s.range);
                }
            }
            Stmt::Pass(s) => self.emit("-- pass", s.range),
            Stmt::Delete(s) => self.visit_delete(s),
            Stmt::Assert(s) => self.visit_assert(s),
            Stmt::Global(s) => {
                self.warn(s.range, "global statement not supported");
            }
            Stmt::Nonlocal(s) => {
                self.warn(s.range, "nonlocal statement not supported");
            }
            Stmt::ClassDef(s) => {
                self.warn(s.range, "classes not supported");
            }
            Stmt::With(s) => {
                self.warn(s.range, "with statement not supported, executing body only");
                for child in &s.body {
                    self.visit_stmt(child);
                }
            }
            Stmt::Match(s) => {
                self.warn(s.range, "match statement not supported");
            }
            _ => {}
        }
    }

    fn visit_assign(&mut self, node: &StmtAssign) {
        for target in &node.targets {
            self.emit_assignment(target, &node.value, node.range);
        }
    }

    fn emit_assignment(&mut self, target: &Expr, value: &Expr, range: TextRange) {
        match target {
            Expr::Name(name) => {
                let ty = self.expr_type(value);
                let val = self.expr(value);
                let n = ident(&name.id);
                if !self.scopes.is_declared(n) {
                    self.emit(&format!("local {} = {}", n, val), range);
                    self.scopes.declare(n);
                    self.scopes.set_type(n, ty);
                } else {
                    self.emit(&format!("{} = {}", n, val), range);
                    // Use set_type for reassignment — the variable gets a new
                    // definite type (e.g. x was Int, now x = 10/3 makes it Float).
                    self.scopes.set_type(n, ty);
                }
            }
            Expr::Tuple(t) => {
                self.emit_tuple_assignment(&t.elts, value, range);
            }
            Expr::List(l) => {
                self.emit_tuple_assignment(&l.elts, value, range);
            }
            Expr::Subscript(sub) => {
                let obj = self.expr(&sub.value);
                let key = self.expr(&sub.slice);
                let val = self.expr(value);
                self.emit(&format!("py.setindex({}, {}, {})", obj, key, val), range);
            }
            Expr::Attribute(attr) => {
                let obj = self.expr(&attr.value);
                let val = self.expr(value);
                self.emit(&format!("{}.{} = {}", obj, ident(&attr.attr), val), range);
            }
            _ => {
                self.warn(range, "unsupported assignment target");
            }
        }
    }

    fn has_complex_target(targets: &[Expr]) -> bool {
        targets.iter().any(|t| {
            matches!(
                t,
                Expr::Subscript(_) | Expr::Attribute(_) | Expr::Tuple(_) | Expr::List(_)
            )
        })
    }

    /// Emit an individual assignment to a single target from a value expression string.
    fn emit_single_target_assign(&mut self, target: &Expr, val_str: &str, range: TextRange) {
        match target {
            Expr::Name(name) => {
                let n = ident(&name.id);
                if !self.scopes.is_declared(n) {
                    self.emit(&format!("local {} = {}", n, val_str), range);
                    self.scopes.declare(n);
                } else {
                    self.emit(&format!("{} = {}", n, val_str), range);
                }
            }
            Expr::Subscript(sub) => {
                let obj = self.expr(&sub.value);
                let key = self.expr(&sub.slice);
                self.emit(
                    &format!("py.setindex({}, {}, {})", obj, key, val_str),
                    range,
                );
            }
            Expr::Attribute(attr) => {
                let obj = self.expr(&attr.value);
                self.emit(
                    &format!("{}.{} = {}", obj, ident(&attr.attr), val_str),
                    range,
                );
            }
            Expr::Tuple(_) | Expr::List(_) => {
                // Nested tuple/list unpacking: unpack val_str via table.unpack, then recursively assign
                let elts: &[Expr] = match target {
                    Expr::Tuple(t) => &t.elts,
                    Expr::List(l) => &l.elts,
                    _ => unreachable!(),
                };
                let tmps: Vec<String> = (0..elts.len()).map(|_| self.temp_var()).collect();
                self.emit(
                    &format!(
                        "local {} = table.unpack({}.data or {{{}}})",
                        tmps.join(", "),
                        val_str,
                        val_str
                    ),
                    range,
                );
                for (sub_target, tmp) in elts.iter().zip(tmps.iter()) {
                    self.emit_single_target_assign(sub_target, tmp, range);
                }
            }
            _ => {
                self.warn(range, "unsupported tuple unpacking target");
            }
        }
    }

    fn emit_tuple_assignment(&mut self, targets: &[Expr], value: &Expr, range: TextRange) {
        let has_complex = Self::has_complex_target(targets);

        // When RHS is a tuple literal with matching length
        if let Expr::Tuple(rhs_tuple) = value {
            if rhs_tuple.elts.len() == targets.len() {
                let vals: Vec<String> = rhs_tuple.elts.iter().map(|e| self.expr(e)).collect();

                if has_complex {
                    // Complex targets (subscripts/attributes): use temp vars then assign individually
                    let tmps: Vec<String> = (0..targets.len()).map(|_| self.temp_var()).collect();
                    self.emit(
                        &format!("local {} = {}", tmps.join(", "), vals.join(", ")),
                        range,
                    );
                    for (target, tmp) in targets.iter().zip(tmps.iter()) {
                        self.emit_single_target_assign(target, tmp, range);
                    }
                } else {
                    // All simple name targets: use direct multi-assignment
                    let names: Vec<String> = targets.iter().map(|e| self.target_str(e)).collect();
                    let new_names: Vec<&str> = names
                        .iter()
                        .filter(|n| !self.scopes.is_declared(n))
                        .map(|s| s.as_str())
                        .collect();

                    if new_names.len() == names.len() {
                        self.emit(
                            &format!("local {} = {}", names.join(", "), vals.join(", ")),
                            range,
                        );
                    } else {
                        for n in &new_names {
                            self.emit(&format!("local {}", n), range);
                            self.scopes.declare(n);
                        }
                        self.emit(
                            &format!("{} = {}", names.join(", "), vals.join(", ")),
                            range,
                        );
                    }

                    for n in &names {
                        self.scopes.declare(n);
                    }
                }
                return;
            }
        }

        // When RHS is a function call, use Lua multi-return directly
        // (avoids the table.unpack path which double-evaluates the call)
        if matches!(value, Expr::Call(_)) {
            let val = self.expr(value);

            if has_complex {
                let tmps: Vec<String> = (0..targets.len()).map(|_| self.temp_var()).collect();
                self.emit(&format!("local {} = {}", tmps.join(", "), val), range);
                for (target, tmp) in targets.iter().zip(tmps.iter()) {
                    self.emit_single_target_assign(target, tmp, range);
                }
            } else {
                let names: Vec<String> = targets.iter().map(|e| self.target_str(e)).collect();
                let new_names: Vec<&str> = names
                    .iter()
                    .filter(|n| !self.scopes.is_declared(n))
                    .map(|s| s.as_str())
                    .collect();

                if new_names.len() == names.len() {
                    self.emit(&format!("local {} = {}", names.join(", "), val), range);
                } else {
                    for n in &new_names {
                        self.emit(&format!("local {}", n), range);
                        self.scopes.declare(n);
                    }
                    self.emit(&format!("{} = {}", names.join(", "), val), range);
                }

                for n in &names {
                    self.scopes.declare(n);
                }
            }
            return;
        }

        // Fallback: use table.unpack for non-tuple RHS (variables holding tuples, etc.)
        let val = self.expr(value);

        if has_complex {
            // Complex targets: unpack into temps, then assign individually
            let tmps: Vec<String> = (0..targets.len()).map(|_| self.temp_var()).collect();
            self.emit(
                &format!(
                    "local {} = table.unpack({}.data or {{{}}})",
                    tmps.join(", "),
                    val,
                    val
                ),
                range,
            );
            for (target, tmp) in targets.iter().zip(tmps.iter()) {
                self.emit_single_target_assign(target, tmp, range);
            }
        } else {
            let names: Vec<String> = targets.iter().map(|e| self.target_str(e)).collect();
            let new_names: Vec<&str> = names
                .iter()
                .filter(|n| !self.scopes.is_declared(n))
                .map(|s| s.as_str())
                .collect();

            if new_names.len() == names.len() {
                self.emit(
                    &format!(
                        "local {} = table.unpack({}.data or {{{}}})",
                        names.join(", "),
                        val,
                        val
                    ),
                    range,
                );
            } else {
                for n in &new_names {
                    self.emit(&format!("local {}", n), range);
                    self.scopes.declare(n);
                }
                self.emit(
                    &format!(
                        "{} = table.unpack({}.data or {{{}}})",
                        names.join(", "),
                        val,
                        val
                    ),
                    range,
                );
            }

            for n in &names {
                self.scopes.declare(n);
            }
        }
    }

    fn visit_augassign(&mut self, node: &StmtAugAssign) {
        let target_type = self.expr_type(&node.target);
        let val_type = self.expr_type(&node.value);
        let target = self.expr(&node.target);
        let val = self.expr(&node.value);
        let both_num = target_type.is_num() && val_type.is_num();
        let either_float = target_type == ExprType::Float || val_type == ExprType::Float;

        // Compute result type for type tracking (matches Python promotion rules)
        let result_type = match node.op {
            Operator::Add => {
                if both_num {
                    if either_float {
                        ExprType::Float
                    } else {
                        ExprType::Int
                    }
                } else if target_type == ExprType::Str && val_type == ExprType::Str {
                    ExprType::Str
                } else {
                    ExprType::Unknown
                }
            }
            // / always produces float
            Operator::Div => {
                if both_num {
                    ExprType::Float
                } else {
                    ExprType::Unknown
                }
            }
            // // preserves int-ness when both are int
            Operator::FloorDiv => {
                if both_num {
                    if either_float {
                        ExprType::Float
                    } else {
                        ExprType::Int
                    }
                } else {
                    ExprType::Unknown
                }
            }
            Operator::Sub | Operator::Mult | Operator::Mod | Operator::Pow => {
                if both_num {
                    if either_float {
                        ExprType::Float
                    } else {
                        ExprType::Int
                    }
                } else {
                    ExprType::Unknown
                }
            }
            Operator::BitAnd
            | Operator::BitOr
            | Operator::BitXor
            | Operator::LShift
            | Operator::RShift => {
                if both_num {
                    ExprType::Int
                } else {
                    ExprType::Unknown
                }
            }
            _ => ExprType::Unknown,
        };

        let code = match node.op {
            Operator::Add => {
                if both_num {
                    format!("{} += {}", target, val)
                } else {
                    format!("{} = py.add({}, {})", target, target, val)
                }
            }
            Operator::Sub => format!("{} -= {}", target, val),
            Operator::Mult => {
                if both_num {
                    format!("{} *= {}", target, val)
                } else {
                    format!("{} = py.mul({}, {})", target, target, val)
                }
            }
            Operator::Div => format!("{} = py.truediv({}, {})", target, target, val),
            Operator::Pow => {
                if both_num {
                    format!("{} = {} ^ {}", target, target, val)
                } else {
                    format!("{} = py.pow({}, {})", target, target, val)
                }
            }
            Operator::FloorDiv => format!("{} = py.floordiv({}, {})", target, target, val),
            Operator::Mod => format!("{} = py.mod({}, {})", target, target, val),
            Operator::BitAnd => format!("{} = bit32.band({}, {})", target, target, val),
            Operator::BitOr => format!("{} = bit32.bor({}, {})", target, target, val),
            Operator::BitXor => format!("{} = bit32.bxor({}, {})", target, target, val),
            Operator::LShift => format!("{} = bit32.lshift({}, {})", target, target, val),
            Operator::RShift => format!("{} = bit32.rshift({}, {})", target, target, val),
            _ => {
                self.warn(node.range, "unsupported augmented assign op");
                format!("{} = {} --[[unsupported augop]] {}", target, target, val)
            }
        };
        self.emit(&code, node.range);

        // Update type tracking for the target variable.
        // Use set_type (not narrow_type) because augmented assignment is a
        // definite reassignment — e.g. `x /= 2` changes x from Int to Float.
        if let Expr::Name(name) = node.target.as_ref() {
            self.scopes.set_type(ident(&name.id), result_type);
        }
    }

    fn visit_functiondef(&mut self, node: &StmtFunctionDef) {
        let name = ident(&node.name);
        let params: Vec<&str> = node.args.args.iter().map(|a| ident(&a.def.arg)).collect();
        let has_vararg = node.args.vararg.is_some();

        // Build parameter list, appending ... for *args
        let param_str = if has_vararg {
            if params.is_empty() {
                "...".to_string()
            } else {
                format!("{}, ...", params.join(", "))
            }
        } else {
            params.join(", ")
        };

        if !self.scopes.is_declared(name) {
            self.emit(
                &format!("local function {}({})", name, param_str),
                node.range,
            );
            self.scopes.declare(name);
        } else {
            self.emit(&format!("function {}({})", name, param_str), node.range);
        }

        self.indent += 1;
        self.scopes.enter();
        for p in &params {
            self.scopes.declare(p);
        }

        // *args → local args = py.tuple({...})
        if let Some(vararg) = &node.args.vararg {
            let vararg_name = ident(&vararg.arg);
            self.scopes.declare(vararg_name);
            self.emit(
                &format!("local {} = py.tuple({{...}})", vararg_name),
                node.range,
            );
        }

        // Default arguments
        let defaults: Vec<&Expr> = node.args.defaults().collect();
        if !defaults.is_empty() {
            let offset = params.len() - defaults.len();
            for (i, default) in defaults.iter().enumerate() {
                let param = params[offset + i];
                let default_val = self.expr(default);
                self.emit(
                    &format!("if {} == nil then {} = {} end", param, param, default_val),
                    node.range,
                );
            }
        }

        for child in &node.body {
            self.visit_stmt(child);
        }

        self.scopes.exit();
        self.indent -= 1;
        self.emit("end", node.range);
    }

    fn visit_async_functiondef(&mut self, node: &StmtAsyncFunctionDef) {
        let name = ident(&node.name);
        let params: Vec<&str> = node.args.args.iter().map(|a| ident(&a.def.arg)).collect();

        if !self.scopes.is_declared(name) {
            self.emit(
                &format!("local function {}({})", name, params.join(", ")),
                node.range,
            );
            self.scopes.declare(name);
        } else {
            self.emit(
                &format!("function {}({})", name, params.join(", ")),
                node.range,
            );
        }

        self.indent += 1;
        self.scopes.enter();
        for p in &params {
            self.scopes.declare(p);
        }
        for child in &node.body {
            self.visit_stmt(child);
        }
        self.scopes.exit();
        self.indent -= 1;
        self.emit("end", node.range);
    }

    fn visit_return(&mut self, node: &StmtReturn) {
        match &node.value {
            None => self.emit("return", node.range),
            Some(val) => {
                if let Expr::Tuple(t) = val.as_ref() {
                    let vals: Vec<String> = t.elts.iter().map(|e| self.expr(e)).collect();
                    self.emit(&format!("return {}", vals.join(", ")), node.range);
                } else {
                    let v = self.expr(val);
                    self.emit(&format!("return {}", v), node.range);
                }
            }
        }
    }

    fn visit_if(&mut self, node: &StmtIf) {
        let test = self.emit_bool_expr(&node.test);
        self.emit(&format!("if {} then", test), node.range);
        self.indent += 1;
        for child in &node.body {
            self.visit_stmt(child);
        }
        self.indent -= 1;
        self.emit_elif_or_else(&node.orelse, node.range);
        self.emit("end", node.range);
    }

    fn emit_elif_or_else(&mut self, orelse: &[Stmt], parent_range: TextRange) {
        if orelse.is_empty() {
            return;
        }

        if orelse.len() == 1 {
            if let Stmt::If(elif) = &orelse[0] {
                let test = self.emit_bool_expr(&elif.test);
                self.emit(&format!("elseif {} then", test), elif.range);
                self.indent += 1;
                for child in &elif.body {
                    self.visit_stmt(child);
                }
                self.indent -= 1;
                self.emit_elif_or_else(&elif.orelse, elif.range);
                return;
            }
        }

        self.emit("else", parent_range);
        self.indent += 1;
        for child in orelse {
            self.visit_stmt(child);
        }
        self.indent -= 1;
    }

    fn visit_while(&mut self, node: &StmtWhile) {
        let has_else = !node.orelse.is_empty();
        let flag = if has_else {
            let f = self.temp_var();
            self.emit(&format!("local {} = false", f), node.range);
            self.break_flag_stack.push(f.clone());
            Some(f)
        } else {
            None
        };

        let test = self.emit_bool_expr(&node.test);
        self.emit(&format!("while {} do", test), node.range);
        self.indent += 1;
        for child in &node.body {
            self.visit_stmt(child);
        }
        self.indent -= 1;
        self.emit("end", node.range);

        if let Some(f) = flag {
            self.break_flag_stack.pop();
            self.emit(&format!("if not {} then", f), node.range);
            self.indent += 1;
            for child in &node.orelse {
                self.visit_stmt(child);
            }
            self.indent -= 1;
            self.emit("end", node.range);
        }
    }

    /// Infer the type(s) of for-loop variables from the iterator expression.
    fn infer_for_target_types(&self, target: &Expr, iter_expr: &Expr) -> Vec<(String, ExprType)> {
        let mut result = Vec::new();

        if let Expr::Call(call) = iter_expr {
            if let Expr::Name(name) = call.func.as_ref() {
                match ident(&name.id) {
                    "range" => {
                        // for i in range(...) → i is Int
                        if let Expr::Name(n) = target {
                            result.push((ident(&n.id).to_string(), ExprType::Int));
                        }
                        return result;
                    }
                    "enumerate" => {
                        // for i, v in enumerate(...) → i is Int, v is Unknown
                        if let Expr::Tuple(t) = target {
                            for (idx, elt) in t.elts.iter().enumerate() {
                                if let Expr::Name(n) = elt {
                                    let ty = if idx == 0 {
                                        ExprType::Int
                                    } else {
                                        ExprType::Unknown
                                    };
                                    result.push((ident(&n.id).to_string(), ty));
                                }
                            }
                        }
                        return result;
                    }
                    _ => {}
                }
            }
        }

        // for ch in "string" → ch is Str
        if let Expr::Name(n) = target {
            let iter_type = self.expr_type(iter_expr);
            if iter_type == ExprType::Str {
                result.push((ident(&n.id).to_string(), ExprType::Str));
                return result;
            }
        }

        result
    }

    fn visit_for(&mut self, node: &StmtFor) {
        let target = self.target_str(&node.target);
        let target_types = self.infer_for_target_types(&node.target, &node.iter);
        let has_else = !node.orelse.is_empty();

        let flag = if has_else {
            let f = self.temp_var();
            self.emit(&format!("local {} = false", f), node.range);
            self.break_flag_stack.push(f.clone());
            Some(f)
        } else {
            None
        };

        if let Expr::Call(call) = node.iter.as_ref() {
            if let Expr::Name(name) = call.func.as_ref() {
                let fn_name = ident(&name.id);
                if matches!(fn_name, "range" | "enumerate" | "zip" | "reversed") {
                    let args: Vec<String> = call.args.iter().map(|a| self.expr(a)).collect();
                    self.emit(
                        &format!("for {} in py.{}({}) do", target, fn_name, args.join(", ")),
                        node.range,
                    );
                    self.indent += 1;
                    self.scopes.enter();
                    self.declare_targets(&node.target);
                    for (name, ty) in &target_types {
                        self.scopes.set_type(name, *ty);
                    }
                    for child in &node.body {
                        self.visit_stmt(child);
                    }
                    self.scopes.exit();
                    self.indent -= 1;
                    self.emit("end", node.range);

                    if let Some(f) = flag {
                        self.break_flag_stack.pop();
                        self.emit(&format!("if not {} then", f), node.range);
                        self.indent += 1;
                        for child in &node.orelse {
                            self.visit_stmt(child);
                        }
                        self.indent -= 1;
                        self.emit("end", node.range);
                    }
                    return;
                }
            }
        }

        let iter_expr = self.expr(&node.iter);
        self.emit(
            &format!("for {} in py.iter({}) do", target, iter_expr),
            node.range,
        );
        self.indent += 1;
        self.scopes.enter();
        self.declare_targets(&node.target);
        for (name, ty) in &target_types {
            self.scopes.set_type(name, *ty);
        }
        for child in &node.body {
            self.visit_stmt(child);
        }
        self.scopes.exit();
        self.indent -= 1;
        self.emit("end", node.range);

        if let Some(f) = flag {
            self.break_flag_stack.pop();
            self.emit(&format!("if not {} then", f), node.range);
            self.indent += 1;
            for child in &node.orelse {
                self.visit_stmt(child);
            }
            self.indent -= 1;
            self.emit("end", node.range);
        }
    }

    fn declare_targets(&mut self, target: &Expr) {
        match target {
            Expr::Name(n) => self.scopes.declare(ident(&n.id)),
            Expr::Tuple(t) => {
                for e in &t.elts {
                    self.declare_targets(e);
                }
            }
            Expr::List(l) => {
                for e in &l.elts {
                    self.declare_targets(e);
                }
            }
            _ => {}
        }
    }

    fn visit_import(&mut self, node: &StmtImport) {
        for alias in &node.names {
            let module = ident(&alias.name).to_string();
            let name = alias
                .asname
                .as_ref()
                .map(|a| ident(a).to_string())
                .unwrap_or_else(|| module.replace('.', "_"));
            // Track all imported names → module for passthrough resolution
            self.import_aliases.insert(name.clone(), module.clone());
            // Map known Python modules to Luau sandbox globals
            let rhs = python_module_to_luau(&module);
            if !self.scopes.is_declared(&name) {
                self.emit(&format!("local {} = {}", name, rhs), node.range);
                self.scopes.declare(&name);
            } else {
                self.emit(&format!("{} = {}", name, rhs), node.range);
            }
        }
    }

    fn visit_importfrom(&mut self, node: &StmtImportFrom) {
        let module = node
            .module
            .as_ref()
            .map(|m| ident(m).to_string())
            .unwrap_or_default();
        for alias in &node.names {
            let attr = ident(&alias.name);
            let name = alias
                .asname
                .as_ref()
                .map(|a| ident(a).to_string())
                .unwrap_or_else(|| attr.to_string());
            // Map known Python from-imports to Luau sandbox globals
            let rhs = python_from_import_to_luau(&module, attr);
            // Track from-import aliases for passthrough resolution (e.g., `from rapidfuzz import fuzz` → fuzz maps to fuzzy)
            self.import_aliases.insert(name.clone(), rhs.clone());
            if !self.scopes.is_declared(&name) {
                self.emit(&format!("local {} = {}", name, rhs), node.range);
                self.scopes.declare(&name);
            } else {
                self.emit(&format!("{} = {}", name, rhs), node.range);
            }
        }
    }

    /// Collect variable names directly assigned in a statement body.
    /// Used to pre-declare variables in try bodies for scoping.
    fn collect_assigned_names(body: &[Stmt]) -> Vec<String> {
        let mut names = Vec::new();
        for stmt in body {
            match stmt {
                Stmt::Assign(a) => {
                    for target in &a.targets {
                        if let Expr::Name(n) = target {
                            names.push(ident(&n.id).to_string());
                        }
                    }
                }
                Stmt::AugAssign(a) => {
                    if let Expr::Name(n) = a.target.as_ref() {
                        names.push(ident(&n.id).to_string());
                    }
                }
                _ => {}
            }
        }
        names
    }

    fn visit_try(&mut self, node: &StmtTry) {
        // Pre-declare variables assigned in the try body so they're accessible
        // in else/finally clauses (the try body is wrapped in an xpcall closure,
        // so `local` inside it wouldn't be visible outside).
        if !node.orelse.is_empty() {
            let new_vars = Self::collect_assigned_names(&node.body);
            for name in &new_vars {
                if !self.scopes.is_declared(name) {
                    self.emit(&format!("local {}", name), node.range);
                    self.scopes.declare(name);
                }
            }
        }

        self.emit("local ok, err = xpcall(function()", node.range);
        self.indent += 1;
        for child in &node.body {
            self.visit_stmt(child);
        }
        self.indent -= 1;
        self.emit("end, function(e) return e end)", node.range);

        if !node.handlers.is_empty() {
            self.emit("if not ok then", node.range);
            self.indent += 1;

            // Unwrap ExceptHandler variants
            let handlers: Vec<&ExceptHandlerExceptHandler> = node
                .handlers
                .iter()
                .map(|h| match h {
                    ExceptHandler::ExceptHandler(eh) => eh,
                })
                .collect();

            if handlers.len() == 1 && handlers[0].type_.is_none() {
                let handler = handlers[0];
                if let Some(name) = &handler.name {
                    self.emit(&format!("local {} = err", ident(name)), handler.range);
                    self.scopes.declare(ident(name));
                }
                for child in &handler.body {
                    self.visit_stmt(child);
                }
            } else {
                for (i, handler) in handlers.iter().enumerate() {
                    if handler.type_.is_none() {
                        if i > 0 {
                            self.emit("else", handler.range);
                            self.indent += 1;
                        }
                        if let Some(name) = &handler.name {
                            self.emit(&format!("local {} = err", ident(name)), handler.range);
                            self.scopes.declare(ident(name));
                        }
                        for child in &handler.body {
                            self.visit_stmt(child);
                        }
                        if i > 0 {
                            self.indent -= 1;
                        }
                    } else {
                        let keyword = if i == 0 { "if" } else { "elseif" };
                        let type_name = if let Some(t) = &handler.type_ {
                            if let Expr::Name(n) = t.as_ref() {
                                format!("\"{}\"", ident(&n.id))
                            } else {
                                self.expr(t)
                            }
                        } else {
                            "nil".to_string()
                        };
                        self.emit(
                            &format!("{} py.isinstance(err, {}) then", keyword, type_name),
                            handler.range,
                        );
                        self.indent += 1;
                        if let Some(name) = &handler.name {
                            self.emit(&format!("local {} = err", ident(name)), handler.range);
                            self.scopes.declare(ident(name));
                        }
                        for child in &handler.body {
                            self.visit_stmt(child);
                        }
                        self.indent -= 1;
                    }
                }
                self.emit("end", node.range);
            }

            self.indent -= 1;

            // else clause: runs when the try body completes without error
            if !node.orelse.is_empty() {
                self.emit("else", node.range);
                self.indent += 1;
                for child in &node.orelse {
                    self.visit_stmt(child);
                }
                self.indent -= 1;
            }

            self.emit("end", node.range);
        }

        for child in &node.finalbody {
            self.visit_stmt(child);
        }
    }

    fn visit_raise(&mut self, node: &StmtRaise) {
        if let Some(exc) = &node.exc {
            let exc_expr = self.expr(exc);
            self.emit(&format!("error({}, 0)", exc_expr), node.range);
        } else {
            self.emit("error(err, 0)", node.range);
        }
    }

    fn visit_delete(&mut self, node: &StmtDelete) {
        for target in &node.targets {
            match target {
                Expr::Subscript(sub) => {
                    let obj = self.expr(&sub.value);
                    let key = self.expr(&sub.slice);
                    self.emit(&format!("py.setindex({}, {}, nil)", obj, key), node.range);
                }
                Expr::Name(n) => {
                    self.emit(&format!("{} = nil", ident(&n.id)), node.range);
                }
                _ => {
                    self.warn(node.range, "unsupported delete target");
                }
            }
        }
    }

    fn visit_assert(&mut self, node: &StmtAssert) {
        let bool_test = self.emit_bool_expr(&node.test);
        if let Some(msg) = &node.msg {
            let msg_str = self.expr(msg);
            self.emit(
                &format!(
                    "if not ({}) then error(\"AssertionError: \" .. py.str({}), 0) end",
                    bool_test, msg_str
                ),
                node.range,
            );
        } else {
            self.emit(
                &format!(
                    "if not ({}) then error(\"AssertionError\", 0) end",
                    bool_test
                ),
                node.range,
            );
        }
    }
}

/// Translate a Luau runtime error back to Python source context.
///
/// Takes an [`ExecError`] (with Luau line already extracted), maps it to
/// the Python source line via the source map, applies Python-style error
/// names, and formats with the Python source line as context.
mod errors;

pub use errors::translate_error;

#[cfg(test)]
mod tests;
