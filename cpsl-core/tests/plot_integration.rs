#![cfg(feature = "mod-plot")]

use tempfile::TempDir;
use cpsl_core::{MountTable, Sandbox};

fn sb_with_workspace() -> (Sandbox, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut mt = MountTable::new();
    mt.parse_and_add(&format!("{}:/workspace", dir.path().display()))
        .unwrap();
    let sb = Sandbox::with_mounts(mt).unwrap();
    (sb, dir)
}

/// Verify the file is valid Plotly HTML output.
fn assert_plotly_html(path: &std::path::Path, label: &str) {
    let html = std::fs::read_to_string(path).unwrap();
    assert!(
        html.contains("<html") || html.contains("plotly"),
        "{}: should be valid Plotly HTML: first 200 chars = {}",
        label,
        &html[..200.min(html.len())]
    );
}

// ── plot.line ──────────────────────────────────────────────────

#[test]
fn line_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3, 4}, {10, 20, 15, 25}, {output = "/workspace/line.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/line.html");
    assert_plotly_html(&dir.path().join("line.html"), "line_basic");
}

#[test]
fn line_with_title_and_labels() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3}, {4, 5, 6}, {title="Test", xlabel="X", ylabel="Y", output="/workspace/lt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/lt.html");
    let html = std::fs::read_to_string(dir.path().join("lt.html")).unwrap();
    assert!(html.contains("Test"), "HTML should contain title");
}

#[test]
fn line_mismatched_lengths_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.line({1, 2}, {10, 20, 30}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("same length"),
        "error: {}",
        err.message
    );
}

// ── plot.bar ───────────────────────────────────────────────────

#[test]
fn bar_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.bar({"A", "B", "C"}, {10, 20, 15}, {output = "/workspace/bar.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/bar.html");
    assert_plotly_html(&dir.path().join("bar.html"), "bar_basic");
}

#[test]
fn bar_horizontal() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.bar({"X", "Y"}, {30, 50}, {horizontal = true, output = "/workspace/hbar.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/hbar.html");
    assert!(dir.path().join("hbar.html").exists());
}

#[test]
fn bar_mismatched_lengths_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.bar({"A"}, {10, 20}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("same length"),
        "error: {}",
        err.message
    );
}

// ── plot.scatter ───────────────────────────────────────────────

#[test]
fn scatter_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({1, 2, 3, 4, 5}, {2, 4, 1, 3, 5}, {output = "/workspace/scatter.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/scatter.html");
    assert_plotly_html(&dir.path().join("scatter.html"), "scatter_basic");
}

#[test]
fn scatter_custom_point_size() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({1, 2, 3}, {4, 5, 6}, {pointSize = 8, output = "/workspace/sp.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/sp.html");
    assert!(dir.path().join("sp.html").exists());
}

// ── plot.histogram ─────────────────────────────────────────────

#[test]
fn histogram_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.histogram({1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5, 5.5}, {output = "/workspace/hist.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/hist.html");
    assert_plotly_html(&dir.path().join("hist.html"), "histogram_basic");
}

#[test]
fn histogram_custom_bins() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.histogram({1, 2, 3, 4, 5, 6, 7, 8, 9, 10}, {bins = 5, output = "/workspace/hist5.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/hist5.html");
    assert!(dir.path().join("hist5.html").exists());
}

#[test]
fn histogram_too_few_points_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.histogram({1}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("at least 2"),
        "error: {}",
        err.message
    );
}

// ── plot.pie ───────────────────────────────────────────────────

#[test]
fn pie_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.pie({"A", "B", "C"}, {30, 50, 20}, {output = "/workspace/pie.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/pie.html");
    assert_plotly_html(&dir.path().join("pie.html"), "pie_basic");
}

#[test]
fn pie_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.pie({"X", "Y"}, {60, 40}, {title = "Split", output = "/workspace/pt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/pt.html");
    let html = std::fs::read_to_string(dir.path().join("pt.html")).unwrap();
    assert!(html.contains("Split"));
}

#[test]
fn pie_zero_total_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.pie({"A"}, {0}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("sum to > 0"),
        "error: {}",
        err.message
    );
}

// ── plot.heatmap ───────────────────────────────────────────────

#[test]
fn heatmap_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.heatmap({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}, {output = "/workspace/heat.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/heat.html");
    assert_plotly_html(&dir.path().join("heat.html"), "heatmap_basic");
}

#[test]
fn heatmap_with_labels() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.heatmap(
            {{1, 2}, {3, 4}},
            {xlabels = {"a", "b"}, ylabels = {"r1", "r2"}, output = "/workspace/hl.html"}
        )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/hl.html");
    assert!(dir.path().join("hl.html").exists());
}

#[test]
fn heatmap_empty_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.heatmap({}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("not be empty"),
        "error: {}",
        err.message
    );
}

// ── plot.multi ─────────────────────────────────────────────────

#[test]
fn multi_line_series() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.multi({
            {x = {1, 2, 3}, y = {10, 20, 30}, label = "A"},
            {x = {1, 2, 3}, y = {30, 20, 10}, label = "B"}
        }, {legend = true, output = "/workspace/multi.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/multi.html");
    assert_plotly_html(&dir.path().join("multi.html"), "multi_line_series");
}

#[test]
fn multi_mixed_types() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.multi({
            {x = {1, 2, 3}, y = {10, 20, 30}, type = "line", label = "Line"},
            {x = {1.5, 2.5}, y = {15, 25}, type = "scatter", label = "Points"}
        }, {output = "/workspace/mixed.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/mixed.html");
    assert!(dir.path().join("mixed.html").exists());
}

// ── plot.figure ────────────────────────────────────────────────

#[test]
fn figure_single_subplot() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            local fig = plot.figure({rows = 1, cols = 1, output = "/workspace/fig.html"})
            fig:subplot(1, 1, "line", {x = {1, 2, 3}, y = {4, 5, 6}})
            return fig:save()
        "#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/fig.html");
    assert_plotly_html(&dir.path().join("fig.html"), "figure_single_subplot");
}

#[test]
fn figure_2x2_grid() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"
            local fig = plot.figure({rows = 2, cols = 2, width = 800, height = 600, output = "/workspace/grid.html"})
            fig:subplot(1, 1, "line", {x = {1, 2, 3}, y = {4, 5, 6}})
            fig:subplot(1, 2, "scatter", {x = {1, 2, 3}, y = {6, 5, 4}})
            fig:subplot(2, 1, "bar", {labels = {"A", "B", "C"}, values = {10, 20, 15}})
            fig:subplot(2, 2, "histogram", {data = {1, 2, 2, 3, 3, 3, 4, 5}}, {bins = 4})
            return fig:save()
        "#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/grid.html");
    assert_plotly_html(&dir.path().join("grid.html"), "figure_2x2_grid");
}

