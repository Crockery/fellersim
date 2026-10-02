use crate::*;

impl Iteration<'_> {
    pub(crate) fn spawn_ruby_storm(&mut self, mechanic: &CompiledMechanic) {
        let speed = mechanic_param(mechanic, parameter_key!("movementSpeed"));
        let radius = mechanic_param(mechanic, parameter_key!("collisionRadius"));
        let lifetime = mechanic_param(mechanic, parameter_key!("lifetimeSeconds"));
        let distance = self.profile.contract.maximum_combat_range_units;
        if speed <= 0.0 || distance > speed * lifetime + radius {
            return;
        }
        // Accepted geometry approximation: one hit on each stacked target at
        // the first straight-line sphere overlap. No curved path or hitboxes.
        let delay_ms = (((distance - radius).max(0.0) / speed) * 1_000.0).ceil() as u64;
        let mut health_multiplier = 1.0;
        let indexes = &self.profile.mechanic_indexes.standing_still;
        if !self.common.execution.charge_lightweight_loop(indexes.len()) {
            return;
        }
        for index in indexes {
            let patient_soul = &self.profile.mechanics[*index];
            if self.common.now_ms
                >= seconds_parameter(patient_soul, parameter_key!("applicationDelaySeconds"))
            {
                health_multiplier *=
                    mechanic_param(patient_soul, parameter_key!("maxHealthMultiplier"));
            }
        }
        // CalculateMagnitude reads MaxHealth once when the actor is spawned.
        // Normalization includes static Stamina/health; Patient Soul is timed.
        let damage = mechanic_param(mechanic, parameter_key!("flatDamage")) * health_multiplier;
        let snapshot = DamageSourceSnapshot {
            hero_damage_scale: self.hero_damage_scale(None),
            expertise: self.effective_expertise(),
            primary_stat_multiplier: 1.0,
            critical_chance: self.effective_critical_strike(None),
        };
        self.push_event(
            self.common.now_ms.saturating_add(delay_ms),
            SharedEvent::RubyStormHit {
                mechanic_index: mechanic.index,
                damage_bits: damage.to_bits(),
                snapshot,
            },
        );
    }

    pub(crate) fn hit_ruby_storm(
        &mut self,
        mechanic_index: usize,
        damage: f64,
        snapshot: DamageSourceSnapshot,
    ) {
        let source = self.profile.mechanics[mechanic_index].damage_source;
        if !self.charge_work_units(u64::from(self.common.target_count)) {
            return;
        }
        for target in 0..self.common.target_count {
            // This is an absolute MaxHealth-derived magnitude, so primary
            // attribute buffs must not multiply it a second time.
            self.damage_raw_with_spread_key(
                None,
                source,
                damage,
                Some(snapshot.expertise),
                Some(1.0),
                Some(snapshot.critical_chance),
                HERO_DAMAGE_SPREAD_WIDTH,
                0.0,
                true,
                false,
                target,
                DamageContext::NONE.with_snapshot(snapshot).as_proc(),
            );
        }
    }
}
