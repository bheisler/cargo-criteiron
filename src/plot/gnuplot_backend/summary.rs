use super::{debug_script, escape_underscores};
use super::{DARK_BLUE, DEFAULT_FONT, KDE_POINTS, LINEWIDTH, POINT_SIZE, SIZE};
use crate::connection::AxisScale;
use crate::estimate::Statistic;
use crate::kde;
use crate::model::Benchmark;
use crate::report::{BenchmarkId, ValueType};
use crate::stats::univariate::Sample;
use crate::value_formatter::ValueFormatter;
use criterion_plot::prelude::*;
use linked_hash_map::LinkedHashMap;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::Child;

const NUM_COLORS: usize = 8;
static COMPARISON_COLORS: [Color; NUM_COLORS] = [
    Color::Rgb(178, 34, 34),
    Color::Rgb(46, 139, 87),
    Color::Rgb(0, 139, 139),
    Color::Rgb(255, 215, 0),
    Color::Rgb(0, 0, 139),
    Color::Rgb(220, 20, 60),
    Color::Rgb(139, 0, 139),
    Color::Rgb(0, 255, 127),
];

impl AxisScale {
    fn to_gnuplot(self) -> Scale {
        match self {
            AxisScale::Linear => Scale::Linear,
            AxisScale::Logarithmic => Scale::Logarithmic,
        }
    }
}

#[cfg_attr(feature = "cargo-clippy", allow(clippy::explicit_counter_loop))]
pub fn line_comparison(
    formatter: &dyn ValueFormatter,
    title: &str,
    all_benchmarks: &[(&BenchmarkId, &Benchmark)],
    path: &Path,
    value_type: ValueType,
    axis_scale: AxisScale,
) -> Child {
    let path = PathBuf::from(path);
    let mut f = Figure::new();

    let input_suffix = match value_type {
        ValueType::Bytes => " Size (Bytes)",
        ValueType::Elements => " Size (Elements)",
        ValueType::Value => "",
    };

    f.set(Font(DEFAULT_FONT))
        .set(SIZE)
        .configure(Key, |k| {
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Outside(Vertical::Top, Horizontal::Right))
        })
        .set(Title(format!("{}: Comparison", escape_underscores(title))))
        .configure(Axis::BottomX, |a| {
            a.set(Label(format!("Input{}", input_suffix)))
                .set(axis_scale.to_gnuplot())
        });

    let mut i = 0;

    let max = all_benchmarks
        .iter()
        .map(|(_, ref data)| {
            data.latest_stats
                .estimates
                .get(&Statistic::Typical)
                .unwrap()
                .point_estimate
        })
        .fold(::std::f64::NAN, f64::max);

    let mut dummy = [1.0];
    let unit = formatter.scale_values(max, &mut dummy);

    f.configure(Axis::LeftY, |a| {
        a.configure(Grid::Major, |g| g.show())
            .configure(Grid::Minor, |g| g.hide())
            .set(Label(format!("Average time ({})", unit)))
            .set(axis_scale.to_gnuplot())
    });

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
            .map(|(id, benchmark)| {
                let x = id.as_number().unwrap();
                let y = benchmark
                    .latest_stats
                    .estimates
                    .get(&Statistic::Typical)
                    .unwrap()
                    .point_estimate;

                (x, y)
            })
            .collect();
        tuples.sort_by(|&(ax, _), &(bx, _)| (ax.partial_cmp(&bx).unwrap_or(Ordering::Less)));
        let (xs, mut ys): (Vec<_>, Vec<_>) = tuples.into_iter().unzip();
        formatter.scale_values(max, &mut ys);

        let function_name = key.as_ref().map(|string| escape_underscores(string));

        f.plot(Lines { x: &xs, y: &ys }, |c| {
            if let Some(name) = function_name {
                c.set(Label(name));
            }
            c.set(LINEWIDTH)
                .set(LineType::Solid)
                .set(COMPARISON_COLORS[i % NUM_COLORS])
        })
        .plot(Points { x: &xs, y: &ys }, |p| {
            p.set(PointType::FilledCircle)
                .set(POINT_SIZE)
                .set(COMPARISON_COLORS[i % NUM_COLORS])
        });

        i += 1;
    }

    debug_script(&path, &f);
    f.set(Output(path)).draw().unwrap()
}

pub fn violin(
    formatter: &dyn ValueFormatter,
    title: &str,
    all_benchmarks: &[(&BenchmarkId, &Benchmark)],
    path: &Path,
    axis_scale: AxisScale,
) -> Child {
    let path = PathBuf::from(&path);

    let kdes = all_benchmarks
        .iter()
        .rev()
        .map(|(_, benchmark)| {
            let (x, mut y) = kde::sweep(
                Sample::new(&benchmark.latest_stats.avg_values),
                KDE_POINTS,
                None,
            );
            let y_max = Sample::new(&y).max();
            for y in y.iter_mut() {
                *y /= y_max;
            }

            (x, y)
        })
        .collect::<Vec<_>>();
    let mut xs = kdes
        .iter()
        .flat_map(|&(ref x, _)| x.iter())
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
    let mut one = [1.0];
    // Scale the X axis units. Use the middle as a "typical value". E.g. if
    // it is 0.002 s then this function will decide that milliseconds are an
    // appropriate unit. It will multiple `one` by 1000, and return "ms".
    let unit = formatter.scale_values((min + max) / 2.0, &mut one);

    let tics = || (0..).map(|x| (f64::from(x)) + 0.5);
    let size = Size(1280, 200 + (25 * all_benchmarks.len()));
    let mut f = Figure::new();
    f.set(Font(DEFAULT_FONT))
        .set(size)
        .set(Title(format!("{}: Violin plot", escape_underscores(title))))
        .configure(Axis::BottomX, |a| {
            a.configure(Grid::Major, |g| g.show())
                .configure(Grid::Minor, |g| g.hide())
                .set(Label(format!("Average time ({})", unit)))
                .set(axis_scale.to_gnuplot())
        })
        .configure(Axis::LeftY, |a| {
            a.set(Label("Input"))
                .set(Range::Limits(0., all_benchmarks.len() as f64))
                .set(TicLabels {
                    positions: tics(),
                    labels: all_benchmarks
                        .iter()
                        .rev()
                        .map(|(id, _)| escape_underscores(id.as_title())),
                })
        });

    let mut is_first = true;
    for (i, &(ref x, ref y)) in kdes.iter().enumerate() {
        let i = i as f64 + 0.5;
        let y1: Vec<_> = y.iter().map(|&y| i + y * 0.45).collect();
        let y2: Vec<_> = y.iter().map(|&y| i - y * 0.45).collect();

        let x: Vec<_> = x.iter().map(|&x| x * one[0]).collect();

        f.plot(FilledCurve { x, y1, y2 }, |c| {
            if is_first {
                is_first = false;

                c.set(DARK_BLUE).set(Label("PDF")).set(Opacity(0.25))
            } else {
                c.set(DARK_BLUE).set(Opacity(0.25))
            }
        });
    }
    debug_script(&path, &f);
    f.set(Output(path)).draw().unwrap()
}