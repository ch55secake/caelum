#!/usr/bin/env python3
import json
import base64
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit, parse_qs

PROJECT = os.getcwd()
PASSWORD = os.environ.get("OPENCODE_SERVER_PASSWORD") or "fake-secret"
SESSION = {"id": "ses_fake", "title": "Fake session", "time": {"updated": 123}, "location": {"directory": PROJECT}, "model": {"providerID": "local", "id": "original"}, "agent": "build"}
INBOX = {}
FORMS = {name: {"id": name, "sessionID": "ses_fake", "title": "Questions", "fields": [
    {"key": "q0", "type": "multiselect", "title": "Targets", "custom": True, "options": [{"label": "macOS", "value": "macOS"}], "required": True}
]} for name in ["frm_live", "frm_saved", "frm_external", "frm_cancelled"]}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def authorized(self):
        expected = "Basic " + base64.b64encode(("opencode:" + PASSWORD).encode()).decode()
        if self.headers.get("Authorization") == expected:
            return True
        self.send_error(401)
        return False

    def send_json(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if not self.authorized():
            return
        path = urlsplit(self.path).path
        if path == "/api/event":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            events = [
                {"type": "form.created", "location": {"directory": PROJECT}, "data": {"form": FORMS["frm_live"]}},
                {"type": "form.created", "location": {"directory": os.path.dirname(PROJECT)}, "data": {"form": dict(FORMS["frm_live"], id="frm_foreign")}},
                {"id": "evt_fake", "type": "session.text.delta", "location": {"directory": PROJECT}, "data": {"sessionID": "ses_fake", "assistantMessageID": "msg_fake", "delta": "hello"}},
                {"id": "evt_permission", "type": "permission.asked", "location": {"directory": PROJECT}, "data": {"id": "per_fake", "sessionID": "ses_fake", "action": "bash", "resources": ["echo hello"], "message": "Run a command?"}}
            ]
            for event in events:
                self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
            self.wfile.flush()
            try:
                while True:
                    time.sleep(0.05)
                    marker = os.path.join(PROJECT, "external-form-resolution")
                    if os.path.exists(marker):
                        os.unlink(marker)
                        for name, kind in [("frm_external", "form.replied"), ("frm_cancelled", "form.cancelled")]:
                            FORMS.pop(name, None)
                            # No location: snapshots/created events establish ownership.
                            event = {"type": kind, "data": {"id": name, "sessionID": "ses_fake", "answer": {"q0": ["external"]}}}
                            self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                    self.wfile.write(b": keepalive\n\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
        elif path == "/api/experimental/media/" + "a" * 64:
            body = b"\x89PNG\r\n\x1a\n"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/api/form":
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            self.send_json({"data": list(FORMS.values()) + [{"id": "frm_global", "sessionID": "global", "fields": []}]})
        elif path == "/api/session":
            archived = SESSION.get("time", {}).get("archived") is not None
            requested_archived = parse_qs(urlsplit(self.path).query).get("archived", ["false"]) == ["true"]
            self.send_json({"data": [SESSION] if archived == requested_archived else [], "cursor": {"previous": None, "next": None}})
        elif path == "/api/session/active":
            self.send_json({"data": {"ses_fake": {"type": "running"}} if os.path.exists(os.path.join(PROJECT, "busy")) else {}})
        elif path == "/api/shell":
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            with open(os.path.join(PROJECT, "background-shells.json")) as shells:
                self.send_json(json.load(shells))
        elif path.startswith("/api/shell/"):
            query = parse_qs(urlsplit(self.path).query)
            assert query["location[directory]"] == [PROJECT]
            shell_id = path.split("/")[3]
            with open(os.path.join(PROJECT, "background-shells.json")) as shells:
                shell = next((s for s in json.load(shells)["data"] if s.get("id") == shell_id), None)
            if shell is None:
                self.send_error(404)
            elif path.endswith("/output"):
                with open(os.path.join(PROJECT, "shell-reads.jsonl"), "a") as reads:
                    reads.write(json.dumps(query) + "\n")
                data = ("x" * 16000 + "latest café output").encode()
                cursor = int(query.get("cursor", ["0"])[0])
                limit = int(query["limit"][0])
                chunk = data[cursor:cursor+limit]
                self.send_json({"data": {"output": chunk.decode(), "size": len(data), "cursor": cursor+len(chunk), "truncated": cursor+len(chunk)<len(data)}})
            else:
                self.send_json({"data": shell})
        elif path == "/api/session/ses_fake":
            session = dict(SESSION)
            self.send_json({"data": session})
        elif path == "/api/session/ses_foreign":
            self.send_json({"data": {"id": "ses_foreign", "location": {"directory": os.path.dirname(PROJECT)}}})
        elif path == "/api/vcs":
            assert parse_qs(urlsplit(self.path).query)["location[directory]"] == [PROJECT]
            if os.path.exists(os.path.join(PROJECT, "no-vcs")):
                self.send_error(404)
                return
            self.send_json({"data": {"branch": {"current": "feature/current"}}})
        elif path == "/api/session/ses_fake/diff":
            assert parse_qs(urlsplit(self.path).query) == {"context": ["0"]}
            if os.path.exists(os.path.join(PROJECT, "no-vcs")):
                self.send_error(404)
                return
            self.send_json({"data": [{"additions": 12, "deletions": 2}, {"additions": 3, "deletions": 5}]})
        elif path == "/api/session/ses_fake/inbox":
            self.send_json({"data": list(INBOX.values())})
        elif path == "/api/session/ses_fake/message":
            query = parse_qs(urlsplit(self.path).query)
            if "cursor" in query:
                assert query["cursor"] == ["older+opaque/=="] and "order" not in query
                self.send_json({"data": [{"id": "msg_old", "type": "user", "text": "Oldest"}], "cursor": {"previous": None, "next": None}})
                return
            assert query["order"] == ["desc"] and query["limit"] == ["50"]
            self.send_json({"data": [
                {"id": "msg_assistant", "type": "assistant", "content": [{"type": "text", "text": "Earlier answer"}, {"id": "tool_1", "type": "tool", "name": "bash", "state": {"status": "completed", "input": {"command": "cargo test"}, "content": [{"type": "text", "text": "All passed"}]}}]},
                {"id": "msg_user", "type": "user", "text": "Earlier prompt"}
            ], "cursor": {"previous": None, "next": "older+opaque/=="}})
        elif path == "/api/model":
            self.send_json({"data": [
                {"id": "model/a", "providerID": "local", "name": "Model A", "enabled": True, "limit": {"context": 200000}, "variants": [{"id": "low"}, {"id": "high"}]},
                {"id": "model/a", "providerID": "router", "canonical": "local", "name": "Model A", "enabled": True, "limit": {"context": 0}},
                {"id": "model/a", "providerID": "custom", "name": "Model A", "enabled": True},
                {"id": "disabled", "providerID": "local", "name": "Hidden", "enabled": False}
            ]})
        elif path == "/api/provider":
            assert parse_qs(urlsplit(self.path).query)["location[directory]"] == [PROJECT]
            if os.path.exists(os.path.join(PROJECT, "no-providers")):
                self.send_error(503)
                return
            self.send_json({"data": [{"id": "local", "name": "Local gateway"}, {"id": "router", "name": "OpenRouter"}]})
        elif path == "/api/model/default":
            self.send_json({"data": {"providerID": "local", "id": "model/a"}})
        elif path == "/api/agent":
            self.send_json({"data": [{"id": "build", "name": "Build", "mode": "primary", "hidden": False}, {"id": "plan", "name": "Plan", "mode": "primary", "hidden": False}, {"id": "child", "name": "Child", "mode": "subagent", "hidden": False}, {"id": "hidden", "name": "Hidden", "mode": "primary", "hidden": True}]})
        elif path == "/api/project":
            assert parse_qs(urlsplit(self.path).query) == {"archived": ["true"]}
            self.send_json([{"id": "project_fake", "canonical": PROJECT, "name": "Fake"}])
        elif path == "/api/experimental/session/stats":
            with open(os.path.join(PROJECT, "stats-query.json"), "w") as query:
                json.dump(parse_qs(urlsplit(self.path).query), query)
            with open(os.path.join(PROJECT, "stats.json")) as stats:
                self.send_json({"data": json.load(stats)})
        elif path in ["/api/command", "/api/experimental/usage/limits"]:
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            if os.path.exists(os.path.join(PROJECT, "command-api-error")):
                self.send_error(503)
                return
            if path == "/api/command":
                data = [{"name": "goal", "description": "Set or control an objective"}, {"name": "team/review"}]
            else:
                usage = os.path.join(PROJECT, "usage.json")
                with open(usage) as source:
                    data = json.load(source)
            self.send_json({"location": {"directory": PROJECT}, "data": data})
        else:
            self.send_error(404)

    def do_PATCH(self):
        if not self.authorized(): return
        assert urlsplit(self.path).path == "/api/session/ses_fake"
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        assert set(body) == {"archived"} and isinstance(body["archived"], bool)
        if os.path.exists(os.path.join(PROJECT, "archive-error")):
            self.send_error(500)
            return
        if body["archived"]:
            SESSION.setdefault("time", {})["archived"] = 456
        else:
            SESSION.setdefault("time", {}).pop("archived", None)
        self.send_response(204)
        self.end_headers()

    def do_DELETE(self):
        if not self.authorized(): return
        if urlsplit(self.path).path.startswith("/api/shell/"):
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            path = os.path.join(PROJECT, "background-shells.json")
            with open(path) as shells: data = json.load(shells)
            shell_id = urlsplit(self.path).path.rsplit("/", 1)[1]
            data["data"] = [s for s in data["data"] if s.get("id") != shell_id]
            with open(path, "w") as shells: json.dump(data, shells)
            self.send_response(204)
            self.end_headers()
            return
        if self.path.startswith("/api/session/ses_fake/inbox/"):
            INBOX.pop(self.path.rsplit("/", 1)[1], None)
            self.send_response(204)
            self.end_headers()
            return
        if urlsplit(self.path).path == "/api/session/ses_fake":
            if os.path.exists(os.path.join(PROJECT, "delete-error")):
                self.send_error(500)
                return
            SESSION["deleted"] = True
            open(os.path.join(PROJECT, "deleted"), "w").close()
            self.send_response(204)
            self.end_headers()
            return
        assert self.path == "/api/session/ses_fake/form/frm_live"
        FORMS.pop("frm_live")
        self.send_response(204)
        self.end_headers()

    def do_POST(self):
        if not self.authorized():
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        path = urlsplit(self.path).path
        with open(os.path.join(PROJECT, "requests.jsonl"), "a") as requests:
            requests.write(json.dumps({"path": path, "body": body}) + "\n")
        if path == "/api/session/ses_fake/form/frm_saved/reply":
            assert body == {"answer": {"q0": ["macOS", "custom target"]}}
            if os.path.exists(os.path.join(PROJECT, "fail-form-reply")):
                self.send_error(503)
                return
            FORMS.pop("frm_saved")
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session":
            if body.get("model"):
                assert body["model"] == {"providerID": "local", "id": "model/a"}
                assert body["agent"] == "build"
                SESSION["model"] = body["model"]
            self.send_json({"data": SESSION})
        elif path == "/api/session/ses_fake/model":
            if body["model"]["id"] == "missing" or body["model"].get("variant") == "invalid":
                self.send_error(400)
                return
            SESSION["model"] = body["model"]
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/agent":
            assert set(body) == {"agent"}
            if body["agent"] not in ["build", "plan"]:
                self.send_error(400)
                return
            SESSION["agent"] = body["agent"]
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/prompt":
            with open(os.path.join(PROJECT, "prompt-request.json"), "w") as request:
                json.dump(body, request)
            if body.get("delivery") == "queue":
                INBOX.setdefault(body["id"], {"id": body["id"], "type": "user", "delivery": "queue", "payload": {"text": body["text"]}})
            self.send_json({"data": {"id": "msg_input", "type": "user", "text": body["text"]}})
        elif path == "/api/session/ses_fake/generate":
            assert set(body) == {"prompt"}
            if os.path.exists(os.path.join(PROJECT, "generate-error")):
                self.send_error(503)
                return
            if os.path.exists(os.path.join(PROJECT, "generate-malformed")):
                self.send_json({"data": {}})
                return
            self.send_json({"data": {"text": "Transient answer: " + body["prompt"]}})
        elif path == "/api/session/ses_fake/command":
            assert set(body) == {"name", "text", "files"}
            if body["name"] not in ["goal", "team/review"]:
                self.send_error(404)
                return
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/interrupt":
            self.send_json({"interrupted": True})
        elif path == "/api/session/ses_fake/permission/per_fake/reply":
            with open(os.path.join(PROJECT, "approval.json"), "w") as approval:
                json.dump(body, approval)
            self.send_json({})
        else:
            self.send_error(404)

server = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[-1])), Handler)
if not os.environ.get("OPENCODE_SERVER_PASSWORD"):
    print("server password " + PASSWORD, flush=True)
server.serve_forever()
