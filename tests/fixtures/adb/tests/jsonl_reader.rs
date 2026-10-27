#[path = "common/jsonl.rs"]
mod jsonl;
use jsonl::complete_json_lines;
use serde_json::json;

#[test]
fn an_incomplete_tail_is_not_a_committed_command() {
    assert_eq!(
        complete_json_lines(b"{\"key\":\"up\"}\n{\"key\":"),
        vec![json!({"key":"up"})]
    );
}
#[test]
fn appending_the_newline_commits_the_next_command() {
    let mut bytes = b"{\"key\":\"up\"}\n{\"key\":\"down\"}".to_vec();
    assert_eq!(complete_json_lines(&bytes), vec![json!({"key":"up"})]);
    bytes.push(b'\n');
    assert_eq!(
        complete_json_lines(&bytes),
        vec![json!({"key":"up"}), json!({"key":"down"})]
    );
}
#[test]
fn an_incomplete_utf8_tail_does_not_hide_committed_commands() {
    assert_eq!(
        complete_json_lines(b"{\"key\":\"up\"}\n{\"key\":\"\xd8"),
        vec![json!({"key":"up"})]
    );
}
#[test]
#[should_panic]
fn a_malformed_complete_record_remains_an_error() {
    complete_json_lines(b"{bad}\n");
}
