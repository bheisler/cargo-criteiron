use crate::plot::plotters_backend::{Colors, DEFAULT_FONT, POINT_SIZE, SIZE};
use crate::plot::{Points, Size};
use crate::report::BenchmarkId;
use crate::stats::univariate::Sample;
use plotters::data::float::pretty_print_float;
use plotters::prelude::*;
use std::path::PathBuf;

pub fn iteration_times(
    colors: &Colors,
    id: &BenchmarkId,
    size: Option<Size>,
    path: PathBuf,
    unit: &str,
    is_thumbnail: bool,
    current_times: Points,
    base_times: Option<Points>,
) {
    let size = size.unwrap_or(SIZE);
    let root_area = SVGBackend::new(&path, size.into()).into_drawing_area();

    let mut cb = ChartBuilder::on(&root_area);

    let (x_range, y_range) = if let Some(base) = &base_times {
        let max_x = Sample::new(current_times.xs)
            .max()
            .max(Sample::new(base.xs).max());
        let x_range = (1.0)..(max_x);
        let y_range = plotters::data::fitting_range(current_times.ys.iter().chain(base.ys.iter()));
        (x_range, y_range)
    } else {
        let max_x = Sample::new(current_times.xs).max();
        let x_range = (1.0)..(max_x);
        let y_range = plotters::data::fitting_range(current_times.ys.iter());
        (x_range, y_range)
    };

    let mut chart = cb
        .margin((5).percent())
        .set_label_area_size(LabelAreaPosition::Left, (5).percent_width().min(60))
        .set_label_area_size(LabelAreaPosition::Bottom, (5).percent_height().min(40))
        .build_cartesian_2d(x_range, y_range)
        .unwrap();

    chart
        .configure_mesh()
        .y_desc(format!("Average Iteration Time ({})", unit))
        .x_label_formatter(&|x| pretty_print_float(*x, true))
        .light_line_style(TRANSPARENT)
        .draw()
        .unwrap();

    chart
        .draw_series(
            (current_times.to_points())
                .map(|(x, y)| Circle::new((x, y), POINT_SIZE, colors.current_sample.filled())),
        )
        .unwrap()
        .label("Current")
        .legend(|(x, y)| Circle::new((x + 10, y), POINT_SIZE, colors.current_sample.filled()));

    if let Some(base_times) = base_times {
        chart
            .draw_series(
                (base_times.to_points())
                    .map(|(x, y)| Circle::new((x, y), POINT_SIZE, colors.previous_sample.filled())),
            )
            .unwrap()
            .label("Base")
            .legend(|(x, y)| Circle::new((x + 10, y), POINT_SIZE, colors.previous_sample.filled()));
    }

    if !is_thumbnail {
        cb.caption(id.as_title(), (DEFAULT_FONT, 20));
        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .draw()
            .unwrap();
    }
}
