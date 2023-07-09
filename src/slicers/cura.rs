use crate::gcode::{exclude_object_end, exclude_object_header, exclude_object_start};
use crate::hulls::KnownObject;
use crate::layers::LayerFilter;
use crate::slicers::{maybe_add_point, CancellationPreProcessor};
use generator::{done, Gn};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek};

pub(crate) struct CuraProcessor {}

impl CuraProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl CancellationPreProcessor for CuraProcessor {
    fn process<'a>(
        &'a self,
        input: impl Read + Seek + Send + 'a,
        layer_filter: &'a LayerFilter,
    ) -> generator::Generator<'a, (), String> {
        let mut input = BufReader::new(input);
        let mut known_objects: HashMap<String, KnownObject> = HashMap::new();
        let mut current_object: Option<&mut KnownObject> = None;
        let mut last_time_elapsed: Option<String> = None;

        for line in input.by_ref().lines() {
            let line = line.unwrap_or("".to_string());
            if line.starts_with(";MESH:") {
                if let Some(object_id) = line.split_once(':').map(|(_, name)| name.trim()) {
                    if object_id == "NONMESH" {
                        continue;
                    }

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

            maybe_add_point(&line, &current_object, layer_filter);

            if line.starts_with(";TIME_ELAPSED:") {
                last_time_elapsed = Some(line);
            }
        }

        input.rewind().unwrap();

        Gn::new_scoped(move |mut s| {
            let mut current_object: Option<&KnownObject> = None;

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

                if line.starts_with(";MESH:") {
                    if let Some(ref mut object) = current_object {
                        s.yield_from(exclude_object_end(&object.name));
                        current_object = None;
                    }

                    if let Some(object_name) = line.split_once(':').map(|(_, name)| name.trim()) {
                        if object_name == "NONMESH" {
                            continue;
                        }

                        current_object = known_objects.get(object_name);
                        if let Some(object) = current_object {
                            s.yield_from(exclude_object_start(&object.name));
                        }
                    }
                }

                if let Some(ref last_time_elapsed) = last_time_elapsed {
                    if &line == last_time_elapsed {
                        if let Some(object) = current_object {
                            s.yield_from(exclude_object_end(&object.name));
                            current_object = None;
                        }
                    }
                }
            }

            if let Some(object) = current_object {
                s.yield_from(exclude_object_end(&object.name));
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
    fn test_cura() {
        let processor = CuraProcessor::new();
        let input = File::open(GCODE_PATH.join("cura.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();
        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cylinder_2_stl"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=union_3_stl"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=cube_1_stl_1"));

        assert!(result.contains(&"G1 X152.563 Y136.21 E0.02148"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cylinder_2_stl")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cylinder_2_stl")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=union_3_stl")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=union_3_stl")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=cube_1_stl_1")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=cube_1_stl_1")
                .count(),
            25
        );
    }
}
