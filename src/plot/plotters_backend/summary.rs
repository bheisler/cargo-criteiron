use super::*;
use crate::connection::AxisScale;
use crate::estimate::Statistic;
use crate::model::Benchmark;
use linked_hash_map::LinkedHashMap;
use plotters::coord::{AsRangedCoord, Shift};
use std::cmp::Ordering;
use std::path::Path;

const NUM_COLORS: usize = 8;
static COMPARISON_COLORS: [RGBColor; NUM_COLORS] = [
    RGBColor(178, 34, 34),
    RGBColor(46, 139, 87),
    RGBColor(0, 139, 139),
    RGBColor(255, 215, 0),
    RGBColor(0, 0, 139),
    RGBColor(220, 20, 60),
    RGBColor(139, 0, 139),
    RGBColor(0, 255, 127),
];

pub fn line_comparison(
    formatter: &dyn ValueFormatter,
    title: &str,
    all_curves: &[(&BenchmarkId, &Benchmark)],
    path: &Path,
    value_type: ValueType,
    axis_scale: AxisScale,
) {
    let (unit, series_data) = line_comparison_series_data(formatter, all_curves);

    let x_range =
        plotters::data::fitting_range(series_data.iter().map(|(_, xs, _)| xs.iter()).flatten());
    let y_range =
        plotters::data::fitting_range(series_data.iter().map(|(_, _, ys)| ys.iter()).flatten());
    let root_area = SVGBackend::new(&path, SIZE)
        .into_drawing_area()
        .titled(&format!("{}: Comparision", title), (DEFAULT_FONT, 20))
        .unwrap();

    match axis_scale {
        AxisScale::Linear => draw_line_comparision_figure(
            root_area,
            &unit,
            x_range,
            y_range,
            value_type,
            series_data,
        ),
        AxisScale::Logarithmic => draw_line_comparision_figure(
            root_area,
            &unit,
            LogRange(x_range),
            LogRange(y_range),
            value_type,
            series_data,
        ),
    }
}

