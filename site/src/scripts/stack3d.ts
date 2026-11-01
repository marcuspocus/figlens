// The first-success frame as figlens reads it: one textured plane per stored layer, which tilts and
// separates into an exploded stack. Tags are HTML, pinned to each layer's corner by projection.
import {
  CanvasTexture,
  Color,
  EdgesGeometry,
  Float32BufferAttribute,
  BufferGeometry,
  Group,
  LineBasicMaterial,
  LineDashedMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SRGBColorSpace,
  Vector3,
  WebGLRenderer,
} from 'three';
import type { Layer } from '../data';
import { prepare, paintLayer } from './screen';

const INK = new Color('#16151a');
const ACCENT = new Color('#5b47b8');
const GAP = 26; // design px between exploded layers
const FLAT = { x: -0.04, y: 0, z: 0 };
const TILT = { x: -0.62, y: 0, z: -0.42 };

type Part = {
  layer: Layer;
  mesh: Mesh<PlaneGeometry, MeshBasicMaterial>;
  edge: LineSegments<EdgesGeometry, LineBasicMaterial>;
  drop?: LineSegments<BufferGeometry, LineDashedMaterial>;
  tag?: HTMLElement;
  leader?: SVGLineElement;
  lift: number;
  dim: number;
};

const ease = (t: number) => 1 - Math.pow(1 - t, 3);

function shadowTexture() {
  const c = document.createElement('canvas');
  c.width = 256;
  c.height = 160;
  const ctx = c.getContext('2d')!;
  ctx.filter = 'blur(18px)';
  ctx.fillStyle = 'rgba(22, 21, 26, 0.32)';
  ctx.fillRect(40, 34, 176, 92);
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  return t;
}

// Dashed verticals from a raised layer's corners down to the card plane, like an exploded drawing.
function dropLines(w: number, h: number) {
  const g = new BufferGeometry();
  const pts: number[] = [];
  for (const [x, y] of [[-w / 2, -h / 2], [w / 2, -h / 2], [w / 2, h / 2], [-w / 2, h / 2]]) pts.push(x, y, 0, x, y, -1);
  for (const [a, b] of [[[-w / 2, -h / 2], [w / 2, -h / 2]], [[w / 2, -h / 2], [w / 2, h / 2]], [[w / 2, h / 2], [-w / 2, h / 2]], [[-w / 2, h / 2], [-w / 2, -h / 2]]]) pts.push(a[0], a[1], -1, b[0], b[1], -1);
  g.setAttribute('position', new Float32BufferAttribute(pts, 3));
  return g;
}

