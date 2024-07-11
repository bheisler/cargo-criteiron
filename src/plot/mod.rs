#[cfg(feature = "gnuplot_backend")]
mod gnuplot_backend;
#[cfg(feature = "plotters_backend")]
mod plotters_backend;

#[cfg(feature = "gnuplot_backend")]
pub use gnuplot_backend::Gnuplot;
#[cfg(feature = "plotters_backend")]
pub use plotters_backend::PlottersBackend;

use crate::connection::AxisScale;
use crate::estimate::Statistic;
use crate::estimate::{ConfidenceInterval, Estimate};
use crate::kde;
use crate::model::Benchmark;
use crate::report::{BenchmarkId, ComparisonData, MeasurementData, ReportContext, ValueType};
use crate::stats::bivariate::regression::Slope;
use crate::stats::bivariate::Data;
use crate::stats::univariate::Sample;
use crate::stats::Distribution;
use crate::value_formatter::ValueFormatter;
use linked_hash_map::LinkedHashMap;
use std::path::PathBuf;

const REPORT_STATS: [Statistic; 7] = [
    Statistic::Typical,
    Statistic::Slope,
    Statistic::Mean,
    Statistic::Median,
    Statistic::MedianAbsDev,
    Statistic::MedianAbsDev,
    Statistic::StdDev,
];
const CHANGE_STATS: [Statistic; 2] = [Statistic::Mean, Statistic::Median];

#[derive(Clone, Copy)]
pub struct PlotContext<'a> {
    pub id: &'a BenchmarkId,
    pub context: &'a ReportContext,
    pub size: Option<Size>,
    pub is_thumbnail: bool,
}

const KDE_POINTS: usize = 500;

#[derive(Debug, Clone, Copy)]
pub struct Size(pub usize, pub usize);

impl<'a> PlotContext<'a> {
    pub fn line_comparison_path(&self) -> PathBuf {
        path!(
            &self.context.output_directory,
            self.id.as_directory_name(),
            "lines.svg"
        )
    }

    pub fn violin_path(&self) -> PathBuf {
        path!(
            &self.context.output_directory,
            self.id.as_directory_name(),
            "violin.svg"
        )
    }
}

pub trait Plotter {
    fn pdf(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn pdf_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn pdf_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );
    fn pdf_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );

    fn iteration_times(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn iteration_times_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn iteration_times_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );
    fn iteration_times_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );

    fn regression(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn regression_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );
    fn regression_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );
    fn regression_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    );

    fn abs_distributions(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    );

    fn rel_distributions(&mut self, ctx: PlotContext<'_>, comparison: &ComparisonData);

    fn line_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        formatter: &ValueFormatter,
        all_curves: &[(&BenchmarkId, &Benchmark)],
        value_type: ValueType,
    );

    fn violin(
        &mut self,
        ctx: PlotContext<'_>,
        formatter: &ValueFormatter,
        all_curves: &[(&BenchmarkId, &Benchmark)],
    );

    fn t_test(&mut self, ctx: PlotContext<'_>, comparison: &ComparisonData);

    fn history(
        &mut self,
        ctx: PlotContext<'_>,
        upper_bound: &[f64],
        point_estimate: &[f64],
        lower_bound: &[f64],
        ids: &[String],
        unit: &str,
    );

    fn wait(&mut self);
}

// Some types representing things we might want to draw

pub struct Point {
    x: f64,
    y: f64,
}

pub struct Line {
    pub start: Point,
    pub end: Point,
}
pub struct VerticalLine {
    x: f64,
}

pub struct LineCurve<'a> {
    xs: &'a [f64],
    ys: &'a [f64],
}

pub struct Points<'a> {
    xs: &'a [f64],
    ys: &'a [f64],
}

pub struct FilledCurve<'a> {
    xs: &'a [f64],
    ys_1: &'a [f64],
    ys_2: &'a [f64],
}

