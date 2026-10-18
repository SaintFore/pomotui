#!/usr/bin/env python3
"""Normal release entry points, real retained candidate, disposable public targets.

Build/package/systemd proof must precede this script. This script never rebuilds,
changes candidate identity, creates production refs or calls production write APIs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from publication_test import publication_remote, github_fixture, aur_index
from recovery_fixture import trusted_run

ROOT=Path(__file__).resolve().parents[2]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('candidate',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args();candidate=args.candidate.resolve();output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
    m=json.loads((candidate/'candidate.json').read_text());source=m['source_commit']
    assert m['version']=='0.0.0', 'Only the isolated nonpublishing stable fixture is accepted'
    identity=hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest()
    def command(label,script,*arguments,success=True):
        result=subprocess.run(['python3',str(ROOT/'packaging/release'/script),*map(str,arguments)],capture_output=True,text=True)
        (output/(label+'.log')).write_text(result.stdout+result.stderr)
        if (result.returncode==0)!=success: raise AssertionError(label+': '+result.stdout+result.stderr)
        return result
    command('gates','release.py','verify-gates','--candidate',candidate,'--acceptance',candidate/'acceptance.json','--checks',candidate/'checks.json')
    with tempfile.TemporaryDirectory(prefix='pomotui-delivery-proof-') as tmp:
        root=Path(tmp);repo=root/'source'
        subprocess.run(['git','clone','--quiet',str(candidate/'source.bundle'),str(repo)],check=True)
        subprocess.run(['git','-C',str(repo),'checkout','-q','-B','main',source],check=True)
        subprocess.run(['git','-C',str(repo),'remote','set-url','origin',str(repo)],check=True)
        pr={'number':42,'merged':True,'state':'closed','title':'release: v0.0.0','merge_commit_sha':source,'head':{'sha':source},'base':{'ref':'main'},'user':{'login':'fixture'}}
        event=root/'event.json';event.write_text(json.dumps({'action':'closed','number':42,'pull_request':pr}))
        selection=output/'selection.json'
        routes={'/repos/test/project/pulls/42':(200,pr),'/repos/test/project/pulls/42/reviews?per_page=100&page=1':(200,[{'user':{'login':'fixture-maintainer'},'state':'APPROVED','commit_id':source}]),'/repos/test/project/pulls/42/commits?per_page=100&page=1':(200,[{'sha':source}])}
        with github_fixture(routes) as (api,calls):
            command('approved-merge','release.py','select','--event',event,'--repo',repo,'--repository','test/project','--api-url',api,'--output',selection)
        notes=(candidate/'release-notes.md').read_text()
        versions={'pomotui':'old-1','pomotui-git':m['git_version']+'-1'}
        for package in versions:
            subprocess.run(['git','init','--bare','--initial-branch=master','-q',str(root/(package+'.git'))],check=True)
        with publication_remote(source,notes=notes,tag='v0.0.0') as (api,state), aur_index(versions) as rpc:
            def stable(action,label,success=True,remote=None,current=candidate,selected=selection,journal=output/'stable-publication.json'):
                return command(label,'release.py',action,'--candidate',current,'--acceptance',current/'acceptance.json','--checks',current/'checks.json','--selection',selected,'--repository','test/project','--api-url',api,'--aur-remote',remote or root/'{package}.git','--aur-rpc-url',rpc,'--discovery-attempts','1','--retry-delay','0','--output',journal,success=success)
            # Missing target authorization fails actual preflight before ANY GitHub write.
            stable('publish-stable','credential-preflight',False,root/'missing-{package}.git',journal=output/'preflight-failure.json')
            assert state['writes']==[]
            stable('publish-stable','pending-required-channel',False)
            pending=json.loads((output/'stable-publication.json').read_text());assert pending['aggregate']=='pending-indexing'
            heads={p:subprocess.check_output(['git','--git-dir',str(root/(p+'.git')),'rev-parse','master']) for p in versions}
            writes=list(state['writes'])
            # Restore SAME accepted candidate and prior journal via the NORMAL recovery CLI.
            with trusted_run(candidate,source,intent='stable',selection=selection) as (artifact_api,artifact_state):
                artifact_state['journals'][1]=output
                recovered=root/'recovered';restored_selection=root/'selection';journal=output/'resumed-publication.json'
                command('restore-original-publication','restore.py','--run-id','123','--intent','stable','--repo',repo,'--candidate',recovered,'--selection',restored_selection,'--journal',journal,'--repository','SaintFore/pomotui','--api-url',artifact_api,'--output',output/'restore.json')
                versions['pomotui']='0.0.0-1'
                stable('verify-stable','same-publication-discovery',current=recovered,selected=restored_selection/'selection.json',journal=journal)
                assert json.loads(journal.read_text())['aggregate']=='complete'
                assert (recovered/'candidate.json').read_bytes()==(candidate/'candidate.json').read_bytes()
            assert state['writes']==writes
            for package,head in heads.items(): assert subprocess.check_output(['git','--git-dir',str(root/(package+'.git')),'rev-parse','master'])==head
            # A separate main push selects Git upkeep and completes through its normal entry.
            push=root/'push.json';push.write_text(json.dumps({'ref':'refs/heads/main','before':subprocess.check_output(['git','-C',str(repo),'rev-parse',source+'^'],text=True).strip(),'after':source}))
            selected=command('automatic-git-selection','git_refresh.py','select','--event',push,'--repo',repo)
            assert json.loads(selected.stdout)['selected']
            command('automatic-git-publication','git_refresh.py','publish','--source',source,'--candidate',candidate,'--acceptance',candidate/'acceptance.json','--checks',candidate/'checks.json','--remote',root/'pomotui-git.git','--rpc-url',rpc,'--discovery-attempts','1','--output',output/'aur-git.json')
            assert json.loads((output/'aur-git.json').read_text())['aggregate']=='complete'
            jobs=output/'jobs.json';jobs.write_text(json.dumps({'gate':{'result':'success'},'publish':{'result':'success'},'macos':{'result':'failure'}}))
            command('stable-report','report.py','--intent','stable','--candidate',candidate,'--journal',output/'resumed-publication.json','--jobs',jobs,'--run-url','https://github.com/test/project/actions/runs/123','--output',output/'delivery-report.md')
            assert 'Delivery: **complete**' in (output/'delivery-report.md').read_text()
            assert 'macos=failure' in (output/'delivery-report.md').read_text()
            command('git-report','report.py','--intent','git','--candidate',candidate,'--journal',output/'aur-git.json','--run-url','https://github.com/test/project/actions/runs/123','--output',output/'git-report.md')
        (output/'proof.json').write_text(json.dumps({'passed':True,'source_commit':source,'candidate_sha256':identity,'production_writes':False,'approved_merge':True,'preflight_before_writes':True,'pending_required_channel':True,'same_publication_artifact_restore':True,'discovery_only_no_writes':True,'automatic_git':True,'macos_failure_independent':True},indent=2)+'\n')
    print('PASS complete Linux delivery through normal entry points with disposable targets')

if __name__=='__main__':main()
