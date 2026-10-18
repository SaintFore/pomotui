#!/usr/bin/env python3
"""Render delivery evidence and update a single release-PR status comment."""
import argparse
import json
import os
from pathlib import Path
import re
import urllib.request

MARKER = '<!-- pomotui-linux-delivery -->'


def load(path):
    return json.loads(Path(path).read_text()) if path and Path(path).is_file() else {}


def cell(value):
    return str(value or '').replace('|', '\\|').replace('\n', ' ')


def render(args):
    candidate = load(Path(args.candidate)/'candidate.json') if args.candidate else {}
    journal = load(args.journal)
    selection = load(args.selection)
    required = candidate.get('required_channels', ['github-linux','aur-stable','aur-git']) if args.intent == 'stable' else ['aur-git']
    channels = journal.get('channels', {})
    complete = bool(required) and all(channels.get(channel,{}).get('stage') in ('discoverable','superseded-discoverable') for channel in required)
    # Never promote an incomplete/failed journal based on unrelated platform jobs.
    complete = complete and journal.get('aggregate') == 'complete'
    state = 'complete' if complete else journal.get('aggregate','blocked-before-publication')
    if state == 'complete': state = 'incomplete-evidence' if not complete else state
    lines = [MARKER, '## Linux delivery', '', f'Delivery: **{state}**', '',
             f"Recorded source: `{candidate.get('source_commit',selection.get('source_commit',args.source or 'not selected'))}`", 
             f"Stable version: `{candidate.get('version',selection.get('version','not selected'))}`", '',
             '| Required channel | Stage | Verification / diagnosis |', '| --- | --- | --- |']
    for channel in required:
        result = channels.get(channel,{})
        history = [event.get('stage','') for event in journal.get('events',[]) if event.get('channel') == channel]
        details = [('Observed stages: '+' → '.join(dict.fromkeys(history))) if history else '',result.get('diagnosis',''),result.get('remote_commit',''),result.get('url',''),result.get('retry','')]
        if result.get('actual_source_commit'):
            details.append('Verified current descendant '+result['actual_source_commit']+' / '+result.get('actual_version','')+'; original recorded source remains above')
        lines.append('| '+cell(channel)+' | '+cell(result.get('stage','not reached'))+' | '+cell('; '.join(str(x) for x in details if x))+' |')
    lines += ['', 'Candidate SHA-256: `'+str(journal.get('candidate_sha256','not available'))+'`']
    if journal.get('diagnosis'): lines += ['', '**Diagnosis:** '+cell(journal['diagnosis'])]
    if args.jobs:
        jobs = load(args.jobs)
        lines += ['', 'Workflow stages: '+', '.join(name+'='+value.get('result','unknown') for name,value in jobs.items())+'.']
    if not journal:
        lines += ['', 'Publication was not reached. Inspect the failed selection, source-check, build, acceptance, restore or credential step in the linked run; no completion is inferred.']
    lines += ['', 'macOS results are independent and never satisfy a required Linux channel.', '',
              '[Run, logs and retained diagnostics]('+args.run_url+')', '',
              'Retry failed jobs in this run before publication, or restore the same accepted publication:', '', '```sh']
    match = re.search(r'/runs/(\d+)',args.run_url)
    run = args.original_run or (match.group(1) if match else 'ORIGINAL_RUN_ID')
    lines += [f'gh workflow run resume-publication.yml --field run_id={run} --field intent={args.intent} --field verify_only={str(args.verify_only).lower()}', '```', '',
              'Use `verify_only=true` for stable pending discovery. Recovery requires the original accepted artifacts (90-day retention); expired artifacts pause rather than rebuild published bytes.', '']
    return '\n'.join(lines), selection


def update_comment(repository, number, body, api_url):
    token = os.environ.get('GH_TOKEN') or os.environ.get('GITHUB_TOKEN')
    if not token: raise ValueError('GH_TOKEN is required to update the release PR report')
    def request(path, method='GET', value=None):
        data = None if value is None else json.dumps(value).encode()
        request = urllib.request.Request(api_url.rstrip('/')+path, data=data, method=method,
            headers={'Authorization':'Bearer '+token,'Accept':'application/vnd.github+json','Content-Type':'application/json'})
        with urllib.request.urlopen(request, timeout=30) as response: return json.load(response)
    page = 1
    while True:
        comments = request(f'/repos/{repository}/issues/{number}/comments?per_page=100&page={page}')
        for comment in comments:
            if comment.get('user',{}).get('type') == 'Bot' and MARKER in comment.get('body',''):
                request(f'/repos/{repository}/issues/comments/{comment["id"]}', 'PATCH', {'body':body})
                return
        if len(comments)<100: break
        page += 1
    request(f'/repos/{repository}/issues/{number}/comments','POST',{'body':body})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['candidate','journal','selection','jobs','source','original-run']:
        parser.add_argument('--'+name)
    parser.add_argument('--intent',choices=['stable','git'],required=True)
    parser.add_argument('--run-url',required=True)
    parser.add_argument('--output',required=True)
    parser.add_argument('--verify-only',action='store_true')
    parser.add_argument('--comment',action='store_true')
    parser.add_argument('--pr',type=int)
    parser.add_argument('--repository',default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
    parser.add_argument('--api-url',default='https://api.github.com')
    args = parser.parse_args()
    body, selection = render(args)
    Path(args.output).parent.mkdir(parents=True,exist_ok=True)
    Path(args.output).write_text(body)
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'],'a') as output: output.write(body)
    if args.comment:
        number = args.pr or selection.get('pull_request') or selection.get('pr')
        if isinstance(number,dict): number = number.get('number')
        if not number: raise ValueError('original release PR number is unavailable; inspect selection diagnostics')
        update_comment(args.repository,int(number),body,args.api_url)

if __name__ == '__main__': main()
