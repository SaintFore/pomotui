#!/usr/bin/env python3
"""Coordinate an approved stable release without rewriting public identities."""
import argparse
import hashlib
import json
import os
import tomllib
import urllib.request
import urllib.error
import urllib.parse
from pathlib import Path
import re
import subprocess
from recovery import Journal, TransientError, atomic, locked


class DownloadRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        redirected=super().redirect_request(request,fp,code,message,headers,newurl)
        if redirected and urllib.parse.urlsplit(request.full_url).netloc!=urllib.parse.urlsplit(newurl).netloc:
            redirected.remove_header('Authorization')
        return redirected


class GitHub:
    def __init__(self, repository, api_url='https://api.github.com'):
        if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
            raise ValueError('invalid repository identity')
        self.repository = repository
        self.base = api_url.rstrip('/')

    def request(self, path, method='GET', payload=None, raw=False, missing=False):
        url = path if path.startswith(('https://', 'http://')) else self.base + path
        headers = {'Accept':'application/octet-stream' if raw else 'application/vnd.github+json', 'User-Agent':'pomotui-release'}
        authenticated_origin=urllib.parse.urlsplit(url).netloc==urllib.parse.urlsplit(self.base).netloc or (self.base=='https://api.github.com' and urllib.parse.urlsplit(url).netloc=='uploads.github.com')
        if os.environ.get('GH_TOKEN') and authenticated_origin:
            headers['Authorization'] = 'Bearer ' + os.environ['GH_TOKEN']
        if isinstance(payload, bytes):
            data=payload; headers['Content-Type']='application/octet-stream'
        else:
            data=json.dumps(payload).encode() if payload is not None else None
            if data is not None: headers['Content-Type']='application/json'
        try:
            with urllib.request.build_opener(DownloadRedirect()).open(urllib.request.Request(url, data=data, headers=headers, method=method), timeout=30) as response:
                body=response.read()
        except urllib.error.HTTPError as error:
            if missing and error.code == 404: return None
            if error.code in (408, 429, 500, 502, 503, 504):
                raise TransientError(f'GitHub temporary HTTP {error.code}: {method} {path}') from error
            raise ValueError(f'GitHub {method} {path}: HTTP {error.code}; check repository token permissions') from error
        except urllib.error.URLError as error:
            raise TransientError(f'GitHub transport interrupted: {method} {path}: {error.reason}') from error
        return body if raw else json.loads(body) if body else None

    def repo(self, path, **kwargs):
        return self.request(f'/repos/{self.repository}/{path}', **kwargs)

    def pages(self, path):
        result=[]
        for page in range(1,101):
            values=self.repo(f'{path}?per_page=100&page={page}')
            result.extend(values)
            if len(values)<100: return result
        raise ValueError('GitHub pagination limit exceeded; refuse incomplete authorization evidence')


def git(repo, *args):
    return subprocess.check_output(['git','-C',str(repo),*args], text=True).strip()


