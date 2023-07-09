use crate::hulls::KnownObject;
use clap::__derive_refs::once_cell;
use generator::{done, Generator, Gn};
use geo::{HasDimensions, Point};
use once_cell::sync::Lazy;
use std::collections::HashMap;

static HEADER_MARKER: Lazy<String> = Lazy::new(|| {
    let version =
        option_env!("CARGO_PKG_VERSION").map_or("".to_string(), |version| format!(" v{version}"));

    format!("; Pre-Processed for Cancel-Object support by preprocess_cancellation{version}\n")
});

fn dump_coords(point: &Point) -> String {
    format!("{x:0.3},{y:0.3}", x = point.x(), y = point.y())
}

pub(crate) struct Command<'a> {
    #[allow(dead_code)]
    pub command: Option<&'a str>,
    pub params: HashMap<&'a str, &'a str>,
}

pub(crate) fn parse_gcode(line: &str) -> Command {
    // Drop the comment
    let line = match line.split_once(';') {
        None => line.trim(),
        Some((line, _comment)) => line.trim(),
    };

    let mut parts = line.split_whitespace();
    let command = parts.next();

    let mut parsed = HashMap::new();

    for param in parts {
        if param.contains('=') {
            param
                .split_once('=')
                .map(|(key, value)| parsed.insert(key, value));
        } else {
            parsed.insert(&param[0..1], &param[1..]);
        }
    }

    Command {
        command,
        params: parsed,
    }
}

pub(crate) fn exclude_object_header(
    known_objects: &HashMap<String, KnownObject>,
) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        s.yield_with("\n\n".into());
        s.yield_with(HEADER_MARKER.to_string());
        s.yield_with(format!(
            "; {count} known objects\n",
            count = known_objects.len()
        ));

        for known_object in known_objects.values() {
            s.yield_from(exclude_object_define(known_object));
        }

        done!()
    })
}

fn exclude_object_define(known_object: &KnownObject) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        s.yield_with(format!(
            "EXCLUDE_OBJECT_DEFINE NAME={name}",
            name = known_object.name
        ));
        if let Some(center) = known_object.hull.center() {
            s.yield_with(format!(" CENTER={center}", center = dump_coords(&center)));
        }

        let polygon = known_object.hull.exterior();
        if !polygon.is_empty() {
            let points: Vec<(f64, f64)> = polygon.iter().map(|p| (p.x(), p.y())).collect();
            if let Ok(coords) = serde_json::to_string(&points) {
                s.yield_with(format!(" POLYGON={coords}", coords = coords));
            }
        }

        s.yield_with("\n".to_string());

        done!()
    })
}

pub(crate) fn exclude_object_start(name: &str) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        s.yield_with(format!("EXCLUDE_OBJECT_START NAME={name}\n"));
        done!()
    })
}

pub(crate) fn exclude_object_end(name: &str) -> Generator<'_, (), String> {
    Gn::new_scoped(move |mut s| {
        s.yield_with(format!("EXCLUDE_OBJECT_END NAME={name}\n"));
        done!()
    })
}
