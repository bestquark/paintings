/* Paintings — existing art, with source credit. MIT. */
(() => {
  'use strict';
  const script = document.currentScript;
  if (document.getElementById('paintings-background')) return;
  const base = new URL('./', script.src);
  const chamber = script.hasAttribute('data-openchamber');
  const sheet = document.createElement('link');
  sheet.rel = 'stylesheet'; sheet.href = new URL('paintings.css' + new URL(script.src).search, base); document.head.append(sheet);
  const background = document.createElement('div'); background.id = 'paintings-background';
  background.setAttribute('aria-hidden', 'true');
  const controls = document.createElement('div'); controls.id = 'paintings-controls';
  controls.setAttribute('aria-label', 'Painting controls');
  const select = document.createElement('select'); select.setAttribute('aria-label', 'Painting stream');
  const credit = document.createElement('a'); credit.target = '_blank'; credit.rel = 'noopener noreferrer';
  const next = document.createElement('button'); next.textContent = 'Next'; next.title = 'Next painting';
  const pause = document.createElement('button'); pause.textContent = 'Pause';
  const close = document.createElement('button'); close.textContent = 'Hide'; close.title = 'Hide painting (click Art to restore)';
  controls.append(select, next, pause, close, credit);
  const menu = document.createElement('details'); menu.id = 'paintings-menu';
  const toggle = document.createElement('summary'); toggle.textContent = 'Art'; toggle.title = 'Painting settings and credit';
  menu.append(toggle, controls);
  if (document.getElementById('app-toolbar')) menu.classList.add('with-toolbar');
  document.body.prepend(background); document.body.append(menu);
  document.documentElement.classList.add(chamber ? 'paintings-openchamber' : 'paintings-gallery');
  let cfg, catalogs = {}, offset = 0, paused = false, previous = '', serial = 0, hidden = false, rotating = false;
  const stored = key => { try { return localStorage.getItem('paintings.' + key); } catch { return null; } };
  const persist = (key, value) => { try { localStorage.setItem('paintings.' + key, value); } catch {} };
  const hash = async text => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))), b => b.toString(16).padStart(2, '0')).join('');
  async function catalog(stream) {
    if (!catalogs[stream]) {
      const response = await fetch(new URL('catalog/' + stream + '.json', base), { cache: 'no-cache' });
      if (!response.ok) throw new Error('Catalog unavailable');
      const data = await response.json();
      catalogs[stream] = (await Promise.all(data.artworks.map(async art => ({ art, key: await hash(cfg.seed + stream + art.id) })))).sort((a, b) => a.key.localeCompare(b.key)).map(x => x.art);
    }
    return catalogs[stream];
  }
  async function rotate(force = false) {
    if (!cfg || hidden || (paused && !force) || (rotating && !force)) return;
    const generation = ++serial;
    rotating = true;
    try {
    const streams = select.value === 'all' ? cfg.streams : [select.value];
    const start = Math.floor(Date.now() / 1000 / cfg.interval_seconds) + offset;
    for (let attempt = 0; attempt < 5; attempt++) {
      try {
        const slot = start + attempt, stream = streams[slot % streams.length];
        const artworks = await catalog(stream), art = artworks[Math.floor(slot / streams.length) % artworks.length];
        if (art.id === previous && !force) return;
        const url = new URL(art.image_url);
        if (url.protocol !== 'https:') throw new Error('Artwork requires HTTPS');
        const image = new Image(); image.referrerPolicy = 'no-referrer';
        await new Promise((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error('Image timed out')), 25000);
          image.onload = () => { clearTimeout(timeout); resolve(); };
          image.onerror = () => { clearTimeout(timeout); reject(new Error('Image unavailable')); };
          image.src = url.href;
        });
        if (generation !== serial) return;
        background.style.backgroundImage = 'url(' + JSON.stringify(url.href) + ')';
        document.documentElement.style.setProperty('--painting-image', background.style.backgroundImage);
        previous = art.id;
        credit.textContent = art.title + ' · ' + art.artist;
        credit.href = art.source_url;
        credit.title = art.title + ' — ' + art.artist + ' — ' + art.license;
        document.dispatchEvent(new CustomEvent('paintings:change', { detail: { stream, artwork: art } }));
        window.paintings.current = { stream, artwork: art };
        return;
      } catch (error) { if (attempt === 4) console.warn('Paintings: keeping previous image.', error.message); }
    }
    } finally { if (generation === serial) rotating = false; }
  }
  next.onclick = () => { offset++; rotate(true); };
  pause.onclick = () => { paused = !paused; pause.textContent = paused ? 'Play' : 'Pause'; pause.setAttribute('aria-pressed', String(paused)); if (!paused) rotate(); };
  select.onchange = () => { offset = 0; persist('stream', select.value); rotate(true); };
  close.onclick = () => { hidden = !hidden; background.hidden = hidden; controls.classList.toggle('is-hidden', hidden); document.documentElement.classList.toggle('paintings-hidden', hidden); close.textContent = hidden ? 'Art' : 'Hide'; persist('hidden', hidden ? 'yes' : 'no'); if (!hidden) rotate(true); };
  window.paintings = { next: () => next.click(), pause: () => pause.click(), current: null };
  fetch(new URL('config/default.json', base), { cache: 'no-cache' }).then(r => { if (!r.ok) throw new Error('Config unavailable'); return r.json(); }).then(data => {
    cfg = data;
    for (const value of ['all', ...cfg.streams]) {
      const option = document.createElement('option'); option.value = value; option.textContent = value.charAt(0).toUpperCase() + value.slice(1); select.append(option);
    }
    select.value = cfg.streams.includes(stored('stream')) ? stored('stream') : 'all';
    background.style.opacity = chamber ? Math.max(0, Math.min(.4, Number(cfg.web_opacity ?? .16))) : 1;
    background.style.backgroundSize = cfg.fit === 'cover' ? 'cover' : 'contain';
    document.documentElement.style.setProperty('--painting-opacity', background.style.opacity);
    document.documentElement.style.setProperty('--painting-fit', background.style.backgroundSize);
    if (stored('hidden') === 'yes') close.click(); else rotate();
    setInterval(rotate, 1000);
  }).catch(error => { menu.remove(); background.remove(); console.warn('Paintings:', error.message); });
})();
