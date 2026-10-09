# Roteiro — validação visual do painel SSH do Studio (descartável)

Evidência-alvo: fluxo pelos CONTROLES da GUI (não por chamadas diretas),
contra servidor SSH descartável em loopback. Delimitação: comprova o fluxo
da interface; não valida operação em VM (T-800-32 cobre a sessão; este
roteiro cobre a interface).

Pré-requisitos: sessão gráfica local, revisão `4c854ad` (ou posterior com
este roteiro), nada nas VMs, nenhuma credencial administrativa.

## 1. Preparar o servidor descartável (terminal 1)

```bash
cd src/rust
export STUDIO_SSH_DIR=/tmp/studio-gui-ssh   # diretório próprio do teste
./scripts/studio-ssh-descartavel.sh start    # imprime porta + impressão
```

Anote porta e impressão SHA256 exibidas. O servidor só escuta em
127.0.0.1, só aceita pubkey, sem senha/PAM/root.

## 2. Abrir o Studio (terminal 2)

```bash
cd src/rust
cargo build --locked --offline -p orchestrator-studio
./target/debug/studio
```

Navegação lateral → **SSH dedicado**.

## 3. Gerar a identidade de teste (na GUI)

1. `diretório:` aponte para `/tmp/studio-gui-ssh/gui` (próprio do teste).
2. Digite uma `senha de desbloqueio` (só em memória).
3. Clique **Gerar identidade** (fase volta a `Idle`; falha aparece sem
   expor a senha).

## 4. Cadastrar a pública no descartável (terminal 1)

```bash
./scripts/studio-ssh-descartavel.sh add-pub /tmp/studio-gui-ssh/gui/studio_ed25519.pub
```

Nenhum `authorized_keys` fora de `/tmp/studio-gui-ssh`.

## 5. Conectar pela GUI

Preencha `host: 127.0.0.1`, `porta:` (a impressa no passo 1),
`usuário:` seu usuário local, `comando: echo PROVA_OK`.
Clique **Conectar e executar** e observe:

- [ ] Bloco **Host desconhecido** exibe `ssh-ed25519` + impressão IGUAL à
      do passo 1 (conferência por fonte independente: o terminal).
- [ ] Janela segue responsiva durante a execução (spinner, sem travar).
- [ ] Clique **Aprovar e salvar no cofre** → fase volta a `Idle`.
- [ ] Clique **Conectar e executar** de novo → bloco **Saída** mostra
      `PROVA_OK` (sem senha/flag exposta na tela).

## 6. Erro sem vazar credencial (na GUI)

Troque o comando para algo inexistente ou pare o servidor (`stop` no
terminal 1) e reconecte: a falha aparece como texto simples, sem senha,
sem caminho de chave privada, sem token.

## 7. Capturas e encerramento

Capture a tela do bloco de aprovação (com a impressão) e do bloco de
saída; confira que nenhuma credencial aparece antes de arquivar.
Encerre o Studio e rode `./scripts/studio-ssh-descartavel.sh stop`;
apague `/tmp/studio-gui-ssh` se desejar (cofre do teste vai junto).

## Impedimento conhecido

Execução automatizada do clique não é possível neste contexto (Wayland/KDE
sem injetor de entrada; nenhuma ferramenta instalada para isso). Este
roteiro é o caminho reproduzível para execução local assistida.
