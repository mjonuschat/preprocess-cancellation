use crate::gcode::{exclude_object_end, exclude_object_header, exclude_object_start};
use crate::hulls::KnownObject;
use crate::layers::LayerFilter;
use crate::slicers::{maybe_add_point, CancellationPreProcessor};
use generator::{done, Gn};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek};

pub(crate) struct Slic3rProcessor {}

impl Slic3rProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl CancellationPreProcessor for Slic3rProcessor {
    fn process<'a>(
        &'a self,
        input: impl Read + Seek + Send + 'a,
        layer_filter: &'a LayerFilter,
    ) -> generator::Generator<'a, (), String> {
        let mut input = BufReader::new(input);
        let mut known_objects: HashMap<String, KnownObject> = HashMap::new();
        let mut current_object: Option<&mut KnownObject> = None;
        for line in input.by_ref().lines() {
            let line = line.unwrap_or("".to_string());
            if line.starts_with("; printing object ") {
                if let Some(object_id) = line.split_once("printing object").map(|(_, o)| o.trim()) {
                    if !known_objects.contains_key(object_id) {
                        tracing::info!("Found object {}", object_id);
                        known_objects.insert(object_id.into(), KnownObject::new(object_id));
                    }

                    known_objects
                        .entry(object_id.to_string())
                        .and_modify(|ko| ko.layer += 1);
                    current_object = known_objects.get_mut(object_id);
                }
            }

            if line.starts_with("; stop printing object ") {
                current_object = None
            }

            maybe_add_point(&line, &current_object, layer_filter);
        }

        input.rewind().unwrap();

        Gn::new_scoped(move |mut s| {
            for line in input.by_ref().lines() {
                let line = line.unwrap_or("".to_string());

                if !line.trim().is_empty() && !line.starts_with(';') {
                    s.yield_from(exclude_object_header(&known_objects));
                }

                s.yield_with(format!("{}\n", &line));

                if !line.trim().is_empty() && !line.starts_with(';') {
                    break;
                }
            }

            for line in input.by_ref().lines() {
                let line = line.unwrap_or("".to_string());

                s.yield_with(format!("{}\n", &line));

                if line.starts_with("; printing object ") {
                    let known_object = line
                        .split_once("printing object")
                        .and_then(|(_, oid)| known_objects.get(oid.trim()));

                    if let Some(known_object) = known_object {
                        s.yield_from(exclude_object_start(&known_object.name));
                    }
                }

                if line.starts_with("; stop printing object ") {
                    let known_object = line
                        .split_once("printing object")
                        .and_then(|(_, oid)| known_objects.get(oid.trim()));

                    if let Some(known_object) = known_object {
                        s.yield_from(exclude_object_end(&known_object.name));
                    }
                }
            }

            done!();
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slicers::tests::collect_definitions;
    use once_cell::sync::Lazy;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    static GCODE_PATH: Lazy<PathBuf> =
        Lazy::new(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("GCode"));

    #[test]
    fn test_superslicer() {
        let processor = Slic3rProcessor::new();
        let input = File::open(GCODE_PATH.join("superslicer.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_id_0_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_id_0_copy_1"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=union_3_id_2_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cylinder_2_id_1_copy_0"));

        assert!(result.contains(&"G1 X164.398 Y143.144 E0.05245"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_id_0_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_id_0_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_id_0_copy_1")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_id_0_copy_1")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cylinder_2_id_1_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cylinder_2_id_1_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=union_3_id_2_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=union_3_id_2_copy_0")
                .count(),
            25
        );
    }

    #[test]
    fn test_prusaslicer() {
        let processor = Slic3rProcessor::new();
        let input = File::open(GCODE_PATH.join("prusaslicer.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_id_0_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_id_0_copy_1"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=union_3_id_2_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cylinder_2_id_1_copy_0"));

        assert!(result.contains(&"G1 X135.298 Y137.411 E0.03649"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cylinder_2_id_1_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cylinder_2_id_1_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_id_0_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_id_0_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_id_0_copy_1")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_id_0_copy_1")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=union_3_id_2_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=union_3_id_2_copy_0")
                .count(),
            25
        );
    }

    #[test]
    fn test_slic3r() {
        let processor = Slic3rProcessor::new();
        let input = File::open(GCODE_PATH.join("slic3r.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl_id_0_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl_id_0_copy_1"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cylinder_2_stl_id_1_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=union_3_stl_id_2_copy_0"));

        assert!(result.contains(&"G1 X97.912 Y94.709 E3.82225"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl_id_0_copy_0")
                .count(),
            16
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl_id_0_copy_0")
                .count(),
            16
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl_id_0_copy_1")
                .count(),
            16
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl_id_0_copy_1")
                .count(),
            16
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cylinder_2_stl_id_1_copy_0")
                .count(),
            16
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cylinder_2_stl_id_1_copy_0")
                .count(),
            16
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=union_3_stl_id_2_copy_0")
                .count(),
            16
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=union_3_stl_id_2_copy_0")
                .count(),
            16
        );
    }

    #[test]
    fn test_orcaslicer() {
        let processor = Slic3rProcessor::new();
        let input = File::open(GCODE_PATH.join("orcaslicer.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl_id_1_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl_id_2_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=union_3_stl_id_0_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cylinder_2_stl_id_3_copy_0"));

        assert!(result.contains(&"G1 X125.188 Y133.259 E.01869"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl_id_1_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl_id_1_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl_id_2_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl_id_2_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cylinder_2_stl_id_3_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cylinder_2_stl_id_3_copy_0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=union_3_stl_id_0_copy_0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=union_3_stl_id_0_copy_0")
                .count(),
            25
        );
    }

    #[test]
    fn test_issue_1_prusaslicer_point_collection() {
        let processor = Slic3rProcessor::new();
        let input = File::open(GCODE_PATH.join("prusaslicer-issue1.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=Shape_Cylinder_id_1_copy_0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=Shape_Box_id_0_copy_0"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=Shape_Cylinder_id_1_copy_0")
                .count(),
            125
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=Shape_Cylinder_id_1_copy_0")
                .count(),
            125
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=Shape_Box_id_0_copy_0")
                .count(),
            125
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=Shape_Box_id_0_copy_0")
                .count(),
            125
        );
    }

    #[test]
    fn test_issue_2_retractions_included_in_bounding_boxes() {
        let processor = Slic3rProcessor::new();
        let input = File::open(
            GCODE_PATH
                .join("regressions")
                .join("issue_2_retractions.gcode"),
        )
        .unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let output: String = processor.process(input, &layer_filter).collect();

        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_0_copy_0"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_0"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_1"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_2"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_3"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_4"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_5"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_6"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_7"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_8"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_9"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_10"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_11"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_12"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_13"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_14"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_15"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_16"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_17"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_18"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_19"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_20"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_21"));
        assert!(output.contains("EXCLUDE_OBJECT_DEFINE NAME=Leaf_stl_id_1_copy_22"));
    }
}
