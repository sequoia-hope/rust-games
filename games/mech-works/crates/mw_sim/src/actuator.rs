//! A joint's motor and driver: the commanded torque becomes a current, the
//! driver clamps it, the winding heats with I²R, and a hot or overloaded
//! driver derates or trips. Damage lowers the limits. Nothing here is
//! scripted: a limp is what a policy does with a weak or hot actuator.

/// What an actuator is: fixed by the part, changed by the workshop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActuatorSpec {
    /// Torque at the driver's full current, N·m (after the gearbox).
    pub peak_torque: f32,
    /// The driver's current limit, A.
    pub max_current: f32,
    /// Winding resistance, Ω.
    pub resistance: f32,
    /// Thermal mass of the winding, J/K.
    pub heat_capacity: f32,
    /// Thermal resistance from winding to ambient, K/W.
    pub thermal_resistance: f32,
}

impl ActuatorSpec {
    /// Torque constant, N·m/A.
    pub fn kt(&self) -> f32 {
        self.peak_torque / self.max_current
    }

    /// An actuator for `peak_torque`, with the mech's common electrical and
    /// thermal design: at half current the winding settles 60 K above
    /// ambient with an 8 s time constant (shorter than a real motor's, so
    /// heat matters within a training episode).
    pub fn with_peak_torque(peak_torque: f32) -> Self {
        let max_current = 100.0;
        let resistance = 0.1;
        let half_power = (0.5 * max_current) * (0.5 * max_current) * resistance;
        let thermal_resistance = 60.0 / half_power;
        let heat_capacity = 8.0 / thermal_resistance;
        Self {
            peak_torque,
            max_current,
            resistance,
            heat_capacity,
            thermal_resistance,
        }
    }
}

/// The temperatures a driver acts on, °C.
pub const AMBIENT: f32 = 30.0;
/// Above this the driver derates its current limit linearly.
pub const DERATE_START: f32 = 90.0;
/// At this it trips to zero torque.
pub const TRIP: f32 = 120.0;
/// A tripped driver comes back once the winding is this cool.
pub const RESET: f32 = 85.0;
/// The current limit left just below the trip point, as a fraction: high
/// enough that a sustained overload still trips (at 70% the winding makes
/// more heat at the trip point than it sheds), so a policy has to lighten
/// a hot joint's load to keep it alive.
pub const DERATE_FLOOR: f32 = 0.7;

/// A weak driver asked for more than this multiple of its limit for
/// longer than [`FAULT_TIME`] faults for [`FAULT_HOLD`].
pub const FAULT_RATIO: f32 = 1.5;
pub const FAULT_TIME: f32 = 0.05;
pub const FAULT_HOLD: f32 = 0.25;

/// An actuator's state: its health and what it is doing now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actuator {
    pub spec: ActuatorSpec,
    /// 1 for a sound part; damage lowers it toward 0. Scales the current
    /// limit (and with it the torque).
    pub health: f32,
    /// How weak the driver is: 1 never faults; below 1 it faults when asked
    /// for more than `driver_strength × FAULT_RATIO` of its limit.
    pub driver_strength: f32,
    /// Winding temperature, °C.
    pub temperature: f32,
    /// Seconds of overcurrent so far (toward a fault).
    pub over_time: f32,
    /// Seconds left of an overcurrent fault.
    pub fault_left: f32,
    /// Tripped on temperature, until it cools to [`RESET`].
    pub tripped: bool,
    /// The current and torque of the last step.
    pub current: f32,
    pub torque: f32,
}

impl Actuator {
    pub fn new(spec: ActuatorSpec) -> Self {
        Self {
            spec,
            health: 1.0,
            driver_strength: 1.0,
            temperature: AMBIENT,
            over_time: 0.0,
            fault_left: 0.0,
            tripped: false,
            current: 0.0,
            torque: 0.0,
        }
    }

    /// The current the driver allows now, A.
    pub fn current_limit(&self) -> f32 {
        if self.tripped || self.fault_left > 0.0 {
            return 0.0;
        }
        let base = self.spec.max_current * self.health.clamp(0.0, 1.0);
        let derate = if self.temperature <= DERATE_START {
            1.0
        } else {
            let t = ((self.temperature - DERATE_START) / (TRIP - DERATE_START)).min(1.0);
            1.0 - t * (1.0 - DERATE_FLOOR)
        };
        base * derate
    }

