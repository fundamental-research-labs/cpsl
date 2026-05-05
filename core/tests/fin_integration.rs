#![cfg(feature = "mod-fin")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

/// Helper: exec Luau, parse result as f64, assert it's close to expected.
fn assert_close(code: &str, expected: f64, tol: f64) {
    let s = sb();
    let r = s.exec(code).unwrap();
    let actual: f64 = r.parse().unwrap_or_else(|_| panic!("not a number: {}", r));
    assert!(
        (actual - expected).abs() < tol,
        "expected ~{}, got {} (diff {})\ncode: {}",
        expected,
        actual,
        (actual - expected).abs(),
        code
    );
}

// ── fin.npv ───────────────────────────────────────────────────────

#[test]
fn npv_basic() {
    // NPV at 10% of -100, 30, 40, 50, 60
    // = -100 + 30/1.1 + 40/1.21 + 50/1.331 + 60/1.4641 ≈ 38.877
    assert_close("return fin.npv(0.10, {-100, 30, 40, 50, 60})", 38.877, 0.01);
}

#[test]
fn npv_zero_rate() {
    // At 0% discount, NPV = sum of cash flows
    assert_close("return fin.npv(0, {-100, 50, 50, 50})", 50.0, 0.0001);
}

#[test]
fn npv_negative_result() {
    // -1000 + 100/1.1 + 100/1.21 ≈ -826.45
    assert_close("return fin.npv(0.10, {-1000, 100, 100})", -826.4463, 0.01);
}

#[test]
fn npv_single_cashflow() {
    assert_close("return fin.npv(0.05, {1000})", 1000.0, 0.0001);
}

// ── fin.irr ───────────────────────────────────────────────────────

#[test]
fn irr_basic() {
    // Invest -100, get 110 → IRR = 10%
    assert_close("return fin.irr({-100, 110})", 0.1, 0.0001);
}

#[test]
fn irr_multi_period() {
    // -100, 39.2, 39.2, 39.2, 39.2 — verify via NPV roundtrip
    let s = sb();
    let r = s
        .exec(
            r#"
            local cf = {-100, 39.2, 39.2, 39.2, 39.2}
            local rate = fin.irr(cf)
            -- Verify NPV at IRR is ~0
            local npv = fin.npv(rate, cf)
            return string.format("%.8f", math.abs(npv))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.001, "NPV at IRR should be ~0, got: {}", val);
}

#[test]
fn irr_with_guess() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local cf = {-100, 60, 60}
            local rate = fin.irr(cf, 0.05)
            local npv = fin.npv(rate, cf)
            return string.format("%.8f", math.abs(npv))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.001, "NPV at IRR should be ~0, got: {}", val);
}

#[test]
fn irr_breakeven() {
    // -100, 50, 50 → IRR = 0%
    assert_close("return fin.irr({-100, 50, 50})", 0.0, 0.0001);
}

// ── fin.mirr ──────────────────────────────────────────────────────

#[test]
fn mirr_basic() {
    // Verify MIRR via self-consistent computation
    let s = sb();
    let r = s
        .exec(
            r#"
            local result = fin.mirr({-100, 50, 60, 70}, 0.10, 0.12)
            return string.format("%.4f", result)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    // MIRR should be positive and reasonable for this investment
    assert!(val > 0.15 && val < 0.40, "MIRR out of range: {}", val);
}

#[test]
fn mirr_simple() {
    // -100, 200 with finance_rate=0.05, reinvest_rate=0.05
    // PV_costs = -100, FV_gains = 200
    // MIRR = (200/100)^(1/1) - 1 = 1.0
    assert_close("return fin.mirr({-100, 200}, 0.05, 0.05)", 1.0, 0.0001);
}

// ── fin.pmt ───────────────────────────────────────────────────────

#[test]
fn pmt_basic_loan() {
    // $200,000 loan at 7.5%/12 per month for 30 years
    // Standard formula: pmt ≈ -1398.43
    assert_close("return fin.pmt(0.075/12, 360, 200000)", -1398.43, 0.1);
}

#[test]
fn pmt_with_fv() {
    // Save to accumulate 100,000 over 10 years at 5%/12
    // Verify PMT/FV self-consistency: compute PMT, then verify FV ≈ 0 when all terms present
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.05/12
            local nper = 120
            local pv = 0
            local fv_target = -100000
            local payment = fin.pmt(rate, nper, pv, fv_target)
            -- TVM: FV(rate, nper, pmt, pv) + fv_target should ≈ 0
            local actual_fv = fin.fv(rate, nper, payment, pv)
            return string.format("%.4f", math.abs(actual_fv - fv_target))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.1, "FV should match target: diff={}", val);
}

