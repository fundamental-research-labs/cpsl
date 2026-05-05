//! Help metadata for the Plotly-backed sandbox plotting module.

use crate::sandbox::{FieldDoc, FnDoc, ModuleDoc, Param, ParamType, ReturnType};

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