def select(args):
    event = json.loads(args.event.read_text())
    pr = event.get('pull_request', {})
    result = {'intent': 'git', 'reason': 'ordinary merge is not stable release intent'}
    if re.fullmatch(r'release: v[0-9]+\.[0-9]+\.[0-9]+', pr.get('title', '')):
        number=event.get('number',pr.get('number'))
        if not isinstance(number,int): raise ValueError('release event has no PR identity')
        api=GitHub(args.repository,args.api_url)
        pr=api.repo(f'pulls/{number}')
        if event.get('action') != 'closed' or not pr.get('merged') or pr.get('base',{}).get('ref') != 'main':
            raise ValueError('release intent must be a merged main release PR')
        source=git(args.repo,'rev-parse',pr['merge_commit_sha']+'^{commit}')
        head=pr['head']['sha']
        latest={}
        for review in api.pages(f'pulls/{number}/reviews'):
            if review['state'] not in ('COMMENTED','PENDING'):
                latest[review['user']['login']]=review
        approved=any(review['state']=='APPROVED' and review.get('commit_id')==head for review in latest.values())
        merger=pr.get('merged_by',{}).get('login')
        if not approved and merger:
            permission=api.repo('collaborators/'+urllib.parse.quote(merger,safe='')+'/permission')
            approved=permission.get('permission') in ('admin','maintain','write')
        if not approved:
            raise ValueError('release PR requires current-source approval or an authorized maintainer explicitly merging this release PR')
        approval='maintainer-merge' if merger and not any(review['state']=='APPROVED' and review.get('commit_id')==head for review in latest.values()) else 'review'
        commits=api.pages(f'pulls/{number}/commits')
        if not commits: raise ValueError('release PR has no recorded source commits')
        first=commits[0]['sha']
        if not re.fullmatch('[0-9a-f]{40}',first): raise ValueError('invalid PR source identity')
        available=subprocess.run(['git','-C',str(args.repo),'cat-file','-e',first+'^{commit}'],capture_output=True)
        if available.returncode:
            # Squash/rebase source commits may not belong to main's ancestry.
            # Fetch the immutable reviewed PR head, never a moving main base.
            subprocess.run(['git','-C',str(args.repo),'fetch','--no-tags','origin',f'refs/pull/{number}/head'],check=True)
            if git(args.repo,'rev-parse','FETCH_HEAD')!=head: raise ValueError('PR head moved after recorded merge')
        previous=git(args.repo,'rev-parse',first+'^')
        version=tomllib.loads(git(args.repo,'show',source+':Cargo.toml'))['workspace']['package']['version']
        if pr['title'] != 'release: v'+version:
            raise ValueError('release PR title must match the Cargo workspace version')
        old_version=tomllib.loads(git(args.repo,'show',previous+':Cargo.toml'))['workspace']['package']['version']
        if version==old_version: raise ValueError('release PR must select a new workspace version')
        changed=git(args.repo,'diff','--name-only',previous,source).splitlines()
        if 'RELEASE_NOTES.md' not in changed or not git(args.repo,'show',source+':RELEASE_NOTES.md').strip():
            raise ValueError('release PR must include nonempty RELEASE_NOTES.md')
        result={'intent':'stable','pull_request':number,'source_commit':source,'version':version,'tag':'v'+version,'notes':'RELEASE_NOTES.md','reviewed_head':head,'approval':approval,'previous_commit':previous}
    args.output.write_text(json.dumps(result, indent=2) + '\n')


def previous(args):
    manifest=json.loads((args.candidate/'candidate.json').read_text())
    api=GitHub(args.repository,args.api_url)
    releases=[]
    for release in api.pages('releases'):
        tag=release.get('tag_name','')
        if release.get('draft') or release.get('prerelease') or not re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+',tag): continue
        version=tuple(map(int,tag[1:].split('.')))
        if manifest.get('tag_intent') and not args.allow_nonpublishing_fixture_predecessor and version>=tuple(map(int,manifest['version'].split('.'))): continue
        releases.append((version,release))
    if not releases: raise ValueError('No published non-draft stable predecessor is available; an unannounced tag is not a released upgrade baseline')
    _,release=max(releases,key=lambda item:item[0]);tag=release['tag_name']
    obj=api.repo('git/ref/tags/'+tag)['object']
    for _ in range(10):
        if obj['type']=='commit': break
        if obj['type']!='tag': raise ValueError('published predecessor tag is not a source commit')
        obj=api.repo('git/tags/'+obj['sha'])['object']
    if obj.get('type')!='commit': raise ValueError('published predecessor tag chain is invalid')
    local=git(args.repo,'rev-parse',tag+'^{commit}')
    if local!=obj['sha']: raise ValueError('published stable predecessor tag/source conflict; fetch and inspect immutable release identity')
    args.output.write_text(json.dumps({'version':tag[1:],'tag':tag,'source_commit':local,'release_id':release['id'],'url':release.get('html_url')},indent=2)+'\n')
    print(tag)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_gates(args):
    subprocess.run(['python3',str(Path(__file__).with_name('candidate.py')),'verify','--output',str(args.candidate),'--require-built'],check=True,stdout=subprocess.DEVNULL)
    manifest=json.loads((args.candidate/'candidate.json').read_text())
    acceptance=json.loads(args.acceptance.read_text())
    checks=json.loads(args.checks.read_text())
    if checks.get('passed') is not True or checks.get('source_commit')!=manifest['source_commit']:
        raise ValueError('applicable source checks have not passed for this candidate')
    if acceptance.get('passed') is not True or acceptance.get('source_commit')!=manifest['source_commit'] or acceptance.get('candidate_sha256')!=digest(args.candidate/'candidate.json'):
        raise ValueError('real upgrade acceptance missing, failed, timed out, or identifies another candidate')
    if set(acceptance.get('packages',{})) != set(manifest['packages']):
        raise ValueError('real upgrade acceptance must cover every required package')
    for channel,name in manifest['packages'].items():
        receipt=acceptance['packages'][channel]
        if receipt.get('passed') is not True or receipt.get('sha256')!=manifest['artifacts'][name]:
            raise ValueError(f'real upgrade acceptance mismatch: {channel}')
    smoke=json.loads((args.candidate/'ubuntu-archive.json').read_text())
    if smoke.get('passed') is not True or smoke.get('platform')!='ubuntu-24.04' or smoke.get('source_commit')!=manifest['source_commit'] or smoke.get('candidate_sha256')!=digest(args.candidate/'candidate.json') or smoke.get('archive_sha256')!=manifest['artifacts']['pomotui-linux-x86_64.tar.gz'] or smoke.get('binary_sha256')!=manifest['binary_sha256']:
        raise ValueError('Ubuntu 24.04 distributed archive compatibility evidence missing or identifies another candidate')
    return manifest


