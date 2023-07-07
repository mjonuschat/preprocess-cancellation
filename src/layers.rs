use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub(crate) enum FilterParserError {
    #[error("The start value {0} could not be parsed")]
    StartValue(String),
    #[error("The stop value {0} could not be parsed")]
    StopValue(String),
    #[error("The given step size of {0} could not be parsed")]
    StepSize(String),
}

#[derive(Clone, Debug)]
struct LayerRange {
    start: usize,
    stop: usize,
    step: usize,
}

impl LayerRange {
    pub fn contains(&self, value: usize) -> bool {
        (self.start <= value && value <= self.stop) && ((value - self.start) % self.step == 0)
    }
}

impl Default for LayerRange {
    fn default() -> Self {
        Self {
            start: 0,
            stop: usize::MAX,
            step: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LayerFilter {
    ranges: Vec<LayerRange>,
}

impl LayerFilter {
    pub fn contains(&self, value: usize) -> bool {
        self.ranges.iter().any(|range| range.contains(value))
    }

    fn parse_filter_string(filters: &str) -> Result<LayerRange, FilterParserError> {
        if let Ok(layer) = filters.parse::<usize>() {
            return Ok(LayerRange {
                start: layer,
                stop: layer,
                step: 1,
            });
        }

        if filters == "*" {
            return Ok(LayerRange::default());
        }

        let mut filters = filters;
        let mut start: usize = 0;
        let mut stop: usize = 1;
        let mut step: usize = 1;

        if filters.starts_with('*') {
            start = 0;
            stop = usize::MAX;
        }

        if filters.contains('/') {
            if let Some((left, right)) = filters.split_once('/') {
                filters = left;
                step = right
                    .parse()
                    .map_err(|_err| FilterParserError::StepSize(right.into()))?;
            }
        }

        if filters.contains('-') {
            if let Some((left, right)) = filters.split_once('-') {
                let left = if left.is_empty() { "0" } else { left };
                start = left
                    .parse()
                    .map_err(|_err| FilterParserError::StartValue(left.into()))?;
                stop = if right.is_empty() {
                    usize::MAX
                } else {
                    right
                        .parse::<usize>()
                        .map_err(|_err| FilterParserError::StopValue(right.into()))?
                };
            }
        }

        Ok(LayerRange { start, stop, step })
    }
}

impl TryFrom<&str> for LayerFilter {
    type Error = FilterParserError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let ranges: Vec<LayerRange> = value
            .split(',')
            .map(Self::parse_filter_string)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { ranges })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_filters_single_layer() {
        let result = LayerFilter::try_from("1").unwrap();
        assert!(result.contains(1));
        assert!(!result.contains(2));
    }

    #[test]
    fn test_layer_filter_all_layers() {
        let result = LayerFilter::try_from("*").unwrap();
        assert!(result.contains(1));
        assert!(result.contains(100));
    }

    #[test]
    fn test_layer_filter_all_step() {
        let result = LayerFilter::try_from("*/2").unwrap();
        assert!(!result.contains(1));
        assert!(result.contains(2));
        assert!(!result.contains(3));
        assert!(result.contains(4));

        assert!(!result.contains(9999));
        assert!(result.contains(10000));
    }

    #[test]
    fn test_layer_filter_bounded_range() {
        let result = LayerFilter::try_from("1-5").unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
        assert!(result.contains(4));
        assert!(result.contains(5));
        assert!(!result.contains(6));
    }

    #[test]
    fn test_layer_filter_range_start_unbounded() {
        let result = LayerFilter::try_from("5-").unwrap();
        assert!(!result.contains(1));
        assert!(!result.contains(2));
        assert!(!result.contains(3));
        assert!(!result.contains(4));
        assert!(result.contains(5));
        assert!(result.contains(6));
    }

    #[test]
    fn test_layer_filter_range_unbounded_stop() {
        let result = LayerFilter::try_from("-5").unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
        assert!(result.contains(3));
        assert!(result.contains(4));
        assert!(result.contains(5));
        assert!(!result.contains(6));
    }

    #[test]
    fn test_layer_filter_range_step() {
        let result = LayerFilter::try_from("1-10/2").unwrap();
        assert!(result.contains(1));
        assert!(!result.contains(2));
        assert!(result.contains(3));
        assert!(!result.contains(4));
        assert!(result.contains(5));
        assert!(!result.contains(6));
        assert!(result.contains(7));
        assert!(!result.contains(8));
        assert!(result.contains(9));
        assert!(!result.contains(10));
        assert!(!result.contains(11));
    }

    #[test]
    fn test_multi_range() {
        let result = LayerFilter::try_from("1,3-5,6-10/2").unwrap();

        // 1
        assert!(result.contains(1));

        // 3-5
        assert!(result.contains(3));
        assert!(result.contains(4));
        assert!(result.contains(5));

        // 6-10/2
        assert!(result.contains(6));
        assert!(result.contains(8));
        assert!(result.contains(10));

        // And definitely none of these
        assert!(!result.contains(0));
        assert!(!result.contains(2));
        assert!(!result.contains(7));
        assert!(!result.contains(9));
        assert!(!result.contains(11));
    }
}
