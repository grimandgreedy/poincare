//! Nice-number tick generation for axis labelling.
//!
//! # Overview
//!
//! Given a data range `[min, max]` and a target tick count, [`nice_ticks`] picks
//! step sizes that land on round numbers (1, 2, 5, 10, 0.1, 0.2, …) so axis labels
//! look clean regardless of the data range.

/// Generate nice-number tick positions with formatted labels for an axis.
///
/// # Arguments
///
/// * `min`: lower bound of the data range.
/// * `max`: upper bound of the data range.
/// * `target_count`: approximate number of ticks desired. Returns at most `target_count + 1`
///   ticks in practice; the exact count depends on the chosen step.
///
/// # Returns
///
/// A vector of `(value, label)` pairs, sorted ascending.  Labels are formatted with
/// enough decimal places so the step size is clearly readable (e.g. `"0.2"` not `"0"`
/// when the step is 0.2).
///
/// # Edge cases
///
/// * `min == max`: returns a single tick at `min`.
/// * `target_count == 0`: returns an empty vector.
/// * `min > max`: treated the same as `(max, min)` but still sorted ascending.
pub fn nice_ticks(min: f64, max: f64, target_count: u32) -> Vec<(f64, String)> {
    if target_count == 0 {
        return Vec::new();
    }

    // Normalise so min ≤ max.
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };

    if (hi - lo).abs() < f64::EPSILON {
        // Degenerate range: return one tick.
        let label = format_tick(lo, 0);
        return vec![(lo, label)];
    }

    // --- nice-number algorithm ---

    // 1. Rough step size.
    let rough_step = (hi - lo) / target_count as f64;

    // 2. Magnitude of rough step (power of 10 floor).
    let magnitude = 10_f64.powf(rough_step.abs().log10().floor());

    // 3. Normalised fraction in [1, 10).
    let fraction = rough_step / magnitude;

    // 4. Pick the nearest nice fraction: 1, 2, 5, or 10.
    let nice_fraction: f64 = if fraction <= 1.5 {
        1.0
    } else if fraction <= 3.0 {
        2.0
    } else if fraction <= 7.0 {
        5.0
    } else {
        10.0
    };

    let nice_step = nice_fraction * magnitude;

    // 5. Number of decimal places needed to express the step cleanly.
    let decimals = (-nice_step.abs().log10().floor()).max(0.0) as usize;

    // 6. First tick: ceiling of lo / nice_step, then * nice_step.
    let first = (lo / nice_step).ceil() * nice_step;

    // 7. Collect ticks up to hi (with small floating-point tolerance).
    let mut ticks = Vec::new();
    let mut t = first;
    while t <= hi + nice_step * 1e-9 {
        let label = format_tick(t, decimals);
        ticks.push((t, label));
        t += nice_step;
    }

    ticks
}

/// Format a single tick value with `decimals` places after the decimal point.
///
/// Values very close to zero (within 1e-10 * nice_step resolution) are snapped
/// to exactly `0.0` to avoid labels like `"-0"` or `"1.23e-15"`.
fn format_tick(value: f64, decimals: usize) -> String {
    // Snap near-zero values to avoid "-0" etc.
    let v = if value.abs() < 1e-10 { 0.0 } else { value };
    format!("{:.prec$}", v, prec = decimals)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract just the values from the tick vec.
    fn values(ticks: &[(f64, String)]) -> Vec<f64> {
        ticks.iter().map(|(v, _)| *v).collect()
    }

    /// Extract just the labels.
    fn labels(ticks: &[(f64, String)]) -> Vec<&str> {
        ticks.iter().map(|(_, l)| l.as_str()).collect()
    }

    #[test]
    fn symmetric_integer_range() {
        let t = nice_ticks(-10.0, 10.0, 5);
        // Should hit -10, -5, 0, 5, 10 (step = 5).
        let v = values(&t);
        assert!(v.contains(&-10.0), "missing -10: {v:?}");
        assert!(v.contains(&-5.0), "missing -5: {v:?}");
        assert!(v.contains(&0.0), "missing 0: {v:?}");
        assert!(v.contains(&5.0), "missing 5: {v:?}");
        assert!(v.contains(&10.0), "missing 10: {v:?}");
        // Labels for step=5 should have 0 decimal places.
        for (_, lbl) in &t {
            assert!(!lbl.contains('.'), "expected integer labels, got {lbl}");
        }
    }

    #[test]
    fn unit_range_five_ticks() {
        let t = nice_ticks(0.0, 1.0, 5);
        // step should be 0.2 => ticks at 0.0, 0.2, 0.4, 0.6, 0.8, 1.0
        let v = values(&t);
        assert!(
            v.iter().any(|x| (x - 0.0).abs() < 1e-9),
            "missing 0.0: {v:?}"
        );
        assert!(
            v.iter().any(|x| (x - 0.2).abs() < 1e-9),
            "missing 0.2: {v:?}"
        );
        assert!(
            v.iter().any(|x| (x - 1.0).abs() < 1e-9),
            "missing 1.0: {v:?}"
        );
        // Labels should contain a decimal point (step = 0.2).
        for (_, lbl) in &t {
            assert!(lbl.contains('.'), "expected decimal labels, got {lbl}");
        }
    }

    #[test]
    fn asymmetric_range_picks_nice_step() {
        let t = nice_ticks(-3.7, 4.2, 5);
        // rough_step ~ 1.58 => nice step = 2 => ticks at -2, 0, 2, 4
        // (first tick is ceil(-3.7/2)*2 = ceil(-1.85)*2 = -1*2 = -2)
        let v = values(&t);
        assert!(
            v.iter().any(|x| (*x - (-2.0)).abs() < 1e-9),
            "missing -2: {v:?}"
        );
        assert!(v.iter().any(|x| (x - 0.0).abs() < 1e-9), "missing 0: {v:?}");
        assert!(v.iter().any(|x| (x - 4.0).abs() < 1e-9), "missing 4: {v:?}");
        // Integer labels for step = 2.
        for (_, lbl) in &t {
            assert!(!lbl.contains('.'), "expected integer labels, got {lbl}");
        }
    }

    #[test]
    fn zero_count_returns_empty() {
        assert!(nice_ticks(0.0, 10.0, 0).is_empty());
    }

    #[test]
    fn degenerate_range_returns_single_tick() {
        let t = nice_ticks(5.0, 5.0, 5);
        assert_eq!(t.len(), 1);
        assert!((t[0].0 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn labels_match_values() {
        for t in nice_ticks(-10.0, 10.0, 4) {
            let parsed: f64 = t.1.parse().expect("label should be parseable as f64");
            assert!(
                (parsed - t.0).abs() < 1e-9,
                "label mismatch: {} vs {}",
                t.1,
                t.0
            );
        }
    }

    #[test]
    fn ticks_are_sorted_ascending() {
        let t = nice_ticks(-100.0, 100.0, 8);
        let v = values(&t);
        for w in v.windows(2) {
            assert!(w[0] < w[1], "ticks not sorted: {v:?}");
        }
    }
}
