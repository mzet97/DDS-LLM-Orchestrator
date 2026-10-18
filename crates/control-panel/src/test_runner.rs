//! Descoberta/execução de testes com histórico (paridade com `test_runner.py`).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Item de teste descoberto.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestItem {
    pub id: String,
    pub path: PathBuf,
    pub category: String,
    pub description: String,
}

/// Execução registrada no histórico.
#[derive(Debug, Clone, Serialize)]
pub struct TestRunRecord {
    pub id: String,
    pub items: Vec<String>,
    pub command: Vec<String>,
    pub return_code: Option<i32>,
    pub output_tail: Vec<String>,
}

fn categorize(file_name: &str) -> String {
    if file_name.starts_with("smoke_") {
        "smoke".into()
    } else if file_name.contains("benchmark") {
        "benchmark".into()
    } else {
        "unit".into()
    }
}

fn first_doc_line(path: &Path) -> String {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .map(|l| l.trim().trim_matches(['"', '#']).trim().to_string())
                .find(|l| !l.is_empty())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Descobre `test_*.py`/`smoke_*.py` recursivamente (discover do Python).
pub fn discover(tests_dir: &Path) -> Vec<TestItem> {
    let mut items = Vec::new();
    let mut stack = vec![tests_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map(|n| n != "__pycache__").unwrap_or(true) {
                    stack.push(path);
                }
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if (name.starts_with("test_") || name.starts_with("smoke_")) && name.ends_with(".py") {
                items.push(TestItem {
                    description: first_doc_line(&path),
                    category: categorize(&name),
                    id: path.display().to_string(),
                    path,
                });
            }
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

/// Executa itens com `python -m pytest` e registra o histórico em JSONL.
pub struct TestRunner {
    tests_dir: PathBuf,
    history_file: PathBuf,
    runs: std::sync::Mutex<Vec<TestRunRecord>>,
    sequence: std::sync::atomic::AtomicU64,
}

impl TestRunner {
    pub fn new(tests_dir: PathBuf, history_file: PathBuf) -> Self {
        Self {
            tests_dir,
            history_file,
            runs: std::sync::Mutex::new(Vec::new()),
            sequence: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn discover(&self) -> Vec<TestItem> {
        discover(&self.tests_dir)
    }

    /// Executa os itens e devolve o registro (também anexado ao JSONL).
    pub fn run(&self, item_ids: &[String]) -> Result<TestRunRecord, String> {
        if item_ids.is_empty() {
            return Err("nenhum item selecionado".into());
        }
        let mut command = vec![
            "python3".to_string(),
            "-m".to_string(),
            "pytest".to_string(),
            "-q".to_string(),
        ];
        command.extend(item_ids.iter().cloned());
        let Some((program, args)) = command.split_first() else {
            return Err("comando vazio".into());
        };
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let tail_start = lines.len().saturating_sub(50);
        let record = TestRunRecord {
            id: format!(
                "test-run-{}",
                self.sequence
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            items: item_ids.to_vec(),
            command,
            return_code: output.status.code(),
            output_tail: lines[tail_start..].to_vec(),
        };
        if let Some(parent) = self.history_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(line) = serde_json::to_string(&record) {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.history_file)
            {
                let _ = writeln!(file, "{line}");
            }
        }
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(record.clone());
        Ok(record)
    }

    /// Detalhe de uma run (GET /run/<id> do original).
    pub fn get_run(&self, run_id: &str) -> Option<TestRunRecord> {
        self.list_history(usize::MAX)
            .into_iter()
            .find(|r| r.id == run_id)
    }

    /// Executa todos os itens de uma categoria (run-category do original).
    pub fn run_category(&self, category: &str) -> Result<TestRunRecord, String> {
        let ids: Vec<String> = self
            .discover()
            .into_iter()
            .filter(|item| item.category == category)
            .map(|item| item.id)
            .collect();
        if ids.is_empty() {
            return Err(format!("nenhum teste na categoria {category}"));
        }
        self.run(&ids)
    }

    pub fn list_history(&self, limit: usize) -> Vec<TestRunRecord> {
        // Histórico em memória + arquivo (mais recentes por último, corta em `limit`).
        let mut records = self.runs.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if records.is_empty() {
            if let Ok(text) = std::fs::read_to_string(&self.history_file) {
                records = text
                    .lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect::<Vec<serde_json::Value>>()
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect();
            }
        }
        let start = records.len().saturating_sub(limit);
        records[start..].to_vec()
    }
}

// Serialização reversa mínima para leitura do histórico.
impl<'de> serde::Deserialize<'de> for TestRunRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Raw {
            id: String,
            items: Vec<String>,
            command: Vec<String>,
            return_code: Option<i32>,
            output_tail: Vec<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(TestRunRecord {
            id: raw.id,
            items: raw.items,
            command: raw.command,
            return_code: raw.return_code,
            output_tail: raw.output_tail,
        })
    }
}
