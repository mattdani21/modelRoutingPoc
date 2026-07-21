const assert = require('node:assert/strict');
const fs = require('node:fs');

const html = fs.readFileSync('web/index.html', 'utf8');
assert.ok(html.indexOf('logic.js') < html.indexOf('app.js'), 'policy logic must load before the app');
assert.match(html, /id="reviewer-input"/);
assert.match(html, /for="task-select"/);
assert.match(html, /for="model-select"/);
assert.match(html, /aria-live="polite"/);

console.log('html contract tests passed');
