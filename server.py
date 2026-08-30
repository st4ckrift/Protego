import json
import urllib.parse
import logging
from http.server import HTTPServer, ThreadingHTTPServer, BaseHTTPRequestHandler
from pathlib import Path
import sys

BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

from gate.config import PORT, HOST
from gate.database import init_db, get_connection
from gate.models import AuthorizeRequest
from gate.gate_service import authorize_transaction, get_all_mandates
from gate.audit_service import get_audit_logs
from agent.agent_service import process_agent_request

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s")
logger = logging.getLogger("server")

STATIC_DIR = BASE_DIR / "static"

class GateHTTPRequestHandler(BaseHTTPRequestHandler):

    def send_json(self, data: dict, status_code: int = 200):
        body = json.dumps(data).encode("utf-8")
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def serve_static(self, file_path: Path, content_type: str):
        if not file_path.exists():
            self.send_json({"error": "File not found"}, 404)
            return
        with open(file_path, "rb") as f:
            content = f.read()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        qs = urllib.parse.parse_qs(parsed.query)

        if path == "/" or path == "/index.html":
            self.serve_static(STATIC_DIR / "index.html", "text/html")
        elif path == "/app.js":
            self.serve_static(STATIC_DIR / "app.js", "application/javascript")
        elif path == "/style.css":
            self.serve_static(STATIC_DIR / "style.css", "text/css")
        elif path == "/gate/mandates":
            mandates = get_all_mandates()
            self.send_json({"status": "success", "mandates": mandates})
        elif path == "/gate/audit":
            mandate_id = qs.get("mandate_id", [None])[0]
            user_id = qs.get("user_id", [None])[0]
            merchant_id = qs.get("merchant_id", [None])[0]
            decision = qs.get("decision", [None])[0]
            limit = int(qs.get("limit", [50])[0])
            
            logs = get_audit_logs(
                mandate_id=mandate_id,
                user_id=user_id,
                merchant_id=merchant_id,
                decision=decision,
                limit=limit
            )
            self.send_json({"status": "success", "count": len(logs), "logs": logs})
        else:
            self.send_json({"error": "Not Found"}, 404)

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        content_len = int(self.headers.get("Content-Length", 0))
        post_data = self.rfile.read(content_len) if content_len > 0 else b"{}"
        
        try:
            payload = json.loads(post_data.decode("utf-8")) if post_data else {}
        except json.JSONDecodeError:
            self.send_json({"error": "Invalid JSON body"}, 400)
            return

        if path == "/gate/authorize":
            user_id = payload.get("user_id")
            merchant_id = payload.get("merchant_id")
            amount = payload.get("amount")
            mandate_id = payload.get("mandate_id")

            if not user_id or not merchant_id or amount is None:
                self.send_json({
                    "error": "Missing required fields: user_id, merchant_id, amount"
                }, 400)
                return

            req = AuthorizeRequest(
                user_id=user_id,
                merchant_id=merchant_id,
                amount=int(amount),
                mandate_id=mandate_id
            )
            decision = authorize_transaction(req)
            dec_dict = decision.to_dict()
            
            # Return status code based on decision (200 for approved, 400 for denied/error)
            status_code = 200 if decision.decision == "approved" else 400
            self.send_json(dec_dict, status_code)

        elif path == "/gate/mandates":
            # Create a new mandate
            mandate_id = payload.get("mandate_id")
            user_id = payload.get("user_id")
            merchant_id = payload.get("merchant_id")
            max_per_txn = payload.get("max_per_txn")
            max_per_day = payload.get("max_per_day")
            max_total = payload.get("max_total")

            if not all([mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total]):
                self.send_json({"error": "Missing mandatory mandate parameters"}, 400)
                return

            conn = get_connection()
            try:
                from datetime import date, datetime, timezone
                cursor = conn.cursor()
                cursor.execute("""
                INSERT INTO mandates (
                    mandate_id, user_id, merchant_id, max_per_txn, max_per_day, max_total,
                    spent_today, spent_total, last_spent_date, expires_at, created_at, status
                ) VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?, 'active')
                """, (
                    mandate_id,
                    user_id,
                    merchant_id,
                    int(max_per_txn),
                    int(max_per_day),
                    int(max_total),
                    date.today().isoformat(),
                    payload.get("expires_at", "2027-12-31T23:59:59Z"),
                    datetime.now(timezone.utc).isoformat()
                ))
                conn.commit()
                self.send_json({"status": "success", "mandate_id": mandate_id}, 201)
            except Exception as e:
                conn.rollback()
                self.send_json({"error": f"Failed to create mandate: {str(e)}"}, 400)
            finally:
                conn.close()

        elif path == "/agent/chat":
            prompt = payload.get("prompt")
            user_id_override = payload.get("user_id")
            if not prompt:
                self.send_json({"error": "Missing prompt field"}, 400)
                return

            result = process_agent_request(prompt, user_id_override=user_id_override)
            self.send_json(result, 200)

        else:
            self.send_json({"error": "Endpoint not found"}, 404)

def run_server(host=HOST, port=PORT):
    init_db()
    server_address = (host, port)
    httpd = ThreadingHTTPServer(server_address, GateHTTPRequestHandler)
    logger.info(f"🛡️  Spend-Gate Server running at http://{host}:{port}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        logger.info("Server shutting down.")
        httpd.server_close()

if __name__ == "__main__":
    run_server()
