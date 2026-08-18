# Spec 500 — Endurecimento DDS-first

## Objetivo

Alinhar o runtime Rust ao objetivo da dissertação: usar DDS no caminho principal sempre
que o participante remoto oferece DDS nativo, mantendo HTTP/gRPC apenas como interfaces
de compatibilidade ou fronteiras para provedores externos sem suporte DDS.

## Requisitos

- **REQ-501:** o agente reutiliza um único `DataWriter<LLMInferenceRequest>` durante a
  vida do `DdsEngine`; uma inferência não cria um writer novo.
- **REQ-502:** a restrição de provedor é um valor tipado de configuração e é publicada
  no campo IDL `provider_constraint`; o caminho local DDS usa `LOCAL_ONLY` por padrão.
- **REQ-503:** as descrições da dissertação distinguem o caminho local direto
  agente→DDS→llama-server do caminho de provedores externos mediado por gateway.
- **REQ-504:** o texto não atribui ao runtime condições de QoS, tópicos ou resultados
  que o código não materializa.

## Critérios de aceite

1. Teste comprova que cada variante tipada produz o literal IDL esperado.
2. Teste comprova que `infer_stream` captura o mesmo writer persistente do engine.
3. `cargo test -p agent --features dds`, Clippy e rustfmt passam.
4. A dissertação compila e descreve fielmente o caminho implementado.
