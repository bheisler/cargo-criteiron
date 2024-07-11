use crate::plot::plotters_backend::{Colors, DEFAULT_FONT, POINT_SIZE, SIZE};
use crate::plot::{FilledCurve, Line, Points, Size};
use crate::report::BenchmarkId;
use plotters::data::float::pretty_print_float;
use plotters::prelude::*;
use std::path::PathBuf;

pub fn regression(
    colors: &Colors,
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
) {
    let size = size.unwrap_or(SIZE);
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let mut cb = ChartBuilder::on(&root_area);
    if !is_thumbnail {
        cb.caption(id.as_title(), (DEFAULT_FONT, 20));
    }

    let x_range = plotters::data::fitting_range(sample.xs.iter());
    let y_range = plotters::data::fitting_range(sample.ys.iter());

    let mut chart = cb
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .x_desc(x_label)
        .y_desc(format!("Total sample time ({})", unit))
        .x_label_formatter(&|x| pretty_print_float(x * x_scale, true))
        .light_line_style(TRANSPARENT)
        .draw()
        .unwrap();

    chart
        .draw_series(
            (sample.to_points())
                .map(|(x, y)| Circle::new((x, y), POINT_SIZE, colors.current_sample.filled())),
        )
        .unwrap()
        .label("Sample")
        .legend(|(x, y)| Circle::new((x + 10, y), POINT_SIZE, colors.current_sample.filled()));

    chart
        .draw_series(std::iter::once(PathElement::new(
            regression.to_line_vec(),
            colors.current_sample,
        )))
        .unwrap()
        .label("Linear regression")
        .legend(|(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                colors.current_sample.filled().stroke_width(2),
            )
        });

    chart
        .draw_series(std::iter::once(Polygon::new(
            vec![
                (confidence_interval.xs[0], confidence_interval.ys_2[0]),
                (confidence_interval.xs[1], confidence_interval.ys_1[1]),
                (confidence_interval.xs[1], confidence_interval.ys_2[1]),
            ],
            colors.current_sample.mix(0.25).filled(),
        )))
        .unwrap()
        .label("Confidence interval")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.current_sample.mix(0.25).filled(),
            )
        });

    if !is_thumbnail {
        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .draw()
            .unwrap();
    }
}

pub fn regression_comparison(
    colors: &Colors,
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
) {
    let y_max = current_regression.end.y.max(base_regression.end.y);
    let size = size.unwrap_or(SIZE);
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let mut cb = ChartBuilder::on(&root_area);
    if !is_thumbnail {
        cb.caption(id.as_title(), (DEFAULT_FONT, 20));
    }

    let mut chart = cb
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(0.0..current_regression.end.x, 0.0..y_max)
        .unwrap();

    chart
        .configure_mesh()
        .x_desc(x_label)
        .y_desc(format!("Total sample time ({})", unit))
        .x_label_formatter(&|x| pretty_print_float(x * x_scale, true))
        .light_line_style(TRANSPARENT)
        .draw()
        .unwrap();

    chart
        .draw_series(vec![
            PathElement::new(base_regression.to_line_vec(), colors.previous_sample).into_dyn(),
            Polygon::new(
                vec![
                    (
                        base_confidence_interval.xs[0],
                        base_confidence_interval.ys_2[0],
                    ),
                    (
                        base_confidence_interval.xs[1],
                        base_confidence_interval.ys_1[1],
                    ),
                    (
                        base_confidence_interval.xs[1],
                        base_confidence_interval.ys_2[1],
                    ),
                ],
                colors.previous_sample.mix(0.25).filled(),
            )
            .into_dyn(),
        ])
        .unwrap()
        .label("Base Sample")
        .legend(|(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                colors.previous_sample.filled().stroke_width(2),
            )
        });

    chart
        .draw_series(vec![
            PathElement::new(current_regression.to_line_vec(), colors.current_sample).into_dyn(),
            Polygon::new(
                vec![
                    (
                        current_confidence_interval.xs[0],
                        current_confidence_interval.ys_2[0],
                    ),
                    (
                        current_confidence_interval.xs[1],
                        current_confidence_interval.ys_1[1],
                    ),
                    (
                        current_confidence_interval.xs[1],
                        current_confidence_interval.ys_2[1],
                    ),
                ],
                colors.current_sample.mix(0.25).filled(),
            )
            .into_dyn(),
        ])
        .unwrap()
        .label("New Sample")
        .legend(|(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                colors.current_sample.filled().stroke_width(2),
            )
        });

    if !is_thumbnail {
        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .draw()
            .unwrap();
    }
}
