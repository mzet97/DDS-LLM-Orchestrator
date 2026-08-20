use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dds_contract::generated::dds_llm_orchestrator::{
    SecurityPolicySnapshot, SecurityPolicyUpdate, ToolCallRequest,
};
use policy_engine::{PolicyDecision as DocumentDecision, PolicyDocument, SecurityLevel};

/// Maximum time a policy sample remains usable.
pub const DEFAULT_POLICY_MAX_AGE: Duration = Duration::from_secs(300);
/// Policy instance consumed by the MCP gateway.
pub const DEFAULT_POLICY_ID: &str = "default";

/// Stable, redaction-safe reason recorded for every denied call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenialReason {
    NoSnapshot,
    InvalidLevel,
    MissingRequester,
    Expired,
    ToolDenied,
    LevelDenied,
}

impl DenialReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoSnapshot => "no_snapshot",
            Self::InvalidLevel => "invalid_level",
            Self::MissingRequester => "missing_requester",
            Self::Expired => "expired",
            Self::ToolDenied => "tool_denied",
            Self::LevelDenied => "level_denied",
        }
    }
}

/// Policy decision carrying only non-sensitive audit metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allowed { version: i32 },
    Denied { reason: DenialReason },
}

impl PolicyDecision {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Typed rejection of an untrusted policy sample.
#[derive(Debug, thiserror::Error)]
pub enum PolicyIngestError {
    #[error("unexpected policy_id")]
    WrongPolicyId,
    #[error("policy version must be positive")]
    InvalidVersion,
    #[error("published_by must be present")]
    MissingPublisher,
    #[error("policy timestamp is zero, future, or expired")]
    InvalidTimestamp,
    #[error("policy JSON is invalid: {0}")]
    InvalidDocument(#[from] policy_engine::PolicyError),
    #[error("policy document version does not match envelope")]
    VersionMismatch,
    #[error("policy document is empty")]
    EmptyDocument,
    #[error("policy version is stale or conflicts with current state")]
    StaleVersion,
    #[error("update does not continue current version/publisher")]
    InvalidUpdateChain,
    #[error("update delta is invalid JSON: {0}")]
    InvalidDeltaJson(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
struct ActivePolicy {
    version: i32,
    document: PolicyDocument,
    publisher: String,
    timestamp_ns: u64,
}

/// Single-policy, bounded fail-closed state fed by DDS snapshots and updates.
pub struct DistributedPolicy {
    policy_id: String,
    max_age_ns: u64,
    active: RwLock<Option<ActivePolicy>>,
}

impl Default for DistributedPolicy {
    fn default() -> Self {
        Self::new(DEFAULT_POLICY_ID, DEFAULT_POLICY_MAX_AGE)
    }
}

impl DistributedPolicy {
    pub fn new(policy_id: impl Into<String>, max_age: Duration) -> Self {
        Self {
            policy_id: policy_id.into(),
            max_age_ns: u64::try_from(max_age.as_nanos()).unwrap_or(u64::MAX),
            active: RwLock::new(None),
        }
    }

    pub fn ingest_snapshot(
        &self,
        snapshot: &SecurityPolicySnapshot,
    ) -> Result<(), PolicyIngestError> {
        self.ingest_snapshot_at(snapshot, unix_now_ns())
    }

    pub fn ingest_snapshot_at(
        &self,
        snapshot: &SecurityPolicySnapshot,
        now_ns: u64,
    ) -> Result<(), PolicyIngestError> {
        self.validate_envelope(
            &snapshot.policy_id,
            snapshot.version,
            &snapshot.published_by,
            snapshot.timestamp_ns,
            now_ns,
        )?;
        let document = PolicyDocument::from_json_str(&snapshot.policy_json)?;
        if document.is_empty() {
            return Err(PolicyIngestError::EmptyDocument);
        }
        if document.version() != snapshot.version {
            return Err(PolicyIngestError::VersionMismatch);
        }

        let mut active = self
            .active
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = active.as_ref() {
            if snapshot.version < current.version
                || snapshot.timestamp_ns <= current.timestamp_ns
                || (snapshot.version == current.version
                    && (document.as_value() != current.document.as_value()
                        || snapshot.published_by != current.publisher))
            {
                return Err(PolicyIngestError::StaleVersion);
            }
        }
        *active = Some(ActivePolicy {
            version: snapshot.version,
            document,
            publisher: snapshot.published_by.clone(),
            timestamp_ns: snapshot.timestamp_ns,
        });
        Ok(())
    }

    pub fn ingest_update(&self, update: &SecurityPolicyUpdate) -> Result<(), PolicyIngestError> {
        self.ingest_update_at(update, unix_now_ns())
    }

    pub fn ingest_update_at(
        &self,
        update: &SecurityPolicyUpdate,
        now_ns: u64,
    ) -> Result<(), PolicyIngestError> {
        self.validate_envelope(
            &update.policy_id,
            update.new_version,
            &update.published_by,
            update.timestamp_ns,
            now_ns,
        )?;
        let delta = serde_json::from_str(&update.rule_delta_json)?;
        let mut active = self
            .active
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = active
            .as_ref()
            .ok_or(PolicyIngestError::InvalidUpdateChain)?;
        if update.previous_version != current.version
            || update.new_version <= current.version
            || update.published_by != current.publisher
            || update.timestamp_ns <= current.timestamp_ns
        {
            return Err(PolicyIngestError::InvalidUpdateChain);
        }
        let mut document = current.document.clone();
        document.apply_delta(&update.operation, &delta)?;
        document.set_version(update.new_version);
        if document.is_empty() {
            return Err(PolicyIngestError::EmptyDocument);
        }
        *active = Some(ActivePolicy {
            version: update.new_version,
            document,
            publisher: update.published_by.clone(),
            timestamp_ns: update.timestamp_ns,
        });
        Ok(())
    }

    pub fn evaluate(&self, request: &ToolCallRequest) -> PolicyDecision {
        self.evaluate_at(request, unix_now_ns())
    }

    pub fn evaluate_at(&self, request: &ToolCallRequest, now_ns: u64) -> PolicyDecision {
        let level = match SecurityLevel::try_from(request.security_level) {
            Ok(level) => level,
            Err(_) => {
                return PolicyDecision::Denied {
                    reason: DenialReason::InvalidLevel,
                }
            }
        };
        if request.requester_id.is_empty() {
            return PolicyDecision::Denied {
                reason: DenialReason::MissingRequester,
            };
        }
        let active = self
            .active
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(policy) = active.as_ref() else {
            return PolicyDecision::Denied {
                reason: DenialReason::NoSnapshot,
            };
        };
        if now_ns < policy.timestamp_ns
            || now_ns.saturating_sub(policy.timestamp_ns) > self.max_age_ns
        {
            return PolicyDecision::Denied {
                reason: DenialReason::Expired,
            };
        }
        if !policy
            .document
            .check_tool_call(&request.requester_id, &request.tool_name)
            .is_allowed()
        {
            return PolicyDecision::Denied {
                reason: DenialReason::ToolDenied,
            };
        }
        match policy
            .document
            .check_llm_request(&request.requester_id, level as i32)
        {
            DocumentDecision::Allowed => PolicyDecision::Allowed {
                version: policy.version,
            },
            DocumentDecision::AllowedNoPolicy | DocumentDecision::Denied(_) => {
                PolicyDecision::Denied {
                    reason: DenialReason::LevelDenied,
                }
            }
        }
    }

    fn validate_envelope(
        &self,
        policy_id: &str,
        version: i32,
        published_by: &str,
        timestamp_ns: u64,
        now_ns: u64,
    ) -> Result<(), PolicyIngestError> {
        if policy_id != self.policy_id {
            return Err(PolicyIngestError::WrongPolicyId);
        }
        if version <= 0 {
            return Err(PolicyIngestError::InvalidVersion);
        }
        if published_by.is_empty() {
            return Err(PolicyIngestError::MissingPublisher);
        }
        if timestamp_ns == 0
            || timestamp_ns > now_ns
            || now_ns.saturating_sub(timestamp_ns) > self.max_age_ns
        {
            return Err(PolicyIngestError::InvalidTimestamp);
        }
        Ok(())
    }
}

fn unix_now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
