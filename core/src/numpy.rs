//! NumPy-compatible module for the Luau sandbox.
//!
//! Exposes `numpy.*` globals backed by `ndarray` (array ops) + `faer` (linalg).
//! Arrays are stored as Lua userdata wrapping `NdArray`.

use crate::sandbox::wrap_module_with_help_hints;
use mlua::{Lua, UserData, UserDataMethods, Value};
use ndarray::{Array1, Array2, Axis};
use std::sync::Arc;
use std::sync::Mutex;

// ── NdArray userdata ────────────────────────────────────────────────

/// The array type exposed to Luau. Can be 1D or 2D.
#[derive(Debug, Clone)]
pub enum NdArray {
    D1(Array1<f64>),
    D2(Array2<f64>),
}

impl NdArray {
    fn shape_vec(&self) -> Vec<usize> {
        match self {
            NdArray::D1(a) => vec![a.len()],
            NdArray::D2(a) => vec![a.nrows(), a.ncols()],
        }
    }

    /// Convert to a flat Vec<f64>.
    fn to_flat_vec(&self) -> Vec<f64> {
        match self {
            NdArray::D1(a) => a.to_vec(),
            NdArray::D2(a) => a.iter().copied().collect(),
        }
    }

    /// Try to get a 2D view. Promotes 1D (n,) to (1, n).
    fn as_2d(&self) -> Array2<f64> {
        match self {
            NdArray::D1(a) => a.clone().into_shape_with_order((1, a.len())).unwrap(),
            NdArray::D2(a) => a.clone(),
        }
    }

    /// Element-wise unary op.
    fn map_elem<F: Fn(f64) -> f64>(&self, f: F) -> NdArray {
        match self {
            NdArray::D1(a) => NdArray::D1(a.mapv(&f)),
            NdArray::D2(a) => NdArray::D2(a.mapv(&f)),
        }
    }
}

impl UserData for NdArray {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| Ok(format!("{:?}", this)));
        methods.add_meta_method("__len", |_, this, ()| Ok(this.shape_vec()[0]));
    }
}

// ── Helper: extract array from Lua value ────────────────────────────

use crate::pyrt_compat::unwrap_py_seq as unwrap_py_table;

fn value_to_array(val: &Value) -> Result<NdArray, mlua::Error> {
    match val {
        Value::UserData(ud) => {
            let arr = ud.borrow::<NdArray>()?;
            Ok(arr.clone())
        }
        Value::Table(t) => table_to_array(t),
        _ => Err(mlua::Error::external(
            "expected a numx array (userdata or table)",
        )),
    }
}

/// Parse a Lua table into an NdArray.
/// - Flat table of numbers → 1D
/// - Table of tables → 2D
/// - py.list/py.tuple wrappers → unwrap `.data` sub-table
fn table_to_array(t: &mlua::Table) -> Result<NdArray, mlua::Error> {
    // Unwrap py.list/py.tuple wrappers to get the raw data table
    let t = unwrap_py_table(t)?;

    let len = t.raw_len();
    if len == 0 {
        return Ok(NdArray::D1(Array1::zeros(0)));
    }

    let first: Value = t.get(1)?;
    match first {
        Value::Table(ref inner) => {
            // 2D: table of tables — unwrap inner rows too
            let inner = unwrap_py_table(inner)?;
            let ncols = inner.raw_len();
            let mut data = Vec::with_capacity(len * ncols);
            for i in 1..=len {
                let row: mlua::Table = t.get(i)?;
                let row = unwrap_py_table(&row)?;
                let row_len = row.raw_len();
                if row_len != ncols {
                    return Err(mlua::Error::external(format!(
                        "row {} has {} elements, expected {}",
                        i, row_len, ncols
                    )));
                }
                for j in 1..=ncols {
                    let v: Value = row.get(j)?;
                    data.push(lua_num(&v)?);
                }
            }
            let arr = Array2::from_shape_vec((len, ncols), data)
                .map_err(|e| mlua::Error::external(e.to_string()))?;
            Ok(NdArray::D2(arr))
        }
        _ => {
            // 1D: flat table of numbers
            let mut data = Vec::with_capacity(len);
            data.push(lua_num(&first)?);
            for i in 2..=len {
                let v: Value = t.get(i)?;
                data.push(lua_num(&v)?);
            }
            Ok(NdArray::D1(Array1::from_vec(data)))
        }
    }
}

/// Extract f64 from a Lua number/integer/string.
/// Accepts strings like "1" or "3.14" since shell args arrive as strings.
fn lua_num(v: &Value) -> Result<f64, mlua::Error> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Integer(i) => Ok(*i as f64),
        Value::String(s) => {
            let s = s
                .to_str()
                .map_err(|_| mlua::Error::external("expected number, got non-UTF8 string"))?;
            s.parse::<f64>()
                .map_err(|_| mlua::Error::external(format!("expected number, got string {:?}", s)))
        }
        _ => Err(mlua::Error::external(format!(
            "expected number, got {:?}",
            v
        ))),
    }
}

/// Convert NdArray back to Lua table(s).
fn array_to_lua(lua: &Lua, arr: &NdArray) -> Result<Value, mlua::Error> {
    match arr {
        NdArray::D1(a) => {
            let t = lua.create_table()?;
            for (i, &v) in a.iter().enumerate() {
                t.set(i + 1, v)?;
            }
            Ok(Value::Table(t))
        }
        NdArray::D2(a) => {
            let t = lua.create_table()?;
            for (i, row) in a.rows().into_iter().enumerate() {
                let rt = lua.create_table()?;
                for (j, &v) in row.iter().enumerate() {
                    rt.set(j + 1, v)?;
                }
                t.set(i + 1, rt)?;
            }
            Ok(Value::Table(t))
        }
    }
}

