use anyhow::{Context, Result};
use std::borrow::ToOwned;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[rustfmt::skip]
const DARK_BLUE: Color = Color { r: 31, g: 120, b: 180 };
#[rustfmt::skip]
const DARK_ORANGE: Color = Color { r: 5, g: 127, b: 0 };
#[rustfmt::skip]
const DARK_RED: Color = Color { r: 7, g: 26, b: 28 };

const NUM_COLORS: usize = 8;
#[rustfmt::skip]
static COMPARISON_COLORS: [Color; NUM_COLORS] = [
    Color { r: 178, g: 34, b: 34 },
    Color { r: 46, g: 139, b: 87 },
    Color { r: 0, g: 139, b: 139 },
    Color { r: 255, g: 215, b: 0 },
    Color { r: 0, g: 0, b: 139 },
    Color { r: 220, g: 20, b: 60 },
    Color { r: 139, g: 0, b: 139 },
    Color { r: 0, g: 255, b: 127 },
];

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Colors {
    /// The color used for the current sample. Defaults to DARK_BLUE
    pub current_sample: Color,
    /// The color used for the previous sample. Defaults to DARK_RED
    pub previous_sample: Color,
    /// The color used for values that are not outliers. Defaults to DARK_BLUE
    pub not_an_outlier: Color,
    /// The color used for values that are mild outliers. Defaults to DARK_ORANGE
    pub mild_outlier: Color,
    /// The color used for values that are severe outliers. Defaults to DARK_RED
    pub severe_outlier: Color,
    /// Sequence of colors used for the line chart. Defaults to COMPARISON_COLORS
    pub comparison_colors: Vec<Color>,
}
impl Default for Colors {
    fn default() -> Self {
        Self {
            current_sample: DARK_BLUE,
            previous_sample: DARK_RED,
            not_an_outlier: DARK_BLUE,
            mild_outlier: DARK_ORANGE,
            severe_outlier: DARK_RED,
            comparison_colors: COMPARISON_COLORS.to_vec(),
        }
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
/// Struct to hold the various configuration settings that we can read from the TOML config file.
struct TomlConfig {
    /// Path to output directory
    pub criterion_home: Option<PathBuf>,
    /// Output format
    pub output_format: Option<String>,
    /// Plotting backend
    pub plotting_backend: Option<String>,

    /// The colors used for the charts. Users may wish to override this to accommodate
    /// colorblindness, or just to make things look prettier.
    pub colors: Colors,
}

#[derive(Debug)]
pub enum OutputFormat {
    Criterion,
    Quiet,
    Verbose,
    Bencher,
}
impl OutputFormat {
    fn from_str(s: &str) -> OutputFormat {
        match s {
            "criterion" => OutputFormat::Criterion,
            "quiet" => OutputFormat::Quiet,
            "verbose" => OutputFormat::Verbose,
            "bencher" => OutputFormat::Bencher,
            other => panic!("Unknown output format string: {}", other),
        }
    }
}

#[derive(Debug)]
pub enum TextColor {
    Always,
    Never,
    Auto,
}
impl TextColor {
    fn from_str(s: &str) -> TextColor {
        match s {
            "always" => TextColor::Always,
            "never" => TextColor::Never,
            "auto" => TextColor::Auto,
            other => panic!("Unknown text color string: {}", other),
        }
    }
}

#[derive(Debug)]
pub enum PlottingBackend {
    Gnuplot,
    Plotters,
    Auto,
    Disabled,
}
impl PlottingBackend {
    fn from_str(s: &str) -> PlottingBackend {
        match s {
            "gnuplot" => PlottingBackend::Gnuplot,
            "plotters" => PlottingBackend::Plotters,
            "auto" => PlottingBackend::Auto,
            "disabled" => PlottingBackend::Disabled,
            other => panic!("Unknown plotting backend: {}", other),
        }
    }
}

#[derive(Debug)]
pub enum MessageFormat {
    Json,
    OpenMetrics,
}
impl MessageFormat {
    fn from_str(s: &str) -> MessageFormat {
        match s {
            "json" => MessageFormat::Json,
            "openmetrics" => MessageFormat::OpenMetrics,
            other => panic!("Unknown message format: {}", other),
        }
    }
}

/// Struct to hold the various configuration settings for cargo-criterion itself.
#[derive(Debug)]
pub struct SelfConfig {
    /// The path to the output directory
    pub criterion_home: PathBuf,
    /// Should we run the benchmarks?
    pub do_run: bool,
    /// Should we fail immediately if a benchmark target fails, or continue with the others?
    pub do_fail_fast: bool,
    /// How should the CLI output be formatted
    pub output_format: OutputFormat,
    /// Should we print the output in color?
    pub text_color: TextColor,
    /// Which plotting backend to use?
    pub plotting_backend: PlottingBackend,
    /// Should we compile the benchmarks in debug mode (true) or release mode (false, default)
    pub debug_build: bool,
    /// Should we print machine-readable output, and if so, in what format?
    pub message_format: Option<MessageFormat>,
    /// The colors to use for charts.
    pub colors: Colors,
    // An optional identifier used to identify this run in the history reports.
    pub history_id: Option<String>,
    // An optional description used to describe this run in the history reports.
    pub history_description: Option<String>,
}

/// Overall struct that represents all of the configuration data for this run.
#[derive(Debug)]
pub struct FullConfig {
    /// The config settings for cargo-criterion
    pub self_config: SelfConfig,
    /// The arguments we pass through to cargo bench
    pub cargo_args: Vec<OsString>,
    /// The additional arguments we pass through to the benchmark executables
    pub additional_args: Vec<OsString>,
}

/// Call `cargo criterion` and parse the output to get the path to the target directory.
fn get_target_directory_from_metadata() -> Result<PathBuf> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()?;

    #[derive(Deserialize)]
    struct MetadataMessage {
        target_directory: String,
    }

    let message: MetadataMessage = serde_json::from_reader(std::io::Cursor::new(out.stdout))?;
    let path = PathBuf::from(message.target_directory);
    Ok(path)
}

/// Parse the command-line arguments, load the criterion.toml config file, and generate a
/// configuration object used for the rest of the run.
#[cfg_attr(feature = "cargo-clippy", allow(clippy::or_fun_call))]
pub fn configure() -> Result<FullConfig, anyhow::Error> {
    use clap::{App, AppSettings, Arg};

    let matches = App::new("cargo-criterion")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Execute, analyze and report on benchmarks of a local package")
        .bin_name("cargo criterion")
        .settings(&[
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::TrailingVarArg,
        ])
        .arg(
            Arg::with_name("lib")
                .long("--lib")
                .help("Benchmark only this package's library"),
        )
        .arg(
            Arg::with_name("bin")
                .long("--bin")
                .takes_value(true)
                .value_name("NAME")
                .multiple(true)
                .help("Benchmark only the specified binary"),
        )
        .arg(
            Arg::with_name("bins")
                .long("--bins")
                .help("Benchmark all binaries"),
        )
        .arg(
            Arg::with_name("example")
                .long("--example")
                .takes_value(true)
                .value_name("NAME")
                .multiple(true)
                .help("Benchmark only the specified example"),
        )
        .arg(
            Arg::with_name("examples")
                .long("--examples")
                .help("Benchmark all examples"),
        )
        .arg(
            Arg::with_name("test")
                .long("--test")
                .takes_value(true)
                .value_name("NAME")
                .multiple(true)
                .help("Benchmark only the specified test target"),
        )
        .arg(
            Arg::with_name("tests")
                .long("--tests")
                .help("Benchmark all tests"),
        )
        .arg(
            Arg::with_name("bench")
                .long("--bench")
                .takes_value(true)
                .value_name("NAME")
                .multiple(true)
                .help("Benchmark only the specified bench target"),
        )
        .arg(
            Arg::with_name("benches")
                .long("--benches")
                .help("Benchmark all benches"),
        )
        .arg(
            Arg::with_name("all-targets")
                .long("--all-targets")
                .help("Benchmark all targets"),
        )
        .arg(
            Arg::with_name("no-run")
                .long("--no-run")
                .help("Compile, but don't run benchmarks"),
        )
        .arg(
            Arg::with_name("package")
                .long("--package")
                .short("p")
                .takes_value(true)
                .value_name("SPEC")
                .multiple(true)
                .help("Package to run benchmarks for"),
        )
        .arg(
            Arg::with_name("all")
                .long("--all")
                .help("Alias for --workspace (deprecated)"),
        )
        .arg(
            Arg::with_name("workspace")
                .long("--workspace")
                .help("Benchmark all packages in the workspace"),
        )
        .arg(
            Arg::with_name("exclude")
                .long("--exclude")
                .takes_value(true)
                .value_name("SPEC")
                .multiple(true)
                .help("Exclude packages from the benchmark"),
        )
        .arg(
            Arg::with_name("jobs")
                .long("--jobs")
                .short("j")
                .takes_value(true)
                .value_name("N")
                .help("Number of parallel jobs, defaults to # of CPUs"),
        )
        .arg(
            Arg::with_name("features")
                .long("--features")
                .takes_value(true)
                .value_name("FEATURE")
                .multiple(true)
                .help("Space-separated list of features to activate"),
        )
        .arg(
            Arg::with_name("all-features")
                .long("--all-features")
                .help("Activate all available features"),
        )
        .arg(
            Arg::with_name("no-default-features")
                .long("--no-default-features")
                .help("Do not activate the 'default' feature"),
        )
        .arg(
            Arg::with_name("target")
                .long("--target")
                .takes_value(true)
                .value_name("TRIPLE")
                .help("Build for the target triple"),
        )
        .arg(
            Arg::with_name("target-dir")
                .long("--target-dir")
                .takes_value(true)
                .value_name("DIRECTORY")
                .help("Directory for all generated artifacts"),
        )
        .arg(
            Arg::with_name("manifest-path")
                .long("--manifest-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to Cargo.toml"),
        )
        .arg(
            Arg::with_name("criterion-manifest-path")
                .long("--criterion-manifest-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to criterion.toml"),
        )
        .arg(
            Arg::with_name("no-fail-fast")
                .long("--no-fail-fast")
                .help("Run all benchmarks regardless of failure"),
        )
        .arg(
            Arg::with_name("debug")
                .long("--debug")
                .help("Build the benchmarks in debug mode.")
                .long_help(
"This option will compile the benchmarks with the 'test' profile, which by default means they will
not be optimized. This may be useful to reduce compile time when benchmarking code written in a
different language (eg. external C modules).

Note however that it will tend to increase the measurement overhead, as the measurement loops 
in the benchmark will not be optimized either. This may result in less-accurate measurements.
")
        )
        .arg(
            Arg::with_name("output-format")
                .long("output-format")
                .takes_value(true)
                .possible_values(&["criterion", "quiet", "verbose", "bencher"])
                .hide_default_value(true)
                .hide_possible_values(true)
                .help("Change the CLI output format. Possible values are criterion, quiet, verbose, bencher.")
                .long_help(
"Change the CLI output format. Possible values are [criterion, quiet, verbose, bencher].

criterion: Prints confidence intervals for measurement and throughput, and indicates whether a \
change was detected from the previous run. The default.

quiet: Like criterion, but does not indicate changes. Useful for simply presenting output numbers, \
eg. on a library's README.

verbose: Like criterion, but prints additional statistics.

bencher: Emulates the output format of the bencher crate and nightly-only libtest benchmarks.
")
        )
        .arg(
            Arg::with_name("plotting-backend")
                .long("plotting-backend")
                .takes_value(true)
                .possible_values(&["gnuplot", "plotters", "disabled"])
                .help("Set the plotting backend. By default, cargo-criterion will use the gnuplot backend if gnuplot is available, or the plotters backend if it isn't. If set to 'disabled', plot generation will be disabled."))
            .arg(Arg::with_name("message-format")
                .long("message-format")
                .takes_value(true)
                .possible_values(&["json", "openmetrics"])
                .help("If set, machine-readable output of the requested format will be printed to stdout.")
                .long_help(
"Change the machine-readable output format. Possible values are [json, openmetrics].

Machine-readable information on the benchmarks will be printed in the requested format to stdout.
All of cargo-criterion's other output will be printed to stderr.

See the documentation for details on the data printed by each format.
")
        )
        .arg(
            Arg::with_name("history_id")
                .long("--history-id")
                .takes_value(true)
                .help("An optional identifier string such as a commit ID that will be shown in the history reports to identify this run.")
        )
        .arg(
            Arg::with_name("history_description")
                .long("--history-description")
                .takes_value(true)
                .help("An optional description string such as a commit message that will be shown in the history reports to describe this run.")
        )
        .arg(
            Arg::with_name("verbose")
                .long("--verbose")
                .short("v")
                .multiple(true)
                .help("Use verbose output (-vv very verbose/build.rs output). Only used for Cargo builds; see also --output-format"),
        )
        .arg(
            Arg::with_name("color")
                .long("--color")
                .takes_value(true)
                .possible_values(&["auto", "always", "never"])
                .help("Coloring: auto, always, never"),
        )
        .arg(
            Arg::with_name("frozen")
                .long("--frozen")
                .help("Require Cargo.lock and cache are up to date"),
        )
        .arg(
            Arg::with_name("locked")
                .long("--locked")
                .help("Require Cargo.lock is up to date"),
        )
        .arg(
            Arg::with_name("offline")
                .long("--offline")
                .help("Run without accessing the network"),
        )
        .arg(
            Arg::with_name("unstable_flags")
                .short("Z")
                .takes_value(true)
                .value_name("FLAG")
                .multiple(true)
                .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details"),
        )
        .arg(
            Arg::with_name("SUBCOMMAND")
                .hidden(true)
                .help("Cargo passes the name of the subcommand as the first param, so ignore it."),
        )
        .arg(
            Arg::with_name("BENCHNAME")
                .help("If specified, only run benches with names that match this regex"),
        )
        .arg(
            Arg::with_name("args")
                .takes_value(true)
                .multiple(true)
                .help("Arguments for the bench binary"),
        )
        .after_help(
            "\
The benchmark filtering argument BENCHNAME and all the arguments following the
two dashes (`--`) are passed to the benchmark binaries and thus Criterion.rs. 
If you're passing arguments to both Cargo and the binary, the ones after `--` go 
to the binary, the ones before go to Cargo. For details about Criterion.rs' arguments see
the output of `cargo criterion -- --help`.

If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be benchmarked. If it is not given, then
the current package is benchmarked. For more information on SPEC and its format,
see the `cargo help pkgid` command.

All packages in the workspace are benchmarked if the `--workspace` flag is supplied. The
`--workspace` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--workspace` flag.

The `--jobs` argument affects the building of the benchmark executable but does
not affect how many jobs are used when running the benchmarks.

Compilation can be customized with the `bench` profile in the manifest.
",
        )
        .get_matches();

    // Load the config file.
    let criterion_manifest_file: PathBuf = matches
        .value_of_os("criterion-manifest-file")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            // Support both capitalized and un-capitalized configuration files.
            if PathBuf::from("Criterion.toml").exists() {
                "Criterion.toml".into()
            } else {
                "criterion.toml".into()
            }
        })
        .into();

    let toml_config = load_toml_file(&criterion_manifest_file)?;

    // Many arguments have to be passed along to Cargo, so construct the list of cargo arguments
    // here.
    let mut cargo_args: Vec<OsString> = vec![];
    if matches.is_present("lib") {
        cargo_args.push("--lib".into());
    }
    if let Some(values) = matches.values_of_os("bin") {
        cargo_args.push("--bin".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("bins") {
        cargo_args.push("--bins".into());
    }
    if let Some(values) = matches.values_of_os("example") {
        cargo_args.push("--example".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("examples") {
        cargo_args.push("--examples".into());
    }
    if let Some(values) = matches.values_of_os("test") {
        cargo_args.push("--test".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("tests") {
        cargo_args.push("--tests".into());
    }
    if let Some(values) = matches.values_of_os("bench") {
        cargo_args.push("--bench".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("benches") {
        cargo_args.push("--benches".into());
    }
    if matches.is_present("all-targets") {
        cargo_args.push("--all-targets".into());
    }
    if let Some(values) = matches.values_of_os("package") {
        cargo_args.push("--package".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("all") {
        cargo_args.push("--all".into());
    }
    if matches.is_present("workspace") {
        cargo_args.push("--workspace".into());
    }
    if let Some(values) = matches.values_of_os("exclude") {
        cargo_args.push("--exclude".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if let Some(value) = matches.value_of_os("jobs") {
        cargo_args.push("--jobs".into());
        cargo_args.push(value.to_owned());
    }
    if let Some(values) = matches.values_of_os("features") {
        cargo_args.push("--features".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }
    if matches.is_present("all-features") {
        cargo_args.push("--all-features".into());
    }
    if matches.is_present("no-default-features") {
        cargo_args.push("--no-default-features".into());
    }
    if let Some(value) = matches.value_of_os("target") {
        cargo_args.push("--target".into());
        cargo_args.push(value.to_owned());
    }
    if let Some(value) = matches.value_of_os("target-dir") {
        cargo_args.push("--target-dir".into());
        cargo_args.push(value.to_owned());
    }
    if let Some(value) = matches.value_of_os("manifest-path") {
        cargo_args.push("--manifest-path".into());
        cargo_args.push(value.to_owned());
    }
    for _ in 0..matches.occurrences_of("verbose") {
        cargo_args.push("--verbose".into());
    }
    if let Some(value) = matches.value_of_os("color") {
        cargo_args.push("--color".into());
        cargo_args.push(value.to_owned());
    }
    if matches.is_present("frozen") {
        cargo_args.push("--frozen".into());
    }
    if matches.is_present("locked") {
        cargo_args.push("--locked".into());
    }
    if matches.is_present("offline") {
        cargo_args.push("--offline".into());
    }
    if let Some(values) = matches.values_of_os("unstable_flags") {
        cargo_args.push("-Z".into());
        cargo_args.extend(values.map(ToOwned::to_owned));
    }

    // Set criterion home to (in descending order of preference):
    // - $CRITERION_HOME
    // - The value from the config file
    // - ${--target-dir}/criterion
    // - ${target directory from `cargo metadata`}/criterion
    // - ./target/criterion
    let criterion_home = if let Some(value) = std::env::var_os("CRITERION_HOME") {
        PathBuf::from(value)
    } else if let Some(home) = toml_config.criterion_home {
        home
    } else if let Some(value) = matches.value_of_os("target-dir") {
        PathBuf::from(value).join("criterion")
    } else if let Ok(mut target_path) = get_target_directory_from_metadata() {
        target_path.push("criterion");
        target_path
    } else {
        PathBuf::from("target/criterion")
    };

    let self_config = SelfConfig {
        output_format: (matches.value_of("output-format"))
            .or(toml_config.output_format.as_deref())
            .map(OutputFormat::from_str)
            .unwrap_or(OutputFormat::Criterion),
        criterion_home,
        do_run: !matches.is_present("no-run"),
        do_fail_fast: !matches.is_present("no-fail-fast"),
        text_color: (matches.value_of("color"))
            .map(TextColor::from_str)
            .unwrap_or(TextColor::Auto),
        plotting_backend: (matches.value_of("plotting-backend"))
            .or(toml_config.plotting_backend.as_deref())
            .map(PlottingBackend::from_str)
            .unwrap_or(PlottingBackend::Auto),
        debug_build: matches.is_present("debug"),
        message_format: (matches.value_of("message-format")).map(MessageFormat::from_str),
        colors: toml_config.colors,
        history_id: matches.value_of("history_id").map(|s| s.to_owned()),
        history_description: matches
            .value_of("history_description")
            .map(|s| s.to_owned()),
    };

    // These are the extra arguments to be passed to the benchmark targets.
    let mut additional_args: Vec<OsString> = vec![];
    additional_args.extend(matches.value_of_os("BENCHNAME").map(ToOwned::to_owned));

    if let Some(args) = matches.values_of_os("args") {
        additional_args.extend(args.map(ToOwned::to_owned));
    }

    let configuration = FullConfig {
        self_config,
        cargo_args,
        additional_args,
    };
    Ok(configuration)
}

/// Load & parse the criterion.toml file (if present).
fn load_toml_file(toml_path: &Path) -> Result<TomlConfig, anyhow::Error> {
    if !toml_path.exists() {
        return Ok(TomlConfig::default());
    };

    let mut file = File::open(toml_path)
        .with_context(|| format!("Failed to open config file {:?}", toml_path))?;

    let mut str_buf = String::new();
    file.read_to_string(&mut str_buf)
        .with_context(|| format!("Failed to read config file {:?}", toml_path))?;

    let config: TomlConfig = toml::from_str(&str_buf)
        .with_context(|| format!("Failed to parse config file {:?}", toml_path))?;
    Ok(config)
}
