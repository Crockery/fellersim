mod allocation_counter {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    };

    pub(super) struct CountingAllocator;

    static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
    static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
    static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
    static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
    static BASELINE_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_allocation(layout.size());
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record_allocation(layout.size());
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
            let new_pointer = unsafe { System.realloc(pointer, old_layout, new_size) };
            if !new_pointer.is_null() {
                if new_size >= old_layout.size() {
                    record_allocation(new_size - old_layout.size());
                } else {
                    LIVE_BYTES.fetch_sub(old_layout.size() - new_size, Ordering::Relaxed);
                }
            }
            new_pointer
        }
    }

    fn record_allocation(bytes: usize) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
        let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
        PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct AllocationMetrics {
        pub(super) allocations: u64,
        pub(super) allocated_bytes: u64,
        pub(super) peak_live_bytes: usize,
    }

    pub(super) fn reset() {
        let live = LIVE_BYTES.load(Ordering::Relaxed);
        BASELINE_LIVE_BYTES.store(live, Ordering::Relaxed);
        PEAK_LIVE_BYTES.store(live, Ordering::Relaxed);
        ALLOCATIONS.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    }

    pub(super) fn metrics() -> AllocationMetrics {
        AllocationMetrics {
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            peak_live_bytes: PEAK_LIVE_BYTES
                .load(Ordering::Relaxed)
                .saturating_sub(BASELINE_LIVE_BYTES.load(Ordering::Relaxed)),
        }
    }
}

#[global_allocator]
static TEST_ALLOCATOR: allocation_counter::CountingAllocator =
    allocation_counter::CountingAllocator;

fn compiled_ability(source: &DpsAbilityModel) -> CompiledAbility {
    CompiledAbility::compile(source).expect("test ability parameters compile")
}

fn fire_frog_mechanic_parameters() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("frogCount".into(), 5.0),
        ("attacksPerFrog".into(), 3.0),
        ("damageToDotTransferFraction".into(), 1.0),
        ("assumedServerTickRateHz".into(), 30.0),
        ("initialPathDistanceUnits".into(), 3_000.0),
        ("spawnRadiusUnits".into(), 30.0),
        ("minimumBatchSpawnDelaySeconds".into(), 0.1),
        ("maximumBatchSpawnDelaySeconds".into(), 0.2),
        ("attackRangeUnits".into(), 400.0),
        ("jumpDurationSeconds".into(), 0.3),
        ("minimumJumpLengthUnits".into(), 280.0),
        ("maximumJumpLengthUnits".into(), 320.0),
        ("minimumJumpPeriodSeconds".into(), 0.3),
        ("maximumJumpPeriodSeconds".into(), 0.7),
    ])
}

fn ability(kind: DpsAbilityKind, coefficient: f64) -> DpsAbilityModel {
    let mechanic_parameters = if kind == DpsAbilityKind::Detonate {
        BTreeMap::from([
            ("initialDelaySeconds".into(), 0.5),
            ("perTargetHitDelaySeconds".into(), 0.4),
            ("hitsPerTarget".into(), 3.0),
            ("betweenHitDelaySeconds".into(), 0.3),
            ("targetCountDamageScalingThreshold".into(), 1.0),
            ("spiritRefundDelaySeconds".into(), 0.2),
            ("spiritRefundSpiritGain".into(), 1.0),
            ("spiritRefundChanceScale".into(), 1.0),
            ("spiritRefundChanceFlatIncrease".into(), 0.0),
        ])
    } else if kind == DpsAbilityKind::FireFrogs {
        fire_frog_mechanic_parameters()
    } else {
        BTreeMap::new()
    };
    DpsAbilityModel {
        id: format!("test:{}", ability_kind_id(kind)),
        name: uptime_name(ability_kind_id(kind)),
        kind,
        manually_castable: true,
        power_coefficient: coefficient,
        damage_spread: 0.0,
        cast_time_ms: 0,
        gcd_ms: 1_000,
        scale_time_with_haste: true,
        gcd_haste_mode: DpsGcdHasteMode::Standard,
        gcd_scales_with_cooldown_reduction: false,
        gcd_scales_with_cooldown_recovery: false,
        cooldown_ms: 0,
        cooldown_scales_with_haste: false,
        cooldown_scales_with_cooldown_recovery: true,
        maximum_charges: 1,
        effect_duration_ms: 0,
        direct_hits: 1,
        max_targets: 1,
        first_hit_delay_ms: 0,
        hit_interval_ms: 0,
        primary_resource_generated: 0.0,
        secondary_resource_cost: u32::from(kind == DpsAbilityKind::Detonate),
        spirit_cost: if kind == DpsAbilityKind::Incinerate {
            10.0
        } else {
            0.0
        },
        channel: None,
        dot: None,
        applies_dot_kind: None,
        dot_application_delay_ms: 0,
        dot_extension_ms: 0,
        off_gcd: false,
        mechanic_parameters,
    }
}

fn dot_ability(kind: DpsAbilityKind, coefficient: f64) -> DpsAbilityModel {
    DpsAbilityModel {
        dot: Some(DotModel {
            power_coefficient: coefficient,
            damage_spread: 0.0,
            duration_ms: 10_000,
            period_ms: 1_000,
            can_crit: false,
            cinder_proc_chance: 0.0,
            cinders_on_proc: 0.0,
            stack_damage_increase: 0.0,
            maximum_stacks: 1,
        }),
        ..ability(kind, 0.0)
    }
}

fn profile(abilities: Vec<DpsAbilityModel>) -> NormalizedDpsProfile {
    let mut uptime_names =
        BTreeMap::from([("buff:spirit-of-heroism".into(), "Spirit of Heroism".into())]);
    for ability in &abilities {
        uptime_names.insert(
            format!("dot:{}", ability_kind_id(ability.kind)),
            ability.name.clone(),
        );
        uptime_names.insert(
            format!("buff:{}", ability_kind_id(ability.kind)),
            ability.name.clone(),
        );
    }
    NormalizedDpsProfile {
        hero_id: ARDEOS_HERO_ID.into(),
        evidence_claim_ids: vec!["test:claim".into()],
        power: 100.0,
        critical_strike: 0.0,
        critical_rating: 0.0,
        critical_multiplier: 2.0,
        expertise: 0.0,
        expertise_rating: 0.0,
        haste: 0.0,
        haste_rating: 0.0,
        cooldown_recovery: 1.0,
        spirit: 0.0,
        spirit_rating: 0.0,
        max_primary_resource: 100.0,
        max_secondary_resource: 3,
        max_spirit: 100.0,
        heroism_haste: 0.0,
        heroism_duration_ms: 5_000,
        abilities,
        apl_target_effects: Vec::new(),
        scenario_no_op_abilities: Vec::new(),
        talents: Vec::new(),
        mechanics: Vec::new(),
        uptime_names,
    }
}