// ── dual-signature (table form) ────────────────────────────────

#[test]
fn line_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({x = {1, 2, 3}, y = {10, 20, 15}, output = "/workspace/lt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/lt.html");
    assert!(dir.path().join("lt.html").exists());
}

#[test]
fn bar_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.bar({labels = {"A", "B"}, values = {10, 20}, output = "/workspace/bt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/bt.html");
    assert!(dir.path().join("bt.html").exists());
}

#[test]
fn scatter_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({x = {1, 2, 3}, y = {4, 5, 6}, output = "/workspace/st.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/st.html");
    assert!(dir.path().join("st.html").exists());
}

#[test]
fn histogram_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.histogram({data = {1, 2, 3, 4, 5, 6, 7, 8}, bins = 4, output = "/workspace/ht.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/ht.html");
    assert!(dir.path().join("ht.html").exists());
}

#[test]
fn pie_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.pie({labels = {"X", "Y"}, values = {60, 40}, output = "/workspace/pt2.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/pt2.html");
    assert!(dir.path().join("pt2.html").exists());
}

#[test]
fn heatmap_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.heatmap({matrix = {{1, 2}, {3, 4}}, output = "/workspace/hmt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/hmt.html");
    assert!(dir.path().join("hmt.html").exists());
}

#[test]
fn multi_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.multi({series = {{x = {1, 2, 3}, y = {10, 20, 30}, label = "A"}}, output = "/workspace/mt.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/mt.html");
    assert!(dir.path().join("mt.html").exists());
}

#[test]
fn line_table_form_with_short_aliases() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({x = {1, 2, 3}, y = {10, 20, 15}, o = "/workspace/lsa.html", t = "Short"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/lsa.html");
    let html = std::fs::read_to_string(dir.path().join("lsa.html")).unwrap();
    assert!(html.contains("Short"), "title from short alias");
}

// ── Custom colors ──────────────────────────────────────────────

#[test]
fn custom_colors() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r##"
            local c = {"#FF0000"}
            return plot.line({1, 2, 3}, {4, 5, 6}, {colors = c, output = "/workspace/red.html"})
        "##,
        )
        .unwrap();
    assert_eq!(r, "/workspace/red.html");
    assert!(dir.path().join("red.html").exists());
}

// ── Custom dimensions ──────────────────────────────────────────

#[test]
fn custom_dimensions() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2}, {3, 4}, {width = 400, height = 300, output = "/workspace/small.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/small.html");
    let html = std::fs::read_to_string(dir.path().join("small.html")).unwrap();
    assert!(
        html.contains("400") || html.contains("width"),
        "should reference width in HTML"
    );
}

// ── Help and errors ────────────────────────────────────────────

#[test]
fn plot_help() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return plot.help()").unwrap();
    assert!(r.contains("plot — Interactive HTML charts"), "help: {}", r);
    assert!(r.contains("plot.line"), "help: {}", r);
    assert!(r.contains("plot.bar"), "help: {}", r);
    assert!(r.contains("plot.scatter"), "help: {}", r);
    assert!(r.contains("plot.histogram"), "help: {}", r);
    assert!(r.contains("plot.pie"), "help: {}", r);
    assert!(r.contains("plot.heatmap"), "help: {}", r);
    assert!(r.contains("plot.multi"), "help: {}", r);
    assert!(r.contains("plot.figure"), "help: {}", r);
    assert!(r.contains("plot.radar"), "help: {}", r);
    assert!(r.contains("plot.candlestick"), "help: {}", r);
    assert!(r.contains("plot.box"), "help: {}", r);
    assert!(r.contains("plot.ohlc"), "help: {}", r);
    assert!(r.contains("plot.contour"), "help: {}", r);
    assert!(r.contains("plot.table"), "help: {}", r);
    assert!(r.contains("plot.scatter3d"), "help: {}", r);
    assert!(r.contains("plot.surface"), "help: {}", r);
    assert!(r.contains("plot.sankey"), "help: {}", r);
}

#[test]
fn plot_bar_nested_tables_shows_inline_help() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.bar({x = {"Jan","Feb"}, y = {{10,20},{30,40}}, output = "/workspace/t.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("expected"),
        "should mention type expectation: {}",
        err.message
    );
    assert!(
        err.message.contains("Usage: plot.bar("),
        "should include inline usage: {}",
        err.message
    );
    assert!(
        err.message.contains("Example:"),
        "should include example: {}",
        err.message
    );
}

#[test]
fn plot_line_nested_tables_shows_inline_help() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.line({1,2,3}, {{10,20,30},{40,50,60}}, {output = "/workspace/t.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("expected"),
        "should mention type expectation: {}",
        err.message
    );
    assert!(
        err.message.contains("Usage: plot.line("),
        "should include inline usage: {}",
        err.message
    );
}

#[test]
fn plot_nonexistent_fn_hint() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.foo()").unwrap_err();
    assert!(
        err.message.contains("plot.foo does not exist"),
        "msg: {}",
        err.message
    );
    assert!(
        err.message.contains("hint: call plot.help() for usage"),
        "msg: {}",
        err.message
    );
}

#[test]
fn global_help_mentions_plot() {
    let (s, _dir) = sb_with_workspace();
    let r = s.exec("return help()").unwrap();
    assert!(
        r.contains("plot"),
        "global help should list plot: {}",
        r
    );
}

// ── Missing output path errors ────────────────────────────────

#[test]
fn missing_output_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.line({1, 2, 3}, {4, 5, 6})"#)
        .unwrap_err();
    assert!(
        err.message.contains("output path is required"),
        "error: {}",
        err.message
    );
}

#[test]
fn figure_missing_output_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.figure({rows = 1, cols = 1})"#)
        .unwrap_err();
    assert!(
        err.message.contains("output path is required"),
        "error: {}",
        err.message
    );
}

// ── No grid option ─────────────────────────────────────────────

#[test]
fn no_grid() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({1, 2, 3}, {4, 5, 6}, {grid = false, output = "/workspace/ng.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/ng.html");
    assert!(dir.path().join("ng.html").exists());
}

// ── plot.radar ────────────────────────────────────────────────