// If the plotting backends aren't enabled, nothing reads some of the fields here.
#[allow(dead_code)]
pub struct Rectangle {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

pub trait PlottingBackend {
    fn abs_distribution(
        &mut self,
        id: &BenchmarkId,
        statistic: Statistic,
        size: Option<Size>,
        path: PathBuf,

        x_unit: &str,
        distribution_curve: LineCurve,
        bootstrap_area: FilledCurve,
        point_estimate: Line,
    );

    fn rel_distribution(
        &mut self,
        id: &BenchmarkId,
        statistic: Statistic,
        size: Option<Size>,
        path: PathBuf,

        distribution_curve: LineCurve,
        confidence_interval: FilledCurve,
        point_estimate: Line,
        noise_threshold: Rectangle,
    );

    fn iteration_times(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,

        unit: &str,
        is_thumbnail: bool,
        current_times: Points,
        base_times: Option<Points>,
    );

    fn regression(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,
        is_thumbnail: bool,

        x_label: &str,
        x_scale: f64,
        unit: &str,
        sample: Points,
        regression: Line,
        confidence_interval: FilledCurve,
    );

    fn regression_comparison(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,
        is_thumbnail: bool,

        x_label: &str,
        x_scale: f64,
        unit: &str,
        current_regression: Line,
        current_confidence_interval: FilledCurve,
        base_regression: Line,
        base_confidence_interval: FilledCurve,
    );

    fn pdf_full(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,

        unit: &str,
        y_label: &str,
        y_scale: f64,
        max_iters: f64,

        pdf: FilledCurve,
        mean: VerticalLine,
        fences: (VerticalLine, VerticalLine, VerticalLine, VerticalLine),
        points: (Points, Points, Points),
    );
    fn pdf_thumbnail(
        &mut self,
        size: Option<Size>,
        path: PathBuf,

        unit: &str,

        mean: Line,
        pdf: FilledCurve,
    );
    fn pdf_comparison(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,
        is_thumbnail: bool,

        unit: &str,

        current_mean: Line,
        current_pdf: FilledCurve,
        base_mean: Line,
        base_pdf: FilledCurve,
    );
    fn t_test(
        &mut self,
        id: &BenchmarkId,
        size: Option<Size>,
        path: PathBuf,

        t: VerticalLine,
        t_distribution: FilledCurve,
    );

    fn line_comparison(
        &mut self,
        path: PathBuf,
        title: &str,
        unit: &str,
        value_type: ValueType,
        axis_scale: AxisScale,
        lines: &[(Option<&String>, LineCurve)],
    );

    fn violin(
        &mut self,
        path: PathBuf,
        title: &str,
        unit: &str,
        axis_scale: AxisScale,
        lines: &[(&str, LineCurve)],
    );

    fn history_plot(
        &mut self,
        id: &BenchmarkId,
        size: Size,
        path: PathBuf,

        point_estimate: LineCurve,
        confidence_interval: FilledCurve,
        ids: &[String],
        unit: &str,
    );

    fn wait(&mut self);
}

pub struct PlotGenerator<B: PlottingBackend> {
    pub backend: B,
}
impl<B: PlottingBackend> PlotGenerator<B> {
    fn abs_distribution(
        &mut self,
        id: &BenchmarkId,
        context: &ReportContext,
        formatter: &ValueFormatter,
        statistic: Statistic,
        distribution: &Distribution<f64>,
        estimate: &Estimate,
        size: Option<Size>,
    ) {
        let ci = &estimate.confidence_interval;
        let typical = ci.upper_bound;
        let mut ci_values = [ci.lower_bound, ci.upper_bound, estimate.point_estimate];
        let unit = formatter.scale_values(typical, &mut ci_values);
        let (lb, ub, point) = (ci_values[0], ci_values[1], ci_values[2]);

        let start = lb - (ub - lb) / 9.;
        let end = ub + (ub - lb) / 9.;
        let mut scaled_xs: Vec<f64> = distribution.iter().cloned().collect();
        let _ = formatter.scale_values(typical, &mut scaled_xs);
        let scaled_xs_sample = Sample::new(&scaled_xs);
        let (kde_xs, ys) = kde::sweep(scaled_xs_sample, KDE_POINTS, Some((start, end)));

        // interpolate between two points of the KDE sweep to find the Y position at the point estimate.
        let n_point = kde_xs
            .iter()
            .position(|&x| x >= point)
            .unwrap_or(kde_xs.len() - 1)
            .max(1); // Must be at least the second element or this will panic
        let slope = (ys[n_point] - ys[n_point - 1]) / (kde_xs[n_point] - kde_xs[n_point - 1]);
        let y_point = ys[n_point - 1] + (slope * (point - kde_xs[n_point - 1]));

        let start = kde_xs
            .iter()
            .enumerate()
            .find(|&(_, &x)| x >= lb)
            .unwrap()
            .0;
        let end = kde_xs
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &x)| x <= ub)
            .unwrap()
            .0;
        let len = end - start;

