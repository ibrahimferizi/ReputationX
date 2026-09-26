"""Run after `cargo build --locked`. Uses local mock providers, never mainnet."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

BINARY = Path(__file__).resolve().parents[1] / "target/debug/walletguard-api"
ADDRESS = "11111111111111111111111111111111"


class Provider(BaseHTTPRequestHandler):
    unavailable = False
    empty_holdings = False
    risky = False

    def log_message(self, *_args):
        pass

    def send_json(self, value, status=200):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.send_json({"message": "unavailable"} if self.unavailable else {"score": 5000 if self.risky else 0},
                       503 if self.unavailable else 200)

    def do_POST(self):
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        method = payload["method"]
        if method == "getBalance":
            result = {"context": {"slot": 1}, "value": 16189_609500000}
        elif method == "getTokenAccountsByOwner":
            holdings = [("EmptyMint", "0")]
            if not self.empty_holdings:
                holdings += [("MintA", "1"), ("MintB", "1"), ("MintA", "1")]
            result = {"value": [
                {"account": {"data": {"parsed": {"info": {
                    "mint": mint, "tokenAmount": {"amount": amount, "uiAmount": int(amount), "decimals": 0}
                }}}}} for mint, amount in holdings
            ]}
        elif method == "getSignaturesForAddress":
            # 100 successes + 1 failure; a separate request exhausts the history.
            first = int(time.time()) - 1600 * 86400
            result = [] if payload["params"][1].get("before") else [
                {"signature": f"sig{i}", "blockTime": first + i * 3600, "err": None}
                for i in range(100)
            ] + [{"signature": "failed", "blockTime": first, "err": "failed"}]
        elif method == "getTransaction":
            result = {"meta": {"err": None, "preBalances": [], "postBalances": []},
                      "transaction": {"message": {"accountKeys": [], "instructions": []}}}
        else:
            raise AssertionError(f"Unexpected RPC method: {method}")
        self.send_json({"jsonrpc": "2.0", "id": payload["id"], "result": result})


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def run_case(provider_url, expected_status, *, skip=False, cap=2, unavailable=False, empty=False, risky=False):
    Provider.risky = risky
    Provider.unavailable = unavailable
    Provider.empty_holdings = empty
    port = free_port()
    env = os.environ | {
        "PORT": str(port), "SOLANA_RPC_URL": provider_url, "SOLANA_HISTORY_RPC_URL": provider_url,
        "HELIUS_API_KEY": "", "RUGCHECK_API_KEY": "", "SOLSNIFFER_API_KEY": "",
        "RUGCHECK_BASE_URL": provider_url, "SCAN_MODE": "fast",
        "SKIP_EXTERNAL_SCANS": str(skip).lower(), "MAX_TOKEN_SCANS": str(cap),
        "RPC_MAX_RETRIES": "0", "SIGNATURE_PAGE_SIZE": "100", "MAX_AGE_SIGNATURE_PAGES": "2",
        "FUNDING_MAX_SIGNATURE_PAGES": "2", "FUNDING_PAGE_DELAY_MS": "0",
        "CORS_ALLOWED_ORIGINS": "https://walletguard.example",
        "IP_SCAN_UNITS_PER_MINUTE": "1000",
    }

    def request(path, method="GET", body=None, headers=None):
        req = urllib.request.Request(
            f"http://127.0.0.1:{port}{path}", method=method,
            data=json.dumps(body).encode() if body is not None else None, headers=headers or {})
        try:
            response = urllib.request.urlopen(req, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, response.headers, response.read().decode()

    with tempfile.TemporaryDirectory(prefix="walletguard-test-") as cwd:
        with open(Path(cwd) / "api.log", "w") as log:
            process = subprocess.Popen([str(BINARY)], cwd=cwd, env=env, stdout=log, stderr=log)
            try:
                for _ in range(100):
                    try:
                        if request("/health")[0] == 200:
                            break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise AssertionError("API startup failed")
                for origin, allowed in [("https://walletguard.example", True), ("https://untrusted.example", False)]:
                    status, headers, _ = request("/api/sybil-scan", "OPTIONS", headers={
                        "Origin": origin, "Access-Control-Request-Method": "POST",
                        "Access-Control-Request-Headers": "content-type"})
                    assert status == 200
                    assert (headers.get("Access-Control-Allow-Origin") == origin) == allowed
                    if allowed:
                        assert "POST" in headers.get("Access-Control-Allow-Methods", "")
                        assert "content-type" in headers.get("Access-Control-Allow-Headers", "").lower()
                assert request("/api/reputation?address=bad")[0] == 400
                status, _, body = request(f"/api/reputation?address={ADDRESS}")
                assert status == 200, body
                report = json.loads(body)
                assert report["api_version"] == "walletguard-v2"
                assert report["scoring_version"] == "activity-v3"
                assert report["reputation_score"] == 93
                assert report["tx_stats"]["count"] == 100  # Excludes failed transaction.
                assert report["tx_stats"]["sampled"] is True
                assert report["defi_exposure"] == {"total_usd": None, "interaction_count": None}
                coverage = report["coverage"]["token_scans"]
                assert coverage["status"] == expected_status, coverage
                assert coverage["held_mints"] == (0 if empty else 2)  # Empty accounts + duplicate mints excluded.
                if unavailable:
                    assert len(coverage["issues"]) == 2, coverage
                    assert all(issue["provider"] == "rugcheck" and
                               issue["code"] == "provider_error" and
                               issue["http_status"] == 503 for issue in coverage["issues"])
                else:
                    assert coverage["issues"] == [], coverage
                metrics = {metric["id"]: metric for metric in report["metrics"]}
                assert metrics["tx_count"]["value"] == "100 sampled"
                assert metrics["defi_exposure"]["risk"] == "unknown"
                assert metrics["token_risk"]["risk"] == ("high" if risky else "low" if expected_status == "complete" else "unknown")
                status, _, body = request("/api/sybil-scan", "POST", {"addresses": [ADDRESS, ADDRESS]},
                                          {"Content-Type": "application/json"})
                batch = json.loads(body)
                assert status == 200, body
                assert batch["total_scanned"] == 1
                assert batch["clusters"] == []
                assert batch["flagged"] == (1 if risky else 0)  # Token flags do not alter the activity score.
                assert batch["wallets"][0]["coverage"]["token_scans"]["status"] == expected_status
                print(f"PASS: API + batch coverage={expected_status}, score=93, sampling, null DeFi, CORS")
            finally:
                process.terminate()
                process.wait(timeout=10)


def main():
    server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        url = f"http://127.0.0.1:{server.server_port}"
        run_case(url, "skipped", skip=True)
        run_case(url, "partial", cap=1)
        run_case(url, "complete")
        run_case(url, "unavailable", unavailable=True)
        run_case(url, "no_holdings", empty=True)
        run_case(url, "partial", cap=1, risky=True)
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
