//! Exploratory inputs using the same preparation rules as equipment.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub const SYNTHETIC_BUILD_SCHEMA_VERSION: u32 = 1;
const MAX_INPUT_STAT: f64 = crate::validation::MAX_DYNAMIC_PARAMETER_ABS;

/// Raw ratings before selected effects. These are not percentages or final attributes.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecondaryRatings {
    pub crit: f64,
    pub haste: f64,
    pub expertise: f64,
    pub spirit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticEffectSelection {
    pub id: String,
    /// Source identity only: this does not occupy an equipment slot.
    #[serde(default)]
    pub source_position_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticSetBonus {
    pub piece_count: u32,
    #[serde(default)]
    pub source_position_id: Option<String>,
}

/// An exploratory build, not a claim that the selections are obtainable equipment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticBuild {
    pub schema_version: u32,
    pub hero_id: String,
    pub talent_points: u32,
    pub selected_talent_ids: Vec<String>,
    pub secondary_ratings: SecondaryRatings,
    #[serde(default)]
    pub primary_stat_add: f64,
    #[serde(default)]
    pub stamina_add: f64,
    #[serde(default)]
    pub legendary: Option<SyntheticEffectSelection>,
    #[serde(default)]
    pub weapon: Option<SyntheticEffectSelection>,
    #[serde(default)]
    pub traits: BTreeMap<String, u32>,
    #[serde(default)]
    pub blessings: BTreeMap<String, u32>,
    /// Power before set multipliers, keyed by catalog family (for example Ruby).
    #[serde(default)]
    pub gem_power: BTreeMap<String, f64>,
    /// Each catalog bonus has its own activation threshold.
    #[serde(default)]
    pub set_bonuses: BTreeMap<String, SyntheticSetBonus>,
    /// Reviewed relic effects, currently no-ops in the stationary dummy encounter.
    #[serde(default)]
    pub relics: Vec<SyntheticEffectSelection>,
    /// Equipment/gem source identities retained by conversion for evidence reporting.
    /// They grant no stats or effects.
    #[serde(default)]
    pub evidence_source_ids: BTreeSet<String>,
}

impl SyntheticBuild {
    pub fn new(hero_id: &str) -> Result<Self, SimulationError> {
        if HeroIdentity::from_id(hero_id).is_none() {
            return Err(invalid("Unsupported hero"));
        }
        Ok(Self {
            schema_version: SYNTHETIC_BUILD_SCHEMA_VERSION,
            hero_id: hero_id.into(),
            talent_points: 14,
            selected_talent_ids: vec![],
            secondary_ratings: SecondaryRatings::default(),
            primary_stat_add: 0.0,
            stamina_add: 0.0,
            legendary: None,
            weapon: None,
            traits: BTreeMap::new(),
            blessings: BTreeMap::new(),
            gem_power: BTreeMap::new(),
            set_bonuses: BTreeMap::new(),
            relics: vec![],
            evidence_source_ids: BTreeSet::new(),
        })
    }

    pub fn with_secondary_ratings(
        &self,
        ratings: SecondaryRatings,
    ) -> Result<Self, SimulationError> {
        let mut build = self.clone();
        build.secondary_ratings = ratings;
        validate_synthetic(&build)?;
        Ok(build)
    }

    pub fn with_rating_delta(&self, delta: SecondaryRatings) -> Result<Self, SimulationError> {
        let ratings = self.secondary_ratings;
        self.with_secondary_ratings(SecondaryRatings {
            crit: ratings.crit + delta.crit,
            haste: ratings.haste + delta.haste,
            expertise: ratings.expertise + delta.expertise,
            spirit: ratings.spirit + delta.spirit,
        })
    }

    /// Stable JSON independent of map insertion order and talent selection order.
    pub fn canonical_json(&self) -> Result<String, SimulationError> {
        validate_synthetic(self)?;
        let mut build = self.clone();
        build.selected_talent_ids.sort();
        build
            .relics
            .sort_by(|a, b| (&a.id, &a.source_position_id).cmp(&(&b.id, &b.source_position_id)));
        serde_json::to_string(&build).map_err(|e| invalid(e.to_string()))
    }
}

fn diagnostic(code: &str, path: &str, message: impl Into<String>) -> Diagnostic {
    let mut d = Diagnostic::error(code, message).at(path);
    d.engine_code = Some(SimulationErrorCode::InvalidBuild);
    d
}

