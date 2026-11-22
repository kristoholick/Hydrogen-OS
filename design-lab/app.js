'use strict';
// Design Lab behaviour. Plain JS, no build step, no dependencies.

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];

// ---------------- shared data ----------------
const APPS = [
  { id: 'archive',     name: 'The Archive',     kind: 'Files',          sig: 'archive',     pal: 'amber',   desc: 'Every file you own, laid out as a land to explore.' },
  { id: 'chronicle',   name: 'Chronicle',       kind: 'Notes',          sig: 'chronicle',   pal: 'gold',    desc: 'Notes and journals, kept by quill.' },
  { id: 'bard',        name: 'The Bard',        kind: 'Music',          sig: 'bard',        pal: 'teal',    desc: 'Music and sound, from a lone lute to a full choir.' },
  { id: 'pictures',    name: 'Moving Pictures', kind: 'Films',          sig: 'pictures',    pal: 'crimson', desc: 'Films and shows upon the great screen.' },
  { id: 'lantern',     name: 'Lanternlight',    kind: 'Photos',         sig: 'lantern',     pal: 'moss',    desc: 'Photographs and paintings, held up to the light.' },
  { id: 'scriptorium', name: 'Scriptorium',     kind: 'Documents',      sig: 'scriptorium', pal: 'ivory',   desc: 'Letters, ledgers and long documents.' },
  { id: 'telemetry',   name: 'Astrolabe',       kind: 'System monitor', sig: 'telemetry',   pal: 'frost',   desc: 'Read the heart of the machine: cores, memory and heat.' },
  { id: 'raven',       name: 'Ravenpost',       kind: 'Mail & messages',sig: 'raven',       pal: 'night',   desc: 'Messages and mail, carried swift by raven.' },
  { id: 'market',      name: 'The Market',      kind: 'App store',      sig: 'market',      pal: 'dusk',    desc: 'Trade for new tools and wonders.' },
  { id: 'forge',       name: 'The Forge',       kind: 'Terminal',       sig: 'forge',       pal: 'crimson', desc: 'Command line and craftsman’s tools.' },
  { id: 'hourglass',   name: 'Hourglass',       kind: 'Clock & calendar',sig: 'hourglass',  pal: 'violet',  desc: 'Bells, alarms and the turning of days.' },
  { id: 'codex',       name: 'Codex',           kind: 'Settings',       sig: 'codex',       pal: 'gold',    desc: 'The laws that govern every part of your realm.' },
];

const art = {}; // cache of generated data URIs
function landscapeURI(key, opts) {
  if (!art[key]) art[key] = Art.svgURI(Art.landscape(opts));
  return art[key];
}

// ---------------- realms ----------------
let realm = 1;
function setRealm(i) {
  realm = Math.max(0, Math.min(2, i));
  $('#track').style.transform = `translateX(${-realm * 100}vw)`;
  document.body.classList.remove('realm-0', 'realm-1', 'realm-2');
  document.body.classList.add('realm-' + realm);
  $$('#realmNav button').forEach(b => b.classList.toggle('on', +b.dataset.realm === realm));
  if (realm === 2) keepEntered();
}
$$('#realmNav button').forEach(b => b.addEventListener('click', () => setRealm(+b.dataset.realm)));

// ---------------- clock ----------------
function tick() {
  const d = new Date();
  const t = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  $$('[data-clock]').forEach(el => (el.textContent = t));
}
tick(); setInterval(tick, 20000);

