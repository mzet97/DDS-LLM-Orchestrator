//! Métricas de hardware do host (paridade com `hardware_metrics.py`).
//! Sem psutil: CPU/RAM de `/proc`, GPU via `nvidia-smi`/`rocm-smi` se existirem.

use serde_json::{json, Value};

fn read_to_string(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Percentual de CPU por amostragem curta de /proc/stat (não bloqueante além de 20 ms).
fn cpu_percent_sample() -> Option<f64> {
    fn totals() -> Option<(u64, u64)> {
        let stat = read_to_string("/proc/stat")?;
        let line = stat.lines().next()?;
        let fields: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|f| f.parse().ok())
            .collect();
        if fields.len() < 4 {
            return None;
        }
        let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
        Some((fields.iter().sum(), idle))
    }
    let (total_a, idle_a) = totals()?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    let (total_b, idle_b) = totals()?;
    let total_delta = total_b.saturating_sub(total_a) as f64;
    if total_delta <= 0.0 {
        return None;
    }
    let idle_delta = idle_b.saturating_sub(idle_a) as f64;
    Some(((total_delta - idle_delta) / total_delta * 100.0 * 10.0).round() / 10.0)
}

/// Métricas de CPU com as chaves do Python (count_logical, count_physical, percent, load_average_1m).
pub fn cpu_metrics() -> Value {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let physical = read_to_string("/proc/cpuinfo")
        .map(|info| {
            let cores: std::collections::BTreeSet<&str> = info
                .lines()
                .filter(|l| l.starts_with("core id"))
                .filter_map(|l| l.split(':').nth(1))
                .collect();
            if cores.is_empty() {
                logical
            } else {
                cores.len()
            }
        })
        .unwrap_or(logical);
    let load1 = read_to_string("/proc/loadavg")
        .and_then(|avg| avg.split_whitespace().next()?.parse::<f64>().ok())
        .map(|v| (v * 100.0).round() / 100.0);
    json!({
        "count_logical": logical,
        "count_physical": physical,
        "percent": cpu_percent_sample(),
        "load_average_1m": load1,
    })
}

/// Métricas de RAM de /proc/meminfo (total_mb, available_mb, used_mb, percent).
pub fn memory_metrics() -> Value {
    let info = read_to_string("/proc/meminfo").unwrap_or_default();
    let kb = |key: &str| -> Option<f64> {
        info.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| (v / 1024.0 * 10.0).round() / 10.0)
    };
    let (total, available) = (kb("MemTotal:"), kb("MemAvailable:"));
    let (total_v, available_v) = (total.unwrap_or(0.0), available.unwrap_or(0.0));
    let used = (total_v - available_v * 10.0).round() / 10.0;
    // used_mb derivado com a mesma precisão de uma casa
    let used_mb = ((total_v - available_v) * 10.0).round() / 10.0;
    let _ = used;
    let percent = if total_v > 0.0 {
        ((total_v - available_v) / total_v * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };
    json!({
        "total_mb": total,
        "available_mb": available,
        "used_mb": used_mb,
        "percent": percent,
    })
}

fn run_capture(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

/// GPUs detectadas; ferramenta ausente ⇒ lista vazia (nunca erro).
pub fn gpu_metrics() -> Value {
    let mut gpus = Vec::new();
    if let Some(csv) = run_capture(
        "nvidia-smi",
        &[
            "--query-gpu=name,utilization.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ],
    ) {
        for line in csv.lines() {
            let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
            if fields.len() >= 4 {
                gpus.push(json!({
                    "vendor": "nvidia",
                    "name": fields[0],
                    "utilization_percent": parse_float(fields[1]),
                    "memory_used_mb": parse_float(fields[2]),
                    "memory_total_mb": parse_float(fields[3]),
                }));
            }
        }
    }
    if gpus.is_empty() {
        if let Some(out) = run_capture(
            "rocm-smi",
            &[
                "--showproductname",
                "--showuse",
                "--showmeminfo",
                "vram",
                "--csv",
            ],
        ) {
            for line in out.lines().skip(1) {
                if line.trim().is_empty() {
                    continue;
                }
                let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
                gpus.push(json!({
                    "vendor": "amd",
                    "name": fields.get(1).copied().unwrap_or("AMD GPU"),
                    "raw": line,
                }));
            }
        }
    }
    Value::Array(gpus)
}

/// Resumo completo do hardware (get_hardware_summary do Python).
pub fn hardware_summary() -> Value {
    json!({
        "cpu": cpu_metrics(),
        "memory": memory_metrics(),
        "gpus": gpu_metrics(),
    })
}

/// Parse tolerante (paridade com _parse_float/_parse_int do Python).
pub fn parse_float(value: &str) -> Option<f64> {
    value.trim().trim_end_matches('%').trim().parse().ok()
}

pub fn parse_int(value: &str) -> Option<i64> {
    value
        .trim()
        .trim_end_matches(['%', 'W'])
        .trim()
        .parse()
        .ok()
}
