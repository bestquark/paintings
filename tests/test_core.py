import importlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from paintings import core

class RotationTests(unittest.TestCase):
 def test_every_artwork_once_per_cycle(self):
  cfg={'interval_seconds':300,'streams':['renaissance','landscapes'],'seed':'test'}
  artworks=[{'id':str(i)} for i in range(7)]
  with patch.object(core,'catalog',return_value=artworks):
   choices=[core.selection(cfg,now=i*300) for i in range(14)]
   for s in cfg['streams']:
    self.assertEqual({a['id'] for stream,a in choices if stream==s},set(map(str,range(7))))
   self.assertEqual(core.selection(cfg,now=301),core.selection(cfg,now=599))
 def test_managed_config_is_idempotent_and_backed_up(self):
  with tempfile.TemporaryDirectory() as folder, patch.object(core,'STATE',Path(folder)/'state'):
   p=Path(folder)/'terminal.conf';p.write_text('font-size = 18\n')
   core.managed_block(p,'config-file = art.conf');core.managed_block(p,'config-file = new.conf')
   self.assertEqual(p.read_text().count('# paintings:start'),1)
   self.assertIn('font-size = 18',p.read_text())
   original=core.backup(p);self.assertEqual(original.read_text(),'font-size = 18\n')
 def test_bad_image_does_not_replace_cache(self):
  with tempfile.TemporaryDirectory() as folder,patch.object(core,'CACHE',Path(folder)):
   with self.assertRaises(ValueError):core.download({'id':'bad','image_url':'http://example.com/a.jpg'})
   self.assertEqual(list(Path(folder).rglob('*')),[])
 def test_offline_preserves_last_success(self):
  from paintings import cli
  from argparse import Namespace
  with tempfile.TemporaryDirectory() as folder,patch.object(cli,'STATE',Path(folder)),patch.object(cli,'config',return_value={'adapters':[]}),patch.object(cli,'sync_config',side_effect=lambda c:c),patch.object(cli,'selection',return_value=('landscapes',{'id':'missing'})),patch.object(cli,'download',side_effect=OSError('offline')):
   current=Path(folder)/'current.json';current.write_text('{"kept":true}')
   with self.assertRaisesRegex(RuntimeError,'keeping'):cli.tick(Namespace(stream=None,id=None,next=False))
   self.assertEqual(json.loads(current.read_text()),{'kept':True})

if __name__=='__main__':unittest.main()

class PreferenceTests(unittest.TestCase):
 def test_local_choices_survive_shared_refresh(self):
  from paintings import cli
  with tempfile.TemporaryDirectory() as folder,patch.object(cli,'STATE',Path(folder)):
   core.write_json(Path(folder)/'shared.json',{'streams':['classical'],'fit':'cover','interval_seconds':300})
   cfg={'streams':['landscapes'],'fit':'contain','interval_seconds':300,'config_url':'https://example.com/config.json','overrides':{'streams':['renaissance'],'fit':'contain'}}
   self.assertEqual(cli.sync_config(cfg)['streams'],['renaissance'])
   self.assertEqual(cli.sync_config(cfg)['fit'],'contain')
   cfg['overrides']={}
   self.assertEqual(cli.sync_config(cfg)['streams'],['classical'])
   self.assertEqual(cli.sync_config(cfg)['fit'],'cover')
 def test_bad_shared_sizing_cannot_reach_adapters(self):
  from paintings import cli
  with tempfile.TemporaryDirectory() as folder,patch.object(cli,'STATE',Path(folder)):
   core.write_json(Path(folder)/'shared.json',{'streams':['classical'],'fit':'invalid','interval_seconds':300})
   with self.assertRaisesRegex(ValueError,'fit'):
    cli.sync_config({'streams':['classical'],'fit':'cover','interval_seconds':300,'config_url':'https://example.com/config.json'})
 def test_web_sync_exposes_only_visual_preferences(self):
  from paintings import cli
  with tempfile.TemporaryDirectory() as folder,patch.object(cli,'STATE',Path(folder)):
   root=Path(folder)/'site';target=root/'paintings/config/default.json';core.write_json(target,{})
   core.write_json(Path(folder)/'web-roots.json',[str(root)])
   cfg={**json.loads((core.ROOT/'config/default.json').read_text()),'config_url':'https://private.example/secret','adapters':['desktop'],'overrides':{'streams':['landscapes']}}
   cli.sync_web(cfg);public=json.loads(target.read_text())
   self.assertNotIn('config_url',public);self.assertNotIn('overrides',public);self.assertNotIn('adapters',public)
   self.assertIn('classical',public['available_streams']);self.assertEqual(public['fit'],'cover')