// ================= THEATRE =================
const thApps = APPS.slice(0, 10);
let thIdx = 0, thLayer = 'a';
function buildTheatre() {
  const row = $('#thRow');
  row.innerHTML = thApps.map((a, i) => `
    <div class="th-card" data-i="${i}" style="background-image:${landscapeURI('card' + a.id, { w: 500, h: 500, seed: i * 7 + 3, palette: a.pal, layers: 4, birds: false })}">
      <div class="sig">${Art.sigil(a.sig, { stroke: 2.2 })}</div>
      <div class="lbl">${a.name}</div>
    </div>`).join('');
  $$('.th-card', row).forEach(c => c.addEventListener('click', () => thFocus(+c.dataset.i)));
  thFocus(0, true);
}
function thFocus(i, first = false) {
  thIdx = (i + thApps.length) % thApps.length;
  const a = thApps[thIdx];
  $$('.th-card').forEach((c, k) => c.classList.toggle('on', k === thIdx));
  $('#thRow').style.transform = `translateX(${-Math.max(0, thIdx - 2) * 168}px)`;
  $('#thTitle').textContent = a.name;
  $('#thDesc').textContent = a.desc;
  $('#thKicker').textContent = a.kind;
  const uri = landscapeURI('hero' + a.id, { w: 1600, h: 900, seed: thIdx * 13 + 5, palette: a.pal, layers: 5, castle: thIdx % 3 === 0, stars: a.pal === 'night' || a.pal === 'frost' });
  const next = thLayer === 'a' ? 'b' : 'a';
  const nextEl = $('.th-art.' + (first ? 'a' : next)), curEl = $('.th-art.' + thLayer);
  nextEl.style.backgroundImage = uri;
  nextEl.classList.remove('show'); void nextEl.offsetWidth; nextEl.classList.add('show');
  if (!first) { nextEl.style.opacity = 1; curEl.style.opacity = 0; thLayer = next; }
}

// ================= AETHER =================
const ALBUMS = [
  ['Embers of the Vale', 'The Lantern Choir', 'amber', 'hearth'],
  ['Salt & Iron', 'Harbour Wives', 'teal', 'compass'],
  ['Candle Hours', 'Brother Aldous', 'gold', 'lantern'],
  ['The Long Road', 'Wayfarers', 'dusk', 'roads'],
  ['Rain on Stone', 'Mira Holt', 'fog', 'scriptorium'],
  ['Hollow Crown', 'Ashgrove', 'crimson', 'crown'],
  ['Northwind', 'The Fjord Band', 'frost', 'raven'],
  ['Glasswater', 'Ilse Varn', 'moss', 'sight'],
];
function buildAether() {
  $('.ae-bg').style.backgroundImage = landscapeURI('aether', { w: 1600, h: 900, seed: 42, palette: 'aether', layers: 4, birds: true });
  $('#aeGrid').innerHTML = ALBUMS.map(([t, a, p, s], i) => `
    <div class="ae-tile"><div class="cv" style="background-image:${landscapeURI('alb' + i, { w: 300, h: 300, seed: 100 + i, palette: p, layers: 3, birds: false })}">${Art.sigil(s, { hatch: false })}</div>
    <b>${t}</b><span>${a}</span></div>`).join('');
  $('#aeCover').style.backgroundImage = landscapeURI('alb0', {});
  $$('#aeRail button').forEach(b => b.addEventListener('click', () => {
    $$('#aeRail button').forEach(x => x.classList.remove('on')); b.classList.add('on');
  }));
  const wrap = $('.ae-win-wrap'), back = $('.ae-win.back');
  $('#aether').addEventListener('mousemove', e => {
    const x = e.clientX / innerWidth - .5, y = e.clientY / innerHeight - .5;
    wrap.style.transform = `rotateY(${x * 8}deg) rotateX(${-y * 6}deg)`;
    back.style.transform = `translateZ(-160px) rotateY(${16 + x * 4}deg) rotateX(${-y * 3}deg) translateX(${-x * 30}px)`;
    $('.ae-bg').style.transform = `scale(1.06) translate(${-x * 18}px, ${-y * 12}px)`;
  });
}

// ================= THE KEEP =================
let kpTab = 'satchel', kpSel = 0, letterShown = false;
const SATCHEL = [...APPS, null, null, null, null];
const COUNTS = { archive: 3, bard: 1, raven: 7, forge: 2 };

