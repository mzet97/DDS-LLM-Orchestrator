"""Contratos versionados do benchmark (Prompt §7). Imutáveis por versão."""

from __future__ import annotations

from dataclasses import dataclass, field

SCHEMA_VERSION = 1
WORKLOAD_SEQ_CHAIN = "seq_chain_v1"
WORKLOAD_FORK_JOIN = "fork_join_v1"
WORKLOAD_FORK_JOIN_SERIAL = "fork_join_serial_v1"


@dataclass(frozen=True, slots=True)
class WorkflowInput:
    case_id: str
    workflow_id: str
    workload_id: str
    workload_version: str = "v1"
    entry_text: str = ""


@dataclass(frozen=True, slots=True)
class StageInput:
    workflow_id: str
    step_id: str
    attempt_id: int
    request_id: str
    agent_role: str
    agent_instance_id: str
    messages: tuple[dict[str, str], ...]
    input_hash: str = ""
    prompt_hash: str = ""


@dataclass(frozen=True, slots=True)
class StageOutput:
    request_id: str
    step_id: str
    content: str
    consumed_hashes: tuple[str, ...] = ()
    output_hash: str = ""
    tokens_prompt: int = 0
    tokens_completion: int = 0


@dataclass(frozen=True, slots=True)
class WorkflowResult:
    workflow_id: str
    workload_id: str
    status: str  # completed | failed | timed_out
    stages: tuple[StageOutput, ...] = ()
    output_hash: str = ""


@dataclass(frozen=True, slots=True)
class FailureRecord:
    workflow_id: str
    step_id: str
    attempt_id: int
    kind: str
    detail: str = ""


@dataclass(frozen=True, slots=True)
class TraceEvent:
    name: str
    workflow_id: str
    timestamp_ns: int
    step_id: str = ""
    request_id: str = ""
    payload: dict[str, str] = field(default_factory=dict)
