use std::io::{Read, Seek};

pub(crate) mod cura;
pub(crate) mod ideamaker;
pub(crate) mod m486;
pub(crate) mod slic3r;

use crate::gcode::{parse_gcode, Command};
use crate::hulls::KnownObject;
use crate::layers::LayerFilter;
use cura::CuraProcessor as Cura;
use ideamaker::IdeaMakerProcessor as IdeaMaker;
use m486::M486Processor as M486;
use slic3r::Slic3rProcessor as Slic3r;

#[enum_dispatch::enum_dispatch]
pub(crate) enum PreProcessorImpl {
    Slic3r,
    Cura,
    IdeaMaker,
    M486,
}

#[enum_dispatch::enum_dispatch(PreProcessorImpl)]
pub(crate) trait CancellationPreProcessor {
    fn process<'a>(
        &'a self,
        input: impl Read + Seek + Send + 'a,
        layer_filter: &'a LayerFilter,
    ) -> generator::Generator<'a, (), String>;
}

pub(crate) fn identify_slicer_marker(line: &str) -> Option<PreProcessorImpl> {
    let line = line.trim();
    if line.starts_with("; generated by SuperSlicer") {
        tracing::info!("Identified slicer: SuperSlicer");
        Some(Slic3r::new().into())
    } else if line.starts_with("; generated by PrusaSlicer") {
        tracing::info!("Identified slicer: PrusaSlicer");
        Some(Slic3r::new().into())
    } else if line.starts_with("; generated by Slic3r") {
        tracing::info!("Identified slicer: Slic3r");
        Some(Slic3r::new().into())
    } else if line.starts_with(";Generated with Cura_SteamEngine") {
        tracing::info!("Identified slicer: Cura");
        Some(Cura::new().into())
    } else if line.starts_with(";Sliced by ideaMaker") {
        tracing::info!("Identified slicer: ideaMaker");
        Some(IdeaMaker::new().into())
    } else if line.starts_with("M486") {
        tracing::info!("Identified slicer: M486");
        Some(M486::new().into())
    } else {
        None
    }
}

pub(crate) fn maybe_add_point(
    line: &str,
    known_object: &Option<&mut KnownObject>,
    layer_filter: &LayerFilter,
) {
    if let Some(current_object) = known_object {
        if layer_filter.contains(current_object.layer as usize)
            && line.trim().to_lowercase().starts_with('g')
        {
            let Command { params, .. } = parse_gcode(line);
            if let Some(_extrude) = params.get("E").and_then(|v| v.parse::<f64>().ok()) {
                let x = params.get("X").and_then(|v| v.parse::<f64>().ok());
                let y = params.get("Y").and_then(|v| v.parse::<f64>().ok());
                if let (Some(x), Some(y)) = (x, y) {
                    current_object.hull.add_point(x, y);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::__derive_refs::once_cell;
    use once_cell::sync::Lazy;
    use regex::Regex;
    use std::collections::HashSet;

    static DEFINITION_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"^(EXCLUDE_OBJECT_DEFINE).*(NAME=\S+).*$"#).unwrap());

    pub(crate) fn collect_definitions(lines: &[&str]) -> HashSet<String> {
        let mut definitions = HashSet::new();
        for line in lines {
            if line.starts_with("EXCLUDE_OBJECT_DEFINE") {
                definitions.insert(line.to_string());
                definitions.insert(DEFINITION_RE.replace(line, r#"$1 $2"#).to_string());
            }
        }

        definitions
    }
}