#[test]
fn radar_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.radar(
                {"Speed", "Power", "Defense", "Magic", "Stamina"},
                {{80, 90, 70, 60, 85}},
                {output = "/workspace/radar.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/radar.html");
    assert_plotly_html(&dir.path().join("radar.html"), "radar_basic");
}

#[test]
fn radar_multi_series() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.radar(
                {"Speed", "Power", "Defense"},
                {{80, 90, 70}, {50, 60, 80}},
                {labels = {"Player A", "Player B"}, title = "Comparison", output = "/workspace/radar2.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/radar2.html");
    let html = std::fs::read_to_string(dir.path().join("radar2.html")).unwrap();
    assert!(html.contains("Comparison"));
}

#[test]
fn radar_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.radar({
                indicators = {"A", "B", "C"},
                data = {{10, 20, 30}},
                output = "/workspace/rt.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/rt.html");
    assert!(dir.path().join("rt.html").exists());
}

#[test]
fn radar_mismatched_data_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.radar({"A", "B", "C"}, {{10, 20}}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("indicators"),
        "error: {}",
        err.message
    );
}

#[test]
fn radar_with_max() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.radar(
                {"Speed", "Power", "Defense"},
                {{80, 90, 70}},
                {max = 100, output = "/workspace/rmax.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/rmax.html");
    assert!(dir.path().join("rmax.html").exists());
}

// ── plot.candlestick ──────────────────────────────────────────

#[test]
fn candlestick_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.candlestick(
                {"Mon", "Tue", "Wed", "Thu", "Fri"},
                {10, 15, 12, 18, 14},
                {15, 12, 18, 14, 16},
                {8, 10, 11, 12, 13},
                {16, 16, 20, 19, 17},
                {output = "/workspace/candle.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/candle.html");
    assert_plotly_html(&dir.path().join("candle.html"), "candlestick_basic");
}

#[test]
fn candlestick_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.candlestick({
                dates = {"Mon", "Tue", "Wed"},
                open = {10, 15, 12},
                close = {15, 12, 18},
                low = {8, 10, 11},
                high = {16, 16, 20},
                output = "/workspace/ct.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ct.html");
    assert!(dir.path().join("ct.html").exists());
}

#[test]
fn candlestick_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.candlestick({
                dates = {"Mon", "Tue"},
                open = {10, 15},
                close = {15, 12},
                low = {8, 10},
                high = {16, 16},
                title = "Stock",
                output = "/workspace/ct2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ct2.html");
    let html = std::fs::read_to_string(dir.path().join("ct2.html")).unwrap();
    assert!(html.contains("Stock"));
}

// ── Theme support ─────────────────────────────────────────────

#[test]
fn theme_grafana() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3}, {4, 5, 6}, {theme = "grafana", output = "/workspace/tg.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/tg.html");
    assert_plotly_html(&dir.path().join("tg.html"), "theme_grafana");
}

#[test]
fn theme_dark() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.bar({"A", "B"}, {10, 20}, {theme = "dark", output = "/workspace/td.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/td.html");
    assert_plotly_html(&dir.path().join("td.html"), "theme_dark");
}

#[test]
fn theme_ant() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({1, 2, 3}, {4, 5, 6}, {theme = "ant", output = "/workspace/ta.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/ta.html");
    assert!(dir.path().join("ta.html").exists());
}

#[test]
fn theme_on_radar() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.radar({"A", "B", "C"}, {{10, 20, 30}}, {theme = "grafana", output = "/workspace/tr.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/tr.html");
    assert!(dir.path().join("tr.html").exists());
}

#[test]
fn theme_on_candlestick() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.candlestick({
                dates = {"Mon", "Tue"},
                open = {10, 15},
                close = {15, 12},
                low = {8, 10},
                high = {16, 16},
                theme = "dark",
                output = "/workspace/tc.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/tc.html");
    assert!(dir.path().join("tc.html").exists());
}

#[test]
fn candlestick_mismatched_lengths_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.candlestick({"Mon", "Tue"}, {10, 15}, {15}, {8, 10}, {16, 16}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("same length"),
        "error: {}",
        err.message
    );
}

// ── Smooth and fill options ───────────────────────────────────

#[test]
fn line_smooth() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3, 4}, {10, 20, 15, 25}, {smooth = true, output = "/workspace/ls.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/ls.html");
    assert_plotly_html(&dir.path().join("ls.html"), "line_smooth");
}

#[test]
fn line_fill() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3, 4}, {10, 20, 15, 25}, {fill = true, output = "/workspace/lf.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/lf.html");
    assert_plotly_html(&dir.path().join("lf.html"), "line_fill");
}

#[test]
fn line_smooth_and_fill() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3, 4}, {10, 20, 15, 25}, {smooth = true, fill = true, output = "/workspace/lsf.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/lsf.html");
    assert!(dir.path().join("lsf.html").exists());
}

#[test]
fn multi_smooth() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.multi({
            {x = {1, 2, 3}, y = {10, 20, 30}, label = "A"},
            {x = {1, 2, 3}, y = {30, 20, 10}, label = "B"}
        }, {smooth = true, output = "/workspace/ms.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ms.html");
    assert!(dir.path().join("ms.html").exists());
}

// ── HTML content validation ────────────────────────────────────

