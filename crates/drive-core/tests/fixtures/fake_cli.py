#!/usr/bin/env python3
"""Isolated integration-test double for the official CLI's filesystem protocol."""
import hashlib
import json
import pathlib
import shutil
import sys

base = pathlib.Path(__file__).parent
remote = base / 'remote'
args = [arg for arg in sys.argv[1:] if arg != '--json']

def path(value):
    if value.startswith('/devices/'):
        return remote / '_devices' / value.removeprefix('/devices/')
    assert value == '/my-files' or value.startswith('/my-files/')
    return remote / value.removeprefix('/my-files').lstrip('/')

def node(item):
    content = item.read_bytes() if item.is_file() else b''
    digest = hashlib.sha1(content).hexdigest()
    return {
        'uid': str(item.relative_to(remote)),
        'name': {'ok': True, 'value': item.name},
        'type': 'folder' if item.is_dir() else 'file',
        'mediaType': 'application/vnd.proton.doc' if item.suffix == '.protondoc' else 'text/plain',
        'activeRevision': None if item.is_dir() else {
            'uid': digest, 'claimedSize': len(content),
            'claimedDigests': {'sha1': digest, 'sha1Verified': True},
        },
    }

def photo_node(uid):
    item = next((base / 'photos' / uid).iterdir())
    content = item.read_bytes()
    digest = hashlib.sha1(content).hexdigest()
    return {'uid': uid, 'name': {'ok': True, 'value': item.name}, 'type': 'photo',
        'mediaType': 'image/jpeg', 'activeRevision': {'uid': digest, 'claimedSize': len(content), 'claimedDigests': {'sha1': digest}}}

def photo_nodes():
    return [photo_node(p.name) for p in sorted((base / 'photos').iterdir())]

def albums():
    return json.loads((base / 'albums.json').read_text())

try:
    if args[0] == 'photo':
        if args[1] == 'timeline':
            print(json.dumps(photo_nodes()))
        elif args[1] == 'download':
            uid = args[-2].split('/')[-1]
            item = next((base / 'photos' / uid).iterdir())
            shutil.copyfile(item, pathlib.Path(args[-1]) / item.name)
            print('{}')
        elif args[1] == 'upload':
            item = pathlib.Path(args[-1])
            digest = hashlib.sha1(item.read_bytes()).hexdigest()
            if not any(n['name']['value'] == item.name and n['activeRevision']['uid'] == digest for n in photo_nodes()):
                uid = 'uploaded-' + digest
                (base / 'photos' / uid).mkdir()
                shutil.copyfile(item, base / 'photos' / uid / item.name)
            print('{}')
        else: raise ValueError('Unknown photo command')
        sys.exit(0)
    if args[0] == 'album':
        data = albums()
        if args[1] == 'list':
            print(json.dumps([{'uid': uid, 'name': {'ok': True, 'value': a['name']}, 'type': 'album', 'album': {'photoCount': len(a['photos'])}} for uid, a in data.items()]))
        elif args[1] == 'photos':
            uid = args[-1].split('/')[-1]
            print(json.dumps([photo_node(p) for p in data[uid]['photos']]))
        elif args[1] == 'add-photo':
            uid, photo = args[2].split('/')[-1], args[3].split('/')[-1]
            if photo not in data[uid]['photos']: data[uid]['photos'].append(photo)
            (base / 'albums.json').write_text(json.dumps(data))
            print('{}')
        else: raise ValueError('Unknown album command')
        sys.exit(0)
    command = args[1]
    if command == 'list' and args[-1] == '/devices':
        print((base / 'devices.json').read_text())
        sys.exit(0)
    if command == 'list':
        if (base / 'fail-list').exists(): raise ValueError('Metadata unavailable')
        entries = sorted(path(args[-1]).iterdir())
        if '--type' in args:
            entries = [p for p in entries if p.is_dir()]
        print(json.dumps([node(p) for p in entries]))
    elif command == 'info':
        if args[2].startswith('/photos/'):
            print(json.dumps(photo_node(args[2].split('/')[-1])))
        else: print(json.dumps(node(path(args[2]))))
    elif command == 'trash':
        if (base / 'fail-trash').exists(): print('[{"ok":false}]')
        else:
            src = path(args[2])
            dest = base / 'trash' / src.relative_to(remote)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(src, dest)
            print('[{"ok":true}]')
    elif command == 'create-folder':
        (path(args[2]) / args[3]).mkdir()
        print('{}')
    elif command in ('upload', 'download'):
        if (base / 'fail-transfer').exists():
            print('Simulated transfer failure', file=sys.stderr)
            sys.exit(1)
        source, target = args[-2:]
        if command == 'upload':
            src = pathlib.Path(source)
            dest = path(target) / src.name
            if args[3] != 'skip' or not dest.exists(): shutil.copyfile(src, dest)
        else:
            src = path(source)
            dest = pathlib.Path(target) / src.name
            shutil.copyfile(src, dest)
        print(json.dumps({'succeeded': 1, 'failed': 0}))
    else:
        raise ValueError(f'Unknown command: {command}')
except Exception as error:
    print(str(error), file=sys.stderr)
    sys.exit(1)
