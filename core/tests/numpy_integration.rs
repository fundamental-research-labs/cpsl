#![cfg(feature = "mod-numpy")]

use cpsl_core::{transpile, Sandbox};

fn sb() -> Sandbox {
    Sandbox::new().unwrap()
}

// ── Array creation ─────────────────────────────────────────────────

#[test]
fn array_1d_from_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        local shape = numx.shape(a)
        return shape[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "3");
}

#[test]
fn array_2d_from_nested_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local shape = numx.shape(a)
        return shape[1] .. "x" .. shape[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2x2");
}

#[test]
fn zeros_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.zeros(3)
        local t = numx.tolist(a)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "0 0 0");
}

#[test]
fn zeros_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.zeros({2, 3})
        local shape = numx.shape(a)
        return shape[1] .. "x" .. shape[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2x3");
}

#[test]
fn ones_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.ones(4)
        local t = numx.tolist(a)
        return t[1] .. " " .. t[4]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 1");
}

#[test]
fn full_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.full({2, 2}, 7)
        local t = numx.tolist(a)
        return t[1][1] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "7 7");
}

#[test]
fn arange_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.arange(0, 5, 1)
        local t = numx.tolist(a)
        return #t .. " " .. t[1] .. " " .. t[5]
    "#,
        )
        .unwrap();
    assert_eq!(r, "5 0 4");
}

#[test]
fn arange_with_step() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.arange(0, 10, 2)
        local t = numx.tolist(a)
        return #t .. " " .. t[1] .. " " .. t[5]
    "#,
        )
        .unwrap();
    assert_eq!(r, "5 0 8");
}

#[test]
fn linspace_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.linspace(0, 1, 5)
        local t = numx.tolist(a)
        return t[1] .. " " .. t[3] .. " " .. t[5]
    "#,
        )
        .unwrap();
    assert_eq!(r, "0 0.5 1");
}

#[test]
fn eye_3x3() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.eye(3)
        local t = numx.tolist(a)
        return t[1][1] .. " " .. t[1][2] .. " " .. t[2][2] .. " " .. t[3][3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 0 1 1");
}

#[test]
fn diag_from_table() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.diag({1, 2, 3})
        local t = numx.tolist(a)
        return t[1][1] .. " " .. t[1][2] .. " " .. t[2][2] .. " " .. t[3][3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 0 2 3");
}

#[test]
fn diag_extract_from_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local d = numx.diag(a)
        local t = numx.tolist(d)
        return t[1] .. " " .. t[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 4");
}

// ── Element-wise ops ───────────────────────────────────────────────

#[test]
fn add_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        local b = numx.array({4, 5, 6})
        local c = numx.add(a, b)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "5 7 9");
}

#[test]
fn add_scalar_broadcast() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        local c = numx.add(a, 10)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "11 12 13");
}

#[test]
fn sub_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({10, 20, 30})
        local b = numx.array({1, 2, 3})
        local c = numx.sub(a, b)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "9 18 27");
}

#[test]
fn mul_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({2, 3, 4})
        local c = numx.mul(a, 3)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "6 9 12");
}

#[test]
fn div_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({10, 20, 30})
        local c = numx.div(a, 10)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 2 3");
}

#[test]
fn pow_array() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({2, 3, 4})
        local c = numx.pow(a, 2)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4 9 16");
}

#[test]
fn abs_array() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({-1, 2, -3})
        local c = numx.abs(a)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 2 3");
}

#[test]
fn sqrt_array() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({4, 9, 16})
        local c = numx.sqrt(a)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2 3 4");
}

#[test]
fn log_exp_roundtrip() {
    let s = sb();
    let r = s.exec(r#"
        local a = numx.array({1, 2, 3})
        local b = numx.log(a)
        local c = numx.exp(b)
        local t = numx.tolist(c)
        return math.floor(t[1] + 0.5) .. " " .. math.floor(t[2] + 0.5) .. " " .. math.floor(t[3] + 0.5)
    "#).unwrap();
    assert_eq!(r, "1 2 3");
}

#[test]
fn trig_sin_cos() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({0})
        local s = numx.sin(a)
        local c = numx.cos(a)
        local ts = numx.tolist(s)
        local tc = numx.tolist(c)
        return ts[1] .. " " .. tc[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "0 1");
}

