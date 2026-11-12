
import base64, json, os, queue, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

events = queue.Queue()
subscribed = False
password = os.environ.get('TEST_PASSWORD')
session = {'id': 'ses_shared', 'title': 'External work', 'location': {'directory': os.getcwd()}, 'time': {'idle': 1}}
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args): pass
    def authorized(self):
        expected = 'Basic ' + base64.b64encode(('opencode:' + password).encode()).decode() if password else None
        if self.headers.get('Authorization') == expected: return True
        self.send_error(401)
        return False
    def reply(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        global subscribed
        if not self.authorized(): return
        path = urlsplit(self.path).path
        if path == '/api/info':
            self.reply({'pid': os.getpid(), 'version': '2.2.1'})
        elif path == '/api/event':
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.end_headers()
            # Headers alone are not a subscription acknowledgement.
            time.sleep(0.1)
            subscribed = True
            self.wfile.write(b'data: {"type":"server.connected","data":{}}\n\n')
            self.wfile.flush()
            try:
                while True:
                    try: frame = ('data: ' + json.dumps(events.get(timeout=0.1)) + '\n\n').encode()
                    except queue.Empty: frame = b': keepalive\n\n'
                    self.wfile.write(frame)
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError): pass
        elif path == '/api/session':
            assert subscribed
            self.reply({'data': [session]})
        elif path == '/api/session/active':
            if os.path.exists('activity-error'): self.send_error(503)
            elif os.path.exists('activity-invalid'): self.reply({'data': []})
            else: self.reply({'data': {} if os.path.exists('idle') else {'ses_shared': {'type': 'running'}}})
        elif path == '/api/session/ses_shared': self.reply({'data': session})
        elif path == '/api/session/ses_shared/message': self.reply({'data': [], 'cursor': {'next': None}})
        else: self.send_error(404)
    def do_POST(self):
        if not self.authorized(): return
        events.put({'type': 'session.text.delta', 'location': {'directory': '/other-project'}, 'data': {'sessionID': 'ses_shared', 'delta': 'Wrong project'}})
        events.put({'type': 'session.reasoning.delta', 'data': {'sessionID': 'ses_shared', 'assistantMessageID': 'msg_external', 'ordinal': 0, 'delta': 'External thinking'}})
        self.reply({})
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
print('http://127.0.0.1:' + str(server.server_port), flush=True)
server.serve_forever()