pub fn diagnose_synthetic(build: &SyntheticBuild) -> Vec<Diagnostic> {
    let data = fellersim_data::catalog();
    let hero = &data["heroes"][&build.hero_id];
    let mut issues = character::diagnose_talents(
        &build.hero_id,
        build.talent_points,
        &build.selected_talent_ids,
    );
    if hero.is_null() {
        return issues;
    }
    if build.schema_version != SYNTHETIC_BUILD_SCHEMA_VERSION {
        issues.push(diagnostic(
            "unsupported-synthetic-schema",
            "/schemaVersion",
            "Expected synthetic schemaVersion 1.",
        ));
    }
    let r = build.secondary_ratings;
    for (path, value) in [
        ("/secondaryRatings/crit", r.crit),
        ("/secondaryRatings/haste", r.haste),
        ("/secondaryRatings/expertise", r.expertise),
        ("/secondaryRatings/spirit", r.spirit),
        ("/primaryStatAdd", build.primary_stat_add),
        ("/staminaAdd", build.stamina_add),
    ] {
        if !value.is_finite() || !(0.0..=MAX_INPUT_STAT).contains(&value) {
            issues.push(diagnostic(
                "invalid-stat",
                path,
                format!("Expected a finite value between 0 and {MAX_INPUT_STAT}."),
            ));
        }
    }
    for (category, ranks) in [("traits", &build.traits), ("blessings", &build.blessings)] {
        for (id, rank) in ranks {
            let def = &data[category][id];
            if def.is_null()
                || *rank == 0
                || f64::from(*rank) > num(def, "maxRank")
                || def["models"][rank.to_string()].is_null()
            {
                issues.push(diagnostic(
                    "invalid-effect-rank",
                    &format!("/{category}/{id}"),
                    format!("Unknown {category} ID or unsupported rank: {id} rank {rank}."),
                ));
            }
        }
    }
    for (family, power) in &build.gem_power {
        if !arr(&data["gems"])
            .iter()
            .any(|g| string(g, "family") == family)
            || !power.is_finite()
            || !(0.0..=MAX_INPUT_STAT).contains(power)
        {
            issues.push(diagnostic(
                "invalid-gem-power",
                &format!("/gemPower/{family}"),
                "Expected a known gem family and finite, nonnegative power at most 1000000.",
            ));
        }
    }
    let valid_position = |position: &Option<String>| {
        position.as_ref().is_none_or(|id| {
            arr(&data["positions"])
                .iter()
                .any(|p| string(p, "id") == id)
        })
    };
    let eligible_sets: BTreeSet<&str> = hero["items"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|i| arr(&i["setBonusIds"]).iter().filter_map(Value::as_str))
        .collect();
    for (id, selection) in &build.set_bonuses {
        // Quickplay is a universal encounter bonus, independent of equipment identity.
        if data["sets"][id].is_null()
            || (!eligible_sets.contains(id.as_str()) && id != "quickplay")
            || !valid_position(&selection.source_position_id)
        {
            issues.push(diagnostic(
                "invalid-set-bonus",
                &format!("/setBonuses/{id}"),
                "Unknown or hero-incompatible set bonus, or invalid source position.",
            ));
        }
    }
    let mut seen = BTreeSet::new();
    for (path, effect, category) in build
        .legendary
        .iter()
        .map(|e| ("/legendary".to_owned(), e, "legendary"))
        .chain(
            build
                .weapon
                .iter()
                .map(|e| ("/weapon".to_owned(), e, "weapon")),
        )
        .chain(
            build
                .relics
                .iter()
                .enumerate()
                .map(|(i, e)| (format!("/relics/{i}"), e, "relic")),
        )
    {
        let def = &hero["items"][&effect.id];
        let kind_matches = match category {
            "legendary" => effect.id.starts_with("legendary-"),
            "weapon" => !def["weapon"].is_null(),
            _ => effect.id.starts_with("relic-"),
        };
        let compatible_position = effect.source_position_id.as_ref().is_none_or(|id| {
            arr(&data["positions"])
                .iter()
                .any(|p| string(p, "id") == id && p["itemType"] == def["itemType"])
        });
        let position = effect.source_position_id.as_deref().unwrap_or_else(|| {
            arr(&data["positions"])
                .iter()
                .find(|p| p["itemType"] == def["itemType"])
                .map_or("", |p| string(p, "id"))
        });
        if def.is_null()
            || !kind_matches
            || !compatible_position
            || !seen.insert((&effect.id, position))
        {
            issues.push(diagnostic(
                "invalid-effect",
                &path,
                format!(
                    "Unknown, duplicate, or incompatible {category} effect: {}.",
                    effect.id
                ),
            ));
        }
    }
    if build.relics.len() > 16 || build.evidence_source_ids.len() > 256 {
        issues.push(diagnostic(
            "synthetic-size",
            "",
            "Too many source identities.",
        ));
    }
    for id in &build.evidence_source_ids {
        if hero["items"][id].is_null()
            && !arr(&data["socketGems"])
                .iter()
                .any(|g| string(g, "id") == id)
        {
            issues.push(diagnostic(
                "invalid-evidence-source",
                "/evidenceSourceIds",
                format!("Unknown source identity: {id}."),
            ));
        }
    }
    issues
}

