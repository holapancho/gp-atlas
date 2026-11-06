const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ deviceScaleFactor: 2, viewport: { width: 1400, height: 900 } });
  for (const [src, dst] of JSON.parse(process.argv[2])) {
    await p.goto('file://' + src);
    await (await p.$('.win')).screenshot({ path: dst, omitBackground: true });
  }
  await b.close();
})();
