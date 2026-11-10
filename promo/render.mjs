// Renders promo/index.html to an MP4 (or to review stills).
//   node render.mjs video AUDIO.wav OUT.mp4
//   node render.mjs stills OUT_DIR t1 t2 t3 ...
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const url = 'file://' + path.join(here, 'index.html');
const FPS = 30, DURATION = 44;
const [mode, ...args] = process.argv.slice(2);
const chrome = process.env.CHROME || '';

const browser = await chromium.launch({ executablePath: chrome || undefined, args: ['--no-sandbox', '--disable-gpu', '--font-render-hinting=none'] });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('pageerror', e => console.error('PAGE ERROR:', e.message));
page.on('console', m => { if (m.type() === 'error') console.error('CONSOLE:', m.text()); });
await page.goto(url, { waitUntil: 'networkidle' });
await page.evaluate(() => window.init());

if (mode === 'stills') {
  const [dir, ...ts] = args; mkdirSync(dir, { recursive: true });
  for (const t of ts) { await page.evaluate(t => window.seek(t), +t); writeFileSync(path.join(dir, `t${String(t).padStart(5, '0')}.png`), await page.screenshot({ type: 'png' })); }
} else if (mode === 'video') {
  const [audio, out] = args;
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'png', '-i', '-', '-i', audio,
    '-map', '0:v', '-map', '1:a', '-c:v', 'libx264', '-preset', 'slow', '-crf', '17', '-pix_fmt', 'yuv420p', '-movflags', '+faststart',
    '-c:a', 'aac', '-b:a', '192k', '-t', String(DURATION), out], { stdio: ['pipe', 'inherit', 'inherit'] });
  const frames = FPS * DURATION;
  for (let i = 0; i < frames; i++) {
    await page.evaluate(t => window.seek(t), i / FPS);
    const buf = await page.screenshot({ type: 'png' });
    if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once('drain', r));
    if (i % 60 === 0) console.log(`frame ${i}/${frames}`);
  }
  ff.stdin.end();
  await new Promise(r => ff.on('close', r));
}
await browser.close();