def pack(args):
    subprocess.run(['python3',str(Path(__file__).with_name('candidate.py')),'verify','--output',str(args.candidate),'--require-built'],check=True)
    manifest=json.loads((args.candidate/'candidate.json').read_text())
    import shutil
    args.output.mkdir(parents=True,exist_ok=True)
    for name in ['candidate.json',*manifest['artifacts']]:
        dest=args.output/name;dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(args.candidate/name,dest)
    for name in ['acceptance.json','checks.json','origin.json','ubuntu-archive.json']:
        if (args.candidate/name).is_file(): shutil.copyfile(args.candidate/name,args.output/name)


def preflight(args):
    manifest=verify_gates(args)
    results=[]
    for channel in manifest['required_channels']:
        if not channel.startswith('aur-'): continue
        command=['python3',str(Path(__file__).with_name('aur.py')),'preflight','--candidate',str(args.candidate),'--channel',channel]
        if args.aur_remote: command.extend(['--remote',args.aur_remote.replace('{package}','pomotui' if channel=='aur-stable' else 'pomotui-git')])
        if args.aur_rpc_url: command.extend(['--rpc-url',args.aur_rpc_url])
        results.append(json.loads(subprocess.check_output(command,text=True)))
    report={'candidate_sha256':digest(args.candidate/'candidate.json'),'channels':results}
    args.output.write_text(json.dumps(report,indent=2)+'\n')