#[test]
fn floor_ceil_round() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1.7, 2.3, 3.5})
        local f = numx.tolist(numx.floor(a))
        local c = numx.tolist(numx.ceil(a))
        local rd = numx.tolist(numx.round(a))
        return f[1] .. " " .. c[1] .. " " .. rd[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 2 4");
}

#[test]
fn round_with_decimals() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({3.14159})
        local b = numx.round(a, 2)
        local t = numx.tolist(b)
        return t[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "3.14");
}

// ── Aggregation ────────────────────────────────────────────────────

#[test]
fn sum_full() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3, 4, 5})
        local s = numx.sum(a)
        local t = numx.tolist(s)
        return t[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "15");
}

#[test]
fn sum_axis_0() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local s = numx.sum(a, 0)
        local t = numx.tolist(s)
        return t[1] .. " " .. t[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4 6");
}

#[test]
fn sum_axis_1() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local s = numx.sum(a, 1)
        local t = numx.tolist(s)
        return t[1] .. " " .. t[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "3 7");
}

#[test]
fn mean_full() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({2, 4, 6})
        local m = numx.mean(a)
        local t = numx.tolist(m)
        return t[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4");
}

#[test]
fn std_var() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({2, 4, 4, 4, 5, 5, 7, 9})
        local v = numx.var(a)
        local tv = numx.tolist(v)
        return tv[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4");
}

#[test]
fn min_max() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({5, 1, 8, 3})
        local mn = numx.tolist(numx.min(a))
        local mx = numx.tolist(numx.max(a))
        return mn[1] .. " " .. mx[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 8");
}

#[test]
fn argmin_argmax() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({5, 1, 8, 3})
        return numx.argmin(a) .. " " .. numx.argmax(a)
    "#,
        )
        .unwrap();
    assert_eq!(r, "2 3"); // 1-based
}

#[test]
fn cumsum_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3, 4})
        local c = numx.cumsum(a)
        local t = numx.tolist(c)
        return t[1] .. " " .. t[2] .. " " .. t[3] .. " " .. t[4]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 3 6 10");
}

// ── Shape ops ──────────────────────────────────────────────────────

#[test]
fn reshape_1d_to_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.arange(1, 7, 1)
        local b = numx.reshape(a, {2, 3})
        local shape = numx.shape(b)
        return shape[1] .. "x" .. shape[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2x3");
}

#[test]
fn transpose_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2, 3}, {4, 5, 6}})
        local t = numx.transpose(a)
        local shape = numx.shape(t)
        local tl = numx.tolist(t)
        return shape[1] .. "x" .. shape[2] .. " " .. tl[1][1] .. " " .. tl[1][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "3x2 1 4");
}

#[test]
fn flatten_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local f = numx.flatten(a)
        local shape = numx.shape(f)
        local t = numx.tolist(f)
        return shape[1] .. " " .. t[1] .. " " .. t[4]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4 1 4");
}

#[test]
fn concatenate_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2})
        local b = numx.array({3, 4})
        local c = numx.concatenate({a, b})
        local t = numx.tolist(c)
        return #t .. " " .. t[1] .. " " .. t[4]
    "#,
        )
        .unwrap();
    assert_eq!(r, "4 1 4");
}

#[test]
fn stack_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2})
        local b = numx.array({3, 4})
        local c = numx.stack({a, b}, 0)
        local shape = numx.shape(c)
        local t = numx.tolist(c)
        return shape[1] .. "x" .. shape[2] .. " " .. t[1][1] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2x2 1 4");
}

// ── Sorting/search ─────────────────────────────────────────────────

#[test]
fn sort_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({3, 1, 4, 1, 5})
        local b = numx.sort(a)
        local t = numx.tolist(b)
        return t[1] .. " " .. t[2] .. " " .. t[5]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 1 5");
}

#[test]
fn argsort_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({3, 1, 2})
        local idx = numx.argsort(a)
        local t = numx.tolist(idx)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2 3 1"); // 1-based: element at index 2 is smallest
}

