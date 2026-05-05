//! Plot module for the Luau sandbox.
//!
//! Exposes `plot.line`, `plot.bar`, `plot.scatter`, `plot.histogram`,
//! `plot.pie`, `plot.heatmap`, `plot.multi`, `plot.figure`, `plot.radar`,
//! `plot.candlestick`, `plot.box`, `plot.ohlc`, `plot.contour`,
//! `plot.table`, `plot.scatter3d`, `plot.surface`, and `plot.sankey`
//! as globals.
//!
//! All rendering is done by the `plotly` Rust crate (interactive HTML output).

use crate::mount::MountTable;
use crate::pyrt_compat::unwrap_py_seq;
use mlua::Value;
use plotly::box_plot::{BoxMean, BoxPoints};
use plotly::common::{Fill, Line, LineShape, Marker, Mode, Orientation, Title};
use plotly::layout::themes::BuiltinTheme;
use plotly::layout::{Annotation, Axis, LayoutPolar, RadialAxis};
use plotly::sankey::{Link, Node};
use plotly::traces::table::{Cells, Header};
use plotly::{
    Bar, BoxPlot, Candlestick, Contour, HeatMap, Histogram, Layout, Ohlc, Pie, Plot, Sankey,
    Scatter, Scatter3D, ScatterPolar, Surface, Table, Trace,
};

mod doc;
mod register;

#[cfg(test)]
pub(crate) use doc::PLOT_DOC;
pub(crate) use register::register_plot_globals;

struct ChartOpts {
    title: Option<String>,
    xlabel: Option<String>,
    ylabel: Option<String>,
    width: usize,
    height: usize,
    output: String,
    colors: Vec<String>,
    legend: bool,
    grid: bool,
    theme: String,
    smooth: bool,
    fill: bool,
}

fn extract_chart_opts(opts: &Option<mlua::Table>, fn_name: &str) -> Result<ChartOpts, mlua::Error> {
    let output = opts
        .as_ref()
        .and_then(|t| {
            t.get::<String>("output")
                .or_else(|_| t.get::<String>("o"))
                .ok()
        })
        .ok_or_else(|| {
            mlua::Error::external(format!(
                "{fn_name}: output path is required (e.g., {{output = '/artifacts/chart.html'}})"
            ))
        })?;
    let mut co = ChartOpts {
        title: None,
        xlabel: None,
        ylabel: None,
        width: 800,
        height: 600,
        output,
        colors: Vec::new(),
        legend: false,
        grid: true,
        theme: "plotly_white".to_string(),
        smooth: false,
        fill: false,
    };
    if let Some(t) = opts {
        if let Ok(v) = t.get::<String>("title").or_else(|_| t.get::<String>("t")) {
            co.title = Some(v);
        }
        if let Ok(v) = t.get::<String>("xlabel").or_else(|_| t.get::<String>("xl")) {
            co.xlabel = Some(v);
        }
        if let Ok(v) = t.get::<String>("ylabel").or_else(|_| t.get::<String>("yl")) {
            co.ylabel = Some(v);
        }
        if let Ok(v) = t.get::<i32>("width").or_else(|_| t.get::<i32>("w")) {
            co.width = v.max(100) as usize;
        }
        if let Ok(v) = t.get::<i32>("height").or_else(|_| t.get::<i32>("h")) {
            co.height = v.max(100) as usize;
        }
        if let Ok(v) = t.get::<bool>("legend").or_else(|_| t.get::<bool>("l")) {
            co.legend = v;
        }
        if let Ok(v) = t.get::<bool>("grid").or_else(|_| t.get::<bool>("g")) {
            co.grid = v;
        }
        if let Ok(tbl) = t.get::<mlua::Table>("colors") {
            let tbl = unwrap_py_seq(&tbl)?;
            for i in 1..=tbl.raw_len() {
                if let Ok(s) = tbl.get::<String>(i) {
                    co.colors.push(s);
                }
            }
        }
        if let Ok(v) = t.get::<String>("theme") {
            co.theme = v;
        }
        if let Ok(v) = t.get::<bool>("smooth") {
            co.smooth = v;
        }
        if let Ok(v) = t.get::<bool>("fill") {
            co.fill = v;
        }
    }
    Ok(co)
}