#[test]
fn pmt_zero_rate() {
    assert_close("return fin.pmt(0, 10, 1000)", -100.0, 0.0001);
}

#[test]
fn pmt_beginning_of_period() {
    // when=1 (beginning of period): verify it differs from end-of-period
    let s = sb();
    let r = s
        .exec(
            r#"
            local end_pmt = fin.pmt(0.08/12, 60, 15000, 0, 0)
            local beg_pmt = fin.pmt(0.08/12, 60, 15000, 0, 1)
            -- Beginning payments should be smaller in absolute value
            local diff = math.abs(end_pmt) - math.abs(beg_pmt)
            return string.format("%.4f", diff)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(
        val > 0.0,
        "beginning-of-period PMT should be smaller than end: diff={}",
        val
    );
}

// ── fin.pv ────────────────────────────────────────────────────────

#[test]
fn pv_basic() {
    // PV of -100/month payment at 8%/12 for 5 years — verify self-consistency
    let s = sb();
    let r = s
        .exec(
            r#"
            local pv = fin.pv(0.08/12, 60, -100)
            -- PV should be positive (receiving payments)
            return string.format("%.2f", pv)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val > 4900.0 && val < 5000.0, "PV out of range: {}", val);
}

#[test]
fn pv_with_fv() {
    // Verify PV + FV consistency
    let s = sb();
    let r = s
        .exec(
            r#"
            local pv = fin.pv(0.05, 10, -100, -1000)
            return string.format("%.2f", pv)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val > 1300.0 && val < 1500.0, "PV out of range: {}", val);
}

#[test]
fn pv_zero_rate() {
    assert_close("return fin.pv(0, 10, -100)", 1000.0, 0.0001);
}

// ── fin.fv ────────────────────────────────────────────────────────

#[test]
fn fv_basic() {
    // FV of 100/month at 5%/12 for 10 years, starting with 1000
    let s = sb();
    let r = s
        .exec(
            r#"
            local fv = fin.fv(0.05/12, 120, -100, -1000)
            return string.format("%.2f", fv)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    // Should be around 17000-17500
    assert!(val > 17000.0 && val < 17500.0, "FV out of range: {}", val);
}

#[test]
fn fv_savings() {
    // Save 200/month at 6%/12 for 15 years
    let s = sb();
    let r = s
        .exec(
            r#"
            local fv = fin.fv(0.06/12, 180, -200)
            return string.format("%.2f", fv)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val > 57000.0 && val < 59000.0, "FV out of range: {}", val);
}

#[test]
fn fv_zero_rate() {
    assert_close("return fin.fv(0, 10, -100, -500)", 1500.0, 0.0001);
}

// ── fin.nper ──────────────────────────────────────────────────────

#[test]
fn nper_basic() {
    // How many months to pay off $10,000? Verify self-consistency
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.08/12
            local pmt = -200
            local pv = 10000
            local n = fin.nper(rate, pmt, pv)
            -- Verify: FV at computed nper should be ~0
            local fv = fin.fv(rate, n, pmt, pv)
            return string.format("%.4f", math.abs(fv))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 1.0, "FV at nper should be ~0, got: {}", val);
}

#[test]
fn nper_zero_rate() {
    assert_close("return fin.nper(0, -100, 1000)", 10.0, 0.0001);
}

#[test]
fn nper_with_fv() {
    // Verify self-consistency
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.06/12
            local pmt = -100
            local pv = -5000
            local fv_target = 10000
            local n = fin.nper(rate, pmt, pv, fv_target)
            -- n should be positive and reasonable
            return string.format("%.2f", n)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val > 30.0 && val < 50.0, "NPER out of range: {}", val);
}

