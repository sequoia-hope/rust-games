// The demo's controls, curves and inspector, around the client's canvas.
// The client (mw_demo) runs the lanes; this page hands it policies
// (glue.load_policy) and commands (glue.stage, JSON), and reads the selected
// lane's telemetry from the status global (G.telemetry, JSON, 10 Hz).
//
// Training data has one layout wherever it comes from: <base>runs.json lists
// the runs; <base><run>/index.json its checkpoints; metrics.jsonl, and
// coverage_recent.json beside them. On the dev server <base> is the
// trainer's live checkpoints/ folder; in a build without it (the public
// site) it is the gallery copied into assets/policies/gallery/.

const $ = (id) => document.getElementById(id);
const JOINT_NAMES = ['hip yaw', 'hip roll', 'hip pitch', 'knee', 'ankle pitch', 'ankle roll'];
const jointLabel = (j) => `${j < 6 ? 'L' : 'R'} ${JOINT_NAMES[j % 6]}`;
const LINK_NAMES = ['torso', ...[0, 1].flatMap((leg) => ['hip', 'hip', 'thigh', 'shin', 'ankle', 'foot'].map((n) => `${leg ? 'R' : 'L'} ${n}`))];
const PARAM_NAMES = ['torso mass', 'CoM fwd', 'CoM up', 'CoM right', 'L leg length', 'R leg length',
  ...Array.from({ length: 12 }, (_, j) => `health ${jointLabel(j)}`),
  ...Array.from({ length: 12 }, (_, j) => `driver ${jointLabel(j)}`), 'friction', 'latency',
  'tool mass', 'tool across', 'tool along'];

let glue, G, params, page;
let base = null;          // where training data comes from
let run = null, index = [], mode = 'viewer';
const send = (o) => glue.stage(JSON.stringify(o));

export async function start(glue_, G_, params_, page_) {
  glue = glue_; G = G_; params = params_; page = page_;
  mode = params.get('mode') || 'viewer';
  wireCommon();
  wireImpact();
  wireDamage();
  wireCalibration();
  setMode(mode);
  await findData();
  setInterval(readTelemetry, 100);
  setInterval(() => { if (mode === 'viewer' && run) loadCurves(); }, 15000);
  window.addEventListener('keydown', (e) => { if (e.key === 'i' || e.key === 'I') toggleInspector(); });
}

function setMode(m) {
  mode = m;
  for (const b of document.querySelectorAll('#modes button')) b.classList.toggle('on', b.dataset.mode === m);
  for (const s of document.querySelectorAll('#panel section[data-for]')) s.hidden = s.dataset.for !== m;
  send({ cmd: 'mode', mode: m });
  const q = new URLSearchParams(location.search);
  q.set('mode', m);
  history.replaceState(null, '', location.pathname + '?' + q.toString());
  if (m !== 'viewer') {
    // One mech, on the shipped policy and its estimator.
    $('lanes').value = '1';
    send({ cmd: 'lanes', n: 1 });
    loadShipped(0);
  } else if (index.length) {
    applyLanes();
  }
  if (m === 'damage' || m === 'calibration') setInspector(true);
}

// ── Common controls ───────────────────────────────────────────────────────

function wireCommon() {
  for (const b of document.querySelectorAll('#modes button')) b.onclick = () => setMode(b.dataset.mode);
  const walk = () => {
    $('fwd-v').textContent = (+$('fwd').value).toFixed(2) + ' m/s';
    $('yaw-v').textContent = (+$('yaw').value).toFixed(2) + ' rad/s';
    send({ cmd: 'walk', forward: +$('fwd').value, yaw: +$('yaw').value });
  };
  $('fwd').oninput = walk;
  $('yaw').oninput = walk;
  $('stop').onclick = () => { $('fwd').value = 0; $('yaw').value = 0; walk(); };
  $('reset').onclick = () => send({ cmd: 'reset' });
  $('pause').onclick = () => { $('pause').classList.toggle('on'); send({ cmd: 'pause', paused: $('pause').classList.contains('on') }); };
  $('push').onclick = () => { const a = Math.random() * 2 * Math.PI; send({ cmd: 'push', x: 0.5 * Math.cos(a), z: 0.5 * Math.sin(a) }); };
  $('speed').oninput = () => { $('speed-v').textContent = (+$('speed').value).toFixed(2) + '×'; send({ cmd: 'speed', speed: +$('speed').value }); };
  $('toggle-inspector').onclick = toggleInspector;
  $('toggle-panel').onclick = () => { $('panel').hidden = !$('panel').hidden; $('toggle-panel').classList.toggle('on', !$('panel').hidden); };
  $('toggle-panel').classList.add('on');
  walk();
}

