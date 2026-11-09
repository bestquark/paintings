#!/usr/bin/env python3
"""Download and decode every catalog image; report perceptual duplicate candidates."""
import concurrent.futures, hashlib, json, sys, time, urllib.error, fcntl
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from paintings.core import catalog,download,STREAMS,ROOT
from PIL import Image,ImageOps,ImageStat
DEST=ROOT/'.work/image-checks';DEST.mkdir(parents=True,exist_ok=True)
def one(a):
 p=DEST/(a['id']+'.json')
 if p.exists():
  prior=json.loads(p.read_text())
  if prior['ok'] or '429' not in prior.get('error',''):return prior
 d={'id':a['id'],'ok':False}
 for attempt in range(4):
  try:
   if 'upload.wikimedia.org' in a['image_url']:
    with (DEST/'wikimedia.lock').open('w') as lock:
     fcntl.flock(lock,fcntl.LOCK_EX)
     stamp=DEST/'wikimedia-next.txt'
     next_time=float(stamp.read_text()) if stamp.exists() else 0
     time.sleep(max(0,min(60,next_time-time.time())))
     stamp.write_text(str(time.time()+1.5))
   path=download(a)
   with Image.open(path) as im:
    im.load();w,h=im.size
    if w<1100 or h<600:raise ValueError('Image too small: '+str(im.size))
    gray=ImageOps.grayscale(im).resize((17,16));v=list(gray.getdata());bits=[v[y*17+x]>v[y*17+x+1] for y in range(16) for x in range(16)]
    d.update(ok=True,width=w,height=h,path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),dhash=hex(int(''.join('1' if b else '0' for b in bits),2)),color=ImageStat.Stat(im.convert('RGB').resize((16,16))).mean)
   break
  except urllib.error.HTTPError as e:
   d['error']=str(e)
   if e.code==429:
    (DEST/'wikimedia-next.txt').write_text(str(time.time()+60))
    time.sleep(60);continue
   if e.code in (502,503,504):time.sleep(5);continue
   break
  except Exception as e:d['error']=str(e);break
 p.write_text(json.dumps(d));return d
if __name__=='__main__':
 items={a['id']:a for s in (sys.argv[1:] or STREAMS) for a in catalog(s)}
 ok=bad=0
 with concurrent.futures.ThreadPoolExecutor(max_workers=2) as ex:
  for d in ex.map(one,items.values()):
   if d['ok']:ok+=1
   else:bad+=1;print('FAIL',d['id'],d.get('error'),flush=True)
   if (ok+bad)%25==0:print('progress',ok,bad,'/',len(items),flush=True)
 print('complete',ok,bad,flush=True)