#[test]
fn line_html_has_plotly() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.line({1, 2, 3}, {4, 5, 6}, {output = "/workspace/lp.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("lp.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
}

#[test]
fn bar_html_has_plotly() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.bar({"A", "B"}, {10, 20}, {output = "/workspace/br.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("br.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
}

#[test]
fn pie_html_has_plotly() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.pie({"A", "B"}, {60, 40}, {output = "/workspace/pp.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("pp.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
}

// ── HTML output validation (6b) ───────────────────────────────

#[test]
fn scatter_html_has_plotly_and_markers() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.scatter({1, 2, 3}, {4, 5, 6}, {output = "/workspace/sc.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("sc.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("markers"), "HTML should contain markers mode");
}

#[test]
fn histogram_html_has_plotly_and_histogram() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.histogram({1,2,3,4,5,6,7,8,9,10}, {output = "/workspace/hv.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("hv.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("histogram"), "HTML should contain histogram trace type");
}

#[test]
fn heatmap_html_has_plotly_and_heatmap() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.heatmap({{1,2},{3,4}}, {output = "/workspace/hm.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("hm.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("heatmap"), "HTML should contain heatmap trace type");
}

#[test]
fn radar_html_has_plotly_and_scatterpolar() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.radar({"A","B","C"}, {{1,2,3}}, {output = "/workspace/rd.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("rd.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("scatterpolar"), "HTML should contain scatterpolar trace type");
}

#[test]
fn candlestick_html_has_plotly_and_candlestick() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.candlestick({"Mon","Tue","Wed"}, {10,15,12}, {15,12,18}, {8,10,11}, {16,16,20}, {output = "/workspace/cs.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("cs.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("candlestick"), "HTML should contain candlestick trace type");
}

#[test]
fn box_html_has_plotly_and_box() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.box({{10,20,30,25}}, {output = "/workspace/bx.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("bx.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("box"), "HTML should contain box trace type");
}

#[test]
fn ohlc_html_has_plotly_and_ohlc() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.ohlc({"Mon","Tue"}, {10,15}, {15,12}, {8,10}, {16,16}, {output = "/workspace/oh.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("oh.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("ohlc"), "HTML should contain ohlc trace type");
}

#[test]
fn contour_html_has_plotly_and_contour() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.contour({0,1,2}, {0,1,2}, {{1,2,3},{4,5,6},{7,8,9}}, {output = "/workspace/ct.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("ct.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("contour"), "HTML should contain contour trace type");
}

#[test]
fn table_html_has_plotly_and_table() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.table({"Name","Val"}, {{"A","1"},{"B","2"}}, {output = "/workspace/tb.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("tb.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("table"), "HTML should contain table trace type");
}

#[test]
fn scatter3d_html_has_plotly_and_scatter3d() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.scatter3d({1,2,3}, {4,5,6}, {7,8,9}, {output = "/workspace/s3.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("s3.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("scatter3d"), "HTML should contain scatter3d trace type");
}

#[test]
fn surface_html_has_plotly_and_surface() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.surface({{1,2},{3,4}}, {output = "/workspace/sf.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("sf.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("surface"), "HTML should contain surface trace type");
}

#[test]
fn sankey_html_has_plotly_and_sankey() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.sankey({"A","B","C"}, {{source=1, target=2, value=10},{source=1, target=3, value=5}}, {output = "/workspace/sk.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("sk.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    assert!(html.contains("sankey"), "HTML should contain sankey trace type");
}

#[test]
fn multi_html_has_plotly_and_traces() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.multi({{x={1,2,3}, y={10,20,15}, label="A"}, {x={1,2,3}, y={5,25,10}, label="B"}}, {output="/workspace/mu.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("mu.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
    // Multi creates scatter traces
    assert!(html.contains("lines"), "HTML should contain lines mode for multi series");
}

#[test]
fn figure_html_has_plotly() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"
        local fig = plot.figure({rows=1, cols=1, output="/workspace/fg.html"})
        fig:subplot(1, 1, "line", {x={1,2,3}, y={4,5,6}})
        fig:save()
    "#).unwrap();
    let html = std::fs::read_to_string(dir.path().join("fg.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
}

#[test]
fn html_output_is_well_formed() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.line({1,2,3}, {4,5,6}, {title="Well-Formed Test", output="/workspace/wf.html"})"#)
        .unwrap();
    let html = std::fs::read_to_string(dir.path().join("wf.html")).unwrap();
    assert!(html.contains("<html") || html.contains("<!DOCTYPE"), "Should start with HTML doctype or tag");
    assert!(html.contains("</html>") || html.contains("</script>"), "Should be a complete HTML document");
    assert!(html.contains("Well-Formed Test"), "Should contain the chart title");
    assert!(html.contains("Plotly") || html.contains("plotly"), "Should reference Plotly library");
}

// ── Edge cases ────────────────────────────────────────────────

#[test]
fn line_single_point() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1}, {5}, {output = "/workspace/sp.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/sp.html");
    assert_plotly_html(&dir.path().join("sp.html"), "line_single_point");
}

#[test]
fn line_negative_values() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.line({1, 2, 3}, {-10, 5, -3}, {output = "/workspace/neg.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/neg.html");
    assert!(dir.path().join("neg.html").exists());
}

#[test]
fn bar_single_item() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.bar({"Only"}, {42}, {output = "/workspace/bs.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/bs.html");
    assert!(dir.path().join("bs.html").exists());
}

#[test]
fn scatter_two_points() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.scatter({0, 100}, {0, 100}, {output = "/workspace/s2.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/s2.html");
    assert!(dir.path().join("s2.html").exists());
}

// ── Edge cases (6d) ───────────────────────────────────────────

#[test]
fn line_large_dataset_1000_points() {
    let (s, dir) = sb_with_workspace();
    // Generate 1000-point dataset in Lua
    let r = s.exec(r#"
        local x = {}
        local y = {}
        for i = 1, 1000 do
            x[i] = i
            y[i] = math.sin(i / 50) * 100
        end
        return plot.line(x, y, {output = "/workspace/large.html", title = "1000 Points"})
    "#).unwrap();
    assert_eq!(r, "/workspace/large.html");
    assert_plotly_html(&dir.path().join("large.html"), "large_dataset");
}

#[test]
fn scatter_large_dataset_2000_points() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"
        local x = {}
        local y = {}
        for i = 1, 2000 do
            x[i] = math.random() * 100
            y[i] = math.random() * 100
        end
        return plot.scatter(x, y, {output = "/workspace/large_scatter.html"})
    "#).unwrap();
    assert_eq!(r, "/workspace/large_scatter.html");
    assert_plotly_html(&dir.path().join("large_scatter.html"), "large_scatter");
}

#[test]
fn line_all_negative_values() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.line({1,2,3,4}, {-50,-30,-45,-10}, {output = "/workspace/neg.html"})"#).unwrap();
    assert_eq!(r, "/workspace/neg.html");
    assert_plotly_html(&dir.path().join("neg.html"), "all_negative");
}

#[test]
fn line_zero_values() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.line({0,0,0}, {0,0,0}, {output = "/workspace/zero.html"})"#).unwrap();
    assert_eq!(r, "/workspace/zero.html");
    assert_plotly_html(&dir.path().join("zero.html"), "zero_values");
}

#[test]
fn bar_negative_values() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.bar({"Loss","Gain","Loss"}, {-20, 50, -10}, {output = "/workspace/neg_bar.html"})"#).unwrap();
    assert_eq!(r, "/workspace/neg_bar.html");
    assert_plotly_html(&dir.path().join("neg_bar.html"), "neg_bar");
}

