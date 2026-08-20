# DDS Security — Deploy real

Este diretório contém scripts para gerar e operar uma PKI DDS Security para
ambientes não confiáveis (rede externa). O modo `local-only` continua
funcionando sem esses artefatos; este diretório é necessário apenas quando se
ativa DDS Security com autenticação, criptografia e controle de acesso.

## Arquivos

- `pki.sh` — gera CA raiz, CA intermediária, identidades e permissions para
cada role do runtime.
- `rotate.sh` — renova identidades e permissions sem recriar as CAs.
- `check-expiry.sh` — alerta sobre certificados que expiram em breve.
- `README.md` — este arquivo.

## Uso rápido

```bash
cd config/dds/security/deploy
./pki.sh --validity-days 90 --output-dir ./certs
```

A saída fica em `./certs/`:

```
certs/
├── ca/
│   ├── root/root_ca_cert.pem          # certificado da CA raiz (offline)
│   └── intermediate/
│       ├── identity_ca_cert.pem       # CA intermediária de identidade
│       ├── identity_ca_key.pem        # chave privada (proteger/vault)
│       ├── permissions_ca_cert.pem    # CA intermediária de permissões
│       └── permissions_ca_key.pem     # chave privada (proteger/vault)
├── identities/
│   ├── orchestrator_{cert,key}.pem
│   ├── agent_{cert,key}.pem
│   └── ...
├── artefacts/
│   ├── governance.p7s
│   ├── permissions_<role>.p7s
│   ├── identity_ca_cert.pem
│   └── permissions_ca_cert.pem
└── manifest.json
```

## Rotação

```bash
./rotate.sh --validity-days 90 --output-dir ./certs
```

Distribua os arquivos renovados (`identities/*`, `artefacts/*`) para os hosts
e reinicie os serviços DDS. O CycloneDDS não suporta hot-reload de
 certificados de participante em runtime.

## Configuração do CycloneDDS

Cada host precisa de um arquivo XML apontando para os artefatos gerados.
Exemplo mínimo (`cyclonedds-secure.xml`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<CycloneDDS xmlns="https://cdds.io/config"
            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
            xsi:schemaLocation="https://cdds.io/config
                                https://raw.githubusercontent.com/eclipse-cyclonedds/cyclonedds/master/etc/cyclonedds.xsd">
  <Domain id="any">
    <General>
      <AllowMulticast>false</AllowMulticast>
      <EnableDDSecurity>
        <Authentication>
          <Library initFunction="init_authentication"
                   finalizeFunction="finalize_authentication"
                   path="dds_security_auth"/>
          <IdentityCertificate>identities/orchestrator_cert.pem</IdentityCertificate>
          <IdentityCA>artefacts/identity_ca_cert.pem</IdentityCA>
          <PrivateKey>identities/orchestrator_key.pem</PrivateKey>
        </Authentication>
        <AccessControl>
          <Library initFunction="init_access_control"
                   finalizeFunction="finalize_access_control"
                   path="dds_security_ac"/>
          <PermissionsCA>artefacts/permissions_ca_cert.pem</PermissionsCA>
          <Governance>artefacts/governance.p7s</Governance>
          <Permissions>artefacts/permissions_orchestrator.p7s</Permissions>
        </AccessControl>
        <Cryptography>
          <Library initFunction="init_crypto"
                   finalizeFunction="finalize_crypto"
                   path="dds_security_crypto"/>
        </Cryptography>
      </EnableDDSecurity>
    </General>
  </Domain>
</CycloneDDS>
```

O runtime lê o caminho do XML via variável de ambiente
`CYCLONEDDS_URI=file:///etc/dds/cyclonedds-secure.xml`.

## Segmentação de rede

DDS Security criptografa e autentica, mas não substitui segmentação de rede.
Recomendações mínimas para deploy externo:

1. **VLAN/isolamento:** coloque os participantes DDS em uma VLAN dedicada,
   separada de tráfego de gerenciamento, HTTP e storage.
2. **Firewall:** permita apenas UDP 7400-7410 (RTPS discovery/data) entre hosts
   autorizados; bloqueie multicast entre VLANs.
3. **Domínios DDS:** use IDs de domínio distintos por ambiente
   (produção/homologação/desenvolvimento).
4. **Descoberta:** desabilite ou restrinja discovery multicast; prefira
   discovery unicast com lista explícita de peers (`<Peers>`).
5. **Monitoramento:** alerte sobre certificados próximos da expiração
   (`./check-expiry.sh`) e sobre handshake DDS falho.
6. **Chaves privadas:** armazene as chaves das CAs e das identidades em
   vault/HSM ou filesystem criptografado; nunca commit em repositório.
