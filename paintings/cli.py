import argparse
import hashlib
import re
import json
import os
import platform
import plistlib
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path
from . import __version__
from .core import ROOT, CONFIG, STATE, CACHE, STREAMS, atomic, write_json, config, catalog, selection, download, backup, current, trim_cache
from .adapters import ADAPTERS, run

def sync_config(cfg):
    """Only public visual preferences are shared; local adapter choices stay local."""
    import urllib.request
    url=cfg.get('config_url')
    if not url:return cfg
    cached=STATE/'shared.json'
    if not cached.exists() or time.time()-cached.stat().st_mtime>3600:
        try:
            if not url.startswith('https://'):raise ValueError('Shared config requires HTTPS')
            with urllib.request.urlopen(url,timeout=8) as response:d=json.load(response)
            if not isinstance(d.get('interval_seconds'),int) or not 30<=d['interval_seconds']<=86400:raise ValueError('Invalid interval')
            if not d.get('streams') or any(s not in STREAMS for s in d['streams']):raise ValueError('Invalid streams')
            write_json(cached,d)
        except Exception as e:print('Shared config: using cached preferences:',str(e),file=sys.stderr)
    if cached.exists():
        d=json.loads(cached.read_text())
        for k in ('interval_seconds','streams','seed','fit','terminal_opacity','web_opacity'):
            if k in d:cfg[k]=d[k]
    return cfg

def tick(args):
    cfg=sync_config(config());STATE.mkdir(parents=True,exist_ok=True)
    with (STATE/'rotation.lock').open('w') as lock:
        if os.name!='nt':
            import fcntl
            try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            except BlockingIOError:return {'skipped':'rotation already running'}
        if getattr(args,'stream',None):cfg['streams']=[args.stream]
        if getattr(args,'id',None):
            found=[(s,a) for s in STREAMS for a in catalog(s) if a['id']==args.id]
            if not found:raise ValueError('Unknown artwork ID')
            stream,art=found[0];image=download(art)
        else:
            failures=[];offset=0
            if getattr(args,'next',False):
                prior=current();offset=(prior or {}).get('manual_offset',0)+1
            for attempt in range(8):
                stream,art=selection(cfg,offset=offset+attempt)
                try:image=download(art);break
                except Exception as e:failures.append({'id':art['id'],'error':str(e)})
            else:
                write_json(STATE/'last-error.json',{'time':time.time(),'failures':failures})
                raise RuntimeError('Sources unavailable; keeping the previous painting')
        results={}
        for name in cfg['adapters']:
            try:results[name]={'ok':True,'detail':ADAPTERS[name](image,cfg)}
            except Exception as e:results[name]={'ok':False,'error':str(e)}
        state={'time':time.time(),'stream':stream,'artwork':art,'path':str(image),'interval_seconds':cfg['interval_seconds'],
            'manual_offset':offset if not getattr(args,'id',None) else 0,'adapters':results}
        write_json(STATE/'current.json',state)
        trim_cache(cfg,{str(image)})
        return state

def install(args):
    cfg=config();cfg['adapters']=args.adapters.split(',')
    if any(a not in ADAPTERS for a in cfg['adapters']):raise ValueError('Unknown adapter')
    if args.config_url:cfg['config_url']=args.config_url
    if args.stream:cfg['streams']=[args.stream]
    backup(CONFIG/'config.json');write_json(CONFIG/'config.json',cfg)
    executable=Path(sys.executable).resolve()
    launcher=Path.home()/'.local/bin/paintings';backup(launcher)
    atomic(launcher,'#!/bin/sh\nexport PAINTINGS_ROOT='+shlex.quote(str(ROOT))+'\nexport PYTHONPATH='+shlex.quote(str(ROOT))+'\nexec '+shlex.quote(str(executable))+' -m paintings "$@"\n');launcher.chmod(0o755)
    command=[str(launcher),'tick']
    logs=STATE/'logs';logs.mkdir(parents=True,exist_ok=True)
    if platform.system()=='Darwin':
        target=Path.home()/'Library/LaunchAgents/ca.bestquark.paintings.plist';backup(target)
        data={'Label':'ca.bestquark.paintings','ProgramArguments':command,'RunAtLoad':True,'StartInterval':cfg['interval_seconds'],
            'ProcessType':'Background','StandardOutPath':str(logs/'rotation.log'),'StandardErrorPath':str(logs/'errors.log'),
            'EnvironmentVariables':{'PATH':'/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'}}
        atomic(target,plistlib.dumps(data));domain='gui/'+str(os.getuid())
        subprocess.run(['launchctl','bootout',domain+'/ca.bestquark.paintings'],capture_output=True)
        run(['launchctl','bootstrap',domain,str(target)])
    elif platform.system()=='Linux':
        base=Path.home()/'.config/systemd/user';base.mkdir(parents=True,exist_ok=True)
        service=base/'paintings.service';timer=base/'paintings.timer'
        for p in (service,timer):backup(p)
        atomic(service,'[Unit]\nDescription=Paintings wallpaper rotation\n[Service]\nType=oneshot\nExecStart='+str(launcher)+' tick\nEnvironment=PATH='+str(Path.home()/'.local/bin')+':/usr/local/bin:/usr/bin:/bin\n')
        atomic(timer,'[Unit]\nDescription=Rotate paintings every five minutes\n[Timer]\nOnBootSec=20\nOnUnitActiveSec='+str(cfg['interval_seconds'])+'s\nAccuracySec=1s\n[Install]\nWantedBy=timers.target\n')
        run(['systemctl','--user','daemon-reload']);run(['systemctl','--user','enable','--now','paintings.timer'])
    else:raise RuntimeError('Automatic scheduling is supported on macOS and Linux; call paintings tick from your scheduler on Windows.')
    return {'installed':str(launcher),'config':str(CONFIG/'config.json'),'interval_seconds':cfg['interval_seconds'],'adapters':cfg['adapters']}