/// Parse shape from Lua: either a single number or a table {rows, cols}.
/// Also accepts string numbers since shell args arrive as strings.
fn parse_shape(val: &Value) -> Result<Vec<usize>, mlua::Error> {
    match val {
        Value::Integer(n) => Ok(vec![*n as usize]),
        Value::Number(n) => Ok(vec![*n as usize]),
        Value::String(s) => {
            let s = s
                .to_str()
                .map_err(|_| mlua::Error::external("shape must be a number or table"))?;
            let n: usize = s.parse().map_err(|_| {
                mlua::Error::external(format!(
                    "shape must be a number or table, got string {:?}",
                    s
                ))
            })?;
            Ok(vec![n])
        }
        Value::Table(t) => {
            let t = unwrap_py_table(t)?;
            let len = t.raw_len();
            let mut shape = Vec::with_capacity(len);
            for i in 1..=len {
                let v: Value = t.get(i)?;
                match v {
                    Value::Integer(n) => shape.push(n as usize),
                    Value::Number(n) => shape.push(n as usize),
                    Value::String(s) => {
                        let s = s
                            .to_str()
                            .map_err(|_| mlua::Error::external("shape elements must be numbers"))?;
                        shape.push(s.parse().map_err(|_| {
                            mlua::Error::external("shape elements must be numbers")
                        })?);
                    }
                    _ => return Err(mlua::Error::external("shape elements must be numbers")),
                }
            }
            Ok(shape)
        }
        _ => Err(mlua::Error::external("shape must be a number or table")),
    }
}

/// Create an NdArray of zeros/ones/full with the given shape.
fn make_filled(shape: &[usize], val: f64) -> Result<NdArray, mlua::Error> {
    match shape.len() {
        1 => Ok(NdArray::D1(Array1::from_elem(shape[0], val))),
        2 => Ok(NdArray::D2(Array2::from_elem((shape[0], shape[1]), val))),
        _ => Err(mlua::Error::external("shape must be 1D or 2D")),
    }
}

// ── Binary element-wise helper ──────────────────────────────────────

/// Check if a value is a scalar number (including string-encoded numbers from shell).
fn is_scalar(v: &Value) -> bool {
    match v {
        Value::Number(_) | Value::Integer(_) => true,
        Value::String(s) => s
            .to_str()
            .map(|s| s.parse::<f64>().is_ok())
            .unwrap_or(false),
        _ => false,
    }
}

/// Apply a binary op element-wise. Supports array+array and array+scalar broadcasting.
fn binop<F: Fn(f64, f64) -> f64>(a: &Value, b: &Value, f: F) -> Result<NdArray, mlua::Error> {
    let a_is_num = is_scalar(a);
    let b_is_num = is_scalar(b);

    if a_is_num && b_is_num {
        let va = lua_num(a)?;
        let vb = lua_num(b)?;
        return Ok(NdArray::D1(Array1::from_vec(vec![f(va, vb)])));
    }

    if a_is_num {
        // scalar + array
        let scalar = lua_num(a)?;
        let arr = value_to_array(b)?;
        return Ok(arr.map_elem(|x| f(scalar, x)));
    }

    if b_is_num {
        // array + scalar
        let arr = value_to_array(a)?;
        let scalar = lua_num(b)?;
        return Ok(arr.map_elem(|x| f(x, scalar)));
    }

    // array + array
    let arr_a = value_to_array(a)?;
    let arr_b = value_to_array(b)?;

    match (&arr_a, &arr_b) {
        (NdArray::D1(a), NdArray::D1(b)) => {
            if a.len() != b.len() {
                return Err(mlua::Error::external(format!(
                    "shape mismatch: ({},) vs ({},)",
                    a.len(),
                    b.len()
                )));
            }
            let result: Array1<f64> = a.iter().zip(b.iter()).map(|(&x, &y)| f(x, y)).collect();
            Ok(NdArray::D1(result))
        }
        (NdArray::D2(a), NdArray::D2(b)) => {
            if a.shape() != b.shape() {
                return Err(mlua::Error::external(format!(
                    "shape mismatch: {:?} vs {:?}",
                    a.shape(),
                    b.shape()
                )));
            }
            let result: Array2<f64> =
                Array2::from_shape_fn(a.dim(), |(i, j)| f(a[[i, j]], b[[i, j]]));
            Ok(NdArray::D2(result))
        }
        _ => Err(mlua::Error::external(
            "binary ops on mixed 1D/2D arrays not supported; reshape first",
        )),
    }
}

// ── Aggregation helpers ─────────────────────────────────────────────

/// Aggregate along an optional axis. axis=nil → full, axis=0 → along rows, axis=1 → along cols.
fn aggregate<F>(
    arr: &NdArray,
    axis: Option<i64>,
    full_fn: F,
    axis_name: &str,
) -> Result<NdArray, mlua::Error>
where
    F: Fn(&[f64]) -> f64,
{
    match axis {
        None => {
            // Full aggregation → scalar (1-element 1D array)
            let flat = arr.to_flat_vec();
            if flat.is_empty() {
                return Err(mlua::Error::external(format!(
                    "{} of empty array",
                    axis_name
                )));
            }
            Ok(NdArray::D1(Array1::from_vec(vec![full_fn(&flat)])))
        }
        Some(ax) => {
            let a2 = arr.as_2d();
            let axis_idx = match ax {
                0 => Axis(0), // reduce rows → result has ncols elements
                1 => Axis(1), // reduce cols → result has nrows elements
                _ => return Err(mlua::Error::external("axis must be 0 or 1")),
            };
            let n_out = if ax == 0 { a2.ncols() } else { a2.nrows() };
            let mut result = Vec::with_capacity(n_out);
            for lane in a2.lanes(axis_idx) {
                let vals: Vec<f64> = lane.iter().copied().collect();
                result.push(full_fn(&vals));
            }
            Ok(NdArray::D1(Array1::from_vec(result)))
        }
    }
}

