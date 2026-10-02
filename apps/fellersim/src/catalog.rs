use crate::{command::Catalog, input, output::Failure};
use fellersim_core::{preparation::*, *};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const KINDS: &[&str] = &[
    "heroes",
    "abilities",
    "talents",
    "equipment",
    "gems",
    "traits",
    "blessings",
    "sets",
    "resources",
    "buffs",
    "target-effects",
    "apl-references",
    "apl-operators",
];
fn token(value: &Value) -> String {
    value.as_str().unwrap_or("").replace('-', "_")
}
fn readable(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("")
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn search(args: Catalog) -> Result<Value, Failure> {
    if !KINDS.contains(&args.kind.as_str()) {
        return Err(Failure::input(
            "unknown-catalog",
            format!("Catalog kinds: {}.", KINDS.join(", ")),
            "/kind",
        ));
    }
    let data = fellersim_data::catalog();
    let build = args
        .character
        .as_ref()
        .map(|p| input::character(p))
        .transpose()?;
    if let (Some(build), Some(hero)) = (&build, &args.hero)
        && build.hero_id != *hero
    {
        return Err(Failure::input(
            "hero-mismatch",
            "--hero must match the character.",
            "/hero",
        ));
    }
    if let Some(build) = &build {
        prepare_character(build, 1).map_err(Failure::from)?;
    }
    let hero_filter = args
        .hero
        .as_deref()
        .or_else(|| build.as_ref().map(|b| b.hero_id.as_str()));
    if let Some(hero) = hero_filter
        && data["heroes"][hero].is_null()
    {
        return Err(Failure::input(
            "unsupported-hero",
            "Unknown hero. Use catalog heroes.",
            "/hero",
        ));
    }
    let heroes = data["heroes"].as_object().unwrap();
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    if ["traits", "blessings", "sets"].contains(&args.kind.as_str()) {
        for (id, value) in data[&args.kind].as_object().unwrap() {
            entries.insert(id.clone(), json!({"id":id,"name":value["source"]["sourceName"],"description":value["source"]["sourceDescription"],"maxRank":value["maxRank"],"models":value["models"],"requiredSetCount":value["requiredSetCount"]}));
        }
    } else if args.kind == "gems" {
        for value in data["socketGems"].as_array().unwrap() {
            entries.insert(value["id"].as_str().unwrap().into(), value.clone());
        }
    } else if args.kind == "apl-operators" {
        for (id, name) in [
            ("&", "and (short circuit)"),
            ("|", "or (short circuit)"),
            ("!", "not"),
            ("=", "equal"),
            ("!=", "not equal"),
            ("<", "less than"),
            ("<=", "less than or equal"),
            (">", "greater than"),
            (">=", "greater than or equal"),
            ("()", "group"),
        ] {
            entries.insert(id.into(), json!({"id":id,"name":name,"token":id}));
        }
    } else {
        for (hero_id, hero) in heroes
            .iter()
            .filter(|(id, _)| hero_filter.is_none_or(|h| h == *id))
        {
            let character = build
                .clone()
                .unwrap_or(empty_character(hero_id).map_err(Failure::from)?);
            let (mut profile, _) = prepare_character(&character, 1).map_err(Failure::from)?;
            if build.is_none() {
                // General hero support includes optional weapon actions. Build queries use only resolved abilities.
                for (item_id, item) in hero["items"].as_object().unwrap() {
                    if !item["weapon"].is_null() {
                        let weapon: DpsAbilityModel =
                            serde_json::from_value(item["weapon"].clone()).unwrap();
                        if !profile.abilities.iter().any(|a| a.kind == weapon.kind) {
                            profile.abilities.push(weapon);
                            let (rarity, levels) = item["validItemLevelsByRarity"]
                                .as_object()
                                .unwrap()
                                .iter()
                                .next()
                                .unwrap();
                            let mut equipped = character.clone();
                            equipped
                                .positions
                                .iter_mut()
                                .find(|p| p.position_id == "weapon")
                                .unwrap()
                                .item = Some(CharacterItem {
                                item_id: item_id.clone(),
                                item_level: levels[0].as_u64().unwrap() as u32,
                                rarity: rarity.clone(),
                                applied_tempers: 0,
                                rolled_modifiers: vec![],
                                gems: vec![],
                                trait_tree: None,
                                blessings: vec![],
                            });
                            let (equipped_profile, _) =
                                prepare_character(&equipped, 1).map_err(Failure::from)?;
                            for effect in equipped_profile.apl_target_effects {
                                if !profile.apl_target_effects.iter().any(|e| e.id == effect.id) {
                                    profile.apl_target_effects.push(effect);
                                }
                            }
                        }
                    }
                }
            }
            let values: Vec<Value> = match args.kind.as_str() {
                "heroes" => vec![json!({"id":hero_id,"name":hero["name"],"modelVersion":hero["evidence"]["modelRevision"]})],
                "abilities" => profile.abilities.iter().map(|a| {
                    let kind = serde_json::to_value(a.kind).unwrap();
                    json!({"id":kind,"name":a.name,"gameAbilityId":a.id,"token":token(&kind),"manuallyCastable":a.manually_castable,"availability":if build.is_some() {"build"} else {"hero"},"model":a})
                }).collect(),
                "talents" => hero["talents"].as_object().unwrap().iter().map(|(id,t)| json!({"id":id,"name":t["model"]["name"],"description":t["description"],"pointCost":t["pointCost"],"model":t["model"],"token":format!("talent.{id}.enabled"),"selected":character.selected_talent_ids.contains(id)})).collect(),
                "equipment" => hero["items"].as_object().unwrap().iter().filter_map(|(id,item)| {
                    if let Some(position) = &args.position {
                        let slot = data["positions"].as_array().unwrap().iter().find(|p| p["id"] == *position);
                        if slot.is_none_or(|p| p["itemType"] != item["itemType"]) { return None; }
                    }
                    if let Some(rarity) = &args.rarity && item["configs"][rarity].is_null() { return None; }
                    if let Some(level) = args.level {
                        let levels = &item["validItemLevelsByRarity"];
                        let valid = if let Some(rarity) = &args.rarity { levels[rarity].as_array().is_some_and(|v| v.contains(&json!(level))) }
                        else { levels.as_object().unwrap().values().any(|v| v.as_array().unwrap().contains(&json!(level))) };
                        if !valid { return None; }
                    }
                    let configurations: BTreeMap<_,_> = item["configs"].as_object().unwrap().iter().map(|(rarity,key)| (rarity, &data["configurations"][key.as_str().unwrap()])).collect();
                    Some(json!({"id":id,"name":item["name"].as_str().unwrap_or(id),"description":item["description"],"itemType":item["itemType"],"maxEquipped":item["maxEquipped"],"validItemLevelsByRarity":item["validItemLevelsByRarity"],"configurations":configurations,"setBonusIds":item["setBonusIds"]}))
                }).collect(),
                "resources" => supported_resources(hero_id).into_iter().map(|r| { let id=serde_json::to_value(r).unwrap(); json!({"id":id,"name":readable(&id),"token":format!("resource.{}.current",token(&id))}) }).collect(),
                "buffs" => supported_buffs(&profile).into_iter().map(|r| { let id=serde_json::to_value(r).unwrap(); json!({"id":id,"name":readable(&id),"token":format!("buff.{}.up",token(&id))}) }).collect(),
                "target-effects" => profile.apl_target_effects.iter().map(|e| { let mut v=serde_json::to_value(e).unwrap(); v["tokens"]=json!(apl_references(&profile).iter().filter(|r| r.reference["effectId"]==e.id).map(|r| &r.token).collect::<Vec<_>>()); v }).collect(),
                "apl-references" => apl_references(&profile).into_iter().map(|r| json!({"id":r.token,"name":r.token,"token":r.token,"valueType":r.value_type,"reference":r.reference})).collect(),
                _ => vec![],
            };
            for mut entry in values {
                let id = entry["id"].as_str().unwrap().to_owned();
                entry["heroIds"] = json!([hero_id]);
                if let Some(existing) = entries.get_mut(&id) {
                    existing["heroIds"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(hero_id));
                } else {
                    entries.insert(id, entry);
                }
            }
        }
    }
    if let Some(position) = &args.position
        && !data["positions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == *position)
    {
        return Err(Failure::input(
            "unknown-position",
            "Use an equipment position from character init.",
            "/position",
        ));
    }
    let query = args.query.as_ref().map(|q| q.to_lowercase());
    let matches: Vec<_> = entries
        .into_values()
        .filter(|v| args.id.as_ref().is_none_or(|id| v["id"] == *id))
        .filter(|v| {
            query.as_ref().is_none_or(|q| {
                ["id", "name", "description", "token"]
                    .iter()
                    .any(|k| v[k].as_str().is_some_and(|s| s.to_lowercase().contains(q)))
            })
        })
        .collect();
    let total = matches.len();
    let entries = matches
        .into_iter()
        .skip(args.offset as usize)
        .take(args.limit as usize)
        .collect::<Vec<_>>();
    Ok(
        json!({"kind":args.kind,"total":total,"offset":args.offset,"limit":args.limit,"nextOffset":((args.offset as usize+entries.len())<total).then_some(args.offset as usize+entries.len()),"entries":entries}),
    )
}