fn rime_ability(kind: DpsAbilityKind) -> DpsAbilityModel {
    let coefficient = match kind {
        DpsAbilityKind::FrostBolt
        | DpsAbilityKind::GlacialBlast
        | DpsAbilityKind::FreezingTorrent
        | DpsAbilityKind::ColdSnap
        | DpsAbilityKind::IceComet
        | DpsAbilityKind::AnimaSpike => 1.0,
        _ => 0.0,
    };
    let mut model = ability(kind, coefficient);
    match kind {
        DpsAbilityKind::IceBlitz => {
            model.effect_duration_ms = 20_000;
            model.mechanic_parameters = BTreeMap::from([
                ("damageMultiplier".into(), 1.2),
                ("extensionSeconds".into(), 0.1),
            ]);
        }
        DpsAbilityKind::BurstingIce => {
            model.effect_duration_ms = 3_000;
            model.max_targets = 19;
            model.mechanic_parameters = BTreeMap::from([
                ("pulsePowerCoefficient".into(), 0.498),
                ("pulsePeriodSeconds".into(), 0.5),
                ("targetCountDamageScalingThreshold".into(), 5.0),
                ("animaPerPulse".into(), 1.0),
                ("animaPerPulseCap".into(), 1.0),
            ]);
        }
        DpsAbilityKind::FrostBolt | DpsAbilityKind::GlacialBlast => {
            model.mechanic_parameters = BTreeMap::from([
                ("assumedProjectileImpactDelaySeconds".into(), 1.5),
                ("serverTickRateCapHz".into(), 30.0),
            ]);
            model.first_hit_delay_ms = 1_500;
            if kind == DpsAbilityKind::FrostBolt {
                model.primary_resource_generated = 3.0;
            } else {
                model.secondary_resource_cost = 2;
            }
        }
        DpsAbilityKind::FreezingTorrent => {
            model.channel = Some(ChannelModel {
                duration_ms: 2_000,
                tick_interval_ms: 400,
                tick_immediately: true,
                scale_duration_with_ability_time_rate: false,
                enable_partial_ticks: true,
                scale_partial_tick_damage: true,
            });
        }
        DpsAbilityKind::WintersBlessing => {
            model.effect_duration_ms = 20_000;
            model.mechanic_parameters = BTreeMap::from([
                ("spiritMultiplier".into(), 1.2),
                ("healingBatchSeconds".into(), 0.5),
                ("damageToHealingFactor".into(), 0.3),
            ]);
        }
        DpsAbilityKind::WrathOfWinter => {
            model.effect_duration_ms = 20_000;
            model.spirit_cost = 100.0;
            model.mechanic_parameters = BTreeMap::from([
                ("damageMultiplier".into(), 1.2),
                ("volleyPeriodSeconds".into(), 4.0),
                ("volleyProjectiles".into(), 3.0),
            ]);
        }
        DpsAbilityKind::ColdSnap => {
            model.maximum_charges = 2;
            model.mechanic_parameters = BTreeMap::from([
                ("orbGain".into(), 1.0),
                ("extraCooldownAccelerationPerHaste".into(), 1.0),
            ]);
        }
        DpsAbilityKind::IceComet => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.first_hit_delay_ms = 300;
            model.secondary_resource_cost = 2;
            model.mechanic_parameters = BTreeMap::from([
                ("targetCountDamageScalingThreshold".into(), 12.0),
                ("initialSpawnDelaySeconds".into(), 1.0),
                ("pulsePeriodSeconds".into(), 0.2),
            ]);
        }
        DpsAbilityKind::FlightOfTheNavir => {
            model.effect_duration_ms = 20_000;
            model.mechanic_parameters = BTreeMap::from([
                ("projectileCount".into(), 5.0),
                ("projectilePowerCoefficient".into(), 0.469),
                ("damageIncreasePerSpirit".into(), 1.5),
                ("assumedProjectileImpactDelaySeconds".into(), 0.5),
                ("serverTickRateCapHz".into(), 30.0),
            ]);
        }
        DpsAbilityKind::AnimaSpike => {
            model.manually_castable = false;
            model.mechanic_parameters = BTreeMap::from([
                ("projectileCount".into(), 3.0),
                ("orbGain".into(), 1.0),
                ("assumedProjectileImpactDelaySeconds".into(), 0.5),
                ("serverTickRateCapHz".into(), 30.0),
            ]);
        }
        _ => {}
    }
    model
}

fn rime_profile() -> NormalizedDpsProfile {
    let mut profile = profile(
        [
            DpsAbilityKind::IceBlitz,
            DpsAbilityKind::BurstingIce,
            DpsAbilityKind::FrostBolt,
            DpsAbilityKind::GlacialBlast,
            DpsAbilityKind::FreezingTorrent,
            DpsAbilityKind::WintersBlessing,
            DpsAbilityKind::WrathOfWinter,
            DpsAbilityKind::ColdSnap,
            DpsAbilityKind::IceComet,
            DpsAbilityKind::FlightOfTheNavir,
            DpsAbilityKind::AnimaSpike,
        ]
        .into_iter()
        .map(rime_ability)
        .collect(),
    );
    profile.hero_id = RIME_HERO_ID.into();
    profile.max_primary_resource = 9.0;
    profile.max_secondary_resource = 5;
    profile
}

fn tariq_ability(kind: DpsAbilityKind) -> DpsAbilityModel {
    let coefficient = match kind {
        DpsAbilityKind::HammerStorm
        | DpsAbilityKind::HeavyStrike
        | DpsAbilityKind::TariqChainLightning
        | DpsAbilityKind::WildSwing
        | DpsAbilityKind::RagingTempest
        | DpsAbilityKind::SkullCrusher
        | DpsAbilityKind::CullingStrike
        | DpsAbilityKind::LeapSmash
        | DpsAbilityKind::FaceBreaker
        | DpsAbilityKind::TariqAttack => 1.0,
        _ => 0.0,
    };
    let mut model = ability(kind, coefficient);
    match kind {
        DpsAbilityKind::HammerStorm => {
            model.channel = Some(ChannelModel {
                duration_ms: 600,
                tick_interval_ms: 200,
                tick_immediately: true,
                scale_duration_with_ability_time_rate: false,
                enable_partial_ticks: false,
                scale_partial_tick_damage: false,
            });
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("tickDamageMultiplier".into(), 1.35),
                ("maximumFuryCost".into(), 25.0),
                ("targetCountDamageScalingThreshold".into(), 12.0),
                ("lightningPowerCoefficient".into(), 0.5),
                ("lightningTargetCountDamageScalingThreshold".into(), 15.0),
                ("lightningDelaySeconds".into(), 0.0),
            ]);
        }
        DpsAbilityKind::HeavyStrike => {
            model.mechanic_parameters = BTreeMap::from([
                ("weakDamageMultiplier".into(), 0.2),
                ("weakResourceMultiplier".into(), 0.2),
                ("cleaveDamageMultiplier".into(), 0.3),
                ("cleaveTargetCountDamageScalingThreshold".into(), 8.0),
                ("lightningPowerCoefficient".into(), 0.5),
                ("lightningTargetCountDamageScalingThreshold".into(), 8.0),
                ("lightningDelaySeconds".into(), 0.0),
            ]);
        }
        DpsAbilityKind::TariqChainLightning => {
            model.max_targets = 3;
            model.mechanic_parameters = BTreeMap::from([
                ("chainHits".into(), 3.0),
                ("jumpDelaySeconds".into(), 0.1),
                ("uniqueTargetDamageIncrease".into(), 0.5),
                ("furyPerHit".into(), 2.0),
            ]);
        }
        DpsAbilityKind::TariqAttack => {
            model.manually_castable = false;
            model.first_hit_delay_ms = 600;
            model.gcd_ms = 0;
            model.off_gcd = true;
            model.mechanic_parameters = BTreeMap::from([("swingDurationSeconds".into(), 30.0), ("hitWindowSeconds".into(), 29.4)]);
        }
        DpsAbilityKind::ThunderCall => model.effect_duration_ms = 5_000,
        DpsAbilityKind::WildSwing | DpsAbilityKind::LeapSmash => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters =
                BTreeMap::from([("targetCountDamageScalingThreshold".into(), 8.0)]);
        }
        DpsAbilityKind::FocusedWrath => {
            model.effect_duration_ms = 5_000;
            model.mechanic_parameters = BTreeMap::from([
                ("maximumStacks".into(), 3.0),
                ("costMultiplier".into(), 0.5),
                ("damageMultiplier".into(), 1.1),
                ("stacks".into(), 2.0),
            ]);
        }
        DpsAbilityKind::RagingTempest => {
            model.effect_duration_ms = 3_000;
            model.spirit_cost = 100.0;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("pulseVisualDelaySeconds".into(), 0.0),
                ("pulseDamageSpread".into(), 0.0),
                ("pulsePowerCoefficient".into(), 0.5),
                ("pulsePeriodSeconds".into(), 1.0),
                ("expertisePerStack".into(), 0.01),
                ("maximumStacks".into(), 20.0),
                ("targetCountDamageScalingThreshold".into(), 5.0),
            ]);
        }
        DpsAbilityKind::SkullCrusher => {
            model.mechanic_parameters = BTreeMap::from([
                ("cleaveDelaySeconds".into(), 0.5),
                ("furyCost".into(), 25.0),
                ("lightningPowerCoefficient".into(), 0.5),
                ("lightningDelaySeconds".into(), 0.0),
                ("cleaveDamageMultiplier".into(), 0.1),
                ("cleaveTargetCountDamageScalingThreshold".into(), 8.0),
            ]);
        }
        DpsAbilityKind::CullingStrike => {
            model.mechanic_parameters = BTreeMap::from([
                ("maximumFuryCost".into(), 10.0),
                ("damageIncreasePerFuryFraction".into(), 0.04),
            ]);
        }
        DpsAbilityKind::FaceBreaker => {
            model.mechanic_parameters = BTreeMap::from([
                ("cleaveDelaySeconds".into(), 0.3),
                ("cleaveDamageMultiplier".into(), 0.5),
                ("cleaveTargetCountDamageScalingThreshold".into(), 12.0),
            ]);
        }
        _ => {}
    }
    model
        .mechanic_parameters
        .insert("spiritRefundDelaySeconds".into(), 0.2);
    model
        .mechanic_parameters
        .insert("spiritRefundSpiritGain".into(), 1.0);
    model
}

