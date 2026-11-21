"""A deterministic, tool-free Responses stream for the native provider SSH smoke."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


@contextmanager
def model():
    calls = []

    class Endpoint(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            if self.path != "/v1/responses":
                self.send_error(404)
                return
            self.rfile.read(int(self.headers["Content-Length"]))
            calls.append(self.path)
            answer = "Confirmed in the VM checkout."
            message = {"type": "message", "role": "assistant", "id": "msg_fixture", "status": "completed",
                       "content": [{"type": "output_text", "text": answer, "annotations": []}]}
            response = {"id": "resp_fixture", "object": "response", "status": "completed", "output": [message],
                        "usage": {"input_tokens": 3, "output_tokens": 4, "total_tokens": 7}}
            events = [
                {"type": "response.created", "response": {**response, "status": "in_progress", "output": []}},
                {"type": "response.output_item.added", "output_index": 0, "item": {**message, "status": "in_progress", "content": []}},
                {"type": "response.output_text.delta", "output_index": 0, "content_index": 0, "item_id": message["id"], "delta": answer},
                {"type": "response.output_item.done", "output_index": 0, "item": message},
                {"type": "response.completed", "response": response},
            ]
            body = "".join(f"event: {e['type']}\ndata: {json.dumps(e)}\n\n" for e in events).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    with ThreadingHTTPServer(("127.0.0.1", 0), Endpoint) as server:
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            yield f"http://127.0.0.1:{server.server_port}/v1", calls
        finally:
            server.shutdown()
            thread.join()