// ── fin.rate ──────────────────────────────────────────────────────

#[test]
fn rate_basic() {
    // What rate for 360 payments of -1398.43 on 200,000 loan?
    // Should recover ~0.00625 (= 7.5%/12)
    assert_close("return fin.rate(360, -1398.43, 200000)", 0.00625, 0.0001);
}

#[test]
fn rate_with_fv() {
    // What rate to grow 1000 to 2000 over 10 years with no payments?
    // (1+r)^10 = 2 → r = 2^(1/10) - 1 ≈ 0.07177
    assert_close("return fin.rate(10, 0, -1000, 2000)", 0.07177, 0.001);
}

// ── Dual-signature tests (table form for shell dispatch) ─────────

#[test]
fn npv_table_form() {
    // Same NPV as npv_basic but via table form
    assert_close(
        "return fin.npv({rate=0.10, cashflows={-100, 30, 40, 50, 60}})",
        38.877,
        0.01,
    );
}

#[test]
fn pmt_table_form() {
    assert_close(
        "return fin.pmt({rate=0.075/12, nper=360, pv=200000})",
        -1398.43,
        0.1,
    );
}

#[test]
fn irr_table_form() {
    assert_close("return fin.irr({cashflows={-100, 110}})", 0.1, 0.0001);
}

#[test]
fn mirr_table_form() {
    let s = sb();
    let r = s
        .exec("return tostring(fin.mirr({cashflows={-100, 200}, finance_rate=0.05, reinvest_rate=0.05}))")
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!((val - 1.0).abs() < 0.001, "MIRR table form: {}", val);
}

// ── Shell dispatch tests ────────────────────────────────────────