#[test]
fn unicode_in_title_and_labels() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.bar({"Tokyo \u{6771}\u{4EAC}","Paris \u{5DF4}\u{9ECE}","Berlin"}, {100,80,60}, {title="World Cities \u{4E16}\u{754C}\u{90FD}\u{5E02}", output="/workspace/unicode.html"})"#).unwrap();
    assert_eq!(r, "/workspace/unicode.html");
    let html = std::fs::read_to_string(dir.path().join("unicode.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should reference Plotly.js");
}

#[test]
fn unicode_in_pie_labels() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.pie({"Caf\u{00E9}", "R\u{00E9}sum\u{00E9}", "Na\u{00EF}ve"}, {40, 35, 25}, {output="/workspace/upie.html"})"#).unwrap();
    assert_eq!(r, "/workspace/upie.html");
    assert_plotly_html(&dir.path().join("upie.html"), "unicode_pie");
}

#[test]
fn scatter_single_point() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.scatter({42}, {99}, {output="/workspace/ssp.html"})"#).unwrap();
    assert_eq!(r, "/workspace/ssp.html");
    assert_plotly_html(&dir.path().join("ssp.html"), "scatter_single");
}

#[test]
fn heatmap_single_cell() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.heatmap({{42}}, {output="/workspace/h1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/h1.html");
    assert_plotly_html(&dir.path().join("h1.html"), "heatmap_single");
}

#[test]
fn heatmap_negative_values() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.heatmap({{-5,-3},{-1,2}}, {output="/workspace/hn.html"})"#).unwrap();
    assert_eq!(r, "/workspace/hn.html");
    assert_plotly_html(&dir.path().join("hn.html"), "heatmap_neg");
}

#[test]
fn box_single_group() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.box({{1,2,3,4,5}}, {output="/workspace/b1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/b1.html");
    assert_plotly_html(&dir.path().join("b1.html"), "box_single");
}

#[test]
fn box_many_groups() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.box({{1,2,3},{4,5,6},{7,8,9},{10,11,12}}, {labels={"A","B","C","D"}, output="/workspace/b4.html"})"#).unwrap();
    assert_eq!(r, "/workspace/b4.html");
    assert_plotly_html(&dir.path().join("b4.html"), "box_many");
}

#[test]
fn contour_single_value() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.contour({0}, {0}, {{5}}, {output="/workspace/c1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/c1.html");
    assert_plotly_html(&dir.path().join("c1.html"), "contour_single");
}

#[test]
fn table_many_rows() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"
        local rows = {}
        for i = 1, 100 do
            rows[i] = {tostring(i), "val_" .. tostring(i)}
        end
        return plot.table({"ID", "Value"}, rows, {output="/workspace/tbl100.html"})
    "#).unwrap();
    assert_eq!(r, "/workspace/tbl100.html");
    assert_plotly_html(&dir.path().join("tbl100.html"), "table_many_rows");
}

#[test]
fn table_empty_rows() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.table({"A","B"}, {}, {output="/workspace/tempty.html"})"#).unwrap();
    assert_eq!(r, "/workspace/tempty.html");
    assert_plotly_html(&dir.path().join("tempty.html"), "table_empty_rows");
}

#[test]
fn scatter3d_single_point() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.scatter3d({1}, {2}, {3}, {output="/workspace/s3d1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/s3d1.html");
    assert_plotly_html(&dir.path().join("s3d1.html"), "scatter3d_single");
}

#[test]
fn surface_single_cell() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.surface({{42}}, {output="/workspace/sf1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/sf1.html");
    assert_plotly_html(&dir.path().join("sf1.html"), "surface_single");
}

#[test]
fn surface_large_grid() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"
        local z = {}
        for i = 1, 50 do
            z[i] = {}
            for j = 1, 50 do
                z[i][j] = math.sin(i/10) * math.cos(j/10)
            end
        end
        return plot.surface(z, {output="/workspace/slarge.html", title="50x50 Surface"})
    "#).unwrap();
    assert_eq!(r, "/workspace/slarge.html");
    assert_plotly_html(&dir.path().join("slarge.html"), "surface_large");
}

#[test]
fn pie_single_slice() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.pie({"Everything"}, {100}, {output="/workspace/ps.html"})"#).unwrap();
    assert_eq!(r, "/workspace/ps.html");
    assert_plotly_html(&dir.path().join("ps.html"), "pie_single");
}

#[test]
fn pie_many_slices() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.pie({"A","B","C","D","E","F","G","H","I","J"}, {10,9,8,7,6,5,4,3,2,1}, {output="/workspace/pm.html"})"#).unwrap();
    assert_eq!(r, "/workspace/pm.html");
    assert_plotly_html(&dir.path().join("pm.html"), "pie_many");
}

#[test]
fn radar_single_series() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.radar({"A","B","C"}, {{50,80,60}}, {output="/workspace/r1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/r1.html");
    assert_plotly_html(&dir.path().join("r1.html"), "radar_single");
}

#[test]
fn multi_single_series() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.multi({{x={1,2,3}, y={10,20,15}, label="Only"}}, {output="/workspace/m1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/m1.html");
    assert_plotly_html(&dir.path().join("m1.html"), "multi_single");
}

#[test]
fn sankey_single_link() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.sankey({"Source","Sink"}, {{source=1,target=2,value=100}}, {output="/workspace/sk1.html"})"#).unwrap();
    assert_eq!(r, "/workspace/sk1.html");
    assert_plotly_html(&dir.path().join("sk1.html"), "sankey_single");
}

