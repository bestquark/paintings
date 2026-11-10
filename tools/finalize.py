#!/usr/bin/env python3
"""Quality gate: decoded images, perceptual scan deduplication and provenance."""
import json,re,sys
from pathlib import Path
import numpy as np
from PIL import Image,ImageOps,ImageDraw
ROOT=Path(__file__).resolve().parents[1]
CHECKS=ROOT/'.work/image-checks'

def title(a):
 s=a['title'];en=re.search(r'label QS:Len,"([^"]+)"',s)
 if en:s=en[1]
 s=re.split(r'\s+(?:label QS:|title QS:)',s)[0]
 s=re.sub(r'^(?:English|German|Italian|French|Dutch|Russian|Polish|Spanish):\s*','',s)
 s=s.strip(' .');s=re.sub(r'\s+',' ',s)
 return s[:220]

def phash(path):
 with Image.open(path) as im:
  gray=np.asarray(ImageOps.grayscale(im).resize((32,32)),dtype=float)
 v=np.arange(32);mat=np.cos(np.pi*(2*v[None,:]+1)*v[:,None]/64)
 d=np.einsum('ij,jk,lk->il',mat,gray,mat)[:8,:8].flatten()[1:];bits=d>np.median(d)
 return int(''.join('1' if x else '0' for x in bits),2)

def finalize(stream,target=200):
 p=ROOT/'catalog'/f'{stream}.json';data=json.loads(p.read_text());items=data['artworks']
 kept=[];seen=[];dropped=[]
 exclusions=json.loads((ROOT/'tools/exclusions.json').read_text()) if (ROOT/'tools/exclusions.json').exists() else {}
 for a in items:
  if a['id'] in exclusions:dropped.append((a['id'],exclusions[a['id']]));continue
  c=CHECKS/(a['id']+'.json')
  if not c.exists():dropped.append((a['id'],'unchecked'));continue
  check=json.loads(c.read_text())
  if not check['ok']:dropped.append((a['id'],check.get('error')));continue
  a['title']=title(a)
  if re.search(r'(?i)\b(detail|dettaglio|detalhe|recto and verso|signature|firma|reverse side|postcard|engravings|sketchbook)\b',a['title']):dropped.append((a['id'],'detail'));continue
  if 'phash' not in check:
   check['phash']=hex(phash(check['path']));c.write_text(json.dumps(check))
  h=int(check['phash'],16)
  duplicate=next((old for oh,old in seen if bin(oh^h).count('1')<=6),None)
  if duplicate:dropped.append((a['id'],'duplicate of '+duplicate));continue
  key=re.sub(r'\W','',a['title'].lower())
  if len(key)>15 and any(re.sub(r'\W','',b['title'].lower())==key and b['artist']==a['artist'] and b.get('date')==a.get('date') for b in kept):dropped.append((a['id'],'same title'));continue
  seen.append((h,a['id']));a['display_width']=check['width'];a['display_height']=check['height'];a['image_sha256']=check['sha256'];kept.append(a)
 (ROOT/'.work'/f'{stream}-verified.json').write_text(json.dumps(kept,ensure_ascii=False))
 (ROOT/'.work'/f'{stream}-excluded.json').write_text(json.dumps(dropped))
 print(stream,'valid unique',len(kept),'dropped',len(dropped),flush=True)
 if '--write' in sys.argv:
  if len(kept)<target:raise ValueError('Fewer than '+str(target)+' verified works in '+stream)
  data['artworks']=kept[:target];p.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
 return kept

def sheet(stream,items):
 folder=ROOT/'.work/contact-sheets';folder.mkdir(exist_ok=True)
 for start in range(0,len(items),64):
  canvas=Image.new('RGB',(1920,1440),'#242420');draw=ImageDraw.Draw(canvas)
  for n,a in enumerate(items[start:start+64]):
   c=json.loads((CHECKS/(a['id']+'.json')).read_text())
   with Image.open(c['path']) as im:
    im=ImageOps.contain(im.convert('RGB'),(232,151));x=(n%8)*240+(240-im.width)//2;y=(n//8)*180
    canvas.paste(im,(x,y));draw.text(((n%8)*240+4,y+153),str(start+n)+' '+a['id'],fill='white');draw.text(((n%8)*240+4,y+166),a['title'][:34],fill='#ddd')
  canvas.save(folder/f'{stream}-{start//64}.jpg',quality=90)
if __name__=='__main__':
 for stream in [x for x in sys.argv[1:] if not x.startswith('--')] or ['renaissance','baroque','romanticism','impressionism','abstraction','visionary','landscapes','classical']:
  items=finalize(stream)
  if '--sheets' in sys.argv:sheet(stream,items)
