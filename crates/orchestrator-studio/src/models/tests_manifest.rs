use super::{manifest_status, ManifestStatus, ModelsError, ModelsManifest, ModelsState};

const SHA_A: &str = "bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517";

// T-890-08 (G-09..11): parse do manifesto com validação de 64 hex.
#[test]
fn manifest_parse_validates_sha_and_normalizes_case() {
    let manifest =
        ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
            .expect("manifesto válido");
    assert_eq!(manifest.expected("qwen.gguf"), Some(SHA_A));
    assert_eq!(manifest.len(), 1);

    // MAIÚSCULAS normalizadas para minúsculas.
    let upper = ModelsManifest::from_json(&format!(
        "{{\"models\": {{\"qwen.gguf\": \"{}\"}}}}",
        SHA_A.to_ascii_uppercase()
    ))
    .expect("sha em maiúsculas é aceito");
    assert_eq!(upper.expected("qwen.gguf"), Some(SHA_A));
}

#[test]
fn manifest_parse_rejects_malformed_inputs() {
    assert!(matches!(
        ModelsManifest::from_json("não é json"),
        Err(ModelsError::Manifest(_))
    ));
    // sem a chave "models"
    assert!(matches!(
        ModelsManifest::from_json("{\"outro\": 1}"),
        Err(ModelsError::Manifest(_))
    ));
    // sha curto
    assert!(matches!(
        ModelsManifest::from_json("{\"models\": {\"a.gguf\": \"abcd\"}}"),
        Err(ModelsError::Manifest(_))
    ));
}

// Cruzamento: pendente sem hash, OK confere, DESVIADO difere,
// SemRegistro fora do manifesto (o coração do G-09..11).
#[test]
fn manifest_status_classifies_ok_deviant_unregistered() {
    let manifest =
        ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
            .expect("manifesto");
    assert_eq!(
        manifest_status(&manifest, "qwen.gguf", ""),
        ManifestStatus::Pendente
    );
    assert_eq!(
        manifest_status(&manifest, "qwen.gguf", SHA_A),
        ManifestStatus::Ok
    );
    assert_eq!(
        manifest_status(&manifest, "qwen.gguf", &"0".repeat(64)),
        ManifestStatus::Desviado
    );
    assert_eq!(
        manifest_status(&manifest, "desconhecido.gguf", SHA_A),
        ManifestStatus::SemRegistro
    );
}

/// Integração: FileDone via poll aplica o status do manifesto carregado.
#[test]
fn poll_applies_manifest_status_after_hash() {
    let dir = std::env::temp_dir().join(format!(
        "studio-manifest-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("qwen.gguf"), b"conteudo do gguf de teste").expect("escreve");
    let expected_sha = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(b"conteudo do gguf de teste");
        format!("{:x}", h.finalize())
    };
    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.refresh();
    state.manifest = Some(
        ModelsManifest::from_json(&format!(
            "{{\"models\": {{\"qwen.gguf\": \"{expected_sha}\"}}}}"
        ))
        .expect("manifesto"),
    );
    for _ in 0..400 {
        state.poll();
        if !state.is_busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!state.is_busy(), "hash não concluiu");
    let artifact = &state.list[0];
    assert_eq!(artifact.sha256_hex, expected_sha);
    assert_eq!(artifact.manifest_status, ManifestStatus::Ok);
    std::fs::remove_dir_all(&dir).ok();
}

/// DESVIADO: sha real difere do congelado (cópia corrompida/trocada).
#[test]
fn poll_marks_deviant_when_sha_differs_from_manifest() {
    let dir = std::env::temp_dir().join(format!(
        "studio-deviant-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("qwen.gguf"), b"arquivo adulterado").expect("escreve");
    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.refresh();
    state.manifest = Some(
        ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
            .expect("manifesto"),
    );
    for _ in 0..400 {
        state.poll();
        if !state.is_busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(state.list[0].manifest_status, ManifestStatus::Desviado);
    std::fs::remove_dir_all(&dir).ok();
}