fn tariq_profile() -> NormalizedDpsProfile {
    let mut profile = profile(
        [
            DpsAbilityKind::HammerStorm,
            DpsAbilityKind::HeavyStrike,
            DpsAbilityKind::TariqChainLightning,
            DpsAbilityKind::ThunderCall,
            DpsAbilityKind::WildSwing,
            DpsAbilityKind::FocusedWrath,
            DpsAbilityKind::RagingTempest,
            DpsAbilityKind::SkullCrusher,
            DpsAbilityKind::CullingStrike,
            DpsAbilityKind::LeapSmash,
            DpsAbilityKind::FaceBreaker,
            DpsAbilityKind::TariqAttack,
        ]
        .into_iter()
        .map(tariq_ability)
        .collect(),
    );
    profile.hero_id = TARIQ_HERO_ID.into();
    profile.max_primary_resource = 100.0;
    profile.max_secondary_resource = 100;
    profile
}

fn tariq_talent(
    number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DpsTalentModel {
    let mechanic_id = match number {
        2 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent2",
        5 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent5",
        14 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent10",
        8 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent8",
        11 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait9",
        6 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent6",
        7 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent7",
        9 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-talent9",
        13 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-passivetalent4",
        18 => "augmentation:fellowship-content-abilities-talents-ink-caa-talent-ink-trait7",
        _ => panic!("test Tariq talent number must have a current fixture mapping"),
    };
    DpsTalentModel {
        id: format!("ink-talent-id-talent{number}"),
        name: format!("Tariq talent {number}"),
        mechanic_id: mechanic_id.into(),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn elarion_ability(kind: DpsAbilityKind) -> DpsAbilityModel {
    let mut model = ability(
        kind,
        if matches!(
            kind,
            DpsAbilityKind::Multishot
                | DpsAbilityKind::FocusedShot
                | DpsAbilityKind::HighwindArrow
                | DpsAbilityKind::HeartseekerBarrage
                | DpsAbilityKind::CelestialShot
                | DpsAbilityKind::ElarionShoot
                | DpsAbilityKind::LunarlightSalvo
                | DpsAbilityKind::LunarlightEruption
        ) {
            1.0
        } else {
            0.0
        },
    );
    match kind {
        DpsAbilityKind::ElarionShoot => {
            model.manually_castable = false;
            model.off_gcd = true;
            model.gcd_ms = 0;
            model.mechanic_parameters.insert("swingDurationSeconds".into(), 2.4);
        }
        DpsAbilityKind::Multishot => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("focusCost".into(), 0.0),
                ("targetCountDamageScalingThreshold".into(), 20.0),
                ("procDamageMultiplier".into(), 2.0),
                ("maximumProcStacks".into(), 5.0),
                ("empoweredMinimumProjectiles".into(), 3.0),
                ("empoweredCostMultiplier".into(), 0.5),
            ]);
        }
        DpsAbilityKind::HighwindArrow => {
            model.max_targets = 3;
            model.cooldown_ms = 10_000;
            model.mechanic_parameters = BTreeMap::from([
                ("focusCost".into(), 0.0),
                ("bounceDamageMultiplier".into(), 0.5),
                ("maximumBounces".into(), 2.0),
                ("minimumTargetsForMultishotProc".into(), 3.0),
                ("resurgentDamageMultiplier".into(), 1.5),
            ]);
        }
        DpsAbilityKind::HeartseekerBarrage => {
            model.cooldown_ms = 10_000;
            model.channel = Some(ChannelModel {
                duration_ms: 300,
                tick_interval_ms: 100,
                tick_immediately: false,
                scale_duration_with_ability_time_rate: false,
                enable_partial_ticks: false,
                scale_partial_tick_damage: false,
            });
            model.mechanic_parameters = BTreeMap::from([
                ("focusCost".into(), 0.0),
                ("impendingDamageIncreasePerProjectile".into(), 0.1),
            ]);
        }
        DpsAbilityKind::LunarlightMark => {
            model.effect_duration_ms = 10_000;
            model.mechanic_parameters = BTreeMap::from([
                ("stacksApplied".into(), 2.0),
                ("maximumStacks".into(), 5.0),
                ("salvoProcChance".into(), 0.0),
                ("eruptionProcChance".into(), 0.0),
                ("salvoCriticalProcChance".into(), 0.0),
            ]);
        }
        DpsAbilityKind::LunarlightEruption => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters =
                BTreeMap::from([("targetCountDamageScalingThreshold".into(), 12.0)]);
        }
        DpsAbilityKind::CelestialShot => {
            model.mechanic_parameters = BTreeMap::from([("focusCost".into(), 0.0)]);
        }
        _ => {}
    }
    model
        .mechanic_parameters
        .insert("spiritRefundDelaySeconds".into(), 0.2);
    model
        .mechanic_parameters
        .insert("spiritRefundSpiritGain".into(), 1.0);
    model
        .mechanic_parameters
        .insert("spiritRefundMainTargetStacks".into(), 5.0);
    model
        .mechanic_parameters
        .insert("spiritRefundAdditionalTargetStacks".into(), 2.0);
    model
}

