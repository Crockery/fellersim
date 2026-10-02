use crate::*;

impl Iteration<'_> {
    pub(crate) fn can_cast(&self, kind: DpsAbilityKind) -> Option<usize> {
        if kind == DpsAbilityKind::TariqAttack {
            return None;
        }
        let index = self.common.abilities_by_kind.get(&kind).copied()?;
        let free_cold_snap = kind == DpsAbilityKind::ColdSnap
            && self.hero.rime().navir_free_cold_snaps > 0
            && self.common.now_ms < self.hero.rime().navir_free_until;
        if !free_cold_snap && self.cooldown_remaining_ms(kind) > 0 {
            return None;
        }
        let ability = &self.profile.abilities[index];
        if kind == DpsAbilityKind::Detonate
            && self.hero.ardeos().embers < ability.secondary_resource_cost
            && !(self.hero.ardeos().apocalyptic_surge > 0
                && self.common.now_ms < self.hero.ardeos().apocalyptic_surge_until)
        {
            return None;
        }
        if kind == DpsAbilityKind::Incinerate && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if matches!(
            kind,
            DpsAbilityKind::GlacialBlast | DpsAbilityKind::IceComet
        ) {
            let glacial_assault_free = kind == DpsAbilityKind::GlacialBlast
                && self
                    .common
                    .selected_talents
                    .contains_key("rime-talent-id-talent1")
                && self.hero.rime().glacial_assault_stacks
                    >= param_u32(
                        self.common
                            .selected_talents
                            .get("rime-talent-id-talent1")
                            .expect("selected Glacial Assault talent"),
                        parameter_key!("maximumStacks"),
                    );
            let required = if self
                .common
                .selected_talents
                .contains_key("rime-talent-id-talent17")
            {
                1
            } else {
                ability.secondary_resource_cost
            };
            if !glacial_assault_free && self.hero.rime().winter_orbs < required {
                return None;
            }
        }
        if kind == DpsAbilityKind::WrathOfWinter && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if kind == DpsAbilityKind::RagingTempest && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if kind == DpsAbilityKind::EventHorizon && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if kind == DpsAbilityKind::MatriarchMacabre && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if kind == DpsAbilityKind::BloodboundSpirit && self.shared.spirit < ability.spirit_cost {
            return None;
        }
        if matches!(
            kind,
            DpsAbilityKind::SkitteringBlades
                | DpsAbilityKind::ArachnidAssault
                | DpsAbilityKind::HemorrhagingStrike
                | DpsAbilityKind::WidowsBite
                | DpsAbilityKind::QueensFang
                | DpsAbilityKind::Backstab
        ) {
            let cost = ability_param(ability, parameter_key!("energyCost"));
            if self.hero.mara().energy + f64::EPSILON < cost {
                return None;
            }
            if matches!(
                kind,
                DpsAbilityKind::ArachnidAssault
                    | DpsAbilityKind::HemorrhagingStrike
                    | DpsAbilityKind::QueensFang
            ) && self.hero.mara().combo_points == 0
            {
                return None;
            }
        }
        if matches!(
            kind,
            DpsAbilityKind::Multishot
                | DpsAbilityKind::HighwindArrow
                | DpsAbilityKind::HeartseekerBarrage
                | DpsAbilityKind::CelestialShot
                | DpsAbilityKind::StarfallVolley
        ) {
            let mut cost = ability_param(ability, parameter_key!("focusCost"));
            if self.common.now_ms < self.hero.elarion().event_horizon_until {
                cost *= self
                    .ability(DpsAbilityKind::EventHorizon)
                    .map(|event_horizon| {
                        ability_param(event_horizon, parameter_key!("focusCostMultiplier"))
                    })
                    .unwrap_or(1.0);
            }
            if kind == DpsAbilityKind::Multishot
                && (self.common.now_ms < self.hero.elarion().skystriders_supremacy_until
                    || (self.hero.elarion().empowered_multishot_stacks > 0
                        && self.common.now_ms < self.hero.elarion().empowered_multishot_until))
            {
                cost *= ability_param(ability, parameter_key!("empoweredCostMultiplier"));
            }
            let free = (kind == DpsAbilityKind::HighwindArrow
                && self.hero.elarion().resurgent_winds_stacks > 0
                && self.common.now_ms < self.hero.elarion().resurgent_winds_until)
                || (kind == DpsAbilityKind::CelestialShot
                    && self.hero.elarion().celestial_impetus_stacks > 0
                    && self.common.now_ms < self.hero.elarion().celestial_impetus_until);
            if !free && self.hero.elarion().focus + f64::EPSILON < cost {
                return None;
            }
        }
        if kind == DpsAbilityKind::ThunderCall
            && self.common.now_ms < self.hero.tariq().raging_tempest_until
        {
            return None;
        }
        if kind == DpsAbilityKind::TariqChainLightning
            && self.common.now_ms >= self.hero.tariq().thunder_call_until
        {
            return None;
        }
        if kind == DpsAbilityKind::CullingStrike
            && !(self.common.now_ms < self.hero.tariq().executioners_grin_until
                && self.hero.tariq().executioners_grin_stacks > 0)
        {
            return None;
        }
        if matches!(
            kind,
            DpsAbilityKind::HammerStorm | DpsAbilityKind::SkullCrusher
        ) {
            let base_cost = if kind == DpsAbilityKind::HammerStorm {
                ability_param(ability, parameter_key!("maximumFuryCost"))
            } else {
                ability_param(ability, parameter_key!("furyCost"))
            };
            let cost_multiplier = if self.hero.tariq().focused_wrath_stacks > 0
                && self.common.now_ms < self.hero.tariq().focused_wrath_until
            {
                self.ability(DpsAbilityKind::FocusedWrath)
                    .map(|focused| ability_param(focused, parameter_key!("costMultiplier")))
                    .unwrap_or(1.0)
            } else {
                1.0
            };
            if self.hero.tariq().fury + f64::EPSILON < base_cost * cost_multiplier {
                return None;
            }
        }
        Some(index)
    }

    pub(crate) fn buff_stacks(&self, buff: AplBuff) -> u32 {
        match buff {
            AplBuff::Wildfire => u32::from(self.common.now_ms < self.hero.ardeos().wildfire_until),
            AplBuff::SpiritOfHeroism => u32::from(self.common.now_ms < self.shared.heroism_until),
            AplBuff::ApocalypticSurge => {
                if self.common.now_ms < self.hero.ardeos().apocalyptic_surge_until {
                    self.hero.ardeos().apocalyptic_surge
                } else {
                    0
                }
            }
            AplBuff::CascadingInferno => self.hero.ardeos().cascading_stacks,
            AplBuff::IceBlitz => u32::from(self.common.now_ms < self.hero.rime().ice_blitz_until),
            AplBuff::WintersBlessing => {
                u32::from(self.common.now_ms < self.hero.rime().winters_blessing_until)
            }
            AplBuff::WrathOfWinter => {
                u32::from(self.common.now_ms < self.hero.rime().wrath_of_winter_until)
            }
            AplBuff::FlightOfTheNavir => {
                u32::from(self.common.now_ms < self.hero.rime().flight_of_the_navir_until)
            }
            AplBuff::GlacialAssault => self.hero.rime().glacial_assault_stacks,
            AplBuff::IcyFlow => {
                if self.common.now_ms < self.hero.rime().icy_flow_until {
                    self.hero.rime().icy_flow_stacks
                } else {
                    0
                }
            }
            AplBuff::SoulfrostTorrent => {
                u32::from(self.common.now_ms < self.hero.rime().soulfrost_torrent_until)
            }
            AplBuff::HarrowingIce => {
                if self.hero.rime().harrowing_ice_until > self.common.now_ms {
                    self.hero.rime().harrowing_ice_stacks
                } else {
                    0
                }
            }
            AplBuff::FrostweaversWrath => {
                if self.common.now_ms < self.hero.rime().frostweavers_wrath_until {
                    self.hero.rime().frostweavers_wrath_stacks
                } else {
                    0
                }
            }
            AplBuff::ThunderCall => {
                u32::from(self.common.now_ms < self.hero.tariq().thunder_call_until)
            }
            AplBuff::FocusedWrath => {
                if self.common.now_ms < self.hero.tariq().focused_wrath_until {
                    self.hero.tariq().focused_wrath_stacks
                } else {
                    0
                }
            }
            AplBuff::RagingTempest => {
                u32::from(self.common.now_ms < self.hero.tariq().raging_tempest_until)
            }
            AplBuff::FarBeyondDriven => {
                if self.common.now_ms < self.hero.tariq().far_beyond_driven_until {
                    self.hero.tariq().far_beyond_driven_stacks
                } else {
                    0
                }
            }
            AplBuff::KillEmAll => {
                if self.common.now_ms < self.hero.tariq().kill_em_all_until {
                    self.hero.tariq().kill_em_all_stacks
                } else {
                    0
                }
            }
            AplBuff::SquareHammer => {
                if self.common.now_ms < self.hero.tariq().square_hammer_until {
                    self.hero.tariq().square_hammer_stacks
                } else {
                    0
                }
            }
            AplBuff::SquareHammerExpertise => {
                u32::from(self.common.now_ms < self.hero.tariq().square_hammer_expertise_until)
            }
            AplBuff::SchismHammerStorm => {
                u32::from(self.common.now_ms < self.hero.tariq().schism_skull_until)
                    * self.hero.tariq().schism_skull_stacks
            }
            AplBuff::SchismSkullCrusher => {
                u32::from(self.common.now_ms < self.hero.tariq().schism_hammer_until)
                    * self.hero.tariq().schism_hammer_stacks
            }
            AplBuff::ExecutionersGrin => {
                u32::from(self.common.now_ms < self.hero.tariq().executioners_grin_until)
            }
            AplBuff::CelestialImpetus => {
                u32::from(self.common.now_ms < self.hero.elarion().celestial_impetus_until)
                    * self.hero.elarion().celestial_impetus_stacks
            }
            AplBuff::EmpoweredMultishot => {
                u32::from(self.common.now_ms < self.hero.elarion().empowered_multishot_until)
                    * self.hero.elarion().empowered_multishot_stacks
            }
            AplBuff::SkystridersGrace => {
                u32::from(self.common.now_ms < self.hero.elarion().skystriders_grace_until)
            }
            AplBuff::EventHorizon => {
                u32::from(self.common.now_ms < self.hero.elarion().event_horizon_until)
            }
            AplBuff::SkystridersSupremacy => {
                if self.common.now_ms < self.hero.elarion().skystriders_supremacy_until {
                    self.hero.elarion().skystriders_supremacy_stacks.max(1)
                } else {
                    0
                }
            }
            AplBuff::ImpendingHeartseeker => {
                u32::from(self.common.now_ms < self.hero.elarion().impending_heartseeker_until)
            }
            AplBuff::ResurgentWinds => {
                u32::from(self.common.now_ms < self.hero.elarion().resurgent_winds_until)
                    * self.hero.elarion().resurgent_winds_stacks
            }
            AplBuff::BroodingShadows => u32::from(self.hero.mara().stealth_active),
            AplBuff::MaidenOfDeath => {
                u32::from(self.common.now_ms < self.hero.mara().maiden_of_death_until)
            }
            AplBuff::MatriarchMacabre => {
                u32::from(self.common.now_ms < self.hero.mara().matriarch_macabre_until)
            }
            AplBuff::AssassinsGuile => {
                u32::from(self.common.now_ms < self.hero.mara().assassins_guile_until)
            }
            AplBuff::DeadlyScheme => {
                if self.common.now_ms < self.hero.mara().deadly_scheme_until {
                    1
                } else {
                    self.hero.mara().deadly_scheme_stacks
                }
            }
            AplBuff::FeedTheQueen => {
                u32::from(self.common.now_ms < self.hero.mara().feed_the_queen_until)
                    * self.hero.mara().feed_the_queen_stacks
            }
            AplBuff::MalevolenceArachnid => {
                u32::from(self.common.now_ms < self.hero.mara().malevolence_arachnid_until)
                    * self.hero.mara().malevolence_arachnid_stacks
            }
            AplBuff::MalevolenceQueen => {
                u32::from(self.common.now_ms < self.hero.mara().malevolence_queen_until)
                    * self.hero.mara().malevolence_queen_stacks
            }
            AplBuff::DrenchedInBlood => {
                u32::from(self.common.now_ms < self.hero.mara().drenched_in_blood_until)
            }
            AplBuff::SerratedEdge => {
                u32::from(self.common.now_ms < self.hero.gunde().serrated_edge_until)
            }
            AplBuff::ReignInBlood => {
                u32::from(self.common.now_ms < self.hero.gunde().reign_in_blood_until)
            }
            AplBuff::BloodboundSpirit => {
                u32::from(self.common.now_ms < self.hero.gunde().bloodbound_spirit_until)
            }
            AplBuff::DeathsArc => {
                u32::from(self.common.now_ms < self.hero.gunde().deaths_arc_until)
            }
            AplBuff::GrimHarvest => {
                u32::from(self.common.now_ms < self.hero.gunde().grim_harvest_until)
            }
            AplBuff::HarvestersToll => {
                u32::from(self.common.now_ms < self.hero.gunde().harvesters_toll_until)
            }
            AplBuff::CrimsonStrikes => {
                u32::from(self.common.now_ms < self.hero.gunde().crimson_strikes_until)
            }
            AplBuff::MurderOfCrows => {
                u32::from(self.common.now_ms < self.hero.gunde().murder_of_crows_until)
                    * self.hero.gunde().murder_of_crows_stacks
            }
            AplBuff::Massacre => {
                u32::from(self.common.now_ms < self.hero.gunde().massacre_until)
                    * self.hero.gunde().massacre_stacks
            }
            AplBuff::AncestralInstinct => {
                u32::from(self.common.now_ms < self.hero.gunde().ancestral_instinct_until)
            }
            AplBuff::Bloodbath => u32::from(self.common.now_ms < self.hero.gunde().bloodbath_until),
            AplBuff::CarrionOnslaught => {
                u32::from(self.common.now_ms < self.hero.gunde().carrion_onslaught_until)
            }
            AplBuff::OpenWounds => {
                u32::from(self.common.now_ms < self.hero.gunde().open_wounds_until[0])
            }
        }
    }

    pub(crate) fn buff_remaining_ms(&self, buff: AplBuff) -> u64 {
        match buff {
            AplBuff::Wildfire => self
                .hero
                .ardeos()
                .wildfire_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SpiritOfHeroism => {
                self.shared.heroism_until.saturating_sub(self.common.now_ms)
            }
            AplBuff::ApocalypticSurge => {
                if self.hero.ardeos().apocalyptic_surge > 0 {
                    self.hero
                        .ardeos()
                        .apocalyptic_surge_until
                        .saturating_sub(self.common.now_ms)
                } else {
                    0
                }
            }
            AplBuff::CascadingInferno => {
                if self.hero.ardeos().cascading_stacks > 0 {
                    ENCOUNTER_DURATION_MS.saturating_sub(self.common.now_ms)
                } else {
                    0
                }
            }
            AplBuff::IceBlitz => self
                .hero
                .rime()
                .ice_blitz_until
                .saturating_sub(self.common.now_ms),
            AplBuff::WintersBlessing => self
                .hero
                .rime()
                .winters_blessing_until
                .saturating_sub(self.common.now_ms),
            AplBuff::WrathOfWinter => self
                .hero
                .rime()
                .wrath_of_winter_until
                .saturating_sub(self.common.now_ms),
            AplBuff::FlightOfTheNavir => self
                .hero
                .rime()
                .flight_of_the_navir_until
                .saturating_sub(self.common.now_ms),
            AplBuff::GlacialAssault => {
                u64::from(self.hero.rime().glacial_assault_stacks > 0)
                    * ENCOUNTER_DURATION_MS.saturating_sub(self.common.now_ms)
            }
            AplBuff::IcyFlow => self
                .hero
                .rime()
                .icy_flow_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SoulfrostTorrent => self
                .hero
                .rime()
                .soulfrost_torrent_until
                .saturating_sub(self.common.now_ms),
            AplBuff::HarrowingIce => self
                .hero
                .rime()
                .harrowing_ice_until
                .saturating_sub(self.common.now_ms),
            AplBuff::FrostweaversWrath => self
                .hero
                .rime()
                .frostweavers_wrath_until
                .saturating_sub(self.common.now_ms),
            AplBuff::ThunderCall => self
                .hero
                .tariq()
                .thunder_call_until
                .saturating_sub(self.common.now_ms),
            AplBuff::FocusedWrath => self
                .hero
                .tariq()
                .focused_wrath_until
                .saturating_sub(self.common.now_ms),
            AplBuff::RagingTempest => self
                .hero
                .tariq()
                .raging_tempest_until
                .saturating_sub(self.common.now_ms),
            AplBuff::FarBeyondDriven => self
                .hero
                .tariq()
                .far_beyond_driven_until
                .saturating_sub(self.common.now_ms),
            AplBuff::KillEmAll => self
                .hero
                .tariq()
                .kill_em_all_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SquareHammer => self
                .hero
                .tariq()
                .square_hammer_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SquareHammerExpertise => self
                .hero
                .tariq()
                .square_hammer_expertise_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SchismHammerStorm => self
                .hero
                .tariq()
                .schism_skull_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SchismSkullCrusher => self
                .hero
                .tariq()
                .schism_hammer_until
                .saturating_sub(self.common.now_ms),
            AplBuff::ExecutionersGrin => self
                .hero
                .tariq()
                .executioners_grin_until
                .saturating_sub(self.common.now_ms),
            AplBuff::CelestialImpetus => self
                .hero
                .elarion()
                .celestial_impetus_until
                .saturating_sub(self.common.now_ms),
            AplBuff::EmpoweredMultishot => self
                .hero
                .elarion()
                .empowered_multishot_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SkystridersGrace => self
                .hero
                .elarion()
                .skystriders_grace_until
                .saturating_sub(self.common.now_ms),
            AplBuff::EventHorizon => self
                .hero
                .elarion()
                .event_horizon_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SkystridersSupremacy => self
                .hero
                .elarion()
                .skystriders_supremacy_until
                .saturating_sub(self.common.now_ms),
            AplBuff::ImpendingHeartseeker => self
                .hero
                .elarion()
                .impending_heartseeker_until
                .saturating_sub(self.common.now_ms),
            AplBuff::ResurgentWinds => self
                .hero
                .elarion()
                .resurgent_winds_until
                .saturating_sub(self.common.now_ms),
            AplBuff::BroodingShadows => {
                u64::from(self.hero.mara().stealth_active)
                    * ENCOUNTER_DURATION_MS.saturating_sub(self.common.now_ms)
            }
            AplBuff::MaidenOfDeath => self
                .hero
                .mara()
                .maiden_of_death_until
                .saturating_sub(self.common.now_ms),
            AplBuff::MatriarchMacabre => self
                .hero
                .mara()
                .matriarch_macabre_until
                .saturating_sub(self.common.now_ms),
            AplBuff::AssassinsGuile => self
                .hero
                .mara()
                .assassins_guile_until
                .saturating_sub(self.common.now_ms),
            AplBuff::DeadlyScheme => self
                .hero
                .mara()
                .deadly_scheme_until
                .saturating_sub(self.common.now_ms),
            AplBuff::FeedTheQueen => self
                .hero
                .mara()
                .feed_the_queen_until
                .saturating_sub(self.common.now_ms),
            AplBuff::MalevolenceArachnid => self
                .hero
                .mara()
                .malevolence_arachnid_until
                .saturating_sub(self.common.now_ms),
            AplBuff::MalevolenceQueen => self
                .hero
                .mara()
                .malevolence_queen_until
                .saturating_sub(self.common.now_ms),
            AplBuff::DrenchedInBlood => self
                .hero
                .mara()
                .drenched_in_blood_until
                .saturating_sub(self.common.now_ms),
            AplBuff::SerratedEdge => self
                .hero
                .gunde()
                .serrated_edge_until
                .saturating_sub(self.common.now_ms),
            AplBuff::ReignInBlood => self
                .hero
                .gunde()
                .reign_in_blood_until
                .saturating_sub(self.common.now_ms),
            AplBuff::BloodboundSpirit => self
                .hero
                .gunde()
                .bloodbound_spirit_until
                .saturating_sub(self.common.now_ms),
            AplBuff::DeathsArc => self
                .hero
                .gunde()
                .deaths_arc_until
                .saturating_sub(self.common.now_ms),
            AplBuff::GrimHarvest => self
                .hero
                .gunde()
                .grim_harvest_until
                .saturating_sub(self.common.now_ms),
            AplBuff::HarvestersToll => self
                .hero
                .gunde()
                .harvesters_toll_until
                .saturating_sub(self.common.now_ms),
            AplBuff::CrimsonStrikes => self
                .hero
                .gunde()
                .crimson_strikes_until
                .saturating_sub(self.common.now_ms),
            AplBuff::MurderOfCrows => self
                .hero
                .gunde()
                .murder_of_crows_until
                .saturating_sub(self.common.now_ms),
            AplBuff::Massacre => self
                .hero
                .gunde()
                .massacre_until
                .saturating_sub(self.common.now_ms),
            AplBuff::AncestralInstinct => self
                .hero
                .gunde()
                .ancestral_instinct_until
                .saturating_sub(self.common.now_ms),
            AplBuff::Bloodbath => self
                .hero
                .gunde()
                .bloodbath_until
                .saturating_sub(self.common.now_ms),
            AplBuff::CarrionOnslaught => self
                .hero
                .gunde()
                .carrion_onslaught_until
                .saturating_sub(self.common.now_ms),
            AplBuff::OpenWounds => {
                self.hero.gunde().open_wounds_until[0].saturating_sub(self.common.now_ms)
            }
        }
    }
}