        let distribution_curve = LineCurve {
            xs: &kde_xs,
            ys: &ys,
        };
        let bootstrap_area = FilledCurve {
            xs: &kde_xs[start..end],
            ys_1: &ys[start..end],
            ys_2: &vec![0.0; len],
        };
        let estimate = Line {
            start: Point { x: point, y: 0.0 },
            end: Point {
                x: point,
                y: y_point,
            },
        };

        self.backend.abs_distribution(
            id,
            statistic,
            size,
            context.report_path(id, &format!("{}.svg", statistic)),
            &unit,
            distribution_curve,
            bootstrap_area,
            estimate,
        );
    }

    fn rel_distribution(
        &mut self,
        id: &BenchmarkId,
        context: &ReportContext,
        statistic: Statistic,
        distribution: &Distribution<f64>,
        estimate: &Estimate,
        noise_threshold: f64,
        size: Option<Size>,
    ) {
        let ci = &estimate.confidence_interval;
        let (lb, ub) = (ci.lower_bound, ci.upper_bound);

        let start = lb - (ub - lb) / 9.;
        let end = ub + (ub - lb) / 9.;
        let (xs, ys) = kde::sweep(distribution, KDE_POINTS, Some((start, end)));
        let xs_ = Sample::new(&xs);

        // interpolate between two points of the KDE sweep to find the Y position at the point estimate.
        let point = estimate.point_estimate;
        let n_point = xs
            .iter()
            .position(|&x| x >= point)
            .unwrap_or(ys.len() - 1)
            .max(1);
        let slope = (ys[n_point] - ys[n_point - 1]) / (xs[n_point] - xs[n_point - 1]);
        let y_point = ys[n_point - 1] + (slope * (point - xs[n_point - 1]));

        let start = xs.iter().enumerate().find(|&(_, &x)| x >= lb).unwrap().0;
        let end = xs
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &x)| x <= ub)
            .unwrap()
            .0;
        let len = end - start;

        let x_min = xs_.min();
        let x_max = xs_.max();

        let (fc_start, fc_end) = if noise_threshold < x_min || -noise_threshold > x_max {
            let middle = (x_min + x_max) / 2.;

            (middle, middle)
        } else {
            (
                if -noise_threshold < x_min {
                    x_min
                } else {
                    -noise_threshold
                },
                if noise_threshold > x_max {
                    x_max
                } else {
                    noise_threshold
                },
            )
        };

        let distribution_curve = LineCurve { xs: &xs, ys: &ys };
        let confidence_interval = FilledCurve {
            xs: &xs[start..end],
            ys_1: &ys[start..end],
            ys_2: &vec![0.0; len],
        };
        let estimate = Line {
            start: Point { x: point, y: 0.0 },
            end: Point {
                x: point,
                y: y_point,
            },
        };
        let noise_threshold = Rectangle {
            left: fc_start,
            right: fc_end,
            top: 1.0,
            bottom: 0.0,
        };

        self.backend.rel_distribution(
            id,
            statistic,
            size,
            context.report_path(id, &format!("change/{}.svg", statistic)),
            distribution_curve,
            confidence_interval,
            estimate,
            noise_threshold,
        );
    }

    fn iteration_time_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        is_thumbnail: bool,
        file_path: PathBuf,
    ) {
        let data = &measurements.avg_times;
        let max_avg_time = data.max();
        let mut scaled_y: Vec<_> = data.iter().map(|(f, _)| f).collect();
        let unit = formatter.scale_values(max_avg_time, &mut scaled_y);
        let scaled_y = Sample::new(&scaled_y);

        let xs: Vec<f64> = (1..=scaled_y.len()).map(|i| i as f64).collect();

        let points = Points {
            xs: &xs,
            ys: scaled_y,
        };
        self.backend.iteration_times(
            ctx.id,
            ctx.size,
            file_path,
            &unit,
            is_thumbnail,
            points,
            None,
        );
    }

    fn iteration_time_comparison_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
        is_thumbnail: bool,
        file_path: PathBuf,
    ) {
        let current_data = &measurements.avg_times;
        let base_data = &comparison.base_avg_times;

        let mut all_data: Vec<f64> = current_data.iter().map(|(f, _)| f).collect();
        all_data.extend_from_slice(base_data);

        let typical_value = Sample::new(&all_data).max();
        let unit = formatter.scale_values(typical_value, &mut all_data);

        let (scaled_current_y, scaled_base_y) = all_data.split_at(current_data.len());
        let scaled_current_y = Sample::new(scaled_current_y);
        let scaled_base_y = Sample::new(scaled_base_y);

        let current_xs: Vec<f64> = (1..=scaled_current_y.len()).map(|i| i as f64).collect();
        let base_xs: Vec<f64> = (1..=scaled_base_y.len()).map(|i| i as f64).collect();

        let current_points = Points {
            xs: &current_xs,
            ys: scaled_current_y,
        };
        let base_points = Points {
            xs: &base_xs,
            ys: scaled_base_y,
        };
        self.backend.iteration_times(
            ctx.id,
            ctx.size,
            file_path,
            &unit,
            is_thumbnail,
            current_points,
            Some(base_points),
        );
    }

    fn regression_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        is_thumbnail: bool,
        file_path: PathBuf,
    ) {
        let slope_estimate = &measurements.absolute_estimates.slope.as_ref().unwrap();
        let slope_dist = &measurements.distributions.slope.as_ref().unwrap();
        let (lb, ub) =
            slope_dist.confidence_interval(slope_estimate.confidence_interval.confidence_level);

        let data = &measurements.data;
        let (max_iters, typical) = (data.x().max(), data.y().max());
        let mut scaled_y: Vec<f64> = data.y().iter().cloned().collect();
        let unit = formatter.scale_values(typical, &mut scaled_y);
        let scaled_y = Sample::new(&scaled_y);

        let point_estimate = Slope::fit(&measurements.data).0;
        let mut scaled_points = [point_estimate * max_iters, lb * max_iters, ub * max_iters];
        let _ = formatter.scale_values(typical, &mut scaled_points);
        let [point, lb, ub] = scaled_points;

        let exponent = (max_iters.log10() / 3.).floor() as i32 * 3;
        let x_scale = 10f64.powi(-exponent);

        let x_label = if exponent == 0 {
            "Iterations".to_owned()
        } else {
            format!("Iterations (x 10^{})", exponent)
        };

        let sample = Points {
            xs: data.x(),
            ys: scaled_y,
        };
        let regression = Line {
            start: Point { x: 0.0, y: 0.0 },
            end: Point {
                x: max_iters,
                y: point,
            },
        };
        let confidence_interval = FilledCurve {
            xs: &[0.0, max_iters],
            ys_1: &[0.0, lb],
            ys_2: &[0.0, ub],
        };

        self.backend.regression(
            ctx.id,
            ctx.size,
            file_path,
            is_thumbnail,
            &x_label,
            x_scale,
            &unit,
            sample,
            regression,
            confidence_interval,
        )
    }

    fn regression_comparison_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
        is_thumbnail: bool,
        file_path: PathBuf,
    ) {
        let base_data = Data::new(&comparison.base_iter_counts, &comparison.base_sample_times);

        let data = &measurements.data;
        let max_iters = base_data.x().max().max(data.x().max());
        let typical = base_data.y().max().max(data.y().max());

        let exponent = (max_iters.log10() / 3.).floor() as i32 * 3;
        let x_scale = 10f64.powi(-exponent);

        let x_label = if exponent == 0 {
            "Iterations".to_owned()
        } else {
            format!("Iterations (x 10^{})", exponent)
        };

        let Estimate {
            confidence_interval:
                ConfidenceInterval {
                    lower_bound: base_lb,
                    upper_bound: base_ub,
                    ..
                },
            point_estimate: base_point,
            ..
        } = comparison.base_estimates.slope.as_ref().unwrap();

        let Estimate {
            confidence_interval:
                ConfidenceInterval {
                    lower_bound: lb,
                    upper_bound: ub,
                    ..
                },
            point_estimate: point,
            ..
        } = measurements.absolute_estimates.slope.as_ref().unwrap();

        let mut points = [
            base_lb * max_iters,
            base_point * max_iters,
            base_ub * max_iters,
            lb * max_iters,
            point * max_iters,
            ub * max_iters,
        ];
        let unit = formatter.scale_values(typical, &mut points);
        let [base_lb, base_point, base_ub, lb, point, ub] = points;

        let current_regression = Line {
            start: Point { x: 0.0, y: 0.0 },
            end: Point {
                x: max_iters,
                y: point,
            },
        };
        let current_confidence_interval = FilledCurve {
            xs: &[0.0, max_iters],
            ys_1: &[0.0, lb],
            ys_2: &[0.0, ub],
        };

        let base_regression = Line {
            start: Point { x: 0.0, y: 0.0 },
            end: Point {
                x: max_iters,
                y: base_point,
            },
        };
        let base_confidence_interval = FilledCurve {
            xs: &[0.0, max_iters],
            ys_1: &[0.0, base_lb],
            ys_2: &[0.0, base_ub],
        };

        self.backend.regression_comparison(
            ctx.id,
            ctx.size,
            file_path,
            is_thumbnail,
            &x_label,
            x_scale,
            &unit,
            current_regression,
            current_confidence_interval,
            base_regression,
            base_confidence_interval,
        )
    }

    fn pdf_full(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        file_path: PathBuf,
    ) {
        let avg_times = &measurements.avg_times;
        let typical = avg_times.max();
        let mut scaled_avg_times: Vec<f64> = (avg_times as &Sample<f64>).iter().cloned().collect();
        let unit = formatter.scale_values(typical, &mut scaled_avg_times);
        let scaled_avg_times = Sample::new(&scaled_avg_times);

        let mean = scaled_avg_times.mean();

        let iter_counts = measurements.iter_counts();
        let &max_iters = iter_counts
            .iter()
            .max_by_key(|&&iters| iters as u64)
            .unwrap();
        let exponent = (max_iters.log10() / 3.).floor() as i32 * 3;
        let y_scale = 10f64.powi(-exponent);

        let y_label = if exponent == 0 {
            "Iterations".to_owned()
        } else {
            format!("Iterations (x 10^{})", exponent)
        };

        let (xs, ys) = kde::sweep(scaled_avg_times, KDE_POINTS, None);
        let (lost, lomt, himt, hist) = avg_times.fences();
        let mut fences = [lost, lomt, himt, hist];
        let _ = formatter.scale_values(typical, &mut fences);
        let [lost, lomt, himt, hist] = fences;

        let pdf = FilledCurve {
            xs: &xs,
            ys_1: &ys,
            ys_2: &vec![0.0; ys.len()],
        };
        let mean = VerticalLine { x: mean };

        let make_fence = |fence| VerticalLine { x: fence };
        let low_severe = make_fence(lost);
        let low_mild = make_fence(lomt);
        let high_mild = make_fence(himt);
        let high_severe = make_fence(hist);

        let (not_xs, not_ys): (Vec<f64>, Vec<f64>) = (avg_times.iter())
            .zip(scaled_avg_times.iter().copied())
            .zip(iter_counts.iter().copied())
            .filter_map(|(((_, point_label), x), y)| {
                if !point_label.is_outlier() {
                    Some((x, y))
                } else {
                    None
                }
            })
            .unzip();
        let not_outlier_points = Points {
            xs: &not_xs,
            ys: &not_ys,
        };

        let (mild_xs, mild_ys): (Vec<f64>, Vec<f64>) = (avg_times.iter())
            .zip(scaled_avg_times.iter().copied())
            .zip(iter_counts.iter().copied())
            .filter_map(|(((_, point_label), x), y)| {
                if point_label.is_mild() {
                    Some((x, y))
                } else {
                    None
                }
            })
            .unzip();
        let mild_points = Points {
            xs: &mild_xs,
            ys: &mild_ys,
        };

        let (severe_xs, severe_ys): (Vec<f64>, Vec<f64>) = (avg_times.iter())
            .zip(scaled_avg_times.iter().copied())
            .zip(iter_counts.iter().copied())
            .filter_map(|(((_, point_label), x), y)| {
                if point_label.is_severe() {
                    Some((x, y))
                } else {
                    None
                }
            })
            .unzip();
        let severe_points = Points {
            xs: &severe_xs,
            ys: &severe_ys,
        };

        self.backend.pdf_full(
            ctx.id,
            ctx.size,
            file_path,
            &unit,
            &y_label,
            y_scale,
            max_iters,
            pdf,
            mean,
            (low_severe, low_mild, high_mild, high_severe),
            (not_outlier_points, mild_points, severe_points),
        );
    }

    fn pdf_thumbnail_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        file_path: PathBuf,
    ) {
        let avg_times = &*measurements.avg_times;
        let typical = avg_times.max();
        let mut scaled_avg_times: Vec<f64> = (avg_times as &Sample<f64>).iter().cloned().collect();
        let unit = formatter.scale_values(typical, &mut scaled_avg_times);
        let scaled_avg_times = Sample::new(&scaled_avg_times);
        let mean = scaled_avg_times.mean();

        let (xs, ys, mean_y) = kde::sweep_and_estimate(scaled_avg_times, KDE_POINTS, None, mean);

        let mean = Line {
            start: Point { x: mean, y: 0.0 },
            end: Point { x: mean, y: mean_y },
        };
        let pdf = FilledCurve {
            xs: &xs,
            ys_1: &ys,
            ys_2: &vec![0.0; ys.len()],
        };

        self.backend
            .pdf_thumbnail(ctx.size, file_path, &unit, mean, pdf);
    }

    fn pdf_comparison_plot(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
        file_path: PathBuf,
        is_thumbnail: bool,
    ) {
        let base_avg_times = Sample::new(&comparison.base_avg_times);
        let typical = base_avg_times.max().max(measurements.avg_times.max());
        let mut scaled_base_avg_times: Vec<f64> = comparison.base_avg_times.clone();
        let unit = formatter.scale_values(typical, &mut scaled_base_avg_times);
        let scaled_base_avg_times = Sample::new(&scaled_base_avg_times);

        let mut scaled_new_avg_times: Vec<f64> = (&measurements.avg_times as &Sample<f64>)
            .iter()
            .cloned()
            .collect();
        let _ = formatter.scale_values(typical, &mut scaled_new_avg_times);
        let scaled_new_avg_times = Sample::new(&scaled_new_avg_times);

        let base_mean = scaled_base_avg_times.mean();
        let new_mean = scaled_new_avg_times.mean();

        let (base_xs, base_ys, base_y_mean) =
            kde::sweep_and_estimate(scaled_base_avg_times, KDE_POINTS, None, base_mean);
        let (xs, ys, y_mean) =
            kde::sweep_and_estimate(scaled_new_avg_times, KDE_POINTS, None, new_mean);

        let base_mean = Line {
            start: Point {
                x: base_mean,
                y: 0.0,
            },
            end: Point {
                x: base_mean,
                y: base_y_mean,
            },
        };
        let base_pdf = FilledCurve {
            xs: &base_xs,
            ys_1: &base_ys,
            ys_2: &vec![0.0; base_ys.len()],
        };

        let current_mean = Line {
            start: Point {
                x: new_mean,
                y: 0.0,
            },
            end: Point {
                x: new_mean,
                y: y_mean,
            },
        };
        let current_pdf = FilledCurve {
            xs: &xs,
            ys_1: &ys,
            ys_2: &vec![0.0; base_ys.len()],
        };

        self.backend.pdf_comparison(
            ctx.id,
            ctx.size,
            file_path,
            is_thumbnail,
            &unit,
            current_mean,
            current_pdf,
            base_mean,
            base_pdf,
        );
    }

    fn t_test_plot(
        &mut self,
        ctx: PlotContext<'_>,
        comparison: &ComparisonData,
        file_path: PathBuf,
    ) {
        let t = comparison.t_value;
        let (xs, ys) = kde::sweep(&comparison.t_distribution, KDE_POINTS, None);

        let t = VerticalLine { x: t };
        let t_distribution = FilledCurve {
            xs: &xs,
            ys_1: &ys,
            ys_2: &vec![0.0; ys.len()],
        };

        self.backend
            .t_test(ctx.id, ctx.size, file_path, t, t_distribution)
    }

    fn history_plot(
        &mut self,
        ctx: PlotContext<'_>,
        size: Size,
        upper_bound: &[f64],
        point_estimate: &[f64],
        lower_bound: &[f64],
        ids: &[String],
        file_path: PathBuf,
        unit: &str,
    ) {
        let xs: Vec<_> = (0..point_estimate.len()).map(|i| i as f64).collect();
        let point_estimate = LineCurve {
            xs: &xs,
            ys: point_estimate,
        };
        let confidence_interval = FilledCurve {
            xs: &xs,
            ys_1: upper_bound,
            ys_2: lower_bound,
        };

        self.backend.history_plot(
            ctx.id,
            size,
            file_path,
            point_estimate,
            confidence_interval,
            ids,
            unit,
        );
    }
}
impl<B: PlottingBackend> Plotter for PlotGenerator<B> {
    fn pdf(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.pdf_full(
            ctx,
            measurements,
            formatter,
            ctx.context.report_path(ctx.id, "pdf.svg"),
        );
    }
    fn pdf_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.pdf_thumbnail_plot(
            ctx,
            measurements,
            formatter,
            ctx.context.report_path(ctx.id, "pdf_small.svg"),
        );
    }
    fn pdf_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.pdf_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            ctx.context.report_path(ctx.id, "both/pdf.svg"),
            false,
        )
    }
    fn pdf_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.pdf_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            ctx.context.report_path(ctx.id, "relative_pdf_small.svg"),
            true,
        )
    }

    fn iteration_times(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.iteration_time_plot(
            ctx,
            measurements,
            formatter,
            false,
            ctx.context.report_path(ctx.id, "iteration_times.svg"),
        );
    }
    fn iteration_times_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.iteration_time_plot(
            ctx,
            measurements,
            formatter,
            true,
            ctx.context.report_path(ctx.id, "iteration_times_small.svg"),
        );
    }
    fn iteration_times_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.iteration_time_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            false,
            ctx.context.report_path(ctx.id, "both/iteration_times.svg"),
        );
    }
    fn iteration_times_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.iteration_time_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            true,
            ctx.context
                .report_path(ctx.id, "relative_iteration_times_small.svg"),
        );
    }

    fn regression(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.regression_plot(
            ctx,
            measurements,
            formatter,
            false,
            ctx.context.report_path(ctx.id, "regression.svg"),
        );
    }

    fn regression_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        self.regression_plot(
            ctx,
            measurements,
            formatter,
            true,
            ctx.context.report_path(ctx.id, "regression_small.svg"),
        );
    }
    fn regression_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.regression_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            false,
            ctx.context.report_path(ctx.id, "both/regression.svg"),
        );
    }
    fn regression_comparison_thumbnail(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
        comparison: &ComparisonData,
    ) {
        self.regression_comparison_plot(
            ctx,
            measurements,
            formatter,
            comparison,
            true,
            ctx.context
                .report_path(ctx.id, "relative_regression_small.svg"),
        );
    }

    fn abs_distributions(
        &mut self,
        ctx: PlotContext<'_>,
        measurements: &MeasurementData<'_>,
        formatter: &ValueFormatter<'_>,
    ) {
        REPORT_STATS
            .iter()
            .filter_map(|stat| {
                measurements.distributions.get(*stat).and_then(|dist| {
                    measurements
                        .absolute_estimates
                        .get(*stat)
                        .map(|est| (*stat, dist, est))
                })
            })
            .for_each(|(statistic, distribution, estimate)| {
                self.abs_distribution(
                    ctx.id,
                    ctx.context,
                    formatter,
                    statistic,
                    distribution,
                    estimate,
                    ctx.size,
                )
            })
    }

    fn rel_distributions(&mut self, ctx: PlotContext<'_>, comparison: &ComparisonData) {
        CHANGE_STATS.iter().for_each(|&statistic| {
            self.rel_distribution(
                ctx.id,
                ctx.context,
                statistic,
                comparison.relative_distributions.get(statistic),
                comparison.relative_estimates.get(statistic),
                comparison.noise_threshold,
                ctx.size,
            )
        });
    }

    fn line_comparison(
        &mut self,
        ctx: PlotContext<'_>,
        formatter: &ValueFormatter,
        all_curves: &[(&BenchmarkId, &Benchmark)],
        value_type: ValueType,
    ) {
        let max = all_curves
            .iter()
            .map(|(_, bench)| bench.latest_stats.estimates.typical().point_estimate)
            .fold(f64::NAN, f64::max);

        let mut dummy = [1.0];
        let unit = formatter.scale_values(max, &mut dummy);

        let mut series_data = vec![];

        let mut function_id_to_benchmarks = LinkedHashMap::new();
        for (id, bench) in all_curves {
            function_id_to_benchmarks
                .entry(&id.function_id)
                .or_insert_with(Vec::new)
                .push((*id, *bench))
        }

        for (key, group) in function_id_to_benchmarks {
            // Unwrap is fine here because the caller shouldn't call this with non-numeric IDs.
            let mut tuples: Vec<_> = group
                .into_iter()
                .map(|(id, bench)| {
                    let x = id.as_number().unwrap();
                    let y = bench.latest_stats.estimates.typical().point_estimate;

                    (x, y)
                })
                .collect();
            tuples.sort_by(|&(ax, _), &(bx, _)| {
                ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Less)
            });
            let function_name = key.as_ref();
            let (xs, mut ys): (Vec<_>, Vec<_>) = tuples.into_iter().unzip();
            formatter.scale_values(max, &mut ys);
            series_data.push((function_name, xs, ys));
        }

        let lines: Vec<_> = series_data
            .iter()
            .map(|(name, xs, ys)| (*name, LineCurve { xs, ys }))
            .collect();

        self.backend.line_comparison(
            ctx.line_comparison_path(),
            ctx.id.as_title(),
            &unit,
            value_type,
            ctx.context.plot_config.summary_scale,
            &lines,
        );
    }

    fn violin(
        &mut self,
        ctx: PlotContext<'_>,
        formatter: &ValueFormatter,
        all_curves: &[(&BenchmarkId, &Benchmark)],
    ) {
        let mut kdes = all_curves
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

        let lines = kdes
            .iter()
            .map(|(name, xs, ys)| (*name, LineCurve { xs, ys }))
            .collect::<Vec<_>>();

        self.backend.violin(
            ctx.violin_path(),
            ctx.id.as_title(),
            &unit,
            ctx.context.plot_config.summary_scale,
            &lines,
        )
    }

    fn t_test(&mut self, ctx: PlotContext<'_>, comparison: &ComparisonData) {
        self.t_test_plot(
            ctx,
            comparison,
            ctx.context.report_path(ctx.id, "change/t-test.svg"),
        )
    }

    fn history(
        &mut self,
        ctx: PlotContext<'_>,
        upper_bound: &[f64],
        point_estimate: &[f64],
        lower_bound: &[f64],
        ids: &[String],
        unit: &str,
    ) {
        self.history_plot(
            ctx,
            ctx.size.unwrap(),
            upper_bound,
            point_estimate,
            lower_bound,
            ids,
            ctx.context.report_path(ctx.id, "history.svg"),
            unit,
        )
    }

    fn wait(&mut self) {
        self.backend.wait();
    }
}
