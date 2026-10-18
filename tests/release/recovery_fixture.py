"""Local GitHub artifact transport used by the normal recovery CLI tests."""
import contextlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import threading
import zipfile


def zipped(directory):
    output=io.BytesIO()
    with zipfile.ZipFile(output,'w') as archive:
        for path in directory.rglob('*'):
            if path.is_file():
                archive.write(path,str(path.relative_to(directory)))
    return output.getvalue()


@contextlib.contextmanager
def trusted_run(directory, source, intent='git'):
    state={'path':f'.github/workflows/{"git-package" if intent=="git" else "stable-release"}.yml',
           'conclusion':'success','expired':False,'calls':[],'attempts':[1],
           'event':'push' if intent=='git' else 'pull_request_target','head_branch':'main'}
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            state['calls'].append(self.path)
            if self.path.endswith('/actions/runs/123'):
                value={'repository':{'full_name':'SaintFore/pomotui'},'path':state['path'],
                    'event':state['event'],'head_branch':state['head_branch'],'run_attempt':max(state['attempts'])}
            elif '/artifacts?' in self.path:
                value={'artifacts':[{'id':456,'name':f'verified-candidate-123-{attempt}-{intent}',
                    'expired':state['expired'],'archive_download_url':base+('/archive' if attempt==1 else f'/archive/{attempt}')} for attempt in state['attempts']]}
            elif '/attempts/' in self.path and '/jobs?' in self.path:
                value={'jobs':[{'name':'gate / gate','conclusion':state['conclusion']}]}
            elif self.path.startswith('/archive'):
                self.send_response(200);self.end_headers();self.wfile.write(zipped(directory));return
            else:
                self.send_response(404);self.end_headers();return
            self.send_response(200);self.end_headers();self.wfile.write(json.dumps(value).encode())
        def log_message(self,*args): pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler);base=f'http://127.0.0.1:{server.server_port}'
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try: yield base,state
    finally: server.shutdown();thread.join();server.server_close()