/// Check if a table has an "output" or "o" key (signals table-form calling convention).
fn has_output_key(t: &mlua::Table) -> bool {
    t.get::<String>("output").is_ok() || t.get::<String>("o").is_ok()
}

// ── Theme mapping ──────────────────────────────────────────────

fn resolve_theme(name: &str) -> BuiltinTheme {
    match name {
        "ant" | "plotly_white" | "white" => BuiltinTheme::PlotlyWhite,
        "grafana" | "plotly_dark" | "dark" => BuiltinTheme::PlotlyDark,
        "seaborn" => BuiltinTheme::Seaborn,
        "seaborn_whitegrid" => BuiltinTheme::SeabornWhitegrid,
        "seaborn_dark" => BuiltinTheme::SeabornDark,
        "matplotlib" => BuiltinTheme::Matplotlib,
        "plotnine" | "ggplot2" => BuiltinTheme::Plotnine,
        _ => BuiltinTheme::PlotlyWhite,
    }
}

// ── Layout builder ─────────────────────────────────────────────

fn build_layout(opts: &ChartOpts) -> Layout {
    let theme = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(theme.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut x_axis = Axis::new();
    if let Some(ref xl) = opts.xlabel {
        x_axis = x_axis.title(Title::with_text(xl));
    }
    if !opts.grid {
        x_axis = x_axis.show_grid(false);
    }
    layout = layout.x_axis(x_axis);

    let mut y_axis = Axis::new();
    if let Some(ref yl) = opts.ylabel {
        y_axis = y_axis.title(Title::with_text(yl));
    }
    if !opts.grid {
        y_axis = y_axis.show_grid(false);
    }
    layout = layout.y_axis(y_axis);

    if !opts.legend {
        layout = layout.show_legend(false);
    }

    if !opts.colors.is_empty() {
        layout = layout.colorway(opts.colors.clone());
    }

    layout
}

fn plot_to_html(plot: &Plot) -> String {
    plot.to_html()
}

// ── Lua table → Vec helpers ───────────────────────────────────

fn lua_table_to_f64_vec(val: &Value) -> Result<Vec<f64>, mlua::Error> {
    match val {
        Value::Table(t) => {
            let t = unwrap_py_seq(t)?;
            let len = t.raw_len();
            let mut v = Vec::with_capacity(len);
            for i in 1..=len {
                let n: f64 = t.get(i).map_err(|_| {
                    mlua::Error::external(format!("expected number at index {}", i))
                })?;
                v.push(n);
            }
            Ok(v)
        }
        _ => Err(mlua::Error::external("expected table of numbers")),
    }
}

fn lua_table_to_string_vec(val: &Value) -> Result<Vec<String>, mlua::Error> {
    match val {
        Value::Table(t) => {
            let t = unwrap_py_seq(t)?;
            let len = t.raw_len();
            let mut v = Vec::with_capacity(len);
            for i in 1..=len {
                let s: String = t.get(i).map_err(|_| {
                    mlua::Error::external(format!("expected string at index {}", i))
                })?;
                v.push(s);
            }
            Ok(v)
        }
        _ => Err(mlua::Error::external("expected table of strings")),
    }
}

fn lua_table_to_f64_matrix(val: &Value) -> Result<Vec<Vec<f64>>, mlua::Error> {
    match val {
        Value::Table(t) => {
            let t = unwrap_py_seq(t)?;
            let len = t.raw_len();
            let mut rows = Vec::with_capacity(len);
            for i in 1..=len {
                let row: Value = t.get(i)?;
                rows.push(lua_table_to_f64_vec(&row)?);
            }
            Ok(rows)
        }
        _ => Err(mlua::Error::external("expected table of tables (matrix)")),
    }
}

fn lua_table_to_string_matrix(val: &Value) -> Result<Vec<Vec<String>>, mlua::Error> {
    match val {
        Value::Table(t) => {
            let t = unwrap_py_seq(t)?;
            let len = t.raw_len();
            let mut rows = Vec::with_capacity(len);
            for i in 1..=len {
                let row: Value = t.get(i)?;
                rows.push(lua_table_to_string_vec(&row)?);
            }
            Ok(rows)
        }
        _ => Err(mlua::Error::external(
            "expected table of tables (string matrix)",
        )),
    }
}

// ── Helpers ────────────────────────────────────────────────────

/// Parse Sankey links from Lua table. Each link is {source=N, target=N, value=N}.
/// Source/target are 1-based Lua indices, converted to 0-based for Plotly.
fn parse_sankey_links(
    links_tbl: &mlua::Table,
) -> Result<(Vec<usize>, Vec<usize>, Vec<f64>), mlua::Error> {
    let mut sources = Vec::new();
    let mut targets = Vec::new();
    let mut values = Vec::new();
    for i in 1..=links_tbl.raw_len() {
        let link: mlua::Table = links_tbl.get(i)?;
        let src: usize = link.get::<usize>("source").map_err(|_| {
            mlua::Error::external(format!("plot.sankey: links[{}] missing 'source'", i))
        })?;
        let tgt: usize = link.get::<usize>("target").map_err(|_| {
            mlua::Error::external(format!("plot.sankey: links[{}] missing 'target'", i))
        })?;
        let val: f64 = link.get::<f64>("value").map_err(|_| {
            mlua::Error::external(format!("plot.sankey: links[{}] missing 'value'", i))
        })?;
        // Convert 1-based Lua indices to 0-based for Plotly
        if src == 0 {
            return Err(mlua::Error::external(format!(
                "plot.sankey: links[{}] 'source' must be >= 1 (got 0)",
                i
            )));
        }
        if tgt == 0 {
            return Err(mlua::Error::external(format!(
                "plot.sankey: links[{}] 'target' must be >= 1 (got 0)",
                i
            )));
        }
        sources.push(src - 1);
        targets.push(tgt - 1);
        values.push(val);
    }
    Ok((sources, targets, values))
}

fn ensure_output_dir(mounts: &MountTable, path: &str) -> Result<std::path::PathBuf, mlua::Error> {
    mounts
        .resolve_write_deep(path)
        .map_err(mlua::Error::external)
}

// ── Rendering functions ────────────────────────────────────────

fn render_line(x: &[f64], y: &[f64], opts: &ChartOpts) -> String {
    let mut trace = Scatter::new(x.to_vec(), y.to_vec())
        .mode(Mode::Lines)
        .show_legend(false);

    if opts.smooth {
        trace = trace.line(Line::new().shape(LineShape::Spline));
    }
    if opts.fill {
        trace = trace.fill(Fill::ToZeroY);
    }
    if let Some(c) = opts.colors.first() {
        trace = trace.line(if opts.smooth {
            Line::new().shape(LineShape::Spline).color(c.clone())
        } else {
            Line::new().color(c.clone())
        });
    }

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

fn render_bar(labels: &[String], values: &[f64], opts: &ChartOpts, horizontal: bool) -> String {
    let layout = build_layout(opts);
    let mut plot = Plot::new();

    if horizontal {
        let mut trace = Bar::new(values.to_vec(), labels.to_vec())
            .orientation(Orientation::Horizontal)
            .show_legend(false);
        if let Some(c) = opts.colors.first() {
            trace = trace.marker(Marker::new().color(c.clone()));
        }
        plot.add_trace(trace);
    } else {
        let mut trace = Bar::new(labels.to_vec(), values.to_vec()).show_legend(false);
        if let Some(c) = opts.colors.first() {
            trace = trace.marker(Marker::new().color(c.clone()));
        }
        plot.add_trace(trace);
    }

    plot.set_layout(layout);
    plot_to_html(&plot)
}

fn render_scatter(x: &[f64], y: &[f64], opts: &ChartOpts, point_size: usize) -> String {
    let mut trace = Scatter::new(x.to_vec(), y.to_vec())
        .mode(Mode::Markers)
        .show_legend(false);

    let mut marker = Marker::new().size(point_size);
    if let Some(c) = opts.colors.first() {
        marker = marker.color(c.clone());
    }
    trace = trace.marker(marker);

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

fn render_histogram(data: &[f64], opts: &ChartOpts, bins: Option<usize>) -> String {
    let mut trace = Histogram::new(data.to_vec()).show_legend(false);

    if let Some(n) = bins {
        trace = trace.n_bins_x(n);
    }

    if let Some(c) = opts.colors.first() {
        trace = trace.marker(Marker::new().color(c.clone()));
    }

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

fn render_pie(labels: &[String], values: &[f64], opts: &ChartOpts, hole: Option<f64>) -> String {
    let mut trace = Pie::new(values.to_vec()).labels(labels.to_vec());

    if let Some(h) = hole {
        trace = trace.hole(h);
    }

    if !opts.colors.is_empty() {
        trace = trace.marker(Marker::new().colors(opts.colors.clone()));
    }

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

fn render_heatmap(
    matrix: &[Vec<f64>],
    opts: &ChartOpts,
    xlabels: &Option<Vec<String>>,
    ylabels: &Option<Vec<String>>,
) -> String {
    let z: Vec<Vec<f64>> = matrix.to_vec();

    let trace = if let (Some(xl), Some(yl)) = (xlabels, ylabels) {
        HeatMap::new(xl.clone(), yl.clone(), z)
    } else if let Some(xl) = xlabels {
        let yl: Vec<String> = (0..matrix.len()).map(|i| i.to_string()).collect();
        HeatMap::new(xl.clone(), yl, z)
    } else if let Some(yl) = ylabels {
        let ncols = matrix.first().map(|r| r.len()).unwrap_or(0);
        let xl: Vec<String> = (0..ncols).map(|i| i.to_string()).collect();
        HeatMap::new(xl, yl.clone(), z)
    } else {
        let ncols = matrix.first().map(|r| r.len()).unwrap_or(0);
        let xl: Vec<String> = (0..ncols).map(|i| i.to_string()).collect();
        let yl: Vec<String> = (0..matrix.len()).map(|i| i.to_string()).collect();
        HeatMap::new(xl, yl, z)
    };

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Multi-series ───────────────────────────────────────────────

struct SeriesData {
    x: Vec<f64>,
    y: Vec<f64>,
    series_type: String,
    label: Option<String>,
}

fn render_multi(series_list: &[SeriesData], opts: &ChartOpts) -> String {
    let mut plot = Plot::new();

    for (i, s) in series_list.iter().enumerate() {
        let name = s
            .label
            .clone()
            .unwrap_or_else(|| format!("Series {}", i + 1));
        let trace: Box<dyn Trace> = match s.series_type.as_str() {
            "scatter" => {
                let mut t = Scatter::new(s.x.clone(), s.y.clone())
                    .mode(Mode::Markers)
                    .name(name);
                if let Some(c) = opts.colors.get(i) {
                    t = t.marker(Marker::new().color(c.clone()));
                }
                t
            }
            _ => {
                // "line" or default
                let mut t = Scatter::new(s.x.clone(), s.y.clone())
                    .mode(Mode::Lines)
                    .name(name);
                if opts.smooth {
                    t = t.line(Line::new().shape(LineShape::Spline));
                }
                if let Some(c) = opts.colors.get(i) {
                    t = t.line(if opts.smooth {
                        Line::new().shape(LineShape::Spline).color(c.clone())
                    } else {
                        Line::new().color(c.clone())
                    });
                }
                t
            }
        };
        plot.add_trace(trace);
    }

    let layout = build_layout(opts);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Figure (multi-subplot composition) ─────────────────────────

struct SubplotEntry {
    row: usize,
    col: usize,
    chart_type: String,
    data: SubplotData,
    opts: SubplotOpts,
}

enum SubplotData {
    XY(Vec<f64>, Vec<f64>),
    LabelValue(Vec<String>, Vec<f64>),
    Values(Vec<f64>),
    Matrix(Vec<Vec<f64>>),
}

struct SubplotOpts {
    title: Option<String>,
    xlabel: Option<String>,
    ylabel: Option<String>,
    bins: usize,
}

fn render_figure(
    subplots: &[SubplotEntry],
    rows: usize,
    cols: usize,
    width: usize,
    height: usize,
    title: &Option<String>,
    theme: &str,
) -> String {
    let mut plot = Plot::new();

    let bt = resolve_theme(theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(width)
        .height(height);

    if let Some(ref t) = title {
        layout = layout.title(Title::with_text(t));
    }

    // For subplots, we set up axis domains.
    // Each subplot gets its own xaxis/yaxis pair.
    // Plotly uses xaxis, xaxis2, xaxis3... and yaxis, yaxis2, yaxis3...
    // with domain specifying the fraction of the plot area.

    let gap = 0.08;
    let cell_w = (1.0 - gap * (cols as f64 - 1.0)) / cols as f64;
    let cell_h = (1.0 - gap * (rows as f64 - 1.0)) / rows as f64;

    let mut annotations = Vec::new();

    for (idx, sp) in subplots.iter().enumerate() {
        let axis_num = idx + 1;
        let x_start = sp.col as f64 * (cell_w + gap);
        let x_end = x_start + cell_w;
        // Plotly y-axis goes bottom-up, but our rows go top-down
        let y_end = 1.0 - sp.row as f64 * (cell_h + gap);
        let y_start = y_end - cell_h;

        let x_axis_name = if axis_num == 1 {
            "x".to_string()
        } else {
            format!("x{}", axis_num)
        };
        let y_axis_name = if axis_num == 1 {
            "y".to_string()
        } else {
            format!("y{}", axis_num)
        };

        let mut xa = Axis::new().domain(&[x_start, x_end]);
        if let Some(ref xl) = sp.opts.xlabel {
            xa = xa.title(Title::with_text(xl));
        }

        let mut ya = Axis::new().domain(&[y_start, y_end]);
        if let Some(ref yl) = sp.opts.ylabel {
            ya = ya.title(Title::with_text(yl));
        }

        // Add subplot title as a centered annotation above the subplot area
        if let Some(ref t) = sp.opts.title {
            let x_center = (x_start + x_end) / 2.0;
            annotations.push(
                Annotation::new()
                    .text(t.clone())
                    .x_ref("paper")
                    .y_ref("paper")
                    .x(x_center)
                    .y(y_end + 0.02)
                    .show_arrow(false),
            );
        }

        // Set axes on layout using the numbered axis setters (Plotly supports up to 8)
        layout = match axis_num {
            1 => layout.x_axis(xa).y_axis(ya),
            2 => layout.x_axis2(xa).y_axis2(ya),
            3 => layout.x_axis3(xa).y_axis3(ya),
            4 => layout.x_axis4(xa).y_axis4(ya),
            5 => layout.x_axis5(xa).y_axis5(ya),
            6 => layout.x_axis6(xa).y_axis6(ya),
            7 => layout.x_axis7(xa).y_axis7(ya),
            8 => layout.x_axis8(xa).y_axis8(ya),
            _ => layout,
        };

        // Create trace for each subplot, assigning it to the correct axis
        let trace: Box<dyn Trace> = match (&sp.chart_type[..], &sp.data) {
            ("line", SubplotData::XY(x, y)) => Scatter::new(x.clone(), y.clone())
                .mode(Mode::Lines)
                .show_legend(false)
                .x_axis(&x_axis_name)
                .y_axis(&y_axis_name),
            ("scatter", SubplotData::XY(x, y)) => Scatter::new(x.clone(), y.clone())
                .mode(Mode::Markers)
                .show_legend(false)
                .x_axis(&x_axis_name)
                .y_axis(&y_axis_name),
            ("bar", SubplotData::LabelValue(l, v)) => Bar::new(l.clone(), v.clone())
                .show_legend(false)
                .x_axis(&x_axis_name)
                .y_axis(&y_axis_name),
            ("histogram", SubplotData::Values(d)) => {
                let mut t = Histogram::new(d.clone())
                    .show_legend(false)
                    .x_axis(&x_axis_name)
                    .y_axis(&y_axis_name);
                if sp.opts.bins > 0 {
                    t = t.n_bins_x(sp.opts.bins);
                }
                t
            }
            ("heatmap", SubplotData::Matrix(m)) => {
                let ncols = m.first().map(|r| r.len()).unwrap_or(0);
                let xl: Vec<String> = (0..ncols).map(|i| i.to_string()).collect();
                let yl: Vec<String> = (0..m.len()).map(|i| i.to_string()).collect();
                HeatMap::new(xl, yl, m.clone())
                    .show_legend(false)
                    .x_axis(&x_axis_name)
                    .y_axis(&y_axis_name)
            }
            (typ, _) => {
                // Unsupported subplot type — show annotation instead of silent dot
                let x_center = (x_start + x_end) / 2.0;
                let y_center = (y_start + y_end) / 2.0;
                annotations.push(
                    Annotation::new()
                        .text(format!("Unsupported subplot type: '{}'", typ))
                        .x_ref("paper")
                        .y_ref("paper")
                        .x(x_center)
                        .y(y_center)
                        .show_arrow(false),
                );
                Scatter::new(Vec::<f64>::new(), Vec::<f64>::new())
                    .show_legend(false)
                    .x_axis(&x_axis_name)
                    .y_axis(&y_axis_name)
            }
        };
        plot.add_trace(trace);
    }

    if !annotations.is_empty() {
        layout = layout.annotations(annotations);
    }

    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Radar chart ────────────────────────────────────────────────

fn render_radar(
    indicators: &[String],
    data: &[Vec<f64>],
    opts: &ChartOpts,
    max_val: Option<f64>,
    series_labels: &[String],
) -> String {
    let mut plot = Plot::new();

    // Close the polygon by appending the first value/indicator again
    let mut theta_closed: Vec<String> = indicators.to_vec();
    theta_closed.push(indicators[0].clone());

    for (i, row) in data.iter().enumerate() {
        let name = series_labels
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("Series {}", i + 1));

        let mut r_closed = row.to_vec();
        r_closed.push(row[0]);

        let trace = ScatterPolar::new(r_closed, theta_closed.clone())
            .fill(Fill::ToSelf)
            .name(name);

        plot.add_trace(trace);
    }

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    if let Some(max) = max_val {
        let radial = RadialAxis::new().range([0.0.into(), max.into()]);
        layout = layout.polar(LayoutPolar::new().radial_axis(radial));
    }

    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Candlestick chart ──────────────────────────────────────────

fn render_candlestick(
    dates: &[String],
    open: &[f64],
    close: &[f64],
    low: &[f64],
    high: &[f64],
    opts: &ChartOpts,
) -> String {
    let trace = Candlestick::new(
        dates.to_vec(),
        open.to_vec(),
        high.to_vec(),
        low.to_vec(),
        close.to_vec(),
    );

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Box plot ───────────────────────────────────────────────────

fn render_box(
    data_sets: &[Vec<f64>],
    opts: &ChartOpts,
    horizontal: bool,
    points: &str,
    show_mean: bool,
    group_labels: &[String],
) -> String {
    let mut plot = Plot::new();

    let box_points = match points {
        "all" => BoxPoints::All,
        "false" | "none" => BoxPoints::False,
        "suspectedoutliers" => BoxPoints::SuspectedOutliers,
        _ => BoxPoints::Outliers,
    };

    let box_mean = if show_mean {
        BoxMean::True
    } else {
        BoxMean::False
    };

    for (i, data) in data_sets.iter().enumerate() {
        let name = group_labels
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("Group {}", i + 1));

        let mut trace = if horizontal {
            BoxPlot::new(data.clone())
                .name(name)
                .orientation(Orientation::Horizontal)
        } else {
            BoxPlot::new(data.clone()).name(name)
        };

        trace = trace
            .box_points(box_points.clone())
            .box_mean(box_mean.clone());

        if let Some(c) = opts.colors.get(i) {
            trace = trace.marker(Marker::new().color(c.clone()));
        }

        plot.add_trace(trace);
    }

    let layout = build_layout(opts);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── OHLC chart ─────────────────────────────────────────────────

fn render_ohlc(
    dates: &[String],
    open: &[f64],
    close: &[f64],
    low: &[f64],
    high: &[f64],
    opts: &ChartOpts,
) -> String {
    let trace = Ohlc::new(
        dates.to_vec(),
        open.to_vec(),
        high.to_vec(),
        low.to_vec(),
        close.to_vec(),
    );

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Contour plot ───────────────────────────────────────────────

fn render_contour(x: &[f64], y: &[f64], z: &[Vec<f64>], opts: &ChartOpts) -> String {
    let trace = Contour::new(x.to_vec(), y.to_vec(), z.to_vec());

    let layout = build_layout(opts);
    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── 3D Scatter plot ────────────────────────────────────────────

fn render_scatter3d(
    x: &[f64],
    y: &[f64],
    z: &[f64],
    opts: &ChartOpts,
    point_size: usize,
) -> String {
    let mut trace = Scatter3D::new(x.to_vec(), y.to_vec(), z.to_vec()).show_legend(false);

    let mut marker = Marker::new().size(point_size);
    if let Some(c) = opts.colors.first() {
        marker = marker.color(c.clone());
    }
    trace = trace.marker(marker);

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── 3D Surface plot ────────────────────────────────────────────

fn render_surface(
    x: &Option<Vec<f64>>,
    y: &Option<Vec<f64>>,
    z: &[Vec<f64>],
    opts: &ChartOpts,
) -> String {
    let mut trace = Surface::new(z.to_vec());

    if let Some(ref xv) = x {
        trace = trace.x(xv.clone());
    }
    if let Some(ref yv) = y {
        trace = trace.y(yv.clone());
    }

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Sankey diagram ─────────────────────────────────────────────

fn render_sankey(
    node_labels: &[String],
    sources: &[usize],
    targets: &[usize],
    values: &[f64],
    opts: &ChartOpts,
) -> String {
    let label_refs: Vec<&str> = node_labels.iter().map(|s| s.as_str()).collect();
    let node = Node::new().label(label_refs);
    let link = Link::new()
        .source(sources.to_vec())
        .target(targets.to_vec())
        .value(values.to_vec());

    let trace = Sankey::new().node(node).link(link);

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Table chart ────────────────────────────────────────────────

fn render_table(headers: &[String], rows: &[Vec<String>], opts: &ChartOpts) -> String {
    // Plotly Table expects column-oriented data: values[col][row]
    let ncols = headers.len();
    let mut columns: Vec<Vec<String>> = vec![Vec::new(); ncols];
    for row in rows {
        for (j, col) in columns.iter_mut().enumerate() {
            col.push(row.get(j).cloned().unwrap_or_default());
        }
    }

    let header = Header::new(headers.to_vec());
    let cells = Cells::new(columns);
    let trace = Table::new(header, cells);

    let bt = resolve_theme(&opts.theme);
    let mut layout = Layout::new()
        .template(bt.build())
        .width(opts.width)
        .height(opts.height);

    if let Some(ref t) = opts.title {
        layout = layout.title(Title::with_text(t));
    }

    let mut plot = Plot::new();
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot_to_html(&plot)
}

// ── Registration ───────────────────────────────────────────────
