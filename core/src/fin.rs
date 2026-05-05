//! Financial functions module for the Luau sandbox.
//!
//! Exposes `fin.npv`, `fin.irr`, `fin.mirr`, `fin.pmt`, `fin.pv`, `fin.fv`,
//! `fin.nper`, `fin.rate` as globals.
//! Pure Rust implementations — no external deps beyond what's already in the sandbox.

use crate::lua_util::register_help_functions;
use crate::sandbox::{
    validate_args, wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use mlua::{Lua, MultiValue, Value};

pub(crate) static FIN_DOC: ModuleDoc = ModuleDoc {
    name: "fin",
    summary: "Financial functions (NPV, IRR, PMT, etc.)",
    functions: &[
        FnDoc {
            name: "npv",
            description: "Net Present Value. Computes the NPV of a series of cash flows at a given discount rate.",
            params: &[
                Param { name: "rate", short: Some('r'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "cashflows", short: Some('c'), typ: ParamType::Table, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.npv({rate=0.1, cashflows={-1000, 300, 400, 500}})"#),
        },
        FnDoc {
            name: "irr",
            description: "Internal Rate of Return. Finds the discount rate that makes NPV = 0.",
            params: &[
                Param { name: "cashflows", short: Some('c'), typ: ParamType::Table, required: true, fields: None },
                Param { name: "guess", short: Some('g'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.irr({cashflows={-1000, 300, 400, 500}})"#),
        },
        FnDoc {
            name: "mirr",
            description: "Modified Internal Rate of Return. Uses separate finance and reinvestment rates.",
            params: &[
                Param { name: "cashflows", short: Some('c'), typ: ParamType::Table, required: true, fields: None },
                Param { name: "finance_rate", short: Some('f'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "reinvest_rate", short: Some('i'), typ: ParamType::Number, required: true, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.mirr({cashflows={-1000, 300, 400, 500}, finance_rate=0.05, reinvest_rate=0.08})"#),
        },
        FnDoc {
            name: "pmt",
            description: "Payment per period for a loan/annuity. Negative = payment out.",
            params: &[
                Param { name: "rate", short: Some('r'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "nper", short: Some('n'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pv", short: Some('p'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "fv", short: Some('f'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "when", short: Some('w'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.pmt({rate=0.05/12, nper=360, pv=200000})"#),
        },
        FnDoc {
            name: "pv",
            description: "Present Value of an annuity.",
            params: &[
                Param { name: "rate", short: Some('r'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "nper", short: Some('n'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pmt", short: Some('p'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "fv", short: Some('f'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "when", short: Some('w'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.pv({rate=0.05/12, nper=360, pmt=-1073.64})"#),
        },
        FnDoc {
            name: "fv",
            description: "Future Value of an annuity.",
            params: &[
                Param { name: "rate", short: Some('r'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "nper", short: Some('n'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pmt", short: Some('p'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pv", short: Some('v'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "when", short: Some('w'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.fv({rate=0.05/12, nper=360, pmt=-500})"#),
        },
        FnDoc {
            name: "nper",
            description: "Number of periods for a loan/annuity.",
            params: &[
                Param { name: "rate", short: Some('r'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pmt", short: Some('p'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pv", short: Some('v'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "fv", short: Some('f'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "when", short: Some('w'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.nper({rate=0.05/12, pmt=-500, pv=10000})"#),
        },
        FnDoc {
            name: "rate",
            description: "Interest rate per period for a loan/annuity (Newton-Raphson).",
            params: &[
                Param { name: "nper", short: Some('n'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pmt", short: Some('p'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "pv", short: Some('v'), typ: ParamType::Number, required: true, fields: None },
                Param { name: "fv", short: Some('f'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "when", short: Some('w'), typ: ParamType::Number, required: false, fields: None },
                Param { name: "guess", short: Some('g'), typ: ParamType::Number, required: false, fields: None },
            ],
            returns: ReturnType::Number,
            example: Some(r#"fin.rate({nper=360, pmt=-1073.64, pv=200000})"#),
        },
    ],
};

// ── Helper: extract f64 from mlua::Value ──────────────────────────────────

fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        Value::Integer(n) => *n as f64,
        _ => 0.0,
    }
}

fn require_f64(v: &Value, fn_name: &str, param_name: &str) -> Result<f64, mlua::Error> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Integer(n) => Ok(*n as f64),
        _ => Err(mlua::Error::external(format!(
            "{}: argument '{}' expected number, got {}",
            fn_name,
            param_name,
            v.type_name()
        ))),
    }
}

fn table_to_vec(v: &Value) -> Result<Vec<f64>, mlua::Error> {
    match v {
        Value::Table(t) => {
            // Handle py.list objects: {__py_type="list", data={...}, length=N}
            let py_type: Option<String> = t.get("__py_type").ok();
            if py_type.as_deref() == Some("list") {
                let data: mlua::Table = t
                    .get("data")
                    .map_err(|_| mlua::Error::external("py.list missing 'data' field"))?;
                return table_values_to_vec(&data);
            }

            // Plain Lua table
            table_values_to_vec(t)
        }
        _ => Err(mlua::Error::external("expected table of numbers")),
    }
}

fn table_values_to_vec(t: &mlua::Table) -> Result<Vec<f64>, mlua::Error> {
    let mut out = Vec::new();
    for pair in t.clone().sequence_values::<Value>() {
        let val = pair?;
        match &val {
            Value::Number(n) => out.push(*n),
            Value::Integer(n) => out.push(*n as f64),
            _ => {
                return Err(mlua::Error::external(
                    "cashflows must be a table of numbers",
                ))
            }
        }
    }
    Ok(out)
}

// ── Financial computations ────────────────────────────────────────────────

/// Net Present Value: sum of cashflows[i] / (1+rate)^i
fn compute_npv(rate: f64, cashflows: &[f64]) -> f64 {
    cashflows
        .iter()
        .enumerate()
        .map(|(i, cf)| cf / (1.0 + rate).powi(i as i32))
        .sum()
}

/// IRR via Newton-Raphson: find rate where NPV(rate, cashflows) = 0
fn compute_irr(cashflows: &[f64], guess: f64) -> Result<f64, String> {
    let mut rate = guess;
    let max_iter = 100;
    let tol = 1e-10;

    for _ in 0..max_iter {
        let mut npv = 0.0_f64;
        let mut dnpv = 0.0_f64;
        for (i, cf) in cashflows.iter().enumerate() {
            let factor = (1.0 + rate).powi(i as i32);
            npv += cf / factor;
            if i > 0 {
                dnpv -= (i as f64) * cf / (1.0 + rate).powi(i as i32 + 1);
            }
        }
        if dnpv.abs() < 1e-30 {
            return Err("IRR: derivative too small, cannot converge".into());
        }
        let new_rate = rate - npv / dnpv;
        if (new_rate - rate).abs() < tol {
            return Ok(new_rate);
        }
        rate = new_rate;
    }
    Err("IRR: did not converge after 100 iterations".into())
}

/// MIRR: Modified Internal Rate of Return
fn compute_mirr(cashflows: &[f64], finance_rate: f64, reinvest_rate: f64) -> Result<f64, String> {
    let n = cashflows.len();
    if n < 2 {
        return Err("MIRR requires at least 2 cash flows".into());
    }
    let n_periods = (n - 1) as f64;

    // PV of negative cash flows (costs), discounted at finance_rate
    let pv_costs: f64 = cashflows
        .iter()
        .enumerate()
        .filter(|(_, cf)| **cf < 0.0)
        .map(|(i, cf)| cf / (1.0 + finance_rate).powi(i as i32))
        .sum();

    // FV of positive cash flows (gains), compounded at reinvest_rate
    let fv_gains: f64 = cashflows
        .iter()
        .enumerate()
        .filter(|(_, cf)| **cf > 0.0)
        .map(|(i, cf)| cf * (1.0 + reinvest_rate).powi((n - 1 - i) as i32))
        .sum();

    if pv_costs.abs() < 1e-30 {
        return Err("MIRR: no negative cash flows".into());
    }

    // MIRR = (FV_gains / -PV_costs)^(1/n) - 1
    let ratio = fv_gains / (-pv_costs);
    if ratio < 0.0 {
        return Err("MIRR: cannot compute (negative ratio)".into());
    }
    Ok(ratio.powf(1.0 / n_periods) - 1.0)
}

/// PMT: payment per period
/// when: 0 = end of period (default), 1 = beginning of period
fn compute_pmt(rate: f64, nper: f64, pv: f64, fv: f64, when: f64) -> f64 {
    if rate.abs() < 1e-15 {
        // Zero interest rate
        return -(pv + fv) / nper;
    }
    let factor = (1.0 + rate).powf(nper);
    let pmt = -(pv * factor + fv) / ((1.0 + rate * when) * (factor - 1.0) / rate);
    pmt
}

/// PV: present value
fn compute_pv(rate: f64, nper: f64, pmt: f64, fv: f64, when: f64) -> f64 {
    if rate.abs() < 1e-15 {
        return -(fv + pmt * nper);
    }
    let factor = (1.0 + rate).powf(nper);
    let pv = -(fv + pmt * (1.0 + rate * when) * (factor - 1.0) / rate) / factor;
    pv
}

/// FV: future value
fn compute_fv(rate: f64, nper: f64, pmt: f64, pv: f64, when: f64) -> f64 {
    if rate.abs() < 1e-15 {
        return -(pv + pmt * nper);
    }
    let factor = (1.0 + rate).powf(nper);
    let fv = -(pv * factor + pmt * (1.0 + rate * when) * (factor - 1.0) / rate);
    fv
}

/// NPER: number of periods
fn compute_nper(rate: f64, pmt: f64, pv: f64, fv: f64, when: f64) -> Result<f64, String> {
    if rate.abs() < 1e-15 {
        if pmt.abs() < 1e-15 {
            return Err("nper: cannot determine periods with zero rate and zero payment".into());
        }
        return Ok(-(pv + fv) / pmt);
    }
    let z = pmt * (1.0 + rate * when) / rate;
    let num = -(fv - z) / (pv + z);
    if num <= 0.0 {
        return Err("nper: cannot compute log of non-positive number".into());
    }
    Ok(num.ln() / (1.0 + rate).ln())
}

/// RATE: interest rate per period via Newton-Raphson
fn compute_rate(
    nper: f64,
    pmt: f64,
    pv: f64,
    fv: f64,
    when: f64,
    guess: f64,
) -> Result<f64, String> {
    let mut rate = guess;
    let max_iter = 100;
    let tol = 1e-10;

    for _ in 0..max_iter {
        let factor = (1.0 + rate).powf(nper);
        let dfactor = nper * (1.0 + rate).powf(nper - 1.0);

        // f(rate) = fv + pv*factor + pmt*(1+rate*when)*(factor-1)/rate = 0
        // For numerical stability, compute differently when rate is near zero
        if rate.abs() < 1e-15 {
            // Linear approximation at rate=0
            rate = guess + 0.01;
            continue;
        }

        let annuity = (factor - 1.0) / rate;
        let dannuity = (dfactor * rate - (factor - 1.0)) / (rate * rate);

        let f = fv + pv * factor + pmt * (1.0 + rate * when) * annuity;
        let df = pv * dfactor + pmt * when * annuity + pmt * (1.0 + rate * when) * dannuity;

        if df.abs() < 1e-30 {
            return Err("rate: derivative too small, cannot converge".into());
        }

        let new_rate = rate - f / df;
        if (new_rate - rate).abs() < tol {
            return Ok(new_rate);
        }
        rate = new_rate;
    }
    Err("rate: did not converge after 100 iterations".into())
}

// ── Argument helpers for table-first-arg functions ────────────────────────
//
// validate_args assumes a single-table arg is a "named args wrapper" and tries
// to extract named/positional keys. This breaks `fin.irr({-100, 110})` where
// the table IS the cashflows. We handle irr/mirr/npv manually.

/// Parse args for fin.npv: (rate, cashflows) or ({rate=..., cashflows=...})
fn parse_npv_args(args: &MultiValue) -> Result<(f64, Vec<f64>), mlua::Error> {
    let vals: Vec<&Value> = args.iter().collect();
    match vals.len() {
        0 => Err(mlua::Error::external(
            "fin.npv: missing required arguments 'rate' (number) and 'cashflows' (table)\n  hint: call fin.help() for usage",
        )),
        1 => {
            // Table form: {rate=0.1, cashflows={...}}
            if let Value::Table(t) = &vals[0] {
                let rate: f64 = t.get("rate").map_err(|_| {
                    mlua::Error::external("fin.npv: missing 'rate' in table argument")
                })?;
                let cf_val: Value = t.get("cashflows").or_else(|_| t.get("c")).map_err(|_| {
                    mlua::Error::external("fin.npv: missing 'cashflows' in table argument")
                })?;
                let cashflows = table_to_vec(&cf_val)?;
                Ok((rate, cashflows))
            } else {
                Err(mlua::Error::external("fin.npv: expected (rate, cashflows) or {rate=..., cashflows=...}"))
            }
        }
        _ => {
            // Positional: npv(rate, cashflows)
            let rate = require_f64(vals[0], "fin.npv", "rate")?;
            let cashflows = table_to_vec(vals[1])?;
            Ok((rate, cashflows))
        }
    }
}

/// Parse args for fin.irr: (cashflows, guess?) or ({cashflows=..., guess=...})
fn parse_irr_args(args: &MultiValue) -> Result<(Vec<f64>, f64), mlua::Error> {
    let vals: Vec<&Value> = args.iter().collect();
    match vals.len() {
        0 => Err(mlua::Error::external(
            "fin.irr: missing required argument 'cashflows' (table)\n  hint: call fin.help() for usage",
        )),
        1 => {
            if let Value::Table(t) = &vals[0] {
                // Could be: irr({-100, 110}) or irr({cashflows={...}, guess=0.1})
                // Detect: if t has a "cashflows" key, it's table form
                let cf_val: Result<Value, _> = t.get("cashflows");
                if let Ok(ref v) = cf_val {
                    if !matches!(v, Value::Nil) {
                        let cashflows = table_to_vec(v)?;
                        let guess: f64 = t.get("guess").unwrap_or(0.1);
                        return Ok((cashflows, guess));
                    }
                }
                // Otherwise the table itself is the cashflows
                let cashflows = table_to_vec(vals[0])?;
                Ok((cashflows, 0.1))
            } else {
                Err(mlua::Error::external("fin.irr: expected table of cashflows"))
            }
        }
        _ => {
            // Positional: irr(cashflows, guess?)
            let cashflows = table_to_vec(vals[0])?;
            let guess = if !matches!(vals[1], Value::Nil) {
                to_f64(vals[1])
            } else {
                0.1
            };
            Ok((cashflows, guess))
        }
    }
}

/// Parse args for fin.mirr: (cashflows, finance_rate, reinvest_rate) or table form
fn parse_mirr_args(args: &MultiValue) -> Result<(Vec<f64>, f64, f64), mlua::Error> {
    let vals: Vec<&Value> = args.iter().collect();
    match vals.len() {
        0 => Err(mlua::Error::external(
            "fin.mirr: missing required arguments\n  hint: call fin.help() for usage",
        )),
        1 => {
            if let Value::Table(t) = &vals[0] {
                // Table form: {cashflows={...}, finance_rate=..., reinvest_rate=...}
                let cf_val: Value = t.get("cashflows").or_else(|_| t.get("c")).map_err(|_| {
                    mlua::Error::external("fin.mirr: missing 'cashflows' in table argument")
                })?;
                let cashflows = table_to_vec(&cf_val)?;
                let finance_rate: f64 = t
                    .get("finance_rate")
                    .or_else(|_| t.get("f"))
                    .map_err(|_| mlua::Error::external("fin.mirr: missing 'finance_rate'"))?;
                let reinvest_rate: f64 = t
                    .get("reinvest_rate")
                    .or_else(|_| t.get("i"))
                    .map_err(|_| mlua::Error::external("fin.mirr: missing 'reinvest_rate'"))?;
                Ok((cashflows, finance_rate, reinvest_rate))
            } else {
                Err(mlua::Error::external(
                    "fin.mirr: expected (cashflows, finance_rate, reinvest_rate)",
                ))
            }
        }
        _ => {
            // Positional: mirr(cashflows, finance_rate, reinvest_rate)
            let cashflows = table_to_vec(vals[0])?;
            let finance_rate = to_f64(vals[1]);
            let reinvest_rate = if vals.len() > 2 {
                to_f64(vals[2])
            } else {
                return Err(mlua::Error::external("fin.mirr: missing 'reinvest_rate'"));
            };
            Ok((cashflows, finance_rate, reinvest_rate))
        }
    }
}

// ── Register Luau globals ─────────────────────────────────────────────────

pub fn register_fin_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let fin_table = lua.create_table()?;

    // fin.npv(rate, cashflows) — custom arg parsing (table-first ambiguity)
    fin_table.set(
        "npv",
        lua.create_function(|_, args: MultiValue| {
            let (rate, cashflows) = parse_npv_args(&args)?;
            Ok(compute_npv(rate, &cashflows))
        })?,
    )?;

    // fin.irr(cashflows, guess?) — custom arg parsing (table-first ambiguity)
    fin_table.set(
        "irr",
        lua.create_function(|_, args: MultiValue| {
            let (cashflows, guess) = parse_irr_args(&args)?;
            compute_irr(&cashflows, guess).map_err(mlua::Error::external)
        })?,
    )?;

    // fin.mirr(cashflows, finance_rate, reinvest_rate) — custom arg parsing
    fin_table.set(
        "mirr",
        lua.create_function(|_, args: MultiValue| {
            let (cashflows, finance_rate, reinvest_rate) = parse_mirr_args(&args)?;
            compute_mirr(&cashflows, finance_rate, reinvest_rate).map_err(mlua::Error::external)
        })?,
    )?;

    // fin.pmt(rate, nper, pv, fv?, when?)
    fin_table.set(
        "pmt",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FIN_DOC.params("pmt"), "fin.pmt")?;
            let rate = to_f64(&validated[0]);
            let nper = to_f64(&validated[1]);
            let pv = to_f64(&validated[2]);
            let fv = if validated.len() > 3 && !matches!(validated[3], Value::Nil) {
                to_f64(&validated[3])
            } else {
                0.0
            };
            let when = if validated.len() > 4 && !matches!(validated[4], Value::Nil) {
                to_f64(&validated[4])
            } else {
                0.0
            };
            Ok(compute_pmt(rate, nper, pv, fv, when))
        })?,
    )?;

    // fin.pv(rate, nper, pmt, fv?, when?)
    fin_table.set(
        "pv",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FIN_DOC.params("pv"), "fin.pv")?;
            let rate = to_f64(&validated[0]);
            let nper = to_f64(&validated[1]);
            let pmt = to_f64(&validated[2]);
            let fv = if validated.len() > 3 && !matches!(validated[3], Value::Nil) {
                to_f64(&validated[3])
            } else {
                0.0
            };
            let when = if validated.len() > 4 && !matches!(validated[4], Value::Nil) {
                to_f64(&validated[4])
            } else {
                0.0
            };
            Ok(compute_pv(rate, nper, pmt, fv, when))
        })?,
    )?;

    // fin.fv(rate, nper, pmt, pv?, when?)
    fin_table.set(
        "fv",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FIN_DOC.params("fv"), "fin.fv")?;
            let rate = to_f64(&validated[0]);
            let nper = to_f64(&validated[1]);
            let pmt = to_f64(&validated[2]);
            let pv = if validated.len() > 3 && !matches!(validated[3], Value::Nil) {
                to_f64(&validated[3])
            } else {
                0.0
            };
            let when = if validated.len() > 4 && !matches!(validated[4], Value::Nil) {
                to_f64(&validated[4])
            } else {
                0.0
            };
            Ok(compute_fv(rate, nper, pmt, pv, when))
        })?,
    )?;

    // fin.nper(rate, pmt, pv, fv?, when?)
    fin_table.set(
        "nper",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FIN_DOC.params("nper"), "fin.nper")?;
            let rate = to_f64(&validated[0]);
            let pmt = to_f64(&validated[1]);
            let pv = to_f64(&validated[2]);
            let fv = if validated.len() > 3 && !matches!(validated[3], Value::Nil) {
                to_f64(&validated[3])
            } else {
                0.0
            };
            let when = if validated.len() > 4 && !matches!(validated[4], Value::Nil) {
                to_f64(&validated[4])
            } else {
                0.0
            };
            compute_nper(rate, pmt, pv, fv, when).map_err(mlua::Error::external)
        })?,
    )?;

    // fin.rate(nper, pmt, pv, fv?, when?, guess?)
    fin_table.set(
        "rate",
        lua.create_function(|_, args: MultiValue| {
            let validated = validate_args(&args, FIN_DOC.params("rate"), "fin.rate")?;
            let nper = to_f64(&validated[0]);
            let pmt = to_f64(&validated[1]);
            let pv = to_f64(&validated[2]);
            let fv = if validated.len() > 3 && !matches!(validated[3], Value::Nil) {
                to_f64(&validated[3])
            } else {
                0.0
            };
            let when = if validated.len() > 4 && !matches!(validated[4], Value::Nil) {
                to_f64(&validated[4])
            } else {
                0.0
            };
            let guess = if validated.len() > 5 && !matches!(validated[5], Value::Nil) {
                to_f64(&validated[5])
            } else {
                0.1
            };
            compute_rate(nper, pmt, pv, fv, when, guess).map_err(mlua::Error::external)
        })?,
    )?;

    register_help_functions(lua, &fin_table, &FIN_DOC)?;

    lua.globals().set("fin", fin_table)?;
    wrap_module_with_help_hints(lua, "fin")?;

    Ok(())
}
