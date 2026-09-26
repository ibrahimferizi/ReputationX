import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { normalizeReport } from "../src/utils/normalizeReport.ts";

// Render the actual TSX with the existing TypeScript compiler; no new test framework.
async function loadComponent(relative) {
  const file = new URL(relative, import.meta.url);
  const result = ts.transpileModule(readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, jsx: ts.JsxEmit.ReactJSX },
  });
  const code = result.outputText.replace(/from "([^"]+)"/g, (_, specifier) => {
    const target = specifier.startsWith(".")
      ? new URL(`${specifier}.ts`, file).href
      : import.meta.resolve(specifier);
    return `from ${JSON.stringify(target)}`;
  });
  return import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
}
const { ScanCoverage } = await loadComponent("../src/components/ScanCoverage.tsx");
const { RiskIcon } = await loadComponent("../src/components/RiskIcon.tsx");

test("rendered coverage does not hide skipped checks behind a high score", () => {
  const report = normalizeReport({
    reputation_score: 93, tx_stats: { count: 100, sampled: true },
    coverage: { history_source: "helius", timed_transactions: 100,
      token_scans: {status: "skipped", held_mints: 67, checked_mints: 0} },
  });
  const html = renderToStaticMarkup(createElement(ScanCoverage, {report}));
  assert.match(html, /100 successful transactions sampled/);
  assert.match(html, /Tokens: Not checked/);
  assert.match(html, /0 of 67/);
  assert.match(html, /not a verified lifetime total/);
  assert.match(html, /DeFi activity/);
  assert.match(html, /Not assessed/);
  assert.doesNotMatch(html, /Clear|Clean wallet/);
});

test("unknown risk renders a neutral accessible icon", () => {
  const html = renderToStaticMarkup(createElement(RiskIcon, {risk: "unknown"}));
  assert.match(html, /aria-label="Risk not assessed"/);
  assert.doesNotMatch(html, /✓|Low risk/);
});


test("coverage renders actionable provider diagnostics", () => {
  const report = normalizeReport({coverage:{token_scans:{status:"unavailable",issues:[
    {provider:"rugcheck",mint:"MintA",code:"authentication_failed",http_status:401,message:"The provider rejected the configured credential."},
    {provider:"solsniffer",mint:"MintB",code:"report_unavailable",http_status:404,message:"The provider has no report for this token."}
  ]}}});
  const html = renderToStaticMarkup(createElement(ScanCoverage, {report}));
  assert.match(html, /Provider diagnostics/);
  assert.match(html, /HTTP 401/);
  assert.match(html, /HTTP 404/);
  assert.match(html, /rejected the configured credential/);
});