fn sum_slice(s: &[f64]) -> f64 {
    s.iter().sum()
}
fn mean_slice(s: &[f64]) -> f64 {
    s.iter().sum::<f64>() / s.len() as f64
}
fn var_slice(s: &[f64]) -> f64 {
    let m = mean_slice(s);
    s.iter().map(|&x| (x - m).powi(2)).sum::<f64>() / s.len() as f64
}
fn std_slice(s: &[f64]) -> f64 {
    var_slice(s).sqrt()
}
fn min_slice(s: &[f64]) -> f64 {
    s.iter().copied().fold(f64::INFINITY, f64::min)
}
fn max_slice(s: &[f64]) -> f64 {
    s.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

// ── Documentation ───────────────────────────────────────────────────

mod doc;

pub(crate) use doc::NUMPY_DOC;

pub fn register_numpy_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let np = lua.create_table()?;

    register_array_creation(lua, &np)?;

    register_elementwise_ops(lua, &np)?;

    register_aggregation_ops(lua, &np)?;

    register_shape_ops(lua, &np)?;

    register_sorting_ops(lua, &np)?;

    register_conversion_and_matrix_ops(lua, &np)?;

    // ── Help ────────────────────────────────────────────────────

    crate::lua_util::register_help_functions(lua, &np, &NUMPY_DOC)?;

    // ── linalg sub-table (placeholder, populated in register_numpy_linalg) ──
    let linalg = lua.create_table()?;
    np.set("linalg", linalg)?;

    // ── random sub-table (placeholder, populated in register_numpy_random) ──
    let random = lua.create_table()?;
    np.set("random", random)?;

    lua.globals().set("numx", np)?;
    wrap_module_with_help_hints(lua, "numx")?;

    Ok(())
}

// ── Linear algebra via faer ─────────────────────────────────────────

pub fn register_numpy_linalg(lua: &Lua) -> Result<(), mlua::Error> {
    use faer::linalg::solvers::{DenseSolveCore, Solve};

    let np: mlua::Table = lua.globals().get("numx")?;
    let linalg: mlua::Table = np.get("linalg")?;

    // numx.linalg.inv(a) → inverse matrix
    linalg.set(
        "inv",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let m = arr.as_2d();
            let (n, nc) = (m.nrows(), m.ncols());
            if n != nc {
                return Err(mlua::Error::external("inv: matrix must be square"));
            }
            let fm = ndarray_to_faer(&m);
            let lu = fm.as_ref().partial_piv_lu();
            let inv = lu.inverse();
            let result = faer_to_ndarray(inv.as_ref());
            lua.create_userdata(NdArray::D2(result))
        })?,
    )?;

    // numx.linalg.det(a) → determinant (scalar wrapped in 1D array)
    linalg.set(
        "det",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let m = arr.as_2d();
            if m.nrows() != m.ncols() {
                return Err(mlua::Error::external("det: matrix must be square"));
            }
            let fm = ndarray_to_faer(&m);
            let det = fm.as_ref().determinant();
            lua.create_userdata(NdArray::D1(Array1::from_vec(vec![det])))
        })?,
    )?;

    // numx.linalg.solve(a, b) → solve Ax = b
    linalg.set(
        "solve",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            let arr_a = value_to_array(&a)?;
            let arr_b = value_to_array(&b)?;
            let ma = arr_a.as_2d();
            if ma.nrows() != ma.ncols() {
                return Err(mlua::Error::external("solve: A must be square"));
            }
            let fm = ndarray_to_faer(&ma);
            let lu = fm.as_ref().partial_piv_lu();

            match &arr_b {
                NdArray::D1(bv) => {
                    if bv.len() != ma.nrows() {
                        return Err(mlua::Error::external("solve: b length must match A rows"));
                    }
                    let fb = faer::Mat::from_fn(bv.len(), 1, |i, _| bv[i]);
                    let x = lu.solve(&fb);
                    let result: Vec<f64> = (0..x.nrows()).map(|i| x[(i, 0)]).collect();
                    lua.create_userdata(NdArray::D1(Array1::from_vec(result)))
                }
                NdArray::D2(bm) => {
                    if bm.nrows() != ma.nrows() {
                        return Err(mlua::Error::external("solve: b rows must match A rows"));
                    }
                    let fb = ndarray_to_faer(bm);
                    let x = lu.solve(&fb);
                    let result = faer_to_ndarray(x.as_ref());
                    lua.create_userdata(NdArray::D2(result))
                }
            }
        })?,
    )?;

    // numx.linalg.eig(a) → {values, vectors}
    linalg.set(
        "eig",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let m = arr.as_2d();
            if m.nrows() != m.ncols() {
                return Err(mlua::Error::external("eig: matrix must be square"));
            }
            let n = m.nrows();
            let fm = ndarray_to_faer(&m);

            let eig = fm
                .as_ref()
                .eigen_from_real()
                .map_err(|e| mlua::Error::external(format!("eig failed: {:?}", e)))?;

            // Extract real parts of eigenvalues from S diagonal
            let s_diag = eig.S();
            let eigenvalues: Vec<f64> = s_diag.column_vector().iter().map(|c| c.re).collect();

            // Extract real parts of eigenvectors from U
            let u_ref = eig.U();
            let mut vectors = Array2::zeros((n, n));
            for i in 0..n {
                for j in 0..n {
                    vectors[[i, j]] = u_ref[(i, j)].re;
                }
            }

            let result = lua.create_table()?;
            result.set(
                "values",
                lua.create_userdata(NdArray::D1(Array1::from_vec(eigenvalues)))?,
            )?;
            result.set("vectors", lua.create_userdata(NdArray::D2(vectors))?)?;
            Ok(Value::Table(result))
        })?,
    )?;

    // numx.linalg.svd(a) → {u, s, vt}
    linalg.set(
        "svd",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let m = arr.as_2d();
            let fm = ndarray_to_faer(&m);

            let svd = fm
                .as_ref()
                .svd()
                .map_err(|e| mlua::Error::external(format!("svd failed: {:?}", e)))?;

            let u = faer_to_ndarray(svd.U());
            let s_diag = svd.S();
            let s_vals: Vec<f64> = s_diag.column_vector().iter().copied().collect();
            let v_ref = svd.V();
            let vt = faer_to_ndarray(v_ref.transpose());

            let result = lua.create_table()?;
            result.set("u", lua.create_userdata(NdArray::D2(u))?)?;
            result.set(
                "s",
                lua.create_userdata(NdArray::D1(Array1::from_vec(s_vals)))?,
            )?;
            result.set("vt", lua.create_userdata(NdArray::D2(vt))?)?;
            Ok(Value::Table(result))
        })?,
    )?;

    // numx.linalg.norm(a) → Frobenius/L2 norm
    linalg.set(
        "norm",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let flat = arr.to_flat_vec();
            let norm: f64 = flat.iter().map(|x| x * x).sum::<f64>().sqrt();
            lua.create_userdata(NdArray::D1(Array1::from_vec(vec![norm])))
        })?,
    )?;

    Ok(())
}

