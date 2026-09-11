#!/usr/bin/env bash
# DDS Security PKI para deploy real.
#
# Gera uma cadeia de certificados de produção (CA raiz offline + CA intermediária
# de operação), identidades para cada role do runtime e documentos de governance
# e permissions assinados. As chaves privadas da CA raiz NÃO são copiadas para
# o diretório de deploy; elas devem ser armazenadas offline (HSM/vault).
#
# Uso:
#   cd config/dds/security/deploy
#   ./pki.sh [--validity-days N] [--output-dir ./certs]
#
# Variáveis de ambiente:
#   DDS_SECURITY_VALIDITY_DAYS  - dias de validade dos certificados (default: 90)
#   DDS_SECURITY_OUTPUT_DIR     - diretório de saída (default: ./certs)
#   DDS_SECURITY_CN_O           - organização no DN (default: DDS-LLM-Orchestrator)
#   DDS_SECURITY_CN_C           - país no DN (default: BR)

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

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_CA_DIR="${OUTPUT_DIR}/ca/root"
INTERMEDIATE_DIR="${OUTPUT_DIR}/ca/intermediate"
IDENTITIES_DIR="${OUTPUT_DIR}/identities"
ARTEFACTS_DIR="${OUTPUT_DIR}/artefacts"

mkdir -p "${ROOT_CA_DIR}" "${INTERMEDIATE_DIR}" "${IDENTITIES_DIR}" "${ARTEFACTS_DIR}"

# ---- helpers ----
fail() { printf 'ERROR: %s\n' "$1" >&2; exit 1; }

require_openssl() {
    command -v openssl >/dev/null 2>&1 || fail "openssl não encontrado"
}

subject() {
    local cn="$1"
    echo "/C=${COUNTRY}/O=${ORG}/CN=${cn}"
}

# ---- CA raiz (offline) ----
# Em produção a chave raiz deve ser gerada em um HSM/vault e NUNCA persistida
# neste diretório. Aqui geramos um exemplo; copie apenas o certificado para
# o diretório de deploy e remova a chave privada do disco operacional.
generate_root_ca() {
    if [[ ! -f "${ROOT_CA_DIR}/root_ca_cert.pem" ]]; then
        openssl req -x509 -newkey rsa:4096 \
            -keyout "${ROOT_CA_DIR}/root_ca_key.pem" \
            -out "${ROOT_CA_DIR}/root_ca_cert.pem" \
            -days 3650 -nodes \
            -subj "$(subject "DDS Security Root CA")" \
            2>/dev/null
        chmod 600 "${ROOT_CA_DIR}/root_ca_key.pem"
        echo "[PKI] Root CA gerada. REMOVA ${ROOT_CA_DIR}/root_ca_key.pem} para armazenamento offline."
    fi
}

# ---- CA intermediária (identidade + permissões) ----
generate_intermediate_ca() {
    local name="$1"
    local cn="$2"
    local key="${INTERMEDIATE_DIR}/${name}_key.pem"
    local cert="${INTERMEDIATE_DIR}/${name}_cert.pem"
    local csr="${INTERMEDIATE_DIR}/${name}_csr.pem"

    if [[ -f "${cert}" ]]; then
        echo "[PKI] Intermediate CA ${name} já existe, pulando."
        return
    fi

    openssl req -newkey rsa:3072 \
        -keyout "${key}" -out "${csr}" \
        -nodes -subj "$(subject "${cn}")" 2>/dev/null
    openssl x509 -req -in "${csr}" \
        -CA "${ROOT_CA_DIR}/root_ca_cert.pem" \
        -CAkey "${ROOT_CA_DIR}/root_ca_key.pem" \
        -CAcreateserial -out "${cert}" -days "$((VALIDITY_DAYS * 3))" 2>/dev/null
    rm -f "${csr}"
    chmod 600 "${key}"
    echo "[PKI] Intermediate CA ${name} gerada."
}

