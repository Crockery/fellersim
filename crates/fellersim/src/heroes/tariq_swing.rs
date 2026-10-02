use crate::*;

impl Iteration<'_> {
    pub(crate) fn tariq_swing_remaining(&self) -> f64 {
        let state = self.hero.tariq();
        let elapsed = self.common.now_ms.saturating_sub(state.swing_updated_ms) as f64;
        state.swing_remaining_ms - elapsed * state.swing_rate
    }

    pub(crate) fn tariq_in_hit_window(&self) -> bool {
        let attack = self.ability(DpsAbilityKind::TariqAttack).unwrap();
        self.tariq_swing_remaining()
            <= ability_param(attack, parameter_key!("hitWindowSeconds")) * 1_000.0
    }

    pub(crate) fn reset_tariq_swing(&mut self, conditional: bool) {
        let attack = self.ability(DpsAbilityKind::TariqAttack).unwrap();
        let duration = ability_param(attack, parameter_key!("swingDurationSeconds")) * 1_000.0;
        let window = ability_param(attack, parameter_key!("hitWindowSeconds")) * 1_000.0;
        let remaining = self.tariq_swing_remaining();
        if conditional && !(window <= remaining && remaining < duration) {
            return;
        }
        // InkUtils_ResetSwingTimer uses duration - visual hit delay - 0.01.
        let reset = duration - attack.first_hit_delay_ms as f64 - 10.0;
        self.hero.tariq_mut().swing_remaining_ms = reset;
        self.hero.tariq_mut().swing_updated_ms = self.common.now_ms;
        self.hero.tariq_mut().swing_next_ms = None;
        self.refresh_tariq_swing_clock();
    }

    pub(crate) fn refresh_tariq_swing_clock(&mut self) {
        if self.profile.contract.hero != HeroIdentity::Tariq {
            return;
        }
        let remaining = self.tariq_swing_remaining();
        // Native rate is Haste + AttackSpeed - 1. Current supported loadouts
        // have the base AttackSpeed of one; Haste is additive in our contract.
        let rate = (1.0 + self.effective_haste()).max(0.05);
        let state = self.hero.tariq_mut();
        state.swing_updated_ms = self.common.now_ms;
        state.swing_rate = if state.auto_attacking { rate } else { 0.0 };
        state.swing_remaining_ms = remaining;
        if !state.auto_attacking {
            return;
        }
        let due = self
            .common
            .now_ms
            .saturating_add((remaining.max(0.0) / rate).ceil() as u64)
            .max(state.auto_blocked_until);
        let next = self.next_cooldown_rate_boundary(due);
        if self.hero.tariq().swing_next_ms == Some(next) {
            return;
        }
        let state = self.hero.tariq_mut();
        state.swing_generation = state.swing_generation.wrapping_add(1);
        state.swing_next_ms = Some(next);
        let generation = state.swing_generation;
        self.push_event(next, TariqEvent::SwingTimer { generation });
    }

    pub(crate) fn tariq_swing_tick(&mut self, generation: u64) {
        if generation != self.hero.tariq().swing_generation {
            return;
        }
        self.hero.tariq_mut().swing_next_ms = None;
        if self.tariq_swing_remaining() > 0.001
            || self.common.now_ms < self.hero.tariq().auto_blocked_until
        {
            return;
        }
        let index = self
            .profile
            .abilities
            .iter()
            .position(|ability| ability.kind == DpsAbilityKind::TariqAttack)
            .unwrap();
        let ability = self.profile.abilities[index].clone();
        self.common.cast_sequence = self.common.cast_sequence.wrapping_add(1);
        let mut context = DamageContext::for_cast(self.common.cast_sequence);
        self.common.result.abilities[ability.damage_source.0].casts += 1;
        self.commit_ability(index, &mut context, false, 0.0);
        let delay =
            (ability.first_hit_delay_ms as f64 / self.hero.tariq().swing_rate).round() as u64;
        self.push_event(
            self.common.now_ms.saturating_add(delay),
            CoreEvent::ImpactBatch {
                index,
                impacts: vec![ImpactSpec {
                    single_hit: false,
                    damage_scale_bits: 1.0_f64.to_bits(),
                    target_index: 0,
                }],
                context: CastImpactContext::new(context),
            },
        );
        // Successful native activation adds a duration, retaining overshoot.
        let remaining = self.tariq_swing_remaining()
            + ability_param(&ability, parameter_key!("swingDurationSeconds")) * 1_000.0;
        self.hero.tariq_mut().swing_remaining_ms = remaining;
        self.hero.tariq_mut().swing_updated_ms = self.common.now_ms;
    }
}
