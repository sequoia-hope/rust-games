// The demo's controls, curves, inspector, and Web Audio (mw_audio + music),
// around the client's canvas.
// The client (mw_demo) runs the lanes; this page hands it policies
// (glue.load_policy) and commands (glue.stage, JSON), and reads the selected
// lane's telemetry (G.telemetry, JSON, 10 Hz) and audio steps (G.audio, JSON,
// every render frame).
//
// Training data has one layout wherever it comes from: <base>runs.json lists
// the runs; <base><run>/index.json its checkpoints (or estimators);
// metrics.jsonl, and coverage_recent.json beside them. On the dev server
// <base> is the trainer's live checkpoints/ folder; in a build without it
// (the public site) it is the gallery copied into assets/policies/gallery/.

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
let run = null, runTeacher = null, index = [], mode = 'viewer';
const send = (o) => glue.stage(JSON.stringify(o));

export async function start(glue_, G_, params_, page_) {
  glue = glue_; G = G_; params = params_; page = page_;
  mode = params.get('mode') || 'viewer';
  wireCommon();
  wirePath();
  wireImpact();
  wireDamage();
  wireCalibration();
  wireAudioGestures();
  setMode(mode);
  await findData();
  setInterval(readTelemetry, 100);
  setInterval(() => { if (mode === 'viewer' && run) loadCurves(); }, 15000);
  requestAnimationFrame(pumpAudio);
  window.addEventListener('keydown', (e) => {
    if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT')) return;
    if (e.key === 'i' || e.key === 'I') toggleInspector();
    if (e.key === 'm' || e.key === 'M') toggleSfx();
    if (e.key === 'o' || e.key === 'O') toggleMusic();
  });
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
  if (m === 'damage' || m === 'calibration' || m === 'path') setInspector(true);
}

// ── Common controls ───────────────────────────────────────────────────────