// ── faer ↔ ndarray conversion helpers ───────────────────────────────

fn ndarray_to_faer(m: &Array2<f64>) -> faer::Mat<f64> {
    let (nrows, ncols) = (m.nrows(), m.ncols());
    faer::Mat::from_fn(nrows, ncols, |i, j| m[[i, j]])
}

fn faer_to_ndarray(m: faer::MatRef<f64>) -> Array2<f64> {
    let (nrows, ncols) = (m.nrows(), m.ncols());
    Array2::from_shape_fn((nrows, ncols), |(i, j)| m[(i, j)])
}

// ── Random number generation ────────────────────────────────────────

pub fn register_numpy_random(lua: &Lua) -> Result<(), mlua::Error> {
    use rand::prelude::*;
    use rand::rngs::StdRng;
    use rand_distr::{Normal, Uniform};

    let np: mlua::Table = lua.globals().get("numx")?;
    let random: mlua::Table = np.get("random")?;

    // Shared RNG state
    let rng = Arc::new(Mutex::new(StdRng::from_os_rng()));

    // numx.random.seed(n)
    {
        let rng = rng.clone();
        random.set(
            "seed",
            lua.create_function(move |_, n: u64| {
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                *r = StdRng::seed_from_u64(n);
                Ok(())
            })?,
        )?;
    }

    // numx.random.rand(shape?) → uniform [0, 1)
    {
        let rng = rng.clone();
        random.set(
            "rand",
            lua.create_function(move |lua, shape: Option<Value>| {
                let s = match shape {
                    Some(v) => parse_shape(&v)?,
                    None => vec![1],
                };
                let total: usize = s.iter().product();
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                let vals: Vec<f64> = (0..total).map(|_| r.random::<f64>()).collect();
                match s.len() {
                    1 => lua.create_userdata(NdArray::D1(Array1::from_vec(vals))),
                    2 => {
                        let arr = Array2::from_shape_vec((s[0], s[1]), vals)
                            .map_err(|e| mlua::Error::external(e.to_string()))?;
                        lua.create_userdata(NdArray::D2(arr))
                    }
                    _ => Err(mlua::Error::external("shape must be 1D or 2D")),
                }
            })?,
        )?;
    }

    // numx.random.randn(shape?) → standard normal
    {
        let rng = rng.clone();
        random.set(
            "randn",
            lua.create_function(move |lua, shape: Option<Value>| {
                let s = match shape {
                    Some(v) => parse_shape(&v)?,
                    None => vec![1],
                };
                let total: usize = s.iter().product();
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                let dist =
                    Normal::new(0.0, 1.0).map_err(|e| mlua::Error::external(e.to_string()))?;
                let vals: Vec<f64> = (0..total).map(|_| r.sample(&dist)).collect();
                match s.len() {
                    1 => lua.create_userdata(NdArray::D1(Array1::from_vec(vals))),
                    2 => {
                        let arr = Array2::from_shape_vec((s[0], s[1]), vals)
                            .map_err(|e| mlua::Error::external(e.to_string()))?;
                        lua.create_userdata(NdArray::D2(arr))
                    }
                    _ => Err(mlua::Error::external("shape must be 1D or 2D")),
                }
            })?,
        )?;
    }

    // numx.random.randint(low, high, shape?)
    {
        let rng = rng.clone();
        random.set(
            "randint",
            lua.create_function(move |lua, (low, high, shape): (i64, i64, Option<Value>)| {
                let s = match shape {
                    Some(v) => parse_shape(&v)?,
                    None => vec![1],
                };
                let total: usize = s.iter().product();
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                let dist = Uniform::new(low, high)
                    .map_err(|e| mlua::Error::external(format!("randint: {}", e)))?;
                let vals: Vec<f64> = (0..total).map(|_| r.sample(&dist) as f64).collect();
                match s.len() {
                    1 => lua.create_userdata(NdArray::D1(Array1::from_vec(vals))),
                    2 => {
                        let arr = Array2::from_shape_vec((s[0], s[1]), vals)
                            .map_err(|e| mlua::Error::external(e.to_string()))?;
                        lua.create_userdata(NdArray::D2(arr))
                    }
                    _ => Err(mlua::Error::external("shape must be 1D or 2D")),
                }
            })?,
        )?;
    }

    // numx.random.choice(a, size?)
    {
        let rng = rng.clone();
        random.set(
            "choice",
            lua.create_function(move |lua, (a, size): (Value, Option<usize>)| {
                let arr = value_to_array(&a)?;
                let flat = arr.to_flat_vec();
                if flat.is_empty() {
                    return Err(mlua::Error::external("choice: array must not be empty"));
                }
                let n = size.unwrap_or(1);
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                let vals: Vec<f64> = (0..n)
                    .map(|_| {
                        let idx = r.random_range(0..flat.len());
                        flat[idx]
                    })
                    .collect();
                lua.create_userdata(NdArray::D1(Array1::from_vec(vals)))
            })?,
        )?;
    }

    // numx.random.normal(mean, std, shape?)
    {
        let rng = rng.clone();
        random.set(
            "normal",
            lua.create_function(
                move |lua, (mean, std_val, shape): (f64, f64, Option<Value>)| {
                    let s = match shape {
                        Some(v) => parse_shape(&v)?,
                        None => vec![1],
                    };
                    let total: usize = s.iter().product();
                    let mut r = rng
                        .lock()
                        .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                    let dist = Normal::new(mean, std_val)
                        .map_err(|e| mlua::Error::external(e.to_string()))?;
                    let vals: Vec<f64> = (0..total).map(|_| r.sample(&dist)).collect();
                    match s.len() {
                        1 => lua.create_userdata(NdArray::D1(Array1::from_vec(vals))),
                        2 => {
                            let arr = Array2::from_shape_vec((s[0], s[1]), vals)
                                .map_err(|e| mlua::Error::external(e.to_string()))?;
                            lua.create_userdata(NdArray::D2(arr))
                        }
                        _ => Err(mlua::Error::external("shape must be 1D or 2D")),
                    }
                },
            )?,
        )?;
    }

    // numx.random.uniform(low, high, shape?)
    {
        let rng = rng.clone();
        random.set(
            "uniform",
            lua.create_function(move |lua, (low, high, shape): (f64, f64, Option<Value>)| {
                let s = match shape {
                    Some(v) => parse_shape(&v)?,
                    None => vec![1],
                };
                let total: usize = s.iter().product();
                let mut r = rng
                    .lock()
                    .map_err(|e| mlua::Error::external(format!("RNG lock poisoned: {}", e)))?;
                let dist = Uniform::new(low, high)
                    .map_err(|e| mlua::Error::external(format!("uniform: {}", e)))?;
                let vals: Vec<f64> = (0..total).map(|_| r.sample(&dist)).collect();
                match s.len() {
                    1 => lua.create_userdata(NdArray::D1(Array1::from_vec(vals))),
                    2 => {
                        let arr = Array2::from_shape_vec((s[0], s[1]), vals)
                            .map_err(|e| mlua::Error::external(e.to_string()))?;
                        lua.create_userdata(NdArray::D2(arr))
                    }
                    _ => Err(mlua::Error::external("shape must be 1D or 2D")),
                }
            })?,
        )?;
    }

    Ok(())
}

