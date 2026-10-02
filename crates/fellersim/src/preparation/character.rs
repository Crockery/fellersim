use super::*;

pub(super) fn ranks(item: &CharacterItem) -> BTreeMap<String, u32> {
    let mut ranks = BTreeMap::new();
    if let Some(tree) = &item.trait_tree {
        for node in &tree.selected_node_ids {
            if let Some(roll) = tree.rolls.iter().find(|r| &r.node_id == node) {
                *ranks.entry(roll.trait_id.clone()).or_default() += 1;
            }
        }
    }
    for modifier in &item.rolled_modifiers {
        if modifier.kind == "item-trait" {
            *ranks.entry(modifier.choice_id.clone()).or_default() += 1;
        }
    }
    ranks
}

pub(super) fn validate(build: &CharacterBuild) -> Result<(), SimulationError> {
    let issues = diagnose(build);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(SimulationError::from_diagnostics(
            SimulationErrorCode::InvalidBuild,
            issues,
        ))
    }
}

pub(super) fn diagnose(build: &CharacterBuild) -> Vec<Diagnostic> {
    let data = fellersim_data::catalog();
    let hero = &data["heroes"][&build.hero_id];
    let mut issues = Vec::new();
    macro_rules! issue {
        ($code:expr, $path:expr, $message:expr) => {
            if issues.len() <= MAX_DIAGNOSTICS {
                let path = ($path).to_string();
                let kind = match $code {
                    "unsupported-hero" => "heroes",
                    "unknown-or-duplicate-talent" | "talent-budget" | "talent-budget-exceeded" => {
                        "talents"
                    }
                    "invalid-gem" | "duplicate-gem" => "gems",
                    "invalid-trait" | "trait-rank-limit" => "traits",
                    "invalid-blessing" => "blessings",
                    _ => "equipment",
                };
                let help = if kind == "heroes" {
                    "Inspect valid heroes with fellersim catalog heroes.".into()
                } else {
                    format!(
                        "Inspect valid choices with fellersim catalog {kind} --hero {}.",
                        build.hero_id
                    )
                };
                let mut diagnostic = Diagnostic::error($code, $message).at(&path).help(help);
                if let Some(value) = serde_json::to_value(build).unwrap().pointer(&path) {
                    if let Some(id) = value.as_str() {
                        diagnostic.identifiers.push(id.into());
                    } else {
                        for key in [
                            "itemId",
                            "positionId",
                            "choiceId",
                            "gemId",
                            "traitId",
                            "blessingId",
                        ] {
                            if let Some(id) = value[key].as_str() {
                                diagnostic.identifiers.push(id.into());
                            }
                        }
                    }
                }
                issues.push(diagnostic);
            }
        };
    }
    if build.schema_version != 6 {
        issue!(
            "unsupported-character-schema",
            "/schemaVersion",
            "Expected character schemaVersion 6."
        );
    }
    if hero.is_null() {
        issue!(
            "unsupported-hero",
            "/heroId",
            "Expected a supported hero. Use fellersim catalog heroes."
        );
        return issues;
    }
    if build.talent_points > 14 {
        issue!(
            "talent-budget",
            "/talentPoints",
            "Talent budget must be at most 14."
        );
    }
    if build.selected_talent_ids.len() > 32
        || build.positions.len() != arr(&data["positions"]).len()
        || build.disabled_conditional_contribution_ids.len() > 256
    {
        issue!(
            "character-size",
            "",
            "Invalid character size, positions, or talent budget."
        );
        return issues;
    }
    let mut selected = BTreeSet::new();
    let mut points = 0.0;
    for (i, id) in build.selected_talent_ids.iter().enumerate() {
        let talent = &hero["talents"][id];
        if talent.is_null() || !selected.insert(id) {
            issue!(
                "unknown-or-duplicate-talent",
                format!("/selectedTalentIds/{i}"),
                format!("Unknown or duplicate talent: {id}")
            );
        } else {
            points += num(talent, "pointCost");
        }
    }
    if points > build.talent_points as f64 {
        issue!(
            "talent-budget-exceeded",
            "/selectedTalentIds",
            "Selected talents exceed the talent point budget."
        );
    }
    let mut positions = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut trait_ranks = BTreeMap::new();
    for (pi, position) in build.positions.iter().enumerate() {
        let path = format!("/positions/{pi}");
        let slot = arr(&data["positions"])
            .iter()
            .find(|p| string(p, "id") == position.position_id);
        if slot.is_none() || !positions.insert(&position.position_id) {
            issue!(
                "invalid-position",
                format!("{path}/positionId"),
                "Unknown or duplicate equipment position."
            );
            continue;
        }
        let Some(item) = &position.item else {
            continue;
        };
        let path = format!("{path}/item");
        let definition = &hero["items"][&item.item_id];
        let config =
            &data["configurations"][definition["configs"][&item.rarity].as_str().unwrap_or("")];
        if definition.is_null()
            || string(definition, "itemType") != string(slot.unwrap(), "itemType")
            || config.is_null()
        {
            issue!(
                "invalid-item",
                &path,
                format!(
                    "{} is not valid for {} at rarity {}",
                    item.item_id, position.position_id, item.rarity
                )
            );
            continue;
        }
        if !arr(&definition["validItemLevelsByRarity"][&item.rarity])
            .iter()
            .any(|n| n.as_u64() == Some(item.item_level as u64))
        {
            issue!(
                "invalid-item-level",
                format!("{path}/itemLevel"),
                "Invalid item level."
            );
        }
        let max_tempers = arr(&data["itemStatModel"]["curves"]["itemRarityAndLevelToMaxTempers"])
            .iter()
            .find(|c| string(c, "id") == item.rarity)
            .map(|c| {
                arr(&c["points"])
                    .iter()
                    .rfind(|p| num(p, "x") <= item.item_level as f64)
                    .map_or(0.0, |p| num(p, "y").trunc().max(0.0))
            })
            .unwrap_or(0.0);
        if item.applied_tempers as f64 > max_tempers {
            issue!(
                "temper-limit",
                format!("{path}/appliedTempers"),
                "Applied tempers exceed the item's limit."
            );
        }
        *counts.entry(item.item_id.clone()).or_insert(0u32) += 1;
        if item.rolled_modifiers.len() > 64 || item.gems.len() > 4 || item.blessings.len() > 64 {
            issue!("item-size", &path, "Too many item selections.");
            continue;
        }
        let mut selections = BTreeSet::new();
        for (i, modifier) in item.rolled_modifiers.iter().enumerate() {
            let slot = arr(&config["slots"])
                .iter()
                .find(|s| string(s, "id") == modifier.slot_id);
            if !selections.insert(&modifier.slot_id)
                || slot.is_none_or(|s| {
                    string(s, "kind") != modifier.kind
                        || !has(&s["allowedChoiceIds"], &modifier.choice_id)
                })
            {
                issue!(
                    "invalid-modifier",
                    format!("{path}/rolledModifiers/{i}"),
                    format!("Invalid or duplicate modifier slot: {}", modifier.slot_id)
                );
            }
        }
        let mut blessings = BTreeMap::new();
        for (i, blessing) in item.blessings.iter().enumerate() {
            let slot = arr(&config["slots"])
                .iter()
                .find(|s| string(s, "id") == blessing.slot_id);
            if !selections.insert(&blessing.slot_id)
                || slot.is_none_or(|s| string(s, "kind") != "blessing")
                || !has(&config["blessingIds"], &blessing.blessing_id)
                || blessing.rank == 0
            {
                issue!(
                    "invalid-blessing",
                    format!("{path}/blessings/{i}"),
                    "Invalid blessing selection."
                );
            }
            *blessings.entry(&blessing.blessing_id).or_insert(0u64) += u64::from(blessing.rank);
        }
        for (id, rank) in blessings {
            if rank as f64 > num(&data["blessings"][id], "maxRank") {
                issue!(
                    "blessing-rank-limit",
                    format!("{path}/blessings"),
                    "Blessing rank cap exceeded."
                );
            }
        }
        for group in arr(&config["rollGroups"]) {
            if selections
                .iter()
                .filter(|id| has(&group["slotIds"], id))
                .count()
                > 1
            {
                issue!(
                    "roll-group-conflict",
                    &path,
                    "A random roll group permits one selection."
                );
            }
        }
        let mut sockets = BTreeSet::new();
        for (i, gem) in item.gems.iter().enumerate() {
            let index = gem
                .socket_id
                .strip_prefix("socket:")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|index| gem.socket_id == format!("socket:{index}"));
            let tier = index
                .and_then(|i| config["socketTiers"].get(i))
                .and_then(Value::as_str)
                .and_then(|s| {
                    s.chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u32>()
                        .ok()
                });
            let definition = arr(&data["socketGems"])
                .iter()
                .find(|g| string(g, "id") == gem.gem_id);
            if !sockets.insert(&gem.socket_id)
                || tier.is_none()
                || definition.is_none_or(|g| num(g, "tier") > tier.unwrap_or(0) as f64)
            {
                issue!(
                    "invalid-gem",
                    format!("{path}/gems/{i}"),
                    "Unknown gem or incompatible socket tier."
                );
            }
        }
        if let Some(tree) = &item.trait_tree {
            if config["traitTree"].is_null()
                || ["Common", "Uncommon", "Rare"].contains(&item.rarity.as_str())
                || tree.rolls.len() > 128
                || tree.selected_node_ids.len() > 32
            {
                issue!(
                    "invalid-trait-tree",
                    format!("{path}/traitTree"),
                    "This item has no active trait tree or its selections are too large."
                );
                continue;
            }
            let nodes = arr(&config["traitTree"]["nodes"]);
            let mut rolls = BTreeSet::new();
            for (i, roll) in tree.rolls.iter().enumerate() {
                if !rolls.insert(&roll.node_id)
                    || nodes
                        .iter()
                        .find(|n| string(n, "id") == roll.node_id)
                        .is_none_or(|n| {
                            arr(&n["parentIds"]).is_empty()
                                || !has(&n["eligibleTraitIds"], &roll.trait_id)
                        })
                {
                    issue!(
                        "invalid-trait-roll",
                        format!("{path}/traitTree/rolls/{i}"),
                        "Unknown, duplicate, or ineligible trait roll."
                    );
                }
            }
            let mut chosen = Vec::new();
            for (i, id) in tree.selected_node_ids.iter().enumerate() {
                let node = nodes.iter().find(|n| string(n, "id") == id);
                if let Some(node) = node.filter(|_| rolls.contains(id)) {
                    chosen.push(node);
                } else {
                    issue!(
                        "invalid-trait-node",
                        format!("{path}/traitTree/selectedNodeIds/{i}"),
                        "Selected trait node has no valid roll."
                    );
                }
            }
            chosen.sort_by(|a, b| num(a, "row").total_cmp(&num(b, "row")));
            let mut rows = BTreeSet::new();
            let mut active: BTreeSet<String> = nodes
                .iter()
                .filter(|n| arr(&n["parentIds"]).is_empty())
                .map(|n| string(n, "id").to_owned())
                .collect();
            for node in chosen {
                if !rows.insert(num(node, "row") as u32)
                    || !arr(&node["parentIds"])
                        .iter()
                        .any(|p| p.as_str().is_some_and(|id| active.contains(id)))
                {
                    issue!(
                        "disconnected-trait-path",
                        format!("{path}/traitTree/selectedNodeIds"),
                        "Trait selections must form one connected path with one node per row."
                    );
                }
                active.insert(string(node, "id").to_owned());
            }
        }
        for (id, rank) in ranks(item) {
            *trait_ranks.entry(id).or_insert(0u32) += rank;
        }
    }
    for (id, count) in counts {
        if count as f64 > num(&hero["items"][&id], "maxEquipped") {
            issue!(
                "item-count-limit",
                "/positions",
                format!("Too many copies of {id}")
            );
        }
    }
    for (id, rank) in trait_ranks {
        if rank as f64 > num(&data["traits"][&id], "maxRank") {
            issue!(
                "trait-rank-limit",
                "/positions",
                format!("Trait rank cap exceeded: {id}")
            );
        }
    }
    issues
}
