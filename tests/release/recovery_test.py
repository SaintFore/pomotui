"""Recovery consumes the normal CLI and disposable authoritative Git/HTTP state."""
import json
from pathlib import Path
import subprocess
import unittest

import candidate_test
from publication_fixture import built_candidate, acceptance
from recovery_fixture import trusted_run

ROOT=Path(__file__).resolve().parents[2]


class RecoveryTest(candidate_test.CandidateTest):
    def setUp(self):
        super().setUp()
        self.plan()
        self.manifest=built_candidate(self.output)
        (self.output/'acceptance.json').write_text(json.dumps(acceptance(self.output)))
        (self.output/'checks.json').write_text(json.dumps({'passed':True,'source_commit':self.manifest['source_commit']}))
        self.git('branch','-M','main')
        self.git('remote','add','origin',str(self.repo))
        self.restored=self.output.parent/'restored'

    def restore(self,api):
        return subprocess.run(['python3',str(ROOT/'packaging/release/restore.py'),
            '--run-id','123','--intent','git','--source',self.manifest['source_commit'],
            '--candidate',str(self.restored),'--selection',str(self.output.parent/'selection'),
            '--repo',str(self.repo),'--api-url',api,'--output',str(self.output.parent/'restoration.json')],
            text=True,capture_output=True)

    def test_whole_run_recovery_retains_original_outputs_after_main_advances(self):
        (self.repo/'unrelated').write_text('Main advanced after acceptance')
        self.git('add','.');self.git('commit','-qm','Later source')
        with trusted_run(self.output,self.manifest['source_commit']) as (api,state):
            state['attempts']=[3,1]
            result=self.restore(api)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertIn('/archive',state['calls'])
            self.assertNotIn('/archive/3',state['calls'])
            for name in ['candidate.json','acceptance.json','checks.json',*self.manifest['artifacts']]:
                self.assertEqual((self.restored/name).read_bytes(),(self.output/name).read_bytes())
            provenance=json.loads((self.restored/'origin.json').read_text())
            self.assertEqual((provenance['run_id'],provenance['attempt'],provenance['artifact_id']),(123,1,456))
            self.assertEqual(provenance['source_commit'],self.manifest['source_commit'])

    def test_forged_workflow_failed_gate_and_expired_original_are_never_rebuilt(self):
        with trusted_run(self.output,self.manifest['source_commit']) as (api,state):
            state['path']='.github/workflows/untrusted-pr.yml'
            result=self.restore(api)
            self.assertNotEqual(result.returncode,0)
            self.assertNotIn('/archive',state['calls'])
            state['path']='.github/workflows/git-package.yml';state['conclusion']='failure'
            result=self.restore(api)
            self.assertNotEqual(result.returncode,0)
            self.assertNotIn('/archive',state['calls'])
            state['conclusion']='success';state['expired']=True
            result=self.restore(api)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('expired',result.stderr)
            self.assertFalse(self.restored.exists())

    def test_wrong_event_unrelated_main_and_modified_gate_receipts_are_rejected(self):
        with trusted_run(self.output,self.manifest['source_commit']) as (api,state):
            state['event']='pull_request'
            self.assertNotEqual(self.restore(api).returncode,0)
            state['event']='push';state['head_branch']='untrusted'
            self.assertNotEqual(self.restore(api).returncode,0)
            state['head_branch']='main'
            checks=self.output/'checks.json';original=checks.read_bytes()
            checks.write_text(json.dumps({'passed':False,'source_commit':self.manifest['source_commit']}))
            self.assertNotEqual(self.restore(api).returncode,0)
            checks.write_bytes(original)
            receipt=self.output/'acceptance.json';original_receipt=receipt.read_bytes()
            altered=json.loads(original_receipt);altered['candidate_sha256']='0'*64
            receipt.write_text(json.dumps(altered))
            self.assertNotEqual(self.restore(api).returncode,0)
            receipt.write_bytes(original_receipt)
            manifest=self.output/'candidate.json';original_manifest=manifest.read_bytes()
            value=json.loads(original_manifest);value['source_commit']='f'*40
            manifest.write_text(json.dumps(value))
            self.assertNotEqual(self.restore(api).returncode,0)
            manifest.write_bytes(original_manifest)
            empty=self.git('hash-object','-t','tree','/dev/null')
            unrelated=self.git('commit-tree',empty,'-m','Unrelated main')
            self.git('update-ref','refs/heads/main',unrelated)
            rejected=self.restore(api)
            self.assertNotEqual(rejected.returncode,0)
            self.assertIn('trusted main',rejected.stderr)


for _name in list(candidate_test.CandidateTest.__dict__):
    if _name.startswith('test_'):
        setattr(RecoveryTest,_name,None)