function buildKeep() {
  $('#kpBrand').innerHTML = Art.sigil('crown', { size: 38 });
  $('#letterSeal').innerHTML = Art.waxSeal('crown', 54);
  $('#kpCats').innerHTML = ['compass', 'hearth', 'bard', 'ward', 'market', 'forge']
    .map((s, i) => `<span class="${i === 0 ? 'on' : ''}">${Art.sigil(s, { hatch: false, stroke: 3 })}</span>`).join('');
  $$('#kpCats span').forEach(sp => sp.addEventListener('click', () => {
    $$('#kpCats span').forEach(x => x.classList.remove('on')); sp.classList.add('on');
  }));

  $('#kpGrid').innerHTML = SATCHEL.map((a, i) => a
    ? `<div class="slot" data-i="${i}">${Art.sigil(a.sig)}${COUNTS[a.id] ? `<span class="cnt">${COUNTS[a.id]}</span>` : ''}</div>`
    : `<div class="slot empty"></div>`).join('');
  $$('#kpGrid .slot:not(.empty)').forEach(s => {
    s.addEventListener('mouseenter', () => kpSelect(+s.dataset.i));
    s.addEventListener('click', () => kpSelect(+s.dataset.i));
  });
  $('#kpCount').textContent = `${APPS.length} of ${SATCHEL.length}`;
  kpSelect(0);

  $$('#kpTabs button').forEach(b => b.addEventListener('click', () => kpSetTab(b.dataset.tab)));
  buildMap();
  buildCodex();
  buildRadial();
}

function kpSelect(i) {
  if (!SATCHEL[i]) return;
  kpSel = i;
  const a = SATCHEL[i];
  $$('#kpGrid .slot').forEach((s, k) => s.classList.toggle('on', k === i));
  $('#kpName').textContent = a.name;
  $('#kpDesc').textContent = a.desc;
  $('#kpBig').innerHTML = Art.sigil(a.sig, { size: 150, stroke: 1.6 });
  const opened = ['moments ago', 'this morning', 'yesterday', '3 days past', 'a fortnight past'][i % 5];
  $('#kpAttrs').innerHTML = `
    <dt>Kind</dt><dd>${a.kind}</dd>
    <dt>Weight</dt><dd>${(12 + i * 7.3).toFixed(1)} MB</dd>
    <dt>Last opened</dt><dd>${opened}</dd>
    <dt>Maker</dt><dd>The Keep</dd>
    <dt>Seals</dt><dd>${['Files', 'None', 'Sound', 'Display', 'Pictures', 'Files', 'System', 'Network', 'Network', 'Full', 'Bells', 'Full'][i]}</dd>`;
}

function kpSetTab(t) {
  kpTab = t;
  $$('#kpTabs button').forEach(b => b.classList.toggle('on', b.dataset.tab === t));
  $$('.kp-view').forEach(v => v.classList.toggle('on', v.dataset.view === t));
}
const TABS = ['satchel', 'archive', 'codex'];

