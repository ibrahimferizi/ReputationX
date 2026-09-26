"""Production UI against mock providers. Requires Playwright externally installed; no paid calls."""
from http.server import ThreadingHTTPServer
from pathlib import Path
import subprocess
import threading
from public_smoke import CountingProvider, api

class BrowserProvider(CountingProvider):
    def do_GET(self):
        if self.path.startswith('/token/'):
            self.send_json({'tokenData': {'score': 74}})
        else:
            self.send_json({'score': 5000 if 'MintB' in self.path else 0})

server = ThreadingHTTPServer(('127.0.0.1', 0), BrowserProvider)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    url = f'http://127.0.0.1:{server.server_port}'
    with api(url, SKIP_EXTERNAL_SCANS='false', MAX_TOKEN_SCANS='2', RUGCHECK_BASE_URL=url,
             SOLSNIFFER_BASE_URL=url, SOLSNIFFER_API_KEY='mock-key', SOLSNIFFER_MAX_CALLS_PER_RUN='10') as request:
        subprocess.run(['node', str(Path(__file__).with_suffix('.mjs')), request.base_url], check=True)
finally:
    server.shutdown()
    server.server_close()
