use crate::connection::{AxisScale, Connection, IncomingMessage, PlotConfiguration};
use crate::model::Model;
use crate::report::{BenchmarkId, Report, ReportContext};
use anyhow::{anyhow, Context, Result};
use std::ffi::OsString;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Structure representing a compiled benchmark executable.
#[derive(Debug)]
pub struct BenchTarget {
    pub name: String,
    pub executable: PathBuf,
}
impl BenchTarget {
    /// Launches this benchmark target with the given additional arguments.
    ///
    /// Opens a localhost socket on an arbitrary port. This port is passed to the target in an
    /// environment variable; recent versions of Criterion.rs will connect to that socket and use
    /// it to communicate information about the status of the run and the measurements taken. Other
    /// benchmark frameworks (or older versions of Criterion.rs) will ignore the port and perform
    /// their benchmarks as they normally do.
    ///
    /// The report will be notified about important events happening with the benchmark and the
    /// model will be updated with the new benchmark IDs and measurements as we go. This function
    /// will block until the benchmark target terminates.
    pub fn execute(
        &self,
        criterion_home: &Path,
        additional_args: &[OsString],
        library_paths: &[PathBuf],
        report: &dyn Report,
        model: &mut Model,
        redirect_stdout: bool,
    ) -> Result<()> {
        let listener = TcpListener::bind("localhost:0")
            .context("Unable to open socket to connect to Criterion.rs")?;
        // listener has to be non-blocking while we wait for connections.
        // Otherwise we'll wait forever if no-one connects to us
        listener
            .set_nonblocking(true)
            .context("Unable to set socket to nonblocking")?;

        let addr = listener
            .local_addr()
            .context("Unable to get local address of socket")?;
        let port = addr.port();

        let mut command = Command::new(&self.executable);
        command
            .arg("--bench")
            .args(additional_args)
            .env(dylib_path_envvar(), dylib_search_path(library_paths)?)
            .env("CRITERION_HOME", criterion_home)
            .env("CARGO_CRITERION_PORT", &port.to_string())
            .stdin(Stdio::null())
            .stdout(if redirect_stdout {
                // If we're printing machine-readable output to stdout, output from the target might
                // interfere with out messages, so intercept it and reprint it to stderr.
                Stdio::piped()
            } else {
                // If not, we might as well let the target see the true stdout.
                Stdio::inherit()
            })
            .stderr(Stdio::inherit());

        debug!("Running '{:?}'", command);

        let mut child = command
            .spawn()
            .with_context(|| format!("Unable to launch bench target {}", self.name))?;

        if redirect_stdout {
            let mut stdout = child.stdout.take().unwrap();
            std::thread::spawn(move || std::io::copy(&mut stdout, &mut std::io::stderr()));
        }

        loop {
            match listener.accept() {
                Ok((socket, _)) => {
                    socket
                        .set_nonblocking(false)
                        .context("Unable to set socket to blocking")?;
                    let conn = Connection::new(socket).with_context(|| {
                        format!("Unable to open connection to bench target {}", self.name)
                    })?;
                    return self.communicate(&mut child, conn, report, criterion_home, model);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection yet, try again in a bit.
                }
                Err(e) => {
                    return Err(e).context("Unable to accept connection to socket");
                }
            };

            match child.try_wait() {
                Err(e) => {
                    return Err(e).context(format!(
                        "Failed to wait for non-Criterion.rs benchmark target {}",
                        self.name
                    ));
                }
                Ok(Some(exit_status)) => {
                    if exit_status.success() {
                        return Ok(());
                    } else {
                        return Err(anyhow!(
                            "Non-Criterion.rs benchmark target {} exited with {}",
                            self.name,
                            exit_status
                        ));
                    }
                }
                Ok(None) => (), // Child still running, keep trying.
            };

            // Wait a bit then poll again.
            std::thread::yield_now();
        }
    }

    /// This function is called when a benchmark connects to the socket. It interacts with the
    /// benchmark target to receive information about the measurements and inform the report and
    /// model about the benchmarks. This function returns when the benchmark target terminates.
    fn communicate(
        &self,
        child: &mut Child,
        mut conn: Connection,
        report: &dyn Report,
        criterion_home: &std::path::Path,
        model: &mut Model,
    ) -> Result<()> {
        let mut context = ReportContext {
            output_directory: criterion_home.join("reports"),
            plot_config: PlotConfiguration {
                summary_scale: AxisScale::Linear,
            },
        };
        let mut any_from_group_executed = false;
        loop {
            let message_opt = conn.recv().with_context(|| {
                format!(
                    "Failed to receive message from Criterion.rs benchmark target {}",
                    self.name
                )
            })?;

            if let Some(message) = message_opt {
                match message {
                    IncomingMessage::BeginningBenchmarkGroup { group } => {
                        any_from_group_executed = false;
                        model.check_benchmark_group(&self.name, &group);
                    }
                    IncomingMessage::FinishedBenchmarkGroup { group } => {
                        let benchmark_group = model.add_benchmark_group(&self.name, &group);
                        {
                            let formatter = crate::value_formatter::ValueFormatter::new(&mut conn);
                            report.summarize(&context, &group, benchmark_group, &formatter);
                            if any_from_group_executed {
                                report.group_separator();
                            }
                        }
                    }
                    IncomingMessage::BeginningBenchmark { id } => {
                        any_from_group_executed = true;
                        let mut id = id.into();
                        model.add_benchmark_id(&self.name, &mut id);
                        self.run_benchmark(&mut conn, report, model, id, &mut context)?;
                    }
                    IncomingMessage::SkippingBenchmark { id } => {
                        let mut id = id.into();
                        model.add_benchmark_id(&self.name, &mut id);
                    }
                    other => panic!("Unexpected message {:?}", other),
                }
            }

            match child.try_wait() {
                Err(e) => {
                    return Err(e).context(format!(
                        "Failed to poll Criterion.rs child process {}",
                        self.name
                    ));
                }
                Ok(Some(exit_status)) => {
                    if exit_status.success() {
                        return Ok(());
                    } else {
                        return Err(anyhow!(
                            "Criterion.rs benchmark target {} exited with {}",
                            self.name,
                            exit_status
                        ));
                    }
                }
                Ok(None) => continue,
            };
        }
    }