let inspectorOn = false;
function setInspector(on) {
  inspectorOn = on;
  $('inspector').hidden = !on;
  $('toggle-inspector').classList.toggle('on', on);
  send({ cmd: 'inspector', on });
}
const toggleInspector = () => setInspector(!inspectorOn);

// ── Training data ─────────────────────────────────────────────────────────

async function getJSON(url) {
  const r = await fetch(url, { cache: 'no-store' });
  if (!r.ok) throw new Error(`${url}: ${r.status}`);
  return r.json();
}

async function findData() {
  // Below dist/<build>/ on the dev server (the page as / or /index.html there).
  const up = /\/dist\/[^/]+\/(index\.html)?$/.test(location.pathname) ? '../../' : '';
  for (const b of [up + 'checkpoints/', 'assets/policies/gallery/']) {
    try {
      const runs = (await getJSON(b + 'runs.json')).runs;
      if (runs.length) { base = b; fillRuns(runs); return; }
    } catch { /* try the next */ }
  }
  $('curves-note').textContent = 'no training data found';
  if (mode === 'viewer') loadShipped(0);
}

// The shipped policy and the estimator distilled from it: a pair (an
// estimator guesses its own teacher's latent, no other's).
const SHIPPED = 'teacher-v2';
let shipped = null;
async function loadShipped(lane) {
  shipped ??= Promise.all(['.bin', '.estimator.bin'].map((ext) =>
    page.download(`assets/policies/${SHIPPED}${ext}`, () => {}).catch((e) => { console.warn(e); return null; })));
  const [policy, estimator] = await shipped;
  if (policy) glue.load_policy(lane, SHIPPED, policy);
  if (estimator) glue.load_policy(lane, SHIPPED + ' estimator', estimator);
  send({ cmd: 'adapt', lane, on: mode === 'calibration' && $('cal-mode').value === 'autonomous' });
}

function fillRuns(runs) {
  const sel = $('run');
  sel.innerHTML = runs.map((r) => `<option>${r}</option>`).join('');
  sel.value = params.get('run') && runs.includes(params.get('run')) ? params.get('run') : runs[runs.length - 1];
  sel.onchange = () => openRun(sel.value);
  $('ckpt').oninput = () => { showCkpt(); applyLanes(); };
  $('lanes').onchange = applyLanes;
  $('spread').onclick = () => { $('ckpt').value = $('ckpt').max; applyLanes(true); };
  openRun(sel.value);
}

async function openRun(name) {
  run = name;
  index = (await getJSON(`${base}${run}/index.json`)).checkpoints;
  $('ckpt').max = String(index.length - 1);
  $('ckpt').value = String(index.length - 1);
  showCkpt();
  if (mode === 'viewer') applyLanes(true);
  loadCurves();
}

function ckptLabel(c) {
  const m = c.steps >= 1e6 ? (c.steps / 1e6).toFixed(1) + 'M' : (c.steps / 1e3).toFixed(0) + 'k';
  return `${m} steps`;
}

function showCkpt() {
  const c = index[+$('ckpt').value];
  if (c) $('ckpt-v').textContent = `${ckptLabel(c)} · falls ${(100 * c.fall_rate).toFixed(0)}%`;
}

const blobs = new Map();
async function blob(file) {
  if (!blobs.has(file)) blobs.set(file, page.download(`${base}${run}/${file}`, () => {}));
  return blobs.get(file);
}

async function loadInto(lane, c) {
  try {
    glue.load_policy(lane, ckptLabel(c), await blob(c.file));
  } catch (e) { console.warn(e); }
}

