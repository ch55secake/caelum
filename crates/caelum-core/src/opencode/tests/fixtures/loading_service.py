import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

release = threading.Event()
lock = threading.Lock()
forms = {}
streams = 0


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def reply(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        global streams
        url = urlsplit(self.path)
        query = parse_qs(url.query)
        if url.path == '/api/info':
            self.reply({'pid': os.getpid(), 'version': '2.2.1'})
        elif url.path == '/api/event':
            with lock:
                streams += 1
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.end_headers()
            self.wfile.write(b'data: {"type":"server.connected","data":{}}\n\n')
            self.wfile.flush()
            try:
                while True:
                    time.sleep(0.05)
                    self.wfile.write(b': keepalive\n\n')
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
        elif url.path == '/api/form':
            name = os.path.basename(query['location[directory]'][0])
            with lock:
                forms[name] = attempt = forms.get(name, 0) + 1
            if name == 'slow' and attempt == 1:
                print('blocked', flush=True)
                release.wait(10)
            data = [] if name != 'slow' else [{
                'id': 'frm_attempt_' + str(attempt), 'sessionID': 'ses_slow',
                'title': 'Choose', 'fields': [{'key': 'q', 'type': 'string'}],
            }]
            self.reply({'data': data})
        elif url.path == '/api/session/active':
            self.reply({'data': {}})
        elif url.path.endswith('/message'):
            self.reply({'data': [{'id': 'msg_old', 'type': 'user', 'text': 'Saved'}],
                        'cursor': {'next': 'opaque'}})
        elif url.path.startswith('/api/session/ses_'):
            name = url.path.rsplit('ses_', 1)[1]
            self.reply({'data': {'id': 'ses_' + name, 'title': 'Saved',
                                'location': {'directory': os.path.join(os.getcwd(), name)}}})
        elif url.path == '/test/release':
            release.set()
            self.reply({})
        elif url.path == '/test/counts':
            with lock:
                self.reply({'streams': streams, 'forms': forms})
        else:
            self.send_error(404)


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
print('http://127.0.0.1:' + str(server.server_port), flush=True)
server.serve_forever()
