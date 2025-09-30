pub struct VersionInfo {
    pub commit_date: &'static str,
    pub commit: &'static str,
}

pub fn version() -> VersionInfo {
    let commit_date = env!("VERGEN_GIT_COMMIT_TIMESTAMP");
    let commit = env!("VERGEN_GIT_SHA");
    VersionInfo {
        commit_date,
        commit,
    }
}
