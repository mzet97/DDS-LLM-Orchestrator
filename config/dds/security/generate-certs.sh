#!/usr/bin/env bash
# Generate DDS Security artefacts for local-only smoke tests.
#
# Usage:
#   cd config/dds/security
#   ./generate-certs.sh
#
# Produces:
#   - Identity CA (identity_ca_cert.pem, identity_ca_key.pem)
#   - Permissions CA (permissions_ca_cert.pem, permissions_ca_key.pem)
#   - Identities for publisher, subscriber and intruder
#   - Governance document + P7S signature
#   - Permissions documents + P7S signatures for each role
#
# Requires: openssl

set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${DIR}"

echo "Generating DDS Security certificates in $(pwd)..."

# ---- Identity CA ----
openssl req -x509 -newkey rsa:2048 \
    -keyout identity_ca_key.pem -out identity_ca_cert.pem \
    -days 365 -nodes \
    -subj "/C=BR/O=DDS-LLM-Orchestrator/CN=Identity CA" \
    2>/dev/null

# ---- Permissions CA ----
openssl req -x509 -newkey rsa:2048 \
    -keyout permissions_ca_key.pem -out permissions_ca_cert.pem \
    -days 365 -nodes \
    -subj "/C=BR/O=DDS-LLM-Orchestrator/CN=Permissions CA" \
    2>/dev/null

# Helper to create a participant identity signed by the identity CA.
generate_identity() {
    local name="$1"
    local cn="$2"
    openssl req -newkey rsa:2048 \
        -keyout "${name}_key.pem" -out "${name}_req.pem" \
        -days 365 -nodes \
        -subj "/C=BR/O=DDS-LLM-Orchestrator/CN=${cn}" \
        2>/dev/null
    openssl x509 -req -in "${name}_req.pem" \
        -CA identity_ca_cert.pem -CAkey identity_ca_key.pem \
        -CAcreateserial -out "${name}_cert.pem" -days 365 \
        2>/dev/null
    rm -f "${name}_req.pem"
}

# ---- Identities ----
generate_identity publisher Publisher
generate_identity subscriber Subscriber

# ---- Intruder identity (self-signed, not trusted by identity CA) ----
openssl req -x509 -newkey rsa:2048 \
    -keyout intruder_key.pem -out intruder_cert.pem \
    -days 365 -nodes \
    -subj "/C=BR/O=DDS-LLM-Orchestrator/CN=Intruder" \
    2>/dev/null

# ---- Governance document ----
cat > governance.xml <<'EOF'
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

# Sign governance with permissions CA.
openssl smime -sign -nodetach -outform PEM \
    -in governance.xml -out governance.p7s \
    -signer permissions_ca_cert.pem -inkey permissions_ca_key.pem \
    2>/dev/null

# Helper to create and sign a permissions document.
generate_permissions() {
    local role="$1"
    local cn="$2"
    local body="$3"

    cat > "permissions_${role}.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="https://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_permissions.xsd">
  <permissions>
    <grant name="${cn}Grant">
      <subject_name>CN=${cn}, O=DDS-LLM-Orchestrator, C=BR</subject_name>
      <validity>
        <not_before>2026-01-01T00:00:00</not_before>
        <not_after>2027-01-01T00:00:00</not_after>
      </validity>${body}
    </grant>
  </permissions>
</dds>
EOF

    openssl smime -sign -nodetach -outform PEM \
        -in "permissions_${role}.xml" -out "permissions_${role}.p7s" \
        -signer permissions_ca_cert.pem -inkey permissions_ca_key.pem \
        2>/dev/null
}

ALLOW_ALL='
      <allow_rule>
        <domains>
          <id_range>
            <min>0</min>
            <max>230</max>
          </id_range>
        </domains>
        <publish>
          <topics>
            <topic>*</topic>
          </topics>
        </publish>
        <subscribe>
          <topics>
            <topic>*</topic>
          </topics>
        </subscribe>
      </allow_rule>
      <default>DENY</default>'

DENY_ALL='
      <default>DENY</default>'

# ---- Permissions ----
# Publisher and subscriber are trusted participants with full access in the
# local-only smoke test. The intruder is denied at the identity CA level, but
# its permissions document is also set to deny everything for defence in depth.
generate_permissions publisher Publisher "${ALLOW_ALL}"
generate_permissions subscriber Subscriber "${ALLOW_ALL}"
generate_permissions intruder Intruder "${DENY_ALL}"

# ---- Clean up ----
rm -f identity_ca_key.pem permissions_ca_key.pem *.srl

echo "Done. Files in ${DIR}:"
ls -la *.pem *.p7s *.xml *.sh 2>/dev/null || true
