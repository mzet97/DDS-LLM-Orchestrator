#!/usr/bin/env bash
# Rotação de certificados DDS Security.
#
# Renova as identidades e permissões de todos os roles sem recriar as CAs.
# A CA intermediária deve ainda estar no diretório de deploy (ou ser
# restaurada de um vault seguro); a CA raiz deve permanecer offline.
#
# Uso:
#   ./rotate.sh [--validity-days N] [--output-dir ./certs]
#
# Processo recomendado de rotação em produção:
#   1. Gerar novos certificados com este script em uma estação segura.
#   2. Copiar apenas identidades/permissions/governance renovadas para os hosts.
#   3. Reiniciar os serviços DDS em janela de manutenção (CycloneDDS não
#      suporta hot-reload de certificados de participante em runtime).
#   4. Validar handshake e sample exchange com o smoke de deploy seguro.

set -euo pipefail

VALIDITY_DAYS="${DDS_SECURITY_VALIDITY_DAYS:-90}"
OUTPUT_DIR="${DDS_SECURITY_OUTPUT_DIR:-./certs}"
ORG="${DDS_SECURITY_CN_O:-DDS-LLM-Orchestrator}"
COUNTRY="${DDS_SECURITY_CN_C:-BR}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --validity-days)
            VALIDITY_DAYS="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "Uso: $0 [--validity-days N] [--output-dir ./certs]" >&2
            exit 1
            ;;
    esac
done

INTERMEDIATE_DIR="${OUTPUT_DIR}/ca/intermediate"
IDENTITIES_DIR="${OUTPUT_DIR}/identities"
ARTEFACTS_DIR="${OUTPUT_DIR}/artefacts"

fail() { printf 'ERROR: %s\n' "$1" >&2; exit 1; }

[[ -f "${INTERMEDIATE_DIR}/identity_ca_key.pem" ]] || fail "CA intermediária de identidade não encontrada em ${INTERMEDIATE_DIR}"
[[ -f "${INTERMEDIATE_DIR}/permissions_ca_key.pem" ]] || fail "CA intermediária de permissões não encontrada em ${INTERMEDIATE_DIR}"

subject() {
    local cn="$1"
    echo "/C=${COUNTRY}/O=${ORG}/CN=${cn}"
}

rotate_identity() {
    local role="$1"
    local cn="$2"
    local key="${IDENTITIES_DIR}/${role}_key.pem"
    local cert="${IDENTITIES_DIR}/${role}_cert.pem"
    local csr="${IDENTITIES_DIR}/${role}_csr.pem"

    openssl req -newkey rsa:2048 \
        -keyout "${key}" -out "${csr}" \
        -nodes -subj "$(subject "${cn}")" 2>/dev/null
    openssl x509 -req -in "${csr}" \
        -CA "${INTERMEDIATE_DIR}/identity_ca_cert.pem" \
        -CAkey "${INTERMEDIATE_DIR}/identity_ca_key.pem" \
        -CAcreateserial -out "${cert}" -days "${VALIDITY_DAYS}" 2>/dev/null
    rm -f "${csr}"
    chmod 600 "${key}"
    echo "[ROTATE] Identidade ${role} renovada."
}

rotate_permissions() {
    local role="$1"
    local cn="$2"
    local xml="${ARTEFACTS_DIR}/permissions_${role}.xml"
    local p7s="${ARTEFACTS_DIR}/permissions_${role}.p7s"

    cat > "${xml}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="https://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_permissions.xsd">
  <permissions>
    <grant name="${cn}Grant">
      <subject_name>CN=${cn}, O=${ORG}, C=${COUNTRY}</subject_name>
      <validity>
        <not_before>$(date -u +%Y-%m-%dT%H:%M:%S)</not_before>
        <not_after>$(date -u -d "+${VALIDITY_DAYS} days" +%Y-%m-%dT%H:%M:%S)</not_after>
      </validity>
      <allow_rule>
        <domains>
          <id_range><min>0</min><max>230</max></id_range>
        </domains>
        <publish>
          <topics><topic>*</topic></topics>
        </publish>
        <subscribe>
          <topics><topic>*</topic></topics>
        </subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
EOF

    openssl smime -sign -nodetach -outform PEM \
        -in "${xml}" -out "${p7s}" \
        -signer "${INTERMEDIATE_DIR}/permissions_ca_cert.pem" \
        -inkey "${INTERMEDIATE_DIR}/permissions_ca_key.pem" 2>/dev/null
    echo "[ROTATE] Permissions ${role} renovadas."
}

rotate_governance() {
    local governance_xml="${ARTEFACTS_DIR}/governance.xml"
    local governance_p7s="${ARTEFACTS_DIR}/governance.p7s"

    # Reutiliza o governance.xml existente; apenas ressina.
    [[ -f "${governance_xml}" ]] || fail "governance.xml não encontrado"
    openssl smime -sign -nodetach -outform PEM \
        -in "${governance_xml}" -out "${governance_p7s}" \
        -signer "${INTERMEDIATE_DIR}/permissions_ca_cert.pem" \
        -inkey "${INTERMEDIATE_DIR}/permissions_ca_key.pem" 2>/dev/null
    echo "[ROTATE] Governance reassinada."
}

ROLES=(
    "orchestrator:Orchestrator"
    "agent:Agent"
    "client:Client"
    "mcp-gateway:MCP Gateway"
    "context-store:Context Store"
    "policy-engine:Policy Engine"
    "observability:Observability"
)

echo "[ROTATE] Iniciando rotação (validade ${VALIDITY_DAYS} dias)..."

for entry in "${ROLES[@]}"; do
    role="${entry%%:*}"
    cn="${entry##*:}"
    rotate_identity "${role}" "${cn}"
    rotate_permissions "${role}" "${cn}"
done

rotate_governance

# Atualiza manifesto
if [[ -f "${OUTPUT_DIR}/manifest.json" ]]; then
    jq --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '.rotated_at = $d' "${OUTPUT_DIR}/manifest.json" > "${OUTPUT_DIR}/manifest.json.tmp" && \
        mv "${OUTPUT_DIR}/manifest.json.tmp" "${OUTPUT_DIR}/manifest.json"
fi

echo "[ROTATE] Done. Distribua identidades/permissions/governance para os hosts e reinicie os serviços."