// --- Archive map ---
const FOLDERS = [
  { name: 'Documents', sig: 'scriptorium', at: [.22, .52], size: '4.2 GB', region: 'C' },
  { name: 'Pictures',  sig: 'lantern',     at: [.34, .36], size: '38 GB',  region: 'C' },
  { name: 'Music',     sig: 'bard',        at: [.27, .66], size: '12 GB',  region: 'C' },
  { name: 'Downloads', sig: 'market',      at: [.41, .52], size: '7.9 GB', region: 'C' },
  { name: 'Projects',  sig: 'forge',       at: [.67, .52], size: '61 GB',  region: 'D' },
  { name: 'Games',     sig: 'crown',       at: [.78, .34], size: '412 GB', region: 'D' },
  { name: 'Backups',   sig: 'ward',        at: [.57, .82], size: '22 GB',  region: 'USB' },
];
function buildMap() {
  const W = 1200, H = 720;
  $('#mapWrap').innerHTML = Art.worldMap({
    w: W, h: H, seed: 9,
    regions: [
      { name: 'The Lowlands', sub: 'Drive C · 237 GB', at: [.3, .5], label: [.17, .18] },
      { name: 'Iron Coast', sub: 'Drive D · 1 TB', at: [.72, .42], label: [.84, .16] },
      { name: 'Isle of Wanderers', sub: 'Removable · 32 GB', at: [.57, .8], label: [.33, .9] },
    ],
    landmarks: FOLDERS,
  });
  const svg = $('#mapWrap svg');
  const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');
  g.innerHTML = FOLDERS.map((f, i) => {
    const x = f.at[0] * W, y = f.at[1] * H;
    return `<g class="landmark" data-i="${i}">
      <circle class="lm-ring" cx="${x}" cy="${y}" r="21"/>
      <svg class="lm-glyph" x="${x - 14}" y="${y - 14}" width="28" height="28" viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round">${Art.SIGILS[f.sig]}</svg>
      <text x="${x}" y="${y + 40}" text-anchor="middle">${f.name}</text></g>`;
  }).join('');
  svg.appendChild(g);
  $('#ledger').innerHTML = FOLDERS.map((f, i) => `<li data-i="${i}">${Art.sigil(f.sig, { hatch: false })}<span>${f.name}</span><small>${f.size}</small></li>`).join('');
  const pick = i => {
    $$('.landmark').forEach(l => l.classList.toggle('on', +l.dataset.i === i));
    $$('#ledger li').forEach(l => l.classList.toggle('on', +l.dataset.i === i));
  };
  $$('.landmark, #ledger li').forEach(el => {
    el.addEventListener('mouseenter', () => pick(+el.dataset.i));
    el.addEventListener('click', () => pick(+el.dataset.i));
  });
  pick(0);
}