def publish(args):
    if args.api_url=='https://api.github.com' and not os.environ.get('GH_TOKEN'):
        raise ValueError('GitHub publication requires GH_TOKEN with repository contents write permission')
    manifest=verify_gates(args)
    selection=json.loads(args.selection.read_text())
    if selection.get('intent')!='stable' or any(selection.get(k)!=manifest.get(k) for k in ['source_commit','version']) or selection.get('tag')!=manifest['tag_intent']:
        raise ValueError('approved merged release PR does not identify this stable candidate')
    credentials=json.loads(args.preflight.read_text())
    expected=set(manifest['required_channels'])-{'github-linux'}
    actual={item.get('channel') for item in credentials.get('channels',[]) if item.get('authorized') is True and item.get('stage')=='preflight-passed' and item.get('candidate_sha256')==digest(args.candidate/'candidate.json')}
    if credentials.get('candidate_sha256')!=digest(args.candidate/'candidate.json') or actual!=expected:
        raise ValueError('required AUR credential/host/package authorization preflight is missing; repair secret, account permission or approved host trust')
    api=GitHub(args.repository,args.api_url)
    tag=manifest['tag_intent']; source=manifest['source_commit']
    ref=api.repo('git/ref/tags/'+tag,missing=True)
    if ref:
        obj=ref['object']
        for _ in range(10):
            if obj['type']=='commit': break
            if obj['type']!='tag': raise ValueError('existing tag has unsupported object identity')
            obj=api.repo('git/tags/'+obj['sha'])['object']
        if obj.get('type')!='commit' or obj['sha']!=source:
            raise ValueError('existing tag conflicts with recorded source; never repoint public tags')
    else:
        api.repo('git/refs',method='POST',payload={'ref':'refs/tags/'+tag,'sha':source})
        created=api.repo('git/ref/tags/'+tag)
        if created.get('object',{}).get('type')!='commit' or created['object'].get('sha')!=source:
            raise ValueError('created tag failed authoritative source verification; preserve remote state and inspect')
    observer=getattr(args,'observer',lambda value: None)
    observer({'channel':'github-linux','stage':'pushed','source_commit':source,'tag':tag,
              'candidate_sha256':digest(args.candidate/'candidate.json'),'detail':'immutable source tag recorded; release assets still require verification'})
    release=api.repo('releases/tags/'+tag,missing=True)
    if release is None:
        release=api.repo('releases',method='POST',payload={'tag_name':tag,'target_commitish':source,'name':tag,'body':(args.candidate/'release-notes.md').read_text(),'draft':True,'prerelease':False})
    if release.get('body')!=(args.candidate/'release-notes.md').read_text():
        raise ValueError('existing release notes conflict with recorded release PR; preserve public output')
    if release.get('prerelease') or release.get('tag_name')!=tag:
        raise ValueError('existing GitHub release identity conflicts with stable release')
    assets=[manifest['source_archive'],'pomotui-linux-x86_64.tar.gz','pomotui-linux-x86_64.sha256']
    remote_assets={asset['name']:asset for asset in api.pages('releases/'+str(release['id'])+'/assets')}
    for name in assets:
        expected_bytes=(args.candidate/name).read_bytes()
        if name not in remote_assets:
            upload=release['upload_url'].split('{')[0]+'?'+urllib.parse.urlencode({'name':name})
            remote_assets[name]=api.request(upload,method='POST',payload=expected_bytes)
        received=api.request(remote_assets[name]['url'],raw=True)
        if received!=expected_bytes:
            raise ValueError(f'immutable GitHub asset conflict: {name}; preserve existing output and inspect source/checksums')
    observer({'channel':'github-linux','stage':'remote-verified','source_commit':source,'tag':tag,
              'candidate_sha256':digest(args.candidate/'candidate.json'),'detail':'all required immutable asset bytes verified; draft is not discoverable'})
    if release.get('draft'):
        api.repo('releases/'+str(release['id']),method='PATCH',payload={'draft':False})
    visible=api.repo('releases/tags/'+tag)
    if visible.get('draft') or visible.get('prerelease') or visible.get('tag_name')!=tag:
        raise ValueError('GitHub release is not discoverable as the intended stable release')
    # User-facing asset URLs must serve the exact retained bytes, not merely
    # appear in an upload API response. Authentication remains on trusted API.
    for name in assets:
        url=remote_assets[name].get('browser_download_url')
        if not url: raise ValueError('GitHub asset has no user-visible download URL: '+name)
        if api.request(url,raw=True)!=(args.candidate/name).read_bytes():
            raise ValueError('GitHub user download checksum mismatch: '+name)
    args.output.write_text(json.dumps({'channel':'github-linux','stage':'discoverable','source_commit':source,'tag':tag,'url':visible['html_url'],'aggregate':'pending-required-aur-channels'},indent=2)+'\n')


def verify_github(args, manifest):
    api=GitHub(args.repository,args.api_url)
    tag=manifest['tag_intent']
    obj=api.repo('git/ref/tags/'+tag)['object']
    for _ in range(10):
        if obj['type']=='commit': break
        if obj['type']!='tag': raise ValueError('GitHub tag has unsupported source identity')
        obj=api.repo('git/tags/'+obj['sha'])['object']
    if obj.get('type')!='commit' or obj['sha']!=manifest['source_commit']:
        raise ValueError('GitHub tag source conflicts with recorded candidate')
    release=api.repo('releases/tags/'+tag)
    if release.get('draft') or release.get('prerelease') or release.get('tag_name')!=tag:
        raise ValueError('GitHub stable source is not publicly discoverable')
    if release.get('body')!=(args.candidate/'release-notes.md').read_text():
        raise ValueError('GitHub release notes conflict with recorded candidate')
    assets={item['name']:item for item in api.pages('releases/'+str(release['id'])+'/assets')}
    for name in [manifest['source_archive'],'pomotui-linux-x86_64.tar.gz','pomotui-linux-x86_64.sha256']:
        asset=assets.get(name,{})
        url=asset.get('browser_download_url')
        if not url or api.request(url,raw=True)!=(args.candidate/name).read_bytes():
            raise ValueError('GitHub user download checksum mismatch or missing asset: '+name)
    return {'channel':'github-linux','stage':'discoverable','source_commit':manifest['source_commit'],'tag':tag,'url':release['html_url']}


