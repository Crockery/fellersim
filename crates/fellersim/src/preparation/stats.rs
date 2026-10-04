use super::*;

fn rating(stat: &str, value: f64) -> f64 {
    let properties = &fellersim_data::catalog()["itemStatModel"]["attributeProperties"];
    let Some(property) = arr(properties).iter().find(|p| string(p, "id") == stat) else {
        return 0.0;
    };
    let mut remaining = (value as f32).max(0.0).round_ties_even() as f64;
    let (mut previous, mut points, mut penalty) = (0.0, 0.0, 1.0);
    for bracket in arr(&property["brackets"]) {
        penalty *= num(bracket, "penaltyPercentage");
        let maximum = num(bracket, "maxBracketValue");
        let consumed =
            remaining.min((maximum - previous) / (num(property, "baseStatMultiplier") * penalty));
        points += consumed * num(property, "baseStatMultiplier") * penalty;
        remaining -= consumed;
        previous = maximum;
        if remaining <= 0.0 {
            break;
        }
    }
    if remaining > 0.0 {
        points += remaining
            * num(property, "baseStatMultiplier")
            * penalty
            * num(property, "finalPenaltyPercentage");
    }
    points / 100.0
}
fn ranked_value(values: &Value, rank: u32) -> Option<f64> {
    values
        .as_object()?
        .iter()
        .filter_map(|(k, v)| Some((k.parse::<u32>().ok()?, v.as_f64()?)))
        .filter(|(r, _)| *r <= rank)
        .max_by_key(|(r, _)| *r)
        .map(|(_, v)| v)
}
fn passive(
    definition: &Value,
    rank: u32,
    modifiers: &mut Vec<Value>,
    sources: &mut BTreeSet<String>,
    ids: &mut BTreeSet<String>,
) {
    for modifier in arr(&definition["modifiers"]) {
        if !["Expertise", "MaxHealth", "MagicDamageReduction"]
            .contains(&string(modifier, "attribute"))
        {
            continue;
        }
        if let Some(value) = ranked_value(&modifier["valuesByRank"], rank) {
            let mut entry = modifier.clone();
            entry["value"] = json!(value);
            entry["sourceId"] = definition["source"]["sourceId"].clone();
            sources.insert(string(&definition["source"], "sourceId").into());
            ids.insert(string(modifier, "mechanicId").into());
            modifiers.push(entry);
        }
    }
}
fn apply_passive(modifiers: &[Value], stat: &str, base: f64) -> f64 {
    let (mut add, mut mul) = (0.0, 1.0);
    for m in modifiers.iter().filter(|m| string(m, "attribute") == stat) {
        match string(m, "operation") {
            "add" => add += num(m, "value"),
            "multiply" => mul *= num(m, "value"),
            _ => {}
        }
    }
    (base + add) * mul
}
fn add_source(
    model: &Value,
    source: &Value,
    instance: String,
    mechanics: &mut Vec<Value>,
    sources: &mut BTreeSet<String>,
    ids: &mut BTreeSet<String>,
) {
    let mut model = model.clone();
    model["instanceId"] = json!(instance);
    sources.insert(string(source, "sourceId").into());
    for id in arr(&source["mechanicIds"]) {
        if let Some(id) = id.as_str() {
            ids.insert(id.into());
        }
    }
    mechanics.push(model);
}
fn target_id(value: &str) -> String {
    let mut result = String::new();
    let mut previous_lower = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && previous_lower {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !result.ends_with('-') {
                result.push('-');
            }
            previous_lower = false;
        }
    }
    result.trim_matches('-').into()
}