#[test]
fn where_conditional() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local cond = numx.array({1, 0, 1, 0})
        local x = numx.array({10, 20, 30, 40})
        local y = numx.array({-1, -2, -3, -4})
        local result = numx.where(cond, x, y)
        local t = numx.tolist(result)
        return t[1] .. " " .. t[2] .. " " .. t[3] .. " " .. t[4]
    "#,
        )
        .unwrap();
    assert_eq!(r, "10 -2 30 -4");
}

#[test]
fn unique_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({3, 1, 2, 1, 3})
        local u = numx.unique(a)
        local t = numx.tolist(u)
        return #t .. " " .. t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "3 1 2 3");
}

// ── Dot / matmul ───────────────────────────────────────────────────

#[test]
fn dot_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        local b = numx.array({4, 5, 6})
        local c = numx.dot(a, b)
        local t = numx.tolist(c)
        return t[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "32"); // 1*4 + 2*5 + 3*6 = 32
}

#[test]
fn dot_2d_matmul() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local b = numx.array({{5, 6}, {7, 8}})
        local c = numx.dot(a, b)
        local t = numx.tolist(c)
        return t[1][1] .. " " .. t[1][2] .. " " .. t[2][1] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "19 22 43 50");
}

#[test]
fn matmul_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.eye(3)
        local b = numx.array({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}})
        local c = numx.matmul(a, b)
        local t = numx.tolist(c)
        return t[1][1] .. " " .. t[2][2] .. " " .. t[3][3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 5 9");
}

// ── Linear algebra ─────────────────────────────────────────────────

#[test]
fn linalg_det_2x2() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local d = numx.linalg.det(a)
        local t = numx.tolist(d)
        return t[1]
    "#,
        )
        .unwrap();
    let val: f64 = r.parse().unwrap();
    assert!(
        (val - (-2.0)).abs() < 1e-10,
        "det should be -2, got {}",
        val
    );
}

#[test]
fn linalg_inv_2x2() {
    let s = sb();
    let r = s.exec(r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local inv = numx.linalg.inv(a)
        -- Verify A * inv(A) ≈ I
        local prod = numx.dot(a, inv)
        local t = numx.tolist(prod)
        return math.floor(t[1][1] + 0.5) .. " " .. math.floor(t[1][2] + 0.5) .. " " .. math.floor(t[2][1] + 0.5) .. " " .. math.floor(t[2][2] + 0.5)
    "#).unwrap();
    assert_eq!(r, "1 0 0 1");
}

#[test]
fn linalg_solve_2x2() {
    let s = sb();
    let r = s
        .exec(
            r#"
        -- Solve: 2x + y = 5, x + 3y = 7
        local a = numx.array({{2, 1}, {1, 3}})
        local b = numx.array({5, 7})
        local x = numx.linalg.solve(a, b)
        local t = numx.tolist(x)
        return math.floor(t[1] * 10 + 0.5) / 10 .. " " .. math.floor(t[2] * 10 + 0.5) / 10
    "#,
        )
        .unwrap();
    assert_eq!(r, "1.6 1.8");
}

#[test]
fn linalg_norm() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({3, 4})
        local n = numx.linalg.norm(a)
        local t = numx.tolist(n)
        return t[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "5");
}

#[test]
fn linalg_svd_basic() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 0}, {0, 2}})
        local result = numx.linalg.svd(a)
        local s = numx.tolist(result.s)
        -- Singular values should be 2, 1 (sorted descending)
        return math.floor(s[1] + 0.5) .. " " .. math.floor(s[2] + 0.5)
    "#,
        )
        .unwrap();
    assert_eq!(r, "2 1");
}

#[test]
fn linalg_eig_diagonal() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{2, 0}, {0, 3}})
        local result = numx.linalg.eig(a)
        local vals = numx.tolist(result.values)
        -- Eigenvalues of diagonal matrix are the diagonal entries
        local v1 = math.floor(vals[1] + 0.5)
        local v2 = math.floor(vals[2] + 0.5)
        local mn = math.min(v1, v2)
        local mx = math.max(v1, v2)
        return mn .. " " .. mx
    "#,
        )
        .unwrap();
    assert_eq!(r, "2 3");
}

// ── Random ─────────────────────────────────────────────────────────

