#!/usr/bin/env bash
# Verifica certificados DDS Security próximos da expiração.
#
# Uso:
#   ./check-expiry.sh [--output-dir ./certs] [--warn-days 30]
#
# Saída: lista certificados que expiram em <= N dias; exit code 1 se houver
# algum. Útil para cron/Prometheus node-exporter textfile.

set -euo pipefail

OUTPUT_DIR="${DDS_SECURITY_OUTPUT_DIR:-./certs}"
WARN_DAYS="${DDS_SECURITY_WARN_DAYS:-30}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --warn-days)
            WARN_DAYS="$2"
            shift 2
            ;;
        *)
            echo "Uso: $0 [--output-dir ./certs] [--warn-days 30]" >&2
            exit 1
            ;;
    esac
done

NOW_EPOCH=$(date +%s)
WARN_EPOCH=$((NOW_EPOCH + WARN_DAYS * 86400))
EXPIRING=0

check_file() {
    local file="$1"
    local not_after
    not_after=$(openssl x509 -in "${file}" -noout -enddate 2>/dev/null | cut -d= -f2)
    if [[ -z "${not_after}" ]]; then
        echo "WARN: não foi possível ler ${file}"
        return
    fi
    local expiry_epoch
    expiry_epoch=$(date -d "${not_after}" +%s)
    local remaining_days=$(( (expiry_epoch - NOW_EPOCH) / 86400 ))
    if [[ ${expiry_epoch} -le ${WARN_EPOCH} ]]; then
        echo "EXPIRING: ${file} expires in ${remaining_days} days (${not_after})"
        EXPIRING=1
    else
        echo "OK: ${file} expires in ${remaining_days} days"
    fi
}

mapfile -t FILES < <(find "${OUTPUT_DIR}" -type f -name '*.pem' ! -name '*_key.pem')
for f in "${FILES[@]}"; do
    check_file "${f}"
done

if [[ ${EXPIRING} -ne 0 ]]; then
    echo "ERROR: há certificados expirando em ${WARN_DAYS} dias ou menos." >&2
    exit 1
fi

echo "All certificates valid for more than ${WARN_DAYS} days."