// The lanes: one at the chosen checkpoint, or spread from the first to it.
let spread = true;
async function applyLanes(spreadNow) {
  if (spreadNow !== undefined) spread = spreadNow;
  const n = +$('lanes').value;
  send({ cmd: 'lanes', n });
  const last = +$('ckpt').value;
  const picks = [];
  for (let i = 0; i < n; i++) {
    const k = n === 1 || !spread ? last : Math.round((last * i) / (n - 1));
    picks.push(index[k]);
  }
  $('lane-labels').textContent = picks.map((c, i) => `lane ${i + 1}: ${ckptLabel(c)}`).join(' · ');
  await Promise.all(picks.map((c, i) => loadInto(i, c)));
  send({ cmd: 'reset' });
}

// ── Curves ────────────────────────────────────────────────────────────────

async function loadCurves() {
  let rows;
  try {
    const r = await fetch(`${base}${run}/metrics.jsonl`, { cache: 'no-store' });
    rows = (await r.text()).trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));
  } catch { return; }
  const at = (k) => rows.map((r) => [r.steps, typeof k === 'function' ? k(r) : r[k]]).filter((p) => p[1] !== null && p[1] !== undefined && !Number.isNaN(p[1]));
  const last = rows[rows.length - 1];
  $('curves-note').textContent = last ? `${(last.steps / 1e6).toFixed(1)}M steps, ${(last.time / 60).toFixed(0)} min` : '';
  const charts = [
    ['episode reward', at('reward'), '#ea851f'],
    ['episode length (s)', at((r) => (r.ep_len === null ? null : r.ep_len / 50)), '#f6c27a'],
    ['fall rate', at('fall_rate'), '#e5603b', [0, 1]],
    ['speed tracking', at('track'), '#7fc97f', [0, 1]],
    ['curriculum level', at('level'), '#9ab8e0', [0, 1]],
  ];
  const box = $('curves');
  if (box.children.length !== charts.length) {
    box.innerHTML = charts.map(() => '<canvas width="300" height="70"></canvas>').join('');
  }
  charts.forEach(([title, pts, color, range], i) => drawChart(box.children[i], title, pts, color, range));
  loadCoverage();
}

function smooth(pts, n) {
  if (pts.length < n * 2) return pts;
  const out = [];
  let s = 0;
  for (let i = 0; i < pts.length; i++) {
    s += pts[i][1];
    if (i >= n) s -= pts[i - n][1];
    if (i >= n - 1) out.push([pts[i][0], s / n]);
  }
  return out;
}

function drawChart(cv, title, pts, color, range) {
  const ctx = cv.getContext('2d');
  const W = cv.width, H = cv.height, pad = 4;
  ctx.clearRect(0, 0, W, H);
  ctx.fillStyle = '#a39d94';
  ctx.font = '11px system-ui';
  if (!pts.length) { ctx.fillText(title + ': no data yet', 6, 14); return; }
  pts = smooth(pts, 5);
  const x0 = pts[0][0], x1 = pts[pts.length - 1][0] || 1;
  let lo = range ? range[0] : Math.min(...pts.map((p) => p[1]));
  let hi = range ? range[1] : Math.max(...pts.map((p) => p[1]));
  if (hi - lo < 1e-6) { hi += 1; lo -= 1; }
  const X = (x) => pad + ((x - x0) / Math.max(1, x1 - x0)) * (W - 2 * pad);
  const Y = (y) => H - pad - ((y - lo) / (hi - lo)) * (H - 2 * pad - 14);
  // Checkpoint marker.
  const c = index[+$('ckpt').value];
  if (c && mode === 'viewer') { ctx.fillStyle = '#ffffff22'; ctx.fillRect(X(c.steps) - 1, 14, 2, H - 14); }
  ctx.strokeStyle = color; ctx.lineWidth = 1.5; ctx.beginPath();
  pts.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))));
  ctx.stroke();
  ctx.fillStyle = '#ece9e4';
  ctx.fillText(`${title}  ${pts[pts.length - 1][1].toFixed(2)}`, 6, 12);
}

