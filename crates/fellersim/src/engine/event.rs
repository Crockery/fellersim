use std::cmp::Ordering;

use crate::*;

#[derive(Debug, Clone)]
pub(crate) enum EventKind {
    Core(CoreEvent),
    Shared(SharedEvent),
    Ardeos(ArdeosEvent),
    Rime(RimeEvent),
    Tariq(TariqEvent),
    Elarion(ElarionEvent),
    Mara(MaraEvent),
    Gunde(GundeEvent),
}

#[derive(Debug, Clone)]
pub(crate) enum CoreEvent {
    PreparedCastCommit {
        cast: Box<PreparedCast>,
    },
    ImpactBatch {
        index: usize,
        impacts: Vec<ImpactSpec>,
        context: CastImpactContext,
    },
    DotTick {
        kind: DotKind,
        generation: u64,
        target_index: u32,
    },
    StarfallHit {
        generation: u64,
        target_index: u32,
    },
    DotExpire {
        kind: DotKind,
        generation: u64,
        target_index: u32,
    },
    ApplyLinkedDot {
        index: usize,
        targets: Vec<u32>,
        context: DamageContext,
    },
    ApplyAbilityDot {
        index: usize,
        targets: Vec<u32>,
        context: DamageContext,
    },
}

impl From<CoreEvent> for EventKind {
    fn from(event: CoreEvent) -> Self {
        Self::Core(event)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SharedEvent {
    RubyStormHit {
        mechanic_index: usize,
        damage_bits: u64,
        snapshot: DamageSourceSnapshot,
    },
    WeaponConeStun {
        index: usize,
        targets: u32,
    },
    PeriodicHealingWake {
        generation: u64,
    },
    WeaponCooldownReduction {
        mechanic_index: usize,
        generation: u64,
        ability_kind: DpsAbilityKind,
    },
    WeaponShadowExplode {
        generation: u64,
        target_index: u32,
    },
    AurastoneDamagePulse {
        mechanic_index: usize,
        generation: u64,
    },
    AmethystSplintersTick {
        mechanic_index: usize,
        generation: u64,
        target_index: u32,
    },
    KindlingHealingTick {
        mechanic_index: usize,
        generation: u64,
    },
    KindlingTick {
        mechanic_index: usize,
        generation: u64,
        target_index: u32,
    },
    WayfarerProc {
        generation: u64,
    },
}

impl From<SharedEvent> for EventKind {
    fn from(event: SharedEvent) -> Self {
        Self::Shared(event)
    }
}

impl From<ArdeosEvent> for EventKind {
    fn from(event: ArdeosEvent) -> Self {
        Self::Ardeos(event)
    }
}

impl From<RimeEvent> for EventKind {
    fn from(event: RimeEvent) -> Self {
        Self::Rime(event)
    }
}

impl From<TariqEvent> for EventKind {
    fn from(event: TariqEvent) -> Self {
        Self::Tariq(event)
    }
}

impl From<ElarionEvent> for EventKind {
    fn from(event: ElarionEvent) -> Self {
        Self::Elarion(event)
    }
}

impl From<MaraEvent> for EventKind {
    fn from(event: MaraEvent) -> Self {
        Self::Mara(event)
    }
}

impl From<GundeEvent> for EventKind {
    fn from(event: GundeEvent) -> Self {
        Self::Gunde(event)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Event {
    pub(crate) at_ms: u64,
    pub(crate) sequence: u64,
    pub(crate) kind: EventKind,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.at_ms == other.at_ms && self.sequence == other.sequence
    }
}

impl Eq for Event {}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.at_ms
            .cmp(&other.at_ms)
            .then(self.sequence.cmp(&other.sequence))
    }
}
impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
