//! Lua global registration for the Plotly-backed plotting module.

use super::doc::PLOT_DOC;
use super::*;
use crate::mount::MountTable;
use crate::sandbox::{arg_error, wrap_module_with_help_hints};
use mlua::{Lua, MultiValue, Value};
use std::sync::Arc;

pub(crate) fn register_plot_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let plot = lua.create_table()?;

    register_basic_charts(lua, &plot, mounts.clone())?;

    register_composite_charts(lua, &plot, mounts.clone())?;

    register_specialized_charts(lua, &plot, mounts.clone())?;

    register_table_and_3d_charts(lua, &plot, mounts.clone())?;

    crate::lua_util::register_help_functions(lua, &plot, &PLOT_DOC)?;

    lua.globals().set("plot", plot)?;
    wrap_module_with_help_hints(lua, "plot")?;

    Ok(())
}

fn register_basic_charts(
    lua: &Lua,
    plot: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
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

    Ok(())
}

fn register_composite_charts(
    lua: &Lua,
    plot: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
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

    Ok(())
}

fn register_specialized_charts(
    lua: &Lua,
    plot: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
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

    Ok(())
}

fn register_table_and_3d_charts(
    lua: &Lua,
    plot: &mlua::Table,
    mounts: Arc<MountTable>,
) -> Result<(), mlua::Error> {
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

    Ok(())
}
