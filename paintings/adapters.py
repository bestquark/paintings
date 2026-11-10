import json
import os
import platform
import shutil
import subprocess
from pathlib import Path
from .core import ROOT, STATE, CONFIG, atomic, backup, managed_block

def run(args,**kw):
    p=subprocess.run(args,capture_output=True,text=True,timeout=kw.pop('timeout',25),**kw)
    if p.returncode:raise RuntimeError((p.stderr or p.stdout).strip()[:500] or f'{args[0]} exited {p.returncode}')
    return p.stdout.strip()

def desktop(image,cfg):
    system=platform.system()
    if system=='Darwin':
        binary=STATE/'bin'/'wallpaper';source=ROOT/'adapters'/'wallpaper.swift'
        if not binary.exists() or source.stat().st_mtime>binary.stat().st_mtime:
            binary.parent.mkdir(parents=True,exist_ok=True)
            run(['swiftc','-O',str(source),'-o',str(binary)],timeout=120)
        before=STATE/'desktop-before.json'
        if not before.exists():atomic(before,run([str(binary),'--inspect']))
        return json.loads(run([str(binary),str(image),cfg.get('fit','contain')]))
    if system=='Linux':
        env=os.environ.copy();bus=Path('/run/user')/str(os.getuid())/'bus'
        if bus.exists():env.setdefault('DBUS_SESSION_BUS_ADDRESS','unix:path='+str(bus))
        pc=subprocess.run(['pgrep','-u',str(os.getuid()),'-x','pcmanfm'],capture_output=True,text=True)
        if shutil.which('pcmanfm') and pc.stdout.strip():
            pid=pc.stdout.split()[0]
            raw=Path('/proc')/pid/'environ'
            for field in raw.read_bytes().split(b'\0'):
                key,sep,value=field.partition(b'=')
                if key.decode(errors='ignore') in ('DISPLAY','WAYLAND_DISPLAY','XAUTHORITY','XDG_RUNTIME_DIR','DBUS_SESSION_BUS_ADDRESS'):
                    env[key.decode()]=value.decode()
            args=(Path('/proc')/pid/'cmdline').read_bytes().decode().split('\0')
            profile=args[args.index('--profile')+1] if '--profile' in args else 'default'
            for original in (Path.home()/'.config/pcmanfm'/profile).glob('desktop-items-*.conf'):backup(original)
            run(['pcmanfm','--profile',profile,'--set-wallpaper='+str(image),'--wallpaper-mode='+('fit' if cfg.get('fit')=='contain' else 'crop')],env=env)
            return {'desktop':'pcmanfm','profile':profile,'image':str(image)}
        if shutil.which('gsettings'):
            schema='org.gnome.desktop.background';keys=run(['gsettings','list-keys',schema],env=env).splitlines()
            old={k:run(['gsettings','get',schema,k],env=env) for k in ('picture-uri','picture-uri-dark','picture-options') if k in keys}
            saved=STATE/'desktop-before.json'
            if not saved.exists():atomic(saved,json.dumps(old,indent=2))
            for key in ('picture-uri','picture-uri-dark'):
                if key in keys:run(['gsettings','set',schema,key,image.as_uri()],env=env)
            run(['gsettings','set',schema,'picture-options','scaled' if cfg.get('fit')=='contain' else 'zoom'],env=env)
            return {k:run(['gsettings','get',schema,k],env=env) for k in ('picture-uri','picture-uri-dark','picture-options') if k in keys}
        if shutil.which('pcmanfm'):
            run(['pcmanfm','--set-wallpaper='+str(image),'--wallpaper-mode='+('fit' if cfg.get('fit')=='contain' else 'crop')],env=env)
            return {'desktop':'pcmanfm','image':str(image)}
        raise RuntimeError('No supported desktop session (GNOME or PCManFM)')
    if system=='Windows':
        import ctypes
        if not ctypes.windll.user32.SystemParametersInfoW(20,0,str(image),3):raise RuntimeError('Windows wallpaper update failed')
        return {'desktop':'Windows','image':str(image)}
    raise RuntimeError('Unsupported desktop platform')

def ghostty(image,cfg):
    generated=CONFIG/'ghostty.conf'
    atomic(generated,f'background-image = {image}\nbackground-image-opacity = {cfg.get("terminal_opacity",.16)}\nbackground-image-fit = {cfg.get("fit","contain")}\nbackground-image-position = center\nbackground-image-repeat = false\n')
    folder=Path.home()/'.config'/'ghostty'
    target=folder/('config.ghostty' if (folder/'config.ghostty').exists() else 'config')
    managed_block(target,'config-file = '+str(generated))
    # Linux Ghostty registers SIGUSR2. macOS does not promise that handler.
    import signal
    p=subprocess.run(['pgrep','-u',str(os.getuid()),'-x','ghostty'],capture_output=True,text=True)
    pids=[int(v) for v in p.stdout.split() if v.isdigit()]
    reloaded=0
    if platform.system()=='Linux':
        for pid in pids:
            # Verify the handler is registered before sending a potentially fatal signal.
            status=Path(f'/proc/{pid}/status').read_text()
            mask=int(next(v.split()[1] for v in status.splitlines() if v.startswith('SigCgt:')),16)
            if mask & (1<<(signal.SIGUSR2-1)):os.kill(pid,signal.SIGUSR2);reloaded+=1
    return {'config':str(target),'reload_signalled':reloaded,'note':'On macOS use Reload Configuration for existing Ghostty windows.'}

