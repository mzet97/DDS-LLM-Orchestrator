//! A identidade Ed25519 NÃO é determinística: cada `generate` usa
//! `PrivateKey::random` e cifra com a senha informada. Duas gerações com
//! a MESMA senha, em diretórios novos, produzem chaves públicas
//! diferentes; uma identidade existente conserva a fingerprint quando
//! apenas desbloqueada.

use studio_ssh::{generate, public_fingerprint, unlock};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-id-rand-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn chave_b64(pub_path: &std::path::Path) -> String {
    let line = std::fs::read_to_string(pub_path).expect("lê .pub");
    line.split_whitespace()
        .nth(1)
        .expect(".pub no formato authorized_keys")
        .to_string()
}

#[test]
fn mesma_senha_em_diretorios_novos_gera_publicas_diferentes() {
    let senha = "mesma-senha-de-teste";
    let dir1 = temp_dir("geracao-1");
    let dir2 = temp_dir("geracao-2");

    let p1 = generate(&dir1, senha).expect("gera 1");
    let p2 = generate(&dir2, senha).expect("gera 2");

    assert_ne!(
        chave_b64(&p1.public_openssh),
        chave_b64(&p2.public_openssh),
        "chaves geradas com a mesma senha devem ser diferentes"
    );

    let fp1 = public_fingerprint(&p1.private_pem, senha).expect("fp 1");
    let fp2 = public_fingerprint(&p2.private_pem, senha).expect("fp 2");
    assert_ne!(fp1, fp2, "fingerprints devem ser diferentes");

    let _ = std::fs::remove_dir_all(&dir1);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn identidade_existente_preserva_fingerprint_ao_desbloquear() {
    let senha = "senha-de-desbloqueio";
    let dir = temp_dir("unlock");

    let paths = generate(&dir, senha).expect("gera");
    let fp_antes = public_fingerprint(&paths.private_pem, senha).expect("fp antes");

    // Desbloqueios repetidos (mesmo arquivo, mesma senha) conservam a chave.
    let fp_depois = public_fingerprint(&paths.private_pem, senha).expect("fp depois");
    assert_eq!(fp_antes, fp_depois);

    let privada = unlock(&paths.private_pem, senha).expect("desbloqueia");
    let privada_pub_linha = format!(
        "{} studio-teste",
        privada
            .public_key()
            .to_openssh()
            .expect("publica em openssh")
    );
    let esperada = std::fs::read_to_string(&paths.public_openssh).expect("lê .pub");
    let esperada_b64 = esperada.split_whitespace().nth(1).expect("campo da chave");
    assert!(
        privada_pub_linha.contains(esperada_b64),
        "a chave desbloqueada corresponde à pública gerada"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