pub fn validate_synthetic(build: &SyntheticBuild) -> Result<(), SimulationError> {
    let issues = diagnose_synthetic(build);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(SimulationError::from_diagnostics(
            SimulationErrorCode::InvalidBuild,
            issues,
        ))
    }
}

fn collect(build: &SyntheticBuild) -> stats::Contributions {
    let data = fellersim_data::catalog();
    let hero = &data["heroes"][&build.hero_id];
    let mut c = stats::Contributions::default();
    let r = build.secondary_ratings;
    c.gear = [
        ("CritChance", r.crit),
        ("Haste", r.haste),
        ("Expertise", r.expertise),
        ("Spirit", r.spirit),
        (string(hero, "primaryStat"), build.primary_stat_add),
        ("Stamina", build.stamina_add),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    c.trait_ranks = build.traits.clone();
    c.blessing_ranks = build.blessings.clone();
    c.gems = build.gem_power.clone();
    c.sets = build
        .set_bonuses
        .iter()
        .map(|(id, s)| {
            (
                id.clone(),
                (
                    s.piece_count,
                    s.source_position_id
                        .clone()
                        .unwrap_or_else(|| string(&data["positions"][0], "id").into()),
                ),
            )
        })
        .collect();
    c.sources = build.evidence_source_ids.clone();
    for effect in build
        .legendary
        .iter()
        .chain(build.weapon.iter())
        .chain(build.relics.iter())
    {
        let def = &hero["items"][&effect.id];
        let position = effect.source_position_id.clone().unwrap_or_else(|| {
            string(
                arr(&data["positions"])
                    .iter()
                    .find(|p| p["itemType"] == def["itemType"])
                    .unwrap(),
                "id",
            )
            .into()
        });
        c.item_effects.push((effect.id.clone(), position));
        c.sources.insert(effect.id.clone());
    }
    c.item_effects.sort_by_key(|(id, pos)| {
        (
            arr(&data["positions"])
                .iter()
                .position(|p| string(p, "id") == pos)
                .unwrap(),
            id.clone(),
        )
    });
    for id in &c.sources {
        for mechanic in arr(&hero["items"][id]["mappedMechanicIds"]) {
            c.mechanic_ids.insert(mechanic.as_str().unwrap().into());
        }
    }
    c
}

pub fn prepare_synthetic_build(
    build: &SyntheticBuild,
    targets: u32,
) -> Result<(NormalizedDpsProfile, DpsEvidenceSnapshotV1), SimulationError> {
    validate_synthetic(build)?;
    if !(1..=MAX_STATIONARY_DUMMY_TARGETS).contains(&targets) {
        return Err(invalid("Targets must be 1–20"));
    }
    stats::resolve(
        &build.hero_id,
        &build.selected_talent_ids,
        collect(build),
        targets,
    )
}

/// Export baseline contributions, not already-buffed attributes (which would double apply effects).
pub fn synthetic_from_character(
    character: &CharacterBuild,
) -> Result<SyntheticBuild, SimulationError> {
    character::validate(character)?;
    let c = stats::collect(character);
    let mut build = SyntheticBuild::new(&character.hero_id)?;
    build.talent_points = character.talent_points;
    build.selected_talent_ids = character.selected_talent_ids.clone();
    let stat = |name: &str| *c.gear.get(name).unwrap_or(&0.0);
    build.secondary_ratings = SecondaryRatings {
        crit: stat("CritChance"),
        haste: stat("Haste"),
        expertise: stat("Expertise"),
        spirit: stat("Spirit"),
    };
    build.primary_stat_add = stat(string(
        &fellersim_data::catalog()["heroes"][&character.hero_id],
        "primaryStat",
    ));
    build.stamina_add = stat("Stamina");
    build.traits = c.trait_ranks;
    build.blessings = c
        .blessing_ranks
        .into_iter()
        .map(|(id, rank)| {
            let cap = num(&fellersim_data::catalog()["blessings"][&id], "maxRank") as u32;
            (id, rank.min(cap))
        })
        .collect();
    build.gem_power = c.gems;
    build.set_bonuses = c
        .sets
        .into_iter()
        .map(|(id, (piece_count, position))| {
            (
                id,
                SyntheticSetBonus {
                    piece_count,
                    source_position_id: Some(position),
                },
            )
        })
        .collect();
    build.evidence_source_ids = c.sources;
    for (id, position) in c.item_effects {
        let effect = SyntheticEffectSelection {
            id,
            source_position_id: Some(position),
        };
        if effect.id.starts_with("legendary-") {
            if build.legendary.replace(effect).is_some() {
                return Err(invalid("Synthetic builds permit one legendary effect."));
            }
        } else if effect.id.starts_with("weapon-") {
            build.weapon = Some(effect);
        } else if effect.id.starts_with("relic-") {
            build.relics.push(effect);
        } else {
            return Err(invalid(format!("Unsupported item effect: {}", effect.id)));
        }
    }
    validate_synthetic(&build)?;
    Ok(build)
}

/// A parsed rotation reusable across builds; mutable combat state is created per run.
pub struct EvaluationContext {
    apl: AplDocument,
}

impl EvaluationContext {
    pub fn new(apl_source: &str) -> Result<Self, SimulationError> {
        let _ = fellersim_data::catalog();
        Ok(Self {
            apl: parse_apl_document(apl_source)?,
        })
    }

    pub fn prepare(
        &self,
        build: &SyntheticBuild,
        options: &SimulationOptions,
        run_id: &str,
    ) -> Result<SimulationRequest, SimulationError> {
        let (profile, evidence) = prepare_synthetic_build(build, options.targets)?;
        assemble_request(
            &build.hero_id,
            profile,
            evidence,
            options,
            run_id,
            &self.apl,
            true,
        )
    }

    /// Complete evaluation identity; provenance and run labels do not affect combat.
    pub fn cache_key(
        &self,
        build: &SyntheticBuild,
        options: &SimulationOptions,
    ) -> Result<String, SimulationError> {
        let request = self.prepare(build, options, "cache")?;
        Ok(digest(
            &json!({"profileFingerprint":request.profile_fingerprint,"iterations":request.iterations,"seed":request.seed}),
        ))
    }

    pub fn evaluate(
        &self,
        build: &SyntheticBuild,
        options: &SimulationOptions,
        run_id: &str,
        cancelled: &AtomicBool,
    ) -> Result<SimulationResult, SimulationError> {
        simulate(&self.prepare(build, options, run_id)?, cancelled, |_| {})
    }

    /// Ordered, lazy results. Invalid entries do not discard other entries; cancellation stops iteration.
    pub fn evaluate_batch<'a>(
        &'a self,
        builds: impl IntoIterator<Item = &'a SyntheticBuild> + 'a,
        options: &'a SimulationOptions,
        cancelled: &'a AtomicBool,
    ) -> impl Iterator<Item = Result<SimulationResult, SimulationError>> + 'a {
        builds
            .into_iter()
            .enumerate()
            .take_while(move |_| !cancelled.load(Ordering::Relaxed))
            .map(move |(i, build)| {
                self.evaluate(build, options, &format!("synthetic-{i}"), cancelled)
            })
    }
}

pub fn prepare_synthetic(
    build: &SyntheticBuild,
    apl_source: &str,
    options: &SimulationOptions,
    run_id: &str,
) -> Result<SimulationRequest, SimulationError> {
    EvaluationContext::new(apl_source)?.prepare(build, options, run_id)
}
