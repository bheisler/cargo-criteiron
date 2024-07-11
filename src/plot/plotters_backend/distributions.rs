use crate::estimate::Statistic;
use crate::plot::plotters_backend::{Colors, DEFAULT_FONT, SIZE};
use crate::plot::{FilledCurve, Line, LineCurve, Rectangle as RectangleArea, Size};
use crate::report::BenchmarkId;
use crate::stats::univariate::Sample;
use plotters::data::float::pretty_print_float;
use plotters::prelude::*;
use std::path::PathBuf;

pub fn abs_distribution(
    colors: &Colors,
    id: &BenchmarkId,
    statistic: Statistic,
    size: Option<Size>,
    path: PathBuf,

    x_unit: &str,
    distribution_curve: LineCurve,
    bootstrap_area: FilledCurve,
    point_estimate: Line,
) {
    let root_area = SVGBackend::new(&path, size.unwrap_or(SIZE).into()).into_drawing_area();

    let x_range = plotters::data::fitting_range(distribution_curve.xs.iter());
    let mut y_range = plotters::data::fitting_range(distribution_curve.ys.iter());

    y_range.end *= 1.1;

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .caption(
            format!("{}:{}", id.as_title(), statistic),
            (DEFAULT_FONT, 20),
        )
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .x_desc(format!("Average time ({})", x_unit))
        .y_desc("Density (a.u.)")
        .x_label_formatter(&|&v| pretty_print_float(v, true))
        .y_label_formatter(&|&v| pretty_print_float(v, true))
        .draw()
        .unwrap();

    chart
        .draw_series(LineSeries::new(
            distribution_curve.to_points(),
            colors.current_sample,
        ))
        .unwrap()
        .label("Bootstrap distribution")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    chart
        .draw_series(AreaSeries::new(
            bootstrap_area.to_points(),
            0.0,
            colors.current_sample.mix(0.25).filled().stroke_width(3),
        ))
        .unwrap()
        .label("Confidence interval")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.current_sample.mix(0.25).filled(),
            )
        });

    chart
        .draw_series(std::iter::once(PathElement::new(
            point_estimate.to_line_vec(),
            colors.current_sample.filled().stroke_width(3),
        )))
        .unwrap()
        .label("Point estimate")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .draw()
        .unwrap();
}

pub fn rel_distribution(
    colors: &Colors,
    id: &BenchmarkId,
    statistic: Statistic,
    size: Option<Size>,
    path: PathBuf,

    distribution_curve: LineCurve,
    confidence_interval: FilledCurve,
    point_estimate: Line,
    noise_threshold: RectangleArea,
) {
    let xs_ = Sample::new(distribution_curve.xs);
    let x_min = xs_.min();
    let x_max = xs_.max();

    let y_range = plotters::data::fitting_range(distribution_curve.ys);
    let root_area = SVGBackend::new(&path, size.unwrap_or(SIZE).into()).into_drawing_area();

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .caption(
            format!("{}:{}", id.as_title(), statistic),
            (DEFAULT_FONT, 20),
        )
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(x_min..x_max, y_range.clone())
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .x_desc("Relative change (%)")
        .y_desc("Density (a.u.)")
        .x_label_formatter(&|&v| pretty_print_float(v, true))
        .y_label_formatter(&|&v| pretty_print_float(v, true))
        .draw()
        .unwrap();

    chart
        .draw_series(LineSeries::new(
            distribution_curve.to_points(),
            colors.current_sample,
        ))
        .unwrap()
        .label("Bootstrap distribution")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    chart
        .draw_series(AreaSeries::new(
            confidence_interval.to_points(),
            0.0,
            colors.current_sample.mix(0.25).filled().stroke_width(3),
        ))
        .unwrap()
        .label("Confidence interval")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.current_sample.mix(0.25).filled(),
            )
        });

    chart
        .draw_series(std::iter::once(PathElement::new(
            point_estimate.to_line_vec(),
            colors.current_sample.filled().stroke_width(3),
        )))
        .unwrap()
        .label("Point estimate")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    chart
        .draw_series(std::iter::once(Rectangle::new(
            [
                (noise_threshold.left, y_range.start),
                (noise_threshold.right, y_range.end),
            ],
            colors.previous_sample.mix(0.1).filled(),
        )))
        .unwrap()
        .label("Noise threshold")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.previous_sample.mix(0.25).filled(),
            )
        });
    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .draw()
        .unwrap();
}
