// Procedural, original artwork for the Design Lab.
// Everything here is generated from code: no external or game assets.
'use strict';

const Art = (() => {
  // ---------- seeded randomness ----------
  function rng(seed) {
    let a = seed >>> 0;
    return () => {
      a |= 0; a = (a + 0x6D2B79F5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // 1D fractal value noise
  function noise1D(r, octaves = 5) {
    const tables = [];
    for (let o = 0; o < octaves; o++) {
      const n = 4 * 2 ** o + 1;
      tables.push(Array.from({ length: n + 1 }, r));
    }
    return x => { // x in [0,1]
      let v = 0, amp = 1, sum = 0;
      tables.forEach((t, o) => {
        const n = t.length - 1, p = x * (n - 1), i = Math.floor(p), f = p - i;
        const s = f * f * (3 - 2 * f);
        v += amp * (t[i] * (1 - s) + t[i + 1] * s);
        sum += amp; amp *= 0.5;
      });
      return v / sum;
    };
  }

  const mix = (a, b, t) => {
    const pa = parseInt(a.slice(1), 16), pb = parseInt(b.slice(1), 16);
    const c = [16, 8, 0].map(s => Math.round(((pa >> s) & 255) * (1 - t) + ((pb >> s) & 255) * t));
    return '#' + c.map(v => v.toString(16).padStart(2, '0')).join('');
  };

  // ---------- palettes ----------
  const PALETTES = {
    amber:   { skyTop: '#120a07', skyBot: '#b8612a', sun: '#ffcf8a', far: '#6b3a22', near: '#0d0706', fog: '#d9874a' },
    teal:    { skyTop: '#04101a', skyBot: '#1d5b66', sun: '#bfe9e4', far: '#1f4249', near: '#03090c', fog: '#5fa3a3' },
    crimson: { skyTop: '#12040a', skyBot: '#8f1d21', sun: '#ffb38a', far: '#4a1418', near: '#080305', fog: '#c0503f' },
    moss:    { skyTop: '#0a0f0b', skyBot: '#5f7357', sun: '#e8efd0', far: '#34432f', near: '#060905', fog: '#9fb08f' },
    ivory:   { skyTop: '#2a2621', skyBot: '#d9c9a3', sun: '#fff6df', far: '#7e705a', near: '#15120e', fog: '#efe3c6' },
    frost:   { skyTop: '#050913', skyBot: '#3c5a80', sun: '#dbe8ff', far: '#24364f', near: '#04070c', fog: '#8aa4c4' },
    violet:  { skyTop: '#0b0712', skyBot: '#5a3f78', sun: '#f1d9ff', far: '#33244a', near: '#07050b', fog: '#9b80b8' },
    gold:    { skyTop: '#0f0b05', skyBot: '#a2803d', sun: '#fff0c2', far: '#5a4622', near: '#0a0804', fog: '#c8a96e' },
    night:   { skyTop: '#030406', skyBot: '#2b3138', sun: '#d7dde3', far: '#1b2026', near: '#020203', fog: '#59636d' },
    dusk:    { skyTop: '#0c0a10', skyBot: '#6a4a46', sun: '#f3cfa6', far: '#3a2c2c', near: '#060506', fog: '#9a7464' },
    fog:     { skyTop: '#1a1b1c', skyBot: '#7d7f7c', sun: '#f2f1ea', far: '#4d4f4d', near: '#0d0e0e', fog: '#b3b4ae' },
    aether:  { skyTop: '#1b1726', skyBot: '#c79a8a', sun: '#fff1e0', far: '#7d6c86', near: '#2a2333', fog: '#e0c2bb' },
  };

  // ---------- landscape ----------
  // Returns an SVG string: sky, sun glow, layered ridges, fog, optional castle and birds.
  function landscape({ w = 1600, h = 900, seed = 1, palette = 'amber', layers = 5, castle = false, birds = true, stars = false }) {
    const p = PALETTES[palette] || PALETTES.amber;
    const r = rng(seed);
    const id = 'l' + seed + palette;
    const sunX = w * (0.25 + r() * 0.5), sunY = h * (0.32 + r() * 0.12);
    let s = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" preserveAspectRatio="xMidYMid slice">
      <defs>
        <linearGradient id="${id}s" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="${p.skyTop}"/><stop offset="0.75" stop-color="${p.skyBot}"/>
        </linearGradient>
        <radialGradient id="${id}g" cx="${sunX / w}" cy="${sunY / h}" r="0.55">
          <stop offset="0" stop-color="${p.sun}" stop-opacity=".85"/>
          <stop offset=".12" stop-color="${p.sun}" stop-opacity=".35"/>
          <stop offset="1" stop-color="${p.sun}" stop-opacity="0"/>
        </radialGradient>
        <linearGradient id="${id}f" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="${p.fog}" stop-opacity="0"/>
          <stop offset="1" stop-color="${p.fog}" stop-opacity=".45"/>
        </linearGradient>
      </defs>
      <rect width="${w}" height="${h}" fill="url(#${id}s)"/>`;
    if (stars) {
      for (let i = 0; i < 140; i++) {
        s += `<circle cx="${(r() * w).toFixed(1)}" cy="${(r() * h * 0.55).toFixed(1)}" r="${(r() * 1.2 + .2).toFixed(2)}" fill="#fff" opacity="${(r() * .6 + .1).toFixed(2)}"/>`;
      }
    }
    s += `<rect width="${w}" height="${h}" fill="url(#${id}g)"/>
      <circle cx="${sunX}" cy="${sunY}" r="${h * 0.035}" fill="${p.sun}" opacity=".9"/>`;

    const castleLayer = 1;
    for (let i = 0; i < layers; i++) {
      const t = i / (layers - 1);
      const n = noise1D(r, 6);
      const base = h * (0.5 + t * 0.32);
      const amp = h * (0.30 - t * 0.14);
      const color = mix(p.far, p.near, Math.pow(t, 0.8));
      const pts = [];
      const steps = 120;
      for (let k = 0; k <= steps; k++) {
        const x = (k / steps) * w;
        const y = base - n(k / steps) * amp + amp * 0.35;
        pts.push([x, y]);
      }
      s += `<path d="M0 ${h} ${pts.map(([x, y]) => `L${x.toFixed(1)} ${y.toFixed(1)}`).join(' ')} L${w} ${h} Z" fill="${color}"/>`;
      if (castle && i === castleLayer) {
        // pick the highest point in the middle half for the keep
        let best = pts[30];
        for (let k = 30; k < 90; k++) if (pts[k][1] < best[1]) best = pts[k];
        s += castleSVG(best[0], best[1] + 6, h * 0.12, color);
      }
      if (i < layers - 1) {
        s += `<rect x="0" y="${base - amp * 0.4}" width="${w}" height="${amp * 0.9}" fill="url(#${id}f)" opacity="${0.6 - t * 0.4}"/>`;
      }
    }
    if (birds) {
      for (let i = 0; i < 6; i++) {
        const bx = sunX + (r() - 0.5) * w * 0.4, by = sunY + (r() - 0.6) * h * 0.2, bs = 4 + r() * 6;
        s += `<path d="M${bx - bs} ${by} q${bs / 2} ${-bs / 2} ${bs} 0 q${bs / 2} ${-bs / 2} ${bs} 0" stroke="${p.near}" stroke-width="1.4" fill="none" opacity=".7"/>`;
      }
    }
    return s + '</svg>';
  }

  function castleSVG(cx, groundY, size, color) {
    const u = size / 10;
    const parts = [];
    const tower = (x, w, hgt, spire) => {
      parts.push(`<rect x="${x}" y="${groundY - hgt}" width="${w}" height="${hgt}"/>`);
      if (spire) parts.push(`<path d="M${x - u * .3} ${groundY - hgt} L${x + w / 2} ${groundY - hgt - u * 3.5} L${x + w + u * .3} ${groundY - hgt} Z"/>`);
      else for (let c = 0; c < 3; c++) parts.push(`<rect x="${x + c * (w / 2.5)}" y="${groundY - hgt - u * .8}" width="${w / 5}" height="${u * .8}"/>`);
    };
    tower(cx - u * 6, u * 2.2, u * 6, true);
    parts.push(`<rect x="${cx - u * 4}" y="${groundY - u * 4}" width="${u * 8}" height="${u * 4}"/>`);
    tower(cx - u * 1.4, u * 2.8, u * 9, true);
    tower(cx + u * 3.6, u * 2, u * 5.5, false);
    tower(cx + u * 6, u * 1.6, u * 7, true);
    return `<g fill="${color}">${parts.join('')}</g>`;
  }

  // ---------- sigils: original engraved-style icons ----------
  // Each is drawn in a 64x64 box, stroked in currentColor.
  const SIGILS = {
    archive: `<path d="M14 18h36v34H14z"/><path d="M14 26h36M22 18v-6h20v6"/><circle cx="32" cy="38" r="6"/><path d="M32 44v6M26 36l-4-2M38 36l4-2"/><path d="M18 30v18M46 30v18" stroke-dasharray="2 3"/>`,
    chronicle: `<path d="M18 52 46 12c3 4 4 10 0 16L24 54z"/><path d="M18 52l-4 6M30 40l10-14M24 46l8-10"/><path d="M40 26c2 2 3 5 1 8" opacity=".7"/>`,
    bard: `<path d="M22 54c-6-10-6-26 4-38 6-6 14-6 18 0"/><path d="M44 16c4 10 2 26-8 38H22"/><path d="M28 20v30M33 18v32M38 18v32"/><circle cx="44" cy="14" r="3"/>`,
    pictures: `<path d="M14 18c10 4 26 4 36 0v20c0 10-8 16-18 16S14 48 14 38z"/><path d="M22 28c2-2 6-2 8 0M34 28c2-2 6-2 8 0"/><path d="M24 42c4 4 12 4 16 0"/>`,
    lantern: `<path d="M26 14h12M28 14v-4h8v4"/><path d="M22 20h20l-2 28H24z"/><path d="M22 48h20v4H22z"/><path d="M32 28c-3 4-3 8 0 12 3-4 3-8 0-12z"/><path d="M27 20v28M37 20v28" opacity=".6"/>`,
    scriptorium: `<path d="M16 16h28c4 0 6 3 6 6v26c0 3-2 5-5 5H20c-3 0-4-2-4-4z"/><path d="M16 16c-3 0-4 2-4 4s1 4 4 4"/><path d="M22 28h20M22 34h20M22 40h14"/>`,
    telemetry: `<circle cx="32" cy="32" r="20"/><circle cx="32" cy="32" r="13"/><path d="M32 8v8M32 48v8M8 32h8M48 32h8"/><path d="M32 32 44 22"/><circle cx="32" cy="32" r="2.5"/>`,
    codex: `<circle cx="32" cy="32" r="10"/><path d="M32 10v8M32 46v8M10 32h8M46 32h8M16.5 16.5l5.7 5.7M41.8 41.8l5.7 5.7M16.5 47.5l5.7-5.7M41.8 22.2l5.7-5.7"/><circle cx="32" cy="32" r="22" stroke-dasharray="3 4"/>`,
    raven: `<path d="M12 40c10-2 18-10 22-22 6 2 10 6 10 10l8 2-8 4c-2 8-10 14-22 14z"/><circle cx="40" cy="26" r="1.6"/><path d="M22 52l-4 6M28 52l-2 6"/>`,
    market: `<path d="M22 22c-6 6-8 18-2 26 6 6 18 6 24 0 6-8 4-20-2-26z"/><path d="M24 22l-4-8h24l-4 8M24 22h16"/><path d="M32 30v14M28 34c0-3 8-3 8 0s-8 3-8 6 8 3 8 0"/>`,
    hourglass: `<path d="M18 10h28M18 54h28M22 10c0 12 20 14 20 22S22 42 22 54M42 10c0 12-20 14-20 22s20 10 20 22"/><path d="M27 48h10l-5-6z"/>`,
    forge: `<path d="M12 26h30c0 6 6 8 12 8v4H42c-2 4-4 6-8 6H24c-4 0-6-2-8-6l-4-12z"/><path d="M22 44v8h20v-8M18 52h28"/>`,
    compass: `<circle cx="32" cy="32" r="20"/><path d="M32 12l5 20-5 20-5-20z"/><path d="M12 32l20-5 20 5-20 5z" opacity=".7"/><circle cx="32" cy="32" r="2"/>`,
    hearth: `<path d="M32 54c-10 0-16-6-16-14 0-10 10-14 10-24 6 4 8 10 8 14 2-2 3-5 3-8 6 6 11 12 11 18 0 8-6 14-16 14z"/><path d="M32 54c-4 0-6-3-6-6 0-4 6-8 6-12 2 4 6 7 6 12 0 3-2 6-6 6z"/>`,
    ward: `<path d="M32 10l18 6v14c0 12-8 20-18 24-10-4-18-12-18-24V16z"/><path d="M32 18v28M22 30h20"/>`,
    tongue: `<path d="M12 16h28v20H24l-8 6v-6h-4z"/><path d="M30 40h4l8 6v-6h10V22H44"/><path d="M18 24h16M18 30h10"/>`,
    sight: `<path d="M8 32c8-12 40-12 48 0-8 12-40 12-48 0z"/><circle cx="32" cy="32" r="8"/><circle cx="32" cy="32" r="3"/>`,
    roads: `<path d="M20 56 28 8M44 56 36 8"/><path d="M32 12v6M32 26v8M32 42v10"/>`,
    crown: `<path d="M12 44 14 20l10 10 8-16 8 16 10-10 2 24z"/><path d="M12 50h40"/>`,
    skull: `<path d="M18 34c-4-14 6-24 14-24s18 10 14 24l-2 6h-4v6H24v-6h-4z"/><circle cx="26" cy="30" r="4"/><circle cx="38" cy="30" r="4"/><path d="M30 40l2-4 2 4"/>`,
  };

  function sigil(name, { size = 64, stroke = 2, hatch = true } = {}) {
    const body = SIGILS[name] || SIGILS.compass;
    // "engraving": a faint offset copy behind the main stroke reads as cut lines
    const shadow = hatch ? `<g transform="translate(1.2 1.2)" opacity=".28">${body}</g>` : '';
    return `<svg class="sigil" viewBox="0 0 64 64" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round">${shadow}${body}</svg>`;
  }

  // ---------- parchment world map (file manager) ----------
  function worldMap({ w = 1200, h = 720, seed = 7, regions = [], landmarks = [] }) {
    const r = rng(seed);
    let blobs = '';
    // land: clusters of ellipses roughened by turbulence
    const centers = regions.map(g => g.at);
    centers.forEach(([cx, cy], i) => {
      for (let k = 0; k < 9; k++) {
        const ex = cx * w + (r() - .5) * w * .16, ey = cy * h + (r() - .5) * h * .2;
        blobs += `<ellipse cx="${ex}" cy="${ey}" rx="${60 + r() * 90}" ry="${40 + r() * 70}"/>`;
      }
    });
    let waves = '';
    for (let i = 0; i < 70; i++) {
      const x = r() * w, y = r() * h;
      waves += `<path d="M${x} ${y} q6 -5 12 0 t12 0" />`;
    }
    let peaks = '';
    for (let i = 0; i < 60; i++) {
      const g = centers[Math.floor(r() * centers.length)];
      const x = g[0] * w + (r() - .5) * w * .18, y = g[1] * h + (r() - .5) * h * .2, s = 7 + r() * 7;
      peaks += `<path d="M${x - s} ${y} L${x} ${y - s * 1.3} L${x + s} ${y}"/><path d="M${x} ${y - s * 1.3} L${x + s * .35} ${y}" opacity=".55"/>`;
    }
    let trees = '';
    for (let i = 0; i < 80; i++) {
      const g = centers[Math.floor(r() * centers.length)];
      const x = g[0] * w + (r() - .5) * w * .2, y = g[1] * h + (r() - .5) * h * .22;
      trees += `<circle cx="${x}" cy="${y}" r="${2.5 + r() * 2}"/>`;
    }
    // roads between consecutive landmarks
    let roads = '';
    for (let i = 1; i < landmarks.length; i++) {
      const a = landmarks[i - 1].at, b = landmarks[i].at;
      const mx = (a[0] + b[0]) / 2 * w + (r() - .5) * 60, my = (a[1] + b[1]) / 2 * h + (r() - .5) * 60;
      roads += `<path d="M${a[0] * w} ${a[1] * h} Q${mx} ${my} ${b[0] * w} ${b[1] * h}"/>`;
    }
    const labels = regions.map(g => {
      const [lx, ly] = g.label || g.at;
      return `<text x="${lx * w}" y="${ly * h}" class="map-region">${g.name}</text>
      <text x="${lx * w}" y="${ly * h + 22}" class="map-sub">${g.sub}</text>`;
    }).join('');

    return `<svg class="worldmap" viewBox="0 0 ${w} ${h}" preserveAspectRatio="xMidYMid meet">
      <defs>
        <filter id="coast" x="-10%" y="-10%" width="120%" height="120%">
          <feTurbulence type="fractalNoise" baseFrequency=".012" numOctaves="4" seed="${seed}"/>
          <feDisplacementMap in="SourceGraphic" scale="70"/>
        </filter>
        <filter id="paper"><feTurbulence type="fractalNoise" baseFrequency=".8" numOctaves="3" seed="3"/>
          <feColorMatrix values="0 0 0 0 .35  0 0 0 0 .27  0 0 0 0 .16  0 0 0 .22 0"/><feComposite in2="SourceGraphic" operator="in"/></filter>
        <radialGradient id="burn" cx=".5" cy=".5" r=".75"><stop offset=".6" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#3b2412" stop-opacity=".75"/></radialGradient>
      </defs>
      <rect width="${w}" height="${h}" fill="#cbb88f"/>
      <g stroke="#6d5a3c" stroke-width="1" fill="none" opacity=".5">${waves}</g>
      <g filter="url(#coast)">
        <g fill="#7c6a4a" transform="translate(4 5)" opacity=".5">${blobs}</g>
        <g fill="#e2d3ae" stroke="#4a3b26" stroke-width="2">${blobs}</g>
      </g>
      <g stroke="#4a3b26" stroke-width="1.4" fill="none">${peaks}</g>
      <g fill="#5d6b46" opacity=".55">${trees}</g>
      <g stroke="#8f1d21" stroke-width="1.6" fill="none" stroke-dasharray="5 6" opacity=".75">${roads}</g>
      ${labels}
      <g class="compass" transform="translate(${w - 90} ${h - 100})" stroke="#4a3b26" fill="none" stroke-width="1.4">
        <circle r="34"/><circle r="26" stroke-dasharray="2 3"/>
        <path d="M0 -44 6 0 0 44 -6 0Z" fill="#4a3b26" fill-opacity=".25"/><path d="M-44 0 0 6 44 0 0 -6Z"/>
        <text y="-50" class="map-sub" text-anchor="middle">N</text>
      </g>
      <rect width="${w}" height="${h}" filter="url(#paper)"/>
      <rect width="${w}" height="${h}" fill="url(#burn)"/>
    </svg>`;
  }

  // ---------- wax seal ----------
  function waxSeal(sig = 'crown', size = 56) {
    return `<svg viewBox="0 0 64 64" width="${size}" height="${size}" class="wax">
      <defs><radialGradient id="wx" cx=".38" cy=".32" r=".8"><stop offset="0" stop-color="#c4383a"/><stop offset=".6" stop-color="#8f1d21"/><stop offset="1" stop-color="#4c0c10"/></radialGradient>
      <filter id="wxr"><feTurbulence baseFrequency=".09" numOctaves="2" seed="4"/><feDisplacementMap in="SourceGraphic" scale="5"/></filter></defs>
      <circle cx="32" cy="32" r="28" fill="url(#wx)" filter="url(#wxr)"/>
      <circle cx="32" cy="32" r="19" fill="none" stroke="#5e1014" stroke-width="2.2"/>
      <g transform="translate(17 17) scale(.47)" fill="none" stroke="#e8a59a" stroke-opacity=".55" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round">${SIGILS[sig]}</g>
    </svg>`;
  }

  // ---------- grain ----------
  const grainURI = `url("data:image/svg+xml;utf8,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="220" height="220"><filter id="n"><feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="3" stitchTiles="stitch"/><feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .55 0"/></filter><rect width="100%" height="100%" filter="url(#n)"/></svg>'
  )}")`;

  // single quotes so the value can sit inside a double-quoted style="" attribute
  const svgURI = svg => `url('data:image/svg+xml;utf8,${encodeURIComponent(svg).replace(/'/g, '%27')}')`;

  return { rng, landscape, sigil, worldMap, waxSeal, grainURI, svgURI, PALETTES, SIGILS };
})();