pub(super) fn prepare(
    build: &CharacterBuild,
    targets: u32,
) -> Result<(NormalizedDpsProfile, DpsEvidenceSnapshotV1), SimulationError> {
    let data = fellersim_data::catalog();
    let hero = &data["heroes"][&build.hero_id];
    let model = &data["itemStatModel"];
    let settings = &model["calculationSettings"];
    let current = &settings["currentItem"];
    let starting = &hero["startingStats"];
    let primary = string(hero, "primaryStat");
    let mut p = hero["base"].clone();
    let mut gear = BTreeMap::<String, f64>::new();
    let mut mechanics = Vec::new();
    let mut modifiers = Vec::new();
    let mut sources = BTreeSet::new();
    let mut mechanic_ids = BTreeSet::new();
    let mut sets = BTreeMap::<String, (u32, String)>::new();
    let mut gems = BTreeMap::<String, f64>::new();
    let mut trait_ranks = BTreeMap::<String, u32>::new();
    let mut blessing_ranks = BTreeMap::<String, u32>::new();
    for position in arr(&data["positions"]) {
        let pos = build
            .positions
            .iter()
            .find(|p| p.position_id == string(position, "id"))
            .unwrap();
        let Some(item) = &pos.item else {
            continue;
        };
        let definition = &hero["items"][&item.item_id];
        sources.insert(item.item_id.clone());
        for id in arr(&definition["mappedMechanicIds"]) {
            mechanic_ids.insert(id.as_str().unwrap().to_owned());
        }
        let level = item.item_level as f64;
        let primary_pool = num(settings, "primaryPowerBase")
            .powf(level / num(current, "itemLevelExponentDivisor"))
            * level.sqrt()
            * num(current, "primaryPoolSquareRootMultiplier");
        let secondary_pool = (num(current, "secondaryPoolLevelFactor") * level).sqrt()
            * num(current, "secondaryPoolMultiplier");
        let weight = arr(&model["slotWeights"])
            .iter()
            .find(|s| string(s, "itemType") == string(definition, "itemType"))
            .map_or(0.0, |s| num(s, "statWeight"));
        let weights = arr(&model["statWeightConfigurations"])
            .iter()
            .find(|w| w["id"] == definition["statWeightConfigurationId"])
            .unwrap();
        for (i, slot) in arr(&definition["fixedSlots"]).iter().enumerate() {
            let id = slot.as_str().unwrap();
            let stat = if id == "HeroRelativePrimaryStat" {
                primary
            } else {
                arr(&model["dynamicSlotDefinitions"])
                    .iter()
                    .find(|s| string(s, "id") == id)
                    .map_or("", |s| string(s, "stat"))
            };
            if stat.is_empty() {
                continue;
            }
            let factor = if id == "HeroRelativePrimaryStat" {
                num(current, "primaryWeight") * num(current, "primaryAndStaminaMultiplier")
            } else if stat == "Stamina" {
                num(current, "staminaWeight") * num(current, "primaryAndStaminaMultiplier")
            } else {
                num(current, "secondaryWeight") * num(current, "secondaryMultiplier")
            };
            let pool = if id == "HeroRelativePrimaryStat" || stat == "Stamina" {
                primary_pool
            } else {
                secondary_pool
            };
            let value = (pool * weight * weights["weights"][i].as_f64().unwrap_or(0.0) / 100.0
                * (factor / num(current, "statFactorDivisor")))
            .round()
                + item.applied_tempers as f64;
            *gear.entry(stat.into()).or_default() += value;
        }
        for m in &item.rolled_modifiers {
            if m.kind == "rolled-stat" {
                let stat = if m.choice_id == "HeroRelativePrimaryStat" {
                    primary
                } else {
                    &m.choice_id
                };
                let pool = if ["Strength", "Agility", "Intellect", "Stamina"].contains(&stat) {
                    primary_pool
                } else {
                    secondary_pool
                };
                *gear.entry(stat.into()).or_default() +=
                    (pool * weight * num(current, "bonusStatCoefficient")).round();
            } else if m.kind == "gem-power"
                && let Some(g) = arr(&data["socketGems"])
                    .iter()
                    .find(|g| string(g, "familyId") == m.choice_id)
            {
                *gems.entry(string(g, "family").into()).or_default() += 100.0;
            }
        }
        for g in &item.gems {
            let gem = arr(&data["socketGems"])
                .iter()
                .find(|v| string(v, "id") == g.gem_id)
                .unwrap();
            *gems.entry(string(gem, "family").into()).or_default() += num(gem, "power");
            sources.insert(g.gem_id.clone());
        }
        for (id, rank) in character::ranks(item) {
            *trait_ranks.entry(id).or_default() += rank;
        }
        for blessing in &item.blessings {
            *blessing_ranks
                .entry(blessing.blessing_id.clone())
                .or_default() += blessing.rank;
        }
        for entry in arr(&definition["sources"]) {
            if string(&entry["source"], "category") == "set-bonuses" {
                continue;
            }
            let instance = string(&entry["model"], "instanceId").replace(
                &format!(":{}:", string(&entry["source"], "positionId")),
                &format!(":{}:", pos.position_id),
            );
            add_source(
                &entry["model"],
                &entry["source"],
                instance,
                &mut mechanics,
                &mut sources,
                &mut mechanic_ids,
            );
        }
        for id in arr(&definition["setBonusIds"]) {
            let entry = sets
                .entry(id.as_str().unwrap().into())
                .or_insert((0, pos.position_id.clone()));
            entry.0 += 1;
        }
        if !definition["weapon"].is_null() {
            p["abilities"]
                .as_array_mut()
                .unwrap()
                .push(definition["weapon"].clone());
        }
    }
    for (category, ranks) in [("traits", trait_ranks), ("blessings", blessing_ranks)] {
        for (id, total_rank) in ranks {
            let definition = &data[category][&id];
            let rank = total_rank.min(num(definition, "maxRank") as u32);
            sources.insert(id.clone());
            if category == "traits" {
                passive(
                    definition,
                    rank,
                    &mut modifiers,
                    &mut sources,
                    &mut mechanic_ids,
                );
            }
            if definition["fullyModeled"] != true
                && !arr(&definition["source"]["mechanicIds"]).is_empty()
            {
                add_source(
                    &definition["models"][rank.to_string()],
                    &definition["source"],
                    format!("unsupported:{category}:equipment:{id}"),
                    &mut mechanics,
                    &mut sources,
                    &mut mechanic_ids,
                );
            }
        }
    }
    for (id, (count, pos)) in &sets {
        let definition = &data["sets"][id];
        if (*count as f64) < number(definition, "requiredSetCount", f64::INFINITY) {
            continue;
        }
        sources.insert(id.clone());
        passive(
            definition,
            1,
            &mut modifiers,
            &mut sources,
            &mut mechanic_ids,
        );
        if definition["fullyModeled"] != true
            && !arr(&definition["source"]["mechanicIds"]).is_empty()
        {
            add_source(
                &definition["models"]["1"],
                &definition["source"],
                format!("unsupported:set-bonuses:{pos}:{id}"),
                &mut mechanics,
                &mut sources,
                &mut mechanic_ids,
            );
        }
    }
    if sets.get("sete-percentage-hdt").is_some_and(|(count, _)| {
        *count as f64
            >= number(
                &data["sets"]["sete-percentage-hdt"],
                "requiredSetCount",
                f64::INFINITY,
            )
    }) {
        for value in gems.values_mut() {
            *value *= 1.25;
        }
    }
    let mut active = BTreeMap::<String, &Value>::new();
    for gem in arr(&data["gems"]) {
        if num(gem, "requiredPower") > *gems.get(string(gem, "family")).unwrap_or(&0.0) {
            continue;
        }
        let key = format!("{}:{}", string(gem, "family"), gem["effectSlot"]);
        if active
            .get(&key)
            .is_none_or(|g| num(g, "requiredPower") < num(gem, "requiredPower"))
        {
            active.insert(key, gem);
        }
    }
    for gem in active.values() {
        let mut m = gem["model"].clone();
        m["instanceId"] = json!(format!("active-gem:{}", string(gem, "id")));
        mechanics.push(m);
    }
    if let Some(m) = mechanics
        .iter()
        .find(|m| string(m, "classification") == "uncovered")
    {
        return Err(SimulationError {
            diagnostics: vec![],
            diagnostics_truncated: false,
            code: SimulationErrorCode::UncoveredMechanics,
            message: format!("Uncovered mechanic: {}", string(m, "sourceName")),
            sources: vec![string(m, "sourceId").into()],
        });
    }
    let stamina = num(starting, "stamina") + gear.get("Stamina").unwrap_or(&0.0);
    let mut stamina_add = 0.0;
    let mut stamina_mul = 1.0;
    let mut conversion = 1.0;
    for m in mechanics
        .iter()
        .filter(|m| string(m, "handler") == "static-stats")
    {
        let q = &m["parameters"];
        stamina_add += num(q, "staminaAdd");
        stamina_mul += number(q, "staminaMultiplier", 1.0) - 1.0;
        conversion += number(q, "staminaHealthConversionMultiplier", 1.0) - 1.0;
    }
    let health = apply_passive(
        &modifiers,
        "MaxHealth",
        num(starting, "baseHealth") + stamina * num(starting, "staminaToMaxHealthMultiplier"),
    );
    let health_mul = modifiers
        .iter()
        .filter(|m| string(m, "attribute") == "MaxHealth" && string(m, "operation") == "multiply")
        .fold(1.0, |acc, m| acc * num(m, "value"));
    let max_health = health
        + ((stamina + stamina_add) * stamina_mul * conversion - stamina)
            * num(starting, "staminaToMaxHealthMultiplier")
            * health_mul;
    for m in &mut mechanics {
        let source = string(m, "sourceId").to_owned();
        let q = m["parameters"].as_object_mut().unwrap();
        for key in [
            "staminaAdd",
            "staminaMultiplier",
            "staminaHealthConversionMultiplier",
        ] {
            q.remove(key);
        }
        if let Some(fraction) = q.remove("maxHealthDamageFraction") {
            q.insert(
                "flatDamage".into(),
                json!(max_health * fraction.as_f64().unwrap()),
            );
        }
        if let Some(expertise) = q.get_mut("expertise") {
            let applied = modifiers
                .iter()
                .filter(|m| {
                    string(m, "sourceId") == source
                        && sets.contains_key(&source)
                        && string(m, "attribute") == "Expertise"
                        && string(m, "operation") == "add"
                })
                .map(|m| num(m, "value"))
                .sum::<f64>();
            *expertise = json!(expertise.as_f64().unwrap() - applied);
        }
    }
    mechanics.sort_by(|a, b| string(a, "instanceId").cmp(string(b, "instanceId")));
    for id in &build.selected_talent_ids {
        let t = &hero["talents"][id];
        p["talents"]
            .as_array_mut()
            .unwrap()
            .push(t["model"].clone());
        for patch in arr(&t["abilityPatches"]) {
            let pointer = format!(
                "/{}",
                arr(&patch["path"])
                    .iter()
                    .map(|s| s.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join("/")
            );
            *p["abilities"]
                .pointer_mut(&pointer)
                .expect("authored ability patch") = patch["value"].clone();
        }
        p["uptimeNames"]
            .as_object_mut()
            .unwrap()
            .extend(t["uptimeNames"].as_object().unwrap().clone());
    }
    p["talents"]
        .as_array_mut()
        .unwrap()
        .sort_by(|a, b| string(a, "id").cmp(string(b, "id")));
    let sum = |key: &str| {
        mechanics
            .iter()
            .map(|m| num(&m["parameters"], key))
            .sum::<f64>()
    };
    let static_sum = |key: &str| {
        mechanics
            .iter()
            .filter(|m| string(m, "handler") == "static-stats")
            .map(|m| num(&m["parameters"], key))
            .sum::<f64>()
    };
    let product = |key: &str| {
        mechanics
            .iter()
            .fold(1.0, |acc, m| acc * number(&m["parameters"], key, 1.0))
    };
    let power_mul = mechanics
        .iter()
        .filter(|m| {
            string(m, "handler") == "static-stats"
                || string(m, "sourceId").starts_with("legendary-")
        })
        .fold(1.0, |acc, m| {
            acc * number(&m["parameters"], "powerMultiplier", 1.0)
        });
    p["power"] = json!(
        (num(starting, &primary.to_ascii_lowercase())
            + gear.get(primary).unwrap_or(&0.0)
            + static_sum("powerAdd"))
            * power_mul
    );
    for (stat, field, rating_field) in [
        ("CritChance", "criticalStrike", "criticalRating"),
        ("Expertise", "expertise", "expertiseRating"),
        ("Haste", "haste", "hasteRating"),
        ("Spirit", "spirit", "spiritRating"),
    ] {
        let base_rating = *gear.get(stat).unwrap_or(&0.0);
        let extra = static_sum(rating_field)
            + if stat == "Spirit" {
                mechanics
                    .iter()
                    .filter(|m| {
                        string(m, "sourceId") == "ItemTrait.ID.IncreasedMainStatAndSpiritRating"
                    })
                    .map(|m| num(&m["parameters"], "spiritRating"))
                    .sum()
            } else {
                0.0
            };
        let base = if stat == "CritChance" {
            num(starting, "critChance")
        } else if stat == "Haste" {
            (number(starting, "haste", 1.0) - 1.0).max(0.0)
        } else {
            0.0
        };
        p[field] = json!(
            apply_passive(&modifiers, stat, base + rating(stat, base_rating))
                + static_sum(field)
                + rating(stat, base_rating + extra)
                - rating(stat, base_rating)
                + if stat == "Haste" {
                    sum("idleHeroismHaste")
                } else {
                    0.0
                }
        );
        p[rating_field] = json!(base_rating + extra);
    }
    p["criticalMultiplier"] = json!(
        num(starting, "critMultiplier")
            * mechanics
                .iter()
                .filter(|m| string(m, "handler") == "static-stats")
                .fold(1.0, |acc, m| acc
                    * number(&m["parameters"], "criticalPowerMultiplier", 1.0))
    );
    p["cooldownRecovery"] = json!(
        1.0 + mechanics
            .iter()
            .filter(|m| string(m, "sourceId").starts_with("legendary-"))
            .map(|m| number(&m["parameters"], "cooldownAccelerationMultiplier", 1.0) - 1.0)
            .sum::<f64>()
    );
    p["maxSpirit"] = json!(num(&p, "maxSpirit") + sum("maxSpiritAdd"));
    p["heroismHaste"] = json!((num(&p, "heroismHaste") - sum("idleHeroismHaste")).max(0.0));
    p["heroismDurationMs"] = json!(
        (num(&p, "heroismDurationMs") + sum("heroismDurationSeconds") * 1000.0).round() as u64
    );
    let cooldown = (1.0 - static_sum("cooldownReduction")).max(0.0);
    for a in p["abilities"].as_array_mut().unwrap() {
        match string(a, "kind") {
            "winters-blessing" => {
                a["maximumCharges"] = json!(
                    mechanics
                        .iter()
                        .map(|m| num(&m["parameters"], "wintersBlessingCharges"))
                        .fold(num(a, "maximumCharges"), f64::max) as u32
                );
            }
            "bursting-ice" => {
                a["effectDurationMs"] = json!(
                    (num(a, "effectDurationMs") + sum("burstingDurationIncreaseSeconds") * 1000.0)
                        .round() as u64
                )
            }
            "heart-splitter" => {
                if let Some(n) = mechanics
                    .iter()
                    .find_map(|m| m["parameters"]["heartSplitterCharges"].as_f64())
                {
                    a["maximumCharges"] = json!(n.round() as u32);
                }
            }
            "grim-carve" => {
                a["directHits"] =
                    json!((num(a, "directHits") + sum("grimCarveAdditionalSpins")).round() as u32)
            }
            _ => {}
        }
        let original = num(a, "cooldownMs");
        if a["mechanicParameters"]["defaultCooldownMs"].is_null() {
            a["mechanicParameters"]["defaultCooldownMs"] = json!(original);
        }
        if a["gcdScalesWithCooldownReduction"] == true {
            a["gcdMs"] = json!((num(a, "gcdMs") * cooldown).round() as u64);
        }
        a["cooldownMs"] = json!((original * cooldown).round() as u64);
        if [
            "incinerate",
            "wrath-of-winter",
            "raging-tempest",
            "event-horizon",
            "matriarch-macabre",
            "bloodbound-spirit",
        ]
        .contains(&string(a, "kind"))
        {
            a["spiritCost"] = json!(num(a, "spiritCost") * product("spiritCostMultiplier"));
        }
    }
    let mut effects = Vec::new();
    for a in arr(&p["abilities"]) {
        if !a["dot"].is_null() {
            effects.push(json!({"id":a["kind"],"name":format!("{} damage over time",string(a,"name")),"source":{"kind":"ability-dot","abilityId":a["kind"]},"effectKind":"damage-over-time","supportedProperties":["active","remaining","stacks"]}));
        }
    }
    for m in &mechanics {
        sources.insert(string(m, "sourceId").into());
        mechanic_ids.insert(string(m, "mechanicId").into());
        p["uptimeNames"][format!("buff:{}", string(m, "instanceId"))] = m["sourceName"].clone();
        if !m["parameters"]["arachnidPoisonDamageFraction"].is_null() {
            p["uptimeNames"]["dot:mara-arachnid-poison"] = m["sourceName"].clone();
        }
        if string(m, "classification") == "modeled"
            && string(m, "sourceId") == "ItemTrait.ID.GemSingleTargetProcOnDamageHeal"
        {
            effects.push(json!({"id":target_id(string(m,"instanceId")),"name":format!("{} debuff",string(m,"sourceName")),"source":{"kind":"mechanic-target-buff","mechanicInstanceId":m["instanceId"]},"effectKind":"debuff","supportedProperties":["active","remaining","stacks"]}));
        }
    }
    let abilities = arr(&p["abilities"]).to_vec();
    for a in abilities {
        p["uptimeNames"][format!("dot:{}", string(&a, "kind"))] = a["name"].clone();
        p["uptimeNames"][format!("buff:{}", string(&a, "kind"))] = a["name"].clone();
    }
    for key in ["abilities", "scenarioNoOpAbilities", "talents"] {
        for source in arr(&p[key]) {
            sources.insert(string(source, "id").into());
        }
    }
    if sources.contains("GA_Bowguy_Helper_LunarlightSalvo")
        || sources.contains("GA_Bowguy_Helper_LunarlightEruption")
    {
        sources.insert("GA_Bowguy_Passive_InstantMarkTarget_Monitor".into());
    }
    let scope = serde_json::to_value(
        HeroIdentity::from_id(&build.hero_id)
            .unwrap()
            .contract()
            .evidence_scope(targets == 1),
    )
    .unwrap();
    let claims = arr(&hero["evidence"]["claims"])
        .iter()
        .filter(|c| {
            c["scope"] == scope
                && (string(c, "sourceKind") == "shared-calculation"
                    || sources.contains(string(c, "sourceId"))
                    || mechanic_ids.contains(string(c, "sourceId"))
                    || arr(&c["mechanicIds"])
                        .iter()
                        .any(|id| id.as_str().is_some_and(|id| mechanic_ids.contains(id))))
        })
        .cloned()
        .collect::<Vec<_>>();
    p["evidenceClaimIds"] = json!(claims.iter().map(|c| &c["id"]).collect::<Vec<_>>());
    p["aplTargetEffects"] = json!(effects);
    p["mechanics"] = json!(mechanics);
    let evidence = json!({"schemaVersion":1,"buildId":data["buildId"],"modelRevision":hero["evidence"]["modelRevision"],"evidenceFingerprint":hero["evidence"]["evidenceFingerprint"],"scope":scope,"claims":claims});
    Ok((
        serde_json::from_value(p).map_err(|e| invalid(format!("Prepared profile: {e}")))?,
        serde_json::from_value(evidence).map_err(|e| invalid(format!("Prepared evidence: {e}")))?,
    ))
}
