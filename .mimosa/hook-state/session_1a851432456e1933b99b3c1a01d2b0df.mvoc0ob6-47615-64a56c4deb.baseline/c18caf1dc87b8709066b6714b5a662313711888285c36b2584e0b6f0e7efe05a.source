use super::dds_impl::presence_sample;
use super::now_unix_ns;

// T-890: construção da amostra de presença — a forma do wire (typename,
// @key, roundtrip XCDR) é gate da dds-contract; aqui vale o conteúdo que
// o nó publica. Função pura: nenhuma entidade DDS no teste.
#[test]
fn presence_sample_stamps_identity_and_fresh_last_seen_ns() {
    let before = now_unix_ns();
    let sample = presence_sample(
        before,
        "no-lab-1",
        "http://127.0.0.1:4317",
        true,
        "dds-agent,llm-local",
    );
    assert_eq!(sample.node_id, "no-lab-1");
    assert_eq!(sample.url, "http://127.0.0.1:4317");
    // Versão do protocolo vem da fonte única do `protocol` do nó (1.0).
    assert_eq!(sample.protocol_major, 1);
    assert_eq!(sample.protocol_minor, 0);
    assert!(sample.token_required);
    assert_eq!(sample.services_hint, "dds-agent,llm-local");
    assert_eq!(sample.last_seen_ns, before);
    assert!(sample.last_seen_ns > 0, "carimbo de agora não pode ser 0");
}