// --- Codex skill tree ---
const CODEX = [
  { name: 'Sight',   sub: 'Display',            sig: 'sight',   desc: 'How the realm is shown: brightness, scale, night light and wallpapers.', opts: [['Night light', 1], ['Scale 125%', 1], ['HDR', 0]], child: ['Realms', 'lantern'] },
  { name: 'Roads',   sub: 'Network',            sig: 'roads',   desc: 'The roads your machine travels: Wi-Fi, Ethernet and proxies.', opts: [['Wi-Fi', 1], ['Metered road', 0], ['Airplane mode', 0]], child: ['Couriers', 'compass'] },
  { name: 'Wards',   sub: 'Privacy & security', sig: 'ward',    desc: 'Locks, seals and who may see what.', opts: [['Firewall', 1], ['Camera access', 0], ['Location', 0]], child: ['Oaths', 'crown'] },
  { name: 'Hearth',  sub: 'Power & storage',    sig: 'hearth',  desc: 'Keep the fire burning: battery, sleep and storage.', opts: [['Battery saver', 0], ['Sleep after 10 min', 1]], child: ['Storehouse', 'archive'] },
  { name: 'Tongues', sub: 'Language & time',    sig: 'tongue',  desc: 'Speech, script and the reckoning of hours.', opts: [['24-hour clock', 1], ['Auto time zone', 1]], child: ['Time', 'hourglass'] },
  { name: 'Voice',   sub: 'Sound',              sig: 'bard',    desc: 'Bells, voices and the bard’s instruments.', opts: [['UI sounds', 1], ['Spatial audio', 0]], child: ['Bells', 'raven'] },
];
function buildCodex() {
  const svg = $('#codexTree');
  const cx = 500, cy = 320;
  const ang = [-90, -30, 30, 90, 150, 210].map(a => a * Math.PI / 180);
  const pos = (a, d) => [cx + Math.cos(a) * d * 1.32, cy + Math.sin(a) * d * .93];
  const node = (x, y, r, sig, shape, cls, label, sub, data) => `
    <g class="node ${cls}" ${data}>
      ${shape === 'diamond'
        ? `<path class="shape" d="M${x} ${y - r} L${x + r} ${y} L${x} ${y + r} L${x - r} ${y} Z"/>`
        : `<circle class="shape" cx="${x}" cy="${y}" r="${r}"/>`}
      <svg x="${x - r * .55}" y="${y - r * .55}" width="${r * 1.1}" height="${r * 1.1}" viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">${Art.SIGILS[sig]}</svg>
      ${label ? `<text class="nlabel" x="${x}" y="${y + r + 20}">${label}</text>` : ''}
      ${sub ? `<text class="nsub" x="${x}" y="${y + r + 38}">${sub}</text>` : ''}
    </g>`;
  let edges = '', nodes = '';
  CODEX.forEach((c, i) => {
    const [x1, y1] = pos(ang[i], 165), [x2, y2] = pos(ang[i], 282);
    edges += `<path class="edge" data-b="${i}" d="M${cx} ${cy} L${x1} ${y1}"/><path class="edge" data-b="${i}" d="M${x1} ${y1} L${x2} ${y2}" stroke-dasharray="4 5"/>`;
    nodes += node(x1, y1, 36, c.sig, 'diamond', '', c.name, c.sub, `data-b="${i}"`);
    nodes += node(x2, y2, 22, c.child[1], 'circle', 'leaf', c.child[0], '', `data-b="${i}"`);
  });
  // small ornamental ring around root
  edges += `<circle cx="${cx}" cy="${cy}" r="72" fill="none" stroke="var(--gold-dim)" stroke-dasharray="2 6"/>`;
  nodes += node(cx, cy, 48, 'codex', 'circle', 'root', '', '', '');
  svg.innerHTML = edges + nodes;
  const show = i => {
    const c = CODEX[i];
    $$('#codexTree .node').forEach(n => n.classList.toggle('on', n.dataset.b == i));
    $$('#codexTree .edge').forEach(e => e.classList.toggle('lit', e.dataset.b == i));
    $('#ciTitle').textContent = `${c.name} · ${c.sub}`;
    $('#ciDesc').textContent = c.desc;
    $('#ciOpts').innerHTML = c.opts.map(([n, on]) => `<div class="ci-opt"><span>${n}</span><span class="toggle ${on ? 'on' : ''}"></span></div>`).join('');
    $$('#ciOpts .toggle').forEach(t => t.addEventListener('click', () => t.classList.toggle('on')));
  };
  $$('#codexTree .node[data-b]').forEach(n => {
    n.addEventListener('mouseenter', () => show(+n.dataset.b));
    n.addEventListener('click', () => show(+n.dataset.b));
  });
  show(0);
}

