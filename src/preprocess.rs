use crate::layers::LayerFilter;
use crate::slicers::{identify_slicer_marker, CancellationPreProcessor, PreProcessorImpl};
use std::ffi::OsStr;
use std::fs::{remove_file, rename, DirBuilder, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PreprocessError {
    #[error("Error reading/writing file {0}")]
    IoError(String),
    #[error("Error seeking to beginning of file")]
    RewindError,
    #[error("Error reading lines from input file")]
    ReadError,
    #[error("Error writing to output file")]
    WriteError,
    #[error("Invalid layer filter definition")]
    InvalidLayerFilter,
    #[error("Error creating output directory")]
    CreateOutputDirectory,
    #[error("Error creating temporary working file")]
    TempFile,
    #[error("Error writing changes to temporary working file")]
    FlushTempFile,
    #[error("The slicer that created this G-Code file could not be identified")]
    UnknownSlicer,
    #[error("Something bad happened :(")]
    Other,
}

fn process(
    input: impl Read + Seek + Send,
    output: &mut impl Write,
    layer_filter: &LayerFilter,
) -> Result<(), PreprocessError> {
    let mut input = BufReader::new(input);
    let mut processor: Option<PreProcessorImpl> = None;

    for line in input.by_ref().lines() {
        let line = line.map_err(|_err| PreprocessError::ReadError)?;
        if line.starts_with("EXCLUDE_OBJECT_DEFINE") || line.starts_with("DEFINE_OBJECT") {
            tracing::info!("GCode already supports cancellation");
            input
                .rewind()
                .map_err(|_err| PreprocessError::RewindError)?;
            std::io::copy(&mut input, output).map_err(|_err| PreprocessError::WriteError)?;

            return Ok(());
        }

        if processor.is_none() {
            processor = identify_slicer_marker(&line);
        }
    }

    match &processor {
        None => {
            tracing::error!("Could not identify slicer");
            Err(PreprocessError::UnknownSlicer)
        }
        Some(processor) => {
            input
                .rewind()
                .map_err(|_err| PreprocessError::RewindError)?;

            for line in processor.process(input.into_inner(), layer_filter) {
                write!(output, "{}", line).map_err(|_err| PreprocessError::WriteError)?;
            }

            Ok(())
        }
    }
}

pub(crate) fn file(
    src: &PathBuf,
    output_suffix: &Option<String>,
    output_dir: &Option<PathBuf>,
    layers: &str,
) -> Result<(), PreprocessError> {
    let mut dest_path = src.clone();

    if let Some(dir) = output_dir {
        dest_path = dir.join(src.file_name().ok_or(PreprocessError::Other)?);
        DirBuilder::new()
            .recursive(true)
            .create(
                dest_path
                    .parent()
                    .ok_or(PreprocessError::CreateOutputDirectory)?,
            )
            .map_err(|_| PreprocessError::CreateOutputDirectory)?;
    }

    if let Some(suffix) = output_suffix {
        match dest_path.extension() {
            Some(extension) => {
                let mut ext = OsStr::new(suffix).to_owned();
                ext.push(".");
                ext.push(extension);

                dest_path.set_extension(ext);
            }
            None => {
                dest_path.set_extension(suffix);
            }
        }
    }

    let layer_filter: LayerFilter = layers
        .try_into()
        .map_err(|_err| PreprocessError::InvalidLayerFilter)?;

    let tempfile = NamedTempFile::new().map_err(|_err| PreprocessError::TempFile)?;

    let reader = BufReader::new(
        File::open(src)
            .map_err(|_err| PreprocessError::IoError(src.to_string_lossy().to_string()))?,
    );
    let mut writer = BufWriter::new(&tempfile);
    match process(reader, &mut writer, &layer_filter) {
        Ok(_) => {
            writer
                .flush()
                .map_err(|_err| PreprocessError::FlushTempFile)?;

            if dest_path.exists() {
                remove_file(&dest_path).map_err(|_err| {
                    PreprocessError::IoError(dest_path.to_string_lossy().to_string())
                })?;
            }

            rename(&tempfile, &dest_path).map_err(|_err| {
                PreprocessError::IoError(dest_path.to_string_lossy().to_string())
            })?;

            Ok(())
        }
        Err(e) => {
            let _result = remove_file(&tempfile);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::{parse_gcode, Command};
    use once_cell::sync::Lazy;
    use ordered_float::OrderedFloat;
    use std::io::Cursor;
    use std::path::Path;

    static GCODE_PATH: Lazy<PathBuf> =
        Lazy::new(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("GCode"));

    static TEST_CASES: Lazy<Vec<(&str, &str, (f64, f64))>> = Lazy::new(|| {
        vec![
            ("inverted_pyramid", "0", (10.0, 10.0)),
            ("inverted_pyramid", "*/5", (28.0, 28.0)),
            ("inverted_pyramid", "*", (30.0, 30.0)),
        ]
    });

    #[test]
    fn test_slicer_layerfilters() {
        for slicer in ["m486"] {
            for (filename, layers, expected_size) in &*TEST_CASES {
                let input =
                    File::open(GCODE_PATH.join(filename).join(format!("{slicer}.gcode"))).unwrap();
                let mut output = Cursor::new(Vec::new());
                let layer_filter = LayerFilter::try_from(*layers).unwrap();

                process(&input, &mut output, &layer_filter).unwrap();

                output.rewind().unwrap();
                let definitions: Vec<_> = output
                    .lines()
                    .map_while(Result::ok)
                    .filter(|line| line.starts_with("EXCLUDE_OBJECT_DEFINE"))
                    .collect();

                assert_eq!(definitions.len(), 2);
                for definition in definitions {
                    let Command { params, .. } = parse_gcode(&definition);
                    let points: Vec<(f64, f64)> =
                        serde_json::from_str(params.get("POLYGON").unwrap_or(&"{}")).unwrap();

                    let xmin = points.iter().map(|p| OrderedFloat(p.0)).min().unwrap();
                    let xmax = points.iter().map(|p| OrderedFloat(p.0)).max().unwrap();
                    let ymin = points.iter().map(|p| OrderedFloat(p.1)).min().unwrap();
                    let ymax = points.iter().map(|p| OrderedFloat(p.1)).max().unwrap();

                    assert_eq!(expected_size, &((xmax - xmin).ceil(), (ymax - ymin).ceil()));
                }
            }
        }
    }
}
