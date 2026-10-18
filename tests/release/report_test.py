import json
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class DeliveryReport(unittest.TestCase):
    def test_pending_and_unproven_completion_are_visible_at_the_public_report_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            candidate = root/'candidate'; candidate.mkdir()
            (candidate/'candidate.json').write_text(json.dumps({'source_commit':'a'*40,'version':'0.4.0','required_channels':['github-linux','aur-stable','aur-git']}))
            journal = root/'publication.json'
            value = {'source_commit':'a'*40,'version':'0.4.0','candidate_sha256':hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest(),'aggregate':'complete','channels':{'github-linux':{'stage':'discoverable'},'aur-stable':{'stage':'pending-indexing','diagnosis':'RPC still exposes old version'},'aur-git':{'stage':'superseded-discoverable','actual_source_commit':'b'*40,'actual_version':'r999.gbbbbbbbbbbbb-1'}}}
            journal.write_text(json.dumps(value))
            command = ['python3',str(ROOT/'packaging/release/report.py'),'--candidate',str(candidate),'--journal',str(journal),'--intent','stable','--run-url','https://github.com/test/project/actions/runs/123','--output',str(root/'report.md')]
            result = subprocess.run(command,capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stderr)
            report = (root/'report.md').read_text()
            self.assertIn('pending-indexing',report)
            self.assertIn('RPC still exposes old version',report)
            self.assertIn('r999.gbbbbbbbbbbbb-1',report)
            self.assertNotIn('Delivery: **complete**',report)
            self.assertIn('--field run_id=123',report)
            value['channels']['aur-stable']['stage']='discoverable'; journal.write_text(json.dumps(value))
            subprocess.run(command,check=True)
            self.assertIn('Delivery: **complete**',(root/'report.md').read_text())

    def test_a_wrong_archive_compatibility_receipt_blocks_the_normal_publisher(self):
        from publication_test import prepared_candidate
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            (candidate/'ubuntu-archive.json').write_text(json.dumps({'passed':True,'source_commit':manifest['source_commit'],'candidate_sha256':'wrong','archive_sha256':manifest['artifacts']['pomotui-linux-x86_64.tar.gz']}))
            result=subprocess.run(['python3',str(ROOT/'packaging/release/release.py'),'verify-gates','--candidate',str(candidate),'--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json')],text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('Ubuntu',result.stderr)

    def test_stable_report_updates_one_existing_bot_comment(self):
        from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
        import os,threading
        writes=[]
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200);self.end_headers();self.wfile.write(json.dumps([{'id':7,'user':{'type':'Bot'},'body':'<!-- pomotui-linux-delivery -->\nold pending report'}]).encode())
            def do_PATCH(self):
                writes.append((self.path,json.loads(self.rfile.read(int(self.headers['Content-Length'])))))
                self.send_response(200);self.end_headers();self.wfile.write(b'{}')
            def log_message(self,*args):pass
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                result=subprocess.run(['python3',str(ROOT/'packaging/release/report.py'),'--intent','stable','--pr','42','--comment','--api-url',f'http://127.0.0.1:{server.server_port}','--run-url','https://github.com/test/project/actions/runs/123','--output',tmp+'/report.md'],env={**os.environ,'GH_TOKEN':'isolated-fixture-token'},text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(len(writes),1)
                self.assertTrue(writes[0][0].endswith('/issues/comments/7'))
                self.assertIn('blocked-before-publication',writes[0][1]['body'])
        finally:server.shutdown();thread.join();server.server_close()

    def test_an_unbound_journal_or_missing_candidate_cannot_report_complete(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate=root/'candidate';candidate.mkdir()
            (candidate/'candidate.json').write_text(json.dumps({'source_commit':'a'*40,'version':'0.4.0','required_channels':['aur-git']}))
            journal=root/'journal.json';value={'aggregate':'complete','source_commit':'b'*40,'candidate_sha256':'wrong','channels':{'aur-git':{'stage':'discoverable'}}};journal.write_text(json.dumps(value))
            command=['python3',str(ROOT/'packaging/release/report.py'),'--intent','git','--candidate',str(candidate),'--journal',str(journal),'--run-url','https://github.com/test/project/actions/runs/123','--output',str(root/'report.md')]
            subprocess.run(command,check=True)
            self.assertNotIn('Delivery: **complete**',(root/'report.md').read_text())
            self.assertIn('does not identify this candidate',(root/'report.md').read_text())
            (candidate/'candidate.json').unlink();subprocess.run(command,check=True)
            self.assertNotIn('Delivery: **complete**',(root/'report.md').read_text())

    def test_late_original_report_cannot_overwrite_a_newer_recovery_report(self):
        from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
        import os,threading
        writes=[]
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                state={'report_run':999,'attempt':1,'source_commit':'a'*40,'publication_run':123}
                body='<!-- pomotui-linux-delivery -->\n<!-- pomotui-linux-delivery-state '+json.dumps(state)+' -->\nDelivery: **complete**'
                self.send_response(200);self.end_headers();self.wfile.write(json.dumps([{'id':7,'user':{'type':'Bot'},'body':body}]).encode())
            def do_PATCH(self):
                writes.append(self.path);self.send_response(200);self.end_headers();self.wfile.write(b'{}')
            def log_message(self,*args):pass
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                result=subprocess.run(['python3',str(ROOT/'packaging/release/report.py'),'--intent','stable','--pr','42','--comment','--api-url',f'http://127.0.0.1:{server.server_port}','--run-url','https://github.com/test/project/actions/runs/123','--output',tmp+'/report.md'],env={**os.environ,'GH_TOKEN':'isolated-fixture-token'},text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(writes,[])
        finally:server.shutdown();thread.join();server.server_close()

    def test_an_unreadable_journal_still_produces_a_failure_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);journal=root/'journal.json';journal.write_text('{interrupted')
            result=subprocess.run(['python3',str(ROOT/'packaging/release/report.py'),'--intent','stable','--journal',str(journal),'--run-url','https://github.com/test/project/actions/runs/123','--output',str(root/'report.md')],text=True,capture_output=True)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertIn('Cannot read retained evidence',(root/'report.md').read_text())
            self.assertNotIn('Delivery: **complete**',(root/'report.md').read_text())