// --- Radial wheel ---
const RITES = [
  ['The Archive', 'archive', 'Open your files'],
  ['The Forge', 'forge', 'Open a terminal'],
  ['Lanternlight', 'lantern', 'Capture the screen'],
  ['Ravenpost', 'raven', 'Read new messages'],
  ['Hearth', 'hearth', 'Sleep, restart or shut down'],
  ['Ward', 'ward', 'Lock the Keep'],
  ['Hourglass', 'hourglass', 'Set a timer'],
  ['Codex', 'codex', 'Open settings'],
];
let riteSel = 0;
function buildRadial() {
  const R0 = 112, R1 = 238, n = RITES.length, gap = 1.6 * Math.PI / 180;
  const pt = (r, a) => `${(Math.cos(a) * r).toFixed(1)} ${(Math.sin(a) * r).toFixed(1)}`;
  let html = '';
  RITES.forEach(([name, sig], i) => {
    const a0 = (i / n) * Math.PI * 2 - Math.PI / 2 - Math.PI / n + gap, a1 = ((i + 1) / n) * Math.PI * 2 - Math.PI / 2 - Math.PI / n - gap;
    const am = (a0 + a1) / 2;
    const [sx, sy] = [Math.cos(am) * 175, Math.sin(am) * 175];
    html += `<g class="slice" data-i="${i}">
      <path class="bg" filter="url(#inkRing)" d="M${pt(R0, a0)} L${pt(R1, a0)} A${R1} ${R1} 0 0 1 ${pt(R1, a1)} L${pt(R0, a1)} A${R0} ${R0} 0 0 0 ${pt(R0, a0)} Z"/>
      <path class="edge" d="M${pt(R1 + 8, a0)} A${R1 + 8} ${R1 + 8} 0 0 1 ${pt(R1 + 8, a1)}"/>
      <svg class="sg" x="${sx - 26}" y="${sy - 26}" width="52" height="52" viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">${Art.SIGILS[sig]}</svg>
    </g>`;
  });
  html += `<circle r="${R0 - 8}" fill="rgba(8,8,8,.82)" stroke="rgba(230,222,200,.18)" filter="url(#inkRing)"/>`;
  $('#radialSvg').innerHTML = html;
  riteSelect(0);
  $('#radial').addEventListener('mousemove', e => {
    const r = $('#radialSvg').getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
    if (Math.hypot(dx, dy) < 40) return;
    let a = Math.atan2(dy, dx) + Math.PI / 2 + Math.PI / RITES.length;
    a = (a + Math.PI * 4) % (Math.PI * 2);
    riteSelect(Math.floor(a / (Math.PI * 2 / RITES.length)));
  });
}
function riteSelect(i) {
  riteSel = (i + RITES.length) % RITES.length;
  $$('#radialSvg .slice').forEach(s => s.classList.toggle('on', +s.dataset.i === riteSel));
  $('#rcName').textContent = RITES[riteSel][0];
  $('#rcDesc').textContent = RITES[riteSel][2];
}
let wheelOpen = false;
function openWheel() { wheelOpen = true; $('#keep').classList.add('wheel-open'); }
function closeWheel(invoke) {
  if (!wheelOpen) return;
  wheelOpen = false; $('#keep').classList.remove('wheel-open');
  if (invoke) showLetter(`Rite invoked: ${RITES[riteSel][0]}`, RITES[riteSel][2] + '.', RITES[riteSel][1]);
}
$('#keep').addEventListener('contextmenu', e => e.preventDefault());
$('#keep').addEventListener('mousedown', e => { if (e.button === 2) openWheel(); });
addEventListener('mouseup', e => { if (e.button === 2) closeWheel(true); });

// --- wax-seal letter ---
let letterTimer;
function showLetter(t, d, sig = 'crown') {
  $('.letter-t').textContent = t;
  $('.letter-d').textContent = d;
  $('#letterSeal').innerHTML = Art.waxSeal(sig, 54);
  $('#letter').classList.add('show');
  clearTimeout(letterTimer);
  letterTimer = setTimeout(() => $('#letter').classList.remove('show'), 4500);
}
function keepEntered() {
  if (letterShown) return;
  letterShown = true;
  setTimeout(() => showLetter('A letter arrives', 'The kernel scribes have prepared an update.'), 1400);
}

// --- Keep backdrop by mood ---
function keepBackdrop(mood) {
  const pal = mood === 'mono' ? 'night' : mood;
  $('.kp-bg').style.backgroundImage = landscapeURI('keep' + pal, { w: 1600, h: 900, seed: 77, palette: pal, layers: 6, castle: true, stars: pal === 'night' });
}

