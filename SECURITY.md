# Segurança do DDS-LLM-Orchestrator

## Status

O runtime em `v0.1.0` possui dois modos de operação DDS:

1. **Local-only / rede confiável** (default): discovery e dados em claro.
   Indicado para loopback, containers em single host ou VLANs estritamente
   controladas.
2. **DDS Security** (externo): autenticação mútua, criptografia e controle de
   acesso via CycloneDDS Security plugins. Requer PKI própria, rotação de
   certificados e segmentação de rede.

## Modo local-only

```bash
# Default do orchestrator/agent; não requer configuração extra.
python -m src.orchestrator.orchestrator.main --dds-domain 0
```

Este modo **não deve ser anunciado como seguro externamente**.

## Modo DDS Security externo

### 1. Gerar PKI

```bash
cd config/dds/security/deploy
./pki.sh --validity-days 90 --output-dir ./certs
```

Armazene as chaves privadas das CAs offline (remova
`certs/ca/root/root_ca_key.pem` do host de build).

### 2. Distribuir artefatos

Cada host recebe:

- `certs/identities/<role>_cert.pem`
- `certs/identities/<role>_key.pem`
- `certs/artefacts/identity_ca_cert.pem`
- `certs/artefacts/permissions_ca_cert.pem`
- `certs/artefacts/governance.p7s`
- `certs/artefacts/permissions_<role>.p7s`
- `cyclonedds-secure.xml` (apontando para os campos acima)

### 3. Segmentação de rede

- VLAN dedicada para tráfego DDS.
- Firewall bloqueando UDP RTPS entre hosts não autorizados.
- Discovery unicast com peer list explícita; multicast desabilitado entre VLANs.
- Domínios DDS distintos por ambiente.

### 4. Rotação

```bash
./rotate.sh --validity-days 90 --output-dir ./certs
./check-expiry.sh --output-dir ./certs --warn-days 30
```

Reinicie os serviços após a distribuição dos novos artefatos.

## Reportar vulnerabilidades

Envie detalhes para matheus.zeitune.developer@gmail.com.
