//! Living documentation (Constitution §9).
//!
//! trycmd executes the markdown transcripts under `tests/cmd/*.md` as black-box
//! snapshot tests against the compiled `clabby` binary. The docs ARE the test: if
//! output drifts, the build fails. After an intentional CLI change, regenerate the
//! snapshots with:
//!
//!   TRYCMD=overwrite cargo test -p clabby --test cli_docs
//!
//! Requires `node` and `git` on PATH (the offline fake tracker is a node script).

#[test]
fn cli_docs() {
    trycmd::TestCases::new().case("tests/cmd/*.md");
}
