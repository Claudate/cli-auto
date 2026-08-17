/**
 * [INPUT]: features/chat/chatThinkingOrb（orbs.jakubantalik.com 引擎 · MIT）
 * [OUTPUT]: start/stop 思考 orb —— 9 状态点阵球画布引擎（orbits/globe/rubik/wave/web/braid/ribbon/ring/morph）
 * [POS]: shared/thinkingOrb.js（MVVM 展示组件；无 IPC · 无业务策略 · 不进 facade）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 移植来源：Jakub Antalik · thinking-orbs（MIT）
 *   https://orbs.jakubantalik.com · https://github.com/Jakubantalik/thinking-orbs
 * 纯移植剥离 React；数学与配置未改。isDark 仅翻转墨色亮度。
 * 默认 rAF 自转；prefers-reduced-motion 时退化成单帧静态图。
 * 暗色判定：优先显式 `dark` 选项 → 祖先 data-theme/class 标记 → 默认浅色。
 * 不回退 prefers-color-scheme（本 app 纯浅色；避免 OS 深色时白点画浅底不可见）。
 */
const ORB_SIZE = 32; // 与 css/chat.css .chat-orb canvas 同步（参考站 20px 预设）
const ORB_DPR = 2; // 画布 DPR 上限

/** 平滑值噪声：伪随机格子 + 双线性插值（braid/web/orbits 需要） */
function _smoothNoise(x, y) {
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const fx = x - x0;
  const fy = y - y0;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const a = _hash(x0, y0);
  const b = _hash(x0 + 1, y0);
  const c = _hash(x0, y0 + 1);
  const d = _hash(x0 + 1, y0 + 1);
  return a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy;
}

/** 哈希：sin 相位 → [0,1) 伪随机（确定性 · 无 Math.random 扰动） */
function _hash(x, y) {
  const s = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
  return s - Math.floor(s);
}

/** 斐波那契球：均匀球面点（无极点簇集） */
function _fibPoint(i, n) {
  const golden = Math.PI * (3 - Math.sqrt(5));
  const y = 1 - (2 * (i + 0.5)) / n;
  const r = Math.sqrt(1 - y * y);
  const th = i * golden;
  return [r * Math.cos(th), y, r * Math.sin(th)];
}

/** 两角差（-π..π） */
function _angDiff(a, b) {
  return Math.atan2(Math.sin(a - b), Math.cos(a - b));
}

/** 相机：绕 Y 轴 tiltY · 绕 X 轴 tiltX · 平移 (px,py) · 缩放 s → (x,y,z) 世界点 */
function _makeCamera(tiltY, tiltX, px, py, s) {
  const sy = Math.sin(tiltY);
  const cy = Math.cos(tiltY);
  const sx = Math.sin(tiltX);
  const cx = Math.cos(tiltX);
  return (vx, vy, vz) => {
    const g = vx * cy + vz * sy; // x/z 面绕 Y 旋转
    const u = -vx * sy + vz * cy;
    const h = vy * cx - u * sx; // y/u 面绕 X 旋转
    const R = vy * sx + u * cx;
    return [px + g * s, py - h * s, R];
  };
}

/** 画点：按 z 排序后绘制（画家算法），白色值 0..1 → 亮度（暗色取反） */
function _fillPoints(ctx, pts, isDark, minR = 0.3) {
  pts.sort((a, b) => a.z - b.z);
  for (const p of pts) {
    const a = p.a ?? 1;
    if (a < 0.02) continue;
    const w = Math.min(1, Math.max(0, p.white));
    const v = Math.round((isDark ? 1 - w : w) * 255);
    ctx.fillStyle = `rgba(${v},${v},${v},${a})`;
    ctx.beginPath();
    ctx.arc(p.x, p.y, Math.max(minR, p.r), 0, Math.PI * 2);
    ctx.fill();
  }
}

/** 画线段（web 状态：节点间的连线） */
function _strokeLines(ctx, lines, isDark) {
  for (const l of lines) {
    const a = l.a ?? 1;
    if (a < 0.02) continue;
    const w = Math.min(1, Math.max(0, l.white));
    const v = Math.round((isDark ? 1 - w : w) * 255);
    ctx.strokeStyle = `rgba(${v},${v},${v},${a})`;
    ctx.lineWidth = l.w;
    ctx.beginPath();
    ctx.moveTo(l.x1, l.y1);
    ctx.lineTo(l.x2, l.y2);
    ctx.stroke();
  }
}

/** 点径随尺寸缩放（320px 基准的幂律） */
function _rScale(size, pow = 0.6) {
  return (size / 300) ** pow;
}

