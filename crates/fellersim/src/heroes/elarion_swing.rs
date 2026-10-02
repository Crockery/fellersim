use crate::*;

impl Iteration<'_> {
    pub(crate) fn elarion_swing_remaining(&self) -> f64 {
        let state = self.hero.elarion();
        if state.swing_remaining_ms <= 0.0 || state.swing_rate <= 0.0 {
            return state.swing_remaining_ms;
        }
        // The native tick only decrements a positive timer. Preserve at most
        // the final millisecond's overshoot while activation is blocked.
        let elapsed = self.common.now_ms.saturating_sub(state.swing_updated_ms) as f64;
        state.swing_remaining_ms
            - elapsed.min((state.swing_remaining_ms / state.swing_rate).ceil()) * state.swing_rate
    }

    pub(crate) fn refresh_elarion_swing_clock(&mut self) {
        if self.profile.contract.hero != HeroIdentity::Elarion
            || self.ability(DpsAbilityKind::ElarionShoot).is_none()
        {
            return;
        }
        let remaining = self.elarion_swing_remaining();
        let rate = (1.0 + self.effective_haste()).max(0.05);
        let state = self.hero.elarion_mut();
        state.swing_remaining_ms = remaining;
        state.swing_updated_ms = self.common.now_ms;
        state.swing_rate = rate;
        if !state.auto_attacking {
            return;
        }
        let due = self
            .common
            .now_ms
            .saturating_add((remaining.max(0.0) / rate).ceil() as u64)
            .max(state.auto_blocked_until);
        let next = self.next_cooldown_rate_boundary(due);
        if self.hero.elarion().swing_next_ms == Some(next) {
            return;
        }
        let state = self.hero.elarion_mut();
        state.swing_generation = state.swing_generation.wrapping_add(1);
        state.swing_next_ms = Some(next);
        let generation = state.swing_generation;
        self.push_event(next, ElarionEvent::SwingTimer { generation });
    }

    pub(crate) fn elarion_swing_tick(&mut self, generation: u64) {
        if generation != self.hero.elarion().swing_generation {
            return;
        }
        self.hero.elarion_mut().swing_next_ms = None;
        if self.elarion_swing_remaining() > 0.001
            || self.common.now_ms < self.hero.elarion().auto_blocked_until
        {
            return;
        }
        let index = self
            .profile
            .abilities
            .iter()
            .position(|ability| ability.kind == DpsAbilityKind::ElarionShoot)
            .unwrap();
        let ability = self.profile.abilities[index].clone();
        self.common.cast_sequence = self.common.cast_sequence.wrapping_add(1);
        let mut context = DamageContext::for_cast(self.common.cast_sequence);
        self.common.result.abilities[ability.damage_source.0].casts += 1;
        self.commit_ability(index, &mut context, false, 0.0);
        self.push_event(
            self.common
                .now_ms
                .saturating_add(ability.first_hit_delay_ms),
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
        self.hero.elarion_mut().swing_remaining_ms = self.elarion_swing_remaining()
            + ability_param(&ability, parameter_key!("swingDurationSeconds")) * 1_000.0;
        self.hero.elarion_mut().swing_updated_ms = self.common.now_ms;
    }
}
