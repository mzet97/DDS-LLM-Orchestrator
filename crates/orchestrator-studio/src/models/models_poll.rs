//! Parte de `models` (extraída para manter os arquivos revisáveis).

use super::*;

impl ModelsState {
    /// Drena o progresso; chamar a cada frame enquanto `is_busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    HashMsg::FileDone { index, sha } => {
                        let status = self.manifest_status_for_index(index, &sha);
                        if let Some(artifact) = self.list.get_mut(index) {
                            artifact.sha256_hex = sha;
                            artifact.manifest_status = status;
                        }
                        if let Some(progress) = self.hashing.as_mut() {
                            progress.done = progress.done.saturating_add(1);
                            progress.current = self
                                .list
                                .get(progress.done)
                                .map(|artifact| artifact.file_name.clone())
                                .unwrap_or_default();
                            progress.current_bytes = 0;
                            progress.current_total = 0;
                            progress.rate_bps = 0;
                        }
                    }
                    HashMsg::Progress {
                        bytes_done,
                        file_total,
                        rate_bps,
                    } => {
                        if let Some(progress) = self.hashing.as_mut() {
                            progress.current_bytes = bytes_done;
                            progress.current_total = file_total;
                            progress.rate_bps = rate_bps;
                        }
                    }
                    HashMsg::Finished => {
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.cancel = None;
            self.hashing = None;
        }
    }

    pub(super) fn spawn_hash(&mut self) {
        if self.list.is_empty() {
            return;
        }
        let dir = self.dir.clone();
        let names: Vec<String> = self
            .list
            .iter()
            .map(|item| item.file_name.clone())
            .collect();
        let (tx, rx) = mpsc::channel();
        let flag = Arc::new(AtomicBool::new(false));
        let stop = flag.clone();
        std::thread::spawn(move || {
            for (index, name) in names.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let tx_progress = tx.clone();
                let result =
                    hash_file_with_progress(&dir, name, Some(&stop), |bytes, total, rate| {
                        // Throttle extra na thread de UI: o worker já limita a
                        // ~6.7 msg/s por arquivo; canal absorve picos.
                        let _ = tx_progress.send(HashMsg::Progress {
                            bytes_done: bytes,
                            file_total: total,
                            rate_bps: rate,
                        });
                    });
                match result {
                    Ok(sha) => {
                        let _ = tx.send(HashMsg::FileDone { index, sha });
                    }
                    Err(ModelsError::Cancelled) => break,
                    Err(_) => continue,
                }
            }
            let _ = tx.send(HashMsg::Finished);
        });
        self.receiver = Some(rx);
        self.cancel = Some(flag);
        self.hashing = Some(HashProgress {
            done: 0,
            total: self.list.len(),
            current: self
                .list
                .first()
                .map(|item| item.file_name.clone())
                .unwrap_or_default(),
            current_bytes: 0,
            current_total: 0,
            rate_bps: 0,
        });
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        Self::new()
    }
}