def kitty(image,cfg):
    generated=CONFIG/'kitty.conf'
    atomic(generated,f'background_image {image}\nbackground_image_layout cscaled\nbackground_image_linear yes\nbackground_tint {1-cfg.get("terminal_opacity",.16):.2f}\n')
    target=Path.home()/'.config'/'kitty'/'kitty.conf';managed_block(target,'include '+str(generated))
    # Kitty supports live config reload via SIGUSR1; never enable remote control.
    import signal
    p=subprocess.run(['pgrep','-u',str(os.getuid()),'-x','kitty'],capture_output=True,text=True)
    pids=[int(v) for v in p.stdout.split() if v.isdigit()]
    for pid in pids:os.kill(pid,signal.SIGUSR1)
    return {'config':str(target),'reload_signalled':len(pids)}

def iterm(image,cfg):
    if platform.system()!='Darwin':raise RuntimeError('iTerm2 requires macOS')
    profiles=Path.home()/'Library'/'Application Support'/'iTerm2'/'DynamicProfiles'/'Paintings.json'
    backup(profiles)
    saved=STATE/'iterm-default-before.json'
    if not saved.exists():
        old=subprocess.run(['defaults','read','com.googlecode.iterm2','DefaultBookmarkGuid'],capture_output=True,text=True)
        atomic(saved,json.dumps({'present':old.returncode==0,'value':old.stdout.strip()}))
    atomic(profiles,json.dumps({'Profiles':[{'Name':'Paintings','Guid':'2C5D36E7-47CF-40C2-A60B-5A835453CD41',
        'Dynamic Profile Parent Name':'Default','Background Image Location':str(image),
        'Background Image Mode':2 if cfg.get('fit')=='cover' else 3,'Blend':1-cfg.get('terminal_opacity',.16)}]},indent=2))
    # Dynamic profiles are watched by iTerm2. No Apple Events or permission prompts.
    run(['defaults','write','com.googlecode.iterm2','DefaultBookmarkGuid','-string','2C5D36E7-47CF-40C2-A60B-5A835453CD41'])
    return {'profile':str(profiles),'default':True,'note':'Existing windows can select Paintings once; new windows use this profile.'}


def opencode(image,cfg):
    folder=Path.home()/'.config'/'opencode';theme=folder/'themes'/'paintings.json'
    # Full theme is shipped, so startup never depends on a remote theme service.
    backup(theme);atomic(theme,(ROOT/'adapters'/'opencode-theme.json').read_bytes())
    executable=shutil.which('opencode')
    if not executable:
        candidate=Path.home()/'.opencode/bin/opencode'
        if candidate.exists():executable=str(candidate)
    version=run([executable,'--version']) if executable else '2'
    if 'v1.' in version or version.startswith('1.'):
        raise RuntimeError('This adapter targets OpenCode 2.x; use a legacy transparent theme for 1.x.')
    target=folder/'cli.json'
    backup(target)
    data=json.loads(target.read_text()) if target.exists() else {}
    data['theme']={'name':'paintings','mode':'dark'};atomic(target,json.dumps(data,indent=2)+'\n')
    return {'theme':str(theme),'version':version,'note':'Transparent canvas uses the terminal emulator background; reopen theme settings for existing sessions.'}

def openchamber(image,cfg):
    # OpenChamber's built-in switch loads its configured server's frontend.
    # Backend selection and authentication are unchanged. Takes effect at next app launch.
    if platform.system()=='Darwin':
        saved=STATE/'openchamber-env-before.json'
        if not saved.exists():
            old=subprocess.run(['launchctl','getenv','OPENCHAMBER_ELECTRON_LOAD_SERVER_UI'],capture_output=True,text=True)
            atomic(saved,json.dumps({'present':old.returncode==0,'value':old.stdout.strip()}))
        run(['launchctl','setenv','OPENCHAMBER_ELECTRON_LOAD_SERVER_UI','1'])
        return {'native':'Server frontend enabled for next OpenChamber launch'}
    if platform.system()=='Linux':
        target=Path.home()/'.local/share/applications/openchamber.desktop'
        if not target.exists():return {'native':'No desktop launcher installed; web frontend remains available'}
        backup(target);text=target.read_text()
        if 'OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1' not in text:
            text=text.replace('Exec=','Exec=env OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1 ',1);atomic(target,text)
        return {'native':str(target),'note':'Uses server frontend on next launch'}
    raise RuntimeError('Native OpenChamber adapter supports macOS and Linux')

ADAPTERS={'desktop':desktop,'ghostty':ghostty,'kitty':kitty,'iterm':iterm,'opencode':opencode,'openchamber':openchamber}