#[test]
fn custom_dimensions_respected() {
    let (s, dir) = sb_with_workspace();
    s.exec(r#"plot.line({1,2,3}, {4,5,6}, {width=1200, height=400, output="/workspace/dim.html"})"#).unwrap();
    let html = std::fs::read_to_string(dir.path().join("dim.html")).unwrap();
    assert!(html.contains("1200"), "HTML should contain custom width");
    assert!(html.contains("400"), "HTML should contain custom height");
}

#[test]
fn minimum_dimensions_enforced() {
    let (s, dir) = sb_with_workspace();
    // Width/height < 100 should be clamped to 100
    s.exec(r#"plot.line({1,2,3}, {4,5,6}, {width=10, height=5, output="/workspace/small.html"})"#).unwrap();
    let html = std::fs::read_to_string(dir.path().join("small.html")).unwrap();
    assert!(html.contains("plotly"), "HTML should still be valid Plotly output");
}

// ── Theme integration tests ───────────────────────────────────

#[test]
fn all_themes_line() {
    let (s, dir) = sb_with_workspace();
    for theme in &["ant", "grafana", "dark", "seaborn", "matplotlib", "plotnine"] {
        let script = format!(
            r#"return plot.line({{1, 2, 3}}, {{4, 5, 6}}, {{theme = "{}", output = "/workspace/{}_line.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_line.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_line.html", theme)), &format!("theme_{}", theme));
    }
}

#[test]
fn all_themes_bar() {
    let (s, dir) = sb_with_workspace();
    for theme in &["ant", "grafana", "dark"] {
        let script = format!(
            r#"return plot.bar({{"A", "B"}}, {{10, 20}}, {{theme = "{}", output = "/workspace/{}_bar.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_bar.html", theme));
        assert!(dir.path().join(format!("{}_bar.html", theme)).exists());
    }
}

#[test]
fn all_themes_pie() {
    let (s, dir) = sb_with_workspace();
    for theme in &["ant", "grafana", "dark"] {
        let script = format!(
            r#"return plot.pie({{"X", "Y"}}, {{60, 40}}, {{theme = "{}", output = "/workspace/{}_pie.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_pie.html", theme));
        assert!(dir.path().join(format!("{}_pie.html", theme)).exists());
    }
}

// ── Theme tests (6c) ──────────────────────────────────────────

const ALL_THEMES: &[&str] = &["ant", "plotly_white", "white", "grafana", "plotly_dark", "dark", "seaborn", "matplotlib", "plotnine", "ggplot2"];

#[test]
fn all_themes_scatter() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.scatter({{1, 2, 3}}, {{4, 5, 6}}, {{theme = "{}", output = "/workspace/{}_scatter.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_scatter.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_scatter.html", theme)), &format!("scatter_{}", theme));
    }
}

#[test]
fn all_themes_histogram() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.histogram({{1,2,3,4,5,6,7,8}}, {{theme = "{}", output = "/workspace/{}_hist.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_hist.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_hist.html", theme)), &format!("hist_{}", theme));
    }
}

#[test]
fn all_themes_heatmap() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.heatmap({{{{1,2}},{{3,4}}}}, {{theme = "{}", output = "/workspace/{}_heat.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_heat.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_heat.html", theme)), &format!("heat_{}", theme));
    }
}

#[test]
fn all_themes_box() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.box({{{{10,20,30,25}}}}, {{theme = "{}", output = "/workspace/{}_box.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_box.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_box.html", theme)), &format!("box_{}", theme));
    }
}

#[test]
fn all_themes_contour() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.contour({{0,1,2}}, {{0,1,2}}, {{{{1,2,3}},{{4,5,6}},{{7,8,9}}}}, {{theme = "{}", output = "/workspace/{}_cont.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_cont.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_cont.html", theme)), &format!("cont_{}", theme));
    }
}

#[test]
fn all_themes_scatter3d() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.scatter3d({{1,2,3}}, {{4,5,6}}, {{7,8,9}}, {{theme = "{}", output = "/workspace/{}_s3d.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_s3d.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_s3d.html", theme)), &format!("s3d_{}", theme));
    }
}

#[test]
fn all_themes_surface() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.surface({{{{1,2}},{{3,4}}}}, {{theme = "{}", output = "/workspace/{}_surf.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_surf.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_surf.html", theme)), &format!("surf_{}", theme));
    }
}

#[test]
fn all_themes_sankey() {
    let (s, dir) = sb_with_workspace();
    for theme in ALL_THEMES {
        let script = format!(
            r#"return plot.sankey({{"A","B","C"}}, {{{{source=1,target=2,value=10}},{{source=1,target=3,value=5}}}}, {{theme = "{}", output = "/workspace/{}_sank.html"}})"#,
            theme, theme
        );
        let r = s.exec(&script).unwrap();
        assert_eq!(r, format!("/workspace/{}_sank.html", theme));
        assert_plotly_html(&dir.path().join(format!("{}_sank.html", theme)), &format!("sank_{}", theme));
    }
}

