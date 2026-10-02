use crate::preparation::*;
use serde_json::Value;

fn equipped_character(position: &str, item_id: &str) -> CharacterBuild {
    let file: Value = serde_json::from_str(include_str!(
        "../../../../apps/fellersim/examples/ardeos.json"
    ))
    .unwrap();
    let mut build: CharacterBuild = serde_json::from_value(file["build"].clone()).unwrap();
    build
        .positions
        .iter_mut()
        .find(|p| p.position_id == position)
        .unwrap()
        .item = Some(CharacterItem {
        item_id: item_id.into(),
        item_level: 15,
        rarity: "Epic".into(),
        applied_tempers: 0,
        rolled_modifiers: vec![],
        gems: vec![],
        trait_tree: None,
        blessings: vec![],
    });
    build
}

#[test]
fn rejects_duplicate_sockets_disguised_with_alternate_numeric_spellings() {
    let mut build = equipped_character("wrists", "wrists-setc-b-expertise");
    let gem = CharacterGem {
        socket_id: "socket:0".into(),
        gem_id: "ItemID.GemType.Amethyst.Tier1".into(),
    };
    build
        .positions
        .iter_mut()
        .find_map(|p| p.item.as_mut())
        .unwrap()
        .gems = vec![gem.clone()];
    assert!(prepare_character(&build, 1).is_ok());
    for alias in ["socket:00", "socket:+0", "socket:0"] {
        let item = build
            .positions
            .iter_mut()
            .find_map(|p| p.item.as_mut())
            .unwrap();
        item.gems = vec![
            gem.clone(),
            CharacterGem {
                socket_id: alias.into(),
                ..gem.clone()
            },
        ];
        assert!(prepare_character(&build, 1).is_err(), "{alias}");
    }
}

#[test]
fn rejects_blessing_rank_totals_that_exceed_u32_without_panicking() {
    let mut build = equipped_character("chest", "chest-c-expertise");
    let item = build
        .positions
        .iter_mut()
        .find_map(|p| p.item.as_mut())
        .unwrap();
    item.blessings = vec![CharacterBlessing {
        slot_id: "random:0:AbilityRank".into(),
        blessing_id: "DynamicItemAbilityRank.01".into(),
        rank: 1,
    }];
    assert!(prepare_character(&build, 1).is_ok());
    let item = build
        .positions
        .iter_mut()
        .find_map(|p| p.item.as_mut())
        .unwrap();
    item.blessings[0].rank = u32::MAX;
    item.blessings.push(CharacterBlessing {
        slot_id: "random:1:AbilityRank".into(),
        blessing_id: "DynamicItemAbilityRank.01".into(),
        rank: 2,
    });
    assert!(prepare_character(&build, 1).is_err());
}

#[test]
fn apl_preserves_comments_disabled_actions_and_operator_precedence() {
    let apl = crate::parse_apl("# opening\r\n# ACTIONS=/infernal_wave,IF=!buff.wildfire.up|resource.spirit.current>=1e1&fight.remains<30 # note\r\nactions+=/fire_ball\r\n# tail").unwrap();
    assert_eq!(apl.rules.len(), 2);
    assert!(!apl.rules[0].enabled);
    assert_eq!(apl.rules[0].leading_comments, ["opening"]);
    assert_eq!(apl.rules[0].inline_comment.as_deref(), Some("note"));
    assert_eq!(apl.trailing_comments, ["tail"]);
    let condition = serde_json::to_value(&apl.rules[0].condition).unwrap();
    assert_eq!(condition["kind"], "any");
    assert_eq!(condition["children"][0]["kind"], "not");
    assert_eq!(condition["children"][1]["kind"], "all");
    assert_eq!(
        condition["children"][1]["children"][0]["right"]["value"],
        10.0
    );
}

#[test]
fn apl_rejects_malformed_unsupported_and_unbounded_conditions() {
    for source in [
        "actions+=/infernal_wave",
        "actions=/infernal_wave, if=fight.remains<5",
        "actions=/infernal_wave,if=(fight.remains<5",
        "actions=/infernal_wave,if=1e999<2",
        "actions=/infernal_wave,if=fight.remains+1<2",
        "actions=/infernal_wave,if=buff.unknown.up",
        "actions=/infernal_wave,if=resource.unknown.current>1",
    ] {
        let error = crate::parse_apl(source).unwrap_err();
        assert!(error.message.contains("line 1, column"), "{error}");
    }
    let nested = format!(
        "actions=/infernal_wave,if={}buff.wildfire.up",
        "!".repeat(1000)
    );
    assert!(crate::parse_apl(&nested).is_err());
}

fn compare(expected: &Value, actual: &Value, path: &str) {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: keys");
            for (key, value) in a {
                compare(value, &b[key], &format!("{path}.{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: length\n{a:?}\n{b:?}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"));
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10 * a.abs().max(1.0),
                "{path}: expected {a}, got {b}"
            );
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

#[test]
fn captured_typescript_preparation_matches_rust() {
    let file = std::fs::File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/fixtures/preparation.json.gz"
    ))
    .unwrap();
    let fixtures: Vec<Value> = serde_json::from_reader(flate2::read::GzDecoder::new(file)).unwrap();
    for (index, fixture) in fixtures.iter().enumerate() {
        let character: CharacterBuild =
            serde_json::from_value(fixture["character"].clone()).unwrap();
        let (profile, evidence) =
            prepare_character(&character, fixture["targets"].as_u64().unwrap() as u32)
                .unwrap_or_else(|e| panic!("fixture {index}: {e}"));
        compare(
            &fixture["profile"],
            &serde_json::to_value(profile).unwrap(),
            &format!("fixture {index}.profile"),
        );
        compare(
            &fixture["evidence"],
            &serde_json::to_value(evidence).unwrap(),
            &format!("fixture {index}.evidence"),
        );
    }
}