fn register_array_creation(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Array creation ──────────────────────────────────────────

    // numx.array(data) → NdArray
    np.set(
        "array",
        lua.create_function(|lua, data: Value| {
            let arr = match &data {
                Value::Table(t) => table_to_array(t)?,
                _ => return Err(mlua::Error::external("numx.array expects a table")),
            };
            lua.create_userdata(arr)
        })?,
    )?;

    // numx.zeros(shape)
    np.set(
        "zeros",
        lua.create_function(|lua, shape: Value| {
            let s = parse_shape(&shape)?;
            lua.create_userdata(make_filled(&s, 0.0)?)
        })?,
    )?;

    // numx.ones(shape)
    np.set(
        "ones",
        lua.create_function(|lua, shape: Value| {
            let s = parse_shape(&shape)?;
            lua.create_userdata(make_filled(&s, 1.0)?)
        })?,
    )?;

    // numx.full(shape, val)
    np.set(
        "full",
        lua.create_function(|lua, (shape, val): (Value, f64)| {
            let s = parse_shape(&shape)?;
            lua.create_userdata(make_filled(&s, val)?)
        })?,
    )?;

    // numx.arange(start, stop, step?)
    np.set(
        "arange",
        lua.create_function(|lua, (start, stop, step): (f64, f64, Option<f64>)| {
            let step = step.unwrap_or(1.0);
            if step == 0.0 {
                return Err(mlua::Error::external("step cannot be zero"));
            }
            let mut vals = Vec::new();
            let mut v = start;
            if step > 0.0 {
                while v < stop {
                    vals.push(v);
                    v += step;
                }
            } else {
                while v > stop {
                    vals.push(v);
                    v += step;
                }
            }
            lua.create_userdata(NdArray::D1(Array1::from_vec(vals)))
        })?,
    )?;

    // numx.linspace(start, stop, count)
    np.set(
        "linspace",
        lua.create_function(|lua, (start, stop, count): (f64, f64, usize)| {
            if count == 0 {
                return lua.create_userdata(NdArray::D1(Array1::zeros(0)));
            }
            if count == 1 {
                return lua.create_userdata(NdArray::D1(Array1::from_vec(vec![start])));
            }
            let step = (stop - start) / (count - 1) as f64;
            let vals: Vec<f64> = (0..count).map(|i| start + step * i as f64).collect();
            lua.create_userdata(NdArray::D1(Array1::from_vec(vals)))
        })?,
    )?;

    // numx.eye(n)
    np.set(
        "eye",
        lua.create_function(|lua, n: usize| {
            let mut arr = Array2::zeros((n, n));
            for i in 0..n {
                arr[[i, i]] = 1.0;
            }
            lua.create_userdata(NdArray::D2(arr))
        })?,
    )?;

    // numx.diag(values)
    np.set(
        "diag",
        lua.create_function(|lua, data: Value| {
            let vals = match &data {
                Value::UserData(ud) => {
                    let arr = ud.borrow::<NdArray>()?;
                    match &*arr {
                        NdArray::D1(a) => a.to_vec(),
                        NdArray::D2(a) => {
                            // If 2D, extract diagonal
                            let n = a.nrows().min(a.ncols());
                            (0..n).map(|i| a[[i, i]]).collect()
                        }
                    }
                }
                Value::Table(t) => {
                    let t = unwrap_py_table(&t)?;
                    let len = t.raw_len();
                    let mut v = Vec::with_capacity(len);
                    for i in 1..=len {
                        let val: Value = t.get(i)?;
                        v.push(lua_num(&val)?);
                    }
                    v
                }
                _ => return Err(mlua::Error::external("numx.diag expects array or table")),
            };
            // If input was 1D, create diagonal matrix
            if let Value::UserData(ud) = &data {
                let arr = ud.borrow::<NdArray>()?;
                if matches!(&*arr, NdArray::D2(_)) {
                    // Extracted diagonal → return as 1D
                    return lua.create_userdata(NdArray::D1(Array1::from_vec(vals)));
                }
            }
            let n = vals.len();
            let mut arr = Array2::zeros((n, n));
            for (i, &v) in vals.iter().enumerate() {
                arr[[i, i]] = v;
            }
            lua.create_userdata(NdArray::D2(arr))
        })?,
    )?;

    Ok(())
}