# ---- Identidade de participante ----
generate_identity() {
    local role="$1"
    local cn="$2"
    local key="${IDENTITIES_DIR}/${role}_key.pem"
    local cert="${IDENTITIES_DIR}/${role}_cert.pem"
    local csr="${IDENTITIES_DIR}/${role}_csr.pem"

    if [[ -f "${cert}" ]]; then
        echo "[PKI] Identidade ${role} já existe, pulando."
        return
    fi

    openssl req -newkey rsa:2048 \
        -keyout "${key}" -out "${csr}" \
        -nodes -subj "$(subject "${cn}")" 2>/dev/null
    openssl x509 -req -in "${csr}" \
        -CA "${INTERMEDIATE_DIR}/identity_ca_cert.pem" \
        -CAkey "${INTERMEDIATE_DIR}/identity_ca_key.pem" \
        -CAcreateserial -out "${cert}" -days "${VALIDITY_DAYS}" 2>/dev/null
    rm -f "${csr}"
    chmod 600 "${key}"
    echo "[PKI] Identidade ${role} gerada (validade ${VALIDITY_DAYS} dias)."
}

# ---- Governance ----
generate_governance() {
    local governance_xml="${ARTEFACTS_DIR}/governance.xml"
    local governance_p7s="${ARTEFACTS_DIR}/governance.p7s"

    cat > "${governance_xml}" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="https://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_governance.xsd">
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id_range>
          <min>0</min>
          <max>230</max>
        </id_range>
      </domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>true</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
      <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>true</enable_discovery_protection>
          <enable_liveliness_protection>true</enable_liveliness_protection>
          <enable_read_access_control>true</enable_read_access_control>
          <enable_write_access_control>true</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
EOF

    openssl smime -sign -nodetach -outform PEM \
        -in "${governance_xml}" -out "${governance_p7s}" \
        -signer "${INTERMEDIATE_DIR}/permissions_ca_cert.pem" \
        -inkey "${INTERMEDIATE_DIR}/permissions_ca_key.pem" 2>/dev/null
    echo "[PKI] Governance assinada."
}

# ---- Permissions por role ----
generate_permissions() {
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
    echo "[PKI] Permissions ${role} assinadas."
}

# ---- main ----
require_openssl

echo "[PKI] Gerando PKI em ${OUTPUT_DIR} (validade ${VALIDITY_DAYS} dias)..."

generate_root_ca
generate_intermediate_ca identity_ca "DDS Security Identity Intermediate CA"
generate_intermediate_ca permissions_ca "DDS Security Permissions Intermediate CA"

# Roles do runtime. Cada binário em produção deve usar sua própria identidade.
ROLES=(
    "orchestrator:Orchestrator"
    "agent:Agent"
    "client:Client"
    "mcp-gateway:MCP Gateway"
    "context-store:Context Store"
    "policy-engine:Policy Engine"
    "observability:Observability"
)

for entry in "${ROLES[@]}"; do
    role="${entry%%:*}"
    cn="${entry##*:}"
    generate_identity "${role}" "${cn}"
    generate_permissions "${role}" "${cn}"
done

generate_governance

# ---- bundle de confiança ----
cp "${INTERMEDIATE_DIR}/identity_ca_cert.pem" "${ARTEFACTS_DIR}/identity_ca_cert.pem"
cp "${INTERMEDIATE_DIR}/permissions_ca_cert.pem" "${ARTEFACTS_DIR}/permissions_ca_cert.pem"

# ---- manifesto ----
cat > "${OUTPUT_DIR}/manifest.json" <<EOF
{
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "validity_days": ${VALIDITY_DAYS},
  "roles": [$(printf '\n    "%s"' "${ROLES[@]//:*}" | sed 's/^    "/,\n    "/' | tail -n +2)
  ],
  "warning": "As chaves privadas das CAs devem ser removidas deste disco e armazenadas offline/HSM.",
  "artefacts": {
    "governance": "${ARTEFACTS_DIR}/governance.p7s",
    "identity_ca": "${ARTEFACTS_DIR}/identity_ca_cert.pem",
    "permissions_ca": "${ARTEFACTS_DIR}/permissions_ca_cert.pem"
  }
}
EOF

echo "[PKI] Done. Revisar ${OUTPUT_DIR}/manifest.json"