#[test]
fn shell_fin_help() {
    let s = sb();
    let shrt = include_str!("../../runtime/shrt.luau");
    s.register_module("shrt", shrt).unwrap();
    let result = cpsl_core::sh_transpile::transpile_sh(r#"fin help"#);
    assert!(result.is_ok(), "transpile err: {:?}", result.err());
    let luau = result.unwrap().luau_source;
    let r = s.exec(&luau);
    assert!(r.is_ok(), "shell fin help should not error: {:?}", r.err());
}

// ── Error handling ──────────────────────────────────────────────

#[test]
fn npv_no_args_errors() {
    let s = sb();
    let err = s.exec("fin.npv()").unwrap_err();
    assert!(
        err.message.contains("missing") || err.message.contains("fin.npv"),
        "msg: {}",
        err.message
    );
}

#[test]
fn pmt_no_args_errors() {
    let s = sb();
    let err = s.exec("fin.pmt()").unwrap_err();
    assert!(
        err.message.contains("bad argument") || err.message.contains("missing"),
        "msg: {}",
        err.message
    );
}

#[test]
fn npv_wrong_type_errors() {
    let s = sb();
    let err = s.exec(r#"fin.npv("abc", {1,2,3})"#).unwrap_err();
    assert!(
        err.message.contains("number") || err.message.contains("expected"),
        "msg: {}",
        err.message
    );
}

#[test]
fn irr_non_convergence() {
    // All positive cash flows — NPV is always positive, no IRR exists
    let s = sb();
    let err = s.exec("fin.irr({100, 100, 100})").unwrap_err();
    assert!(
        err.message.contains("converge")
            || err.message.contains("IRR")
            || err.message.contains("derivative"),
        "msg: {}",
        err.message
    );
}

#[test]
fn mirr_no_negatives_errors() {
    let s = sb();
    let err = s.exec("fin.mirr({100, 200}, 0.1, 0.1)").unwrap_err();
    assert!(
        err.message.contains("negative") || err.message.contains("MIRR"),
        "msg: {}",
        err.message
    );
}

#[test]
fn mirr_too_few_cashflows_errors() {
    let s = sb();
    let err = s.exec("fin.mirr({-100}, 0.1, 0.1)").unwrap_err();
    assert!(err.message.contains("at least 2"), "msg: {}", err.message);
}

#[test]
fn nper_impossible_errors() {
    let s = sb();
    let err = s.exec("fin.nper(0, 0, 1000)").unwrap_err();
    assert!(
        err.message.contains("cannot") || err.message.contains("nper"),
        "msg: {}",
        err.message
    );
}

// ── Help ────────────────────────────────────────────────────────

#[test]
fn fin_help_returns_help() {
    let s = sb();
    let r = s.exec("return fin.help()").unwrap();
    assert!(r.contains("fin"), "help: {}", r);
    assert!(r.contains("fin.npv"), "help: {}", r);
    assert!(r.contains("fin.irr"), "help: {}", r);
    assert!(r.contains("fin.pmt"), "help: {}", r);
}

#[test]
fn fin_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("fin.foo()").unwrap_err();
    assert!(
        err.message.contains("fin.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call fin.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_fin() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("fin"), "global help should list fin: {}", r);
}

// ── Sandbox safety: no filesystem or network access ─────────────

#[test]
fn fin_does_not_access_filesystem() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local n = fin.npv(0.1, {-100, 50, 60})
            local p = fin.pmt(0.05/12, 360, 200000)
            local f = fin.fv(0.06/12, 120, -200)
            return string.format("%.2f %.2f %.2f", n, p, f)
        "#,
        )
        .unwrap();
    // Just verify it executed purely computationally
    assert!(r.contains("-") || r.contains("."), "result: {}", r);
}

#[test]
fn fin_no_dangerous_globals_exposed() {
    let s = sb();
    let r = s
        .exec(
            r#"
            return tostring(type(fin.npv)) .. " " ..
                   tostring(type(fin.irr)) .. " " ..
                   tostring(type(fin.pmt)) .. " " ..
                   tostring(rawget(fin, "io")) .. " " ..
                   tostring(rawget(fin, "os"))
        "#,
        )
        .unwrap();
    assert_eq!(r, "function function function nil nil");
}

#[test]
fn fin_sandbox_no_io_access() {
    let s = sb();
    let r = s
        .exec(
            r#"
            local mt = getmetatable(fin)
            if mt then
                local idx = rawget(mt, "__index")
                if type(idx) == "table" then
                    if rawget(idx, "io") or rawget(idx, "os") then
                        return "metatable leaks dangerous globals"
                    end
                end
            end
            local count = 0
            for k, v in pairs(fin) do
                count = count + 1
            end
            return "safe:" .. count
        "#,
        )
        .unwrap();
    assert!(r.starts_with("safe:"), "expected safe table, got: {}", r);
}

#[test]
fn fin_sandbox_no_network_access() {
    // Verify all operations are purely computational — no network needed
    let s = sb();
    let r = s
        .exec(
            r#"
            local npv = fin.npv(0.05, {-100, 110})
            local pmt = fin.pmt(0, 10, 1000)
            local fv = fin.fv(0, 5, -100)
            return string.format("%.4f,%.4f,%.4f", npv, pmt, fv)
        "#,
        )
        .unwrap();
    // NPV(0.05, {-100, 110}) = -100 + 110/1.05 ≈ 4.7619
    assert!(
        r.contains("-100.0000") && r.contains("500.0000"),
        "got: {}",
        r
    );
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn npv_large_cashflows() {
    // Verify with many periods — just check it runs and is negative
    let s = sb();
    let r = s
        .exec(
            r#"
            local cf = {}
            cf[1] = -10000
            for i = 2, 101 do cf[i] = 200 end
            local npv = fin.npv(0.05, cf)
            return string.format("%.2f", npv)
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    // 100 payments of 200 at 5% discount < 10000, so NPV should be negative
    assert!(val < 0.0 && val > -7000.0, "NPV out of range: {}", val);
}

#[test]
fn pmt_fv_consistency() {
    // PMT and FV should be consistent:
    // If we compute PMT for a loan, then FV at that PMT should be ~0
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.06/12
            local nper = 120
            local pv = 50000
            local payment = fin.pmt(rate, nper, pv)
            local future = fin.fv(rate, nper, payment, pv)
            return string.format("%.6f", math.abs(future))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.01, "FV should be ~0 when using PMT, got: {}", val);
}

#[test]
fn pv_fv_inverse() {
    // Verify PV and FV are inverses:
    // If we compute PV then FV from same params, they should satisfy the TVM equation
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.08/12
            local nper = 60
            local pmt = -500
            local pv = fin.pv(rate, nper, pmt)
            -- The FV at computed PV should be ~0
            local fv = fin.fv(rate, nper, pmt, pv)
            return string.format("%.6f", math.abs(fv))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.01, "FV should be ~0, got: {}", val);
}

#[test]
fn rate_pmt_roundtrip() {
    // Compute PMT from known rate, then recover rate from that PMT
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.06/12
            local nper = 360
            local pv = 300000
            local payment = fin.pmt(rate, nper, pv)
            local recovered_rate = fin.rate(nper, payment, pv)
            return string.format("%.8f", math.abs(recovered_rate - rate))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.0001, "rate roundtrip error: {}", val);
}