    /// Helper function for communicating with the benchmark target about a single benchmark.
    fn run_benchmark(
        &self,
        conn: &mut Connection,
        report: &dyn Report,
        model: &mut Model,
        id: BenchmarkId,
        context: &mut ReportContext,
    ) -> Result<()> {
        report.benchmark_start(&id, context);

        loop {
            let message = conn.recv().with_context(|| {
                format!(
                    "Failed to receive message from Criterion.rs benchmark {}",
                    self.name
                )
            })?;
            let message = match message {
                Some(message) => message,
                None => return Ok(()),
            };
            match message {
                IncomingMessage::Warmup { nanos } => {
                    report.warmup(&id, context, nanos);
                }
                IncomingMessage::MeasurementStart {
                    sample_count,
                    estimate_ns,
                    iter_count,
                } => {
                    report.measurement_start(&id, context, sample_count, estimate_ns, iter_count);
                }
                IncomingMessage::MeasurementComplete {
                    iters,
                    times,
                    plot_config,
                    sampling_method,
                    benchmark_config,
                } => {
                    context.plot_config = plot_config;
                    report.analysis(&id, context);

                    let avg_values: Vec<f64> = iters
                        .iter()
                        .zip(times.iter())
                        .map(|(iter, time)| *time / *iter)
                        .collect();

                    if times.iter().any(|&f| f == 0.0) {
                        error!("At least one measurement of benchmark {} took zero time per \
                        iteration. This should not be possible. If using iter_custom, please verify \
                        that your routine is correctly measured.", id.as_title());
                        // Create and drop a value formatter because the benchmark will be waiting
                        // for that
                        crate::value_formatter::ValueFormatter::new(conn);
                        return Ok(());
                    }

                    let saved_stats = model.get_last_sample(&id).cloned();

                    let benchmark_config: crate::analysis::BenchmarkConfig =
                        benchmark_config.into();

                    let measured_data = crate::analysis::analysis(
                        &benchmark_config,
                        id.throughput.clone(),
                        crate::analysis::MeasuredValues {
                            iteration_count: &iters,
                            sample_values: &times,
                            avg_values: &avg_values,
                        },
                        saved_stats.as_ref().map(|stats| {
                            let measured_values = crate::analysis::MeasuredValues {
                                iteration_count: &stats.iterations,
                                sample_values: &stats.values,
                                avg_values: &stats.avg_values,
                            };
                            (measured_values, &stats.estimates)
                        }),
                        sampling_method,
                    );

                    if let Err(e) = model.benchmark_complete(&id, &measured_data) {
                        error!(
                            "Failed to save results for target {} benchmark {}: {}",
                            self.name,
                            id.as_title(),
                            e
                        );
                    }

                    {
                        let formatter = crate::value_formatter::ValueFormatter::new(conn);
                        report.measurement_complete(&id, context, &measured_data, &formatter);

                        match model.load_history(&id) {
                            Ok(history) => report.history(context, &id, &history, &formatter),
                            Err(e) => error!("Failed to load historical data: {:?}", e),
                        }
                    }
                    return Ok(());
                }
                other => panic!("Unexpected message {:?}", other),
            }
        }
    }
}

// This dylib path logic is adapted from Cargo.
pub fn dylib_path_envvar() -> &'static str {
    if cfg!(windows) {
        "PATH"
    } else if cfg!(target_os = "macos") {
        "DYLD_FALLBACK_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

pub fn dylib_path() -> Vec<PathBuf> {
    match std::env::var_os(dylib_path_envvar()) {
        Some(var) => std::env::split_paths(&var).collect(),
        None => Vec::new(),
    }
}

fn dylib_search_path(linked_paths: &[PathBuf]) -> Result<OsString> {
    let mut dylib_path = dylib_path();
    let dylib_path_is_empty = dylib_path.is_empty();
    dylib_path.extend(linked_paths.iter().cloned());
    if cfg!(target_os = "macos") && dylib_path_is_empty {
        if let Some(home) = std::env::var_os("HOME") {
            dylib_path.push(PathBuf::from(home).join("lib"));
        }
        dylib_path.push(PathBuf::from("/usr/local/lib"));
        dylib_path.push(PathBuf::from("/usr/lib"));
    }
    std::env::join_paths(&dylib_path)
        .with_context(|| format!("Failed to join dynamic lib search paths together. Does {} have an unterminated quote character? Paths:\n{:?}", dylib_path_envvar(), &dylib_path))
}
