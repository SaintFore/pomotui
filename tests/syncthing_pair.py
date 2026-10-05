"""Isolated Syncthing transport helper for spec #1 acceptance.

Uses only temporary profiles, explicit localhost addresses, and generated keys.
API references: https://docs.syncthing.net/rest/config.html
"""
import copy
import json
import pathlib
import socket
import subprocess
import time
import urllib.request
import xml.etree.ElementTree as ET


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class SyncthingPair:
    def __init__(self, root):
        self.root = pathlib.Path(root)
        self.peers = []
        for name in ('a', 'b'):
            home = self.root / name / 'syncthing'
            replica = self.root / name / 'replica'
            replica.mkdir(parents=True, exist_ok=True)
            subprocess.run(['syncthing', 'generate', '--home', str(home), '--no-port-probing'], check=True, capture_output=True)
            tree = ET.parse(home / 'config.xml')
            config = tree.getroot()
            peer = dict(home=home, replica=replica, tree=tree,
                        identity=config.find('device').get('id'),
                        gui=free_port(), tcp=free_port(), process=None)
            config.find('gui/address').text = f"127.0.0.1:{peer['gui']}"
            peer['key'] = config.find('gui/apikey').text
            options = config.find('options')
            options.find('listenAddress').text = f"tcp://127.0.0.1:{peer['tcp']}"
            for tag in ('globalAnnounceEnabled', 'localAnnounceEnabled', 'relaysEnabled', 'natEnabled', 'startBrowser', 'crashReportingEnabled'):
                options.find(tag).text = 'false'
            options.find('autoUpgradeIntervalH').text = '0'
            options.find('urAccepted').text = '-1'
            options.find('reconnectionIntervalS').text = '1'
            self.peers.append(peer)
        for peer, other in (self.peers, self.peers[::-1]):
            config = peer['tree'].getroot()
            device = copy.deepcopy(config.find('defaults/device'))
            device.set('id', other['identity'])
            device.find('address').text = f"tcp://127.0.0.1:{other['tcp']}"
            config.insert(0, device)
            folder = copy.deepcopy(config.find('defaults/folder'))
            folder.set('id', 'pomotui-acceptance')
            folder.set('path', str(peer['replica']))
            folder.set('label', 'isolated pomotui acceptance')
            folder.set('rescanIntervalS', '1')
            folder.set('fsWatcherEnabled', 'false')
            folder.find('minDiskFree').text = '0'
            folder.append(ET.Element('device', {'id': other['identity']}))
            config.insert(0, folder)
            peer['tree'].write(peer['home'] / 'config.xml', encoding='unicode')

    def api(self, index, path, method='GET', data=None):
        peer = self.peers[index]
        request = urllib.request.Request(
            f"http://127.0.0.1:{peer['gui']}/rest/{path}", method=method,
            data=None if data is None else json.dumps(data).encode(),
            headers={'X-API-Key': peer['key'], 'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=5) as response:
            body = response.read()
            return json.loads(body) if body else None

    def start(self):
        for peer in self.peers:
            peer['log'] = open(peer['home'] / 'acceptance.log', 'w')
            peer['process'] = subprocess.Popen(
                ['syncthing', 'serve', '--home', str(peer['home']), '--no-browser', '--no-restart', '--no-upgrade'],
                stdout=peer['log'], stderr=subprocess.STDOUT)
        self.wait(lambda: all(self.api(i, 'system/ping') for i in (0, 1)))

    def pause(self, paused):
        for i, peer in enumerate(self.peers):
            other = self.peers[1-i]
            self.api(i, f"config/devices/{other['identity']}", 'PATCH', {'paused': paused})
        if paused:
            self.wait(lambda: all(not self.api(i, 'system/connections')['connections'].get(self.peers[1-i]['identity'], {}).get('connected', False) for i in (0, 1)))

    def scan(self):
        for i in (0, 1):
            self.api(i, 'db/scan?folder=pomotui-acceptance', 'POST')

    def wait(self, predicate, timeout=40):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                if predicate():
                    return
            except (OSError, ValueError):
                pass
            time.sleep(.1)
        raise AssertionError('Syncthing acceptance barrier timed out')

    def close(self):
        for peer in self.peers:
            process = peer.get('process')
            if process:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                peer['log'].close()


if __name__ == '__main__':
    import tempfile
    with tempfile.TemporaryDirectory(prefix='pomotui-syncthing-probe-') as root:
        pair = SyncthingPair(root)
        try:
            pair.start()
            a, b = (p['replica'] / 'pomotui.sync' for p in pair.peers)
            a.write_text('baseline')
            pair.scan()
            pair.wait(lambda: b.exists() and b.read_text() == 'baseline')
            pair.pause(True)
            a.write_text('offline a')
            b.write_text('offline b')
            pair.scan()
            pair.pause(False)
            pair.wait(lambda: any(any(p['replica'].glob('*sync-conflict*')) for p in pair.peers))
            print('PASS: isolated localhost Syncthing baseline and divergent-write conflict')
        finally:
            pair.close()