fn elarion_profile(abilities: impl IntoIterator<Item = DpsAbilityModel>) -> NormalizedDpsProfile {
    let mut profile = profile(abilities.into_iter().collect());
    profile.hero_id = ELARION_HERO_ID.into();
    profile
}

fn elarion_talent(
    number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DpsTalentModel {
    let mechanic_id = match number {
        1 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent1",
        2 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-trait3",
        4 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent8",
        5 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent2",
        6 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent9",
        13 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent7",
        3 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent3",
        7 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent10",
        10 => {
            "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-passivetalent7"
        }
        11 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent14",
        14 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent11",
        15 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent12",
        17 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent15",
        18 => "augmentation:fellowship-content-abilities-talents-bowguy-caa-talent-bowguy-talent4",
        _ => panic!("test Elarion talent number must have a current fixture mapping"),
    };
    DpsTalentModel {
        id: format!("bowguy-talent-id-talent{number}"),
        name: format!("Elarion talent {number}"),
        mechanic_id: mechanic_id.into(),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn mara_ability(kind: DpsAbilityKind) -> DpsAbilityModel {
    let coefficient = if matches!(
        kind,
        DpsAbilityKind::SkitteringBlades
            | DpsAbilityKind::ArachnidAssault
            | DpsAbilityKind::MaraAttack
            | DpsAbilityKind::HemorrhagingStrike
            | DpsAbilityKind::WidowsBite
            | DpsAbilityKind::QueensFang
            | DpsAbilityKind::Backstab
            | DpsAbilityKind::CausticPoison
            | DpsAbilityKind::VolatilePoisonEruption
            | DpsAbilityKind::SeethingBurst
            | DpsAbilityKind::HemotoxinEruption
    ) {
        1.0
    } else {
        0.0
    };
    let mut model = ability(kind, coefficient);
    match kind {
        DpsAbilityKind::SkitteringBlades => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("energyCost".into(), 35.0),
                ("comboPointsPerHit".into(), 1.0),
                ("comboPointsPerCriticalHit".into(), 2.0),
                ("targetCountDamageScalingThreshold".into(), 8.0),
            ]);
        }
        DpsAbilityKind::ArachnidAssault => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("energyCost".into(), 45.0),
                ("damageMultiplierPerComboPoint".into(), 0.2),
                ("targetCountDamageScalingThreshold".into(), 8.0),
            ]);
        }
        DpsAbilityKind::MaraAttack => {
            model.mechanic_parameters = BTreeMap::from([
                ("energyRegenerationPerSecond".into(), 15.0),
                ("energyRegenerationHasteScaler".into(), 1.0),
                ("spiritRefundDelaySeconds".into(), 0.2),
                ("creepingDeathHasteScaler".into(), 1.0),
            ]);
        }
        DpsAbilityKind::HemorrhagingStrike => {
            model.cooldown_ms = 2_000;
            model.dot = Some(DotModel {
                power_coefficient: 1.0,
                damage_spread: 0.0,
                duration_ms: 12_000,
                period_ms: 3_000,
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            });
            model.mechanic_parameters = BTreeMap::from([
                ("energyCost".into(), 20.0),
                ("bleedDurationPerComboPointSeconds".into(), 3.0),
                ("bleedRefreshCarryOverFraction".into(), 0.3),
                ("energyPerBleedTick".into(), 2.0),
            ]);
        }
        DpsAbilityKind::WidowsBite => {
            model.cooldown_ms = 9_000;
            model.direct_hits = 2;
            model.maximum_charges = 3;
            model.mechanic_parameters = BTreeMap::from([
                ("energyGain".into(), 30.0),
                ("poisonAdditionalDelaySeconds".into(), 0.0),
                ("comboPointsPerHit".into(), 2.0),
                ("comboPointsPerCriticalHit".into(), 3.0),
                ("secondaryHitMultiplier".into(), 0.75),
            ]);
        }
        DpsAbilityKind::MaidenOfDeath => {
            model.cooldown_ms = 60_000;
            model.gcd_ms = 0;
            model.off_gcd = true;
            model.effect_duration_ms = 10_000;
            model.mechanic_parameters = BTreeMap::from([
                ("energyGenerationMultiplier".into(), 1.2),
                ("damageMultiplier".into(), 1.2),
            ]);
        }
        DpsAbilityKind::MatriarchMacabre => {
            model.effect_duration_ms = 20_000;
            model.spirit_cost = 100.0;
            model.mechanic_parameters = BTreeMap::from([
                ("damageMultiplier".into(), 1.2),
                ("cloneDamageMultiplier".into(), 0.5),
                ("copyActivationDelaySeconds".into(), 0.3),
            ]);
        }
        DpsAbilityKind::QueensFang => {
            model.mechanic_parameters = BTreeMap::from([
                ("energyCost".into(), 40.0),
                ("damageMultiplierPerComboPoint".into(), 0.2),
            ]);
        }
        DpsAbilityKind::Backstab => {
            model.mechanic_parameters = BTreeMap::from([
                ("energyCost".into(), 20.0),
                ("comboPointsPerHit".into(), 2.0),
                ("comboPointsPerCriticalHit".into(), 3.0),
                ("behindDamageMultiplier".into(), 1.4),
                ("poisonComboPointMultiplier".into(), 3.0),
                ("poisonAdditionalDelaySeconds".into(), 0.0),
                ("causticCriticalStrikeBonus".into(), 1.0),
            ]);
        }
        DpsAbilityKind::BroodingShadows => {
            model.cooldown_ms = 15_000;
            model.cooldown_scales_with_haste = true;
            model.gcd_ms = 0;
            model.off_gcd = true;
        }
        DpsAbilityKind::FinalStratagem => {
            model.cooldown_ms = 180_000;
            model.gcd_ms = 0;
            model.off_gcd = true;
        }
        DpsAbilityKind::SeethingPoison
        | DpsAbilityKind::VolatilePoison
        | DpsAbilityKind::CorrosiveSpill
        | DpsAbilityKind::Hemotoxin => {
            model.manually_castable = false;
            model.dot = Some(DotModel {
                power_coefficient: 1.0,
                damage_spread: 0.0,
                duration_ms: 9_000,
                period_ms: 1_000,
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: if kind == DpsAbilityKind::Hemotoxin {
                    3
                } else {
                    1
                },
            });
            if kind == DpsAbilityKind::VolatilePoison {
                model.mechanic_parameters = BTreeMap::from([
                    ("minimumDurationSeconds".into(), 6.0),
                    ("maximumDurationSeconds".into(), 8.0),
                    ("maximumTicks".into(), 6.0),
                ]);
            }
            if kind == DpsAbilityKind::Hemotoxin {
                model.mechanic_parameters.insert("refreshCarryOverFraction".into(), 0.3);
            }
            if kind == DpsAbilityKind::SeethingPoison {
                model
                    .mechanic_parameters
                    .insert("energyRegenerationMultiplier".into(), 1.2);
            }
        }
        DpsAbilityKind::VolatilePoisonEruption => {
            model.manually_castable = false;
        }
        DpsAbilityKind::HemotoxinEruption => {
            model.manually_castable = false;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("primaryBleedFraction".into(), 0.5),
                ("areaBleedFraction".into(), 0.25),
                ("targetCountDamageScalingThreshold".into(), 1.0),
            ]);
        }
        DpsAbilityKind::CausticPoison => model.manually_castable = false,
        DpsAbilityKind::SeethingBurst => {
            model.manually_castable = false;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters.insert("targetCountDamageScalingThreshold".into(), 10.0);
        }
        _ => {}
    }
    model
}

