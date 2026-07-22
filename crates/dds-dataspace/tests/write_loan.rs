//! Teste de round-trip dedicado para o zero-copy loan de `TaskOutput`
//! (Fase 4 de `OPTIMIZATION_PLAN.md`) — a mudança que exercita a correção do
//! `DdsType::Native` feita na crate `cyclonedds` nesta mesma sessão.
//!
//! Cobre exatamente o que o plano exigia antes de aceitar zero-copy no
//! streaming: >= 1000 chunks reais via DDS, 0 gaps de `seq_num`, e os 3
//! campos `String` (`task_id`, `content`, `agent_id`) chegando intactos do
//! outro lado — provando que o caminho `DdsString::new(..)` no loan e a
//! conversão de volta para `String` na leitura (`clone_out`, já corrigido na
//! WF-4) são consistentes ponta a ponta, não só "compila".
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::TaskOutput;
use dds_dataspace::writer_pool::WriteRequest;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 83;
const N: u32 = 1000;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_output(seq: u32) -> TaskOutput {
    TaskOutput {
        task_id: "write-loan-roundtrip-task".into(),
        seq_num: seq,
        // Conteúdo variável por chunk para provar que cada DdsString carrega
        // o valor certo (não um ponteiro reaproveitado/stale de outro loan).
        content: format!("chunk-{seq}"),
        is_final: seq == N - 1,
        finish_reason: if seq == N - 1 { 1 } else { 0 },
        agent_id: "agent-write-loan-test".into(),
        token_count: seq + 1,
        emitted_at_ns: now_ns(),
    }
}

/// 1000 chunks via loan zero-copy (`write_output_loan`, através do
/// `WriterPool`/`WriteRequest::Output`): 0 gaps de `seq_num`, e os campos
/// `String` (incluindo o conteúdo variável por chunk) chegam intactos.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn task_output_loan_roundtrip_1000_chunks_no_gaps() {
    let ds_pub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let pool = ds_pub.new_writer_pool(2, 4096);

    let mut stream = Box::pin(ds_sub.stream_task_outputs());
    tokio::time::sleep(Duration::from_millis(1500)).await; // settle/match

    for seq in 0..N {
        pool.submit(WriteRequest::Output(make_output(seq)))
            .expect("submit");
    }

    let mut seen: Vec<Option<TaskOutput>> = (0..N).map(|_| None).collect();
    let mut received = 0usize;
    while received < N as usize {
        match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
            Ok(Some(out)) => {
                let idx = out.seq_num as usize;
                assert!(idx < N as usize, "seq_num {idx} fora do intervalo esperado");
                assert!(
                    seen[idx].is_none(),
                    "seq_num {idx} recebido duas vezes (duplicata)"
                );
                seen[idx] = Some((*out).clone());
                received += 1;
            }
            Ok(None) => panic!("stream fechou antes de receber os {N} chunks"),
            Err(_) => panic!("timeout esperando chunk (recebidos {received}/{N})"),
        }
    }

    // 0 gaps: todo seq_num de 0..N foi recebido exatamente uma vez.
    let missing: Vec<usize> = seen
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_none())
        .map(|(i, _)| i)
        .collect();
    assert!(missing.is_empty(), "gaps de seq_num: {missing:?}");

    // Round-trip dos campos String (o ponto central desta correção): cada
    // chunk deve carregar o conteúdo/ids exatos escritos, não um valor
    // zerado/corrompido/reaproveitado de outro loan.
    let unique_contents: HashSet<String> = seen
        .iter()
        .map(|o| o.as_ref().unwrap().content.clone())
        .collect();
    assert_eq!(
        unique_contents.len(),
        N as usize,
        "conteúdo dos chunks não é único — indício de corrupção/reuso de buffer do loan"
    );
    for (idx, out) in seen.iter().enumerate() {
        let out = out.as_ref().unwrap();
        assert_eq!(out.task_id, "write-loan-roundtrip-task");
        assert_eq!(out.agent_id, "agent-write-loan-test");
        assert_eq!(out.content, format!("chunk-{idx}"));
        assert_eq!(out.token_count, idx as u32 + 1);
    }
    assert!(seen[N as usize - 1].as_ref().unwrap().is_final);

    println!("[Fase 4] {N} chunks via write_loan zero-copy: 0 gaps, campos String íntegros");

    pool.drain_and_shutdown();
    ds_pub.shutdown().await.unwrap();
    ds_sub.shutdown().await.unwrap();
}
