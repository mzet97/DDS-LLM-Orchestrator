const CANDIDATE_REV: &str = "98aae322dbabd64e9cd0ab3fe3cf18822930ef87";
const CANDIDATE_URL: &str = "https://github.com/mzet97/cyclonedds-rust.git";
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
const CONTRACT_MANIFEST: &str = include_str!("../Cargo.toml");
const LOCKFILE: &str = include_str!("../../../Cargo.lock");

#[test]
fn candidate_dependencies_use_one_immutable_git_revision() {
    // Given: the runtime workspace and contract build manifests.
    let expected = format!("git = \"{CANDIDATE_URL}\", rev = \"{CANDIDATE_REV}\"");

    // When: every CycloneDDS dependency declaration is inspected.
    let declarations = WORKSPACE_MANIFEST.matches(&expected).count()
        + CONTRACT_MANIFEST.matches(&expected).count();

    // Then: runtime, sys, and build tooling share the exact immutable candidate.
    assert_eq!(declarations, 3);
    assert!(!WORKSPACE_MANIFEST.contains("cyclonedds = { path ="));
    assert!(!CONTRACT_MANIFEST.contains("cyclonedds-build = { path ="));
}

#[test]
fn lockfile_records_the_candidate_git_source() {
    // Given: the lockfile committed by the runtime workspace.
    let source_prefix = format!("git+{CANDIDATE_URL}?rev={CANDIDATE_REV}#");

    // When: package source identities are inspected.
    let locked_sources = LOCKFILE.matches(&source_prefix).count();

    // Then: every CycloneDDS package resolved from the same candidate commit.
    assert_eq!(locked_sources, 5);
    assert!(LOCKFILE.contains(&format!("#{CANDIDATE_REV}")));
}

#[test]
fn contract_inputs_are_repository_local() {
    // Given: both IDLs and their generated C metadata are part of this crate.
    let inputs = [
        include_bytes!("../idl/OrchestratorDDS.idl").as_slice(),
        include_bytes!("../idl/OrchestratorDDS.c").as_slice(),
        include_bytes!("../idl/OrchestratorV4.idl").as_slice(),
        include_bytes!("../idl/OrchestratorV4.c").as_slice(),
    ];

    // When: the clean-checkout build inputs are inspected.
    let all_present = inputs.iter().all(|input| !input.is_empty());

    // Then: no contract input depends on a monorepo parent checkout.
    assert!(all_present);
}
