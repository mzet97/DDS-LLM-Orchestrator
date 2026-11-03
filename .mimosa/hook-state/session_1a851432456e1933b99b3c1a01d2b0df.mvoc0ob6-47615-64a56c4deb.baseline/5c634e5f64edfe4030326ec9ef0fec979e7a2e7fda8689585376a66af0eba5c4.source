//! ID físico por tentativa (fase 900; usado pelo materializer E pela
//! barreira do agente — fonte única).
//!
//! Convenção V1: `task_id = "laya1-" + hex(sha256("{workflow}|{plan_rev}|{node}|{attempt}"))`.

use sha2::{Digest, Sha256};

pub const LAYA1_PREFIX: &str = "laya1-";

pub fn laya1_task_id(workflow_id: &str, plan_revision: u32, node_id: &str, attempt: u32) -> String {
    let pre = format!("{workflow_id}|{plan_revision}|{node_id}|{attempt}");
    format!("{LAYA1_PREFIX}{:x}", Sha256::digest(pre.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_prefixed() {
        let a = laya1_task_id("wf", 1, "n", 1);
        assert_eq!(a, laya1_task_id("wf", 1, "n", 1));
        assert!(a.starts_with("laya1-"));
        assert_eq!(a.len(), 6 + 64);
        assert_ne!(a, laya1_task_id("wf", 1, "n", 2));
    }
}