fn register_elementwise_ops(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Element-wise ops ────────────────────────────────────────

    np.set(
        "add",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            lua.create_userdata(binop(&a, &b, |x, y| x + y)?)
        })?,
    )?;

    np.set(
        "sub",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            lua.create_userdata(binop(&a, &b, |x, y| x - y)?)
        })?,
    )?;

    np.set(
        "mul",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            lua.create_userdata(binop(&a, &b, |x, y| x * y)?)
        })?,
    )?;

    np.set(
        "div",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            lua.create_userdata(binop(&a, &b, |x, y| x / y)?)
        })?,
    )?;

    np.set(
        "pow",
        lua.create_function(|lua, (a, exp): (Value, Value)| {
            lua.create_userdata(binop(&a, &exp, |x, e| x.powf(e))?)
        })?,
    )?;

    // Unary ops
    np.set(
        "abs",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.abs()))
        })?,
    )?;

    np.set(
        "sqrt",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.sqrt()))
        })?,
    )?;

    np.set(
        "log",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.ln()))
        })?,
    )?;

    np.set(
        "exp",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.exp()))
        })?,
    )?;

    np.set(
        "sin",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.sin()))
        })?,
    )?;

    np.set(
        "cos",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.cos()))
        })?,
    )?;

    np.set(
        "tan",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.tan()))
        })?,
    )?;

    np.set(
        "floor",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.floor()))
        })?,
    )?;

    np.set(
        "ceil",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(arr.map_elem(|x| x.ceil()))
        })?,
    )?;

    np.set(
        "round",
        lua.create_function(|lua, (a, decimals): (Value, Option<i32>)| {
            let arr = value_to_array(&a)?;
            let d = decimals.unwrap_or(0);
            let factor = 10f64.powi(d);
            lua.create_userdata(arr.map_elem(|x| (x * factor).round() / factor))
        })?,
    )?;

    Ok(())
}

fn register_aggregation_ops(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Aggregation ─────────────────────────────────────────────

    np.set(
        "sum",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, sum_slice, "sum")?)
        })?,
    )?;

    np.set(
        "mean",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, mean_slice, "mean")?)
        })?,
    )?;

    np.set(
        "std",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, std_slice, "std")?)
        })?,
    )?;

    np.set(
        "var",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, var_slice, "var")?)
        })?,
    )?;

    np.set(
        "min",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, min_slice, "min")?)
        })?,
    )?;

    np.set(
        "max",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(aggregate(&arr, axis, max_slice, "max")?)
        })?,
    )?;

    np.set(
        "argmin",
        lua.create_function(|_, a: Value| {
            let arr = value_to_array(&a)?;
            let flat = arr.to_flat_vec();
            if flat.is_empty() {
                return Err(mlua::Error::external("argmin of empty array"));
            }
            let idx = flat
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap()
                .0;
            Ok(idx + 1) // 1-based for Lua
        })?,
    )?;

    np.set(
        "argmax",
        lua.create_function(|_, a: Value| {
            let arr = value_to_array(&a)?;
            let flat = arr.to_flat_vec();
            if flat.is_empty() {
                return Err(mlua::Error::external("argmax of empty array"));
            }
            let idx = flat
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap()
                .0;
            Ok(idx + 1) // 1-based for Lua
        })?,
    )?;

    np.set(
        "cumsum",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            match axis {
                None => {
                    let flat = arr.to_flat_vec();
                    let mut cumulative = Vec::with_capacity(flat.len());
                    let mut s = 0.0;
                    for v in &flat {
                        s += v;
                        cumulative.push(s);
                    }
                    lua.create_userdata(NdArray::D1(Array1::from_vec(cumulative)))
                }
                Some(ax) => {
                    let a2 = arr.as_2d();
                    let (nrows, ncols) = (a2.nrows(), a2.ncols());
                    let mut result = a2.clone();
                    match ax {
                        0 => {
                            for j in 0..ncols {
                                for i in 1..nrows {
                                    result[[i, j]] += result[[i - 1, j]];
                                }
                            }
                        }
                        1 => {
                            for i in 0..nrows {
                                for j in 1..ncols {
                                    result[[i, j]] += result[[i, j - 1]];
                                }
                            }
                        }
                        _ => return Err(mlua::Error::external("axis must be 0 or 1")),
                    }
                    lua.create_userdata(NdArray::D2(result))
                }
            }
        })?,
    )?;

    Ok(())
}

