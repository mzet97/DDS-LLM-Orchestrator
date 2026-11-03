"""Ordem de campos dos structs do contrato — espelho da fixture congelada.

Gerado de `WorkflowPlanning.schema.json` por `tools/m1_02_mock_gen.py`-like;
a fonte da verdade é a fixture + IDL (gate `schema_fixture_matches_idl_source`
no dds-contract). Este módulo existe para o digest Python não depender de ler
JSON em runtime.
"""

STRUCT_FIELDS: dict[str, list[str]] = {
    "ProfileProbability": ["profile", "probability"],
    "CapabilityProbability": ["capability_id", "probability", "answer_confidence"],
    "ToolVersion": ["tool_id", "version"],
    "InputRef": ["producer_node_id", "output_name"],
    "OutputRef": ["name", "context_ref", "content_hash", "byte_count", "source_task_id"],
    "PlanNode": [
        "node_id", "kind", "required_capabilities", "model_id", "target_agent_id",
        "prompt_template", "dependencies", "inputs", "output_names", "max_attempts",
        "deadline_ns", "max_tokens", "max_output_bytes", "security_context_ref",
        "priority", "side_effect_key",
    ],
    "NodeState": [
        "node_id", "status", "attempt_id", "task_id", "assigned_agent_id", "ready",
        "started_at_ns", "completed_at_ns", "outputs", "reason_code",
    ],
    "WorkflowIntent": [
        "request_id", "workflow_id", "client_id", "schema_version", "intent_revision",
        "command", "expected_plan_revision", "prompt", "created_at_ns", "deadline_ns",
        "security_context_ref", "policy_revision", "questions_revision",
        "capability_catalog_revision", "checkpoint_revision", "max_tokens",
        "max_cost_microunits", "budget_unit", "fallback_template_id", "payload_hash",
    ],
    "RoutingDecision": [
        "request_id", "decision_id", "workflow_id", "schema_version", "intent_revision",
        "producer_id", "model_id", "checkpoint_revision", "questions_revision",
        "policy_revision", "capability_catalog_revision", "qos_profile",
        "qos_probabilities", "capability_probabilities", "requires_planning_probability",
        "priority_score", "priority_probabilities", "confidence", "answer_confidence",
        "calibration_revision", "created_at_ns", "valid_until_ns", "decision_status",
        "reason_code", "evidence_digest", "payload_hash",
    ],
    "AgentCapabilities": [
        "agent_id", "schema_version", "announcement_revision",
        "capability_catalog_revision", "capabilities", "model_ids", "tool_versions",
        "adaptive_protocol_version", "security_context_ref", "created_at_ns",
        "valid_until_ns",
    ],
    "WorkflowPlan": [
        "workflow_id", "plan_revision", "schema_version", "request_id",
        "intent_revision", "effective_decision_id", "policy_revision",
        "capability_catalog_revision", "planner_id", "planner_revision",
        "created_at_ns", "deadline_ns", "qos_profile", "nodes", "plan_hash",
    ],
    "WorkflowState": [
        "workflow_id", "plan_revision", "schema_version", "state_revision",
        "request_id", "effective_decision_id", "plan_hash", "status", "reason_code",
        "nodes", "created_at_ns", "updated_at_ns", "expires_at_ns",
        "cancelled_by_request_id", "effective_qos_config_hash", "fallback_template_id",
    ],
}
