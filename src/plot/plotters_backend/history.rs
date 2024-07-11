use crate::plot::plotters_backend::{Colors, DEFAULT_FONT};
use crate::plot::{FilledCurve, LineCurve, Size};
use plotters::data::float::pretty_print_float;
use plotters::prelude::*;
use std::path::PathBuf;

pub fn history(
    colors: &Colors,
    title: &str,
    size: Size,
    path: PathBuf,
    point_estimate: LineCurve,
    confidence_interval: FilledCurve,
    ids: &[String],
    unit: &str,
) {
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let x_range = plotters::data::fitting_range(point_estimate.xs.iter());
    let mut y_range = plotters::data::fitting_range(
        confidence_interval
            .ys_1
            .iter()
            .chain(confidence_interval.ys_2.iter()),
    );

    y_range.end *= 1.1;
    y_range.start /= 1.1;

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .caption(format!("{} History", title), (DEFAULT_FONT, 20))
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .y_desc(format!("Average time ({})", unit))
        .x_desc("History")
        .x_label_formatter(&|&v| ids[v as usize].clone())
        .y_label_formatter(&|&v| pretty_print_float(v, true))
        .x_labels(ids.len())
        .draw()
        .unwrap();

    chart
        .draw_series(LineSeries::new(
            point_estimate.to_points(),
            colors.current_sample,
        ))
        .unwrap()
        .label("Point estimate")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    let polygon_points: Vec<(f64, f64)> = confidence_interval
        .xs
        .iter()
        .copied()
        .zip(confidence_interval.ys_1.iter().copied())
        .chain(
            confidence_interval
                .xs
                .iter()
                .rev()
                .copied()
                .zip(confidence_interval.ys_2.iter().rev().copied()),
        )
        .collect();

    chart
        .draw_series(std::iter::once(Polygon::new(
            polygon_points,
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

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .draw()
        .unwrap();
}