    /// The torque the actuator can give now, N·m.
    pub fn torque_limit(&self) -> f32 {
        self.current_limit() * self.spec.kt()
    }

    /// Asks for `torque` for `dt` seconds; returns the torque delivered,
    /// and advances the thermal and fault state.
    pub fn step(&mut self, torque: f32, dt: f32) -> f32 {
        let kt = self.spec.kt();
        let wanted = torque / kt;
        let limit = self.current_limit();

        // A weak driver faults on a sustained overload.
        let fault_at = self.spec.max_current * self.health * FAULT_RATIO * self.driver_strength;
        if self.driver_strength < 1.0 && self.fault_left <= 0.0 && wanted.abs() > fault_at {
            self.over_time += dt;
            if self.over_time >= FAULT_TIME {
                self.fault_left = FAULT_HOLD;
                self.over_time = 0.0;
            }
        } else {
            self.over_time = 0.0;
        }

        let current = if self.fault_left > 0.0 {
            0.0
        } else {
            wanted.clamp(-limit, limit)
        };
        self.fault_left = (self.fault_left - dt).max(0.0);

        // The winding: I²R in, conduction to ambient out.
        let power = current * current * self.spec.resistance;
        let loss = (self.temperature - AMBIENT) / self.spec.thermal_resistance;
        self.temperature += (power - loss) / self.spec.heat_capacity * dt;
        if self.temperature >= TRIP {
            self.tripped = true;
        } else if self.tripped && self.temperature <= RESET {
            self.tripped = false;
        }

        self.current = current;
        self.torque = current * kt;
        self.torque
    }

    /// Temperature scaled for an observation: 0 at ambient, 1 at the trip.
    pub fn temperature_obs(&self) -> f32 {
        (self.temperature - AMBIENT) / (TRIP - AMBIENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.005;

    fn act() -> Actuator {
        Actuator::new(ActuatorSpec::with_peak_torque(1000.0))
    }

    #[test]
    fn torque_is_clamped_to_the_current_limit() {
        let mut a = act();
        assert!((a.step(500.0, DT) - 500.0).abs() < 1e-3);
        assert!((a.step(5000.0, DT) - 1000.0).abs() < 1e-3);
        assert!((a.step(-5000.0, DT) + 1000.0).abs() < 1e-3);
    }

    #[test]
    fn health_lowers_the_limit() {
        let mut a = act();
        a.health = 0.4;
        assert!((a.step(5000.0, DT) - 400.0).abs() < 1e-3);
    }

    #[test]
    fn half_current_settles_sixty_kelvin_up() {
        let mut a = act();
        for _ in 0..(120.0 / DT) as usize {
            a.step(500.0, DT);
        }
        assert!(
            (a.temperature - (AMBIENT + 60.0)).abs() < 0.5,
            "{}",
            a.temperature
        );
        assert!(!a.tripped);
    }

    #[test]
    fn full_current_derates_then_trips_then_recovers() {
        let mut a = act();
        let mut derated = false;
        let mut steps = 0;
        while !a.tripped {
            a.step(1000.0, DT);
            if a.temperature > DERATE_START + 5.0 && a.torque < 990.0 {
                derated = true;
            }
            steps += 1;
            assert!(steps < 100_000, "never tripped");
        }
        assert!(derated, "should derate before tripping");
        assert_eq!(a.step(1000.0, DT), 0.0);
        while a.tripped {
            a.step(0.0, DT);
        }
        assert!(a.temperature <= RESET + 0.01);
        assert!(a.step(100.0, DT) > 99.0);
    }

    #[test]
    fn a_weak_driver_faults_on_overload() {
        let mut a = act();
        a.driver_strength = 0.5; // faults above 75 A
        for _ in 0..20 {
            a.step(900.0, DT);
        }
        assert!(a.fault_left > 0.0, "should have faulted");
        assert_eq!(a.step(900.0, DT), 0.0);
        for _ in 0..((FAULT_HOLD / DT) as usize + 2) {
            a.step(0.0, DT);
        }
        assert!(a.step(300.0, DT) > 299.0);
    }

    #[test]
    fn a_sound_driver_never_faults() {
        let mut a = act();
        for _ in 0..100 {
            a.step(5000.0, DT);
        }
        assert_eq!(a.fault_left, 0.0);
    }
}
