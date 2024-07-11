use crate::plot::plotters_backend::{Colors, DEFAULT_FONT, POINT_SIZE, SIZE};
use crate::plot::{FilledCurve, Line, Points, Size, VerticalLine};
use crate::report::BenchmarkId;
use crate::stats::univariate::Sample;
use plotters::data::float::pretty_print_float;
use plotters::prelude::*;
use plotters::style::RGBAColor;
use std::path::PathBuf;

pub fn pdf_full(
    colors: &Colors,
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
) {
    let (low_severe, low_mild, high_mild, high_severe) = fences;
    let (not_outlier, mild, severe) = points;
    let xs_ = Sample::new(pdf.xs);

    let size = size.unwrap_or(SIZE);
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let range = plotters::data::fitting_range(pdf.ys_1.iter());

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .caption(id.as_title(), (DEFAULT_FONT, 20))
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Right, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(xs_.min()..xs_.max(), 0.0..max_iters)
        .unwrap()
        .set_secondary_coord(xs_.min()..xs_.max(), 0.0..range.end);

    chart
        .configure_mesh()
        .disable_mesh()
        .y_desc(y_label)
        .x_desc(format!("Average Time ({})", unit))
        .x_label_formatter(&|&x| pretty_print_float(x, true))
        .y_label_formatter(&|&y| pretty_print_float(y * y_scale, true))
        .draw()
        .unwrap();

    chart
        .configure_secondary_axes()
        .y_desc("Density (a.u.)")
        .x_label_formatter(&|&x| pretty_print_float(x, true))
        .y_label_formatter(&|&y| pretty_print_float(y, true))
        .draw()
        .unwrap();

    chart
        .draw_secondary_series(AreaSeries::new(
            pdf.to_points(),
            0.0,
            colors.current_sample.mix(0.5).filled(),
        ))
        .unwrap()
        .label("PDF")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.current_sample.mix(0.5).filled(),
            )
        });

    chart
        .draw_series(std::iter::once(PathElement::new(
            mean.to_line_vec(max_iters),
            colors.not_an_outlier,
        )))
        .unwrap()
        .label("Mean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.not_an_outlier));

    chart
        .draw_series(vec![
            PathElement::new(low_mild.to_line_vec(max_iters), colors.mild_outlier),
            PathElement::new(high_mild.to_line_vec(max_iters), colors.mild_outlier),
            PathElement::new(low_severe.to_line_vec(max_iters), colors.severe_outlier),
            PathElement::new(high_severe.to_line_vec(max_iters), colors.severe_outlier),
        ])
        .unwrap();

    let mut draw_data_point_series = |points: Points, color: RGBAColor, name: &str| {
        chart
            .draw_series(
                (points.to_points()).map(|(x, y)| Circle::new((x, y), POINT_SIZE, color.filled())),
            )
            .unwrap()
            .label(name)
            .legend(move |(x, y)| Circle::new((x + 10, y), POINT_SIZE, color.filled()));
    };

    draw_data_point_series(
        not_outlier,
        colors.not_an_outlier.to_rgba(),
        "\"Clean\" sample",
    );
    draw_data_point_series(mild, colors.mild_outlier.to_rgba(), "Mild outliers");
    draw_data_point_series(severe, colors.severe_outlier.to_rgba(), "Severe outliers");
    chart.configure_series_labels().draw().unwrap();
}

pub fn pdf_thumbnail(
    colors: &Colors,
    size: Option<Size>,
    path: PathBuf,
    unit: &str,
    mean: Line,
    pdf: FilledCurve,
) {
    let xs_ = Sample::new(pdf.xs);
    let ys_ = Sample::new(pdf.ys_1);

    let y_limit = ys_.max() * 1.1;

    let size = size.unwrap_or(SIZE);
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let mut chart = ChartBuilder::on(&root_area)
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(xs_.min()..xs_.max(), 0.0..y_limit)
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .y_desc("Density (a.u.)")
        .x_desc(format!("Average Time ({})", unit))
        .x_label_formatter(&|&x| pretty_print_float(x, true))
        .y_label_formatter(&|&y| pretty_print_float(y, true))
        .x_labels(5)
        .draw()
        .unwrap();

    chart
        .draw_series(AreaSeries::new(
            pdf.to_points(),
            0.0,
            colors.current_sample.mix(0.25).filled(),
        ))
        .unwrap();

    chart
        .draw_series(std::iter::once(PathElement::new(
            mean.to_line_vec(),
            colors.current_sample.filled().stroke_width(2),
        )))
        .unwrap();
}

pub fn pdf_comparison(
    colors: &Colors,
    id: &BenchmarkId,
    size: Option<Size>,
    path: PathBuf,
    is_thumbnail: bool,
    unit: &str,
    current_mean: Line,
    current_pdf: FilledCurve,
    base_mean: Line,
    base_pdf: FilledCurve,
) {
    let x_range = plotters::data::fitting_range(base_pdf.xs.iter().chain(current_pdf.xs.iter()));
    let y_range =
        plotters::data::fitting_range(base_pdf.ys_1.iter().chain(current_pdf.ys_1.iter()));

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
        .build_cartesian_2d(x_range, y_range.clone())
        .unwrap();

    chart
        .configure_mesh()
        .disable_mesh()
        .y_desc("Density (a.u.)")
        .x_desc(format!("Average Time ({})", unit))
        .x_label_formatter(&|&x| pretty_print_float(x, true))
        .y_label_formatter(&|&y| pretty_print_float(y, true))
        .x_labels(5)
        .draw()
        .unwrap();

    chart
        .draw_series(AreaSeries::new(
            base_pdf.to_points(),
            y_range.start,
            colors.previous_sample.mix(0.5).filled(),
        ))
        .unwrap()
        .label("Base PDF")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.previous_sample.mix(0.5).filled(),
            )
        });

    chart
        .draw_series(AreaSeries::new(
            current_pdf.to_points(),
            y_range.start,
            colors.current_sample.mix(0.5).filled(),
        ))
        .unwrap()
        .label("New PDF")
        .legend(|(x, y)| {
            Rectangle::new(
                [(x, y - 5), (x + 20, y + 5)],
                colors.current_sample.mix(0.5).filled(),
            )
        });

    chart
        .draw_series(std::iter::once(PathElement::new(
            base_mean.to_line_vec(),
            colors.previous_sample.filled().stroke_width(2),
        )))
        .unwrap()
        .label("Base Mean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.previous_sample));

    chart
        .draw_series(std::iter::once(PathElement::new(
            current_mean.to_line_vec(),
            colors.current_sample.filled().stroke_width(2),
        )))
        .unwrap()
        .label("New Mean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors.current_sample));

    if !is_thumbnail {
        chart.configure_series_labels().draw().unwrap();
    }
}
