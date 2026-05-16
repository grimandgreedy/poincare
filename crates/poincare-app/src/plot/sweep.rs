/// Per-parameter ping-pong sweep/animation configuration.
#[derive(Clone, Debug)]
pub(crate) struct ParameterSweep {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Range traversals per second (min→max counts as one traversal).
    pub speed: f64,
    pub playing: bool,
    /// Current position in \[0.0, 1.0\]: 0.0 = min, 1.0 = max.
    pub phase: f64,
    /// +1.0 = moving toward max, -1.0 = moving toward min.
    pub direction: f64,
}

impl Default for ParameterSweep {
    fn default() -> Self {
        Self {
            min: -5.0,
            max: 5.0,
            step: 0.1,
            speed: 0.5,
            playing: false,
            phase: 0.0,
            direction: 1.0,
        }
    }
}

impl ParameterSweep {
    /// Build a sweep with a sensible range centred on the current value.
    pub fn new_for_value(current: f64) -> Self {
        let half = 5.0_f64.max(current.abs() + 1.0);
        let span = (half * 2.0).max(1.0);
        Self {
            min: current - half,
            max: current + half,
            step: (span / 100.0).max(0.01),
            ..Self::default()
        }
    }

    /// Advance the sweep by `dt` seconds. Returns the new parameter value.
    /// Has no effect (returns current value) when `playing` is false.
    pub fn tick(&mut self, dt: f64) -> f64 {
        if !self.playing {
            return self.current_value();
        }
        self.phase += self.direction * dt * self.speed;
        if self.phase >= 1.0 {
            self.phase = 1.0;
            self.direction = -1.0;
        } else if self.phase <= 0.0 {
            self.phase = 0.0;
            self.direction = 1.0;
        }
        self.current_value()
    }

    pub fn current_value(&self) -> f64 {
        self.min + self.phase * (self.max - self.min)
    }

    /// Reset phase to start (min) and stop playback.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.direction = 1.0;
        self.playing = false;
    }
}