#[test]
fn unknown_theme_falls_back_to_plotly_white() {
    let (s, dir) = sb_with_workspace();
    let r = s.exec(r#"return plot.line({1,2,3}, {4,5,6}, {theme = "nonexistent_theme_xyz", output = "/workspace/unk.html"})"#).unwrap();
    assert_eq!(r, "/workspace/unk.html");
    assert_plotly_html(&dir.path().join("unk.html"), "unknown_theme_fallback");
}

// ── Error cases for new chart types ───────────────────────────

#[test]
fn radar_empty_indicators_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.radar({}, {{}}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(!err.message.is_empty());
}

#[test]
fn candlestick_missing_fields_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.candlestick({dates = {"Mon"}, open = {10}, output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(!err.message.is_empty());
}

#[test]
fn radar_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.radar()").unwrap_err();
    assert!(!err.message.is_empty());
}

#[test]
fn candlestick_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.candlestick()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.box ──────────────────────────────────────────────────

#[test]
fn box_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.box({{10, 20, 30, 25, 15, 35}}, {output = "/workspace/box.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/box.html");
    assert_plotly_html(&dir.path().join("box.html"), "box_basic");
}

#[test]
fn box_multiple_groups() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.box(
                {{10, 20, 30, 25, 15, 35}, {5, 15, 25, 20, 10, 30}},
                {labels = {"Group A", "Group B"}, title = "Comparison", output = "/workspace/box2.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/box2.html");
    let html = std::fs::read_to_string(dir.path().join("box2.html")).unwrap();
    assert!(html.contains("Comparison"));
}

#[test]
fn box_horizontal() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.box({{1, 2, 3, 4, 5}}, {horizontal = true, output = "/workspace/boxh.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/boxh.html");
    assert!(dir.path().join("boxh.html").exists());
}

#[test]
fn box_with_mean() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.box({{1, 2, 3, 4, 5, 6, 7, 8}}, {mean = true, output = "/workspace/boxm.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/boxm.html");
    assert!(dir.path().join("boxm.html").exists());
}

#[test]
fn box_all_points() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.box({{1, 2, 3, 4, 5}}, {points = "all", output = "/workspace/boxp.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/boxp.html");
    assert!(dir.path().join("boxp.html").exists());
}

#[test]
fn box_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(r#"return plot.box({data = {{10, 20, 30}}, labels = {"Test"}, output = "/workspace/boxt.html"})"#)
        .unwrap();
    assert_eq!(r, "/workspace/boxt.html");
    assert!(dir.path().join("boxt.html").exists());
}

#[test]
fn box_empty_data_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.box({}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.ohlc ──────────────────────────────────────────────────

#[test]
fn ohlc_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.ohlc(
                {"Mon", "Tue", "Wed", "Thu", "Fri"},
                {10, 15, 12, 18, 14},
                {15, 12, 18, 14, 16},
                {8, 10, 11, 12, 13},
                {16, 16, 20, 19, 17},
                {output = "/workspace/ohlc.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ohlc.html");
    assert_plotly_html(&dir.path().join("ohlc.html"), "ohlc_basic");
}

#[test]
fn ohlc_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.ohlc({
                dates = {"Mon", "Tue", "Wed"},
                open = {10, 15, 12},
                close = {15, 12, 18},
                low = {8, 10, 11},
                high = {16, 16, 20},
                output = "/workspace/ohlct.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ohlct.html");
    assert!(dir.path().join("ohlct.html").exists());
}

#[test]
fn ohlc_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.ohlc({
                dates = {"Mon", "Tue"},
                open = {10, 15},
                close = {15, 12},
                low = {8, 10},
                high = {16, 16},
                title = "OHLC Chart",
                output = "/workspace/ohlct2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/ohlct2.html");
    let html = std::fs::read_to_string(dir.path().join("ohlct2.html")).unwrap();
    assert!(html.contains("OHLC Chart"));
}

#[test]
fn ohlc_mismatched_lengths_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.ohlc({"Mon", "Tue"}, {10, 15}, {15}, {8, 10}, {16, 16}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("same length"),
        "error: {}",
        err.message
    );
}

#[test]
fn ohlc_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.ohlc()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.contour ───────────────────────────────────────────────

#[test]
fn contour_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.contour(
                {0, 1, 2},
                {0, 1, 2},
                {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                {output = "/workspace/contour.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/contour.html");
    assert_plotly_html(&dir.path().join("contour.html"), "contour_basic");
}

#[test]
fn contour_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.contour({
                x = {0, 1, 2},
                y = {0, 1, 2},
                z = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                output = "/workspace/contourt.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/contourt.html");
    assert!(dir.path().join("contourt.html").exists());
}

#[test]
fn contour_with_title_and_labels() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.contour({
                x = {0, 1, 2},
                y = {0, 1, 2},
                z = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                title = "Density",
                xlabel = "X",
                ylabel = "Y",
                output = "/workspace/contour2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/contour2.html");
    let html = std::fs::read_to_string(dir.path().join("contour2.html")).unwrap();
    assert!(html.contains("Density"));
}

#[test]
fn contour_empty_z_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.contour({0, 1}, {0, 1}, {}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("not be empty"),
        "error: {}",
        err.message
    );
}

#[test]
fn contour_mismatched_z_rows_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.contour({0, 1, 2}, {0, 1}, {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("rows"),
        "error: {}",
        err.message
    );
}

#[test]
fn contour_mismatched_z_cols_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.contour({0, 1, 2}, {0, 1, 2}, {{1, 2}, {3, 4}, {5, 6}}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("columns"),
        "error: {}",
        err.message
    );
}

#[test]
fn contour_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.contour()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.table ─────────────────────────────────────────────────

#[test]
fn table_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.table({"Name", "Score"}, {{"Alice", "95"}, {"Bob", "87"}}, {output = "/workspace/table.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/table.html");
    assert_plotly_html(&dir.path().join("table.html"), "table_basic");
}

#[test]
fn table_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.table({
                headers = {"Col1", "Col2", "Col3"},
                rows = {{"a", "b", "c"}, {"d", "e", "f"}},
                output = "/workspace/tablet.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/tablet.html");
    assert!(dir.path().join("tablet.html").exists());
}

#[test]
fn table_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.table({
                headers = {"Item", "Price"},
                rows = {{"Widget", "$10"}, {"Gadget", "$25"}},
                title = "Products",
                output = "/workspace/tablet2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/tablet2.html");
    let html = std::fs::read_to_string(dir.path().join("tablet2.html")).unwrap();
    assert!(html.contains("Products"));
}

#[test]
fn table_single_row() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.table({"A", "B"}, {{"1", "2"}}, {output = "/workspace/tables.html"})"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/tables.html");
    assert!(dir.path().join("tables.html").exists());
}

#[test]
fn table_empty_headers_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.table({}, {{"a"}}, {output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("not be empty"),
        "error: {}",
        err.message
    );
}

#[test]
fn table_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.table()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.scatter3d ─────────────────────────────────────────────

#[test]
fn scatter3d_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.scatter3d(
                {1, 2, 3, 4, 5},
                {2, 4, 1, 3, 5},
                {10, 20, 15, 25, 30},
                {output = "/workspace/scatter3d.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/scatter3d.html");
    assert_plotly_html(&dir.path().join("scatter3d.html"), "scatter3d_basic");
}

#[test]
fn scatter3d_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.scatter3d({
                x = {1, 2, 3},
                y = {4, 5, 6},
                z = {7, 8, 9},
                output = "/workspace/scatter3dt.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/scatter3dt.html");
    assert!(dir.path().join("scatter3dt.html").exists());
}

#[test]
fn scatter3d_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.scatter3d({
                x = {1, 2, 3},
                y = {4, 5, 6},
                z = {7, 8, 9},
                title = "3D Points",
                output = "/workspace/scatter3d2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/scatter3d2.html");
    let html = std::fs::read_to_string(dir.path().join("scatter3d2.html")).unwrap();
    assert!(html.contains("3D Points"));
}

#[test]
fn scatter3d_mismatched_lengths_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(
            r#"plot.scatter3d({1, 2, 3}, {4, 5}, {7, 8, 9}, {output = "/workspace/bad.html"})"#,
        )
        .unwrap_err();
    assert!(
        err.message.contains("same length"),
        "error: {}",
        err.message
    );
}

#[test]
fn scatter3d_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.scatter3d()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.surface ───────────────────────────────────────────────

#[test]
fn surface_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.surface(
                {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                {output = "/workspace/surface.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/surface.html");
    assert_plotly_html(&dir.path().join("surface.html"), "surface_basic");
}

#[test]
fn surface_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.surface({
                z = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                output = "/workspace/surfacet.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/surfacet.html");
    assert!(dir.path().join("surfacet.html").exists());
}

#[test]
fn surface_with_xyz() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.surface({
                x = {0, 1, 2},
                y = {0, 1, 2},
                z = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                title = "3D Surface",
                output = "/workspace/surface2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/surface2.html");
    let html = std::fs::read_to_string(dir.path().join("surface2.html")).unwrap();
    assert!(html.contains("3D Surface"));
}

#[test]
fn surface_positional_xyz() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.surface(
                {0, 1, 2},
                {0, 1, 2},
                {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}},
                {output = "/workspace/surface3.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/surface3.html");
    assert!(dir.path().join("surface3.html").exists());
}