#[test]
fn random_seed_deterministic() {
    let s = sb();
    let r = s
        .exec(
            r#"
        numx.random.seed(42)
        local a = numx.random.rand(3)
        local t1 = numx.tolist(a)

        numx.random.seed(42)
        local b = numx.random.rand(3)
        local t2 = numx.tolist(b)

        return tostring(t1[1] == t2[1]) .. " " .. tostring(t1[2] == t2[2])
    "#,
        )
        .unwrap();
    assert_eq!(r, "true true");
}

#[test]
fn random_rand_shape() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.random.rand({2, 3})
        local shape = numx.shape(a)
        return shape[1] .. "x" .. shape[2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "2x3");
}

#[test]
fn random_randn_shape() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.random.randn({5})
        local shape = numx.shape(a)
        return shape[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "5");
}

#[test]
fn random_randint_range() {
    let s = sb();
    let r = s
        .exec(
            r#"
        numx.random.seed(0)
        local a = numx.random.randint(0, 10, {100})
        local t = numx.tolist(a)
        local all_in_range = true
        for i = 1, 100 do
            if t[i] < 0 or t[i] >= 10 then
                all_in_range = false
            end
        end
        return tostring(all_in_range)
    "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn random_normal_shape() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.random.normal(0, 1, {10})
        local shape = numx.shape(a)
        return shape[1]
    "#,
        )
        .unwrap();
    assert_eq!(r, "10");
}

#[test]
fn random_uniform_range() {
    let s = sb();
    let r = s
        .exec(
            r#"
        numx.random.seed(0)
        local a = numx.random.uniform(5, 10, {100})
        local t = numx.tolist(a)
        local all_in_range = true
        for i = 1, 100 do
            if t[i] < 5 or t[i] >= 10 then
                all_in_range = false
            end
        end
        return tostring(all_in_range)
    "#,
        )
        .unwrap();
    assert_eq!(r, "true");
}

#[test]
fn random_choice() {
    let s = sb();
    let r = s
        .exec(
            r#"
        numx.random.seed(0)
        local pool = numx.array({10, 20, 30})
        local c = numx.random.choice(pool, 5)
        local t = numx.tolist(c)
        local all_valid = true
        for i = 1, 5 do
            if t[i] ~= 10 and t[i] ~= 20 and t[i] ~= 30 then
                all_valid = false
            end
        end
        return tostring(all_valid) .. " " .. #t
    "#,
        )
        .unwrap();
    assert_eq!(r, "true 5");
}

// ── 2D ops ─────────────────────────────────────────────────────────

#[test]
fn add_2d_arrays() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local b = numx.array({{5, 6}, {7, 8}})
        local c = numx.add(a, b)
        local t = numx.tolist(c)
        return t[1][1] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "6 12");
}

#[test]
fn cumsum_axis_0() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local c = numx.cumsum(a, 0)
        local t = numx.tolist(c)
        return t[1][1] .. " " .. t[2][1] .. " " .. t[1][2] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 4 2 6");
}

// ── Help / meta ────────────────────────────────────────────────────

#[test]
fn numpy_help() {
    let s = sb();
    let r = s.exec("return numx.help()").unwrap();
    assert!(r.contains("numx"), "help: {}", r);
    assert!(r.contains("numx.array"), "help: {}", r);
}

#[test]
fn numpy_nonexistent_fn_hint() {
    let s = sb();
    let err = s.exec("numx.foo()").unwrap_err();
    assert!(
        err.message.contains("numx.foo does not exist"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_numpy() {
    let s = sb();
    let r = s.exec("return help()").unwrap();
    assert!(r.contains("numx"), "global help should list numx: {}", r);
}

// ── tolist roundtrip ───────────────────────────────────────────────

#[test]
fn tolist_1d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({10, 20, 30})
        local t = numx.tolist(a)
        return t[1] .. " " .. t[2] .. " " .. t[3]
    "#,
        )
        .unwrap();
    assert_eq!(r, "10 20 30");
}

#[test]
fn tolist_2d() {
    let s = sb();
    let r = s
        .exec(
            r#"
        local a = numx.array({{1, 2}, {3, 4}})
        local t = numx.tolist(a)
        return t[1][1] .. " " .. t[1][2] .. " " .. t[2][1] .. " " .. t[2][2]
    "#,
        )
        .unwrap();
    assert_eq!(r, "1 2 3 4");
}

// ── Error handling ─────────────────────────────────────────────────

#[test]
fn shape_mismatch_add() {
    let s = sb();
    let err = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        local b = numx.array({1, 2})
        numx.add(a, b)
    "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("shape mismatch"),
        "msg: {}",
        err.message
    );
}

