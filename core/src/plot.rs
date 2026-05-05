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
use crate::sandbox::{
    arg_error, wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType,
    ReturnType,
};
use mlua::{Lua, MultiValue, Value};
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
use std::sync::Arc;

const PLOT_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc { name: "output", typ: "string", required: true, description: "Output file path (.html). REQUIRED." },
    FieldDoc { name: "title", typ: "string", required: false, description: "Chart title" },
    FieldDoc { name: "xlabel", typ: "string", required: false, description: "X-axis label" },
    FieldDoc { name: "ylabel", typ: "string", required: false, description: "Y-axis label" },
    FieldDoc { name: "width", typ: "number", required: false, description: "Width in pixels (default 800)" },
    FieldDoc { name: "height", typ: "number", required: false, description: "Height in pixels (default 600)" },
    FieldDoc { name: "colors", typ: "table", required: false, description: "List of hex color strings" },
    FieldDoc { name: "theme", typ: "string", required: false, description: "Chart theme: 'plotly_white' (default), 'plotly_dark', 'seaborn', 'matplotlib', 'plotnine'" },
    FieldDoc { name: "smooth", typ: "boolean", required: false, description: "Smooth curve interpolation (line charts only)" },
    FieldDoc { name: "fill", typ: "boolean", required: false, description: "Area fill under line (line charts only)" },
];

const PLOT_SCATTER_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "pointSize",
        typ: "number",
        required: false,
        description: "Point size in pixels",
    },
];

const PLOT_HISTOGRAM_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "bins",
        typ: "number",
        required: false,
        description: "Number of bins (auto if omitted)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
];

const PLOT_PIE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "hole",
        typ: "number",
        required: false,
        description: "Donut hole size 0-1 (0 = pie, 0.4 = donut)",
    },
];

const PLOT_HEATMAP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "xlabels",
        typ: "table",
        required: false,
        description: "Custom X-axis labels",
    },
    FieldDoc {
        name: "ylabels",
        typ: "table",
        required: false,
        description: "Custom Y-axis labels",
    },
];

const PLOT_MULTI_SERIES_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "x",
        typ: "table",
        required: true,
        description: "X values",
    },
    FieldDoc {
        name: "y",
        typ: "table",
        required: true,
        description: "Y values",
    },
    FieldDoc {
        name: "type",
        typ: "string",
        required: false,
        description: r#""line" (default) or "scatter""#,
    },
    FieldDoc {
        name: "label",
        typ: "string",
        required: false,
        description: "Legend label for this series",
    },
];

const PLOT_MULTI_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "legend",
        typ: "boolean",
        required: false,
        description: "Show legend (default false)",
    },
    FieldDoc {
        name: "grid",
        typ: "boolean",
        required: false,
        description: "Show grid (default true)",
    },
];

const PLOT_FIGURE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "rows",
        typ: "number",
        required: false,
        description: "Number of subplot rows (default 1)",
    },
    FieldDoc {
        name: "cols",
        typ: "number",
        required: false,
        description: "Number of subplot columns (default 1)",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
];

const PLOT_RADAR_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "max",
        typ: "number",
        required: false,
        description: "Max value for all axes (default: auto from data)",
    },
    FieldDoc {
        name: "labels",
        typ: "table",
        required: false,
        description: "Series labels for legend",
    },
];

const PLOT_CANDLESTICK_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
];

const PLOT_BOX_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "horizontal",
        typ: "boolean",
        required: false,
        description: "Horizontal box plot (default false)",
    },
    FieldDoc {
        name: "points",
        typ: "string",
        required: false,
        description: "'all', 'outliers' (default), 'suspectedoutliers', or 'false'",
    },
    FieldDoc {
        name: "mean",
        typ: "boolean",
        required: false,
        description: "Show mean line (default false)",
    },
    FieldDoc {
        name: "labels",
        typ: "table",
        required: false,
        description: "Group labels for each data array",
    },
];

const PLOT_OHLC_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
];

const PLOT_CONTOUR_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
];

const PLOT_TABLE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
];

const PLOT_SCATTER3D_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
    FieldDoc {
        name: "colors",
        typ: "table",
        required: false,
        description: "List of hex color strings",
    },
    FieldDoc {
        name: "pointSize",
        typ: "number",
        required: false,
        description: "Point size in pixels (default 4)",
    },
];

const PLOT_SURFACE_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "xlabel",
        typ: "string",
        required: false,
        description: "X-axis label",
    },
    FieldDoc {
        name: "ylabel",
        typ: "string",
        required: false,
        description: "Y-axis label",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
];

const PLOT_SANKEY_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "output",
        typ: "string",
        required: true,
        description: "Output file path (.html). REQUIRED.",
    },
    FieldDoc {
        name: "title",
        typ: "string",
        required: false,
        description: "Chart title",
    },
    FieldDoc {
        name: "width",
        typ: "number",
        required: false,
        description: "Width in pixels (default 800)",
    },
    FieldDoc {
        name: "height",
        typ: "number",
        required: false,
        description: "Height in pixels (default 600)",
    },
];

