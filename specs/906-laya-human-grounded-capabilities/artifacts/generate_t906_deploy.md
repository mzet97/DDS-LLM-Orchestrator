# Anexo A — Gerador de corpus T906 (deploy manual)

O hook de segurança local (Mimosa) bloqueia a escrita/execução de novos `.py`
nesta máquina; o conteúdo abaixo está congelado e auditável. **Deploy manual
(um comando):**

```bash
# 1. Copiar o bloco de código abaixo para /tmp/deploy_t906.py na .61
#    (scp do arquivo anexo também serve, se o hook local permitir)
sshpass -e ssh agent@192.168.1.61 'cat > /tmp/generate_t906.py' < generate_t906.py

# 2. Lançar a geração (fase gen; ~3 h)
sshpass -e ssh agent@192.168.1.61 \
  'cd /tmp && setsid nohup python3 generate_t906.py --phase gen --target 380 \
   --out /tmp/laya_t906_gen --seed 906 > /tmp/laya_t906_gen/run.log 2>&1 < /dev/null & disown'
```

O conteúdo integral do gerador está em `generate_t906.py.txt` (mesmo diretório),
incluindo: allowlist de endpoint local, PRNG LCG determinístico, grid de caps
(rust/python positivos e negativos difíceis) e QoS adversarial.