fn mara_profile() -> NormalizedDpsProfile {
    let mut profile = profile(
        [
            DpsAbilityKind::SkitteringBlades,
            DpsAbilityKind::ArachnidAssault,
            DpsAbilityKind::MaraAttack,
            DpsAbilityKind::HemorrhagingStrike,
            DpsAbilityKind::WidowsBite,
            DpsAbilityKind::MaidenOfDeath,
            DpsAbilityKind::MatriarchMacabre,
            DpsAbilityKind::QueensFang,
            DpsAbilityKind::BroodingShadows,
            DpsAbilityKind::Backstab,
            DpsAbilityKind::FinalStratagem,
            DpsAbilityKind::CausticPoison,
            DpsAbilityKind::SeethingPoison,
            DpsAbilityKind::VolatilePoison,
            DpsAbilityKind::VolatilePoisonEruption,
            DpsAbilityKind::SeethingBurst,
            DpsAbilityKind::CorrosiveSpill,
            DpsAbilityKind::Hemotoxin,
            DpsAbilityKind::HemotoxinEruption,
        ]
        .into_iter()
        .map(mara_ability)
        .collect(),
    );
    profile.hero_id = MARA_HERO_ID.into();
    profile.max_primary_resource = 200.0;
    profile.max_secondary_resource = 6;
    profile
}

fn mara_talent(
    number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DpsTalentModel {
    let mechanic_id = match number {
        2 => "talent:mara:malevolence",
        3 => "talent:mara:deadly-scheme",
        4 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-trait3",
        14 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent13",
        16 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-talent9",
        5 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-passivetalent2",
        10 => "augmentation:fellowship-content-abilities-talents-mara-caa-mara-trait1",
        11 => "augmentation:fellowship-content-abilities-talents-mara-caa-mara-trait2",
        17 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-passivetalent4",
        6 => "talent:mara:efficient-killer",
        12 => "talent:mara:sinners-pride",
        18 => "talent:mara:arachnid-onslaught",
        19 => "talent:mara:bloodrush",
        8 => "talent:mara:macabre-stratagem",
        15 => "talent:mara:feed-the-queen",
        13 => "augmentation:fellowship-content-abilities-talents-mara-caa-talent-mara-trait1",
        _ => panic!("test Mara talent number must have a current fixture mapping"),
    };
    DpsTalentModel {
        id: format!("mara-talent-id-talent{number}"),
        name: format!("Mara talent {number}"),
        mechanic_id: mechanic_id.into(),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn gunde_ability(kind: DpsAbilityKind) -> DpsAbilityModel {
    let coefficient = if matches!(
        kind,
        DpsAbilityKind::DoubleStrike
            | DpsAbilityKind::Warbound
            | DpsAbilityKind::BloodboundSpirit
            | DpsAbilityKind::HeartSplitter
            | DpsAbilityKind::Rupture
            | DpsAbilityKind::BloodArc
            | DpsAbilityKind::ReaversEdge
            | DpsAbilityKind::GundeAttack
            | DpsAbilityKind::ButchersHook
            | DpsAbilityKind::GrimCarve
            | DpsAbilityKind::Bloodcraze
            | DpsAbilityKind::RavensPrecision
    ) {
        1.0
    } else {
        0.0
    };
    let mut model = ability(kind, coefficient);
    match kind {
        DpsAbilityKind::DoubleStrike => {
            model.direct_hits = 2;
            model
                .mechanic_parameters
                .insert("rendTransferFraction".into(), 0.5);
        }
        DpsAbilityKind::Warbound => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model
                .mechanic_parameters
                .insert("rendTransferFraction".into(), 0.5);
        }
        DpsAbilityKind::OwedInBlood => {
            model.mechanic_parameters = BTreeMap::from([
                ("maximumBloodFeathers".into(), 5.0),
                ("bloodFeatherDurationSeconds".into(), 45.0),
                ("rendDamagePerFeather".into(), 400.0),
                ("bloodcrazePowerCoefficient".into(), 1.0),
                ("bloodcrazePulsePeriodSeconds".into(), 1.0),
                ("bloodcrazePulses".into(), 5.0),
            ]);
        }
        DpsAbilityKind::BloodboundSpirit => {
            model.effect_duration_ms = 10_000;
            model.spirit_cost = 100.0;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.dot = Some(DotModel {
                power_coefficient: 1.0,
                damage_spread: 0.0,
                duration_ms: 5_000,
                period_ms: 1_000,
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            });
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 0.5),
                ("damageMultiplier".into(), 1.2),
            ]);
        }
        DpsAbilityKind::ReignInBlood => {
            model.effect_duration_ms = 10_000;
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 0.3),
                ("talentRendTransferFraction".into(), 0.25),
            ]);
        }
        DpsAbilityKind::HeartSplitter => {
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 1.0),
                ("exsanguinateFraction".into(), 0.3),
            ]);
        }
        DpsAbilityKind::Rupture => {
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 0.5),
                ("openWoundsDamageMultiplier".into(), 1.25),
                ("openWoundsDurationSeconds".into(), 6.0),
            ]);
        }
        DpsAbilityKind::Slaughter => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.dot = Some(DotModel {
                power_coefficient: 0.0,
                damage_spread: 0.0,
                duration_ms: 3_000,
                period_ms: 1_000,
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            });
            model
                .mechanic_parameters
                .insert("consumedRendDamageMultiplier".into(), 1.6);
        }
        DpsAbilityKind::BloodArc => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 0.5),
                ("serratedEdgeTransferFraction".into(), 0.2),
                ("serratedEdgeDurationSeconds".into(), 10.0),
                ("targetCountDamageScalingThreshold".into(), 8.0),
            ]);
        }
        DpsAbilityKind::ReaversEdge | DpsAbilityKind::GrimCarve => {
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("rendTransferFraction".into(), 0.5),
                ("targetCountDamageScalingThreshold".into(), 8.0),
            ]);
            if kind == DpsAbilityKind::GrimCarve {
                model.mechanic_parameters.insert("projectileSpawnDelaySeconds".into(), 0.0);
                model.direct_hits = 3;
            }
        }
        DpsAbilityKind::Rend => {
            model.manually_castable = false;
            model.dot = Some(DotModel {
                power_coefficient: 0.0,
                damage_spread: 0.0,
                duration_ms: 30_000,
                period_ms: 3_000,
                can_crit: true,
                cinder_proc_chance: 0.0,
                cinders_on_proc: 0.0,
                stack_damage_increase: 0.0,
                maximum_stacks: 1,
            });
            model.mechanic_parameters = BTreeMap::from([
                ("featherActivationDelaySeconds".into(), 0.5),
                ("maximumGroundFeathers".into(), 5.0),
                ("featherChance1".into(), 0.0),
                ("featherChance2".into(), 0.0),
                ("featherChance3".into(), 0.0),
                ("featherChance4".into(), 0.0),
                ("featherChance5".into(), 0.0),
                ("featherChance6".into(), 0.0),
                ("featherChance7".into(), 0.0),
                ("spiritRefundDelaySeconds".into(), 0.2),
                ("spiritRefundSpiritGain".into(), 1.0),
                ("spiritRefundFeathers".into(), 5.0),
            ]);
        }
        DpsAbilityKind::GundeAttack => {
            model.manually_castable = false;
            model.off_gcd = true;
            model.gcd_ms = 0;
            model.mechanic_parameters.insert("swingDurationSeconds".into(), 1.5);
        }
        DpsAbilityKind::Exsanguinate => model.manually_castable = false,
        DpsAbilityKind::Bloodcraze => {
            model.manually_castable = false;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model.mechanic_parameters = BTreeMap::from([
                ("damageMultiplierPerAdditionalTarget".into(), 1.5),
                ("targetCountDamageScalingThreshold".into(), 8.0),
            ]);
        }
        DpsAbilityKind::RavensPrecision | DpsAbilityKind::Oathshatter => {
            model.manually_castable = false;
            model.max_targets = MAX_STATIONARY_DUMMY_TARGETS;
            model
                .mechanic_parameters
                .insert("targetCountDamageScalingThreshold".into(), 8.0);
        }
        _ => {}
    }
    model
}

