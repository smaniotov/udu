use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_shows_udu_usage_and_every_option() {
    Command::cargo_bin("udu")
        .expect("resolve udu binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: udu"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--root"))
        .stdout(predicate::str::contains("--soundpack"))
        .stdout(predicate::str::contains("--device-name"))
        .stdout(predicate::str::contains("--service"));
}

#[test]
fn reports_an_unknown_option() {
    Command::cargo_bin("udu")
        .expect("resolve udu binary")
        .arg("--bogus")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--bogus"));
}

#[test]
fn reports_a_missing_option_value() {
    Command::cargo_bin("udu")
        .expect("resolve udu binary")
        .args(["--root"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--root"));
}
