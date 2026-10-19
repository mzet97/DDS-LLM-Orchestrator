use super::*;

fn fixture_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-models-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp criado");
    std::fs::write(dir.join("a-Q4_K_M.gguf"), b"conteudo-a").expect("fixture a");
    std::fs::write(dir.join("b-Q4_K_M.gguf"), b"conteudo-b-b").expect("fixture b");
    std::fs::write(dir.join("nota.txt"), b"ignorado").expect("nao-gguf");
    dir
}

#[test]
fn quick_lists_without_hashing() {
    let dir = fixture_dir("quick");
    let list = quick_inventory(&dir).expect("lista");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].size_bytes, 10);
    assert!(list.iter().all(|item| item.sha256_hex.is_empty()));
}

#[test]
fn hash_is_stable_and_hex() {
    let dir = fixture_dir("hash");
    let first = hash_file(&dir, "a-Q4_K_M.gguf").expect("hash");
    let second = hash_file(&dir, "a-Q4_K_M.gguf").expect("hash de novo");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

/// PRD 3.7: fração e ETA do arquivo corrente derivam da taxa medida.
#[test]
fn hash_progress_derives_fraction_and_eta() {
    let progress = HashProgress {
        done: 1,
        total: 3,
        current: String::from("b.gguf"),
        current_bytes: 250,
        current_total: 1_000,
        rate_bps: 50,
    };
    assert!((progress.current_fraction() - 0.25).abs() < f32::EPSILON);
    assert_eq!(
        progress.current_eta_secs(),
        Some(15),
        "750 bytes restantes a 50 B/s"
    );

    let unknown = HashProgress {
        done: 0,
        total: 1,
        current: String::new(),
        current_bytes: 0,
        current_total: 0,
        rate_bps: 0,
    };
    assert_eq!(unknown.current_fraction(), 0.0);
    assert_eq!(unknown.current_eta_secs(), None);
}

/// Cancelamento intra-arquivo: flag interrompe o hash imediatamente.
#[test]
fn hash_file_with_progress_stops_on_flag() {
    let dir = fixture_dir("cancel");
    let stop = AtomicBool::new(true); // já cancelado
    let err = hash_file_with_progress(&dir, "a-Q4_K_M.gguf", Some(&stop), |_, _, _| {})
        .expect_err("flag cancelada aborta");
    assert!(matches!(err, ModelsError::Cancelled));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn background_hash_completes_through_poll() {
    let dir = fixture_dir("poll");
    let mut state = ModelsState::new();
    state.dir = dir.clone();
    state.refresh();

    assert!(state.is_busy());
    for _ in 0..1000 {
        state.poll();
        if !state.is_busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!state.is_busy());
    assert!(state.list.iter().all(|item| item.sha256_hex.len() == 64));
}

#[test]
fn missing_dir_becomes_typed_error() {
    let dir = std::env::temp_dir().join("studio-models-inexistente-xyz");

    let err = quick_inventory(&dir).expect_err("ausente deve falhar");

    assert!(matches!(err, ModelsError::Unreadable { .. }));
}

#[test]
fn env_dir_wins_over_home_fallback() {
    // T-830-06: env definido (mesmo apontando fora de HOME) vence.
    assert_eq!(
        resolve_models_dir(Some("/opt/modelos"), Some("/home/alguem")),
        "/opt/modelos"
    );
    // Espaços nas bordas não criam diretório fantasma.
    assert_eq!(resolve_models_dir(Some("  "), Some("/home/alguem")), "");
}

#[test]
fn home_fallback_only_when_tese_models_exists() {
    let home = std::env::temp_dir().join(format!("studio-models-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("tese/models")).expect("fixture home");
    let expected = home.join("tese/models").display().to_string();

    assert_eq!(
        resolve_models_dir(None, home.to_str()),
        expected,
        "$HOME/tese/models existente é o fallback"
    );
    let _ = std::fs::remove_dir_all(&home);

    // Sem o diretório (ou sem HOME): vazio — a UI sugere STUDIO_MODELS_DIR.
    assert_eq!(resolve_models_dir(None, home.to_str()), "");
    assert_eq!(resolve_models_dir(None, Some("/inexistente")), "");
    assert_eq!(resolve_models_dir(None, None), "");
}