function wireCommon() {
  for (const b of document.querySelectorAll('#modes button')) {
    b.onclick = () => { uiClick(0); setMode(b.dataset.mode); };
  }
  const walk = () => {
    $('fwd-v').textContent = (+$('fwd').value).toFixed(2) + ' m/s';
    $('yaw-v').textContent = (+$('yaw').value).toFixed(2) + ' rad/s';
    send({ cmd: 'walk', forward: +$('fwd').value, yaw: +$('yaw').value });
  };
  $('fwd').oninput = walk;
  $('yaw').oninput = walk;
  $('stop').onclick = () => { uiClick(0); $('fwd').value = 0; $('yaw').value = 0; walk(); };
  $('reset').onclick = () => { uiClick(0); send({ cmd: 'reset' }); };
  $('pause').onclick = () => {
    uiClick(0);
    $('pause').classList.toggle('on');
    send({ cmd: 'pause', paused: $('pause').classList.contains('on') });
  };
  $('push').onclick = () => {
    uiClick(0);
    const a = Math.random() * 2 * Math.PI;
    send({ cmd: 'push', x: 0.5 * Math.cos(a), z: 0.5 * Math.sin(a) });
  };
  $('speed').oninput = () => {
    $('speed-v').textContent = (+$('speed').value).toFixed(2) + '×';
    send({ cmd: 'speed', speed: +$('speed').value });
  };
  if ($('dust')) {
    $('dust').oninput = () => {
      $('dust-v').textContent = `${Math.round(100 * +$('dust').value)}%`;
      send({ cmd: 'weather', dust: +$('dust').value });
    };
  }
  if ($('toggle-sfx')) $('toggle-sfx').onclick = () => { toggleSfx(); uiClick(0); };
  if ($('toggle-music')) $('toggle-music').onclick = () => { toggleMusic(); uiClick(0); };
  $('toggle-inspector').onclick = () => { uiClick(0); toggleInspector(); };
  $('toggle-panel').onclick = () => {
    uiClick(0);
    $('panel').hidden = !$('panel').hidden;
    $('toggle-panel').classList.toggle('on', !$('panel').hidden);
  };
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
const SHIPPED = 'teacher-v3';
let shipped = null;
async function loadShipped(lane) {
  shipped ??= Promise.all(['.bin', '.estimator.bin'].map((ext) =>
    page.download(`assets/policies/${SHIPPED}${ext}`, () => {}).catch((e) => { console.warn(e); return null; })));
  const [policy, estimator] = await shipped;
  if (policy) glue.load_policy(lane, SHIPPED, policy);
  if (estimator) glue.load_policy(lane, SHIPPED + ' estimator', estimator);
  send({ cmd: 'adapt', lane, on: mode === 'path' || (mode === 'calibration' && $('cal-mode').value === 'autonomous') });
}

function fillRuns(runs) {
  const sel = $('run');
  sel.innerHTML = runs.map((r) => `<option>${r}</option>`).join('');
  sel.value = params.get('run') && runs.includes(params.get('run')) ? params.get('run') : runs[runs.length - 1];
  sel.onchange = () => openRun(sel.value);
  $('ckpt').oninput = () => { showCkpt(); applyLanes(); };
  $('lanes').onchange = applyLanes;
  $('spread').onclick = () => { uiClick(0); $('ckpt').value = $('ckpt').max; applyLanes(true); };
  openRun(sel.value);
}

async function openRun(name) {
  run = name;
  const meta = await getJSON(`${base}${run}/index.json`);
  index = meta.checkpoints || meta.estimators || [];
  runTeacher = meta.teacher || null;
  $('ckpt').max = String(Math.max(0, index.length - 1));
  $('ckpt').value = String(Math.max(0, index.length - 1));
  showCkpt();
  if (mode === 'viewer' && index.length) applyLanes(true);
  loadCurves();
}

function ckptLabel(c) {
  if (!c) return 'none';
  const m = c.steps >= 1e6 ? (c.steps / 1e6).toFixed(1) + 'M' : (c.steps / 1e3).toFixed(0) + 'k';
  return `${m} steps`;
}

function showCkpt() {
  const c = index[+$('ckpt').value];
  if (!c) return;
  const fallText = c.fall_rate !== undefined && c.fall_rate !== null
    ? ` · falls ${(100 * c.fall_rate).toFixed(0)}%`
    : (runTeacher ? ` · teacher ${runTeacher}` : '');
  $('ckpt-v').textContent = `${ckptLabel(c)}${fallText}`;
}

const blobs = new Map();
async function blob(runName, file) {
  const key = `${runName}/${file}`;
  if (!blobs.has(key)) blobs.set(key, page.download(`${base}${key}`, () => {}));
  return blobs.get(key);
}

async function loadTeacherFor(lane, teacherRun) {
  try {
    const tmeta = await getJSON(`${base}${teacherRun}/index.json`);
    const ckpts = tmeta.checkpoints || [];
    if (ckpts.length) {
      const last = ckpts[ckpts.length - 1];
      glue.load_policy(lane, teacherRun, await blob(teacherRun, last.file));
      return;
    }
  } catch { /* fall back to shipped */ }
  await loadShipped(lane);
}

async function loadInto(lane, c) {
  if (!c) return;
  try {
    if (runTeacher) {
      await loadTeacherFor(lane, runTeacher);
      glue.load_policy(lane, `${run} (${ckptLabel(c)})`, await blob(run, c.file));
      send({ cmd: 'adapt', lane, on: true });
    } else {
      glue.load_policy(lane, ckptLabel(c), await blob(run, c.file));
      send({ cmd: 'adapt', lane, on: false });
    }
  } catch (e) { console.warn(e); }
}

// The lanes: one at the chosen checkpoint, or spread from the first to it.
let spread = true;
async function applyLanes(spreadNow) {
  if (spreadNow !== undefined) spread = spreadNow;
  if (!index.length) return;
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
  if (!rows.length) return;
  const at = (k) => rows.map((r) => [r.steps, typeof k === 'function' ? k(r) : r[k]]).filter((p) => p[1] !== null && p[1] !== undefined && !Number.isNaN(p[1]));
  const last = rows[rows.length - 1];
  const isEstimator = last.latent_mse !== undefined;
  $('curves-note').textContent = last
    ? `${(last.steps / 1e6).toFixed(1)}M steps, ${(last.time / 60).toFixed(0)} min${isEstimator ? ' (estimator DAgger)' : ''}`
    : '';
  const charts = isEstimator ? [
    ['latent MSE', at('latent_mse'), '#ea851f'],
    ['body param MSE', at('params_mse'), '#6cc0e5'],
    ['fall rate', at('fall_rate'), '#e5603b', [0, 1]],
    ['speed tracking', at('track'), '#7fc97f', [0, 1]],
    ['student share', at('student_share'), '#9ab8e0', [0, 1]],
  ] : [
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
  const val = pts[pts.length - 1][1];
  ctx.fillText(`${title}  ${val < 0.1 && val > 0 ? val.toFixed(3) : val.toFixed(2)}`, 6, 12);
}

async function loadCoverage() {
  const targetRun = runTeacher || run;
  let eps;
  try { eps = (await getJSON(`${base}${targetRun}/coverage_recent.json`)).episodes; } catch { return; }
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

function wirePath() {
  if (!$('pdens')) return;
  const upd = (rebuild = false) => {
    $('pdens-v').textContent = (+$('pdens').value).toFixed(2);
    $('pscale-v').textContent = (+$('pscale').value).toFixed(2) + '×';
    $('prough-v').textContent = (+$('prough').value).toFixed(2) + ' m';
    send({
      cmd: 'path',
      density: +$('pdens').value,
      scale: +$('pscale').value,
      roughness: +$('prough').value,
      rebuild,
    });
  };
  for (const id of ['pdens', 'pscale', 'prough']) $(id).oninput = () => upd(false);
  $('prebuild').onclick = () => { uiClick(0); upd(true); };
  $('pdamage').onclick = () => {
    uiClick(1);
    const legJoints = [2, 3, 4, 8, 9, 10];
    const joint = legJoints[Math.floor(Math.random() * legJoints.length)];
    send({ cmd: 'damage', joint, health: -0.5, driver: -0.5 });
  };
}

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
  $('fire').onclick = () => { uiClick(0); send({ cmd: 'fire' }); };
  upd();
}

function wireDamage() {
  $('joint').innerHTML = Array.from({ length: 12 }, (_, j) => `<option value="${j}">${jointLabel(j)}</option>`).join('');
  $('joint').value = '3';
  const upd = () => { $('health-v').textContent = `${Math.round(100 * (0.6 + 0.4 * +$('health').value))}% torque`; };
  $('health').oninput = upd;
  upd();
  $('damage').onclick = () => {
    uiClick(1);
    send({ cmd: 'damage', joint: +$('joint').value, health: +$('health').value, ...($('weak-driver').checked ? { driver: -0.6 } : {}) });
  };
  $('heat').onclick = () => {
    uiClick(1);
    send({ cmd: 'damage', joint: +$('joint').value, health: 1, heat: 112 });
  };
  $('repair').onclick = () => { uiClick(0); send({ cmd: 'repair' }); };
}

function wireCalibration() {
  const upd = () => { $('cal-v').textContent = (+$('cal').value).toFixed(1) + ' s'; };
  $('cal').oninput = upd;
  upd();
  $('swap').onclick = () => {
    uiClick(2);
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
  $('calibrate').onclick = () => { uiClick(0); send({ cmd: 'calibrate', secs: +$('cal').value }); };
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
function syncSlider(id, valId, val, fmt) {
  const el = $(id);
  if (!el || val === undefined || val === null || document.activeElement === el) return;
  if (Math.abs(+el.value - val) > 1e-3) {
    el.value = String(val);
    if (valId && $(valId)) $(valId).textContent = fmt(val);
  }
}

function readTelemetry() {
  if (!G.telemetry) return;
  let t;
  try { t = JSON.parse(G.telemetry); } catch { return; }
  lastTelemetry = t;
  if (G.hits !== undefined) $('hits').textContent = G.hits;
  if (t.obstacles !== undefined && $('pobs')) $('pobs').textContent = t.obstacles;

  // Keep UI sliders/buttons synced with staged commands and keyboard keys.
  syncSlider('fwd', 'fwd-v', t.forward, (v) => v.toFixed(2) + ' m/s');
  syncSlider('yaw', 'yaw-v', t.yaw, (v) => v.toFixed(2) + ' rad/s');
  syncSlider('speed', 'speed-v', t.speedScale, (v) => v.toFixed(2) + '×');
  syncSlider('dust', 'dust-v', t.dust, (v) => `${Math.round(100 * v)}%`);
  if (t.paused !== undefined && $('pause')) $('pause').classList.toggle('on', !!t.paused);

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

  // 5×3 terrain height scan (heading frame: 5 longitudinal × 3 lateral).
  if ($('scan') && Array.isArray(t.scan) && t.scan.length === 15) {
    const cells = t.scan.map((dh) => {
      const cm = Math.round(dh * 100);
      const hi = Math.max(0, Math.min(1, Math.abs(dh) / 0.35));
      const bg = dh > 0.03
        ? `rgba(234, 133, 31, ${(0.18 + 0.65 * hi).toFixed(2)})`
        : dh < -0.03
          ? `rgba(108, 192, 229, ${(0.15 + 0.55 * hi).toFixed(2)})`
          : 'rgba(255, 255, 255, 0.05)';
      return `<span style="background:${bg}">${cm >= 0 ? '+' + cm : cm}cm</span>`;
    });
    // Render rows from left (-Z) to right (+Z), columns from back (-0.35 m) to ahead (+1.25 m).
    const ordered = [];
    for (let iz = 0; iz < 3; iz++) {
      for (let ix = 0; ix < 5; ix++) ordered.push(cells[ix * 3 + iz]);
    }
    $('scan').innerHTML = '<div style="margin-top:4px">height scan (5×3 ahead, relative to support foot):</div>'
      + `<div class="scan-grid">${ordered.join('')}</div>`;
  }

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

// ── Web Audio: mw_audio (physics-driven SFX) + music (orchestral score) ───

const SCORE_STATIONS = JSON.stringify([{
  key: 'orchestral',
  name: 'Walker Score',
  freq: 'SCORE',
  tagline: 'Procedural orchestral score',
  dj: '',
  genres: [['orchestral', 1.0]],
}]);

let audioCtx = null;
let sfxOn = false, sfxReady = false, sfxPort = null, sfxFallback = null, sfxGain = null;
let musicOn = false, musicReady = false, musicPort = null, musicFallback = null, musicGain = null;
let lastAudioSeq = 0;

function ensureAudioCtx() {
  if (!audioCtx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    audioCtx = new AC({ latencyHint: 'interactive' });
  }
  if (audioCtx.state === 'suspended') audioCtx.resume().catch(() => {});
  return audioCtx;
}

function wireAudioGestures() {
  // Auto-enable physics SFX on the user's first gesture on the page.
  const wake = () => {
    window.removeEventListener('pointerdown', wake);
    window.removeEventListener('keydown', wake);
    if (!sfxOn) toggleSfx(true);
  };
  window.addEventListener('pointerdown', wake, { passive: true });
  window.addEventListener('keydown', wake, { passive: true });
}

async function initSfx() {
  if (sfxReady) return;
  sfxReady = true;
  const ctx = ensureAudioCtx();
  if (!ctx) return;
  sfxGain = ctx.createGain();
  sfxGain.gain.value = sfxOn ? 0.85 : 0.0;
  sfxGain.connect(ctx.destination);

  try {
    const wasmBytes = await (await fetch('mw_audio.wasm')).arrayBuffer();
    if (ctx.audioWorklet) {
      await ctx.audioWorklet.addModule('mw-audio-worklet.js');
      const node = new AudioWorkletNode(ctx, 'mw-sfx', {
        numberOfInputs: 0,
        numberOfOutputs: 1,
        outputChannelCount: [2],
      });
      node.connect(sfxGain);
      node.port.postMessage({ wasmBytes }, [wasmBytes]);
      sfxPort = node.port;
      return;
    }
    // Fallback for non-secure (http://) origins without AudioWorklet:
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    const wasm = instance.exports;
    wasm.mw_audio_init(ctx.sampleRate);
    const proc = ctx.createScriptProcessor(512, 0, 2);
    const frames = 512;
    const ptr = wasm.mw_audio_alloc_f32(frames * 2);
    proc.onaudioprocess = (ev) => {
      const L = ev.outputBuffer.getChannelData(0);
      const R = ev.outputBuffer.getChannelData(1);
      if (!sfxOn) { L.fill(0); R.fill(0); return; }
      wasm.mw_audio_render(ptr, frames);
      const view = new Float32Array(wasm.memory.buffer, ptr, frames * 2);
      for (let i = 0; i < frames; i++) {
        L[i] = view[i * 2];
        R[i] = view[i * 2 + 1];
      }
    };
    proc.connect(sfxGain);
    sfxFallback = wasm;
  } catch (e) {
    console.warn('mw_audio init failed:', e);
  }
}

async function initMusic() {
  if (musicReady) return;
  musicReady = true;
  const ctx = ensureAudioCtx();
  if (!ctx) return;
  musicGain = ctx.createGain();
  musicGain.gain.value = musicOn ? 0.42 : 0.0;
  musicGain.connect(ctx.destination);

  try {
    const wasmBytes = await (await fetch('music.wasm')).arrayBuffer();
    if (ctx.audioWorklet) {
      await ctx.audioWorklet.addModule('music-worklet.js');
      const node = new AudioWorkletNode(ctx, 'music-radio', {
        numberOfInputs: 0,
        numberOfOutputs: 1,
        outputChannelCount: [2],
      });
      node.connect(musicGain);
      node.port.postMessage({ wasmBytes, stationsJson: SCORE_STATIONS }, [wasmBytes]);
      node.port.postMessage({ station: 0 });
      node.port.postMessage({ playing: musicOn });
      musicPort = node.port;
      return;
    }
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    const wasm = instance.exports;
    const enc = new TextEncoder().encode(SCORE_STATIONS);
    const jptr = wasm.music_alloc_bytes(enc.length);
    new Uint8Array(wasm.memory.buffer, jptr, enc.length).set(enc);
    wasm.music_init_stations(ctx.sampleRate, ((Date.now() & 0xffffffff) >>> 0) || 1, jptr, enc.length);
    wasm.music_set_station(0);
    const proc = ctx.createScriptProcessor(1024, 0, 2);
    const frames = 1024;
    const ptr = wasm.music_alloc_f32(frames * 2);
    proc.onaudioprocess = (ev) => {
      const L = ev.outputBuffer.getChannelData(0);
      const R = ev.outputBuffer.getChannelData(1);
      if (!musicOn) { L.fill(0); R.fill(0); return; }
      wasm.music_render(ptr, frames);
      const view = new Float32Array(wasm.memory.buffer, ptr, frames * 2);
      for (let i = 0; i < frames; i++) {
        L[i] = view[i * 2];
        R[i] = view[i * 2 + 1];
      }
    };
    proc.connect(musicGain);
    musicFallback = wasm;
  } catch (e) {
    console.warn('music init failed:', e);
  }
}

function toggleSfx(force) {
  sfxOn = force !== undefined ? force : !sfxOn;
  if ($('toggle-sfx')) $('toggle-sfx').classList.toggle('on', sfxOn);
  if (sfxOn) initSfx();
  if (sfxGain && audioCtx) {
    sfxGain.gain.setTargetAtTime(sfxOn ? 0.85 : 0.0, audioCtx.currentTime, 0.02);
  }
}

function toggleMusic(force) {
  musicOn = force !== undefined ? force : !musicOn;
  if ($('toggle-music')) $('toggle-music').classList.toggle('on', musicOn);
  if (musicOn) initMusic();
  if (musicPort) musicPort.postMessage({ playing: musicOn });
  if (musicGain && audioCtx) {
    musicGain.gain.setTargetAtTime(musicOn ? 0.42 : 0.0, audioCtx.currentTime, 0.04);
  }
}

function uiClick(kind = 0) {
  if (!sfxOn) return;
  if (sfxPort) sfxPort.postMessage({ ui: kind });
  else if (sfxFallback && sfxFallback.mw_audio_push_ui) sfxFallback.mw_audio_push_ui(kind);
}

function pushFallbackStep(wasm, s) {
  const ptr = wasm.mw_audio_step_buf_ptr();
  const buf = new Float32Array(wasm.memory.buffer, ptr, 65);
  buf[0] = s.dt ?? 0.002;
  buf[1] = s.body_mass ?? 4200;
  buf[2] = s.foot_force ? s.foot_force[0] : 0;
  buf[3] = s.foot_force ? s.foot_force[1] : 0;
  buf[4] = s.foot_slip ? s.foot_slip[0] : 0;
  buf[5] = s.foot_slip ? s.foot_slip[1] : 0;
  buf[6] = s.foot_on && s.foot_on[0] ? 1 : 0;
  buf[7] = s.foot_on && s.foot_on[1] ? 1 : 0;
  for (let j = 0; j < 12; j++) {
    buf[8 + j] = s.torque ? s.torque[j] : 0;
    buf[20 + j] = s.qd ? s.qd[j] : 0;
    buf[32 + j] = s.temp ? s.temp[j] : 30;
    buf[44 + j] = (s.tripped && (s.tripped & (1 << j))) ? 1 : 0;
  }
  buf[56] = s.at_limit_mask ?? 0;
  buf[57] = s.overcurrent_mask ?? 0;
  buf[58] = s.fell ? 1 : 0;
  buf[59] = s.torso_acc ?? 0;
  buf[60] = s.torso_omega ?? 0;
  buf[61] = s.wind ?? 0.25;
  const imp = s.impacts && s.impacts.length ? s.impacts[0] : null;
  buf[62] = imp ? imp[0] : 0;
  buf[63] = imp ? imp[1] : 0;
  buf[64] = imp ? imp[2] : 0;
  wasm.mw_audio_push_step();
}

function pumpAudio() {
  requestAnimationFrame(pumpAudio);
  if (!sfxOn || !G || !G.audio) return;
  let pkt;
  try { pkt = JSON.parse(G.audio); } catch { return; }
  if (!pkt || !pkt.seq || pkt.seq === lastAudioSeq || !Array.isArray(pkt.steps)) return;
  lastAudioSeq = pkt.seq;
  if (sfxPort) {
    sfxPort.postMessage({ steps: pkt.steps });
  } else if (sfxFallback) {
    for (const s of pkt.steps) pushFallbackStep(sfxFallback, s);
  }
}
