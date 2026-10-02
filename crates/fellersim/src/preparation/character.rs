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
    let data = fellersim_data::catalog();
    let hero = &data["heroes"][&build.hero_id];
    if build.schema_version != 6 || hero.is_null() {
        return Err(invalid(
            "Expected character schemaVersion 6 and a supported hero.",
        ));
    }
    if build.talent_points > 14
        || build.selected_talent_ids.len() > 32
        || build.positions.len() != arr(&data["positions"]).len()
        || build.disabled_conditional_contribution_ids.len() > 256
    {
        return Err(invalid(
            "Invalid character size, positions, or talent budget.",
        ));
    }
    let mut selected = BTreeSet::new();
    let mut points = 0.0;
    for id in &build.selected_talent_ids {
        let talent = &hero["talents"][id];
        if talent.is_null() || !selected.insert(id) {
            return Err(invalid(format!("Unknown or duplicate talent: {id}")));
        }
        points += num(talent, "pointCost");
    }
    if points > build.talent_points as f64 {
        return Err(invalid("Selected talents exceed the talent point budget."));
    }
    let mut positions = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut trait_ranks = BTreeMap::new();
    for position in &build.positions {
        let slot = arr(&data["positions"])
            .iter()
            .find(|p| string(p, "id") == position.position_id);
        if slot.is_none() || !positions.insert(&position.position_id) {
            return Err(invalid("Unknown or duplicate equipment position."));
        }
        let Some(item) = &position.item else {
            continue;
        };
        let definition = &hero["items"][&item.item_id];
        let config =
            &data["configurations"][definition["configs"][&item.rarity].as_str().unwrap_or("")];
        if definition.is_null()
            || string(definition, "itemType") != string(slot.unwrap(), "itemType")
            || config.is_null()
        {
            return Err(invalid(format!(
                "{} is not valid for {} at rarity {}",
                item.item_id, position.position_id, item.rarity
            )));
        }
        if !arr(&definition["validItemLevelsByRarity"][&item.rarity])
            .iter()
            .any(|n| n.as_u64() == Some(item.item_level as u64))
        {
            return Err(invalid("Invalid item level."));
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
            return Err(invalid("Applied tempers exceed the item's limit."));
        }
        *counts.entry(item.item_id.clone()).or_insert(0u32) += 1;
        if item.rolled_modifiers.len() > 64 || item.gems.len() > 4 || item.blessings.len() > 64 {
            return Err(invalid("Too many item selections."));
        }
        let mut selections = BTreeSet::new();
        for modifier in &item.rolled_modifiers {
            let slot = arr(&config["slots"])
                .iter()
                .find(|s| string(s, "id") == modifier.slot_id);
            if !selections.insert(&modifier.slot_id)
                || slot.is_none_or(|s| {
                    string(s, "kind") != modifier.kind
                        || !has(&s["allowedChoiceIds"], &modifier.choice_id)
                })
            {
                return Err(invalid(format!(
                    "Invalid or duplicate modifier slot: {}",
                    modifier.slot_id
                )));
            }
        }
        let mut blessings = BTreeMap::new();
        for blessing in &item.blessings {
            let slot = arr(&config["slots"])
                .iter()
                .find(|s| string(s, "id") == blessing.slot_id);
            if !selections.insert(&blessing.slot_id)
                || slot.is_none_or(|s| string(s, "kind") != "blessing")
                || !has(&config["blessingIds"], &blessing.blessing_id)
                || blessing.rank == 0
            {
                return Err(invalid("Invalid blessing selection."));
            }
            *blessings.entry(&blessing.blessing_id).or_insert(0u32) += blessing.rank;
        }
        for (id, rank) in blessings {
            if rank as f64 > num(&data["blessings"][id], "maxRank") {
                return Err(invalid("Blessing rank cap exceeded."));
            }
        }
        for group in arr(&config["rollGroups"]) {
            if selections
                .iter()
                .filter(|id| has(&group["slotIds"], id))
                .count()
                > 1
            {
                return Err(invalid("A random roll group permits one selection."));
            }
        }
        let mut sockets = BTreeSet::new();
        for gem in &item.gems {
            let index = gem
                .socket_id
                .strip_prefix("socket:")
                .and_then(|s| s.parse::<usize>().ok());
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
                return Err(invalid("Unknown gem or incompatible socket tier."));
            }
        }
        if let Some(tree) = &item.trait_tree {
            if config["traitTree"].is_null()
                || ["Common", "Uncommon", "Rare"].contains(&item.rarity.as_str())
                || tree.rolls.len() > 128
                || tree.selected_node_ids.len() > 32
            {
                return Err(invalid(
                    "This item has no active trait tree or its selections are too large.",
                ));
            }
            let nodes = arr(&config["traitTree"]["nodes"]);
            let mut rolls = BTreeSet::new();
            for roll in &tree.rolls {
                if !rolls.insert(&roll.node_id)
                    || nodes
                        .iter()
                        .find(|n| string(n, "id") == roll.node_id)
                        .is_none_or(|n| {
                            arr(&n["parentIds"]).is_empty()
                                || !has(&n["eligibleTraitIds"], &roll.trait_id)
                        })
                {
                    return Err(invalid("Unknown, duplicate, or ineligible trait roll."));
                }
            }
            let mut chosen = Vec::new();
            for id in &tree.selected_node_ids {
                let node = nodes.iter().find(|n| string(n, "id") == id);
                if node.is_none() || !rolls.contains(id) {
                    return Err(invalid("Selected trait node has no valid roll."));
                }
                chosen.push(node.unwrap());
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
                    return Err(invalid(
                        "Trait selections must form one connected path with one node per row.",
                    ));
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
            return Err(invalid(format!("Too many copies of {id}")));
        }
    }
    for (id, rank) in trait_ranks {
        if rank as f64 > num(&data["traits"][&id], "maxRank") {
            return Err(invalid(format!("Trait rank cap exceeded: {id}")));
        }
    }
    Ok(())
}