export function mount(stage: HTMLElement, layers: Layer[], opts: { tags: 'all' | 'active' }) {
  const renderer = new WebGLRenderer({ alpha: true, antialias: true, powerPreference: 'low-power' });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  const canvas = renderer.domElement;
  canvas.setAttribute('aria-hidden', 'true');
  canvas.className = 'stage-canvas';
  stage.prepend(canvas);

  const scene = new Scene();
  const camera = new PerspectiveCamera(26, 1, 10, 5000);
  const stack = new Group();
  scene.add(stack);

  const shadow = new Mesh(new PlaneGeometry(420, 260), new MeshBasicMaterial({ map: shadowTexture(), transparent: true, depthWrite: false }));
  shadow.renderOrder = -1;
  stack.add(shadow);

  const svg = stage.querySelector<SVGSVGElement>('[data-leaders]');
  const maxAniso = renderer.capabilities.getMaxAnisotropy();

  const parts: Part[] = layers.map((layer) => {
    const [x, y, w, h] = layer.box;
    const mat = new MeshBasicMaterial({ transparent: true, depthWrite: false, opacity: 0 });
    const mesh = new Mesh(new PlaneGeometry(w, h), mat);
    mesh.position.set(x + w / 2 - 160, -(y + h / 2 - 100), 0);
    mesh.renderOrder = layer.depth;
    const edge = new LineSegments(new EdgesGeometry(mesh.geometry), new LineBasicMaterial({ color: INK, transparent: true, opacity: 0 }));
    mesh.add(edge);
    let drop: Part['drop'];
    if (layer.depth >= 2) {
      drop = new LineSegments(dropLines(w, h), new LineDashedMaterial({ color: INK, dashSize: 3, gapSize: 3, transparent: true, opacity: 0 }));
      mesh.add(drop);
    }
    stack.add(mesh);
    const tag = stage.querySelector<HTMLElement>(`[data-tag="${layer.id}"]`) ?? undefined;
    let leader: SVGLineElement | undefined;
    if (tag && svg) {
      leader = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      svg.append(leader);
    }
    return { layer, mesh, edge, drop, tag, leader, lift: 0, dim: 1 };
  });

  let t = 0;
  let tTarget = 0;
  let active: string | null = null;
  const tilt = { x: 0, y: 0 };
  const tiltTarget = { x: 0, y: 0 };
  let size = { w: 1, h: 1 };
  let raf = 0;
  let last = 0;

  const corner = new Vector3();
  // Tags stand in one column on the right, in the order of their anchors, pushed apart so none overlap.
  const placeTags = () => {
    const rows = parts.filter((p) => p.tag).map((p) => {
      const [, , w] = p.layer.box;
      corner.set(w / 2, 0, 0);
      p.mesh.localToWorld(corner).project(camera);
      return { p, ax: (corner.x * 0.5 + 0.5) * size.w, ay: (-corner.y * 0.5 + 0.5) * size.h, h: p.tag!.offsetHeight };
    }).sort((a, b) => a.ay - b.ay);
    let floor = 0;
    for (const r of rows) {
      const ty = Math.max(r.ay - r.h / 2, floor);
      floor = ty + r.h + 8;
      const tag = r.p.tag!;
      const tx = size.w - tag.offsetWidth;
      const show = opts.tags === 'all' ? ease(Math.max(0, (t - 0.7) / 0.3)) : active === r.p.layer.id ? 1 : 0;
      tag.style.transform = `translate(${tx}px, ${ty}px)`;
      tag.style.opacity = String(show);
      if (r.p.leader) {
        r.p.leader.setAttribute('x1', String(r.ax));
        r.p.leader.setAttribute('y1', String(r.ay));
        r.p.leader.setAttribute('x2', String(tx));
        r.p.leader.setAttribute('y2', String(ty + r.h / 2));
        r.p.leader.style.opacity = String(show);
      }
    }
  };

  const render = () => {
    const e = ease(t);
    stack.rotation.set(FLAT.x + (TILT.x - FLAT.x) * e + tilt.y, FLAT.y + (TILT.y - FLAT.y) * e + tilt.x, FLAT.z + (TILT.z - FLAT.z) * e);
    shadow.position.set(10 * e, -12 - 6 * e, -8 - 30 * e);
    for (const p of parts) {
      const z = p.layer.depth * (0.4 + GAP * e) + p.lift;
      p.mesh.position.z = z;
      p.mesh.material.opacity = p.mesh.material.map ? p.dim : 0;
      p.edge.material.color.copy(active === p.layer.id ? ACCENT : INK);
      p.edge.material.opacity = (active === p.layer.id ? 0.9 : 0.28) * e * p.dim;
      if (p.drop) {
        // Reach down to the card plane (depth 1), whatever this layer's height.
        const len = z - (0.4 + GAP * e) - parts[1].lift;
        const pos = p.drop.geometry.attributes.position;
        for (let i = 0; i < pos.count; i++) if (pos.getZ(i) !== 0) pos.setZ(i, -Math.max(len, 0.001));
        pos.needsUpdate = true;
        p.drop.computeLineDistances();
        p.drop.material.opacity = 0.35 * e * p.dim;
      }
    }
    renderer.render(scene, camera);
    placeTags();
  };

  const step = (now: number) => {
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    const k = 1 - Math.exp(-dt * 3.2);
    t += (tTarget - t) * k;
    tilt.x += (tiltTarget.x - tilt.x) * (1 - Math.exp(-dt * 6));
    tilt.y += (tiltTarget.y - tilt.y) * (1 - Math.exp(-dt * 6));
    let moving = Math.abs(tTarget - t) > 0.001 || Math.abs(tiltTarget.x - tilt.x) > 0.0005 || Math.abs(tiltTarget.y - tilt.y) > 0.0005;
    for (const p of parts) {
      const lift = active === p.layer.id ? 22 : 0;
      const dim = active && active !== p.layer.id ? 0.32 : 1;
      p.lift += (lift - p.lift) * (1 - Math.exp(-dt * 8));
      p.dim += (dim - p.dim) * (1 - Math.exp(-dt * 8));
      if (Math.abs(lift - p.lift) > 0.05 || Math.abs(dim - p.dim) > 0.005) moving = true;
    }
    render();
    raf = moving ? requestAnimationFrame(step) : 0;
  };
  const kick = () => {
    if (raf) return;
    last = performance.now();
    raf = requestAnimationFrame(step);
  };

  const resize = () => {
    const r = stage.getBoundingClientRect();
    size = { w: Math.max(1, r.width), h: Math.max(1, r.height) };
    renderer.setSize(size.w, size.h, false);
    camera.aspect = size.w / size.h;
    // Fit the exploded stack plus the tag column beside it.
    const v = Math.tan((camera.fov * Math.PI) / 360);
    const d = Math.max(280 / (v * camera.aspect), 150 / v);
    camera.position.set(0, 0, d);
    stack.position.x = opts.tags === 'all' ? -d * v * camera.aspect * 0.3 : 0;
    camera.updateProjectionMatrix();
    render();
  };
  new ResizeObserver(resize).observe(stage);

  const fine = matchMedia('(pointer: fine)').matches;
  if (fine) {
    stage.addEventListener('pointermove', (e) => {
      const r = stage.getBoundingClientRect();
      tiltTarget.x = ((e.clientX - r.left) / r.width - 0.5) * 0.22;
      tiltTarget.y = ((e.clientY - r.top) / r.height - 0.5) * 0.14;
      kick();
    });
    stage.addEventListener('pointerleave', () => { tiltTarget.x = tiltTarget.y = 0; kick(); });
  }

  const ready = prepare().then(() => Promise.all(
    parts.map(async (p) => {
      const tex = new CanvasTexture(await paintLayer(p.layer, 6));
      tex.colorSpace = SRGBColorSpace;
      tex.anisotropy = maxAniso;
      p.mesh.material.map = tex;
      p.mesh.material.needsUpdate = true;
    }),
  )).then(() => {
    resize();
    stage.classList.add('is-3d');
  });

  return {
    ready,
    explode(on: boolean, instant = false) {
      tTarget = on ? 1 : 0;
      if (instant) { t = tTarget; render(); } else kick();
    },
    highlight(id: string | null) {
      active = id;
      kick();
    },
  };
}
