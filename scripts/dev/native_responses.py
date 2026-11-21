"""Deterministic Responses streams for the native provider SSH smoke."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


@contextmanager
def model(approval=False, reconnect_marker=None):
    calls = []

    class Endpoint(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            if self.path != "/v1/responses":
                self.send_error(404)
                return
            raw = self.rfile.read(int(self.headers["Content-Length"]))
            calls.append(reconnect_marker is not None and reconnect_marker.exists())
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
            if approval:
                if self.headers.get("Content-Encoding") == "zstd":
                    from compression.zstd import decompress
                    raw = decompress(raw)
                tools = {t.get("name"): t for t in json.loads(raw).get("tools", [])}
                name = next((n for n in ("exec_command", "shell", "bash", "shell_command") if n in tools), None)
                if name:
                    fields = tools[name].get("parameters", {}).get("properties", {})
                    args = {"cmd" if "cmd" in fields else "command": "mkdir approval-fixture"}
                    if "description" in fields:
                        args["description"] = "Create a fixture directory"
                    call = {"type":"function_call", "id":"fc_fixture", "call_id":"call_fixture",
                            "name":name, "arguments":json.dumps(args), "status":"completed"}
                    response["output"] = [call]
                    events = [events[0], {"type":"response.output_item.added", "output_index":0, "item":{**call,"status":"in_progress","arguments":""}},
                              {"type":"response.function_call_arguments.delta","output_index":0,"item_id":call["id"],"delta":call["arguments"]},
                              {"type":"response.function_call_arguments.done","output_index":0,"item_id":call["id"],"arguments":call["arguments"]},
                              {"type":"response.output_item.done","output_index":0,"item":call}, events[-1]]
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