#[test]
fn surface_empty_z_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.surface({z = {}, output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("not be empty"),
        "error: {}",
        err.message
    );
}

#[test]
fn surface_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.surface()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── plot.sankey ────────────────────────────────────────────────

#[test]
fn sankey_basic() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.sankey(
                {"A", "B", "C"},
                {{source = 1, target = 2, value = 10}, {source = 1, target = 3, value = 5}},
                {output = "/workspace/sankey.html"}
            )"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/sankey.html");
    assert_plotly_html(&dir.path().join("sankey.html"), "sankey_basic");
}

#[test]
fn sankey_table_form() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.sankey({
                nodes = {"Input", "Process", "Output"},
                links = {{source = 1, target = 2, value = 8}, {source = 2, target = 3, value = 6}},
                output = "/workspace/sankeyt.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/sankeyt.html");
    assert!(dir.path().join("sankeyt.html").exists());
}

#[test]
fn sankey_with_title() {
    let (s, dir) = sb_with_workspace();
    let r = s
        .exec(
            r#"return plot.sankey({
                nodes = {"A", "B", "C", "D"},
                links = {
                    {source = 1, target = 2, value = 10},
                    {source = 1, target = 3, value = 5},
                    {source = 2, target = 4, value = 8}
                },
                title = "Energy Flow",
                output = "/workspace/sankey2.html"
            })"#,
        )
        .unwrap();
    assert_eq!(r, "/workspace/sankey2.html");
    let html = std::fs::read_to_string(dir.path().join("sankey2.html")).unwrap();
    assert!(html.contains("Energy Flow"));
}

#[test]
fn sankey_empty_nodes_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s
        .exec(r#"plot.sankey({nodes = {}, links = {}, output = "/workspace/bad.html"})"#)
        .unwrap_err();
    assert!(
        err.message.contains("not be empty"),
        "error: {}",
        err.message
    );
}

#[test]
fn sankey_no_args_errors() {
    let (s, _dir) = sb_with_workspace();
    let err = s.exec("plot.sankey()").unwrap_err();
    assert!(!err.message.is_empty());
}

// ── HTML gallery (one of each chart type) ──────────────────────

#[test]
fn html_gallery_all_types() {
    let (s, dir) = sb_with_workspace();
    let scripts = vec![
        r#"plot.line({1,2,3,4,5}, {10,25,15,30,20}, {title="Line Chart", output="/workspace/gallery_line.html"})"#,
        r#"plot.bar({"Jan","Feb","Mar","Apr"}, {100,200,150,300}, {title="Bar Chart", output="/workspace/gallery_bar.html"})"#,
        r#"plot.scatter({1,2,3,4,5}, {2,4,1,3,5}, {title="Scatter Plot", output="/workspace/gallery_scatter.html"})"#,
        r#"plot.histogram({1,1.5,2,2.5,3,3,3.5,4,4,4.5,5,5.5,6,6,7,8}, {title="Histogram", output="/workspace/gallery_hist.html"})"#,
        r#"plot.pie({"Desktop","Mobile","Tablet"}, {55,35,10}, {title="Pie Chart", output="/workspace/gallery_pie.html"})"#,
        r#"plot.heatmap({{1,2,3},{4,5,6},{7,8,9}}, {title="Heatmap", output="/workspace/gallery_heat.html"})"#,
        r#"plot.radar({"Speed","Power","Defense","Magic","Stamina"}, {{80,90,70,60,85},{50,60,80,90,70}}, {labels={"Hero","Villain"}, title="Radar Chart", output="/workspace/gallery_radar.html"})"#,
        r#"plot.candlestick({dates={"Mon","Tue","Wed","Thu","Fri"}, open={10,15,12,18,14}, close={15,12,18,14,16}, low={8,10,11,12,13}, high={16,16,20,19,17}, title="Candlestick", output="/workspace/gallery_candle.html"})"#,
        r#"plot.line({1,2,3,4,5}, {10,25,15,30,20}, {smooth=true, fill=true, title="Smooth Area", output="/workspace/gallery_area.html"})"#,
        r#"plot.box({{10,20,30,25,15,35},{5,15,25,20,10,30}}, {labels={"A","B"}, title="Box Plot", output="/workspace/gallery_box.html"})"#,
        r#"plot.ohlc({dates={"Mon","Tue","Wed","Thu","Fri"}, open={10,15,12,18,14}, close={15,12,18,14,16}, low={8,10,11,12,13}, high={16,16,20,19,17}, title="OHLC", output="/workspace/gallery_ohlc.html"})"#,
        r#"plot.contour({x={0,1,2}, y={0,1,2}, z={{1,2,3},{4,5,6},{7,8,9}}, title="Contour", output="/workspace/gallery_contour.html"})"#,
        r#"plot.table({headers={"Name","Score","Grade"}, rows={{"Alice","95","A"},{"Bob","87","B"},{"Carol","92","A"}}, title="Results", output="/workspace/gallery_table.html"})"#,
        r#"plot.scatter3d({x={1,2,3,4,5}, y={2,4,1,3,5}, z={10,20,15,25,30}, title="3D Scatter", output="/workspace/gallery_scatter3d.html"})"#,
        r#"plot.surface({z={{1,2,3},{4,5,6},{7,8,9}}, title="Surface", output="/workspace/gallery_surface.html"})"#,
        r#"plot.sankey({nodes={"A","B","C","D"}, links={{source=1,target=2,value=10},{source=1,target=3,value=5},{source=2,target=4,value=8}}, title="Flow", output="/workspace/gallery_sankey.html"})"#,
    ];
    for script in &scripts {
        s.exec(script).unwrap();
    }
    let expected = vec![
        "gallery_line.html", "gallery_bar.html", "gallery_scatter.html",
        "gallery_hist.html", "gallery_pie.html", "gallery_heat.html",
        "gallery_radar.html", "gallery_candle.html", "gallery_area.html",
        "gallery_box.html", "gallery_ohlc.html", "gallery_contour.html",
        "gallery_table.html", "gallery_scatter3d.html", "gallery_surface.html",
        "gallery_sankey.html",
    ];
    for name in &expected {
        let path = dir.path().join(name);
        assert!(path.exists(), "missing: {}", name);
        assert_plotly_html(&path, name);
    }
}
