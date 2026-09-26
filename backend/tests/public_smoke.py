"""Public-service regression checks using local RPC mocks; never calls mainnet."""
import concurrent.futures
from contextlib import contextmanager
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer
from api_smoke import Provider, BINARY, ADDRESS, free_port

OTHER = "11111111111111111111111111111112"

class CountingProvider(Provider):
    count = 0
    delay = 0
    fail = False
    lock = threading.Lock()

    def do_POST(self):
        with self.lock:
            type(self).count += 1
        if type(self).fail:
            self.send_json({"jsonrpc":"2.0", "id":1, "error":{"code":-32000,"message":"TEST_SECRET_MUST_NOT_APPEAR"}})
            return
        time.sleep(type(self).delay)
        try:
            super().do_POST()
        except (BrokenPipeError, ConnectionResetError):
            pass  # Expected when the API cancels a timed-out request.


@contextmanager
def api(provider_url, **overrides):
    port = free_port()
    env = os.environ | {
        "PORT": str(port), "SOLANA_RPC_URL": provider_url, "SOLANA_HISTORY_RPC_URL": provider_url,
        "HELIUS_API_KEY": "", "RUGCHECK_API_KEY": "", "SOLSNIFFER_API_KEY": "",
        "SCAN_MODE": "fast", "SKIP_EXTERNAL_SCANS": "true", "RPC_MAX_RETRIES": "0",
        "SIGNATURE_PAGE_SIZE": "100", "MAX_AGE_SIGNATURE_PAGES": "2",
        "FUNDING_MAX_SIGNATURE_PAGES": "2", "FUNDING_PAGE_DELAY_MS": "0",
        "IP_SCAN_UNITS_PER_MINUTE": "1000", "FRESH_SCANS_PER_HOUR": "100",
        "CACHE_TTL_SECONDS": "300", "TRUSTED_PROXY_IPS": "", "MAX_ACTIVE_REQUESTS": "8",
        "WALLET_TIMEOUT_SECONDS": "5", "REQUEST_TIMEOUT_SECONDS": "10",
        "STATIC_DIR": str(Path(__file__).resolve().parents[2] / "frontend/dist"),
    } | overrides

    def request(path, body=None, headers=None):
        req = urllib.request.Request(f"http://127.0.0.1:{port}{path}",
            data=json.dumps(body).encode() if body is not None else None,
            headers={"Content-Type": "application/json"} | (headers or {}))
        try:
            response = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, response.headers, response.read().decode()

    request.base_url = f"http://127.0.0.1:{port}"
    with tempfile.TemporaryDirectory(prefix="walletguard-public-") as cwd:
        with open(Path(cwd) / "api.log", "w") as log:
            process = subprocess.Popen([str(BINARY)], cwd=cwd, env=env, stdout=log, stderr=log)
            try:
                for _ in range(100):
                    try:
                        if request("/health")[0] == 200: break
                    except OSError: time.sleep(0.05)
                else: raise AssertionError("API did not start")
                yield request
            finally:
                process.terminate()
                process.wait(timeout=15)
                assert process.returncode == 0, "Graceful SIGTERM failed"
                assert "TEST_SECRET_MUST_NOT_APPEAR" not in (Path(cwd) / "api.log").read_text()


def main():
    server = ThreadingHTTPServer(("127.0.0.1", 0), CountingProvider)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{server.server_port}"
    path = f"/api/reputation?address={ADDRESS}"
    try:
        with api(url, FRESH_SCANS_PER_HOUR="1") as request:
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
                reports = list(pool.map(lambda _: request(path), range(4)))
            assert all(status == 200 for status, _, _ in reports), reports
            assert len({json.loads(body)["generated_at"] for _, _, body in reports}) == 1
            count = CountingProvider.count
            assert request(path)[0] == 200
            assert CountingProvider.count == count, "Cache hit called upstream"
            status, headers, _ = request(f"/api/reputation?address={OTHER}")
            assert status == 429 and int(headers["Retry-After"]) > 0
            assert CountingProvider.count == count, "Exhausted budget called upstream"
            assert request("/api/sybil-scan", {"addresses": [ADDRESS] * 11})[0] == 400
            assert request("/api/sybil-scan", {"addresses": ["x" * 9000]})[0] == 413
            assert request("/api/unknown")[0] == 404
            status, headers, body = request("/")
            assert status == 200 and '<div id="root">' in body
            assert "frame-ancestors 'none'" in headers["Content-Security-Policy"]
            assert headers["X-Content-Type-Options"] == "nosniff"
            assert request("/.env")[0] == 404
        print("PASS: coalesced misses, cached reuse, global budget, body/batch limits, static UI, headers, SIGTERM")

        with api(url, IP_SCAN_UNITS_PER_MINUTE="2") as request:
            assert request(path)[0] == 200
            assert request(path)[0] == 200
            status, headers, _ = request(path, headers={"X-Forwarded-For": "192.0.2.1", "X-WalletGuard-Client-IP": "192.0.2.2"})
            assert status == 429 and "Retry-After" in headers
            assert request("/health")[0] == 200
        print("PASS: per-peer rate limit, forged forwarding headers ignored, health remains available")

        with api(url, CACHE_TTL_SECONDS="1") as request:
            old = json.loads(request(path)[2])["generated_at"]
            time.sleep(1.1)
            assert json.loads(request(path)[2])["generated_at"] != old
        print("PASS: expired reports are regenerated")

        with api(url, CACHE_MAX_WALLETS="1") as request:
            assert request(path)[0] == 200
            assert request(f"/api/reputation?address={OTHER}")[0] == 200
            count = CountingProvider.count
            assert request(path)[0] == 200
            assert CountingProvider.count > count, "Cache did not evict at its capacity"
        print("PASS: cache capacity bounds retained reports")

        CountingProvider.fail = True
        with api(url) as request:
            status, _, body = request(path)
            assert status == 500 and "TEST_SECRET_MUST_NOT_APPEAR" not in body
            count = CountingProvider.count
            assert request(path)[0] == 500
            assert CountingProvider.count > count, "An error was cached"
        CountingProvider.fail = False
        print("PASS: failed scans are not cached and upstream secrets are absent from errors/logs")

        CountingProvider.delay = 2
        with api(url, MAX_ACTIVE_REQUESTS="1", REQUEST_TIMEOUT_SECONDS="1", WALLET_TIMEOUT_SECONDS="5") as request:
            with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                future = pool.submit(request, "/api/sybil-scan", {"addresses": [ADDRESS, OTHER]})
                time.sleep(0.2)
                assert request(path)[0] == 503
                assert request("/health")[0] == 200
                assert future.result()[0] == 504
            count = CountingProvider.count
            time.sleep(2.2)
            assert CountingProvider.count == count, "Batch continued upstream work after deadline"
        print("PASS: overload rejection, request deadline, batch work stops after cancellation")
        with api(url, WALLET_TIMEOUT_SECONDS="1", REQUEST_TIMEOUT_SECONDS="10") as request:
            started = time.monotonic()
            assert request(path)[0] == 504
            assert time.monotonic() - started < 2
        print("PASS: wallet deadline applies before the longer request deadline")
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