async function loadCoverage() {
  let eps;
  try { eps = (await getJSON(`${base}${run}/coverage_recent.json`)).episodes; } catch { return; }
  const cv = $('coverage'), ctx = cv.getContext('2d');
  const W = cv.width, H = cv.height;
  ctx.clearRect(0, 0, W, H);
  ctx.strokeStyle = '#333'; ctx.beginPath(); ctx.moveTo(W / 2, 0); ctx.lineTo(W / 2, H); ctx.moveTo(0, H / 2); ctx.lineTo(W, H / 2); ctx.stroke();
  for (const e of eps) {
    const x = ((e.p[0] + 1) / 2) * W, y = H - ((e.p[1] + 1) / 2) * H;
    ctx.fillStyle = e.fell ? '#e5603bcc' : '#7fc97f88';
    ctx.fillRect(x - 1.5, y - 1.5, 3, 3);
  }
  ctx.fillStyle = '#a39d94'; ctx.font = '10px system-ui';
  ctx.fillText('lighter ← torso mass → heavier', 6, H - 4);
  ctx.save(); ctx.translate(10, H - 30); ctx.rotate(-Math.PI / 2); ctx.fillText('CoM back → forward', 0, 0); ctx.restore();
}

// ── Demos ─────────────────────────────────────────────────────────────────

function wireImpact() {
  $('target').innerHTML += LINK_NAMES.map((n, i) => `<option value="${i}">${n}</option>`).join('');
  const upd = () => {
    $('mass-v').textContent = $('mass').value + ' kg';
    $('pspeed-v').textContent = $('pspeed').value + ' m/s';
    $('rate-v').textContent = (+$('rate').value).toFixed(1) + ' /s';
    const t = $('target').value;
    send({ cmd: 'range', mass: +$('mass').value, speed: +$('pspeed').value, rate: +$('rate').value, target: t === '' ? null : +t });
  };
  for (const id of ['mass', 'pspeed', 'rate']) $(id).oninput = upd;
  $('target').onchange = upd;
  $('fire').onclick = () => send({ cmd: 'fire' });
  upd();
}

function wireDamage() {
  $('joint').innerHTML = Array.from({ length: 12 }, (_, j) => `<option value="${j}">${jointLabel(j)}</option>`).join('');
  $('joint').value = '3';
  const upd = () => { $('health-v').textContent = `${Math.round(100 * (0.6 + 0.4 * +$('health').value))}% torque`; };
  $('health').oninput = upd;
  upd();
  $('damage').onclick = () => send({ cmd: 'damage', joint: +$('joint').value, health: +$('health').value, ...($('weak-driver').checked ? { driver: -0.6 } : {}) });
  $('heat').onclick = () => send({ cmd: 'damage', joint: +$('joint').value, health: 1, heat: 112 });
  $('repair').onclick = () => send({ cmd: 'repair' });
}

function wireCalibration() {
  const upd = () => { $('cal-v').textContent = (+$('cal').value).toFixed(1) + ' s'; };
  $('cal').oninput = upd;
  upd();
  $('swap').onclick = () => {
    const t = lastTelemetry;
    if (!t) return;
    const p = t.params.slice();
    const part = $('part').value;
    if (part === 'heavy') p[0] = 1;
    if (part === 'light') p[0] = -1;
    if (part === 'com') p[1] = 1;
    if (part === 'shin') p[4] = 1;
    if (part === 'tool-right') { p[32] = 1; p[33] = 1; p[34] = 0.4; }
    if (part === 'tool-left') { p[32] = 0.3; p[33] = -1; p[34] = 0.4; }
    if (part === 'no-tool') p[32] = -1;
    send({ cmd: 'swap', params: p });
  };
  $('calibrate').onclick = () => send({ cmd: 'calibrate', secs: +$('cal').value });
  const calMode = () => {
    const auto = $('cal-mode').value === 'autonomous';
    for (const el of document.querySelectorAll('.explicit-only')) el.hidden = auto;
    for (const el of document.querySelectorAll('.autonomous-only')) el.hidden = !auto;
    send({ cmd: 'adapt', on: auto });
  };
  $('cal-mode').onchange = calMode;
  calMode();
}

