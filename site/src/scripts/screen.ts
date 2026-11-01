// Draws each layer of the illustrative frame as its own crisp canvas, at `scale` canvas px per design px.
import type { Layer } from '../data';

const FONT = '"Schibsted Grotesk Variable", "Schibsted Grotesk", system-ui, sans-serif';

let logo: Promise<HTMLImageElement> | undefined;
const loadLogo = () =>
  (logo ??= new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = '/layers/logo.webp';
  }));

export async function prepare() {
  await Promise.all([document.fonts.load(`600 22px ${FONT}`), document.fonts.load(`400 11px ${FONT}`), loadLogo()]);
}

function draw(ctx: CanvasRenderingContext2D, layer: Layer, img: HTMLImageElement) {
  const [, , w, h] = layer.box;
  switch (layer.id) {
    case 'frame':
      ctx.fillStyle = '#e9e7f3';
      ctx.fillRect(0, 0, w, h);
      break;
    case 'card':
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.roundRect(0.5, 0.5, w - 1, h - 1, 14);
      ctx.fill();
      ctx.strokeStyle = 'rgba(22, 21, 26, 0.1)';
      ctx.stroke();
      break;
    case 'logo':
      ctx.drawImage(img, 0, 0, w, h);
      break;
    case 'title':
      ctx.fillStyle = '#16151a';
      ctx.font = `600 22px ${FONT}`;
      ctx.textBaseline = 'middle';
      ctx.fillText('Ship the screen', 0, h / 2);
      break;
    case 'body':
      ctx.fillStyle = '#55535c';
      ctx.font = `400 11px ${FONT}`;
      ctx.textBaseline = 'top';
      ctx.fillText('Built from design.fig, then', 0, 1);
      ctx.fillText('checked against the frame.', 0, 16);
      break;
    case 'button':
      ctx.fillStyle = '#5b47b8';
      ctx.beginPath();
      ctx.roundRect(0, 0, w, h, 7);
      ctx.fill();
      ctx.fillStyle = '#ffffff';
      ctx.font = `600 11px ${FONT}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText('Open build', w / 2, h / 2 + 0.5);
      break;
    case 'check':
      ctx.fillStyle = '#a8e6bf';
      ctx.beginPath();
      ctx.arc(w / 2, h / 2, w / 2, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = '#16151a';
      ctx.lineWidth = 2.4;
      ctx.lineCap = ctx.lineJoin = 'round';
      ctx.beginPath();
      ctx.moveTo(w * 0.3, h * 0.52);
      ctx.lineTo(w * 0.45, h * 0.66);
      ctx.lineTo(w * 0.71, h * 0.37);
      ctx.stroke();
      break;
  }
}

export async function paintLayer(layer: Layer, scale: number) {
  const img = await loadLogo();
  const [, , w, h] = layer.box;
  const c = document.createElement('canvas');
  c.width = Math.round(w * scale);
  c.height = Math.round(h * scale);
  const ctx = c.getContext('2d')!;
  ctx.scale(scale, scale);
  draw(ctx, layer, img);
  return c;
}

// The flat screen, for browsers without WebGL.
export async function paintComposite(layers: Layer[], scale: number) {
  const img = await loadLogo();
  const [, , fw, fh] = layers[0].box;
  const c = document.createElement('canvas');
  c.width = fw * scale;
  c.height = fh * scale;
  const ctx = c.getContext('2d')!;
  ctx.scale(scale, scale);
  for (const l of [...layers].sort((a, b) => a.depth - b.depth)) {
    ctx.save();
    ctx.translate(l.box[0], l.box[1]);
    draw(ctx, l, img);
    ctx.restore();
  }
  return c;
}
