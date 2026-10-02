use crate::preparation::*;
use serde_json::Value;

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
