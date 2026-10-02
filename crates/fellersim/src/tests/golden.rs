use super::*;

#[test]
fn ardeos_full_result_matches_structural_refactor_fixture() {
    assert_full_result_fixture(
        "ardeos",
        ardeos_fixture_request(),
        include_str!("fixtures/ardeos-result.json"),
    );
}

#[test]
fn rime_full_result_matches_structural_refactor_fixture() {
    assert_full_result_fixture(
        "rime",
        rime_fixture_request(),
        include_str!("fixtures/rime-result.json"),
    );
}
