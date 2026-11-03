//! Expõe o hash curto do git da árvore como env de compilação (badge de
//! versão do header, mockup 3.x "v1.0-9b1b799"). Fallback honesto "dev"
//! quando a árvore não é um repositório (ex.: tarball) — nada inventado.

fn main() {
    let hash = std::env::var("STUDIO_GIT_HASH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_short_hash)
        .unwrap_or_else(|| String::from("dev"));
    println!("cargo:rustc-env=STUDIO_GIT_HASH={hash}");
}

/// `git rev-parse --short HEAD` a partir do diretório do crate.
fn git_short_hash() -> Option<String> {
    let dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let hash = hash.trim().to_owned();
    (!hash.is_empty()).then_some(hash)
}
