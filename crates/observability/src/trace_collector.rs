//! Trace Collector — agregador de Execution.Trace.
//!
//! Substitui `trace_collector/trace_collector.py` com DashMap em memória.

use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Evento de trace processado.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub trace_id: String,
    pub seq_num: u32,
    pub event_type: i32,
    pub task_id: String,
    pub agent_id: String,
    pub payload_json: String,
    pub timestamp_ns: u64,
}

/// Agregador de Execution.Trace events.
pub struct TraceCollector {
    events: DashMap<String, Vec<TraceEvent>>,
    total_count: AtomicU64,
    output_dir: PathBuf,
}

impl TraceCollector {
    pub fn new(output_dir: &str) -> anyhow::Result<Self> {
        let dir = PathBuf::from(output_dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            events: DashMap::new(),
            total_count: AtomicU64::new(0),
            output_dir: dir,
        })
    }

    /// Ingestão de um evento de trace.
    pub fn ingest(
        &self,
        event: &dds_contract::generated::dds_llm_orchestrator::ExecutionTraceEvent,
    ) {
        let trace_event = TraceEvent {
            trace_id: event.trace_id.clone(),
            seq_num: event.seq_num,
            event_type: event.event_type,
            task_id: event.task_id.clone(),
            agent_id: event.agent_id.clone(),
            payload_json: event.payload_json.clone(),
            timestamp_ns: event.timestamp_ns,
        };

        self.events
            .entry(event.trace_id.clone())
            .or_default()
            .push(trace_event);
        self.total_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Retorna eventos de um trace.
    pub fn get_trace(&self, trace_id: &str) -> Vec<TraceEvent> {
        self.events
            .get(trace_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    /// Retorna todos os trace_ids.
    pub fn all_trace_ids(&self) -> Vec<String> {
        self.events.iter().map(|e| e.key().clone()).collect()
    }

    /// Número total de traces.
    pub fn trace_count(&self) -> usize {
        self.events.len()
    }

    /// Número total de eventos.
    pub fn event_count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Flush eventos para arquivo JSONL (drain-and-write).
    ///
    /// Cada evento é gravado **exatamente uma vez**: os eventos são retirados
    /// do mapa (drain) antes da escrita, e o append seguinte só contém o delta
    /// — sem isso, o flush periódico duplicava todo o histórico a cada 30 s
    /// (REQ/T-820-04). Em erro de escrita, os eventos drenados são reinseridos
    /// (best-effort) para não serem perdidos — o pior caso é duplicação de uma
    /// escrita parcial, nunca perda.
    pub fn flush(&self) -> anyhow::Result<()> {
        // Drain: retira os eventos de cada entrada e remove as entradas
        // vazias (memória liberada; `get_trace` pós-flush devolve vazio).
        let mut drained: Vec<TraceEvent> = Vec::new();
        for mut entry in self.events.iter_mut() {
            drained.extend(std::mem::take(entry.value_mut()));
        }
        self.events.retain(|_, events| !events.is_empty());

        if drained.is_empty() {
            return Ok(());
        }

        let write_result = (|| -> anyhow::Result<()> {
            let file_path = self.output_dir.join("traces.jsonl");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)?;

            use std::io::Write;
            for event in &drained {
                let json = serde_json::json!({
                    "trace_id": event.trace_id,
                    "seq_num": event.seq_num,
                    "event_type": event.event_type,
                    "task_id": event.task_id,
                    "agent_id": event.agent_id,
                    "payload_json": event.payload_json,
                    "timestamp_ns": event.timestamp_ns,
                });
                writeln!(file, "{}", json)?;
            }
            Ok(())
        })();

        if let Err(e) = write_result {
            tracing::warn!(
                error = %e,
                count = drained.len(),
                "flush de traces falhou; eventos reinseridos (best-effort)"
            );
            for event in drained {
                self.events
                    .entry(event.trace_id.clone())
                    .or_default()
                    .push(event);
            }
            return Err(e);
        }
        Ok(())
    }

    /// Limpa eventos antigos (uso em testes; o serviço usa `flush`, que drena).
    pub fn clear(&self) {
        self.events.clear();
        self.total_count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dds_contract::generated::dds_llm_orchestrator::ExecutionTraceEvent;

    fn make_event(trace_id: &str, seq: u32) -> ExecutionTraceEvent {
        ExecutionTraceEvent {
            trace_id: trace_id.into(),
            seq_num: seq,
            event_type: 1,
            task_id: "t1".into(),
            request_id: "r1".into(),
            agent_id: "a1".into(),
            component_id: "c1".into(),
            component_type: 0,
            payload_json: "{}".into(),
            timestamp_ns: 1000 + seq as u64,
        }
    }

    #[test]
    fn trace_collector_ingest_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let tc = TraceCollector::new(dir.path().to_str().unwrap()).unwrap();

        tc.ingest(&make_event("trace-1", 0));
        tc.ingest(&make_event("trace-1", 1));
        tc.ingest(&make_event("trace-2", 0));

        assert_eq!(tc.trace_count(), 2);
        assert_eq!(tc.event_count(), 3);

        let events = tc.get_trace("trace-1");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq_num, 0);
        assert_eq!(events[1].seq_num, 1);
    }

    #[test]
    fn trace_collector_flush_writes_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let tc = TraceCollector::new(dir.path().to_str().unwrap()).unwrap();

        tc.ingest(&make_event("t1", 0));
        tc.ingest(&make_event("t1", 1));
        tc.flush().unwrap();

        let file_path = dir.path().join("traces.jsonl");
        assert!(file_path.exists());

        let content = std::fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["trace_id"], "t1");
        assert_eq!(first["seq_num"], 0);
    }

    /// REQ/T-820-04: dois flushes não duplicam linhas — o arquivo contém
    /// exatamente o total de eventos ingeridos (flush drena o mapa).
    #[test]
    fn two_flushes_do_not_duplicate_lines_and_file_matches_total() {
        let dir = tempfile::tempdir().unwrap();
        let tc = TraceCollector::new(dir.path().to_str().unwrap()).unwrap();

        tc.ingest(&make_event("a", 0));
        tc.ingest(&make_event("a", 1));
        tc.flush().unwrap();

        // Eventos chegando entre flushes vão para o delta do próximo flush.
        tc.ingest(&make_event("b", 0));
        tc.flush().unwrap();

        let file_path = dir.path().join("traces.jsonl");
        let content = std::fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3, "sem duplicação entre flushes");

        // Cada (trace_id, seq_num) aparece exatamente uma vez.
        let mut keys: Vec<(String, u32)> = lines
            .iter()
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).unwrap();
                (
                    v["trace_id"].as_str().unwrap().into(),
                    v["seq_num"].as_u64().unwrap() as u32,
                )
            })
            .collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 3);

        // O mapa foi drenado: pós-flush não há eventos pendentes, mas o
        // contador de vida (ingestões) permanece com o total.
        assert_eq!(tc.trace_count(), 0);
        assert_eq!(tc.event_count(), 3);

        // Terceiro flush sem eventos novos não acrescenta linhas.
        tc.flush().unwrap();
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content.lines().filter(|l| !l.trim().is_empty()).count(), 3);
    }

    /// REQ/T-820-04: em erro de escrita os eventos drenados são reinseridos
    /// (best-effort) — nada é perdido.
    #[test]
    fn flush_error_reinserts_drained_events() {
        let dir = tempfile::tempdir().unwrap();
        let tc = TraceCollector::new(dir.path().to_str().unwrap()).unwrap();
        tc.ingest(&make_event("t1", 0));
        tc.ingest(&make_event("t1", 1));

        // Remove o diretório de saída: a abertura do arquivo falha.
        std::fs::remove_dir_all(dir.path()).unwrap();
        assert!(tc.flush().is_err());

        // Eventos de volta no mapa (nada perdido).
        assert_eq!(tc.trace_count(), 1);
        assert_eq!(tc.get_trace("t1").len(), 2);
        assert_eq!(tc.event_count(), 2);
    }
}
