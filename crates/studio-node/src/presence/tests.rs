use super::{advertised_url, dds_domain_from_env, ENV_DDS_DOMAIN, HEARTBEAT};

// T-890: bind 0.0.0.0 atrás de NAT/overlay não é conectável — o override
// público vence; branco/vazio cai no derivado do bind.
#[test]
fn advertised_url_prefers_public_override() {
    use std::net::IpAddr;
    let any: IpAddr = "0.0.0.0".parse().unwrap();
    let lo: IpAddr = "127.0.0.1".parse().unwrap();
    assert_eq!(
        advertised_url(any, 4317, Some(String::from("http://192.168.1.62:4317"))),
        "http://192.168.1.62:4317"
    );
    assert_eq!(
        advertised_url(any, 4317, Some(String::from("   "))),
        "http://0.0.0.0:4317",
        "override vazio ignora"
    );
    assert_eq!(advertised_url(lo, 4317, None), "http://127.0.0.1:4317");
}

// T-890: node_id default é a autoridade da URL endereçável (única por
// instalação); 0.0.0.0 cai para hostname/bind — 3 hosts NUNCA colidem
// na mesma chave do tópico keyed (KL1 guardaria uma instância só).
#[cfg(feature = "dds")]
#[test]
fn default_node_id_derives_from_public_url() {
    use std::net::IpAddr;
    let any: IpAddr = "0.0.0.0".parse().unwrap();
    assert_eq!(
        super::dds_impl::default_node_id("http://192.168.1.62:4317", any, 4317),
        "192.168.1.62:4317"
    );
    assert_eq!(
        super::dds_impl::default_node_id("http://0.0.0.0:4317", any, 4317),
        format!(
            "{}:4317",
            std::env::var("HOSTNAME").unwrap_or_else(|_| "0.0.0.0".into())
        )
    );
}

// T-890: a env decide a presença — ausente/vazia/lixo desativam (nó
// segue HTTP-only), inteiro válido ativa. Função pura: sem DDS.
#[test]
fn dds_domain_env_resolves_presence_on_off() {
    assert_eq!(dds_domain_from_env(None), None, "env ausente desativa");
    assert_eq!(
        dds_domain_from_env(Some(String::new())),
        None,
        "env vazia desativa"
    );
    assert_eq!(
        dds_domain_from_env(Some("   ".into())),
        None,
        "env só de espaço desativa"
    );
    assert_eq!(
        dds_domain_from_env(Some("nao-e-numero".into())),
        None,
        "lixo desativa (não derruba o nó)"
    );
    assert_eq!(dds_domain_from_env(Some("0".into())), Some(0));
    assert_eq!(
        dds_domain_from_env(Some(" 42 ".into())),
        Some(42),
        "espaços em volta são tolerados"
    );
}

#[test]
fn heartbeat_is_five_seconds_under_ten_second_lease() {
    // Duas omissões de beat ainda cabem no lease ManualByTopic de 10 s.
    assert_eq!(HEARTBEAT.as_secs(), 5);
    assert!(HEARTBEAT.as_nanos() < 10_000_000_000);
}

#[test]
fn env_constant_is_stable() {
    // Contrato operacional documentado: renomear quebra deploy (Ansible)
    // e GUI.
    assert_eq!(ENV_DDS_DOMAIN, "STUDIO_NODE_DDS_DOMAIN");
}
