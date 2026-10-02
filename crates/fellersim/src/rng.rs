use crate::{SimulationError, SimulationErrorCode};

// DefaultGame.ini configures this for UCRRandomStreamComponent. The native
// RollProcPerMinute implementation clamps the divisor to 0.1 before use.
pub(crate) const FIRST_PROC_CHANCE_FLOOR_PPM_DIVISOR: f32 = 9.0;
pub(crate) const MAX_RPPM_BAD_LUCK_SECONDS: f32 = 1_000.0;
// Build 25485623 stores this rounded constant at RVA 0x8cd0734. It is one
// float above the literal 0.05; that difference can cross a proc boundary.
const RPPM_BAD_LUCK_RATE: f32 = f32::from_bits(0x3d4c_ccce);

// UCRRandomStreamComponent indexes this native table by chance rounded to the
// nearest percentage point. Each value is the per-failure threshold decay that
// produces the requested long-run chance while suppressing streaks.
#[allow(clippy::excessive_precision)]
pub(crate) const CONTROLLED_RANDOM_FACTORS: [f32; 101] = [
    1.0,
    0.999842823,
    0.999372184,
    0.99858737,
    0.997489989,
    0.996079504,
    0.994353354,
    0.992320299,
    0.989976823,
    0.987323165,
    0.984358072,
    0.98108995,
    0.977510273,
    0.973632514,
    0.969447076,
    0.964966297,
    0.960183799,
    0.955135703,
    0.949759007,
    0.94411546,
    0.93817538,
    0.931944132,
    0.925439596,
    0.918646991,
    0.91158092,
    0.904245615,
    0.896643937,
    0.888772905,
    0.880638301,
    0.872264326,
    0.863632858,
    0.854712844,
    0.845594227,
    0.836190343,
    0.826578021,
    0.816753447,
    0.806658566,
    0.796402514,
    0.785905063,
    0.775190175,
    0.764275074,
    0.753200769,
    0.741862416,
    0.730398834,
    0.71871227,
    0.706894219,
    0.694856822,
    0.68264246,
    0.670327544,
    0.657848954,
    0.645235062,
    0.6324597,
    0.619563162,
    0.606526911,
    0.593426645,
    0.580169916,
    0.566839218,
    0.553292334,
    0.539853752,
    0.526208699,
    0.512536764,
    0.498813927,
    0.485045046,
    0.471181333,
    0.457302243,
    0.443413943,
    0.429503262,
    0.415506482,
    0.401567012,
    0.38765198,
    0.373695225,
    0.359745115,
    0.345868856,
    0.331981093,
    0.31815201,
    0.304365695,
    0.290644556,
    0.277024776,
    0.263462812,
    0.249986023,
    0.236542806,
    0.223382816,
    0.210130796,
    0.197115764,
    0.184175551,
    0.171426639,
    0.158810839,
    0.146292359,
    0.133954003,
    0.121768393,
    0.109754287,
    0.0979399607,
    0.0863209665,
    0.0748278722,
    0.0635780841,
    0.0524956733,
    0.0415893458,
    0.0308760721,
    0.0203953665,
    0.0100950971,
    -1.0,
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcPerMinuteState {
    pub(crate) last_roll_seconds: f32,
    pub(crate) last_proc_seconds: f32,
    pub(crate) has_procced: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ControlledRandomState {
    pub(crate) failure_threshold: f32,
    pub(crate) chance_factor: f32,
    pub(crate) chance_bucket: usize,
}

pub(crate) fn controlled_random_bucket(chance_factor: f32) -> usize {
    if chance_factor <= 0.0 {
        0
    } else if chance_factor >= 1.0 {
        100
    } else {
        ((chance_factor * 100.0 + 0.5) as usize).clamp(1, 99)
    }
}

pub(crate) fn real_ppm_probability(
    now_seconds: f32,
    state: ProcPerMinuteState,
    scaled_ppm: f32,
) -> f32 {
    let seconds_since_roll = (now_seconds - state.last_roll_seconds).max(0.0);
    let seconds_since_proc =
        (now_seconds - state.last_proc_seconds).clamp(0.0, MAX_RPPM_BAD_LUCK_SECONDS);
    let base_chance = seconds_since_roll * (1.0 / 60.0) * scaled_ppm;
    let bad_luck_multiplier = (seconds_since_proc * scaled_ppm * RPPM_BAD_LUCK_RATE - 3.5).max(1.0);
    let protected_chance = base_chance * bad_luck_multiplier;
    if state.has_procced {
        protected_chance
    } else {
        (scaled_ppm / FIRST_PROC_CHANCE_FLOOR_PPM_DIVISOR.max(0.1)).max(protected_chance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HitOutcome {
    Miss,
    Critical,
    Hit,
}

pub(crate) fn hit_outcome_from_roll(
    roll: f64,
    miss_chance: f64,
    critical_chance: f64,
) -> HitOutcome {
    // The native outcome table consumes one draw, with avoidance before crit.
    // In the maintained rear-facing scenario only Miss remains enabled.
    if roll < miss_chance {
        HitOutcome::Miss
    } else if critical_chance > 0.0 && roll - miss_chance < critical_chance {
        HitOutcome::Critical
    } else {
        HitOutcome::Hit
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SplitMix64 {
    pub(crate) state: u64,
}
impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub(crate) fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub(crate) fn chance(&mut self, probability: f64) -> bool {
        probability >= 1.0
            || (probability > 0.0 && (self.next() as f64 / u64::MAX as f64) < probability)
    }

    pub(crate) fn native_15_bit_chance(&mut self, threshold: f32) -> bool {
        // Native code multiplies by the stored f32 reciprocal (RVA 0x7bd8a58).
        // Division rounds differently for some of the 32,768 possible draws.
        let roll = (self.next() & 0x7fff) as f32 * (1.0_f32 / 32_767.0);
        threshold > roll
    }

    pub(crate) fn native_uniform_f32(&mut self) -> f32 {
        (self.next() as f32) * 5.421_011e-20
    }

    pub(crate) fn uniform_f64(&mut self, minimum: f64, maximum: f64) -> f64 {
        minimum + (maximum - minimum) * (self.next() as f64 / u64::MAX as f64)
    }

    pub(crate) fn damage_outcome_is_critical(&mut self, critical_chance: f64) -> bool {
        self.damage_outcome(0.0, critical_chance) == HitOutcome::Critical
    }

    pub(crate) fn damage_outcome(&mut self, miss_chance: f64, critical_chance: f64) -> HitOutcome {
        hit_outcome_from_roll(
            self.next() as f64 / u64::MAX as f64,
            miss_chance,
            critical_chance,
        )
    }

    pub(crate) fn damage_spread_roll(&mut self) -> f64 {
        damage_spread_roll_from_random(self.next())
    }

    pub(crate) fn shuffle<T>(&mut self, values: &mut [T]) {
        for upper in (1..values.len()).rev() {
            let selected = (self.next() % (upper as u64 + 1)) as usize;
            values.swap(upper, selected);
        }
    }
}

pub(crate) fn damage_spread_roll_from_random(random: u64) -> f64 {
    (random & 0x7fff) as f64 / 32_767.0 - 0.5
}

pub(crate) fn parse_seed(seed: &str) -> Result<u64, SimulationError> {
    if seed.len() != 16
        || !seed
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "seed must be sixteen lowercase hexadecimal characters",
        ));
    }
    u64::from_str_radix(seed, 16).map_err(|_| {
        SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "seed is outside the supported range",
        )
    })
}

pub(crate) fn split_seed(seed: u64, index: u32) -> u64 {
    let mut rng = SplitMix64::new(seed ^ index as u64);
    rng.next()
}
