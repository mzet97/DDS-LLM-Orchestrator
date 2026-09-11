#include "OrchestratorV4.h"

static_assert(dds_llm_orchestrator_MS_TEXT == 0);
static_assert(dds_llm_orchestrator_MS_VISION == 1);
static_assert(dds_llm_orchestrator_MS_EMBEDDING == 2);
static_assert(dds_llm_orchestrator_MS_TRANSCRIPTION == 3);

int main() {
  return dds_llm_orchestrator_MS_TRANSCRIPTION == 3 ? 0 : 1;
}
