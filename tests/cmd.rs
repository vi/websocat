#![cfg(feature="ioful_tests")]
use assert_cmd::Command;

#[test]

fn trivial() {
    let mut cmd = Command::cargo_bin("websocat").unwrap();
    let assert = cmd
        .arg("literal:12345")
        .assert();
    assert
        .code(0)
        .stdout("12345");
}