fn register_shape_ops(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Shape ops ───────────────────────────────────────────────

    np.set(
        "shape",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let s = arr.shape_vec();
            let t = lua.create_table()?;
            for (i, &d) in s.iter().enumerate() {
                t.set(i + 1, d)?;
            }
            Ok(Value::Table(t))
        })?,
    )?;

    np.set(
        "reshape",
        lua.create_function(|lua, (a, shape): (Value, Value)| {
            let arr = value_to_array(&a)?;
            let s = parse_shape(&shape)?;
            let flat = arr.to_flat_vec();
            let total: usize = s.iter().product();
            if total != flat.len() {
                return Err(mlua::Error::external(format!(
                    "cannot reshape array of size {} into shape {:?}",
                    flat.len(),
                    s
                )));
            }
            let result = match s.len() {
                1 => NdArray::D1(Array1::from_vec(flat)),
                2 => NdArray::D2(
                    Array2::from_shape_vec((s[0], s[1]), flat)
                        .map_err(|e| mlua::Error::external(e.to_string()))?,
                ),
                _ => return Err(mlua::Error::external("shape must be 1D or 2D")),
            };
            lua.create_userdata(result)
        })?,
    )?;

    np.set(
        "transpose",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            match arr {
                NdArray::D1(a) => lua.create_userdata(NdArray::D1(a)), // no-op for 1D
                NdArray::D2(a) => lua.create_userdata(NdArray::D2(a.t().to_owned())),
            }
        })?,
    )?;

    np.set(
        "flatten",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            lua.create_userdata(NdArray::D1(Array1::from_vec(arr.to_flat_vec())))
        })?,
    )?;

    np.set(
        "concatenate",
        lua.create_function(|lua, (arrays, axis): (mlua::Table, Option<i64>)| {
            let axis = axis.unwrap_or(0);
            let arrays = unwrap_py_table(&arrays)?;
            let len = arrays.raw_len();
            if len == 0 {
                return lua.create_userdata(NdArray::D1(Array1::zeros(0)));
            }

            let mut arrs: Vec<NdArray> = Vec::with_capacity(len);
            for i in 1..=len {
                let v: Value = arrays.get(i)?;
                arrs.push(value_to_array(&v)?);
            }

            // All 1D → concatenate into 1D
            let all_1d = arrs.iter().all(|a| matches!(a, NdArray::D1(_)));
            if all_1d && axis == 0 {
                let mut flat = Vec::new();
                for a in &arrs {
                    flat.extend(a.to_flat_vec());
                }
                return lua.create_userdata(NdArray::D1(Array1::from_vec(flat)));
            }

            // 2D concatenation
            let mats: Vec<Array2<f64>> = arrs.iter().map(|a| a.as_2d()).collect();
            match axis {
                0 => {
                    // Stack vertically
                    let ncols = mats[0].ncols();
                    let mut rows = Vec::new();
                    for m in &mats {
                        if m.ncols() != ncols {
                            return Err(mlua::Error::external(
                                "all arrays must have same ncols for axis=0 concat",
                            ));
                        }
                        rows.extend(m.iter().copied());
                    }
                    let nrows: usize = mats.iter().map(|m| m.nrows()).sum();
                    let result = Array2::from_shape_vec((nrows, ncols), rows)
                        .map_err(|e| mlua::Error::external(e.to_string()))?;
                    lua.create_userdata(NdArray::D2(result))
                }
                1 => {
                    // Stack horizontally
                    let nrows = mats[0].nrows();
                    for m in &mats {
                        if m.nrows() != nrows {
                            return Err(mlua::Error::external(
                                "all arrays must have same nrows for axis=1 concat",
                            ));
                        }
                    }
                    let total_cols: usize = mats.iter().map(|m| m.ncols()).sum();
                    let mut result = Array2::zeros((nrows, total_cols));
                    let mut col_offset = 0;
                    for m in &mats {
                        for i in 0..nrows {
                            for j in 0..m.ncols() {
                                result[[i, col_offset + j]] = m[[i, j]];
                            }
                        }
                        col_offset += m.ncols();
                    }
                    lua.create_userdata(NdArray::D2(result))
                }
                _ => Err(mlua::Error::external("axis must be 0 or 1")),
            }
        })?,
    )?;

    np.set(
        "stack",
        lua.create_function(|lua, (arrays, axis): (mlua::Table, Option<i64>)| {
            let axis = axis.unwrap_or(0);
            let arrays = unwrap_py_table(&arrays)?;
            let len = arrays.raw_len();
            if len == 0 {
                return lua.create_userdata(NdArray::D1(Array1::zeros(0)));
            }

            let mut arrs: Vec<NdArray> = Vec::with_capacity(len);
            for i in 1..=len {
                let v: Value = arrays.get(i)?;
                arrs.push(value_to_array(&v)?);
            }

            // All must be 1D with same length for stack
            let vecs: Vec<Vec<f64>> = arrs.iter().map(|a| a.to_flat_vec()).collect();
            let elem_len = vecs[0].len();
            for v in &vecs {
                if v.len() != elem_len {
                    return Err(mlua::Error::external(
                        "all arrays must have same shape for stack",
                    ));
                }
            }

            match axis {
                0 => {
                    // Stack as rows → (n_arrays, elem_len) matrix
                    let mut data = Vec::with_capacity(len * elem_len);
                    for v in &vecs {
                        data.extend(v);
                    }
                    let result = Array2::from_shape_vec((len, elem_len), data)
                        .map_err(|e| mlua::Error::external(e.to_string()))?;
                    lua.create_userdata(NdArray::D2(result))
                }
                1 => {
                    // Stack as columns → (elem_len, n_arrays) matrix
                    let mut result = Array2::zeros((elem_len, len));
                    for (j, v) in vecs.iter().enumerate() {
                        for (i, &val) in v.iter().enumerate() {
                            result[[i, j]] = val;
                        }
                    }
                    lua.create_userdata(NdArray::D2(result))
                }
                _ => Err(mlua::Error::external("axis must be 0 or 1")),
            }
        })?,
    )?;

    Ok(())
}

