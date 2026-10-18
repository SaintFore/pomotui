#!/usr/bin/env python3
"""Publication policy exercised through the public coordinator CLI."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import contextlib
from pathlib import Path
import subprocess
import tempfile
import hashlib
import shutil
import urllib.parse
from publication_fixture import built_candidate, acceptance, pin_source_fixture
import unittest

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / 'packaging/release/release.py'

@contextlib.contextmanager
def github_fixture(routes):
    calls = []
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            calls.append(('GET', self.path))
            status, body = routes.get(self.path, (404, {'message':'Not Found'}))
            self.send_response(status); self.end_headers()
            self.wfile.write(json.dumps(body).encode())
        def log_message(self, *args): pass
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    try: yield f'http://127.0.0.1:{server.server_port}', calls
    finally: server.shutdown(); thread.join(); server.server_close()


class ReleasePolicy(unittest.TestCase):
    def test_an_ordinary_merge_cannot_select_a_stable_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            event = Path(tmp) / 'event.json'
            event.write_text(json.dumps({'action': 'closed', 'pull_request': {'merged': True, 'title': 'Fix startup', 'base': {'ref': 'main'}}}))
            result = subprocess.run(['python3', str(CLI), 'select', '--event', str(event), '--repo', str(ROOT), '--output', str(Path(tmp) / 'selection.json')], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads((Path(tmp) / 'selection.json').read_text())['intent'], 'git')

    def test_explicit_release_requires_approval_at_the_reviewed_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); repo=root/'repo'; repo.mkdir()
            def git(*args):
                return subprocess.check_output(['git','-C',str(repo),*args], text=True).strip()
            git('init','-q'); git('config','user.name','Fixture'); git('config','user.email','fixture@example.invalid')
            (repo/'Cargo.toml').write_text('[workspace.package]\nversion="0.1.0"\n')
            git('add','.');git('commit','-qm','Previous version');base=git('rev-parse','HEAD')
            (repo/'Cargo.toml').write_text('[workspace.package]\nversion="0.2.0"\n')
            (repo/'RELEASE_NOTES.md').write_text('New release notes\n')
            git('add','.');git('commit','-qm','Release');source=git('rev-parse','HEAD')
            pr={'number':42,'merged':True,'state':'closed','title':'release: v0.2.0','merge_commit_sha':source,'head':{'sha':source},'base':{'ref':'main','sha':base},'user':{'login':'author'}}
            event=root/'event.json';event.write_text(json.dumps({'action':'closed','number':42,'pull_request':pr}))
            routes={'/repos/test/project/pulls/42':(200,pr),'/repos/test/project/pulls/42/reviews?per_page=100&page=1':(200,[]),'/repos/test/project/pulls/42/commits?per_page=100&page=1':(200,[{'sha':source}])}
            with github_fixture(routes) as (url,calls):
                command=['python3',str(CLI),'select','--event',str(event),'--repo',str(repo),'--repository','test/project','--api-url',url,'--output',str(root/'selected.json')]
                result=subprocess.run(command,text=True,capture_output=True)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('approval',result.stderr)
                routes['/repos/test/project/pulls/42/reviews?per_page=100&page=1']=(200,[{'user':{'login':'maintainer'},'state':'APPROVED','commit_id':source}])
                result=subprocess.run(command,text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                selected=json.loads((root/'selected.json').read_text())
                self.assertEqual((selected['intent'],selected['source_commit'],selected['version']),('stable',source,'0.2.0'))
                # The live main/base identity can advance after the PR closes.
                pr['base']['sha']=source
                result=subprocess.run(command,text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                routes['/repos/test/project/pulls/42/reviews?per_page=100&page=1']=(200,[{'user':{'login':'maintainer'},'state':'APPROVED','commit_id':base}])
                result=subprocess.run(command,text=True,capture_output=True)
                self.assertNotEqual(result.returncode,0,'stale approval must not authorize changed release source')
                # A solo owner explicitly authorizes a release by merging its
                # versioned release PR; a second human is not required.
                pr['merged_by']={'login':'author'}
                routes['/repos/test/project/collaborators/author/permission']=(200,{'permission':'admin'})
                result=subprocess.run(command,text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(json.loads((root/'selected.json').read_text())['approval'],'maintainer-merge')

class PreviousStable(unittest.TestCase):
    def test_unpublished_newer_tag_is_not_an_upgrade_predecessor(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);repo=root/'source';repo.mkdir()
            def git(*args): return subprocess.check_output(['git','-C',str(repo),*args],text=True).strip()
            git('init','-q');git('config','user.name','Fixture');git('config','user.email','fixture@example.invalid')
            (repo/'source').write_text('Released source');git('add','.');git('commit','-qm','Released stable')
            stable=git('rev-parse','HEAD');git('tag','v0.3.0')
            (repo/'source').write_text('Unpublished candidate');git('commit','-qam','Unpublished tag')
            git('tag','v0.4.0')
            candidate=root/'candidate';candidate.mkdir();(candidate/'candidate.json').write_text(json.dumps({'version':'0.5.0','tag_intent':'v0.5.0'}))
            routes={'/repos/test/project/releases?per_page=100&page=1':(200,[{'id':1,'tag_name':'v0.3.0','draft':False,'prerelease':False,'html_url':'https://example.invalid/v0.3.0'},{'id':2,'tag_name':'v0.4.0','draft':True,'prerelease':False}]),'/repos/test/project/git/ref/tags/v0.3.0':(200,{'object':{'type':'commit','sha':stable}})}
            with github_fixture(routes) as (url,calls):
                result=subprocess.run(['python3',str(CLI),'previous','--candidate',str(candidate),'--repo',str(repo),'--repository','test/project','--api-url',url,'--output',str(root/'previous.json')],text=True,capture_output=True)
                self.assertEqual(result.returncode,0,result.stderr)
                previous=json.loads((root/'previous.json').read_text())
                self.assertEqual((previous['tag'],previous['source_commit']),('v0.3.0',stable))
                self.assertNotIn(('GET','/repos/test/project/git/ref/tags/v0.4.0'),calls)


def prepared_candidate(root):
    repo=root/'source';repo.mkdir()
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
        shutil.copyfile(ROOT/name,repo/name)
    pin_source_fixture(repo)
    shutil.copytree(ROOT/'packaging',repo/'packaging')
    for crate in (ROOT/'crates').iterdir():
        dest=repo/'crates'/crate.name;dest.mkdir(parents=True)
        shutil.copyfile(crate/'Cargo.toml',dest/'Cargo.toml')
    subprocess.run(['git','init','-q',str(repo)],check=True)
    subprocess.run(['git','-C',str(repo),'add','.'],check=True)
    subprocess.run(['git','-C',str(repo),'-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','Source fixture'],check=True)
    notes=root/'notes';notes.write_text('Chosen release notes\n')
    candidate=root/'candidate'
    subprocess.run(['python3',str(ROOT/'packaging/release/candidate.py'),'plan','--repo',str(repo),'--intent','stable','--notes',str(notes),'--output',str(candidate)],check=True,capture_output=True)
    manifest=built_candidate(candidate)
    (candidate/'acceptance.json').write_text(json.dumps(acceptance(candidate)))
    (candidate/'checks.json').write_text(json.dumps({'passed':True,'source_commit':manifest['source_commit']}))
    (root/'selection.json').write_text(json.dumps({'intent':'stable','source_commit':manifest['source_commit'],'version':manifest['version'],'tag':manifest['tag_intent']}))
    sha=hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest()
    (root/'preflight.json').write_text(json.dumps({'candidate_sha256':sha,'channels':[{'channel':c,'authorized':True,'stage':'preflight-passed','candidate_sha256':sha} for c in ['aur-stable','aur-git']]}))
    return candidate,manifest


@contextlib.contextmanager
def publication_remote(source, notes='Chosen release notes\n', conflicting_tag=False, corrupt_download=False, wrong_created_tag=False, tag='v0.3.0'):
    state={'lost_response':False,'failures':{},'tag':('f'*40 if conflicting_tag else None),'release':None,'assets':{},'writes':[],'notes':notes}
    class Handler(BaseHTTPRequestHandler):
        def reply(self,code,value,raw=False):
            self.send_response(code);self.end_headers();self.wfile.write(value if raw else json.dumps(value).encode())
        def do_GET(self):
            path=urllib.parse.urlsplit(self.path).path
            if state['failures'].get(path,0):
                state['failures'][path]-=1
                return self.reply(503,{'message':'temporary failure'})
            if path.endswith('/git/ref/tags/'+tag):
                return self.reply(200,{'object':{'type':'commit','sha':state['tag']}}) if state['tag'] else self.reply(404,{})
            if path.endswith('/releases/tags/'+tag):
                return self.reply(200,state['release']) if state['release'] else self.reply(404,{})
            if path.endswith('/releases/1/assets'): return self.reply(200,list(state['assets'].values()))
            if path.startswith('/asset/') or path.startswith('/download/'):
                name=path.split('/')[-1];data=state['assets'][name]['data']
                return self.reply(200,b'wrong bytes' if corrupt_download else bytes.fromhex(data),True)
            return self.reply(404,{})
        def do_POST(self):
            body=self.rfile.read(int(self.headers.get('Content-Length',0)));state['writes'].append(self.path)
            if self.path.endswith('/git/refs'):
                state['tag']='e'*40 if wrong_created_tag else json.loads(body)['sha']
                if state['lost_response']:
                    state['lost_response']=False
                    return self.reply(503,{'message':'response lost after write'})
                return self.reply(201,{'ref':'refs/tags/v0.3.0'})
            if self.path.endswith('/releases'):
                value=json.loads(body);value.update(id=1,upload_url=base+'/upload{?name,label}',html_url=base+'/release')
                state['release']=value;return self.reply(201,value)
            if self.path.startswith('/upload?'):
                name=urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)['name'][0]
                value={'name':name,'url':base+'/asset/'+name,'browser_download_url':base+'/download/'+name,'data':body.hex()}
                state['assets'][name]=value;return self.reply(201,value)
            return self.reply(404,{})
        def do_PATCH(self):
            state['writes'].append(self.path);state['release'].update(json.loads(self.rfile.read(int(self.headers['Content-Length']))));self.reply(200,state['release'])
        def log_message(self,*args): pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler);base=f'http://127.0.0.1:{server.server_port}'
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try: yield base,state
    finally: server.shutdown();thread.join();server.server_close()


class ReleaseDelivery(unittest.TestCase):
    def publish(self,root,candidate,url):
        return subprocess.run(['python3',str(CLI),'publish-github','--candidate',str(candidate),'--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json'),'--selection',str(root/'selection.json'),'--preflight',str(root/'preflight.json'),'--repository','test/project','--api-url',url,'--output',str(root/'result.json')],text=True,capture_output=True)

    def test_gate_and_credential_failures_never_write_a_public_channel(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            with publication_remote(manifest['source_commit']) as (url,state):
                for kind in ['checks','acceptance','credentials']:
                    path=candidate/(kind+'.json') if kind!='credentials' else root/'preflight.json'
                    original=path.read_bytes();value=json.loads(original)
                    if kind=='credentials': value['channels']=value['channels'][:1]
                    else: value['passed']=False
                    path.write_text(json.dumps(value))
                    result=self.publish(root,candidate,url)
                    self.assertNotEqual(result.returncode,0,result.stdout)
                    self.assertEqual(state['writes'],[])
                    path.write_bytes(original)
                result=self.publish(root,candidate,url)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(state['tag'],manifest['source_commit'])
                self.assertFalse(state['release']['draft'])
                self.assertEqual(json.loads((root/'result.json').read_text())['aggregate'],'pending-required-aur-channels')
                writes=list(state['writes'])
                result=self.publish(root,candidate,url)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(state['writes'],writes,'same-publication rerun must verify matching outputs without writes')
                state['release']['body']='Different public release notes'
                result=self.publish(root,candidate,url)
                self.assertNotEqual(result.returncode,0,'same version must not silently reuse conflicting public notes')
                self.assertEqual(state['writes'],writes)

    def test_manifest_transfer_keeps_hidden_metadata_notes_and_bundle_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            (candidate/'unrelated-build-cache').write_bytes(b'Not part of the recorded output')
            result=subprocess.run(['python3',str(CLI),'pack','--candidate',str(candidate),'--output',str(root/'portable')],text=True,capture_output=True)
            self.assertEqual(result.returncode,0,result.stderr)
            for name in ['candidate.json','source.bundle','release-notes.md','aur-stable/.SRCINFO','aur-git/.SRCINFO','acceptance.json','checks.json']:
                self.assertEqual((candidate/name).read_bytes(),(root/'portable'/name).read_bytes())
            self.assertFalse((root/'portable/unrelated-build-cache').exists())

    def test_real_preflight_checks_both_aur_targets_before_github_delivery(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','-q',str(root/(package+'.git'))],check=True)
            command=['python3',str(CLI),'preflight','--candidate',str(candidate),'--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json'),'--aur-remote',str(root/'{package}.git'),'--aur-rpc-url','http://127.0.0.1:1/rpc','--output',str(root/'preflight.json')]
            result=subprocess.run(command,text=True,capture_output=True)
            self.assertEqual(result.returncode,0,result.stderr)
            report=json.loads((root/'preflight.json').read_text())
            self.assertEqual({c['package'] for c in report['channels']},{'pomotui','pomotui-git'})
            with publication_remote(manifest['source_commit']) as (url,state):
                result=self.publish(root,candidate,url)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertFalse(state['release']['draft'])
                # The preflight performs no actual AUR ref update.
                for package in ['pomotui','pomotui-git']:
                    refs=subprocess.check_output(['git','--git-dir',str(root/(package+'.git')),'show-ref'],text=True) if (root/(package+'.git')/'refs/heads/master').exists() else ''
                    self.assertEqual(refs,'')

    def test_conflicting_tag_is_preserved_without_any_release_write(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            with publication_remote(manifest['source_commit'],conflicting_tag=True) as (url,state):
                result=self.publish(root,candidate,url)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('never repoint',result.stderr)
                self.assertEqual(state['writes'],[])
                self.assertEqual(state['tag'],'f'*40)

    def test_created_tag_is_read_back_before_release_publication(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            with publication_remote(manifest['source_commit'],wrong_created_tag=True) as (url,state):
                result=self.publish(root,candidate,url)
                self.assertNotEqual(result.returncode,0,'write success is not authoritative tag verification')
                self.assertIsNone(state['release'])
                self.assertEqual(state['tag'],'e'*40)

    def test_upload_success_with_wrong_remote_bytes_is_not_delivery(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            with publication_remote(manifest['source_commit'],corrupt_download=True) as (url,state):
                result=self.publish(root,candidate,url)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('asset conflict',result.stderr)
                self.assertTrue(state['release']['draft'])
                self.assertFalse((root/'result.json').exists())

@contextlib.contextmanager
def aur_index(versions):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            package=urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)['arg[]'][0]
            self.send_response(200);self.end_headers()
            self.wfile.write(json.dumps({'results':[{'Name':package,'Version':versions[package]}]}).encode())
        def log_message(self,*args): pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try: yield f'http://127.0.0.1:{server.server_port}/rpc/v5/info'
    finally: server.shutdown();thread.join();server.server_close()


class StableAurDelivery(unittest.TestCase):
    def command(self,action,root,candidate,url,rpc):
        return subprocess.run(['python3',str(CLI),action,'--candidate',str(candidate),
            '--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json'),
            '--selection',str(root/'selection.json'),'--repository','test/project','--api-url',url,
            '--aur-remote',str(root/'{package}.git'),'--aur-rpc-url',rpc,
            '--discovery-attempts','1','--output',str(root/'stable.json')],text=True,capture_output=True)

    def test_stable_publication_delivers_both_required_aur_channels_and_can_verify_without_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
            versions={'pomotui':'old-1','pomotui-git':manifest['git_version']+'-1'}
            with publication_remote(manifest['source_commit']) as (url,state), aur_index(versions) as rpc:
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertNotEqual(result.returncode,0,'pending indexing must not report overall success')
                report=json.loads((root/'stable.json').read_text())
                self.assertEqual(report['aggregate'],'pending-indexing')
                self.assertEqual(report['channels']['aur-stable']['stage'],'pending-indexing')
                self.assertEqual(report['channels']['aur-git']['stage'],'discoverable')
                self.assertEqual([e['stage'] for e in report['events'] if e['channel']=='aur-stable'],['pushed','remote-verified','pending-indexing'])
                heads={package:subprocess.check_output(['git','--git-dir',str(root/(package+'.git')),'rev-parse','master']) for package in versions}
                for channel,package in [('aur-stable','pomotui'),('aur-git','pomotui-git')]:
                    for name in ['PKGBUILD','.SRCINFO','pomotui.install']:
                        actual=subprocess.check_output(['git','--git-dir',str(root/(package+'.git')),'show','master:'+name])
                        self.assertEqual(actual,(candidate/channel/name).read_bytes())
                prior_events=list(report['events'])
                versions['pomotui']=manifest['version']+'-1';writes=list(state['writes'])
                result=self.command('verify-stable',root,candidate,url,rpc)
                self.assertEqual(result.returncode,0,result.stderr)
                resumed=json.loads((root/'stable.json').read_text())
                self.assertEqual(resumed['aggregate'],'complete')
                self.assertEqual(resumed['events'][:len(prior_events)],prior_events)
                self.assertEqual(len(resumed['attempts']),2)
                self.assertEqual(state['writes'],writes)
                for package,head in heads.items():
                    self.assertEqual(subprocess.check_output(['git','--git-dir',str(root/(package+'.git')),'rev-parse','master']),head)

    def test_transient_retry_exhaustion_retains_original_candidate_and_can_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
            versions={'pomotui':manifest['version']+'-1','pomotui-git':manifest['git_version']+'-1'}
            original=(candidate/'candidate.json').read_bytes()
            with publication_remote(manifest['source_commit']) as (url,state), aur_index(versions) as rpc:
                state['failures']['/repos/test/project/git/ref/tags/v0.3.0']=3
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertNotEqual(result.returncode,0)
                journal=json.loads((root/'stable.json').read_text())
                self.assertEqual(journal['aggregate'],'retry-exhausted')
                self.assertEqual(len(journal['attempts']),3)
                self.assertEqual(state['writes'],[])
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual((candidate/'candidate.json').read_bytes(),original)
                self.assertEqual(len(json.loads((root/'stable.json').read_text())['attempts']),4)
                writes=list(state['writes'])
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(state['writes'],writes)

    def test_uncertain_write_and_duplicate_attempts_reconcile_without_reposting(self):
        from concurrent.futures import ThreadPoolExecutor
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
            versions={'pomotui':manifest['version']+'-1','pomotui-git':manifest['git_version']+'-1'}
            with publication_remote(manifest['source_commit']) as (url,state), aur_index(versions) as rpc:
                state['lost_response']=True
                with ThreadPoolExecutor(max_workers=2) as pool:
                    futures=[pool.submit(self.command,'publish-stable',root,candidate,url,rpc) for _ in range(2)]
                    results=[future.result() for future in futures]
                self.assertTrue(all(result.returncode==0 for result in results),[result.stderr for result in results])
                self.assertEqual(state['writes'].count('/repos/test/project/git/refs'),1)
                self.assertEqual(state['writes'].count('/repos/test/project/releases'),1)
                journal=json.loads((root/'stable.json').read_text())
                self.assertEqual(journal['aggregate'],'complete')
                self.assertEqual(len(journal['attempts']),3)
                self.assertEqual(journal['attempts'][0]['stage'],'failed')

    def test_remote_content_mismatch_is_paused_independently_of_index_discovery(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
            versions={'pomotui':manifest['version']+'-1','pomotui-git':manifest['git_version']+'-1'}
            with publication_remote(manifest['source_commit']) as (url,state), aur_index(versions) as rpc:
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertEqual(result.returncode,0,result.stderr)
                clone=root/'conflict'
                subprocess.run(['git','clone','-q',str(root/'pomotui.git'),str(clone)],check=True)
                (clone/'pomotui.install').write_text('Conflicting remote hook')
                subprocess.run(['git','-C',str(clone),'add','.'],check=True)
                subprocess.run(['git','-C',str(clone),'-c','user.name=Other','-c','user.email=other@example.invalid','commit','-qm','Conflicting remote'],check=True)
                subprocess.run(['git','-C',str(clone),'push','-q','origin','HEAD:master'],check=True)
                head=subprocess.check_output(['git','--git-dir',str(root/'pomotui.git'),'rev-parse','master'])
                writes=list(state['writes'])
                result=self.command('verify-stable',root,candidate,url,rpc)
                self.assertNotEqual(result.returncode,0)
                report=json.loads((root/'stable.json').read_text())
                self.assertEqual(report['aggregate'],'paused')
                self.assertIn('pomotui/pomotui.install',report['diagnosis'])
                self.assertEqual(state['writes'],writes)
                self.assertEqual(subprocess.check_output(['git','--git-dir',str(root/'pomotui.git'),'rev-parse','master']),head)

    def test_unusable_github_source_blocks_both_aur_deliveries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            for package in ['pomotui','pomotui-git']:
                subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
            with publication_remote(manifest['source_commit'],corrupt_download=True) as (url,state), aur_index({}) as rpc:
                result=self.command('publish-stable',root,candidate,url,rpc)
                self.assertNotEqual(result.returncode,0)
                for package in ['pomotui','pomotui-git']:
                    head=subprocess.run(['git','--git-dir',str(root/(package+'.git')),'rev-parse','--verify','master'],capture_output=True)
                    self.assertNotEqual(head.returncode,0)
                report=json.loads((root/'stable.json').read_text())
                self.assertEqual(report['aggregate'],'paused')
                self.assertFalse(any(event['channel'].startswith('aur-') for event in report['events']))

if __name__ == '__main__':
    unittest.main()
