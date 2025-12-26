#![cfg(feature = "ioful_tests")]
use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
#[expect(
    deprecated,
    reason = "cargo_bin is deprecated, cargo_bin! is not, `use` does not differenciate them"
)]
use assert_cmd::cargo::cargo_bin;

#[test]

fn trivial() {
    let mut cmd = cargo_bin_cmd!("websocat");
    let assert = cmd.arg("literal:12345").assert();
    assert.code(0).stdout("12345");
}

#[cfg(feature = "online_tests")]
#[test]
fn simple_roundtrip() {
    let mut cmd = cargo_bin_cmd!("websocat");
    let assert = cmd
        .arg("-t")
        .arg("ws://ws.vi-server.org/mirror")
        .write_stdin("12345\nqwerty\n")
        .assert();
    assert.code(0).stdout("12345\nqwerty\n");
}

#[cfg(feature = "online_tests")]
#[test]
fn binary_roundtrip() {

    let mut cmd = cargo_bin_cmd!("websocat");
    let assert = cmd
        .arg("-b")
        .arg("ws://ws.vi-server.org/mirror")
        .write_stdin("12345\nqwerty\n")
        .assert();
    assert.code(0).stdout("12345\nqwerty\n");
}

#[cfg(unix)]
#[test]
fn cmd_endpoint() {
    let mut cmd = cargo_bin_cmd!("websocat");
    let assert = cmd.arg("-b").arg("cmd:/bin/printf 'ABC\\x00DEF'").assert();
    assert.code(0).stdout(b"ABC\x00DEF" as &[u8]);
}

#[cfg(unix)]
#[test]
fn exec_endpoint() {
    let mut cmd = cargo_bin_cmd!("websocat");
    let assert = cmd
        .arg("-b")
        .arg("exec:/bin/printf")
        .arg("--exec-args")
        .arg("%s\\x00%s")
        .arg("ABC")
        .arg("DEF")
        .assert();
    assert.code(0).stdout(b"ABC\x00DEF" as &[u8]);
}

#[cfg(unix)]
#[test]
fn tricky() {
    let wsc = cargo_bin!("websocat").to_str().unwrap();
    let cmdline = format!(
        r#"
        {wsc} -b --global-timeout-ms=500 --oneshot tcp-l:127.0.0.1:13000 mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' & 
        sleep 0.05;
        {wsc} -b tcp:127.0.0.1:13000 mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'
    "#
    );
    let mut cmd = Command::new("sh");
    let assert = cmd.arg("-c").arg(cmdline).assert();
    assert.code(0).stdout("").stderr("");
}

#[cfg(unix)]
#[test]
fn async_fd() {
    let wsc = cargo_bin!("websocat").to_str().unwrap();
    let cmdline = format!(
        r#"
        {wsc} -b --global-timeout-ms=500 --oneshot tcp-l:127.0.0.1:13001 mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' & 
        sleep 0.05;
        {wsc} -b tcp:127.0.0.1:13001 --exec-dup2=3 exec:{wsc} --exec-args -b async-fd:3 mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'
    "#
    );
    let mut cmd = Command::new("sh");
    let assert = cmd.arg("-c").arg(cmdline).assert();
    assert.code(0).stdout("").stderr("");
}

#[cfg(unix)]
#[test]
fn async_fd_exec() {
    let wsc = cargo_bin!("websocat").to_str().unwrap();
    let cmdline = format!(
        r#"
        {wsc} -b --global-timeout-ms=500 --oneshot tcp-l:127.0.0.1:13002 mock_stream_socket:'W qqq\n|R www\n|W eee\n|R tt\n' & 
        sleep 0.05;
        {wsc} -b tcp:127.0.0.1:13002 --exec-dup2=0,1 exec:{wsc} --exec-dup2-execve --exec-args -b async-fd:0 mock_stream_socket:'R qqq\n|W www\n|R eee\n|W tt\n'
    "#
    );
    let mut cmd = Command::new("sh");
    let assert = cmd.arg("-c").arg(cmdline).assert();
    assert.code(0).stdout("").stderr("");
}
