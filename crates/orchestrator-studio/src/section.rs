//! Seção exibida no painel central e grupos da navegação lateral.

/// Seção exibida no painel central (navegação lateral exigida no §30).
/// Sem "Catálogo" local (T-830-04): a leitura do catálogo vive em
/// "Catálogo compartilhado" (autoridade no nó via `catalog_remote`).
/// "Máquinas" (REQ/T-840-03) registra nós remotos no MESMO catálogo
/// compartilhado e sonda cada um via `GET /version`.
/// Grupos da navegação (design Stitch: 5 grupos numerados + tag à direita).
pub(crate) const NAV_GROUPS: &[(&str, &str, &[Section])] = &[
    (
        "1. Sistema",
        "DDS",
        &[Section::Overview, Section::Topology, Section::Logs],
    ),
    (
        "2. Nós & Infraestrutura",
        "HOST",
        &[
            Section::Machines,
            Section::Node,
            Section::Services,
            Section::Shared,
            Section::Ssh,
        ],
    ),
    (
        "3. Enxame IA & Inferência",
        "AI",
        &[
            Section::Agents,
            Section::Inference,
            Section::Launch,
            Section::Workflow,
            #[cfg(feature = "dds")]
            Section::Tools,
        ],
    ),
    ("4. Artefatos", "BIN", &[Section::Models]),
    ("5. Diagnóstico", "OPS", &[Section::Dispatch]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Overview,
    Node,
    Inference,
    Launch,
    Agents,
    Dispatch,
    Models,
    Services,
    Shared,
    Machines,
    Ssh,
    Topology,
    Workflow,
    #[cfg(feature = "dds")]
    Tools,
    Logs,
}

impl Section {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão Geral",
            Self::Node => "Nó studio-node",
            Self::Inference => "Inferência",
            Self::Launch => "Subir Inferência",
            Self::Agents => "Agentes",
            Self::Dispatch => "Despacho",
            Self::Models => "Modelos GGUF",
            Self::Services => "Serviços",
            Self::Shared => "Catálogo Compartilhado",
            Self::Machines => "Máquinas",
            Self::Ssh => "SSH Dedicado",
            Self::Topology => "Topologia DDS",
            Self::Workflow => "Workflow",
            #[cfg(feature = "dds")]
            Self::Tools => "Ferramentas & Tool Calls",
            Self::Logs => "Logs da GUI",
        }
    }

    /// Numeração do mockup (sidebar "3.1 … 3.14").
    pub(crate) fn num(self) -> &'static str {
        match self {
            Self::Overview => "3.1",
            Self::Node => "3.2",
            Self::Inference => "3.3",
            Self::Launch => "3.4",
            Self::Agents => "3.5",
            Self::Dispatch => "3.6",
            Self::Models => "3.7",
            Self::Services => "3.8",
            Self::Shared => "3.9",
            Self::Machines => "3.10",
            Self::Ssh => "3.15",
            Self::Topology => "3.11",
            Self::Workflow => "3.12",
            #[cfg(feature = "dds")]
            Self::Tools => "3.13",
            Self::Logs => "3.14",
        }
    }

    /// Tela pela numeração do mockup ("3.1".."3.14") — `STUDIO_SCREEN`
    /// (capturas de verificação/testes manuais). Fora da lista → `None`.
    pub(crate) fn from_num(num: &str) -> Option<Self> {
        Some(match num.trim() {
            "3.1" => Self::Overview,
            "3.2" => Self::Node,
            "3.3" => Self::Inference,
            "3.4" => Self::Launch,
            "3.5" => Self::Agents,
            "3.6" => Self::Dispatch,
            "3.7" => Self::Models,
            "3.8" => Self::Services,
            "3.9" => Self::Shared,
            "3.10" => Self::Machines,
            "3.15" => Self::Ssh,
            "3.11" => Self::Topology,
            "3.12" => Self::Workflow,
            #[cfg(feature = "dds")]
            "3.13" => Self::Tools,
            "3.14" => Self::Logs,
            _ => return None,
        })
    }

    /// Tag curta mono da sidebar (design Stitch: DOM/MESH/500/AUTO/…).
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::Overview => "DOM",
            Self::Node => "PROT",
            Self::Inference => "LLAMA",
            Self::Launch => "RUN",
            Self::Agents => "HB",
            Self::Dispatch => ":8080",
            Self::Models => "SHA256",
            Self::Services => "SYSTEMD",
            Self::Shared => "REV409",
            Self::Machines => "AUTO",
            Self::Ssh => "SSH",
            Self::Topology => "MESH",
            Self::Workflow => "DAG",
            #[cfg(feature = "dds")]
            Self::Tools => "SEC",
            Self::Logs => "500",
        }
    }
}
