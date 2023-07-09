use dashmap::DashSet;
use geo::{ConvexHull, MultiPoint, Point, Simplify};
use itertools::{Itertools, MinMaxResult};
use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use regex::Regex;

static CLEAN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\W+"#).unwrap());

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct DecimalPoint {
    x: OrderedFloat<f64>,
    y: OrderedFloat<f64>,
}

impl DecimalPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HullTracker {
    points: DashSet<DecimalPoint>,
}

impl HullTracker {
    pub fn add_point(&self, x: f64, y: f64) {
        self.points.insert(DecimalPoint::new(x, y));
    }

    pub fn center(&self) -> Option<Point> {
        let x = match self.points.iter().map(|p| p.x).minmax() {
            MinMaxResult::NoElements => return None,
            MinMaxResult::OneElement(value) => value,
            MinMaxResult::MinMax(min_x, max_x) => (min_x + max_x) / 2.0,
        };
        let y = match self.points.iter().map(|p| p.y).minmax() {
            MinMaxResult::NoElements => return None,
            MinMaxResult::OneElement(value) => value,
            MinMaxResult::MinMax(min_y, max_y) => (min_y + max_y) / 2.0,
        };

        Some(Point::new(x.into(), y.into()))
    }

    fn as_multipoint(&self) -> MultiPoint {
        MultiPoint::new(
            self.points
                .iter()
                .map(|p| Point::new(p.x.into(), p.y.into()))
                .collect::<Vec<Point>>(),
        )
    }
    pub fn exterior(&self) -> MultiPoint {
        self.as_multipoint()
            .convex_hull()
            .simplify(&0.02)
            .exterior()
            .points()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct KnownObject {
    pub(crate) name: String,
    pub(crate) hull: HullTracker,
    pub(crate) layer: isize,
}

impl KnownObject {
    pub fn new(name: &str) -> Self {
        Self {
            name: Self::clean_id(name),
            ..Default::default()
        }
    }

    fn clean_id(name: &str) -> String {
        let ascii_name = any_ascii::any_ascii(name);
        CLEAN_RE
            .replace_all(&ascii_name, "_")
            .trim_matches('_')
            .into()
    }
}

impl Default for KnownObject {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            hull: HullTracker::default(),
            layer: -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hulls_simple() {
        let ht = HullTracker::default();
        ht.add_point(0.0, 0.0);
        ht.add_point(0.0, 1.0);
        ht.add_point(1.0, 1.0);
        ht.add_point(1.0, 0.0);

        assert_eq!(
            ht.exterior(),
            MultiPoint::new(vec![
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(0.0, 1.0),
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0)
            ])
        );
        assert_eq!(ht.center(), Some(Point::new(0.5, 0.5)));
    }

    #[test]
    fn test_hulls_rhombus() {
        let ht = HullTracker::default();
        ht.add_point(0.0, 5.0);
        ht.add_point(5.0, 10.0);
        ht.add_point(10.0, 5.0);
        ht.add_point(5.0, 0.0);

        assert_eq!(
            ht.exterior(),
            MultiPoint::new(vec![
                Point::new(5.0, 0.0),
                Point::new(10.0, 5.0),
                Point::new(5.0, 10.0),
                Point::new(0.0, 5.0),
                Point::new(5.0, 0.0)
            ])
        );
        assert_eq!(ht.center(), Some(Point::new(5.0, 5.0)));
    }

    #[test]
    fn test_hulls_circle() {
        let ht = HullTracker::default();
        ht.add_point(0.0, 5.0);
        ht.add_point(5.0, 10.0);
        ht.add_point(10.0, 5.0);
        ht.add_point(5.0, 0.0);

        let center = DecimalPoint::new(5.0, 5.0);

        for i in 0..360 {
            let i = i as f64;
            ht.add_point(
                center.x.into_inner() + 5.0 * i.to_radians().cos(),
                center.y.into_inner() + 5.0 * i.to_radians().sin(),
            );
        }

        for point in ht.exterior() {
            let dist = ((5.0 - point.x()).powf(2.0) + (5.0 - point.y()).powf(2.0)).sqrt();
            assert!((4.9..=5.1).contains(&dist));
        }

        assert_eq!(ht.center(), Some(Point::new(5.0, 5.0)));
    }

    #[test]
    fn test_unicode_object_names() {
        let known_object = KnownObject::new("Dé id:0 copy 0");
        assert_eq!(known_object.name, "De_id_0_copy_0")
    }
}