// ================= KEYBOARD =================
addEventListener('keydown', e => {
  const k = e.key.toLowerCase();
  if (e.target.tagName === 'INPUT') return;
  if (k === 'q' || k === 'pageup') return setRealm(realm - 1);
  if (k === 'e' || k === 'pagedown') return setRealm(realm + 1);
  if (k === 't') return $('#tweaker').classList.toggle('open');
  if (k === 'escape') { $('#tweaker').classList.remove('open'); return closeWheel(false); }

  if (realm === 0) {
    if (k === 'arrowright') thFocus(thIdx + 1);
    if (k === 'arrowleft') thFocus(thIdx - 1);
  }
  if (realm === 2) {
    if (k === 'tab') { e.preventDefault(); if (!e.repeat) openWheel(); return; }
    if (wheelOpen) {
      if (k === 'arrowright' || k === 'arrowdown') riteSelect(riteSel + 1);
      if (k === 'arrowleft' || k === 'arrowup') riteSelect(riteSel - 1);
      return;
    }
    if (k === 'z') kpSetTab(TABS[(TABS.indexOf(kpTab) + 2) % 3]);
    if (k === 'c') kpSetTab(TABS[(TABS.indexOf(kpTab) + 1) % 3]);
    if (kpTab === 'satchel') {
      const step = { arrowright: 1, arrowleft: -1, arrowdown: 4, arrowup: -4 }[k];
      if (step) { e.preventDefault(); const n = kpSel + step; if (SATCHEL[n]) kpSelect(n); }
      if (k === 'enter') showLetter(`Opening ${SATCHEL[kpSel].name}`, SATCHEL[kpSel].desc, SATCHEL[kpSel].sig);
    }
  }
});
addEventListener('keyup', e => { if (e.key === 'Tab' && realm === 2) closeWheel(true); });

// ================= STYLE TWEAKER =================
const DEFAULTS = { panel: 'ink', font: 'cinzel', accent: 'gold', sel: 'crimson', mood: 'night', grain: 0.12 };
let style = { ...DEFAULTS };
try { Object.assign(style, JSON.parse(localStorage.getItem('sigil-style') || '{}')); } catch { /* ignore */ }

function applyStyle() {
  const b = document.body;
  [...b.classList].filter(c => /^(panel|font|accent|sel|mood)-/.test(c)).forEach(c => b.classList.remove(c));
  ['panel', 'font', 'accent', 'sel', 'mood'].forEach(k => b.classList.add(`${k}-${style[k]}`));
  document.documentElement.style.setProperty('--grain', style.grain);
  $('#grainRange').value = style.grain;
  $('#grainVal').textContent = Math.round(style.grain * 100) + '%';
  $$('#tweaker .seg').forEach(seg => $$('button', seg).forEach(bt => bt.classList.toggle('on', bt.dataset.v === style[seg.dataset.key])));
  keepBackdrop(style.mood);
  try { localStorage.setItem('sigil-style', JSON.stringify(style)); } catch { /* ignore */ }
  $('#twOut').textContent = tokens();
}
function tokens() {
  const cs = getComputedStyle(document.body);
  const v = n => cs.getPropertyValue(n).trim();
  return JSON.stringify({
    choices: style,
    colors: { gold: v('--gold'), goldDim: v('--gold-dim'), selection: style.sel, crimson: v('--crimson'), ivory: v('--ivory'), ash: v('--ash'), ink: v('--ink'), void: v('--void') },
    fonts: { keepHeader: v('--kp-font'), body: 'EB Garamond', mono: 'JetBrains Mono', ui: 'Inter' },
  }, null, 2);
}
$$('#tweaker .seg').forEach(seg => $$('button', seg).forEach(bt => bt.addEventListener('click', () => {
  style[seg.dataset.key] = bt.dataset.v; applyStyle();
})));
$('#grainRange').addEventListener('input', e => { style.grain = +e.target.value; applyStyle(); });
$('#twClose').addEventListener('click', () => $('#tweaker').classList.remove('open'));
$('#twCopy').addEventListener('click', async () => {
  const txt = tokens();
  try { await navigator.clipboard.writeText(txt); $('#twCopy').textContent = 'Copied ✓'; }
  catch { getSelection().selectAllChildren($('#twOut')); $('#twCopy').textContent = 'Selected — press Ctrl+C'; }
  setTimeout(() => ($('#twCopy').textContent = 'Copy tokens'), 1800);
});

// ================= BOOT =================
$('#grain').style.backgroundImage = Art.grainURI;
buildTheatre();
buildAether();
buildKeep();
applyStyle();
// first placement without the slide animation
$('#track').style.transition = 'none';
setRealm(new URLSearchParams(location.search).has('realm') ? +new URLSearchParams(location.search).get('realm') : 1);
requestAnimationFrame(() => requestAnimationFrame(() => ($('#track').style.transition = '')));