/* ============================ 9 个状态引擎 ============================ */

/** weaving → braid：球面 ghost 底 + 3 股螺旋辫带（参考站招牌态） */
const _drawBraid = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.76;
  const cam = _makeCamera(t * 0.4, 0.3, c, c, 1);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const pts = [];
  const ghostN = o.ghostN ?? 150;
  for (let i = 0; i < ghostN; i++) {
    const f = _fibPoint(i, ghostN);
    const [x, y, z] = cam(f[0] * r, f[1] * r, f[2] * r);
    const d = (z / r + 1) / 2;
    pts.push({ x, y, z, r: 0.8 * rs, white: 0.78, a: 0.1 + 0.22 * d });
  }
  const strandN = o.strandN ?? 52;
  const turns = o.turns ?? 3;
  for (let s = 0; s < 3; s++) {
    const ph = (s / 3) * 2 * Math.PI;
    for (let i = 0; i < strandN; i++) {
      const D = (ct(i / strandN + t * 0.045) * 2 - 1) * 0.96;
      const rad = Math.sqrt(Math.max(0, 1 - D * D));
      const fade = Math.min(1, (1 - Math.abs(D)) / 0.1);
      const th = D * Math.PI * turns + ph;
      const swell = 1 + 0.075 * Math.sin(D * Math.PI * turns * 2 + ph * 2 + t * 0.8);
      const rr = rad * r * swell;
      const [x, y, z] = cam(Math.cos(th) * rr, D * r * swell, Math.sin(th) * rr);
      const d = (z / r + 1) / 2;
      pts.push({
        x,
        y,
        z,
        r: ((o.rBase ?? 1.2) + (o.rDepth ?? 1.8) * d) * rs,
        white: 0.55 - 0.45 * d,
        a: fade * (0.45 + 0.55 * d),
      });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** 一步一毫秒内做有意义的推进（避免整个函数糊在一行） */
const ct = (v) => v - Math.floor(v);

/** searching → globe：经纬球 + 高斯扫描亮带沿经线扫过 */
const _drawGlobe = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.82;
  const tilt = 0.4 + 0.06 * Math.sin(t * 0.35);
  const cam = _makeCamera(t * 0.5, tilt, c, c, r);
  const scan = t * (0.5 + (1.7 - 0.5) * (o.scanMul ?? 1));
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const dimBase = o.dimBase ?? 1;
  const pts = [];
  const latRings = o.latRings ?? 17;
  const lonDensity = o.lonDensity ?? 44;
  for (let lat = 0; lat <= latRings; lat++) {
    const phi = -Math.PI / 2 + (lat / latRings) * Math.PI;
    const cosPhi = Math.cos(phi);
    const sinPhi = Math.sin(phi);
    const lonN = Math.max(1, Math.round(Math.abs(cosPhi) * lonDensity));
    for (let lon = 0; lon < lonN; lon++) {
      const th = (lon / lonN) * 2 * Math.PI;
      const [x, y, z] = cam(cosPhi * Math.cos(th), sinPhi, cosPhi * Math.sin(th));
      const d = (z + 1) / 2;
      const ga = Math.exp(-(Math.pow(_angDiff(th + t * 0.5, scan), 2)) / 0.18) * Math.max(0, z);
      pts.push({
        x,
        y,
        z,
        r: ((o.rBase ?? 0.6) + (o.rDepth ?? 1.7) * d + (o.rBoost ?? 1) * ga) * rs,
        white: (o.inkFar ?? 0.62) - (o.inkSpan ?? 0.54) * d,
        a: dimBase + (1 - dimBase) * Math.min(1, ga),
      });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** 一步：旋转一圈的「子网格逐片点亮」状态 (rubik) 的辅助——一次旋转某一层的量 */
function _tweenAmount(p, n, s, a) {
  const total = 2 * n * s + a;
  const e = p % total;
  const amt = new Array(n).fill(0);
  let active = -1;
  if (e < 2 * n * s) {
    const face = Math.floor(e / s);
    const f = (e - face * s) / s;
    const h = 1 - Math.pow(1 - Math.min(1, f / 0.7), 3);
    if (face < n) {
      for (let i = 0; i < face; i++) amt[i] = 1;
      amt[face] = h;
      active = face;
    } else {
      const j = 2 * n - 1 - face;
      for (let i = 0; i < j; i++) amt[i] = 1;
      amt[j] = 1 - h;
      active = j;
    }
  }
  return { amount: amt, active };
}

/** 对点按某次旋转应用角度 */
function _applyRot(p, moves, amt) {
  let [x, y, z] = p;
  let active = false;
  for (let i = 0; i < moves.length; i++) {
    if (amt.amount[i] <= 0) continue;
    const m = moves[i];
    const v = m.axis === 0 ? x : m.axis === 1 ? y : z;
    if (v < m.lo || v >= m.hi) continue;
    if (i === amt.active) active = true;
    const ang = m.ang * amt.amount[i];
    const cos = Math.cos(ang);
    const sin = Math.sin(ang);
    if (m.axis === 0) {
      const ny = y * cos - z * sin;
      z = y * sin + z * cos;
      y = ny;
    } else if (m.axis === 1) {
      const nx = x * cos + z * sin;
      z = -x * sin + z * cos;
      x = nx;
    } else {
      const nx = x * cos - y * sin;
      y = x * sin + y * cos;
      x = nx;
    }
  }
  return [x, y, z, active];
}

/** 生成本轮的转动序列（确定性哈希） */
function _makeMoves(n) {
  const moves = [];
  for (let i = 0; i < n; i++) {
    const axis = Math.min(2, Math.floor(_hash(i, 2.3) * 3));
    const lo = -1 + 0.5 * Math.min(3, Math.floor(_hash(i, 5.9) * 4));
    const dir = _hash(i, 7.7) < 0.5 ? 1 : -1;
    moves.push({ axis, lo, hi: lo + 0.5, ang: dir * Math.PI / 2 });
  }
  return moves;
}

/** solving → rubik：球面子网格逐片点亮转动 */
const _drawRubik = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.82;
  const cam = _makeCamera(t * 0.55, 0.35 + 0.1 * Math.sin(t * 0.9), c, c, r);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const moveCount = o.moveCount ?? 14;
  const moves = _makeMoves(moveCount);
  const sweep = _tweenAmount(t, moveCount, 0.42, 1.2);
  const pts = [];
  const latRings = o.latRings ?? 15;
  const lonDensity = o.lonDensity ?? 40;
  for (let lat = 0; lat <= latRings; lat++) {
    const phi = -Math.PI / 2 + (lat / latRings) * Math.PI;
    const cosPhi = Math.cos(phi);
    const sinPhi = Math.sin(phi);
    const lonN = Math.max(1, Math.round(Math.abs(cosPhi) * lonDensity));
    for (let lon = 0; lon < lonN; lon++) {
      const th = (lon / lonN) * 2 * Math.PI;
      const [px, py, pz, act] = _applyRot([cosPhi * Math.cos(th), sinPhi, cosPhi * Math.sin(th)], moves, sweep);
      const [x, y, z] = cam(px, py, pz);
      const d = (z + 1) / 2;
      pts.push({
        x,
        y,
        z,
        r: ((o.rBase ?? 0.6) + (o.rDepth ?? 1.7) * d + (act ? o.rActive ?? 0.3 : 0)) * rs,
        white: (o.inkFar ?? 0.62) - (o.inkSpan ?? 0.54) * d - (act ? 0.14 : 0),
      });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** listening → wave：纬环随正弦波呼吸起伏 */
const _drawWave = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.874;
  const cam = _makeCamera(t * 0.18, 0.38, c, c, 1);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const pts = [];
  const rings = o.rings ?? 15;
  const lonDensity = o.lonDensity ?? 40;
  for (let ring = 0; ring <= rings; ring++) {
    const phi = -Math.PI / 2 + (ring / rings) * Math.PI;
    const cosPhi = Math.cos(phi);
    const sinPhi = Math.sin(phi);
    const und = 0.62 * Math.sin(t * 2.1 - ring * 0.52) + 0.38 * Math.sin(t * 1.27 + ring * 0.83);
    const rr = r * (0.88 + 0.105 * und);
    const lonN = Math.max(1, Math.round(Math.abs(cosPhi) * lonDensity));
    for (let lon = 0; lon < lonN; lon++) {
      const th = (lon / lonN) * 2 * Math.PI;
      const [x, y, z] = cam(cosPhi * Math.cos(th) * rr, sinPhi * rr, cosPhi * Math.sin(th) * rr);
      const d = (z / r + 1) / 2;
      const boost = Math.max(0, und);
      pts.push({
        x,
        y,
        z,
        r: ((o.rBase ?? 0.6) + (o.rDepth ?? 1.7) * d) * (1 + 0.4 * boost) * rs,
        white: 0.66 - 0.56 * d - 0.1 * boost,
      });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** 平滑插值（morph 形变用） */
function _smoothstep(v) {
  return v * v * (3 - 2 * v);
}

/** 弧长参数化：沿多边形轮廓均匀取点（morph 沿形状轮廓排点） */
function _arcPath(poly) {
  const n = poly.length;
  const lens = [];
  let total = 0;
  for (let i = 0; i < n; i++) {
    const a = poly[i];
    const b = poly[(i + 1) % n];
    const d = Math.hypot(b[0] - a[0], b[1] - a[1]);
    lens.push(d);
    total += d;
  }
  return (t) => {
    let d = t * total;
    let i = 0;
    while (d > lens[i] && i < n - 1) d -= lens[i++];
    const a = poly[i];
    const b = poly[(i + 1) % n];
    const k = lens[i] ? Math.min(1, d / lens[i]) : 0;
    return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
  };
}

/** 三种轮廓：圆弧 / 三角 / 方形（morph 在这三者间形变） */
const _circlePath = (t) => {
  const a = -Math.PI / 2 + t * 2 * Math.PI;
  return [Math.cos(a) * 0.24, Math.sin(a) * 0.24];
};
const _triPath = _arcPath([
  [0, -0.26],
  [0.24, 0.16],
  [-0.24, 0.16],
]);
const _sqPath = _arcPath([
  [0, -0.2],
  [0.2, -0.2],
  [0.2, 0.2],
  [-0.2, 0.2],
  [-0.2, -0.2],
]);
const _MORPH_PATHS = [_circlePath, _triPath, _sqPath];

/** 点数随图标密度 */
function _iconDots(d) {
  return Math.max(6, Math.round(34 * d));
}

/** shaping → morph：点阵在 圆→三角→方形 间形变 */
const _drawMorph = (ctx, size, t, isDark, o) => {
  const HOLD = 1.4; // 每个形状保持时长
  const SW = 0.9; // 形变时长
  const PER = HOLD + SW;
  const nPaths = _MORPH_PATHS.length;
  const phase = t % (PER * nPaths);
  const fromIdx = Math.floor(phase / PER);
  const inner = phase - fromIdx * PER;
  const mix = inner > HOLD ? _smoothstep((inner - HOLD) / SW) : 0;
  const spread = o.spread ?? 1;
  const a = _MORPH_PATHS[fromIdx];
  const b = _MORPH_PATHS[(fromIdx + 1) % nPaths];
  const SAMPLE = 160;
  const ring = [];
  for (let i = 0; i < SAMPLE; i++) {
    const k = i / SAMPLE;
    const pa = a(k);
    const pb = b(k);
    ring.push([(pa[0] + (pb[0] - pa[0]) * mix) * spread, (pa[1] + (pb[1] - pa[1]) * mix) * spread]);
  }
  const lens = [];
  let total = 0;
  for (let i = 0; i < SAMPLE; i++) {
    const p = ring[i];
    const q = ring[(i + 1) % SAMPLE];
    const d = Math.hypot(q[0] - p[0], q[1] - p[1]);
    lens.push(d);
    total += d;
  }
  const dots = _iconDots(o.iconD ?? 1);
  const dotR = (o.rDot ?? 0.021) * 1.35 * spread;
  const bob = 1 + 0.02 * Math.sin(inner * 3.1);
  const pts = [];
  const c = size / 2;
  let acc = 0;
  let seg = 0;
  for (let i = 0; i < dots; i++) {
    const target = (i / dots) * total;
    while (acc + lens[seg] < target && seg < SAMPLE - 1) acc += lens[seg++];
    const p = ring[seg];
    const q = ring[(seg + 1) % SAMPLE];
    const k = lens[seg] ? Math.min(1, (target - acc) / lens[seg]) : 0;
    const px = (p[0] + (q[0] - p[0]) * k) * bob;
    const py = (p[1] + (q[1] - p[1]) * k) * bob;
    pts.push({ x: c + px * size, y: c + py * size, z: 0, r: Math.max(0.35, dotR * size), white: 0.1 });
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** connecting → web：节点球网 + 信号沿边脉冲 */
const _drawWeb = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.8 * (o.spread ?? 1);
  const cam = _makeCamera(t * 0.12, 0.32, c, c, r);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const nodeN = o.nodeN ?? 30;
  const thr = o.thr ?? 0.72;
  const nodeR = o.nodeR ?? 1.4;
  const nodeRDepth = o.nodeRDepth ?? 1.8;
  const nodes = [];
  for (let i = 0; i < nodeN; i++) {
    const f = _fibPoint(i, nodeN);
    const jx = f[0] + 0.3 * (_smoothNoise(i * 0.31 + 9, t * 0.24) - 0.5) * 2;
    const jy = f[1] + 0.3 * (_smoothNoise(i * 0.53 + 27, t * 0.21) - 0.5) * 2;
    const jz = f[2] + 0.3 * (_smoothNoise(i * 0.77 + 55, t * 0.27) - 0.5) * 2;
    const m = Math.sqrt(jx * jx + jy * jy + jz * jz);
    nodes.push([jx / m, jy / m, jz / m]);
  }
  const lines = [];
  const dots = [];
  for (let i = 0; i < nodeN; i++) {
    for (let j = i + 1; j < nodeN; j++) {
      const dx = nodes[i][0] - nodes[j][0];
      const dy = nodes[i][1] - nodes[j][1];
      const dz = nodes[i][2] - nodes[j][2];
      const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
      if (dist >= thr) continue;
      const [x1, y1, z1] = cam(...nodes[i]);
      const [x2, y2, z2] = cam(...nodes[j]);
      const midD = ((z1 + z2) / 2 + 1) / 2;
      lines.push({
        x1,
        y1,
        x2,
        y2,
        white: 0.42,
        a: (1 - dist / thr) * (0.3 + 0.55 * midD),
        w: Math.max(0.6, (o.lineW ?? 0.8) * rs),
      });
    }
  }
  for (let i = 0; i < nodeN; i++) {
    const [x, y, z] = cam(...nodes[i]);
    const d = (z + 1) / 2;
    const pulse = 1 + 0.25 * Math.sin(t * 1.4 + i * 2.7);
    dots.push({ x, y, z, r: (nodeR + nodeRDepth * d) * pulse * rs, white: 0.55 - 0.45 * d });
  }
  const signals = o.signals ?? 5;
  for (let i = 0; i < signals; i++) {
    const tick = Math.floor(t * 0.55 + i * 7.31);
    const a = Math.floor(_hash(tick, i * 3.1 + 1.7) * nodeN);
    const b = Math.floor(_hash(tick, i * 5.7 + 4.2) * nodeN);
    if (a === b) continue;
    const f = ct(t * 0.55 + i * 7.31);
    const sx = nodes[a][0] + (nodes[b][0] - nodes[a][0]) * f;
    const sy = nodes[a][1] + (nodes[b][1] - nodes[a][1]) * f;
    const sz = nodes[a][2] + (nodes[b][2] - nodes[a][2]) * f;
    const m = Math.max(1e-6, Math.sqrt(sx * sx + sy * sy + sz * sz));
    const [x, y, z] = cam(sx / m, sy / m, sz / m);
    const d = (z + 1) / 2;
    dots.push({ x, y, z, r: (nodeR * 1.5 + nodeRDepth * d) * rs, white: 0.05, a: 0.5 + 0.5 * d });
  }
  _strokeLines(ctx, lines, isDark);
  _fillPoints(ctx, dots, isDark, o.rMin);
};

/** breathing → ring / composing → ribbon：绕轴转动的带点（faceOn 时转成环） */
const _drawRibbon = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.78;
  const spin = o.spin ?? 1;
  const cam = _makeCamera(t * 0.1 * spin, 0.3, c, c, 1);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const pts = [];
  const ghostN = o.ghostN ?? 150;
  for (let i = 0; i < ghostN; i++) {
    const f = _fibPoint(i, ghostN);
    const [x, y, z] = cam(f[0] * r, f[1] * r, f[2] * r);
    const d = (z / r + 1) / 2;
    pts.push({ x, y, z, r: 0.8 * rs, white: 0.78, a: 0.1 + 0.22 * d });
  }
  const yaw = t * 0.24 * spin;
  const pitch = o.faceOn ? -0.3 : 0.55 + 0.3 * Math.sin(t * 0.18) * spin;
  const k = Math.cos(yaw);
  const D = 0;
  const i = Math.sin(yaw);
  const d = -i * Math.sin(pitch);
  const y = Math.cos(pitch);
  const p = k * Math.sin(pitch);
  const f = D * p - i * y;
  const P = i * d - k * p;
  const m = k * y - D * d;
  const wob = 0.23 * (o.wobMul ?? 1);
  const bandR = o.faceOn ? r / (1 + 0.85 * wob) : r;
  const lanes = o.lanes ?? 5;
  const segs = o.segs ?? 88;
  const bands = Math.max(1, Math.round(lanes * (o.bandMul ?? 1)));
  for (let b = 0; b < bands; b++) {
    const off = (b - (bands - 1) / 2) * 0.075;
    const edge = Math.abs(b - (bands - 1) / 2) / Math.max(1, (bands - 1) / 2);
    for (let s = 0; s < segs; s++) {
      const th = (s / segs) * 2 * Math.PI;
      const u = (0.16 * Math.sin(th * 3 - t * 1.7 + b * 0.22) + 0.07 * Math.sin(th * 5 + t * 1.1)) * (o.wobMul ?? 1);
      const F = o.faceOn ? 1 + u : 1;
      const T = o.faceOn ? off : off + u;
      const X = k * Math.cos(th) + d * Math.sin(th) + f * T;
      const Y = D * Math.cos(th) + y * Math.sin(th) + P * T;
      const Z = i * Math.cos(th) + p * Math.sin(th) + m * T;
      const len = Math.sqrt(X * X + Y * Y + Z * Z);
      const rad = bandR * F;
      const [x, y2, z] = cam((X / len) * rad, (Y / len) * rad, (Z / len) * rad);
      const dep = (z / r + 1) / 2;
      pts.push({
        x,
        y: y2,
        z,
        r: ((o.rBase ?? 1.1) + (o.rDepth ?? 1.7) * dep) * (1 - 0.25 * edge) * rs,
        white: 0.52 - 0.44 * dep + 0.18 * edge,
        a: 0.4 + 0.6 * dep,
      });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/** working → orbits：轨道环 + 环绕粒子 */
const _drawOrbits = (ctx, size, t, isDark, o) => {
  const c = size / 2;
  const r = size / 2 * 0.82;
  const cam = _makeCamera(t * 0.12, 0.3, c, c, 1);
  const rs = _rScale(size, o.rsPow ?? 0.6);
  const pts = [];
  const orbitN = o.orbitN ?? 12;
  const ghostN = o.ghostN ?? 40;
  const particles = o.particles ?? 3;
  for (let i = 0; i < orbitN; i++) {
    const h1 = _hash(i, 1.7);
    const h2 = _hash(i, 5.2);
    const h3 = _hash(i, 8.9);
    const rr = r * (0.45 + 0.52 * h1);
    const th0 = h1 * 2 * Math.PI;
    const acosY = Math.acos(2 * h2 - 1);
    const p0 = Math.sin(acosY) * Math.cos(th0);
    const py = Math.cos(acosY);
    const p1 = Math.sin(acosY) * Math.sin(th0);
    let ax = -py;
    let ay = p0;
    const az = 0;
    const axLen = Math.max(1e-6, Math.sqrt(ax * ax + ay * ay));
    ax /= axLen;
    ay /= axLen;
    const bx = py * az - p1 * ay;
    const by = p1 * ax - p0 * az;
    const bz = p0 * ay - py * ax;
    const dir = (0.25 + 0.55 * h3) * (h3 > 0.5 ? 1 : -1);
    for (let g = 0; g < ghostN; g++) {
      const a = (g / ghostN) * 2 * Math.PI;
      const [x, y, z] = cam((ax * Math.cos(a) + bx * Math.sin(a)) * rr, (ay * Math.cos(a) + by * Math.sin(a)) * rr, (az * Math.cos(a) + bz * Math.sin(a)) * rr);
      const d = (z / rr + 1) / 2; // 深度按轨道半径归一
      pts.push({ x, y, z, r: (o.ghostR ?? 0.9) * rs, white: 0.72, a: (o.ghostA ?? 0.5) * (0.4 + 0.6 * d) });
    }
    for (let p = 0; p < particles; p++) {
      const a = t * dir + (p / particles) * 2 * Math.PI + h2 * 6;
      const [x, y, z] = cam((ax * Math.cos(a) + bx * Math.sin(a)) * rr, (ay * Math.cos(a) + by * Math.sin(a)) * rr, (az * Math.cos(a) + bz * Math.sin(a)) * rr);
      const d = (z / rr + 1) / 2; // 深度按轨道半径归一
      pts.push({ x, y, z, r: ((o.partR ?? 1.2) + (o.partRDepth ?? 1.6) * d) * rs, white: 0.3 - 0.22 * d });
    }
  }
  _fillPoints(ctx, pts, isDark, o.rMin);
};

/* ============================ 分发表 + 预设 ============================ */

const MODE_DRAWS = {
  orbits: _drawOrbits,
  globe: _drawGlobe,
  rubik: _drawRubik,
  wave: _drawWave,
  web: _drawWeb,
  braid: _drawBraid,
  ribbon: _drawRibbon,
  ring: _drawRibbon, // ring 复用 ribbon 绘制器，faceOn 配置把它转成正面圆环
  morph: _drawMorph,
};

/** 状态 → 渲染模式 */
const STATE_TO_MODE = {
  working: "orbits",
  searching: "globe",
  solving: "rubik",
  listening: "wave",
  connecting: "web",
  weaving: "braid",
  composing: "ribbon",
  breathing: "ring",
  shaping: "morph",
};

/** 各模式默认配置（手调参数） */
const PRESETS = {
  globe: { latRings: 17, lonDensity: 44, rBase: 0.6, rDepth: 1.7, rBoost: 1, inkFar: 0.62, inkSpan: 0.54, rsPow: 0.6, rMin: 0.3 },
  orbits: { orbitN: 12, ghostN: 40, ghostR: 0.9, ghostA: 0.5, particles: 3, partR: 1.2, partRDepth: 1.6, rsPow: 0.6, rMin: 0.3 },
  rubik: { latRings: 15, lonDensity: 40, moveCount: 14, rBase: 0.6, rDepth: 1.7, rActive: 0.3, inkFar: 0.62, inkSpan: 0.54, rsPow: 0.6, rMin: 0.3 },
  wave: { rings: 15, lonDensity: 40, rBase: 0.6, rDepth: 1.7, rsPow: 0.6, rMin: 0.3 },
  web: { nodeN: 30, thr: 0.72, signals: 5, nodeR: 1.4, nodeRDepth: 1.8, lineW: 0.8, rsPow: 0.6, rMin: 0.3 },
  braid: { strandN: 52, turns: 3, ghostN: 150, rBase: 1.2, rDepth: 1.8, rsPow: 0.6, rMin: 0.3 },
  ribbon: { lanes: 5, segs: 88, ghostN: 150, rBase: 1.1, rDepth: 1.7, rsPow: 0.6, rMin: 0.3 },
  ring: { lanes: 5, segs: 88, ghostN: 0, faceOn: 1, rBase: 1.1, rDepth: 1.7, rsPow: 0.6, rMin: 0.3 },
  morph: { rDot: 0.021, iconD: 1, rMin: 0.25 },
};

/** 小尺寸预设：参考站 20px 档（每状态速度/密度/尺寸独立调参） */
const SMALL_PRESET = {
  orbits: { speed: 3.9, count: 0.238, size: 2.4 },
  globe: { speed: 2.665, count: 0.105, size: 1.75, extra: { scanMul: 4.335, dimBase: 0.45 } },
  rubik: { speed: 1.95, count: 0.088, size: 1.9 },
  wave: { speed: 3.998, count: 0.105, size: 1.6 },
  web: { speed: 6.63, count: 0.25, size: 1.52 },
  braid: { speed: 2.75, count: 0.1125, size: 1.36 },
  ribbon: { speed: 3.12, count: 0.051, size: 1.073, extra: { spin: 0, bandMul: 4.94, wobMul: 1 } },
  ring: { speed: 3.78, count: 0.028, size: 1.622, extra: { spin: 0, bandMul: 3.968, wobMul: 0.565 } },
  morph: { speed: 2.08, count: 0.53, size: 1.011, extra: { spread: 1.45 } },
};

/** 按密度/尺寸缩放配置的计数与尺寸类参数 */
function _scaleCounts(opts, density) {
  const out = { ...opts };
  const seen = new Set();
  const s = Math.sqrt(density);
  const pairs = [
    ["latRings", "lonDensity"],
    ["rings", "lonDensity"],
    ["lanes", "segs"],
  ];
  for (const [k1, k2] of pairs) {
    const v1 = out[k1];
    const v2 = out[k2];
    if (v1 != null && v2 != null && !seen.has(k1) && !seen.has(k2)) {
      out[k1] = Math.max(2, Math.round(v1 * s));
      out[k2] = Math.max(2, Math.round(v2 * s));
      seen.add(k1);
      seen.add(k2);
    }
  }
  for (const k of ["orbitN", "ghostN", "nodeN", "strandN", "signals"]) {
    const v = out[k];
    if (v != null && v !== 0 && !seen.has(k)) out[k] = Math.max(1, Math.round(v * density));
  }
  for (const k of ["iconD"]) {
    const v = out[k];
    if (v != null) out[k] = Math.max(0.02, v * density);
  }
  return out;
}

/** 按尺寸缩放半径类参数 */
function _scaleRadius(opts, factor) {
  const out = { ...opts };
  for (const k of ["rBase", "rDepth", "rActive", "rDot", "ghostR", "partR", "partRDepth", "nodeR", "nodeRDepth"]) {
    const v = out[k];
    if (v != null) out[k] = v * factor;
  }
  out.rSizeMul = (out.rSizeMul ?? 1) * factor;
  return out;
}

const _resolveCache = new Map();

/** 解析状态+尺寸 → { mode, speed, opts }（带缓存） */
export function resolvePreset(state, size) {  const key = state + "-" + size;
  const hit = _resolveCache.get(key);
  if (hit) return hit;
  const mode = STATE_TO_MODE[state] || "orbits";
  const preset = SMALL_PRESET[mode];
  let opts = { ...PRESETS[mode] };
  if (preset) {
    if (preset.count !== 1) opts = _scaleCounts(opts, preset.count);
    if (preset.size !== 1) opts = _scaleRadius(opts, preset.size);
    if (preset.extra) opts = { ...opts, ...preset.extra };
  }
  const resolved = { mode, speed: preset ? preset.speed : 1.885, opts };
  _resolveCache.set(key, resolved);
  return resolved;
}

/** 自 DOM 向上探测 dark 主题（未来 app 加暗色时用） */
function _detectDark(el) {
  let n = el;
  while (n) {
    const t = n.getAttribute("data-theme");
    if (t === "dark") return true;
    if (t === "light") return false;
    if (n.classList.contains("dark")) return true;
    if (n.classList.contains("light")) return false;
    n = n.parentElement;
  }
  return null;
}

/**
 * 启动思考 orb 动画。返回 stop 函数（幂等）。
 * opts:
 *   state  — 九个状态之一（默认 weaving = 参考站 braid 招牌态）
 *   size   — 画布 px 边长（默认 32；参考站小尺寸档）
 *   speed  — 速度倍率（默认 1）
 *   dark   — 显式浅色/深色（缺省：祖先 data-theme/class → 默认浅色；不回退 OS 偏好）
 * 同一 canvas 重复调用返回已存在的 stop；DOM 移除时自动停止（rAF/IO 防泄漏）。
 */
export function startThinkingOrb(canvas, opts = {}) {
  if (!canvas) return null;
  if (canvas.dataset.ccoOrb === "1") return canvas._ccoOrbStop || null;

  const state = opts.state || "weaving";
  const size = opts.size || ORB_SIZE;
  const speed = opts.speed ?? 1;
  const dpr = Math.min(window.devicePixelRatio || 1, ORB_DPR);
  canvas.width = Math.round(size * dpr);
  canvas.height = Math.round(size * dpr);
  canvas.style.width = size + "px";
  canvas.style.height = size + "px";
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const { mode, speed: baseSpeed, opts: preset } = resolvePreset(state, size);
  const draw = MODE_DRAWS[mode];
  const rate = baseSpeed * speed;
  // 暗色判定：显式 dark 优先 → 祖先 data-theme/class → 默认浅色（不跟 OS prefers-color-scheme）
  const dark =
    opts.dark != null ? !!opts.dark : _detectDark(canvas) ?? false;

  const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  let reducedMotion = motionQuery.matches;
  let raf = 0;
  let io = null;

  function drawFrame(ts) {
    draw(ctx, size, (ts / 1000) * rate, dark, preset);
  }

  function frame(ts) {
    drawFrame(ts);
    if (!reducedMotion) raf = requestAnimationFrame(frame);
  }

  function onMotionChange(event) {
    reducedMotion = event.matches;
    cancelAnimationFrame(raf);
    if (reducedMotion) drawFrame(0);
    else raf = requestAnimationFrame(frame);
  }

  const stop = () => {
    cancelAnimationFrame(raf);
    motionQuery.removeEventListener("change", onMotionChange);
    if (io) io.disconnect();
    ctx.clearRect(0, 0, size, size);
    delete canvas.dataset.ccoOrb;
    delete canvas._ccoOrbStop;
  };

  if (reducedMotion) drawFrame(0);
  else raf = requestAnimationFrame(frame);
  motionQuery.addEventListener("change", onMotionChange);

  canvas.dataset.ccoOrb = "1";
  canvas._ccoOrbStop = stop;

  // 从 DOM 移除 → rootBounds 变 null → 自动 stop，避免 rAF 泄漏
  io = new IntersectionObserver((entries) => {
    for (const en of entries) {
      if (en.rootBounds == null) stop();
    }
  });
  io.observe(canvas);

  return stop;
}

/** 停止 orb（幂等；canvas 可为 null/未启动）。 */
export function stopThinkingOrb(canvas) {
  if (canvas && typeof canvas._ccoOrbStop === "function") {
    canvas._ccoOrbStop();
  }
}

/* 导出渲染表与状态映射（与参考站导出对齐；供对比测试/未来多状态预览） */
export { MODE_DRAWS, STATE_TO_MODE };
