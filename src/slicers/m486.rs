use crate::gcode::{
    exclude_object_end, exclude_object_header, exclude_object_start, parse_gcode, Command,
};
use crate::hulls::KnownObject;
use crate::layers::LayerFilter;
use crate::slicers::{maybe_add_point, CancellationPreProcessor};
use generator::{done, Gn};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek};

pub(crate) struct M486Processor {}

impl M486Processor {
    pub fn new() -> Self {
        Self {}
    }
}

impl CancellationPreProcessor for M486Processor {
    fn process<'a>(
        &'a self,
        input: impl Read + Seek + Send + 'a,
        layer_filter: &'a LayerFilter,
    ) -> generator::Generator<'a, (), String> {
        let mut input = BufReader::new(input);
        let mut known_objects: HashMap<String, KnownObject> = HashMap::new();
        let mut current_object: Option<String> = None;

        for line in input.by_ref().lines() {
            let line = line.unwrap_or("".to_string());
            if line.starts_with("M486") {
                let Command { params, .. } = parse_gcode(&line);
                if let Some(object_id) = params.get("T") {
                    if let Ok(end) = object_id.parse::<isize>() {
                        for i in -1..end {
                            tracing::info!("Found object {}", i);
                            known_objects
                                .entry(format!("{i}"))
                                .or_insert(KnownObject::new(&format!("{i}")));
                        }
                    }
                } else if let Some(object_id) = params.get("S") {
                    known_objects
                        .entry(object_id.to_string())
                        .and_modify(|ko| ko.layer += 1);

                    current_object = Some(object_id.to_string());
                }
            }

            if let Some(current_object_name) = &current_object {
                let current_object = known_objects.get_mut(current_object_name);
                maybe_add_point(&line, &current_object, layer_filter);
            }
        }

        input.rewind().unwrap();

        Gn::new_scoped(move |mut s| {
            let mut current_object: Option<&KnownObject> = None;

            for line in input.by_ref().lines() {
                let line = line.unwrap_or("".to_string());

                if !line.trim().is_empty() && !line.starts_with(';') {
                    let objects: HashMap<String, KnownObject> = known_objects
                        .iter()
                        .filter(|(name, _)| *name != "-1")
                        .map(|(name, o)| (name.to_owned(), o.to_owned()))
                        .collect();
                    s.yield_from(exclude_object_header(&objects));
                }

                s.yield_with(format!("{}\n", &line));

                if line.trim().is_empty() && !line.starts_with(';') {
                    break;
                }
            }

            for line in input.by_ref().lines() {
                let line = line.unwrap_or("".to_string());

                s.yield_with(format!("{}\n", &line));

                if line.to_uppercase().starts_with("M486") {
                    let Command { params, .. } = parse_gcode(&line);

                    if let Some(object_id) = params.get("S") {
                        if let Some(obj) = &current_object {
                            s.yield_from(exclude_object_end(&obj.name));
                            current_object = None
                        }

                        if *object_id != "-1" {
                            current_object = known_objects.get(*object_id);
                            if let Some(known_object) = current_object {
                                s.yield_from(exclude_object_start(&known_object.name));
                            }
                        }
                    }

                    s.yield_with("; ".to_string()) // Comment out the original M486 lines
                }

                s.yield_with(format!("{}\n", &line));
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
    fn test_m486() {
        let processor = M486Processor::new();
        let input = File::open(GCODE_PATH.join("m486.gcode")).unwrap();
        let layer_filter = LayerFilter::try_from("*").unwrap();

        let result: String = processor.process(input, &layer_filter).collect();
        let result: Vec<&str> = result.split('\n').collect();

        let definitions = collect_definitions(&result);

        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=0"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=1"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=2"));
        assert!(definitions.contains("EXCLUDE_OBJECT_DEFINE NAME=3"));

        assert!(result.contains(&"G1 X137.005 Y163.371 E0.29649"));

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=0")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=0")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=1")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=1")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=2")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=2")
                .count(),
            25
        );

        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_START NAME=3")
                .count(),
            25
        );
        assert_eq!(
            result
                .iter()
                .filter(|line| *line == &"EXCLUDE_OBJECT_END NAME=3")
                .count(),
            25
        );
    }
}