def stable_attempt(args, journal):
    from aur import AurChannel
    manifest=verify_gates(args)
    if set(manifest['required_channels'])!={'github-linux','aur-stable','aur-git'} or not manifest.get('tag_intent'):
        raise ValueError('stable publication requires a stable candidate and every Linux channel')
    destination=args.output
    report=journal.value
    save=journal.save
    event=journal.event
    try:
        if args.command=='publish-stable':
            args.preflight=destination.with_name(destination.stem+'-preflight.json')
            args.output=args.preflight
            preflight(args)
            args.output=destination.with_name(destination.stem+'-github.json')
            args.observer=event
            publish(args)
        event(verify_github(args,manifest))
        for channel,package in [('aur-stable','pomotui'),('aur-git','pomotui-git')]:
            client=AurChannel(args.candidate,channel,
                remote=args.aur_remote.replace('{package}',package) if args.aur_remote else None,
                rpc_url=args.aur_rpc_url,acceptance=args.acceptance,
                discovery_attempts=args.discovery_attempts,discovery_delay=args.discovery_delay,
                observer=event,provenance_api_url=args.provenance_api_url,source_repository=args.source_repository)
            previous=report['channels'].get(channel,{}).get('stage')
            event(client.verify() if args.command=='verify-stable' or previous in ('remote-verified','pending-indexing','discoverable','superseded-discoverable') else client.publish())
        report['aggregate']='complete' if all(value['stage'] in ('discoverable','superseded-discoverable') for value in report['channels'].values()) else 'pending-indexing'
        save()
        if report['aggregate']!='complete':
            raise ValueError('AUR remote content verified; pending discovery. Run verify-stable with the same candidate; no new commit or push is needed')
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        if report['aggregate']=='in-progress' and not isinstance(error,(TransientError,TimeoutError,ConnectionError)): report['aggregate']='paused'
        report['diagnosis']=str(error);save();raise
    finally:
        args.output=destination


def stable(args):
    with locked(args.output):
        journal=Journal(args.output,args.candidate)
        journal.execute(lambda: stable_attempt(args,journal),args.retry_attempts,args.retry_delay)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('select')
    p.add_argument('--event', type=Path, required=True)
    p.add_argument('--repo', type=Path, default=Path('.'))
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
    p.add_argument('--api-url', default='https://api.github.com')
    p.set_defaults(func=select)
    p=commands.add_parser('previous')
    p.add_argument('--candidate',type=Path,required=True)
    p.add_argument('--repo',type=Path,default=Path('.'))
    p.add_argument('--repository',default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
    p.add_argument('--api-url',default='https://api.github.com')
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--allow-nonpublishing-fixture-predecessor',action='store_true')
    p.set_defaults(func=previous)
    for command,func in [('verify-gates',verify_gates),('pack',pack),('preflight',preflight),('publish-github',publish)]:
        p=commands.add_parser(command)
        p.add_argument('--candidate',type=Path,required=True)
        if command=='pack':
            p.add_argument('--output',type=Path,required=True)
        else:
            p.add_argument('--acceptance',type=Path,required=True)
            p.add_argument('--checks',type=Path,required=True)
        if command=='preflight':
            p.add_argument('--aur-remote')
            p.add_argument('--aur-rpc-url')
            p.add_argument('--output',type=Path,required=True)
        if command=='publish-github':
            p.add_argument('--selection',type=Path,required=True)
            p.add_argument('--preflight',type=Path,required=True)
            p.add_argument('--repository',default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
            p.add_argument('--api-url',default='https://api.github.com')
            p.add_argument('--output',type=Path,required=True)
        p.set_defaults(func=func)
    for command in ['publish-stable','verify-stable']:
        p=commands.add_parser(command)
        p.add_argument('--candidate',type=Path,required=True)
        p.add_argument('--acceptance',type=Path,required=True)
        p.add_argument('--checks',type=Path,required=True)
        p.add_argument('--selection',type=Path,required=True)
        p.add_argument('--repository',default=os.environ.get('GITHUB_REPOSITORY','SaintFore/pomotui'))
        p.add_argument('--api-url',default='https://api.github.com')
        p.add_argument('--aur-remote')
        p.add_argument('--aur-rpc-url')
        p.add_argument('--provenance-api-url')
        p.add_argument('--source-repository',type=Path)
        p.add_argument('--retry-attempts',type=int,default=3)
        p.add_argument('--retry-delay',type=float,default=2)
        p.add_argument('--discovery-attempts',type=int,default=6)
        p.add_argument('--discovery-delay',type=float,default=10)
        p.add_argument('--output',type=Path,required=True)
        p.set_defaults(func=stable)
    args = parser.parse_args()
    try:
        args.func(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Paused: {error}\n')


if __name__ == '__main__':
    main()
