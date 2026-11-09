// Render the shipped Store frontend with synthetic local data for store images.
// This server is not bundled with the app and never reads a user's configuration.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const dist = resolve(root, 'apps/desktop/dist-store');
const port = Number(process.env.LINKPILOT_ASSET_PORT || 4178);
const types = { '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.html': 'text/html' };
const escape = (s) => s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');

async function safeFile(base, path) {
  const file = resolve(base, '.' + path);
  if (!file.startsWith(base + '/')) throw new Error('Path outside asset root');
  return readFile(file);
}

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://127.0.0.1:${port}`);
    const locale = url.searchParams.get('locale') || 'en-US';
    if (!['en-US', 'zh-Hans', 'zh-Hant', 'ja'].includes(locale)) throw new Error('Unknown locale');
    if (url.pathname === '/shot') {
      const copy = JSON.parse(await readFile(resolve(root, `marketing/app-store/0.5.15/${locale}.json`), 'utf8'));
      const scene = copy.screenshots.find((s) => s.id === url.searchParams.get('scene')) || copy.screenshots[0];
      res.setHeader('Content-Type', 'text/html; charset=utf-8');
      res.end(`<!doctype html><html lang="${locale}" translate="no" class="notranslate"><meta charset="utf-8"><title>LinkPilot ${locale} ${scene.id}</title>
<style>*{box-sizing:border-box}html,body{margin:0;width:1440px;height:900px;overflow:hidden}body{color:#fff;font-family:-apple-system,BlinkMacSystemFont,'Helvetica Neue','PingFang SC','Hiragino Sans',sans-serif;background:radial-gradient(ellipse at 80% 85%,#323054,transparent 60%),radial-gradient(ellipse at 15% 15%,#293d59,transparent 58%),#111122}header{position:absolute;left:62px;top:40px;right:62px}header h1{font-size:43px;font-weight:700;letter-spacing:-1px;line-height:1.2;margin:0 0 14px;max-width:1240px}header p{font-size:21px;color:#d2d6e5;line-height:1.4;margin:0}.brand{position:absolute;right:0;top:1px;color:#aeb8d2;font-size:15px;font-weight:600;letter-spacing:1px}section{position:absolute;left:60px;top:180px;width:1320px;height:666px;border-radius:15px;overflow:hidden;border:1px solid #ffffff36;box-shadow:0 28px 70px #0007;background:#212227}.chrome{height:25px;background:#292b34;border-bottom:1px solid #ffffff0c;display:flex;align-items:center;padding-left:13px;gap:7px}.dot{width:9px;height:9px;border-radius:50%;background:#ff6057}.dot:nth-child(2){background:#ffbd2e}.dot:nth-child(3){background:#28c840}doubao-ai-csui,[id="doubao-ai-assistant"],[id^="doubao-ai-translate-image-assistant"]{display:none!important}iframe{display:block;border:0;width:1320px;height:641px}</style>
<header><div class="brand">LinkPilot</div><h1>${escape(scene.headline)}</h1><p>${escape(scene.caption)}</p></header>
<section><div class="chrome"><i class="dot"></i><i class="dot"></i><i class="dot"></i></div><iframe title="LinkPilot" src="/app.html?locale=${locale}"></iframe></section></html>`);
    } else if (url.pathname === '/app.html') {
      let html = await readFile(resolve(dist, 'index.html'), 'utf8');
      html = html.replace(/<html[^>]*>/, `<html lang="${locale}" translate="no" class="notranslate">`);
      html = html.replace('<head>', '<head><meta name="google" content="notranslate"><script src="/fixture.js"></script><style>doubao-ai-csui,[id="doubao-ai-assistant"],[id^="doubao-ai-translate-image-assistant"]{display:none!important}</style>');
      res.setHeader('Content-Type', 'text/html; charset=utf-8');
      res.end(html);
    } else if (/^\/icons\/(chrome|brave|safari)\.png$/.test(url.pathname)) {
      res.setHeader('Content-Type', 'image/png');
      res.end(await readFile(resolve(root, 'tools/store-assets' + url.pathname)));
    } else if (url.pathname === '/fixture.js') {
      res.setHeader('Content-Type', 'text/javascript; charset=utf-8');
      res.end(await readFile(resolve(root, 'tools/store-assets/fixture.js')));
    } else {
      res.setHeader('Content-Type', types[extname(url.pathname)] || 'application/octet-stream');
      res.end(await safeFile(dist, decodeURIComponent(url.pathname)));
    }
  } catch (error) {
    res.statusCode = 404;
    res.end(String(error.message));
  }
});
server.listen(port, '127.0.0.1', () => console.log(`Store asset preview: http://127.0.0.1:${port}/shot?locale=en-US&scene=01-rules`));
