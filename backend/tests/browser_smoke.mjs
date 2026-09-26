import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const {chromium} = createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright');
const browser = await chromium.launch({headless:true});
try {
  const page = await browser.newPage({viewport:{width:1280,height:900}});
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  await page.goto(process.argv[2]);
  assert.match(await page.title(), /WalletGuard/);
  await page.locator('#wallet').fill('11111111111111111111111111111111');
  await page.getByRole('button', {name:'Check wallet',exact:true}).click();
  await page.locator('.score').waitFor();
  assert.equal(await page.locator('.score').textContent(), '93');
  assert.match(await page.locator('body').textContent(), /SolSniffer checked/);
  assert.match(await page.locator('body').textContent(), /1 token\(s\) flagged/);
  await page.locator('.metric-row').filter({hasText:'Token / Honeypot Risk'}).getByRole('button', {name:'Show review notes',exact:true}).click();
  assert.match(await page.locator('body').textContent(), /without the wallet owner's involvement/);
  await page.getByRole('button', {name:'Sybil Scan',exact:true}).click();
  await page.locator('#sybil-addresses').fill('11111111111111111111111111111111\n11111111111111111111111111111112');
  await page.getByRole('button', {name:'Scan',exact:true}).click();
  await page.waitForFunction(() => document.body.textContent.includes('2 wallets scanned'));
  assert.match(await page.locator('body').textContent(), /2 flagged/);
  assert.equal(await page.locator('canvas').count(), 1);
  for (const width of [375, 320]) {
    await page.setViewportSize({width,height:812});
    // Wait for the graph's ResizeObserver and canvas frame to catch up.
    await page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth + 1);
  }
  await page.getByRole('button', {name:'Wallet Check',exact:true}).click();
  await page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth + 1);
  await page.getByText('About this beta · Data and privacy', {exact:true}).click();
  await page.getByRole('button', {name:'Clear WalletGuard data in this browser'}).click();
  await page.waitForLoadState();
  assert.equal(await page.evaluate(() => Object.keys(localStorage).filter(k => k.startsWith('walletguard_')).length), 0);
  assert.equal(await page.evaluate(() => Object.keys(sessionStorage).filter(k => k.startsWith('walletguard_')).length), 0);
  assert.deepEqual(failures, []);
  console.log('PASS: production Chromium UI, wallet scan, token timestamps/flags, review notes, batch/canvas, mobile fit, clear data, no page errors');
} finally { await browser.close(); }