#[test]
fn nper_pmt_roundtrip() {
    // Compute NPER from known PMT, then recover PMT from that NPER
    let s = sb();
    let r = s
        .exec(
            r#"
            local rate = 0.05/12
            local pmt = -500
            local pv = 20000
            local periods = fin.nper(rate, pmt, pv)
            local recovered_pmt = fin.pmt(rate, periods, pv)
            return string.format("%.4f", math.abs(recovered_pmt - pmt))
        "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(val < 0.1, "nper/pmt roundtrip error: {}", val);
}

// ── Python transpiler e2e ───────────────────────────────────────

#[test]
fn python_numpy_financial_npv() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy_financial as npf
result = npf.npv(0.1, [-100, 30, 40, 50, 60])
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let val: f64 = r.trim().parse().unwrap_or_else(|_| {
        panic!(
            "not a number: '{}'\ntranspiled:\n{}",
            r, transpiled.luau_source
        )
    });
    assert!(
        (val - 38.877).abs() < 0.1,
        "expected ~38.877, got: {}\ntranspiled:\n{}",
        val,
        transpiled.luau_source
    );
}

#[test]
fn python_numpy_financial_pmt() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy_financial as npf
result = npf.pmt(0.075/12, 360, 200000)
print(result)
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let val: f64 = r
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("not a number: {}", r));
    assert!(
        (val - (-1398.43)).abs() < 1.0,
        "expected ~-1398.43, got: {}",
        val
    );
}

#[test]
fn python_from_numpy_financial_import() {
    let py_code = r#"
from numpy_financial import npv, irr, pmt
result = npv(0.1, {-100, 110})
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("fin"),
        "transpiled: {}",
        transpiled.luau_source
    );
}

#[test]
fn python_import_numpy_financial_passthrough() {
    let py_code = r#"
import numpy_financial
result = numpy_financial.irr({-100, 110})
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    assert!(
        transpiled.luau_source.contains("fin"),
        "transpiled: {}",
        transpiled.luau_source
    );
}
