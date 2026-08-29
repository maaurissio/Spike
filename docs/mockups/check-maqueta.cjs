// Comprobación sin dependencias de la lógica y las celdas de la maqueta.
// No sustituye las pruebas visuales en navegador o terminal.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const html = fs.readFileSync(path.join(__dirname, 'vtracker-maqueta.source.html'), 'utf8');
const exported = fs.readFileSync(path.join(__dirname, 'vtracker-maqueta.html'), 'utf8').replace(/\r\n/g, '\n');
const escaped = html.replace(/\r\n/g, '\n').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#x27;');
assert.ok(exported.includes(escaped), 'Regenerar el exportado: no coincide con el fragmento');
let markup = '', buttons = [], handlers = {}, focused;
const content = {
  set innerHTML(value) {
    markup = value;
    buttons = [...value.matchAll(/data-action="([^"]+)"/g)].map(m => ({
      dataset: {action: m[1]}, events: {},
      addEventListener(k, fn) { this.events[k] = fn; },
      focus() { focused = this.dataset.action; },
      getClientRects() { return [1]; },
    }));
  },
  querySelectorAll() { return buttons; },
};
const root = {dataset: {}, querySelector() { return content; }, addEventListener(k, fn) { handlers[k] = fn; }};
vm.runInNewContext(html.match(/<script>([\s\S]*?)<\/script>/)[1], {document: {getElementById() { return root; }}});
function screens() {
  return [...markup.matchAll(/<pre class="term-screen term-(large|small)"[^>]*>([\s\S]*?)<\/pre>/g)].map(m => ({
    width: m[1] === 'large' ? 72 : 38,
    text: m[2].replace(/<[^>]*>/g, ''),
  }));
}
function check() {
  assert.equal(screens().length, 2);
  for (const screen of screens()) for (const row of screen.text.split('\n'))
    assert.equal(row.length, screen.width, `Ancho ${screen.width}: ${row}`);
}
function click(action) {
  const btn = buttons.find(b => b.dataset.action === action);
  assert.ok(btn, `Control ${action}`);
  btn.events.click(); check();
}
function key(value) { handlers.keydown({key: value, preventDefault() {}}); check(); }
check();
assert.equal(buttons.filter(b => b.dataset.action.startsWith('player:')).length, 20);
assert.ok(screens()[0].text.split('\n').length <= 24);
assert.ok(screens()[1].text.split('\n').length <= 26);
const first = screens()[0].text;
assert.ok(first.indexOf('ALIADOS') < first.indexOf('TUS RONDAS'));
assert.ok(first.indexOf('TUS RONDAS') < first.indexOf('ENEMIGOS'));
const timeline = first.slice(first.indexOf('TUS RONDAS'), first.indexOf('ENEMIGOS')).split('\n').slice(1,4);
assert.equal(timeline.length, 3);
assert.match(timeline[0], /1K\s+0K\s+2K\s+1K\s+4K\s+0K\s+—K/);
assert.match(timeline[2], /1D\s+1D\s+0D\s+1D\s+0D\s+1D\s+—D/);
assert.match(timeline[1], /R7\*/);
assert.match(first, /8 KILLS \/ 4 MUERTES/);
click('tracker:0'); assert.match(screens()[0].text, /Norte \/ Sova/);
assert.match(screens()[0].text, /Riot ID verificado/);
assert.match(markup, /<button[^>]*disabled[^>]*>\[Abrir Tracker.gg\]<\/button>/);
assert.doesNotMatch(markup, /href=|https:\/\/tracker.gg\/valorant\/profile/);
key('Escape');
click('player:6'); assert.match(screens()[0].text, /estadísticas ocultas/);
key('g'); assert.match(screens()[0].text, /Identidad oculta: sin enlace/);
assert.doesNotMatch(screens()[0].text.split('Oculto / Cypher')[1], /HS .* ADR/);
key('ArrowDown'); assert.equal(focused, 'player:7'); key('Escape');
for (let i = 0; i < 5; i++) click('tab:' + i);
click('interval'); assert.match(screens()[0].text, /Detección\s+5 s/);
click('logs'); assert.match(screens()[0].text, /Log cambios\s+\[x\]/);
for (const expected of ['dark','light','mono','system']) { click('theme'); assert.equal(root.dataset.theme, expected); }
click('tab:3'); key('ArrowDown'); assert.equal(focused, 'history:1');
click('history:1'); assert.match(screens()[0].text, /DERROTA/); assert.match(screens()[0].text, /Haven/);
click('post'); assert.match(screens()[0].text, /TUS RONDAS/);
click('tab:3'); click('history:2'); assert.match(screens()[0].text, /VICTORIA/); assert.match(screens()[0].text, /Bind/);
key('1'); click('match'); assert.match(screens()[0].text, /ENEMIGOS/);
console.log('OK: 72/38 columnas; partida <=24/26 líneas; K/D numéricos; 10 jugadores; Tracker simulado sin enlaces reales; privacidad; cinco vistas y controles.');