// ── Telemetry: the HUD and the inspector ──────────────────────────────────

let lastTelemetry = null;
function readTelemetry() {
  if (!G.telemetry) return;
  let t;
  try { t = JSON.parse(G.telemetry); } catch { return; }
  lastTelemetry = t;
  if (G.hits !== undefined) $('hits').textContent = G.hits;
  const lane = t.lanes[t.lane];
  $('hud').innerHTML = `<b>${(t.speed).toFixed(2)}</b> m/s · height <b>${t.height.toFixed(2)}</b> m`
    + ` · falls <b>${t.lanes.map((l) => l.falls).join(' / ')}</b>`
    + (lane && lane.label ? ` · ${lane.label}` : (lane && !lane.policy ? ' · no policy: holding the pose' : ''))
    + (t.calibrating !== null && t.calibrating !== undefined ? ` · calibrating ${(100 * t.calibrating).toFixed(0)}%` : '')
    + (t.adaptive ? ' · walking on the estimator' : '');
  if (inspectorOn) drawInspector(t);
}

function meter(frac, cls = '', cap = null, marks = []) {
  const f = Math.max(0, Math.min(1, frac));
  return `<div class="meter ${cls}"><i style="width:${(100 * f).toFixed(1)}%"></i>`
    + (cap !== null ? `<i class="cap" style="width:${(100 * Math.min(1, cap)).toFixed(1)}%"></i>` : '')
    + marks.map((m) => `<span class="mark" style="left:${(100 * m).toFixed(1)}%"></span>`).join('') + '</div>';
}

function drawInspector(t) {
  const rows = ['<div class="jrow head"><span>actuator</span><span>torque / limit</span><span>current</span><span>winding °C</span><span></span></div>'];
  t.joints.forEach((j, i) => {
    const tq = Math.abs(j.torque) / j.peak, lim = j.limit / j.peak;
    const cur = Math.abs(j.current) / j.maxCurrent;
    // 30 °C ambient to 130: derating starts at 90, trips at 120.
    const temp = (j.temp - 30) / 100;
    const tag = j.tripped ? '<span class="tag trip">TRIP</span>' : j.fault ? '<span class="tag fault">FAULT</span>'
      : `<span class="tag ok">${Math.round(100 * j.health)}%</span>`;
    rows.push(`<div class="jrow"><span>${jointLabel(i)}</span>${meter(tq, '', lim)}${meter(cur)}${meter(temp, 'temp', null, [0.6, 0.9])}${tag}</div>`);
  });
  $('joints').innerHTML = rows.join('');
  $('feet').innerHTML = 'feet: ' + t.feet.map((f, i) => `${i ? 'R' : 'L'} ${f.on ? (f.force / 1000).toFixed(1) + ' kN' : 'in the air'}`).join(' · ');
  // Body parameters: the truth (white), what the policy is told (orange)
  // and the estimator's guess from the motion (blue).
  const est = t.estimated;
  const off = (i) => t.params[i] < 0.999 || t.believed[i] < 0.999 || (est && est[i] < 0.8);
  const show = [0, 1, 2, 3, 4, 5, ...t.params.map((_, i) => i).filter((i) => i >= 6 && i < 30 && off(i)), 30, 31, 32, 33, 34];
  const at = (x) => (50 + 50 * Math.max(-1.1, Math.min(1.1, x))).toFixed(1);
  $('params').innerHTML = '<div style="margin:4px 0">body parameters: <span style="color:#f4f0e8">true</span> · '
    + `<span style="color:var(--accent)">${t.adaptive ? 'told (unused)' : 'what the policy is told'}</span>`
    + (est ? ' · <span style="color:var(--est)">estimated from the motion</span>' : '') + '</div>'
    + show.map((i) => `<div class="prow"><span>${PARAM_NAMES[i]}</span><div class="pbar"><span class="mid"></span>`
      + `<i class="true" style="left:${at(t.params[i])}%"></i><i class="told" style="left:${at(t.believed[i])}%"></i>`
      + (est ? `<i class="est" style="left:${at(est[i])}%"></i>` : '') + '</div></div>').join('');
}
