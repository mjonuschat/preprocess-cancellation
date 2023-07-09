use anyhow::Result;
use clap::{ArgAction, ColorChoice, Parser, ValueHint};
use std::path::PathBuf;
use tracing::Level;

mod gcode;
mod hulls;
mod layers;
mod preprocess;
mod slicers;
mod types;

/// Preprocess G-Code files to inject support for Klipper's EXCLUDE_OBJECT feature.
///
/// Current supported slicers:{n}
///   * Cura{n}
///   * Slic3r beta{n}
///   * PrusaSlicer{n}
///   * Superslicer{n}
///   * Ideamaker{n}
///   * GCode with Marlin M486 tags
#[derive(clap::Parser, Debug)]
#[clap(author, about, version, name = "Preprocess Cancellation", color=ColorChoice::Auto)]
pub(crate) struct Cli {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, action=ArgAction::Count)]
    verbose: u8,
    /// Add a suffix to the G-code output. Without this the file will be rewritten in place.
    #[clap(short = 'o', long)]
    pub output_suffix: Option<String>,
    /// G-Code output directory
    #[clap(short='O', long, value_hint=ValueHint::DirPath)]
    pub output_dir: Option<PathBuf>,
    /// Use shapely to generate a hull polygon for objects
    ///
    /// This is a no-op and only exists for compatibility with the Python version
    #[clap(long, hide=true, group="processing", conflicts_with="disable_shapely", action=ArgAction::SetTrue)]
    enable_shapely: bool,
    /// Disable using shapely for low memory systems
    ///
    /// This is a no-op and only exists for compatibility with the Python version
    #[clap(long, hide=true, group="processing", conflicts_with="enable_shapely", action=ArgAction::SetTrue)]
    disable_shapely: bool,
    /// Layers to collect shape points from.
    ///
    /// '*' will collect all layers
    /// '*[n]' to collect every nth layer
    /// 'n-m' to collect layers from n to m
    #[clap(
        short = 'l',
        long,
        group = "processing",
        default_value = "*",
        value_name = "LAYERS",
        conflicts_with = "fast"
    )]
    pub layers: String,
    /// Use only the first layer for point collection
    #[clap(long, group="processing", conflicts_with="layers", action=ArgAction::SetTrue)]
    pub fast: bool,
    /// G-code input files
    #[clap(value_hint=ValueHint::FilePath, num_args=1..)]
    pub gcode: Vec<PathBuf>,
}

fn setup_logging(verbose: u8) -> Result<()> {
    let log_level = match verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    // Logging
    tracing_subscriber::fmt().with_max_level(log_level).init();

    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();
    setup_logging(args.verbose)?;

    for filename in args.gcode {
        tracing::debug!("Processing GCode file: {}", filename.to_string_lossy());

        let result = preprocess::file(
            &filename,
            &args.output_suffix,
            &args.output_dir,
            &args.layers,
        );

        match result {
            Ok(_) => {
                tracing::info!("Successfully processed {}", filename.to_string_lossy());
            }
            Err(e) => {
                tracing::error!(
                    "Error processing file {}: {}",
                    &filename.to_string_lossy(),
                    e
                );
                anyhow::bail!("Error: {e}");
            }
        }
    }

    Ok(())
}
