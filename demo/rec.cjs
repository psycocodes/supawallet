const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: __dirname + '/raw', size: { width: 1280, height: 720 } } });
  const p = await ctx.newPage(); await p.goto('file://' + __dirname + '/demo.html'); await p.waitForTimeout(46000);
  await ctx.close(); await b.close();
})();
