//! Protocolo administrativo versionado (§31: versão, schema e compatibilidade).
//!
//! O nó anuncia [`NODE_PROTOCOL_VERSION`]; par incompatível é bloqueado com
//! erro tipado — nunca escondido atrás de retry.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Versão do protocolo administrativo suportada por este nó.
pub const NODE_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

/// Versão `major.minor` do protocolo administrativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

/// Chave de idempotência de uma operação administrativa (G-06/G-50).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub String);

/// Operação administrativa sobre recursos próprios do nó (RF-05/06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdminOp {
    /// Bootstrap do nó; repetição não duplica (G-05).
    Bootstrap { node_name: String },
    /// Liga/desliga um serviço próprio do nó.
    SetService { service: String, running: bool },
}

/// Envelope que atravessa o fio: versão do emissor + operação idempotente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminEnvelope {
    pub protocol: ProtocolVersion,
    pub operation_id: OperationId,
    pub op: AdminOp,
}

/// Erros do protocolo (fronteira: aqui nasce o erro tipado).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    /// Par com `major` distinto, ou mais novo que o nó: aplicar seria incorreto.
    #[error("protocolo incompativel: no={node:?} par={peer:?}")]
    Incompatible {
        node: ProtocolVersion,
        peer: ProtocolVersion,
    },
}

impl ProtocolVersion {
    /// Compatibilidade: mesmo `major` e par não mais novo que o nó.
    /// `minor` maior no nó aceita par mais antigo (evolução compatível).
    #[must_use]
    pub const fn accepts(&self, peer: ProtocolVersion) -> bool {
        self.major == peer.major && peer.minor <= self.minor
    }

    /// Erro tipado quando [`accepts`](Self::accepts) nega.
    pub const fn check(&self, peer: ProtocolVersion) -> Result<(), ProtocolError> {
        if self.accepts(peer) {
            Ok(())
        } else {
            Err(ProtocolError::Incompatible { node: *self, peer })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_same_version() {
        assert!(NODE_PROTOCOL_VERSION.accepts(NODE_PROTOCOL_VERSION));
        assert!(NODE_PROTOCOL_VERSION.check(NODE_PROTOCOL_VERSION).is_ok());
    }

    #[test]
    fn node_newer_accepts_older_peer_on_same_major() {
        let node = ProtocolVersion { major: 1, minor: 2 };
        let peer = ProtocolVersion { major: 1, minor: 0 };

        assert!(node.accepts(peer));
    }

    #[test]
    fn refuses_newer_peer_and_distinct_major() {
        let node = NODE_PROTOCOL_VERSION;

        let newer = ProtocolVersion { major: 1, minor: 1 };
        assert!(!node.accepts(newer));
        let err = node.check(newer).expect_err("par mais novo deve falhar");
        assert_eq!(err, ProtocolError::Incompatible { node, peer: newer });

        let other_major = ProtocolVersion { major: 2, minor: 0 };
        assert!(!node.accepts(other_major));
        assert!(node.check(other_major).is_err());
    }

    #[test]
    fn envelope_survives_json_wire() {
        let envelope = AdminEnvelope {
            protocol: NODE_PROTOCOL_VERSION,
            operation_id: OperationId(String::from("op-1")),
            op: AdminOp::SetService {
                service: String::from("dds-agent"),
                running: true,
            },
        };

        let wire = serde_json::to_string(&envelope).expect("envelope deve serializar para o fio");
        let back: AdminEnvelope =
            serde_json::from_str(&wire).expect("fio deve desserializar no mesmo schema");

        assert_eq!(back, envelope);
    }
}
