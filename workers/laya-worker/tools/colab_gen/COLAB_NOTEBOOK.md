# Notebook Colab interativo — geração complementar do dataset (opcional)

O CLI `colab` **perde a alocação de GPU** quando não há browser conectado à
sessão (mesmo com o kernel ocupado). Para usar o Colab de forma confiável,
abra um notebook INTERATIVO (browser) e cole a célula abaixo — ela gera um
chunk e salva no Drive, que você baixa depois.

## Célula única (copiar e colar num notebook Colab com GPU T4/L4/A100)

```python
# T-900-A — geração de chunk do dataset de calibração no Colab
# 1. monta Drive  2. baixa modelo (verifica pin)  3. sobe llama-server
# 4. gera N exemplos  5. salva em /content/drive/MyDrive/tese-calib/
CELL = """
import os, sys, json, re, time, random, hashlib, subprocess, urllib.request
from pathlib import Path

# --- config ---
N = 1500        # exemplos deste chunk
SEED = 201      # MUDE por chunk (202, 203, ...)
EXPECTED_SHA = "90fd944d227e9d9b68e7e2c7d5b57b79d4c66ed521b0919fbbd932cf834f6f8e"
GGUF_URL = "https://huggingface.co/unsloth/gemma-4-12b-it-GGUF/resolve/main/gemma-4-12b-it-UD-Q4_K_XL.gguf"
LLAMA_URL = "https://github.com/ggml-org/llama.cpp/releases/download/b11671/llama-b11671-bin-ubuntu-cuda-12.8-x64.tar.gz"
CUDART_URL = "https://github.com/ggml-org/llama.cpp/releases/download/b11671/cudart-llama-b11671-bin-ubuntu-cuda-12.8-x64.tar.gz"

from google.colab import drive
drive.mount('/content/drive')
OUT = Path('/content/drive/MyDrive/tese-calib'); OUT.mkdir(parents=True, exist_ok=True)
BASE = Path('/content/colab_gen'); BASE.mkdir(exist_ok=True)

# --- modelo + pin ---
gguf = BASE / 'models/gemma-4-12b-it-UD-Q4_K_XL.gguf'
if not gguf.exists():
    gguf.parent.mkdir(exist_ok=True)
    print('baixando modelo…'); import shutil
    subprocess.run(['curl','-sL','--retry','3','-o',str(gguf)+'.part',GGUF_URL],check=True)
    gguf.rename(str(gguf)+'.tmp'); (BASE/'models').mkdir(exist_ok=True); gguf = BASE/'models/gemma-4-12b-it-UD-Q4_K_XL.gguf'
    import os; os.rename(str(gguf)+'.tmp', gguf)
got = hashlib.sha256(gguf.read_bytes()).hexdigest()
assert got == EXPECTED_SHA, f'PIN FAIL: {got}'
print('pin OK')

# --- llama-server CUDA ---
llm = BASE/'llm'
if not (llm/'llama-b11671/llama-server').exists():
    llm.mkdir(exist_ok=True)
    for url,name in [(LLAMA_URL,'llama.tar.gz'),(CUDART_URL,'cudart.tar.gz')]:
        subprocess.run(['curl','-sL','--retry','3','-o',str(llm/name),url],check=True)
    subprocess.run(['tar','xzf','llama.tar.gz'],cwd=llm,check=True)
    subprocess.run(['bash','-c','mkdir -p cudart && tar xzf cudart.tar.gz -C cudart'],cwd=llm,check=True)
env = dict(os.environ, LD_LIBRARY_PATH=f"{llm/'llama-b11671'}:{llm/'cudart/cudart-llama-b11671-bin-ubuntu-cuda-12.8-x64'}:/usr/lib64-nvidia")
srv = subprocess.Popen([str(llm/'llama-b11671/llama-server'),'-m',str(gguf),'-ngl','99','-np','1','-c','8192','--host','127.0.0.1','--port','8901'],env=env,stdout=open('/tmp/llama.log','w'),stderr=2,start_new_session=True)
for i in range(120):
    time.sleep(5)
    try:
        if b'ok' in urllib.request.urlopen('http://127.0.0.1:8901/health',timeout=2).read(): break
    except: pass
else: raise RuntimeError('server sem health')
print('server OK; GPU MiB:', __import__('subprocess').run(['nvidia-smi','--query-gpu=memory.used','--format=csv,noheader'],capture_output=True,text=True).stdout.strip())

# --- geração (espelho do gen_dataset.py) ---
CAPS = ['rust_refactor','python_refactor','unit_testing','security_audit','code_review','research','summarization','translation']
QOS = ['QI_BALANCED','QI_CRITICAL','QI_STREAM_LIKE','QI_LOW_COST','QI_FAILOVER']
QOSD = {'QI_BALANCED':'common work','QI_CRITICAL':'critical urgent work','QI_STREAM_LIKE':'interactive streaming','QI_LOW_COST':'cheap batch','QI_FAILOVER':'resilient to failures'}
CAPT = {'rust_refactor':'refatoração Rust','python_refactor':'refatoração Python','unit_testing':'testes','security_audit':'auditoria de segurança','code_review':'revisão de código','research':'pesquisa','summarization':'resumo','translation':'tradução'}
def spec(rng):
    lang=rng.choice(['pt','en']); nc=rng.choices([1,2,3],weights=[50,35,15])[0]
    return {'lang':lang,'caps':sorted(rng.sample(CAPS,nc)),'planning':rng.random()<0.35,'urgency':rng.choices(['low','normal','high'],weights=[30,50,20])[0],'qos':rng.choices(QOS,weights=[40,15,10,20,15])[0]}
def prompt(s):
    caps=', '.join((CAPT[c] if s['lang']=='pt' else c.replace('_',' ')) for c in s['caps'])
    plan=('múltiplas etapas dependentes' if s['lang']=='pt' else 'multiple dependent steps') if s['planning'] else ('etapa ÚNICA' if s['lang']=='pt' else 'SINGLE-step')
    lang_txt=('Escreva UM pedido curto em português que exija' if s['lang']=='pt' else 'Write ONE short request in English requiring')
    return f'{lang_txt}: {caps}. Context: {s["qos"]}, {s["urgency"]}. Must be {plan}. Reply ONLY the request.'
def extract(raw):
    lines=[l.strip().strip('*').strip() for l in raw.splitlines()]
    c=[l for l in lines if l and not l.startswith(('-','*','#')) and ':' not in l[:24] and len(l)>=15]
    return c[-1] if c else ''
def call(p,t=0.9):
    for a in range(5):
        try:
            b=json.dumps({'messages':[{'role':'user','content':p}],'max_tokens':1024,'temperature':t}).encode()
            r=urllib.request.Request('http://127.0.0.1:8901/v1/chat/completions',data=b,headers={'Content-Type':'application/json'})
            d=json.load(urllib.request.urlopen(r,timeout=180))
            m=d['choices'][0]['message']
            return extract((m.get('content') or m.get('reasoning_content') or '').strip())
        except Exception as e: time.sleep(2*(a+1))
    return ''
def langok(t,l):
    pt=sum(t.lower().count(w) for w in [' o ',' a ',' de ',' que ',' para ']); en=sum(t.lower().count(w) for w in [' the ',' and ',' to ',' for ',' with '])
    return pt>=en if l=='pt' else en>pt

qs={'qos_profile':{'type':'choice','instructions':'Which QoS objective?','criteria':{p:QOSD[p] for p in QOS}},
    'needs_planning':{'type':'noul','instructions':'Multiple dependent tasks?'},
    'complexity':{'type':'score','instructions':'How complex?','criteria':['simple','moderate','complex','very_complex']},
    'urgency':{'type':'score','instructions':'How urgent?','criteria':['low','normal','high']}}
for c in CAPS: qs[f'cap__{c}']={'type':'noul','instructions':f"Requires '{c}'?"}

rng=random.Random(SEED); seen=set(); rows=[]
for i in range(N*6):
    if len(rows)>=N: break
    s=spec(rng); text=call(prompt(s))
    text=re.sub(r'^(pedido|request)\s*:?\s*','',text,flags=re.I).strip().strip('"')
    if not(20<=len(text)<=600) or not langok(text,s['lang']): continue
    k=re.sub(r'\s+',' ',text.lower()).strip()[:120]
    if k in seen: continue
    seen.add(k)
    rows.append({'state':{'text':text},'questions':qs,'expected':{'qos_profile':s['qos'],'needs_planning':s['planning'],'complexity':min(3,len(s['caps'])+(1 if s['planning'] else 0)),'urgency':['low','normal','high'].index(s['urgency']),**{f'cap__{c}':(c in s['caps']) for c in CAPS}},'language':s['lang'],'tags':[f'lang:{s["lang"]}',f'qos:{s["qos"]}','synthetic-by-construction-gemma4']})
    if len(rows)%100==0: print(f'{len(rows)}/{N}',flush=True)

out=OUT/f'colab-chunk-seed{SEED}.jsonl'
out.write_text('\n'.join(json.dumps(r,ensure_ascii=False) for r in rows)+'\n')
print(f'SALVO: {out} ({len(rows)} exemplos)')
srv.terminate()
"""
exec(CELL)
```

## Depois de gerar

1. Baixe o arquivo do Drive (`colab-chunk-seed<N>.jsonl`)
2. Copie para `evidence/calibration/colab-chunks/`
3. Execute: `python3 tools/merge_datasets.py evidence/calibration/calib_v2/ evidence/calibration/dataset_calib_v1.jsonl evidence/calibration/colab-chunks/*.jsonl`
4. Recalibre: `~/.cache/tese-laya/venv/bin/python tools/calibrate_laya.py evidence/calibration/calib_v2/train.jsonl evidence/calibration/calib_v2/`
