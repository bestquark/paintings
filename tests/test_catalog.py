import json
import unittest
from paintings.core import STREAMS,catalog
class CatalogTests(unittest.TestCase):
 def test_catalog_contract(self):
  for stream in STREAMS:
   with self.subTest(stream=stream):
    items=catalog(stream)
    self.assertGreaterEqual(len(items),200)
    self.assertEqual(len(items),len({a['id'] for a in items}))
    self.assertEqual(len(items),len({a['image_sha256'] for a in items}))
    for a in items:
     self.assertIs(a['generated'],False)
     for key in ('title','artist','credit','license','source_url','license_url','image_url'):self.assertTrue(a[key],(a['id'],key))
     self.assertTrue(a['image_url'].startswith('https://'))
     self.assertGreaterEqual(a['display_width'],1100)
     self.assertGreaterEqual(a['display_height'],600)
 def test_requested_anchors(self):
  self.assertIn('commons-183030',{a['id'] for a in catalog('romanticism')})
  self.assertTrue(any(a['artist']=='Vincent van Gogh' for a in catalog('impressionism')))
  self.assertTrue(any(a['artist'] in ('Joachim Patinir','Pieter Bruegel the Elder','Albrecht Altdorfer') for a in catalog('landscapes')))
  self.assertTrue(any(a['artist'] in ('Rick Guidice','Don Davis') for a in catalog('visionary')))
