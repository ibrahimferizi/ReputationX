import assert from "node:assert/strict";
import { test } from "node:test";
import { coverageSummary, individualFlagReasons } from "../src/utils/coverage.ts";
import { normalizeReport } from "../src/utils/normalizeReport.ts";

test("unknown checks do not produce individual risk flags", () => {
  const wallet = normalizeReport({reputation_score: 93, metrics:[{id:"token_risk", risk:"unknown"},{id:"defi_exposure",risk:"unknown"}]});
  assert.deepEqual(individualFlagReasons(wallet), []);
});

test("a low activity score alone does not flag ownership", () => {
  const wallet = normalizeReport({reputation_score: 40, metrics:[]});
  assert.deepEqual(individualFlagReasons(wallet), []);
});

test("coverage separates sample counts and incomplete token scans", () => {
  for (const [status, label] of [["skipped","Not checked"],["timed_out","Scan timed out"],["unavailable","Unavailable"],["partial","Partial coverage"],["complete","Checks completed"]]) {
    const wallet = normalizeReport({tx_stats:{count:100,sampled:true},coverage:{token_scans:{status,held_mints:67,checked_mints:3,fully_checked_mints:3}}});
    assert.equal(coverageSummary(wallet).history, "100 successful transactions sampled");
    assert.equal(coverageSummary(wallet).tokens, label);
    assert.match(coverageSummary(wallet).tokenDetail, /3 of 67/);
  }
});


test("server rate limits provide retry guidance without leaking raw responses", async () => {
  const { apiError } = await import("../src/utils/apiError.ts");
  const response = new Response("upstream-secret", {status:429, headers:{"Retry-After":"120"}});
  const error = await apiError(response);
  assert.match(error.message, /2 minute/);
  assert.doesNotMatch(error.message, /upstream-secret/);
});


test("null and non-object API failures still show a useful message", async () => {
  const { apiError } = await import("../src/utils/apiError.ts");
  for (const body of ["null", "false", "42"]) {
    assert.match((await apiError(new Response(body, {status:503}))).message, /busy or waking up/);
  }
});