def uninstall(args):
    if platform.system()=='Darwin':subprocess.run(['launchctl','bootout','gui/'+str(os.getuid())+'/ca.bestquark.paintings'],capture_output=True)
    elif platform.system()=='Linux':subprocess.run(['systemctl','--user','disable','--now','paintings.timer'],capture_output=True)
    iterm_before=STATE/'iterm-default-before.json'
    if platform.system()=='Darwin' and iterm_before.exists():
        old=json.loads(iterm_before.read_text())
        args=['defaults','write','com.googlecode.iterm2','DefaultBookmarkGuid','-string',old['value']] if old['present'] else ['defaults','delete','com.googlecode.iterm2','DefaultBookmarkGuid']
        subprocess.run(args,capture_output=True)
    env_before=STATE/'openchamber-env-before.json'
    if platform.system()=='Darwin' and env_before.exists():
        old=json.loads(env_before.read_text())
        args=['launchctl','setenv','OPENCHAMBER_ELECTRON_LOAD_SERVER_UI',old['value']] if old['present'] else ['launchctl','unsetenv','OPENCHAMBER_ELECTRON_LOAD_SERVER_UI']
        run(args)
    restored=[]
    for meta in (STATE/'backups').glob('*.json'):
        d=json.loads(meta.read_text());target=Path(d['path']);data=meta.with_suffix('')
        if target.name=='index.html' and target.exists():
            html=re.sub(r'<script[^>]*data-paintings[^>]*></script>','',target.read_text())
            atomic(target,html)
        elif target.name=='openchamber.desktop' and target.exists():
            atomic(target,target.read_text().replace('Exec=env OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1 ','Exec=',1))
        elif target.exists() and '# paintings:start' in target.read_text(errors='ignore'):
            text=re.sub(r'\n?# paintings:start\n.*?# paintings:end\n?', '\n',target.read_text(),flags=re.S)
            atomic(target,text)
        elif target.name=='cli.json' and target.parent.name=='opencode' and target.exists():
            value=json.loads(target.read_text());original=json.loads(data.read_text()) if d['existed'] else {}
            if isinstance(value.get('theme'),dict) and value['theme'].get('name')=='paintings':
                if 'theme' in original:value['theme']=original['theme']
                else:value.pop('theme',None)
                write_json(target,value)
        elif d['existed']:atomic(target,data.read_bytes())
        elif target.exists():target.unlink()
        restored.append(str(target))
    return {'stopped':True,'restored_configs':restored,'note':'Artwork cache and provenance retained; choose a desktop picture in system settings.'}

def install_web(args):
    target=Path(args.root).resolve();target.mkdir(parents=True,exist_ok=True)
    dest=target/'paintings';dest.mkdir(exist_ok=True)
    for directory in ('catalog','config'):
        shutil.copytree(ROOT/directory,dest/directory,dirs_exist_ok=True)
    for f in ('paintings.js','paintings.css'):
        shutil.copy2(ROOT/'web'/f,dest/f)
    index=target/'index.html'
    if index.exists():
        backup(index);s=index.read_text()
        digest=hashlib.sha256((ROOT/'web/paintings.js').read_bytes()+(ROOT/'web/paintings.css').read_bytes()).hexdigest()[:12]
        tag='<script defer src="/paintings/paintings.js?v='+digest+'" data-paintings data-openchamber></script>'
        s=re.sub(r'<script[^>]*data-paintings[^>]*></script>','',s)
        atomic(index,s.replace('</head>',tag+'</head>',1))
    return {'installed':str(dest),'entry':str(index),'reload':'Refresh the browser; no backend restart required.'}

def main():
    ap=argparse.ArgumentParser(description='Existing art for your desktop and terminals.');ap.add_argument('--version',action='version',version=__version__)
    sub=ap.add_subparsers(dest='command',required=True)
    for name in ('tick','next'):
        p=sub.add_parser(name);p.add_argument('--stream',choices=STREAMS);p.add_argument('--id')
    sub.add_parser('current');sub.add_parser('streams')
    p=sub.add_parser('install');p.add_argument('--adapters',default='desktop');p.add_argument('--config-url');p.add_argument('--stream',choices=STREAMS)
    p=sub.add_parser('install-web');p.add_argument('--root',required=True)
    sub.add_parser('uninstall')
    p=sub.add_parser('download');p.add_argument('--stream',choices=STREAMS);p.add_argument('--limit',type=int,default=0)
    args=ap.parse_args()
    try:
        if args.command in ('tick','next'):args.next=args.command=='next';result=tick(args)
        elif args.command=='current':result=current()
        elif args.command=='streams':result={s:len(catalog(s)) for s in STREAMS}
        elif args.command=='install':result=install(args)
        elif args.command=='uninstall':result=uninstall(args)
        elif args.command=='install-web':result=install_web(args)
        elif args.command=='download':
            count=0;failed=[]
            for s in ([args.stream] if args.stream else STREAMS):
                for a in catalog(s)[:args.limit or None]:
                    try:download(a);count+=1
                    except Exception as e:failed.append({'id':a['id'],'error':str(e)})
            result={'downloaded':count,'failed':failed}
        print(json.dumps(result,indent=2,ensure_ascii=False))
    except Exception as e:print('paintings:',str(e),file=sys.stderr);return 1
    return 0

if __name__=='__main__':sys.exit(main())
