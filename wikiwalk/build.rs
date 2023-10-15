fn main() {
    vergen::EmitBuilder::builder()
    .git_commit_timestamp()
    .git_sha(true)
    .emit()
    .unwrap();
}
