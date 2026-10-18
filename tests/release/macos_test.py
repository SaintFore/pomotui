"""Optional macOS delivery through the normal CLI and isolated HTTP remote."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import publication_test
from publication_test import CLI, prepared_candidate, publication_remote


class MacDelivery(unittest.TestCase):
    def test_optional_delivery_reuses_bytes_and_refuses_conflicts_without_changing_linux(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            mac=root/'mac';mac.mkdir()
            name='pomotui-macos-aarch64.tar.gz'
            (mac/name).write_bytes(b'compiled macOS archive fixture')
            checksum=hashlib.sha256((mac/name).read_bytes()).hexdigest()
            (mac/'pomotui-macos-aarch64.sha256').write_text(checksum+'  '+name+'\n')
            receipt={'source_commit':manifest['source_commit'],'target':'aarch64-apple-darwin','artifacts':{name:checksum,'pomotui-macos-aarch64.sha256':hashlib.sha256((mac/'pomotui-macos-aarch64.sha256').read_bytes()).hexdigest()}}
            (mac/'build.json').write_text(json.dumps(receipt))
            (root/'publication.json').write_text(json.dumps({'source_commit':manifest['source_commit'],'candidate_sha256':hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest(),'channels':{'github-linux':{'stage':'discoverable'}}}))
            with publication_remote(manifest['source_commit']) as (url,state):
                command=['python3',str(CLI),'publish-macos','--publication',str(root/'publication.json'),'--candidate',str(candidate),'--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json'),'--macos',str(mac),'--repository','test/project','--api-url',url,'--retry-delay','0','--output',str(root/'mac.json')]
                def run(): return subprocess.run(command,text=True,capture_output=True)
                self.assertNotEqual(run().returncode,0)
                self.assertEqual(state['writes'],[],'optional output cannot create a release or tag')
                result=publication_test.ReleaseDelivery().publish(root,candidate,url)
                self.assertEqual(result.returncode,0,result.stderr)
                linux=(root/'result.json').read_bytes()
                state['failures']['/repos/test/project/releases/tags/v0.3.0']=1
                result=run();self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(bytes.fromhex(state['assets'][name]['data']),(mac/name).read_bytes())
                writes=list(state['writes'])
                self.assertEqual(run().returncode,0)
                self.assertEqual(state['writes'],writes)
                state['assets'][name]['data']=b'conflicting published bytes'.hex()
                result=run();self.assertNotEqual(result.returncode,0)
                self.assertIn('asset conflict',result.stderr)
                self.assertEqual(state['writes'],writes)
                self.assertEqual((root/'result.json').read_bytes(),linux)
                self.assertFalse(state['release']['draft'])
                # Wrong-source receipts and nonportable checksum paths fail before writes.
                state['assets'][name]['data']=(mac/name).read_bytes().hex()
                for field,value in [('source_commit','f'*40),('target','unknown')]:
                    wrong=dict(receipt);wrong[field]=value
                    (mac/'build.json').write_text(json.dumps(wrong))
                    self.assertNotEqual(run().returncode,0)
                (mac/'build.json').write_text(json.dumps(receipt))
                (mac/'pomotui-macos-aarch64.sha256').write_text(checksum+'  /runner/temp/'+name+'\n')
                receipt['artifacts']['pomotui-macos-aarch64.sha256']=hashlib.sha256((mac/'pomotui-macos-aarch64.sha256').read_bytes()).hexdigest()
                (mac/'build.json').write_text(json.dumps(receipt))
                result=run();self.assertNotEqual(result.returncode,0)
                self.assertIn('basename',result.stderr)
                self.assertEqual(state['writes'],writes)

    def test_second_architecture_requires_healthy_gates_and_immutable_linux_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);candidate,manifest=prepared_candidate(root)
            mac=root/'mac';mac.mkdir()
            name='pomotui-macos-x86_64.tar.gz';checksum_name='pomotui-macos-x86_64.sha256'
            (mac/name).write_bytes(b'intel macOS archive fixture')
            (mac/checksum_name).write_text(hashlib.sha256((mac/name).read_bytes()).hexdigest()+'  '+name+'\n')
            receipt={'source_commit':manifest['source_commit'],'target':'x86_64-apple-darwin',
                'artifacts':{n:hashlib.sha256((mac/n).read_bytes()).hexdigest() for n in [name,checksum_name]}}
            (mac/'build.json').write_text(json.dumps(receipt))
            (root/'publication.json').write_text(json.dumps({'source_commit':manifest['source_commit'],'candidate_sha256':hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest(),'channels':{'github-linux':{'stage':'discoverable'}}}))
            with publication_remote(manifest['source_commit']) as (url,state):
                result=publication_test.ReleaseDelivery().publish(root,candidate,url)
                self.assertEqual(result.returncode,0,result.stderr)
                writes=list(state['writes'])
                command=['python3',str(CLI),'publish-macos','--publication',str(root/'publication.json'),'--candidate',str(candidate),'--acceptance',str(candidate/'acceptance.json'),'--checks',str(candidate/'checks.json'),'--macos',str(mac),'--repository','test/project','--api-url',url,'--output',str(root/'mac.json')]
                def run(): return subprocess.run(command,text=True,capture_output=True)
                publication=(root/'publication.json').read_bytes()
                (root/'publication.json').write_text('{}')
                self.assertNotEqual(run().returncode,0)
                (root/'publication.json').write_bytes(publication)
                checks=(candidate/'checks.json').read_bytes()
                (candidate/'checks.json').write_text('{}')
                self.assertNotEqual(run().returncode,0)
                (candidate/'checks.json').write_bytes(checks)
                state['tag']='f'*40
                self.assertNotEqual(run().returncode,0)
                self.assertEqual(state['writes'],writes)
                state['tag']=manifest['source_commit']
                result=run();self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(bytes.fromhex(state['assets'][checksum_name]['data']),(mac/checksum_name).read_bytes())


if __name__=='__main__': unittest.main()
