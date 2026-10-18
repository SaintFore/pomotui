#!/usr/bin/env python3
"""Restore immutable gate outputs from a trusted publication run, never rebuild."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile

from release import GitHub, verify_gates
from recovery import TransientError, atomic

PATHS = {'stable': '.github/workflows/stable-release.yml', 'git': '.github/workflows/git-package.yml'}


def extract(api, artifact, output):
    if artifact.get('expired'):
        raise ValueError('original publication artifacts expired; cannot safely reconstruct publication bytes')
    data = api.request(artifact['archive_download_url'], raw=True, accept='application/vnd.github+json')
    output.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        for info in archive.infolist():
            name = Path(info.filename)
            if name.is_absolute() or '..' in name.parts or (info.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError('unsafe publication artifact path')
        archive.extractall(output)


def artifacts_for(api, run_id):
    result=[]
    for page in range(1,101):
        batch=api.repo(f'actions/runs/{run_id}/artifacts?per_page=100&page={page}')['artifacts']
        result.extend(batch)
        if len(batch)<100:
            return result
    raise ValueError('artifact pagination limit exceeded; refuse incomplete recovery identity')


def restore(args):
    api = GitHub(args.repository, args.api_url)
    run = api.repo(f'actions/runs/{args.run_id}')
    if run.get('repository', {}).get('full_name') != args.repository or run.get('path', '').split('@')[0] != PATHS[args.intent]:
        raise ValueError('recovery requires the original trusted publication workflow in this repository')
    expected_event = 'pull_request_target' if args.intent == 'stable' else 'push'
    if run.get('event') != expected_event or (args.intent == 'git' and run.get('head_branch') != 'main'):
        raise ValueError('untrusted publication event/ref; recovery cannot consume PR workflow artifacts')
    artifacts = artifacts_for(api,args.run_id)
    matches = []
    for artifact in artifacts:
        match = re.fullmatch(rf'verified-candidate-{args.run_id}-(\d+)-{args.intent}', artifact['name'])
        if match:
            matches.append((int(match[1]), artifact))
    if not matches:
        if args.optional and run.get('run_attempt', 1) == 1:
            return {'restored': False}
        # A failed pre-gate attempt has never been allowed publication credentials.
        jobs = api.repo(f'actions/runs/{args.run_id}/jobs?filter=all&per_page=100')['jobs']
        if args.optional and not any(job['name'] == 'publish' and job.get('started_at') for job in jobs):
            return {'restored': False}
        raise ValueError('original verified candidate unavailable; never regenerate a possibly published version')
    accepted=[]
    for attempt, artifact in sorted(matches,key=lambda item:item[0]):
        jobs=api.repo(f'actions/runs/{args.run_id}/attempts/{attempt}/jobs?per_page=100')['jobs']
        if any(job['name'] in ('gate / gate','gate') and job.get('conclusion')=='success' for job in jobs):
            accepted.append((attempt,artifact))
            break
    if not accepted:
        if args.optional:
            jobs=api.repo(f'actions/runs/{args.run_id}/jobs?filter=all&per_page=100')['jobs']
            if not any(job['name']=='publish' and job.get('started_at') for job in jobs):
                return {'restored':False}
        raise ValueError('original required gate job did not succeed; refuse candidate receipts without trusted CI evidence')
    attempt,artifact=accepted[0]
    extract(api, artifact, args.candidate)
    receipt = argparse.Namespace(candidate=args.candidate, acceptance=args.candidate/'acceptance.json', checks=args.candidate/'checks.json')
    manifest = verify_gates(receipt)
    source = manifest['source_commit']
    if args.source and args.source != source:
        raise ValueError('restored candidate differs from requested immutable source')
    # Main history is authoritative, never a PR artifact claiming main identity.
    subprocess.run(['git','-C',str(args.repo),'fetch','--quiet','origin','main'], check=True)
    if subprocess.run(['git','-C',str(args.repo),'merge-base','--is-ancestor',source,'FETCH_HEAD'], capture_output=True).returncode:
        raise ValueError('restored candidate is not in trusted main history')
    if args.intent == 'stable':
        selections=[]
        for item in artifacts:
            match=re.fullmatch(rf'stable-selection-{args.run_id}-(\d+)',item['name'])
            if match and int(match[1])<=attempt:
                selections.append((int(match[1]),item))
        selected=min(selections,key=lambda item:item[0])[1] if selections else None
        if selected is None:
            raise ValueError('original approved release selection missing')
        extract(api, selected, args.selection)
        selection = json.loads((args.selection/'selection.json').read_text())
        if selection.get('intent') != 'stable' or selection.get('source_commit') != source or selection.get('version') != manifest['version'] or selection.get('tag') != manifest['tag_intent']:
            raise ValueError('original release approval/selection differs from retained candidate')
    origin = {'run_id': args.run_id, 'attempt': attempt, 'artifact_id': artifact['id'],
              'repository': args.repository, 'source_commit': source,
              'candidate_sha256': hashlib.sha256((args.candidate/'candidate.json').read_bytes()).hexdigest()}
    atomic(args.candidate/'origin.json', origin)
    if args.journal:
        prefix = 'stable-publication' if args.intent == 'stable' else 'git-publication'
        prior = []
        for item in artifacts:
            found = re.fullmatch(rf'{prefix}-{args.run_id}-(\d+)', item['name'])
            if found:
                prior.append((int(found[1]),item))
        import tempfile
        for _, prior_artifact in sorted(prior,key=lambda item:item[0],reverse=True):
            with tempfile.TemporaryDirectory(prefix='pomotui-journal-') as tmp:
                extract(api,prior_artifact,Path(tmp))
                filename = 'stable-publication.json' if args.intent == 'stable' else 'aur-git.json'
                path = Path(tmp)/filename
                if path.is_file():
                    value=json.loads(path.read_text())
                    if value.get('candidate_sha256') != origin['candidate_sha256']:
                        raise ValueError('retained journal candidate identity differs from original gate')
                    atomic(args.journal,value)
                    break
    if args.journal and args.journal_run_id:
        resume=api.repo(f'actions/runs/{args.journal_run_id}')
        if resume.get('repository',{}).get('full_name')!=args.repository or resume.get('path','').split('@')[0]!='.github/workflows/resume-publication.yml' or resume.get('event')!='workflow_dispatch' or resume.get('head_branch')!='main':
            raise ValueError('journal continuation must come from trusted main recovery workflow')
        previous=artifacts_for(api,args.journal_run_id)
        matches=[]
        for item in previous:
            match=re.fullmatch(rf'publication-resume-{args.journal_run_id}-(\d+)',item['name'])
            if match: matches.append((int(match[1]),item))
        import tempfile
        for _, prior_artifact in sorted(matches,key=lambda item:item[0],reverse=True):
            with tempfile.TemporaryDirectory(prefix='pomotui-resume-journal-') as tmp:
                extract(api,prior_artifact,Path(tmp))
                if not (Path(tmp)/'restore.json').is_file() or not (Path(tmp)/'publication.json').is_file():
                    continue
                retained=json.loads((Path(tmp)/'restore.json').read_text())
                value=json.loads((Path(tmp)/'publication.json').read_text())
                if retained.get('origin',{}).get('run_id')!=args.run_id or value.get('candidate_sha256')!=origin['candidate_sha256']:
                    raise ValueError('recovery journal belongs to another original publication')
                atomic(args.journal,value)
                break
    return dict(restored=True, source_commit=source, origin=origin)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-id', type=int, required=True)
    parser.add_argument('--intent', choices=PATHS, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--selection', type=Path, required=True)
    parser.add_argument('--source')
    parser.add_argument('--repo',type=Path,default=Path('.'))
    parser.add_argument('--journal',type=Path)
    parser.add_argument('--journal-run-id',type=int)
    parser.add_argument('--optional', action='store_true')
    parser.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
    parser.add_argument('--api-url', default='https://api.github.com')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = restore(args)
    atomic(args.output, result)
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            print('restored='+str(result['restored']).lower(), file=output)
            print('source='+result.get('source_commit', ''), file=output)


if __name__ == '__main__':
    try:
        main()
    except TransientError as error:
        raise SystemExit('Transient: '+str(error))
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        raise SystemExit('Paused: '+str(error))