fn register_sorting_ops(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Sorting/search ──────────────────────────────────────────

    np.set(
        "sort",
        lua.create_function(|lua, (a, axis): (Value, Option<i64>)| {
            let arr = value_to_array(&a)?;
            match axis {
                None | Some(0) => {
                    match arr {
                        NdArray::D1(a) => {
                            let mut v = a.to_vec();
                            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            lua.create_userdata(NdArray::D1(Array1::from_vec(v)))
                        }
                        NdArray::D2(a) => {
                            // Sort each column (axis=0) or each row
                            let axis_val = axis.unwrap_or(0);
                            let (nrows, ncols) = (a.nrows(), a.ncols());
                            let mut result = a.clone();
                            if axis_val == 0 {
                                for j in 0..ncols {
                                    let mut col: Vec<f64> = (0..nrows).map(|i| a[[i, j]]).collect();
                                    col.sort_by(|a, b| {
                                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    for (i, &v) in col.iter().enumerate() {
                                        result[[i, j]] = v;
                                    }
                                }
                            } else {
                                for i in 0..nrows {
                                    let mut row: Vec<f64> = (0..ncols).map(|j| a[[i, j]]).collect();
                                    row.sort_by(|a, b| {
                                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    for (j, &v) in row.iter().enumerate() {
                                        result[[i, j]] = v;
                                    }
                                }
                            }
                            lua.create_userdata(NdArray::D2(result))
                        }
                    }
                }
                Some(1) => {
                    let a2 = arr.as_2d();
                    let (nrows, ncols) = (a2.nrows(), a2.ncols());
                    let mut result = a2.clone();
                    for i in 0..nrows {
                        let mut row: Vec<f64> = (0..ncols).map(|j| a2[[i, j]]).collect();
                        row.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        for (j, &v) in row.iter().enumerate() {
                            result[[i, j]] = v;
                        }
                    }
                    lua.create_userdata(NdArray::D2(result))
                }
                _ => Err(mlua::Error::external("axis must be 0 or 1")),
            }
        })?,
    )?;

    np.set(
        "argsort",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let flat = arr.to_flat_vec();
            let mut indices: Vec<usize> = (0..flat.len()).collect();
            indices.sort_by(|&a, &b| {
                flat[a]
                    .partial_cmp(&flat[b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // 1-based for Lua
            let result: Vec<f64> = indices.iter().map(|&i| (i + 1) as f64).collect();
            lua.create_userdata(NdArray::D1(Array1::from_vec(result)))
        })?,
    )?;

    // numx.where_(cond, x, y) — named where_ to avoid Lua keyword issues
    // Also register as "where" for Luau access
    let where_fn = lua.create_function(|lua, (cond, x, y): (Value, Value, Value)| {
        let c = value_to_array(&cond)?;
        let xv = value_to_array(&x)?;
        let yv = value_to_array(&y)?;
        let cf = c.to_flat_vec();
        let xf = xv.to_flat_vec();
        let yf = yv.to_flat_vec();
        if cf.len() != xf.len() || cf.len() != yf.len() {
            return Err(mlua::Error::external(
                "where: all arrays must have same length",
            ));
        }
        let result: Vec<f64> = cf
            .iter()
            .zip(xf.iter().zip(yf.iter()))
            .map(|(&c, (&x, &y))| if c != 0.0 { x } else { y })
            .collect();
        // Preserve shape from cond
        match c {
            NdArray::D1(_) => lua.create_userdata(NdArray::D1(Array1::from_vec(result))),
            NdArray::D2(ref a) => {
                let arr = Array2::from_shape_vec(a.dim(), result)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                lua.create_userdata(NdArray::D2(arr))
            }
        }
    })?;
    np.set("where_", where_fn.clone())?;
    // Luau doesn't reserve "where", so register it directly too
    np.set("where", where_fn)?;

    np.set(
        "unique",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            let mut flat = arr.to_flat_vec();
            flat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            flat.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
            lua.create_userdata(NdArray::D1(Array1::from_vec(flat)))
        })?,
    )?;

    Ok(())
}

fn register_conversion_and_matrix_ops(lua: &Lua, np: &mlua::Table) -> Result<(), mlua::Error> {
    // ── Conversion ──────────────────────────────────────────────

    np.set(
        "tolist",
        lua.create_function(|lua, a: Value| {
            let arr = value_to_array(&a)?;
            array_to_lua(lua, &arr)
        })?,
    )?;

    // ── Dot product / matmul (basic, faer-backed linalg added in 1b) ──

    np.set(
        "dot",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            let arr_a = value_to_array(&a)?;
            let arr_b = value_to_array(&b)?;
            match (&arr_a, &arr_b) {
                (NdArray::D1(a), NdArray::D1(b)) => {
                    if a.len() != b.len() {
                        return Err(mlua::Error::external(
                            "dot: 1D arrays must have same length",
                        ));
                    }
                    let result: f64 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
                    lua.create_userdata(NdArray::D1(Array1::from_vec(vec![result])))
                }
                (NdArray::D2(a), NdArray::D2(b)) => {
                    if a.ncols() != b.nrows() {
                        return Err(mlua::Error::external(format!(
                            "matmul: ({},{}) × ({},{}) — inner dimensions must match",
                            a.nrows(),
                            a.ncols(),
                            b.nrows(),
                            b.ncols()
                        )));
                    }
                    let result = a.dot(b);
                    lua.create_userdata(NdArray::D2(result))
                }
                (NdArray::D2(a), NdArray::D1(b)) => {
                    if a.ncols() != b.len() {
                        return Err(mlua::Error::external(
                            "dot: matrix cols must match vector length",
                        ));
                    }
                    let result = a.dot(b);
                    lua.create_userdata(NdArray::D1(result))
                }
                (NdArray::D1(a), NdArray::D2(b)) => {
                    if a.len() != b.nrows() {
                        return Err(mlua::Error::external(
                            "dot: vector length must match matrix rows",
                        ));
                    }
                    let result = b.t().dot(a);
                    lua.create_userdata(NdArray::D1(result))
                }
            }
        })?,
    )?;

    np.set(
        "matmul",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            let arr_a = value_to_array(&a)?;
            let arr_b = value_to_array(&b)?;
            let ma = arr_a.as_2d();
            let mb = arr_b.as_2d();
            if ma.ncols() != mb.nrows() {
                return Err(mlua::Error::external(format!(
                    "matmul: ({},{}) × ({},{}) — inner dimensions must match",
                    ma.nrows(),
                    ma.ncols(),
                    mb.nrows(),
                    mb.ncols()
                )));
            }
            let result = ma.dot(&mb);
            lua.create_userdata(NdArray::D2(result))
        })?,
    )?;

    Ok(())
}
