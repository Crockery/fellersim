use crate::*;

impl Iteration<'_> {
    pub(crate) fn dot_instances(
        &self,
        target: u32,
        ability: DpsAbilityKind,
    ) -> impl Iterator<Item = (DotKind, &DotState)> {
        self.common
            .dots
            .get_key_value(&(target, DotKind::Ability(ability)))
            .into_iter()
            .chain(self.common.dots.range(
                (target, DotKind::OverlappingAbility(ability, 0))
                    ..=(target, DotKind::OverlappingAbility(ability, u64::MAX)),
            ))
            .take_while(|_| self.common.execution.charge(1))
            .map(|((_, kind), dot)| (*kind, dot))
    }

    pub(crate) fn record_dot_uptime(&mut self, target: u32, kind: DotKind, dot: &DotState) {
        let end = dot
            .expires_ms
            .min(self.common.now_ms)
            .min(ENCOUNTER_DURATION_MS);
        let start = dot.started_ms.min(end);
        let active = if kind.has_independent_instances() {
            let intervals = self
                .common
                .dot_uptime_intervals
                .entry((target, kind.ability_kind().unwrap()))
                .or_default();
            if !self.common.execution.charge_usize(intervals.len() + 1) {
                return;
            }
            let previous: u64 = intervals.iter().map(|(start, end)| end - start).sum();
            intervals.push((start, end));
            intervals.sort_unstable();
            let mut merged: Vec<(u64, u64)> = Vec::new();
            for &(start, end) in intervals.iter() {
                if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                    last.1 = last.1.max(end);
                } else {
                    merged.push((start, end));
                }
            }
            let total: u64 = merged.iter().map(|(start, end)| end - start).sum();
            *intervals = merged;
            total - previous
        } else {
            end.saturating_sub(start)
        };
        let key = format!("dot:{}", kind.id());
        *self.common.result.uptimes.entry(key).or_default() +=
            active as f64 / ENCOUNTER_DURATION_MS as f64 / self.common.target_count as f64;
    }

    pub(crate) fn reschedule_dot_ticks(&mut self) {
        if !self.common.execution.charge_usize(self.common.dots.len()) {
            return;
        }
        let updates = self
            .common
            .dots
            .iter()
            .filter(|((_, kind), dot)| {
                (kind.is_ardeos_hero_dot()
                    || matches!(
                        kind.ability_kind(),
                        Some(
                            DpsAbilityKind::CorrosiveSpill
                                | DpsAbilityKind::HemorrhagingStrike
                                | DpsAbilityKind::SeethingPoison
                                | DpsAbilityKind::VolatilePoison
                                | DpsAbilityKind::Hemotoxin
                        )
                    ))
                    && dot.expires_ms >= self.common.now_ms
                    && dot.next_tick_ms > self.common.now_ms
            })
            .map(|((target_index, kind), dot)| {
                let new_period = self.dot_period(*kind, dot.model.period_ms);
                let remaining_fraction = dot.next_tick_ms.saturating_sub(self.common.now_ms) as f64
                    / dot.scheduled_period_ms.max(1) as f64;
                let next_tick_ms = self.common.now_ms.saturating_add(
                    (remaining_fraction.clamp(0.0, 1.0) * new_period as f64)
                        .round()
                        .max(1.0) as u64,
                );
                (
                    *target_index,
                    *kind,
                    dot.generation.wrapping_add(1),
                    new_period,
                    next_tick_ms,
                    dot.expires_ms,
                )
            })
            .collect::<Vec<_>>();
        if !self.common.execution.charge_usize(updates.len()) {
            return;
        }
        for (target_index, kind, generation, period, next_tick_ms, expires_ms) in updates {
            if let Some(dot) = self.common.dots.get_mut(&(target_index, kind)) {
                dot.generation = generation;
                dot.scheduled_period_ms = period;
                dot.next_tick_ms = next_tick_ms;
                dot.last_tick_ms = next_tick_ms.saturating_sub(period);
            }
            self.push_event(
                next_tick_ms,
                CoreEvent::DotTick {
                    kind,
                    generation,
                    target_index,
                },
            );
            self.push_event(
                expires_ms,
                CoreEvent::DotExpire {
                    kind,
                    generation,
                    target_index,
                },
            );
        }
    }

    pub(crate) fn dot_period(&self, kind: DotKind, base_ms: u64) -> u64 {
        let wildfire = if kind.is_ardeos_hero_dot()
            && self.common.now_ms < self.hero.ardeos().wildfire_until
        {
            self.ability(DpsAbilityKind::Wildfire)
                .and_then(|ability| ability.parameters.get(parameter_key!("tickRateMultiplier")))
                .filter(|value| *value > 0.0)
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let haste = if kind.is_damage_derived()
            || kind.ability_kind() == Some(DpsAbilityKind::BloodboundSpirit)
        {
            1.0
        } else {
            (1.0 + self.effective_haste()).max(0.05)
        };
        ((base_ms as f64) / (haste * wildfire)).round().max(1.0) as u64
    }

    pub(crate) fn effective_haste(&self) -> f64 {
        self.profile.haste
            + match &self.hero {
                HeroState::Rime(state) => state.icy_flow_casting_haste,
                _ => 0.0,
            }
            + secondary_rating_delta(
                self.profile.haste_rating,
                self.dynamic_rating_bonus(parameter_key!("hasteRating")),
            )
            + self.dynamic_stat_bonus(parameter_key!("hasteBonus"))
            + if self.common.now_ms < self.shared.heroism_until {
                self.profile.heroism_haste
            } else {
                0.0
            }
            + if matches!(
                &self.hero,
                HeroState::Tariq(state) if self.common.now_ms < state.thunder_call_until
            ) {
                self.common
                    .selected_talents
                    .get("ink-talent-id-talent7")
                    .map(|talent| param(talent, parameter_key!("hasteBonus")))
                    .unwrap_or(0.0)
            } else {
                0.0
            }
            + if matches!(
                &self.hero,
                HeroState::Elarion(state) if self.common.now_ms < state.skystriders_grace_until
            ) {
                self.ability(DpsAbilityKind::SkystridersGrace)
                    .map(|ability| ability_param(ability, parameter_key!("hasteBonus")))
                    .unwrap_or(0.0)
            } else {
                0.0
            }
    }

    pub(crate) fn activate_heroism(&mut self) {
        if self.shared.heroism_until <= self.common.now_ms {
            if let Some(started_ms) = self.shared.heroism_started_ms.take() {
                let active = self
                    .shared
                    .heroism_until
                    .min(ENCOUNTER_DURATION_MS)
                    .saturating_sub(started_ms.min(ENCOUNTER_DURATION_MS));
                *self
                    .common
                    .result
                    .uptimes
                    .entry("buff:spirit-of-heroism".into())
                    .or_default() += active as f64 / ENCOUNTER_DURATION_MS as f64;
            }
            self.shared.heroism_started_ms = Some(self.common.now_ms);
        } else if self.shared.heroism_started_ms.is_none() {
            self.shared.heroism_started_ms = Some(self.common.now_ms);
        }
        self.shared.heroism_until = self
            .common
            .now_ms
            .saturating_add(self.profile.heroism_duration_ms);
        if let Some(mechanic) = self
            .profile
            .mechanic_indexes
            .heroism_power_set
            .map(|index| Arc::clone(&self.profile.mechanics[index]))
        {
            if !self
                .common
                .execution
                .charge_usize(self.shared.heroism_power_set_expirations.len())
            {
                return;
            }
            let duration = seconds_parameter(&mechanic, parameter_key!("durationSeconds"));
            // StackingType::None creates a separate effect for each activation.
            self.shared
                .heroism_power_set_expirations
                .retain(|until| *until > self.common.now_ms);
            self.shared
                .heroism_power_set_expirations
                .push(self.common.now_ms.saturating_add(duration));
            self.activate_dynamic_buff(&mechanic, duration, 1, 0.0);
        }
    }

    pub(crate) fn dot_remaining(&self, kind: DpsAbilityKind) -> u64 {
        if DotKind::Ability(kind).has_independent_instances() {
            return self
                .dot_instances(0, kind)
                .map(|(_, dot)| dot.expires_ms.saturating_sub(self.common.now_ms))
                .max()
                .unwrap_or(0);
        }
        self.common
            .dots
            .get(&(0, DotKind::Ability(kind)))
            .map(|dot| dot.expires_ms.saturating_sub(self.common.now_ms))
            .unwrap_or(0)
    }
    pub(crate) fn ability(&self, kind: DpsAbilityKind) -> Option<&Arc<CompiledAbility>> {
        self.profile.ability(kind)
    }
    pub(crate) fn reduce_cooldown(&mut self, kind: DpsAbilityKind, amount: u64) {
        let base_ms = self
            .ability(kind)
            .map(|ability| ability.cooldown_ms as f64)
            .unwrap_or(0.0);
        let execution = &self.common.execution;
        if let Some(cooldown) = self.common.cooldowns.get_mut(&kind) {
            // Native ReduceCooldownForAbility spends reduction across charge
            // boundaries; excess work continues into the next missing charge.
            advance_cooldown_state(cooldown, amount as f64, base_ms, execution);
            if cooldown.used_charges == 0 {
                self.common.cooldowns.remove(&kind);
            }
        }
    }
    pub(crate) fn restore_ability_charge(&mut self, kind: DpsAbilityKind) {
        let mut ready = false;
        if let Some(cooldown) = self.common.cooldowns.get_mut(&kind) {
            cooldown.used_charges = cooldown.used_charges.saturating_sub(1);
            if cooldown.used_charges == 0 {
                cooldown.remaining_ms = 0.0;
                ready = true;
            }
        }
        if ready {
            self.common.cooldowns.remove(&kind);
        }
    }
    pub(crate) fn reset_cooldown(&mut self, kind: DpsAbilityKind) {
        self.common.cooldowns.remove(&kind);
    }
    pub(crate) fn reduce_cooldown_fraction(&mut self, kind: DpsAbilityKind, reduction: f64) {
        let base_ms = self
            .ability(kind)
            .map(|ability| ability.cooldown_ms as f64)
            .unwrap_or(0.0);
        let execution = &self.common.execution;
        if let Some(cooldown) = self.common.cooldowns.get_mut(&kind) {
            cooldown.remaining_ms *= 1.0 - reduction.clamp(0.0, 1.0);
            advance_cooldown_state(cooldown, 0.0, base_ms, execution);
            if cooldown.used_charges == 0 {
                self.common.cooldowns.remove(&kind);
            }
        }
    }
    pub(crate) fn reduce_weapon_cooldown_fraction(&mut self, reduction: f64) {
        for kind in [
            DpsAbilityKind::WeaponFrostVolley,
            DpsAbilityKind::WeaponArcaneChannel,
            DpsAbilityKind::WeaponChainLightning,
            DpsAbilityKind::WeaponShadowMark,
            DpsAbilityKind::WeaponCleaveCharge,
            DpsAbilityKind::WeaponFrontalCone,
            DpsAbilityKind::WeaponInstantAoe,
        ] {
            self.reduce_cooldown_fraction(kind, reduction);
        }
    }
    pub(crate) fn reduce_weapon_cooldowns(&mut self, amount: u64) {
        for kind in [
            DpsAbilityKind::WeaponFrostVolley,
            DpsAbilityKind::WeaponArcaneChannel,
            DpsAbilityKind::WeaponChainLightning,
            DpsAbilityKind::WeaponShadowMark,
            DpsAbilityKind::WeaponCleaveCharge,
            DpsAbilityKind::WeaponFrontalCone,
            DpsAbilityKind::WeaponInstantAoe,
        ] {
            self.reduce_cooldown(kind, amount);
        }
    }
    pub(crate) fn increase_dot_duration(&mut self, target_index: u32, kind: DotKind, amount: u64) {
        if matches!(kind, DotKind::Ability(_)) && kind.has_independent_instances() {
            let keys = self
                .dot_instances(target_index, kind.ability_kind().unwrap())
                .map(|(kind, _)| kind)
                .collect::<Vec<_>>();
            for key in keys {
                self.increase_single_dot_duration(target_index, key, amount);
            }
        } else {
            self.increase_single_dot_duration(target_index, kind, amount);
        }
    }

    fn increase_single_dot_duration(&mut self, target_index: u32, kind: DotKind, amount: u64) {
        let mut reschedule = None;
        let mut refresh_source = None;
        if let Some(dot) = self
            .common
            .dots
            .get_mut(&(target_index, kind))
            .filter(|dot| dot.expires_ms >= self.common.now_ms)
        {
            dot.expires_ms = dot.expires_ms.saturating_add(amount);
            reschedule = Some((dot.expires_ms, dot.generation));
            if !kind.applies_effect_each_tick()
                && dot.model.duration_ms > 0
                && dot.expires_ms.saturating_sub(self.common.now_ms) == dot.model.duration_ms
            {
                refresh_source = Some(dot.source);
            }
        }
        if let Some((expires_ms, generation)) = reschedule {
            self.push_event(
                expires_ms,
                CoreEvent::DotExpire {
                    kind,
                    generation,
                    target_index,
                },
            );
        }
        if let Some(source) = refresh_source {
            self.trigger_kindling_on_harmful_effect_application(
                kind.ability_kind(),
                source,
                target_index,
            );
        }
    }

    pub(crate) fn increase_dot_duration_up_to_total(
        &mut self,
        target_index: u32,
        kind: DotKind,
        amount: u64,
    ) {
        let mut reschedule = None;
        let mut refresh_source = None;
        if let Some(dot) = self
            .common
            .dots
            .get_mut(&(target_index, kind))
            .filter(|dot| dot.expires_ms >= self.common.now_ms)
        {
            let remaining = dot.expires_ms.saturating_sub(self.common.now_ms);
            let new_remaining = remaining.saturating_add(amount).min(dot.model.duration_ms);
            let expires_ms = self.common.now_ms.saturating_add(new_remaining);
            // IncreaseActiveEffectDuration broadcasts even for a zero
            // extension. A full timer has zero elapsed fraction, which the
            // native duration listener treats as a refresh. Uptime's
            // started_ms cannot identify this after earlier reapplications.
            if !kind.applies_effect_each_tick()
                && dot.model.duration_ms > 0
                && new_remaining == dot.model.duration_ms
            {
                refresh_source = Some(dot.source);
            }
            if expires_ms != dot.expires_ms {
                dot.expires_ms = expires_ms;
                reschedule = Some((dot.expires_ms, dot.generation));
            }
        }
        if let Some(source) = refresh_source {
            self.trigger_kindling_on_harmful_effect_application(
                kind.ability_kind(),
                source,
                target_index,
            );
        }
        if let Some((expires_ms, generation)) = reschedule {
            self.push_event(
                expires_ms,
                CoreEvent::DotExpire {
                    kind,
                    generation,
                    target_index,
                },
            );
        }
    }
}
