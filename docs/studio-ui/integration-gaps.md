# Studio UI — integration gaps (lacunas, não implementação disfarçada)

Formato por lacuna: tela/ação → entrada preparada → dado externo
necessário → fallback da UI → evidência com simulação.

| # | Tela/ação | Entrada da UI | Dado externo necessário | Fallback |
|---|---|---|---|---|
| G-INT-01 | T03 adicionar/reconectar máquina | alias, endpoint, credencial ref. | Porta de adoção/inventário (inexistente) | `CapabilityNotice`: sem serviço de implantação conectado; intenções registradas, nada executado |
| G-INT-02 | T05 publicar agente | definição + revisão-base | Aceite versionado do projeto (parcial: catálogo local) | Rascunho local + diff; publicar desabilitado com motivo ou preview simulado rotulado |
| G-INT-03 | T06 testar ferramenta | args validados, gateway, destino | Executor por tipo (só tipos suportados executam) | "Requer implementação de executor"; sem shell/MCP improvisado |
| G-INT-04 | T07 transferência de GGUF | origem, destino, artefato | Serviço de transferência/SSH gerenciado | Botão explica ausência; copiar caminho ≠ transferir |
| G-INT-05 | T08 progresso de carga do modelo | config do servidor | Progresso/identidade do artefato observado (adapter só informa processo iniciado) | Estado indeterminado; nunca "pronto para DDS" só por porta HTTP |
| G-INT-06 | T09 aplicar config do monitor | definição, host, domínio | Aplicação remota versionada | Revisão local + intenção registrada |
| G-INT-07 | T10 executar workflow | solicitação completa | Runner/serviço executor existente | Encaminhar somente se porta existir; senão preview rotulado |
| G-INT-08 | T11 cancelar operação | operation_id | Capacidade de cancelamento do contrato | "Parar de acompanhar" com texto explicativo |
| G-INT-09 | T02 métricas de atenção | — | Telemetria com população/intervalo | "Não medido", nunca zero inventado |
| G-INT-10 | J01/J05 catálogo compartilhado vivo | projeto conhecido | Snapshot/eventos autorizados + revisão remota | Preview com provedor de teste; "Criado em outra GUI" preserva identidade |

Regra: adapter ao vivo nunca retorna fixture em falha; nunca migrar
silenciosamente de conexão real para demonstração.