fn gunde_profile() -> NormalizedDpsProfile {
    let mut profile = profile(
        [
            DpsAbilityKind::DoubleStrike,
            DpsAbilityKind::Warbound,
            DpsAbilityKind::OwedInBlood,
            DpsAbilityKind::BloodboundSpirit,
            DpsAbilityKind::ReignInBlood,
            DpsAbilityKind::HeartSplitter,
            DpsAbilityKind::Rupture,
            DpsAbilityKind::Slaughter,
            DpsAbilityKind::BloodArc,
            DpsAbilityKind::ReaversEdge,
            DpsAbilityKind::GundeAttack,
            DpsAbilityKind::ButchersHook,
            DpsAbilityKind::GrimCarve,
            DpsAbilityKind::Rend,
            DpsAbilityKind::Exsanguinate,
            DpsAbilityKind::Bloodcraze,
            DpsAbilityKind::RavensPrecision,
            DpsAbilityKind::Oathshatter,
        ]
        .into_iter()
        .map(gunde_ability)
        .collect(),
    );
    profile.hero_id = GUNDE_HERO_ID.into();
    profile.max_primary_resource = 100.0;
    profile.max_secondary_resource = 5;
    profile
}

fn gunde_legendary(
    trait_number: u8,
    additions: impl IntoIterator<Item = (&'static str, f64)>,
) -> DynamicMechanicInstance {
    let mut parameters = BTreeMap::from([
        ("powerMultiplier".into(), 1.0),
        ("cooldownAccelerationMultiplier".into(), 1.0),
    ]);
    parameters.extend(
        additions
            .into_iter()
            .map(|(key, value)| (key.to_string(), value)),
    );
    DynamicMechanicInstance {
        instance_id: format!("test:gunde-trait{trait_number}"),
        source_id: format!("legendary-gunde-trait{trait_number}"),
        source_name: format!("Gunde trait {trait_number}"),
        mechanic_id: format!("augmentation:test-gunde-trait{trait_number}"),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters,
        reason: None,
    }
}

fn gunde_talent(
    number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DpsTalentModel {
    DpsTalentModel {
        id: format!("gunde-talent-id-talent{number}"),
        name: format!("Gunde talent {number}"),
        mechanic_id: format!("augmentation:test-gunde-talent{number}"),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
        reason: None,
    }
}

fn rime_talent(
    number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DpsTalentModel {
    let mechanic_id = match number {
        1 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-glacialassault"
        }
        2 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent17"
        }
        3 => "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-chillblain",
        4 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-unrelentingice"
        }
        5 => "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-icyflow",
        6 => "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-avalanche",
        7 => "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-talent",
        8 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-wisdomofthenorth"
        }
        9 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent19"
        }
        10 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent12"
        }
        11 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent22"
        }
        12 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent20"
        }
        13 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent16"
        }
        14 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent13"
        }
        15 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent21"
        }
        16 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-passivetalent7"
        }
        17 => "augmentation:fellowship-content-abilities-talents-rime-caa-rime-trait1",
        18 => {
            "augmentation:fellowship-content-abilities-talents-rime-caa-talent-rime-chanceonorbgain-nextspendercritincrease"
        }
        _ => panic!("test Rime talent number must be current"),
    };
    DpsTalentModel {
        id: format!("rime-talent-id-talent{number}"),
        name: format!("Rime talent {number}"),
        mechanic_id: mechanic_id.into(),
        classification: MechanicClassification::Modeled,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn rime_legendary(
    trait_number: u8,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DynamicMechanicInstance {
    DynamicMechanicInstance {
        instance_id: format!("test:rime-trait{trait_number}"),
        source_id: format!("legendary-rime-trait{trait_number}"),
        source_name: format!("Rime trait {trait_number}"),
        mechanic_id: format!("augmentation:test-rime-trait{trait_number}"),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn mara_legendary(
    trait_number: u8,
    additions: impl IntoIterator<Item = (&'static str, f64)>,
) -> DynamicMechanicInstance {
    let mut parameters = BTreeMap::from([
        ("powerMultiplier".into(), 1.0),
        ("cooldownAccelerationMultiplier".into(), 1.0),
    ]);
    parameters.extend(
        additions
            .into_iter()
            .map(|(key, value)| (key.to_string(), value)),
    );
    DynamicMechanicInstance {
        instance_id: format!("test:mara-trait{trait_number}"),
        source_id: format!("legendary-mara-trait{trait_number}"),
        source_name: format!("Mara trait {trait_number}"),
        mechanic_id: format!("augmentation:test-mara-trait{trait_number}"),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters,
        reason: None,
    }
}

fn ardeos_mechanic(
    source_id: &str,
    parameters: impl IntoIterator<Item = (&'static str, f64)>,
) -> DynamicMechanicInstance {
    DynamicMechanicInstance {
        instance_id: source_id.into(),
        source_id: source_id.into(),
        source_name: source_id.into(),
        mechanic_id: format!("test:{source_id}"),
        classification: MechanicClassification::Modeled,
        handler: DynamicMechanicHandler::HeroSource,
        ability_kind: None,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        reason: None,
    }
}

fn apl(
    actions: impl IntoIterator<Item = (&'static str, Option<AplExpressionNode>)>,
) -> ActionPriorityListV2 {
    ActionPriorityListV2 {
        schema_version: ACTION_PRIORITY_LIST_SCHEMA_VERSION,
        rules: actions
            .into_iter()
            .enumerate()
            .map(|(index, (ability_id, condition))| AplRule {
                id: format!("test-rule-{index}"),
                enabled: true,
                ability_id: ability_id.into(),
                leading_comments: Vec::new(),
                inline_comment: None,
                condition,
            })
            .collect(),
        trailing_comments: Vec::new(),
    }
}

fn number(value: f64) -> AplNumericOperand {
    AplNumericOperand::Number { value }
}

fn reference(reference: AplNumericReference) -> AplNumericOperand {
    AplNumericOperand::Reference { reference }
}

fn expression(id: &str, expression: AplExpression) -> AplExpressionNode {
    AplExpressionNode {
        id: id.into(),
        expression,
    }
}

fn boolean_reference(id: &str, reference: AplBooleanReference) -> AplExpressionNode {
    expression(id, AplExpression::BooleanReference { reference })
}

fn comparison(
    id: &str,
    left: AplNumericOperand,
    operator: AplComparisonOperator,
    right: AplNumericOperand,
) -> AplExpressionNode {
    expression(
        id,
        AplExpression::Comparison {
            left,
            operator,
            right,
        },
    )
}

fn request(
    profile: NormalizedDpsProfile,
    target_count: u32,
    status: DpsEvidenceClaimStatus,
) -> SimulationRequest {
    let scope = if target_count == 1 {
        DpsEvidenceScope::ArdeosSingleDummy
    } else {
        DpsEvidenceScope::ArdeosStackedDummies
    };
    SimulationRequest {
        schema_version: SIMULATOR_SCHEMA_VERSION,
        run_id: "test-run".into(),
        data_build_id: "test-build".into(),
        model_version: ARDEOS_MODEL_VERSION.into(),
        profile_fingerprint: "test-profile".into(),
        hero_id: ARDEOS_HERO_ID.into(),
        scenario_id: ARDEOS_SCENARIO_ID.into(),
        scenario: StationaryDummyScenarioV3 {
            schema_version: STATIONARY_DUMMY_SCENARIO_SCHEMA_VERSION,
            target_count,
            target_distance_units: ARDEOS_MAX_COMBAT_RANGE_UNITS,
        },
        evidence: DpsEvidenceSnapshotV1 {
            schema_version: DPS_EVIDENCE_SNAPSHOT_SCHEMA_VERSION,
            build_id: "test-build".into(),
            model_revision: "test-evidence-r1".into(),
            evidence_fingerprint: "test-evidence-fingerprint".into(),
            scope,
            claims: vec![DpsEvidenceClaimSnapshot {
                id: "test:claim".into(),
                source_id: "test:source".into(),
                source_name: "Test source".into(),
                status,
            }],
        },
        iterations: MIN_SIMULATION_ITERATIONS,
        seed: "0000000000000001".into(),
        profile,
        action_priority_list: apl([("infernal-wave", None)]),
    }
}

fn rime_request(profile: NormalizedDpsProfile) -> SimulationRequest {
    let mut request = request(profile, 1, DpsEvidenceClaimStatus::Verified);
    request.model_version = RIME_MODEL_VERSION.into();
    request.hero_id = RIME_HERO_ID.into();
    request.scenario_id = RIME_SCENARIO_ID.into();
    request.scenario.target_distance_units = RIME_MAX_COMBAT_RANGE_UNITS;
    request.evidence.scope = DpsEvidenceScope::RimeSingleDummy;
    request.action_priority_list = apl([("frost-bolt", None)]);
    request
}

fn tariq_request(profile: NormalizedDpsProfile) -> SimulationRequest {
    let mut request = request(profile, 1, DpsEvidenceClaimStatus::Approximate);
    request.model_version = TARIQ_MODEL_VERSION.into();
    request.hero_id = TARIQ_HERO_ID.into();
    request.scenario_id = TARIQ_SCENARIO_ID.into();
    request.scenario.target_distance_units = TARIQ_MAX_COMBAT_RANGE_UNITS;
    request.evidence.scope = DpsEvidenceScope::TariqSingleDummy;
    request.action_priority_list = apl([("wild-swing", None)]);
    request
}

fn mara_request(profile: NormalizedDpsProfile) -> SimulationRequest {
    let mut request = request(profile, 1, DpsEvidenceClaimStatus::Approximate);
    request.model_version = MARA_MODEL_VERSION.into();
    request.hero_id = MARA_HERO_ID.into();
    request.scenario_id = MARA_SCENARIO_ID.into();
    request.scenario.target_distance_units = MARA_MAX_COMBAT_RANGE_UNITS;
    request.evidence.scope = DpsEvidenceScope::MaraSingleDummy;
    request.action_priority_list = apl([("mara-attack", None)]);
    request
}

fn gunde_request(profile: NormalizedDpsProfile) -> SimulationRequest {
    let mut request = request(profile, 1, DpsEvidenceClaimStatus::Approximate);
    request.model_version = GUNDE_MODEL_VERSION.into();
    request.hero_id = GUNDE_HERO_ID.into();
    request.scenario_id = GUNDE_SCENARIO_ID.into();
    request.scenario.target_distance_units = GUNDE_MAX_COMBAT_RANGE_UNITS;
    request.evidence.scope = DpsEvidenceScope::GundeSingleDummy;
    request.action_priority_list = apl([("double-strike", None)]);
    request
}

fn fight_window(start_seconds: f64, end_seconds: f64) -> AplExpressionNode {
    AplExpressionNode {
        id: format!("fight-window-{start_seconds}-{end_seconds}"),
        expression: AplExpression::All {
            children: vec![
                AplExpressionNode {
                    id: format!("fight-window-start-{start_seconds}"),
                    expression: AplExpression::Comparison {
                        left: reference(AplNumericReference::FightElapsed),
                        operator: AplComparisonOperator::GreaterThanOrEqual,
                        right: number(start_seconds),
                    },
                },
                AplExpressionNode {
                    id: format!("fight-window-end-{end_seconds}"),
                    expression: AplExpression::Comparison {
                        left: reference(AplNumericReference::FightElapsed),
                        operator: AplComparisonOperator::LessThan,
                        right: number(end_seconds),
                    },
                },
            ],
        },
    }
}

fn fixture_apl(ids: &[&str], window_seconds: f64) -> ActionPriorityListV2 {
    ActionPriorityListV2 {
        schema_version: ACTION_PRIORITY_LIST_SCHEMA_VERSION,
        rules: ids
            .iter()
            .enumerate()
            .map(|(index, id)| AplRule {
                id: format!("fixture-rule-{index}"),
                enabled: true,
                ability_id: (*id).to_owned(),
                leading_comments: Vec::new(),
                inline_comment: None,
                condition: Some(fight_window(
                    index as f64 * window_seconds,
                    (index + 1) as f64 * window_seconds,
                )),
            })
            .collect(),
        trailing_comments: Vec::new(),
    }
}

fn ardeos_fixture_request() -> SimulationRequest {
    let mut apocalypse = ability(DpsAbilityKind::Apocalypse, 1.4);
    apocalypse.max_targets = 3;
    apocalypse.cooldown_ms = 30_000;
    apocalypse.applies_dot_kind = Some(DpsAbilityKind::SearingBlaze);
    apocalypse.dot_application_delay_ms = 300;
    apocalypse.mechanic_parameters = BTreeMap::from([
        ("targetCountDamageScalingThreshold".into(), 3.0),
        ("resourceProcChance".into(), 0.25),
        ("cindersOnResourceProc".into(), 20.0),
    ]);
    let mut incinerate = ability(DpsAbilityKind::Incinerate, 0.35);
    incinerate.max_targets = 3;
    incinerate.spirit_cost = 20.0;
    incinerate.channel = Some(ChannelModel {
        duration_ms: 2_500,
        tick_interval_ms: 500,
        tick_immediately: true,
        scale_duration_with_ability_time_rate: false,
        enable_partial_ticks: true,
        scale_partial_tick_damage: false,
    });
    incinerate.mechanic_parameters =
        BTreeMap::from([("targetCountDamageScalingThreshold".into(), 3.0)]);
    let mut fire_ball = dot_ability(DpsAbilityKind::FireBall, 0.2);
    fire_ball.power_coefficient = 1.1;
    fire_ball.max_targets = 3;
    fire_ball.cooldown_ms = 12_000;
    fire_ball.maximum_charges = 2;
    fire_ball.mechanic_parameters = BTreeMap::from([
        ("targetCountDamageScalingThreshold".into(), 3.0),
        ("damageToDotTransferFraction".into(), 0.4),
        ("resourceProcChance".into(), 0.2),
        ("cindersOnResourceProc".into(), 20.0),
    ]);
    let mut wildfire = ability(DpsAbilityKind::Wildfire, 0.0);
    wildfire.effect_duration_ms = 10_000;
    wildfire.cooldown_ms = 20_000;
    wildfire.off_gcd = true;
    wildfire
        .mechanic_parameters
        .insert("tickRateMultiplier".into(), 2.0);
    let mut pyromania = ability(DpsAbilityKind::Pyromania, 0.0);
    pyromania.max_targets = 3;
    pyromania.applies_dot_kind = Some(DpsAbilityKind::EngulfingFlames);
    pyromania.dot_application_delay_ms = 250;
    let mut frogs = dot_ability(DpsAbilityKind::FireFrogs, 0.1);
    frogs.power_coefficient = 0.55;
    frogs.cooldown_ms = 15_000;
    let mut infernal_wave = ability(DpsAbilityKind::InfernalWave, 0.9);
    infernal_wave.primary_resource_generated = 35.0;
    infernal_wave.first_hit_delay_ms = 1_000;
    let mut profile = profile(vec![
        infernal_wave,
        ability(DpsAbilityKind::Detonate, 0.8),
        apocalypse,
        dot_ability(DpsAbilityKind::SearingBlaze, 0.18),
        dot_ability(DpsAbilityKind::EngulfingFlames, 0.3),
        incinerate,
        frogs,
        wildfire,
        pyromania,
        fire_ball,
    ]);
    profile.critical_strike = 0.2;
    profile.expertise = 0.1;
    profile.haste = 0.15;
    profile.spirit = 0.1;
    profile.scenario_no_op_abilities = REQUIRED_SCENARIO_NO_OP_ABILITIES
        .into_iter()
        .map(|id| ScenarioNoOpAbility {
            id: id.to_owned(),
            name: id.to_owned(),
            reason: "No damage effect in the stationary fixture scenario.".into(),
        })
        .collect();
    profile.talents.extend([
            DpsTalentModel {
                id: "firemage-talent-id-talent3".into(),
                name: "Agonizing Blaze".into(),
                mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-agonizingblaze".into(),
                classification: MechanicClassification::Modeled,
                parameters: BTreeMap::from([
                    ("damagePerStack".into(), 0.04),
                    ("maximumStacks".into(), 10.0),
                ]),
                reason: None,
            },
            DpsTalentModel {
                id: "firemage-talent-id-talent15".into(),
                name: "Burning Initiative".into(),
                mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-firemage-trait2".into(),
                classification: MechanicClassification::Modeled,
                parameters: BTreeMap::from([
                    ("startingSpirit".into(), 80.0),
                    ("startingEmbers".into(), 2.0),
                ]),
                reason: None,
            },
            DpsTalentModel {
                id: "firemage-talent-id-talent17".into(),
                name: "Bursting Flames".into(),
                mechanic_id: "augmentation:fellowship-content-abilities-talents-firemage-caa-talent-firemage-burstingflames".into(),
                classification: MechanicClassification::Modeled,
                parameters: BTreeMap::from([
                    ("damageFraction".into(), 0.35),
                    ("maximumRadius".into(), 1_500.0),
                    ("targetCountDamageScalingThreshold".into(), 3.0),
                    ("visualDelaySeconds".into(), 0.3),
                ]),
                reason: None,
            },
        ]);
    let mut request = request(profile, 3, DpsEvidenceClaimStatus::Verified);
    request.run_id = "fixture-ardeos".into();
    request.seed = "0123456789abcdef".into();
    request.action_priority_list = fixture_apl(
        &[
            "wildfire",
            "pyromania",
            "apocalypse",
            "fire-frogs",
            "fire-ball",
            "engulfing-flames",
            "detonate",
            "incinerate",
            "searing-blaze",
            "infernal-wave",
        ],
        20.0,
    );
    request
}

fn rime_fixture_request() -> SimulationRequest {
    let mut profile = rime_profile();
    profile.critical_strike = 0.18;
    profile.expertise = 0.12;
    profile.haste = 0.16;
    profile.spirit = 0.08;
    profile.talents.extend([
        rime_talent(
            1,
            [
                ("maximumStacks", 3.0),
                ("damageMultiplier", 1.25),
                ("explosionDamageFraction", 0.3),
                ("explosionRadius", 1_000.0),
                ("targetCountDamageScalingThreshold", 3.0),
            ],
        ),
        rime_talent(
            3,
            [
                ("durationSeconds", 3.0),
                ("powerCoefficientPerStack", 0.56),
                ("targetCountDamageScalingThreshold", 3.0),
                ("criticalExtraStackChance", 0.25),
                ("criticalExtraStacks", 1.0),
            ("maximumStacks", 99.0),
            ],
        ),
        rime_talent(
            9,
            [
                ("procsPerMinute", 2.0),
                ("durationSeconds", 8.0),
                ("tickRateMultiplier", 1.5),
                ("criticalStrikeBonus", 0.2),
            ],
        ),
        rime_talent(
            17,
            [
                ("singleTargetDamageMultiplier", 1.2),
                ("multiTargetDamageMultiplier", 0.8),
                ("singleTargetPulsePeriodSeconds", 1.0),
                ("multiTargetPulsePeriodSeconds", 1.0),
                ("singleTargetInitialDelaySeconds", 0.2),
                ("multiTargetInitialDelaySeconds", 0.4),
            ],
        ),
    ]);
    profile
        .uptime_names
        .insert("buff:glacial-assault".into(), "Glacial Assault".into());
    profile
        .uptime_names
        .insert("buff:soulfrost-torrent".into(), "Soulfrost Torrent".into());
    profile.mechanics.push(rime_legendary(
        1,
        [
            ("powerMultiplier", 1.1),
            ("cooldownAccelerationMultiplier", 1.15),
            ("startingSpirit", 100.0),
            ("wintersBlessingCharges", 2.0),
            ("undulatingSpiritDurationSeconds", 12.0),
        ],
    ));
    let mut request = rime_request(profile);
    request.run_id = "fixture-rime".into();
    request.seed = "fedcba9876543210".into();
    request.scenario.target_count = 3;
    request.evidence.scope = DpsEvidenceScope::RimeStackedDummies;
    request.action_priority_list = fixture_apl(
        &[
            "frost-bolt",
            "glacial-blast",
            "ice-comet",
            "bursting-ice",
            "freezing-torrent",
            "flight-of-the-navir",
            "cold-snap",
            "ice-blitz",
            "winters-blessing",
            "wrath-of-winter",
        ],
        20.0,
    );
    request
}

fn assert_full_result_fixture(name: &str, request: SimulationRequest, expected: &str) {
    let actual =
        simulate(&request, &AtomicBool::new(false), |_| {}).expect("fixture request simulates");
    let actual = serde_json::to_value(actual).expect("fixture result serializes");
    let actual = serde_json::to_string_pretty(&actual).expect("fixture result formats");
    assert_eq!(actual, expected.trim(), "{name} full result changed");
}