pub(crate) static PLOT_DOC: ModuleDoc = ModuleDoc {
    name: "plot",
    summary: "Interactive HTML charts (line, bar, scatter, histogram, pie, heatmap, 3D, sankey, ...)",
    functions: &[
        FnDoc {
            name: "line",
            description: "Line chart — connects data points with lines. Use for trends over time or ordered sequences.\n    Options: smooth=true for spline interpolation, fill=true for area fill under the line.\n    Use plot.multi() to overlay multiple line series on one chart.",
            params: &[
                Param { name: "x", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "y", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.line({x={1,2,3,4,5}, y={10,25,15,30,20}, title="Monthly Revenue", xlabel="Month", ylabel="Revenue ($K)", output="/artifacts/trend.html"})"#),
        },
        FnDoc {
            name: "bar",
            description: "Bar chart — displays categorical data with rectangular bars.\n    Set horizontal=true for horizontal bars. Use plot.multi() to group multiple bar series.",
            params: &[
                Param { name: "labels", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "values", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.bar({labels={"Q1","Q2","Q3","Q4"}, values={100,200,150,300}, title="Quarterly Sales", output="/artifacts/sales.html"})"#),
        },
        FnDoc {
            name: "scatter",
            description: "Scatter plot — shows individual data points as markers.\n    Use for correlation analysis or when data has no natural ordering.\n    Set pointSize to control marker size (default 6).",
            params: &[
                Param { name: "x", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "y", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_SCATTER_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.scatter({x={1,2,3,4,5}, y={2.1,3.9,6.2,7.8,10.1}, title="Height vs Weight", pointSize=8, output="/artifacts/scatter.html"})"#),
        },
        FnDoc {
            name: "histogram",
            description: "Histogram — shows frequency distribution of numeric data with auto-binning.\n    Plotly automatically determines optimal bin count. Override with bins=N.\n    Requires at least 2 data points.",
            params: &[
                Param { name: "data", short: Some('d'), typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_HISTOGRAM_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.histogram({data={72,85,90,78,92,88,76,95,81,87,93,79,84,91,86}, bins=5, title="Test Score Distribution", xlabel="Score", output="/artifacts/hist.html"})"#),
        },
        FnDoc {
            name: "pie",
            description: "Pie chart — shows proportional composition of a whole.\n    Set hole=0.4 for a donut chart. Values must sum to > 0.\n    Best for 2-7 categories; use bar chart for more.",
            params: &[
                Param { name: "labels", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "values", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_PIE_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.pie({labels={"Desktop","Mobile","Tablet"}, values={55,35,10}, hole=0.4, title="Traffic by Device", output="/artifacts/devices.html"})"#),
        },
        FnDoc {
            name: "heatmap",
            description: "2D heatmap — visualizes matrix data as colored cells.\n    Matrix is row-major: {{row1}, {row2}, ...}. Optionally provide xlabels and ylabels.\n    Good for correlation matrices, confusion matrices, or any 2D grid data.",
            params: &[
                Param { name: "matrix", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_HEATMAP_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.heatmap({matrix={{1,0.8,0.2},{0.8,1,0.5},{0.2,0.5,1}}, xlabels={"A","B","C"}, ylabels={"A","B","C"}, title="Correlation Matrix", output="/artifacts/heat.html"})"#),
        },
        FnDoc {
            name: "multi",
            description: "Multiple series on one chart — overlay line and/or scatter traces.\n    Each series is {x={...}, y={...}, type=\"line\"|\"scatter\", label=\"name\"}.\n    X values must be numeric (use plot.bar for categorical axes).\n    Set legend=true to show the legend.",
            params: &[
                Param { name: "series", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_MULTI_SERIES_FIELDS) },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_MULTI_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.multi({{x={1,2,3,4}, y={10,20,15,25}, label="2024"}, {x={1,2,3,4}, y={8,18,22,20}, label="2023", type="scatter"}}, {legend=true, title="YoY Comparison", output="/artifacts/compare.html"})"#),
        },
        FnDoc {
            name: "figure",
            description: "Multi-subplot composition — arrange multiple charts in a grid.\n    Returns a figure object with methods:\n      fig:subplot(row, col, chartType, data, opts?) — add a subplot\n      fig:save() -> string — render all subplots and save, returns path\n    chartType can be: \"line\", \"scatter\", \"bar\", \"histogram\", \"heatmap\".\n    Rows and columns are 1-indexed.",
            params: &[
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_FIGURE_OPTS_FIELDS) },
            ],
            returns: ReturnType::Table,
            example: Some(r#"local fig = plot.figure({rows=1, cols=2, output="/artifacts/dashboard.html", title="Dashboard"})
fig:subplot(1, 1, "line", {x={1,2,3}, y={10,20,15}})
fig:subplot(1, 2, "bar", {labels={"A","B"}, values={30,50}})
return fig:save()"#),
        },
        FnDoc {
            name: "radar",
            description: "Radar (spider/polar) chart — compares multiple variables on radial axes.\n    Each data row is one series plotted as a filled polygon.\n    indicators are the axis labels, data is a matrix of rows.\n    Set max= to fix the radial scale (default: auto from data).",
            params: &[
                Param { name: "indicators", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "data", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_RADAR_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.radar({indicators={"Speed","Power","Defense","Magic","Stamina"}, data={{80,90,70,60,85},{50,60,80,90,70}}, labels={"Hero","Villain"}, title="Character Stats", output="/artifacts/radar.html"})"#),
        },
        FnDoc {
            name: "candlestick",
            description: "Candlestick chart — visualizes OHLC financial data with filled bodies.\n    Green/hollow body = close > open (bullish), red/filled = close < open (bearish).\n    All arrays (dates, open, close, low, high) must have the same length.\n    Use plot.ohlc() for tick-mark style instead of filled bodies.",
            params: &[
                Param { name: "dates", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "open", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "close", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "low", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "high", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_CANDLESTICK_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.candlestick({dates={"2024-01-02","2024-01-03","2024-01-04"}, open={150,155,148}, close={155,148,160}, low={148,145,147}, high={158,156,162}, title="AAPL", output="/artifacts/candle.html"})"#),
        },
        FnDoc {
            name: "box",
            description: "Box-and-whisker plot — shows statistical distribution (median, quartiles, outliers).\n    Pass one or more data arrays as a matrix: {{group1...}, {group2...}}.\n    Options: horizontal=true, points='all'|'outliers'|'suspectedoutliers'|'false', mean=true.\n    Set labels={\"A\",\"B\"} to name each group.",
            params: &[
                Param { name: "data", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_BOX_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.box({data={{10,20,30,25,15,35,28},{5,15,25,20,10,30,18}}, labels={"Treatment","Control"}, mean=true, title="Experiment Results", output="/artifacts/box.html"})"#),
        },
        FnDoc {
            name: "ohlc",
            description: "OHLC chart — open-high-low-close with tick marks (no filled bodies).\n    Same data format as candlestick. Use when you prefer a cleaner look\n    or need to overlay with other indicators.",
            params: &[
                Param { name: "dates", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "open", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "close", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "low", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "high", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_OHLC_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.ohlc({dates={"Mon","Tue","Wed"}, open={10,15,12}, close={15,12,18}, low={8,10,11}, high={16,16,20}, title="Weekly OHLC", output="/artifacts/ohlc.html"})"#),
        },
        FnDoc {
            name: "contour",
            description: "2D contour plot — shows z values as contour lines/filled regions over an x-y grid.\n    z is a matrix where z[i][j] is the value at (x[j], y[i]).\n    z must have len(y) rows, each with len(x) columns.",
            params: &[
                Param { name: "x", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "y", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "z", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_CONTOUR_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.contour({x={-2,-1,0,1,2}, y={-2,-1,0,1,2}, z={{8,5,2,5,8},{5,2,1,2,5},{2,1,0,1,2},{5,2,1,2,5},{8,5,2,5,8}}, title="2D Gaussian", output="/artifacts/contour.html"})"#),
        },
        FnDoc {
            name: "table",
            description: "Formatted data table — displays tabular data as an interactive HTML table.\n    Useful for presenting query results, summaries, or comparison tables.\n    Headers is a list of column names, rows is a list of row lists (strings).",
            params: &[
                Param { name: "headers", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "rows", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_TABLE_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.table({headers={"Name","Score","Grade"}, rows={{"Alice","95","A"},{"Bob","87","B+"},{"Carol","92","A-"}}, title="Exam Results", output="/artifacts/results.html"})"#),
        },
        FnDoc {
            name: "scatter3d",
            description: "3D scatter plot — plots points in 3D space with interactive rotation.\n    Accepts x, y, z arrays of equal length. Set pointSize to control marker size (default 4).",
            params: &[
                Param { name: "x", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "y", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "z", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_SCATTER3D_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.scatter3d({x={1,2,3,4,5}, y={2,4,1,3,5}, z={10,20,15,25,30}, pointSize=6, title="3D Clusters", output="/artifacts/3d.html"})"#),
        },
        FnDoc {
            name: "surface",
            description: "3D surface plot — renders a continuous surface over a grid with interactive rotation.\n    z is a matrix (required). x and y arrays are optional (default: indices).\n    Can be called as plot.surface(z_matrix, opts) or plot.surface(x, y, z, opts).",
            params: &[
                Param { name: "x", short: None, typ: ParamType::Table, required: false, fields: None },
                Param { name: "y", short: None, typ: ParamType::Table, required: false, fields: None },
                Param { name: "z", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_SURFACE_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.surface({z={{1,2,3,4},{5,6,7,8},{9,10,11,12},{13,14,15,16}}, title="Elevation Map", output="/artifacts/surface.html"})"#),
        },
        FnDoc {
            name: "sankey",
            description: "Sankey flow diagram — shows flows/transfers between nodes.\n    nodes is a list of labels. links is a list of {source=N, target=N, value=N}\n    where source/target are 1-based indices into the nodes list.\n    Good for budget flows, energy transfers, or user journey funnels.",
            params: &[
                Param { name: "nodes", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "links", short: None, typ: ParamType::Table, required: true, fields: None },
                Param { name: "opts", short: None, typ: ParamType::Table, required: true, fields: Some(PLOT_SANKEY_OPTS_FIELDS) },
            ],
            returns: ReturnType::String,
            example: Some(r#"plot.sankey({nodes={"Revenue","Costs","Profit","Salaries","Marketing","Net"}, links={{source=1,target=2,value=60},{source=1,target=3,value=40},{source=2,target=4,value=35},{source=2,target=5,value=25},{source=3,target=6,value=40}}, title="Financial Flow", output="/artifacts/flow.html"})"#),
        },
    ],
};

// ── Common option extraction ───────────────────────────────────

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

pub(crate) fn register_plot_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let plot = lua.create_table()?;

    // ── plot.line(x, y, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "line",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.line", PLOT_DOC.params("line")));
                }
                let first = args[0].clone();
                let y_opt = args.get(1).cloned();
                let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (x, y, co) = match y_opt {
                    Some(y_val) => {
                        let x = lua_table_to_f64_vec(&first)?;
                        let y = lua_table_to_f64_vec(&y_val)?;
                        (x, y, extract_chart_opts(&opts, "plot.line")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.line: expected table")),
                        };
                        let x_val: Value = t.get("x")?;
                        let y_val: Value = t.get("y")?;
                        let x = lua_table_to_f64_vec(&x_val)?;
                        let y = lua_table_to_f64_vec(&y_val)?;
                        let opts_tbl = Some(t.clone());
                        (x, y, extract_chart_opts(&opts_tbl, "plot.line")?)
                    }
                };
                if x.len() != y.len() {
                    return Err(mlua::Error::external(
                        "plot.line: x and y must have the same length",
                    ));
                }
                let html = render_line(&x, &y, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.bar(labels, values, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "bar",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.bar", PLOT_DOC.params("bar")));
                }
                let first = args[0].clone();
                let values_opt = args.get(1).cloned();
                let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (labels, values, horizontal, co) = match values_opt {
                    Some(values_val) => {
                        let labels = lua_table_to_string_vec(&first)?;
                        let values = lua_table_to_f64_vec(&values_val)?;
                        let horizontal = opts
                            .as_ref()
                            .and_then(|t| t.get::<bool>("horizontal").ok())
                            .unwrap_or(false);
                        (
                            labels,
                            values,
                            horizontal,
                            extract_chart_opts(&opts, "plot.bar")?,
                        )
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.bar: expected table")),
                        };
                        let lv: Value = t.get("labels")?;
                        let vv: Value = t.get("values")?;
                        let labels = lua_table_to_string_vec(&lv)?;
                        let values = lua_table_to_f64_vec(&vv)?;
                        let horizontal = t.get::<bool>("horizontal").unwrap_or(false);
                        let opts_tbl = Some(t.clone());
                        (
                            labels,
                            values,
                            horizontal,
                            extract_chart_opts(&opts_tbl, "plot.bar")?,
                        )
                    }
                };
                if labels.len() != values.len() {
                    return Err(mlua::Error::external(
                        "plot.bar: labels and values must have the same length",
                    ));
                }
                let html = render_bar(&labels, &values, &co, horizontal);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.scatter(x, y, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "scatter",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.scatter", PLOT_DOC.params("scatter")));
                }
                let first = args[0].clone();
                let y_opt = args.get(1).cloned();
                let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (x, y, point_size, co) = match y_opt {
                    Some(y_val) => {
                        let x = lua_table_to_f64_vec(&first)?;
                        let y = lua_table_to_f64_vec(&y_val)?;
                        let ps = opts
                            .as_ref()
                            .and_then(|t| t.get::<i32>("pointSize").ok())
                            .unwrap_or(6) as usize;
                        (x, y, ps, extract_chart_opts(&opts, "plot.scatter")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.scatter: expected table")),
                        };
                        let xv: Value = t.get("x")?;
                        let yv: Value = t.get("y")?;
                        let x = lua_table_to_f64_vec(&xv)?;
                        let y = lua_table_to_f64_vec(&yv)?;
                        let ps = t.get::<i32>("pointSize").unwrap_or(6) as usize;
                        let opts_tbl = Some(t.clone());
                        (x, y, ps, extract_chart_opts(&opts_tbl, "plot.scatter")?)
                    }
                };
                if x.len() != y.len() {
                    return Err(mlua::Error::external(
                        "plot.scatter: x and y must have the same length",
                    ));
                }
                let html = render_scatter(&x, &y, &co, point_size);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.histogram(data, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "histogram",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.histogram", PLOT_DOC.params("histogram")));
                }
                let first = args[0].clone();
                let opts: Option<mlua::Table> = args.get(1).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (data, bins_opt, co) = match &first {
                    Value::Table(t) if has_output_key(t) => {
                        let dv: Value = t.get("data")?;
                        let data = lua_table_to_f64_vec(&dv)?;
                        let bins_opt = t.get::<usize>("bins").ok();
                        let opts_tbl = Some(t.clone());
                        (
                            data,
                            bins_opt,
                            extract_chart_opts(&opts_tbl, "plot.histogram")?,
                        )
                    }
                    _ => {
                        let data = lua_table_to_f64_vec(&first)?;
                        let bins_opt = opts.as_ref().and_then(|t| t.get::<usize>("bins").ok());
                        (data, bins_opt, extract_chart_opts(&opts, "plot.histogram")?)
                    }
                };
                if data.len() < 2 {
                    return Err(mlua::Error::external(
                        "plot.histogram: need at least 2 data points",
                    ));
                }
                let html = render_histogram(&data, &co, bins_opt);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.pie(labels, values, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "pie",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.pie", PLOT_DOC.params("pie")));
                }
                let first = args[0].clone();
                let values_opt = args.get(1).cloned();
                let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (labels, values, hole, co) = match values_opt {
                    Some(values_val) => {
                        let labels = lua_table_to_string_vec(&first)?;
                        let values = lua_table_to_f64_vec(&values_val)?;
                        let hole = opts.as_ref().and_then(|t| t.get::<f64>("hole").ok());
                        (labels, values, hole, extract_chart_opts(&opts, "plot.pie")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.pie: expected table")),
                        };
                        let lv: Value = t.get("labels")?;
                        let vv: Value = t.get("values")?;
                        let labels = lua_table_to_string_vec(&lv)?;
                        let values = lua_table_to_f64_vec(&vv)?;
                        let hole = t.get::<f64>("hole").ok();
                        let opts_tbl = Some(t.clone());
                        (
                            labels,
                            values,
                            hole,
                            extract_chart_opts(&opts_tbl, "plot.pie")?,
                        )
                    }
                };
                if labels.len() != values.len() {
                    return Err(mlua::Error::external(
                        "plot.pie: labels and values must have the same length",
                    ));
                }
                let total: f64 = values.iter().sum();
                if total <= 0.0 {
                    return Err(mlua::Error::external("pie chart values must sum to > 0"));
                }
                let html = render_pie(&labels, &values, &co, hole);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.heatmap(matrix, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "heatmap",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.heatmap", PLOT_DOC.params("heatmap")));
                }
                let first = args[0].clone();
                let opts: Option<mlua::Table> = args.get(1).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (matrix, xlabels, ylabels, co) = match &first {
                    Value::Table(t) if has_output_key(t) => {
                        let mv: Value = t.get("matrix")?;
                        let matrix = lua_table_to_f64_matrix(&mv)?;
                        let xlabels = t
                            .get::<Value>("xlabels")
                            .ok()
                            .and_then(|v| lua_table_to_string_vec(&v).ok());
                        let ylabels = t
                            .get::<Value>("ylabels")
                            .ok()
                            .and_then(|v| lua_table_to_string_vec(&v).ok());
                        let opts_tbl = Some(t.clone());
                        (
                            matrix,
                            xlabels,
                            ylabels,
                            extract_chart_opts(&opts_tbl, "plot.heatmap")?,
                        )
                    }
                    _ => {
                        let matrix = lua_table_to_f64_matrix(&first)?;
                        let xlabels = opts.as_ref().and_then(|t| {
                            t.get::<Value>("xlabels")
                                .ok()
                                .and_then(|v| lua_table_to_string_vec(&v).ok())
                        });
                        let ylabels = opts.as_ref().and_then(|t| {
                            t.get::<Value>("ylabels")
                                .ok()
                                .and_then(|v| lua_table_to_string_vec(&v).ok())
                        });
                        (
                            matrix,
                            xlabels,
                            ylabels,
                            extract_chart_opts(&opts, "plot.heatmap")?,
                        )
                    }
                };
                if matrix.is_empty() {
                    return Err(mlua::Error::external("heatmap matrix must not be empty"));
                }
                let html = render_heatmap(&matrix, &co, &xlabels, &ylabels);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.multi(series, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "multi",
            lua.create_function(
                move |_, args: MultiValue| {
                    if args.is_empty() {
                        return Err(arg_error("plot.multi", PLOT_DOC.params("multi")));
                    }
                    let first = args[0].clone();
                    let opts: Option<mlua::Table> = args.get(1).and_then(|v| match v {
                        Value::Table(t) => Some(t.clone()),
                        _ => None,
                    });
                    let (series_tbl_val, co) = match &first {
                        Value::Table(t) if has_output_key(t) => {
                            let sv: Value = t.get("series")?;
                            let opts_tbl = Some(t.clone());
                            (sv, extract_chart_opts(&opts_tbl, "plot.multi")?)
                        }
                        _ => {
                            (first.clone(), extract_chart_opts(&opts, "plot.multi")?)
                        }
                    };
                    let series_tbl = match &series_tbl_val {
                        Value::Table(t) => unwrap_py_seq(t)?,
                        _ => {
                            return Err(mlua::Error::external(
                                "plot.multi: expected table of series",
                            ))
                        }
                    };
                    let mut series_list = Vec::new();
                    for i in 1..=series_tbl.raw_len() {
                        let entry: mlua::Table = series_tbl.get(i)?;
                        let x_val: Value = entry.get("x")?;
                        let y_val: Value = entry.get("y")?;
                        let x = lua_table_to_f64_vec(&x_val).map_err(|_| {
                            mlua::Error::external(format!(
                                "plot.multi: series[{}].x must be numeric values (use plot.bar for categorical/string axes)",
                                i
                            ))
                        })?;
                        let y = lua_table_to_f64_vec(&y_val).map_err(|_| {
                            mlua::Error::external(format!(
                                "plot.multi: series[{}].y must be numeric values",
                                i
                            ))
                        })?;
                        let series_type = entry
                            .get::<String>("type")
                            .unwrap_or_else(|_| "line".to_string());
                        let label = entry.get::<String>("label").ok();
                        series_list.push(SeriesData {
                            x,
                            y,
                            series_type,
                            label,
                        });
                    }
                    let html = render_multi(&series_list, &co);
                    let host_path = ensure_output_dir(&m, &co.output)?;
                    std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                    Ok(co.output)
                },
            )?,
        )?;
    }

    // ── plot.figure(opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "figure",
            lua.create_function(move |lua, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.figure", PLOT_DOC.params("figure")));
                }
                let opts: Option<mlua::Table> = args.get(0).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let rows = opts
                    .as_ref()
                    .and_then(|t| t.get::<i32>("rows").ok())
                    .unwrap_or(1) as usize;
                let cols = opts
                    .as_ref()
                    .and_then(|t| t.get::<i32>("cols").ok())
                    .unwrap_or(1) as usize;
                let width = opts
                    .as_ref()
                    .and_then(|t| t.get::<i32>("width").ok())
                    .unwrap_or(800) as usize;
                let height = opts
                    .as_ref()
                    .and_then(|t| t.get::<i32>("height").ok())
                    .unwrap_or(600) as usize;
                let output = opts
                    .as_ref()
                    .and_then(|t| t.get::<String>("output").ok())
                    .ok_or_else(|| {
                        mlua::Error::external(
                            "plot.figure: output path is required (e.g., {output = '/artifacts/figure.html'})",
                        )
                    })?;
                let title = opts.as_ref().and_then(|t| t.get::<String>("title").ok());
                let theme = opts.as_ref()
                    .and_then(|t| t.get::<String>("theme").ok())
                    .unwrap_or_else(|| "plotly_white".to_string());

                let fig = lua.create_table()?;
                fig.set("_rows", rows as i32)?;
                fig.set("_cols", cols as i32)?;
                fig.set("_width", width as i32)?;
                fig.set("_height", height as i32)?;
                fig.set("_output", output)?;
                fig.set("_theme", theme)?;
                if let Some(ref t) = title {
                    fig.set("_title", t.clone())?;
                }
                fig.set("_subplots", lua.create_table()?)?;

                // figure:subplot(row, col, chartType, data, opts?)
                fig.set(
                    "subplot",
                    lua.create_function(
                        |_,
                         (fig_tbl, row, col, chart_type, data, sp_opts): (
                            mlua::Table,
                            i32,
                            i32,
                            String,
                            mlua::Table,
                            Option<mlua::Table>,
                        )| {
                            let subplots: mlua::Table = fig_tbl.get("_subplots")?;
                            let idx = subplots.raw_len() + 1;
                            data.set("_row", row - 1)?;
                            data.set("_col", col - 1)?;
                            data.set("_chartType", chart_type)?;
                            if let Some(ref o) = sp_opts {
                                if let Ok(t) = o.get::<String>("title") {
                                    data.set("_spTitle", t)?;
                                }
                                if let Ok(x) = o.get::<String>("xlabel") {
                                    data.set("_spXlabel", x)?;
                                }
                                if let Ok(y) = o.get::<String>("ylabel") {
                                    data.set("_spYlabel", y)?;
                                }
                                if let Ok(b) = o.get::<i32>("bins") {
                                    data.set("_spBins", b)?;
                                }
                            }
                            subplots.set(idx, data)?;
                            Ok(fig_tbl)
                        },
                    )?,
                )?;

                // figure:save() -> string
                let m2 = m.clone();
                fig.set(
                    "save",
                    lua.create_function(move |_, fig_tbl: mlua::Table| {
                        let rows = fig_tbl.get::<i32>("_rows")? as usize;
                        let cols = fig_tbl.get::<i32>("_cols")? as usize;
                        let width = fig_tbl.get::<i32>("_width")? as usize;
                        let height = fig_tbl.get::<i32>("_height")? as usize;
                        let output: String = fig_tbl.get("_output")?;
                        let title: Option<String> = fig_tbl.get("_title").ok();
                        let theme: String = fig_tbl.get::<String>("_theme")
                            .unwrap_or_else(|_| "plotly_white".to_string());
                        let subplots_tbl: mlua::Table = fig_tbl.get("_subplots")?;

                        let mut subplots = Vec::new();
                        for i in 1..=subplots_tbl.raw_len() {
                            let sp: mlua::Table = subplots_tbl.get(i)?;
                            let row = sp.get::<i32>("_row")? as usize;
                            let col = sp.get::<i32>("_col")? as usize;
                            let chart_type: String = sp.get("_chartType")?;
                            let sp_title: Option<String> = sp.get("_spTitle").ok();
                            let sp_xlabel: Option<String> = sp.get("_spXlabel").ok();
                            let sp_ylabel: Option<String> = sp.get("_spYlabel").ok();
                            let sp_bins = sp.get::<i32>("_spBins").unwrap_or(10) as usize;

                            let data = match chart_type.as_str() {
                                "line" | "scatter" => {
                                    let xv: Value = sp.get("x")?;
                                    let yv: Value = sp.get("y")?;
                                    SubplotData::XY(
                                        lua_table_to_f64_vec(&xv)?,
                                        lua_table_to_f64_vec(&yv)?,
                                    )
                                }
                                "bar" => {
                                    let lv: Value = sp.get("labels")?;
                                    let vv: Value = sp.get("values")?;
                                    SubplotData::LabelValue(
                                        lua_table_to_string_vec(&lv)?,
                                        lua_table_to_f64_vec(&vv)?,
                                    )
                                }
                                "histogram" => {
                                    let dv: Value = sp.get("data")?;
                                    SubplotData::Values(lua_table_to_f64_vec(&dv)?)
                                }
                                "heatmap" => {
                                    let mv: Value = sp.get("matrix")?;
                                    SubplotData::Matrix(lua_table_to_f64_matrix(&mv)?)
                                }
                                _ => {
                                    return Err(mlua::Error::external(format!(
                                        "unsupported subplot type: {}",
                                        chart_type
                                    )))
                                }
                            };

                            subplots.push(SubplotEntry {
                                row,
                                col,
                                chart_type,
                                data,
                                opts: SubplotOpts {
                                    title: sp_title,
                                    xlabel: sp_xlabel,
                                    ylabel: sp_ylabel,
                                    bins: sp_bins,
                                },
                            });
                        }

                        let html =
                            render_figure(&subplots, rows, cols, width, height, &title, &theme);
                        let host_path = ensure_output_dir(&m2, &output)?;
                        std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                        Ok(output)
                    })?,
                )?;

                Ok(fig)
            })?,
        )?;
    }

    // ── plot.radar(indicators, data, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "radar",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.radar", PLOT_DOC.params("radar")));
                }
                let first = args[0].clone();
                let data_opt = args.get(1).cloned();
                let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (indicators, data, max_val, series_labels, co) = match data_opt {
                    Some(data_val) => {
                        let indicators = lua_table_to_string_vec(&first)?;
                        let data = lua_table_to_f64_matrix(&data_val)?;
                        let max_val = opts.as_ref().and_then(|t| t.get::<f64>("max").ok());
                        let labels = opts
                            .as_ref()
                            .and_then(|t| t.get::<Value>("labels").ok())
                            .and_then(|v| lua_table_to_string_vec(&v).ok())
                            .unwrap_or_default();
                        (
                            indicators,
                            data,
                            max_val,
                            labels,
                            extract_chart_opts(&opts, "plot.radar")?,
                        )
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.radar: expected table")),
                        };
                        let iv: Value = t.get("indicators")?;
                        let indicators = lua_table_to_string_vec(&iv)?;
                        let dv: Value = t.get("data")?;
                        let data = lua_table_to_f64_matrix(&dv)?;
                        let max_val = t.get::<f64>("max").ok();
                        let labels = t
                            .get::<Value>("labels")
                            .ok()
                            .and_then(|v| lua_table_to_string_vec(&v).ok())
                            .unwrap_or_default();
                        let opts_tbl = Some(t.clone());
                        (
                            indicators,
                            data,
                            max_val,
                            labels,
                            extract_chart_opts(&opts_tbl, "plot.radar")?,
                        )
                    }
                };
                if indicators.is_empty() {
                    return Err(mlua::Error::external(
                        "plot.radar: indicators must not be empty",
                    ));
                }
                for (i, row) in data.iter().enumerate() {
                    if row.len() != indicators.len() {
                        return Err(mlua::Error::external(format!(
                            "plot.radar: data[{}] has {} values but there are {} indicators",
                            i + 1,
                            row.len(),
                            indicators.len()
                        )));
                    }
                }
                let html = render_radar(&indicators, &data, &co, max_val, &series_labels);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.candlestick(dates, open, close, low, high, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "candlestick",
            lua.create_function(
                move |_, args: MultiValue| {
                    if args.is_empty() {
                        return Err(arg_error("plot.candlestick", PLOT_DOC.params("candlestick")));
                    }
                    let first = args[0].clone();
                    let open_opt = args.get(1).cloned();
                    let (dates, open, close, low, high, co) = match open_opt {
                        Some(open_val) => {
                            let dates = lua_table_to_string_vec(&first)?;
                            let open = lua_table_to_f64_vec(&open_val)?;
                            let close_val = args.get(2).ok_or_else(|| {
                                mlua::Error::external("plot.candlestick: missing 'close' argument")
                            })?;
                            let close = lua_table_to_f64_vec(close_val)?;
                            let low_val = args.get(3).ok_or_else(|| {
                                mlua::Error::external("plot.candlestick: missing 'low' argument")
                            })?;
                            let low = lua_table_to_f64_vec(low_val)?;
                            let high_val = args.get(4).ok_or_else(|| {
                                mlua::Error::external("plot.candlestick: missing 'high' argument")
                            })?;
                            let high = lua_table_to_f64_vec(high_val)?;
                            let opts: Option<mlua::Table> = args.get(5).and_then(|v| match v {
                                Value::Table(t) => Some(t.clone()),
                                _ => None,
                            });
                            (dates, open, close, low, high, extract_chart_opts(&opts, "plot.candlestick")?)
                        }
                        None => {
                            let t = match &first {
                                Value::Table(t) => t,
                                _ => return Err(mlua::Error::external("plot.candlestick: expected table")),
                            };
                            let dv: Value = t.get("dates")?;
                            let dates = lua_table_to_string_vec(&dv)?;
                            let ov: Value = t.get("open")?;
                            let open = lua_table_to_f64_vec(&ov)?;
                            let cv: Value = t.get("close")?;
                            let close = lua_table_to_f64_vec(&cv)?;
                            let lv: Value = t.get("low")?;
                            let low = lua_table_to_f64_vec(&lv)?;
                            let hv: Value = t.get("high")?;
                            let high = lua_table_to_f64_vec(&hv)?;
                            let opts_tbl = Some(t.clone());
                            (dates, open, close, low, high, extract_chart_opts(&opts_tbl, "plot.candlestick")?)
                        }
                    };
                    let n = dates.len();
                    if open.len() != n || close.len() != n || low.len() != n || high.len() != n {
                        return Err(mlua::Error::external(
                            "plot.candlestick: dates, open, close, low, high must all have the same length",
                        ));
                    }
                    let html = render_candlestick(&dates, &open, &close, &low, &high, &co);
                    let host_path = ensure_output_dir(&m, &co.output)?;
                    std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                    Ok(co.output)
                },
            )?,
        )?;
    }

    // ── plot.box(data, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "box",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.box", PLOT_DOC.params("box")));
                }
                let first = args[0].clone();
                let opts: Option<mlua::Table> = args.get(1).and_then(|v| match v {
                    Value::Table(t) => Some(t.clone()),
                    _ => None,
                });
                let (data_sets, horizontal, points, show_mean, group_labels, co) = match &first {
                    Value::Table(t) if has_output_key(t) => {
                        // Table form: plot.box({data={{...},{...}}, labels={...}, output=...})
                        let dv: Value = t.get("data")?;
                        let data_sets = lua_table_to_f64_matrix(&dv)?;
                        let horizontal = t.get::<bool>("horizontal").unwrap_or(false);
                        let points = t
                            .get::<String>("points")
                            .unwrap_or_else(|_| "outliers".to_string());
                        let show_mean = t.get::<bool>("mean").unwrap_or(false);
                        let labels = t
                            .get::<Value>("labels")
                            .ok()
                            .and_then(|v| lua_table_to_string_vec(&v).ok())
                            .unwrap_or_default();
                        let opts_tbl = Some(t.clone());
                        (
                            data_sets,
                            horizontal,
                            points,
                            show_mean,
                            labels,
                            extract_chart_opts(&opts_tbl, "plot.box")?,
                        )
                    }
                    _ => {
                        // Positional form: plot.box({{...},{...}}, {opts})
                        let data_sets = lua_table_to_f64_matrix(&first)?;
                        let horizontal = opts
                            .as_ref()
                            .and_then(|t| t.get::<bool>("horizontal").ok())
                            .unwrap_or(false);
                        let points = opts
                            .as_ref()
                            .and_then(|t| t.get::<String>("points").ok())
                            .unwrap_or_else(|| "outliers".to_string());
                        let show_mean = opts
                            .as_ref()
                            .and_then(|t| t.get::<bool>("mean").ok())
                            .unwrap_or(false);
                        let labels = opts
                            .as_ref()
                            .and_then(|t| t.get::<Value>("labels").ok())
                            .and_then(|v| lua_table_to_string_vec(&v).ok())
                            .unwrap_or_default();
                        (
                            data_sets,
                            horizontal,
                            points,
                            show_mean,
                            labels,
                            extract_chart_opts(&opts, "plot.box")?,
                        )
                    }
                };
                if data_sets.is_empty() {
                    return Err(mlua::Error::external("plot.box: data must not be empty"));
                }
                for (i, ds) in data_sets.iter().enumerate() {
                    if ds.is_empty() {
                        return Err(mlua::Error::external(format!(
                            "plot.box: data[{}] must not be empty",
                            i + 1
                        )));
                    }
                }
                let html = render_box(
                    &data_sets,
                    &co,
                    horizontal,
                    &points,
                    show_mean,
                    &group_labels,
                );
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.ohlc(dates, open, close, low, high, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "ohlc",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.ohlc", PLOT_DOC.params("ohlc")));
                }
                let first = args[0].clone();
                let open_opt = args.get(1).cloned();
                let (dates, open, close, low, high, co) = match open_opt {
                    Some(open_val) => {
                        let dates = lua_table_to_string_vec(&first)?;
                        let open = lua_table_to_f64_vec(&open_val)?;
                        let close_val = args.get(2).ok_or_else(|| {
                            mlua::Error::external("plot.ohlc: missing 'close' argument")
                        })?;
                        let close = lua_table_to_f64_vec(close_val)?;
                        let low_val = args.get(3).ok_or_else(|| {
                            mlua::Error::external("plot.ohlc: missing 'low' argument")
                        })?;
                        let low = lua_table_to_f64_vec(low_val)?;
                        let high_val = args.get(4).ok_or_else(|| {
                            mlua::Error::external("plot.ohlc: missing 'high' argument")
                        })?;
                        let high = lua_table_to_f64_vec(high_val)?;
                        let opts: Option<mlua::Table> = args.get(5).and_then(|v| match v {
                            Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (
                            dates,
                            open,
                            close,
                            low,
                            high,
                            extract_chart_opts(&opts, "plot.ohlc")?,
                        )
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.ohlc: expected table")),
                        };
                        let dv: Value = t.get("dates")?;
                        let dates = lua_table_to_string_vec(&dv)?;
                        let ov: Value = t.get("open")?;
                        let open = lua_table_to_f64_vec(&ov)?;
                        let cv: Value = t.get("close")?;
                        let close = lua_table_to_f64_vec(&cv)?;
                        let lv: Value = t.get("low")?;
                        let low = lua_table_to_f64_vec(&lv)?;
                        let hv: Value = t.get("high")?;
                        let high = lua_table_to_f64_vec(&hv)?;
                        let opts_tbl = Some(t.clone());
                        (
                            dates,
                            open,
                            close,
                            low,
                            high,
                            extract_chart_opts(&opts_tbl, "plot.ohlc")?,
                        )
                    }
                };
                let n = dates.len();
                if open.len() != n || close.len() != n || low.len() != n || high.len() != n {
                    return Err(mlua::Error::external(
                        "plot.ohlc: dates, open, close, low, high must all have the same length",
                    ));
                }
                let html = render_ohlc(&dates, &open, &close, &low, &high, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.contour(x, y, z, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "contour",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.contour", PLOT_DOC.params("contour")));
                }
                let first = args[0].clone();
                let y_opt = args.get(1).cloned();
                let (x, y, z, co) = match y_opt {
                    Some(y_val) => {
                        let x = lua_table_to_f64_vec(&first)?;
                        let y = lua_table_to_f64_vec(&y_val)?;
                        let z_val = args.get(2).ok_or_else(|| {
                            mlua::Error::external("plot.contour: missing 'z' argument")
                        })?;
                        let z = lua_table_to_f64_matrix(z_val)?;
                        let opts: Option<mlua::Table> = args.get(3).and_then(|v| match v {
                            Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (x, y, z, extract_chart_opts(&opts, "plot.contour")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.contour: expected table")),
                        };
                        let xv: Value = t.get("x")?;
                        let x = lua_table_to_f64_vec(&xv)?;
                        let yv: Value = t.get("y")?;
                        let y = lua_table_to_f64_vec(&yv)?;
                        let zv: Value = t.get("z")?;
                        let z = lua_table_to_f64_matrix(&zv)?;
                        let opts_tbl = Some(t.clone());
                        (x, y, z, extract_chart_opts(&opts_tbl, "plot.contour")?)
                    }
                };
                if z.is_empty() {
                    return Err(mlua::Error::external(
                        "plot.contour: z matrix must not be empty",
                    ));
                }
                if z.len() != y.len() {
                    return Err(mlua::Error::external(format!(
                        "plot.contour: z must have {} rows (one per y value), got {}",
                        y.len(),
                        z.len()
                    )));
                }
                for (i, row) in z.iter().enumerate() {
                    if row.len() != x.len() {
                        return Err(mlua::Error::external(format!(
                            "plot.contour: z[{}] must have {} columns (one per x value), got {}",
                            i + 1,
                            x.len(),
                            row.len()
                        )));
                    }
                }
                let html = render_contour(&x, &y, &z, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.table(headers, rows, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "table",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.table", PLOT_DOC.params("table")));
                }
                let first = args[0].clone();
                let rows_opt = args.get(1).cloned();
                let (headers, rows, co) = match rows_opt {
                    Some(rows_val) => {
                        let headers = lua_table_to_string_vec(&first)?;
                        let rows = lua_table_to_string_matrix(&rows_val)?;
                        let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                            Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (headers, rows, extract_chart_opts(&opts, "plot.table")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.table: expected table")),
                        };
                        let hv: Value = t.get("headers")?;
                        let headers = lua_table_to_string_vec(&hv)?;
                        let rv: Value = t.get("rows")?;
                        let rows = lua_table_to_string_matrix(&rv)?;
                        let opts_tbl = Some(t.clone());
                        (headers, rows, extract_chart_opts(&opts_tbl, "plot.table")?)
                    }
                };
                if headers.is_empty() {
                    return Err(mlua::Error::external(
                        "plot.table: headers must not be empty",
                    ));
                }
                let html = render_table(&headers, &rows, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.scatter3d(x, y, z, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "scatter3d",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.scatter3d", PLOT_DOC.params("scatter3d")));
                }
                let first = args[0].clone();
                let y_opt = args.get(1).cloned();
                let (x, y, z, point_size, co) = match y_opt {
                    Some(y_val) => {
                        let x = lua_table_to_f64_vec(&first)?;
                        let y = lua_table_to_f64_vec(&y_val)?;
                        let z_val = args.get(2).ok_or_else(|| {
                            mlua::Error::external("plot.scatter3d: missing 'z' argument")
                        })?;
                        let z = lua_table_to_f64_vec(z_val)?;
                        let opts: Option<mlua::Table> = args.get(3).and_then(|v| match v {
                            Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        let ps = opts
                            .as_ref()
                            .and_then(|t| t.get::<i32>("pointSize").ok())
                            .unwrap_or(4) as usize;
                        (x, y, z, ps, extract_chart_opts(&opts, "plot.scatter3d")?)
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => {
                                return Err(mlua::Error::external("plot.scatter3d: expected table"))
                            }
                        };
                        let xv: Value = t.get("x")?;
                        let x = lua_table_to_f64_vec(&xv)?;
                        let yv: Value = t.get("y")?;
                        let y = lua_table_to_f64_vec(&yv)?;
                        let zv: Value = t.get("z")?;
                        let z = lua_table_to_f64_vec(&zv)?;
                        let ps = t.get::<i32>("pointSize").unwrap_or(4) as usize;
                        let opts_tbl = Some(t.clone());
                        (
                            x,
                            y,
                            z,
                            ps,
                            extract_chart_opts(&opts_tbl, "plot.scatter3d")?,
                        )
                    }
                };
                if x.len() != y.len() || x.len() != z.len() {
                    return Err(mlua::Error::external(
                        "plot.scatter3d: x, y, and z must have the same length",
                    ));
                }
                let html = render_scatter3d(&x, &y, &z, &co, point_size);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.surface(z, opts?) or plot.surface(x, y, z, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "surface",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.surface", PLOT_DOC.params("surface")));
                }
                let first = args[0].clone();
                let second = args.get(1).cloned();
                let (x, y, z, co) = match &first {
                    Value::Table(t) if has_output_key(t) => {
                        // Table form: plot.surface({z=..., x=..., y=..., output=...})
                        let zv: Value = t.get("z")?;
                        let z = lua_table_to_f64_matrix(&zv)?;
                        let x = t
                            .get::<Value>("x")
                            .ok()
                            .and_then(|v| lua_table_to_f64_vec(&v).ok());
                        let y = t
                            .get::<Value>("y")
                            .ok()
                            .and_then(|v| lua_table_to_f64_vec(&v).ok());
                        let opts_tbl = Some(t.clone());
                        (x, y, z, extract_chart_opts(&opts_tbl, "plot.surface")?)
                    }
                    _ => {
                        // Try positional: could be (z_matrix, opts) or (x, y, z, opts)
                        // If first arg is a matrix (table of tables), treat as z
                        let first_is_matrix = match &first {
                            Value::Table(t) => {
                                let t = unwrap_py_seq(t)?;
                                t.raw_len() > 0
                                    && t.get::<Value>(1)
                                        .ok()
                                        .map(|v| matches!(v, Value::Table(_)))
                                        .unwrap_or(false)
                            }
                            _ => false,
                        };
                        if first_is_matrix {
                            // plot.surface(z_matrix, opts?)
                            let z = lua_table_to_f64_matrix(&first)?;
                            let opts: Option<mlua::Table> = second.as_ref().and_then(|v| match v {
                                Value::Table(t) => Some(t.clone()),
                                _ => None,
                            });
                            (None, None, z, extract_chart_opts(&opts, "plot.surface")?)
                        } else {
                            // plot.surface(x, y, z, opts?)
                            let x = lua_table_to_f64_vec(&first)?;
                            let y_val = second.ok_or_else(|| {
                                mlua::Error::external("plot.surface: missing 'y' argument")
                            })?;
                            let y = lua_table_to_f64_vec(&y_val)?;
                            let z_val = args.get(2).ok_or_else(|| {
                                mlua::Error::external("plot.surface: missing 'z' argument")
                            })?;
                            let z = lua_table_to_f64_matrix(z_val)?;
                            let opts: Option<mlua::Table> = args.get(3).and_then(|v| match v {
                                Value::Table(t) => Some(t.clone()),
                                _ => None,
                            });
                            (
                                Some(x),
                                Some(y),
                                z,
                                extract_chart_opts(&opts, "plot.surface")?,
                            )
                        }
                    }
                };
                if z.is_empty() {
                    return Err(mlua::Error::external(
                        "plot.surface: z matrix must not be empty",
                    ));
                }
                let html = render_surface(&x, &y, &z, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    // ── plot.sankey(nodes, links, opts?) ──
    {
        let m = mounts.clone();
        plot.set(
            "sankey",
            lua.create_function(move |_, args: MultiValue| {
                if args.is_empty() {
                    return Err(arg_error("plot.sankey", PLOT_DOC.params("sankey")));
                }
                let first = args[0].clone();
                let links_opt = args.get(1).cloned();
                let (node_labels, sources, targets, values, co) = match links_opt {
                    Some(links_val) => {
                        let nodes = lua_table_to_string_vec(&first)?;
                        let links_tbl = match &links_val {
                            Value::Table(t) => unwrap_py_seq(t)?,
                            _ => {
                                return Err(mlua::Error::external(
                                    "plot.sankey: links must be a table",
                                ))
                            }
                        };
                        let (sources, targets, values) = parse_sankey_links(&links_tbl)?;
                        let opts: Option<mlua::Table> = args.get(2).and_then(|v| match v {
                            Value::Table(t) => Some(t.clone()),
                            _ => None,
                        });
                        (
                            nodes,
                            sources,
                            targets,
                            values,
                            extract_chart_opts(&opts, "plot.sankey")?,
                        )
                    }
                    None => {
                        let t = match &first {
                            Value::Table(t) => t,
                            _ => return Err(mlua::Error::external("plot.sankey: expected table")),
                        };
                        let nv: Value = t.get("nodes")?;
                        let node_labels = lua_table_to_string_vec(&nv)?;
                        let lv: Value = t.get("links")?;
                        let links_tbl = match &lv {
                            Value::Table(t) => unwrap_py_seq(t)?,
                            _ => {
                                return Err(mlua::Error::external(
                                    "plot.sankey: links must be a table",
                                ))
                            }
                        };
                        let (sources, targets, values) = parse_sankey_links(&links_tbl)?;
                        let opts_tbl = Some(t.clone());
                        (
                            node_labels,
                            sources,
                            targets,
                            values,
                            extract_chart_opts(&opts_tbl, "plot.sankey")?,
                        )
                    }
                };
                if node_labels.is_empty() {
                    return Err(mlua::Error::external(
                        "plot.sankey: nodes must not be empty",
                    ));
                }
                let html = render_sankey(&node_labels, &sources, &targets, &values, &co);
                let host_path = ensure_output_dir(&m, &co.output)?;
                std::fs::write(&host_path, &html).map_err(mlua::Error::external)?;
                Ok(co.output)
            })?,
        )?;
    }

    crate::lua_util::register_help_functions(lua, &plot, &PLOT_DOC)?;

    lua.globals().set("plot", plot)?;
    wrap_module_with_help_hints(lua, "plot")?;

    Ok(())
}
