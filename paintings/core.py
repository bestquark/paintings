import hashlib
import shutil
import json
import os
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(os.environ.get('PAINTINGS_ROOT', Path(__file__).resolve().parents[1]))
if not (ROOT/'catalog').is_dir(): ROOT=Path(sys.prefix)/'share'/'paintings'
CONFIG = Path(os.environ.get('XDG_CONFIG_HOME',Path.home()/'.config'))/'paintings'
STATE = Path(os.environ.get('XDG_STATE_HOME',Path.home()/'.local'/'state'))/'paintings'
CACHE = Path(os.environ.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'paintings'
STREAMS = ('renaissance','baroque','romanticism','impressionism','abstraction','visionary','landscapes','classical')

def atomic(path,data):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    temporary=path.with_name(path.name+'.tmp-'+str(os.getpid()))
    temporary.write_bytes(data if isinstance(data,bytes) else data.encode())
    temporary.replace(path)

def write_json(path,value): atomic(path,json.dumps(value,indent=2,ensure_ascii=False)+'\n')

def config():
    d=json.loads((ROOT/'config'/'default.json').read_text())
    if (CONFIG/'config.json').exists():d.update(json.loads((CONFIG/'config.json').read_text()))
    return validate_config(d)

def validate_config(d):
    if not isinstance(d['interval_seconds'],int) or not 30<=d['interval_seconds']<=86400:raise ValueError('interval_seconds must be 30–86400')
    if not d['streams'] or any(s not in STREAMS for s in d['streams']):raise ValueError('Unknown or empty stream selection')
    if d.get('fit') not in ('cover','contain'):raise ValueError('fit must be cover or contain')
    return d

def catalog(stream):
    d=json.loads((ROOT/'catalog'/f'{stream}.json').read_text())
    return d['artworks']

def selection(cfg,now=None,offset=0):
    slot=int((time.time() if now is None else now)//cfg['interval_seconds'])+offset
    streams=cfg['streams'];stream=streams[slot%len(streams)]
    items=catalog(stream)
    if not items:raise ValueError('Empty stream: '+stream)
    # Shared ordering, without repetitions within each full stream cycle.
    items=sorted(items,key=lambda a:hashlib.sha256((cfg['seed']+stream+a['id']).encode()).hexdigest())
    return stream,items[(slot//len(streams))%len(items)]

def image_path(art):
    ext=Path(urllib.parse.urlsplit(art['image_url']).path).suffix.lower()
    if ext not in ('.jpg','.jpeg','.png'):ext='.jpg'
    return CACHE/'images'/(art['id']+ext)

def download(art):
    path=image_path(art)
    if path.exists() and path.stat().st_size>1000:return path
    url=art['image_url']
    if urllib.parse.urlsplit(url).scheme!='https':raise ValueError('Image URLs must use HTTPS')
    CACHE.mkdir(parents=True,exist_ok=True)
    if shutil.disk_usage(CACHE).free < 256*1024*1024:raise RuntimeError('Less than 256 MiB free; keeping the cached painting')
    req=urllib.request.Request(url,headers={'User-Agent':'Paintings/1.0 (https://github.com/bestquark/paintings)'})
    with urllib.request.urlopen(req,timeout=35) as r:
        data=r.read(60*1024*1024+1)
    if len(data)>60*1024*1024:raise ValueError('Artwork exceeds 60 MB')
    if not (data.startswith(b'\xff\xd8\xff') or data.startswith(b'\x89PNG\r\n\x1a\n')):raise ValueError('Source did not return JPEG or PNG')
    atomic(path,data)
    return path

def backup(path):
    path=Path(path)
    target=STATE/'backups'/hashlib.sha256(str(path).encode()).hexdigest()[:16]
    target.parent.mkdir(parents=True,exist_ok=True)
    target.parent.chmod(0o700)
    if not target.with_suffix('.json').exists():
        if path.exists():
            atomic(target,path.read_bytes());target.chmod(0o600)
        write_json(target.with_suffix('.json'),{'path':str(path),'existed':path.exists(),'saved_at':time.time()})
    return target

def managed_block(path,content):
    import re
    path=Path(path);backup(path)
    old=path.read_text() if path.exists() else ''
    old=re.sub(r'\n?# paintings:start\n.*?# paintings:end\n?', '\n',old,flags=re.S)
    atomic(path,old.rstrip()+'\n\n# paintings:start\n'+content.rstrip()+'\n# paintings:end\n')

def current():
    p=STATE/'current.json'
    return json.loads(p.read_text()) if p.exists() else None

def trim_cache(cfg,keep):
    files=sorted((CACHE/'images').glob('*'),key=lambda p:p.stat().st_mtime)
    total=sum(p.stat().st_size for p in files);limit=int(cfg.get('cache_limit_mb',4096))*1024*1024
    for p in files:
        if total<=limit:break
        if str(p) in keep:continue
        n=p.stat().st_size;p.unlink();total-=n
