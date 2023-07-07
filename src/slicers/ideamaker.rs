use crate::gcode::{exclude_object_end, exclude_object_header, exclude_object_start};
use crate::hulls::KnownObject;
use crate::layers::LayerFilter;
use crate::slicers::{maybe_add_point, CancellationPreProcessor};
use generator::{done, Gn};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek};

pub(crate) struct IdeaMakerProcessor {}

impl IdeaMakerProcessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl CancellationPreProcessor for IdeaMakerProcessor {
    fn process<'a>(
        &'a self,
        input: impl Read + Seek + Send + 'a,
        layer_filter: &'a LayerFilter,
    ) -> generator::Generator<'a, (), String> {
        let mut input = BufReader::new(input);
        let mut known_objects: HashMap<String, KnownObject> = HashMap::new();
        let mut current_object: Option<&mut KnownObject> = None;

        let mut object_name: Option<String> = None;

        for line in input.by_ref().lines() {
            let line = line.unwrap_or("".to_string());
            if line.starts_with(";PRINTING:") {
                object_name = line.split_once(':').map(|(_, name)| name.trim().into());
                continue;
            }

            if let Some(name) = &object_name {
                if line.starts_with(";PRINTING_ID:") {
                    if let Some(object_id) =
                        line.split_once(':').map(|(_, object_id)| object_id.trim())
                    {
                        if object_id == "-1" {
                            continue;
                        }

                        if !known_objects.contains_key(object_id) {
                            tracing::info!("Found object {}", object_id);
                            known_objects.insert(object_id.into(), KnownObject::new(name));
                            object_name = None;
                        }

                        known_objects
                            .entry(object_id.to_string())
                            .and_modify(|ko| ko.layer += 1);
                        current_object = known_objects.get_mut(object_id);
                    }
                } else {
                    object_name = None
                }
            }

            maybe_add_point(&line, &current_object, layer_filter);
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

                if line.starts_with(";PRINTING_ID:") {
                    match line.split_once(':').map(|(_, name)| name.trim()) {
                        None => panic!("Could not determine currently printing object"),
                        Some(printing_id) => {
                            if let Some(object) = current_object {
                                s.yield_from(exclude_object_end(&object.name));
                                current_object = None
                            }

                            if printing_id == "-1" {
                                continue;
                            }

                            current_object = known_objects.get(printing_id);
                            if let Some(current_object) = current_object {
                                s.yield_from(exclude_object_start(&current_object.name));
                            }
                        }
                    }
                }

                if line == ";REMAINING_TIME: 0\n" {
                    if let Some(object) = current_object {
                        s.yield_from(exclude_object_end(&object.name));
                        current_object = None;
                    }
                }
            }

            if let Some(current_object) = current_object {
                s.yield_from(exclude_object_end(&current_object.name));
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
    fn test_ideamaker() {
        let processor = IdeaMakerProcessor::new();
        let input = File::open(GCODE_PATH.join("ideamaker.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=test_bed_part1_3mf"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=test_bed_part2_3mf"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=test_bed_part0_3mf"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=test_bed_part0_1_3mf"));

        assert!(result.contains(&"G1 X100.759 Y106.827 E11.9581"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=test_bed_part1_3mf")
                .count(),
            32
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=test_bed_part1_3mf")
                .count(),
            32
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=test_bed_part2_3mf")
                .count(),
            32
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=test_bed_part2_3mf")
                .count(),
            32
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=test_bed_part0_3mf")
                .count(),
            33
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=test_bed_part0_3mf")
                .count(),
            33
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=test_bed_part0_1_3mf")
                .count(),
            33
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=test_bed_part0_1_3mf")
                .count(),
            33
        );
    }
}