#[test]
fn reshape_size_mismatch() {
    let s = sb();
    let err = s
        .exec(
            r#"
        local a = numx.array({1, 2, 3})
        numx.reshape(a, {2, 2})
    "#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("cannot reshape"),
        "msg: {}",
        err.message
    );
}

#[test]
fn inv_non_square() {
    let s = sb();
    let err = s
        .exec(
            r#"
        local a = numx.array({{1, 2, 3}, {4, 5, 6}})
        numx.linalg.inv(a)
    "#,
        )
        .unwrap_err();
    assert!(err.message.contains("square"), "msg: {}", err.message);
}

// ── Python transpiler e2e ──────────────────────────────────────────

#[test]
fn python_import_numpy_as_np() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // Use numpy functions that don't require Python list → Lua table conversion.
    // numx.zeros/ones/arange return sandbox arrays directly.
    let py_code = r#"
import numpy as np
z = np.zeros(3)
o = np.ones(3)
result = np.add(z, o)
t = np.tolist(result)
print(json.encode(t))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v[0], 1.0);
    assert_eq!(v[1], 1.0);
    assert_eq!(v[2], 1.0);
}

#[test]
fn python_numpy_arange_linspace() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy as np
a = np.arange(0, 5, 1)
shape = np.shape(a)
print(json.encode(shape))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v[0], 5);
}

#[test]
fn python_numpy_linalg_det() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // Use numx.eye which doesn't need Python list input
    let py_code = r#"
import numpy as np
a = np.eye(2)
d = np.linalg.det(a)
t = np.tolist(d)
print(json.encode(t))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    let val = v[0].as_f64().unwrap();
    assert!(
        (val - 1.0).abs() < 1e-10,
        "det of identity should be 1, got {}",
        val
    );
}

#[test]
fn python_numpy_random_seed() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy as np
np.random.seed(42)
a = np.random.rand(5)
shape = np.shape(a)
print(json.encode(shape))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v[0], 5);
}

#[test]
fn python_numpy_array_from_list_1d() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy
a = numpy.array([1, 2, 3, 4, 5])
print(json.encode(numpy.tolist(a)))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([1, 2, 3, 4, 5]));
}

#[test]
fn python_numpy_array_from_list_2d() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    let py_code = r#"
import numpy as np
a = np.array([[1, 2], [3, 4]])
shape = np.shape(a)
print(json.encode(shape))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([2, 2]));
}

#[test]
fn python_numpy_zeros_tuple_shape() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // np.zeros((2, 3)) — tuple shape transpiles through py.tuple
    let py_code = r#"
import numpy as np
a = np.zeros((2, 3))
print(json.encode(np.shape(a)))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([2, 3]));
}

#[test]
fn python_numpy_diag_from_list() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // np.diag([1, 2, 3]) — list arg transpiles through py.list
    let py_code = r#"
import numpy as np
d = np.diag([1, 2, 3])
print(json.encode(np.shape(d)))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([3, 3]));
}

#[test]
fn python_numpy_concatenate_list_of_arrays() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // np.concatenate([a, b]) — outer list transpiles through py.list
    let py_code = r#"
import numpy as np
a = np.array([1, 2])
b = np.array([3, 4])
c = np.concatenate([a, b])
print(json.encode(np.tolist(c)))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([1, 2, 3, 4]));
}

#[test]
fn python_numpy_stack_list_of_arrays() {
    let s = sb();
    let pyrt = include_str!("../../runtime/pyrt.luau");
    s.setup_python_runtime(pyrt).unwrap();

    // np.stack([a, b]) — outer list transpiles through py.list
    let py_code = r#"
import numpy as np
a = np.array([1, 2])
b = np.array([3, 4])
c = np.stack([a, b])
print(json.encode(np.shape(c)))
"#;
    let transpiled = transpile::transpile(py_code).unwrap();
    let r = s.exec(&transpiled.luau_source).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v, serde_json::json!([2, 2]));
}
