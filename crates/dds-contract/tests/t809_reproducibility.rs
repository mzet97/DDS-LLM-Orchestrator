const CYCLONEDDS_VERSION: &str = "=3.0.0-alpha.4";
const SYS_VERSION: &str = "=1.1.2";
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
const CONTRACT_MANIFEST: &str = include_str!("../Cargo.toml");
const LOCKFILE: &str = include_str!("../../../Cargo.lock");

#[test]
fn candidate_dependencies_use_exact_crates_io_versions() {
    // Given: the runtime workspace and contract build manifests.
    // When: every CycloneDDS dependency declaration is inspected.
    // Then: runtime and sys use exact prerelease versions from crates.io,
    //       and build tooling uses the workspace version.
    assert!(
        WORKSPACE_MANIFEST.contains(&format!(
            "cyclonedds = {{ version = \"{CYCLONEDDS_VERSION}\" }}"
        )),
        "workspace must pin exact cyclonedds crates.io version"
    );
    assert!(
        WORKSPACE_MANIFEST.contains(&format!(
            "cyclonedds-rust-sys = {{ version = \"{SYS_VERSION}\" }}"
        )),
        "workspace must pin exact cyclonedds-rust-sys crates.io version"
    );
    assert!(
        CONTRACT_MANIFEST.contains("cyclonedds-build = { workspace = true }"),
        "dds-contract must inherit cyclonedds-build from workspace"
    );

    // No git or path dependencies remain for CycloneDDS crates.
    assert!(!WORKSPACE_MANIFEST.contains("github.com/mzet97/cyclonedds-rust"));
    assert!(!CONTRACT_MANIFEST.contains("github.com/mzet97/cyclonedds-rust"));
    assert!(!WORKSPACE_MANIFEST.contains("cyclonedds = { path ="));
    assert!(!CONTRACT_MANIFEST.contains("cyclonedds-build = { path ="));
}

#[test]
fn lockfile_records_registry_sources() {
    // Given: the lockfile committed by the runtime workspace.
    // When: package source identities are inspected.
    // Then: every CycloneDDS package resolves from crates.io registry.
    assert!(LOCKFILE.contains("name = \"cyclonedds\""));
    assert!(LOCKFILE.contains("name = \"cyclonedds-rust-sys\""));
    assert!(LOCKFILE.contains("name = \"cyclonedds-build\""));
    assert!(LOCKFILE.contains("name = \"cyclonedds-derive\""));
    assert!(LOCKFILE.contains("source = \"registry+https://github.com/rust-lang/crates.io-index\""));
    assert!(!LOCKFILE.contains("github.com/mzet97/cyclonedds-rust"));
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