fn draw_line_comparision_figure<XR: AsRangedCoord<Value = f64>, YR: AsRangedCoord<Value = f64>>(
    root_area: DrawingArea<SVGBackend, Shift>,
    y_unit: &str,
    x_range: XR,
    y_range: YR,
    value_type: ValueType,
    data: Vec<(Option<&String>, Vec<f64>, Vec<f64>)>,
) {
    let input_suffix = match value_type {
        ValueType::Bytes => " Size (Bytes)",
        ValueType::Elements => " Size (Elements)",
        ValueType::Value => "",
    };

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_ranged(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .x_desc(format!("Input{}", input_suffix))
        .y_desc(format!("Average time ({})", y_unit))
        .draw()
        .unwrap();

    for (id, (name, xs, ys)) in (0..).zip(data.into_iter()) {
        let series = chart
            .draw_series(
                LineSeries::new(
                    xs.into_iter().zip(ys.into_iter()),
                    COMPARISON_COLORS[id % NUM_COLORS].filled(),
                )
                .point_size(POINT_SIZE),
            )
            .unwrap();
        if let Some(name) = name {
            let name: &str = &*name;
            series.label(name).legend(move |(x, y)| {
                Rectangle::new(
                    [(x, y - 5), (x + 20, y + 5)],
                    COMPARISON_COLORS[id % NUM_COLORS].filled(),
                )
            });
        }
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .draw()
        .unwrap();
}

#[allow(clippy::type_complexity)]
fn line_comparison_series_data<'a>(
    formatter: &dyn ValueFormatter,
    all_benchmarks: &[(&'a BenchmarkId, &'a Benchmark)],
) -> (String, Vec<(Option<&'a String>, Vec<f64>, Vec<f64>)>) {
    let max = all_benchmarks
        .iter()
        .map(|(_, bench)| {
            bench
                .latest_stats
                .estimates
                .get(&Statistic::Typical)
                .unwrap()
                .point_estimate
        })
        .fold(::std::f64::NAN, f64::max);

    let mut dummy = [1.0];
    let unit = formatter.scale_values(max, &mut dummy);

    let mut series_data = vec![];

    let mut function_id_to_benchmarks = LinkedHashMap::new();
    for (id, bench) in all_benchmarks {
        function_id_to_benchmarks
            .entry(&id.function_id)
            .or_insert(Vec::new())
            .push((*id, *bench))
    }

    for (key, mut group) in function_id_to_benchmarks {
        // Unwrap is fine here because the caller shouldn't call this with non-numeric IDs.
        let mut tuples: Vec<_> = group
            .into_iter()
            .map(|(id, bench)| {
                let x = id.as_number().unwrap();
                let y = bench
                    .latest_stats
                    .estimates
                    .get(&Statistic::Typical)
                    .unwrap()
                    .point_estimate;

                (x, y)
            })
            .collect();
        tuples.sort_by(|&(ax, _), &(bx, _)| (ax.partial_cmp(&bx).unwrap_or(Ordering::Less)));
        let function_name = key.as_ref();
        let (xs, mut ys): (Vec<_>, Vec<_>) = tuples.into_iter().unzip();
        formatter.scale_values(max, &mut ys);
        series_data.push((function_name, xs, ys));
    }
    (unit, series_data)
}

pub fn violin(
    formatter: &dyn ValueFormatter,
    title: &str,
    all_benchmarks: &[(&BenchmarkId, &Benchmark)],
    path: &Path,
    axis_scale: AxisScale,
) {
    let mut kdes = all_benchmarks
        .iter()
        .rev()
        .map(|(id, sample)| {
            let (x, mut y) = kde::sweep(
                Sample::new(&sample.latest_stats.avg_values),
                KDE_POINTS,
                None,
            );
            let y_max = Sample::new(&y).max();
            for y in y.iter_mut() {
                *y /= y_max;
            }

            (id.as_title(), x, y)
        })
        .collect::<Vec<_>>();

    let mut xs = kdes
        .iter()
        .flat_map(|&(_, ref x, _)| x.iter())
        .filter(|&&x| x > 0.);
    let (mut min, mut max) = {
        let &first = xs.next().unwrap();
        (first, first)
    };
    for &e in xs {
        if e < min {
            min = e;
        } else if e > max {
            max = e;
        }
    }
    let mut dummy = [1.0];
    let unit = formatter.scale_values(max, &mut dummy);
    kdes.iter_mut().for_each(|&mut (_, ref mut xs, _)| {
        formatter.scale_values(max, xs);
    });

    let x_range = plotters::data::fitting_range(kdes.iter().map(|(_, xs, _)| xs.iter()).flatten());
    let y_range = -0.5..all_benchmarks.len() as f64 - 0.5;

    let size = (960, 150 + (18 * all_benchmarks.len() as u32));

    let root_area = SVGBackend::new(&path, size)
        .into_drawing_area()
        .titled(&format!("{}: Violin plot", title), (DEFAULT_FONT, 20))
        .unwrap();

    match axis_scale {
        AxisScale::Linear => draw_violin_figure(root_area, &unit, x_range, y_range, kdes),
        AxisScale::Logarithmic => {
            draw_violin_figure(root_area, &unit, LogRange(x_range), y_range, kdes)
        }
    }
}

#[allow(clippy::type_complexity)]
fn draw_violin_figure<XR: AsRangedCoord<Value = f64>, YR: AsRangedCoord<Value = f64>>(
    root_area: DrawingArea<SVGBackend, Shift>,
    unit: &str,
    x_range: XR,
    y_range: YR,
    data: Vec<(&str, Box<[f64]>, Box<[f64]>)>,
) {
    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (10).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_width().min(40))
        .build_ranged(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .y_desc("Input")
        .x_desc(format!("Average time ({})", unit))
        .y_label_style((DEFAULT_FONT, 10))
        .y_label_formatter(&|v: &f64| data[v.round() as usize].0.to_string())
        .y_labels(data.len())
        .draw()
        .unwrap();

    for (i, (_, x, y)) in data.into_iter().enumerate() {
        let base = i as f64;

        chart
            .draw_series(AreaSeries::new(
                x.iter().zip(y.iter()).map(|(x, y)| (*x, base + *y / 2.0)),
                base,
                &DARK_BLUE.mix(0.25),
            ))
            .unwrap();

        chart
            .draw_series(AreaSeries::new(
                x.iter().zip(y.iter()).map(|(x, y)| (*x, base - *y / 2.0)),
                base,
                &DARK_BLUE.mix(0.25),
            ))
            .unwrap();
    }
}