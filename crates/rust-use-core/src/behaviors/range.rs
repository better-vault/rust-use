use super::{action, dispatch_value, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};

/// A bounded numeric value or an ordered span within the same bounds.
#[derive(Debug, Clone)]
pub struct Range {
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    end: Option<f64>,
}

impl Range {
    /// Start at the lower bound with a step of `1.0`.
    ///
    /// Reversed bounds are ordered, NaN bounds become zero, and infinite bounds
    /// become the nearest finite endpoints.
    pub fn new(min: f64, max: f64) -> Self {
        let finite = |bound: f64| {
            if bound.is_nan() {
                0.0
            } else {
                bound.clamp(-f64::MAX, f64::MAX)
            }
        };
        let (min, max) = (finite(min), finite(max));
        let (min, max) = (min.min(max), min.max(max));
        Self {
            min,
            max,
            step: 1.0,
            value: min,
            end: None,
        }
    }

    /// Set the spacing and re-snap the current value or span.
    /// A non-positive or non-finite step makes the range continuous.
    #[must_use]
    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            0.0
        };
        self.value = self.snap(self.value);
        self.end = self.end.map(|end| self.snap(end));
        self
    }

    /// Enable two thumbs and store the initial endpoints in ascending order.
    /// A NaN endpoint uses the corresponding current endpoint.
    #[must_use]
    pub fn span(mut self, start: f64, end: f64) -> Self {
        let (old_start, old_end) = self.bounds();
        let start = self.snap(if start.is_nan() { old_start } else { start });
        let end = self.snap(if end.is_nan() { old_end } else { end });
        self.value = start.min(end);
        self.end = Some(start.max(end));
        self
    }

    /// Return the current value, or the lower endpoint of a span.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Return both endpoints, or the current value twice in single-thumb mode.
    pub fn bounds(&self) -> (f64, f64) {
        (self.value, self.end.unwrap_or(self.value))
    }

    /// Clamp and snap from the lower bound, then return the stored value.
    /// In a span, the lower thumb stops at the upper thumb. NaN leaves the value in place.
    pub fn set(&mut self, value: f64) -> f64 {
        if !value.is_nan() {
            self.value = self.snap(value).min(self.end.unwrap_or(self.max));
        }
        self.value
    }

    /// Set both endpoints, enabling the span on success.
    ///
    /// # Errors
    /// Returns an invalid argument for NaN or a rejection if the snapped start exceeds the end.
    pub fn set_span(&mut self, start: f64, end: f64) -> Result<(f64, f64), ActionError> {
        for (name, value) in [("start", start), ("end", end)] {
            if value.is_nan() {
                return Err(ActionError::InvalidArg {
                    name,
                    reason: "NaN is not a range value".into(),
                });
            }
        }
        let (start, end) = (self.snap(start), self.snap(end));
        if start > end {
            return Err(ActionError::Rejected("range start exceeds end".into()));
        }
        self.value = start;
        self.end = Some(end);
        Ok((start, end))
    }

    /// Increase the value by one step within its bounds.
    pub fn increment(&mut self) {
        self.set(self.value + self.step);
    }

    /// Decrease the value by one step within its bounds.
    pub fn decrement(&mut self) {
        self.set(self.value - self.step);
    }

    fn snap(&self, value: f64) -> f64 {
        if self.step == 0.0 || value <= self.min || value >= self.max {
            return value.clamp(self.min, self.max);
        }
        let offset = value - self.min;
        let steps = if offset.is_finite() {
            offset / self.step
        } else {
            value / self.step - self.min / self.step
        };
        if steps.is_finite() {
            steps
                .round()
                .mul_add(self.step, self.min)
                .clamp(self.min, self.max)
        } else {
            value
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn number(value: f64) -> Value {
    // The upper bounds are exclusive because MAX rounds up when cast to f64.
    if value.fract() == 0.0 {
        if (0.0..18_446_744_073_709_551_616.0).contains(&value) {
            return Value::from(value as u64);
        }
        if (-9_223_372_036_854_775_808.0..0.0).contains(&value) {
            return Value::from(value as i64);
        }
    }
    Value::from(value)
}

const ACTIONS: [ActionSpec; 2] = [
    action("set", "Set the range value.", &[param("value", "float")]),
    action(
        "set_span",
        "Set both range endpoints.",
        &[param("start", "float"), param("end", "float")],
    ),
];

impl Behavior for Range {
    fn describe(&self, out: &mut Describe) {
        if let Some(end) = self.end {
            out.value([number(self.value), number(end)]);
        } else {
            out.value(number(self.value));
        }
        out.attr("min", number(self.min))
            .attr("max", number(self.max));
        if !matches!(self.step, 1.0) {
            out.attr("step", number(self.step));
        }
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn is_enabled(&self, name: &str) -> bool {
        match name {
            "set" => true,
            "set_span" => self.end.is_some(),
            _ => false,
        }
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch_value(&ACTIONS, name, args, || {
            if !self.is_enabled(name) {
                return Err(ActionError::Disabled(name.into()));
            }
            if name == "set" {
                Ok(number(self.set(args.get(0, "value")?)))
            } else {
                self.set_span(args.get(0, "start")?, args.get(1, "end")?)?;
                Ok(Value::Null)
            }
        })
    }
}
