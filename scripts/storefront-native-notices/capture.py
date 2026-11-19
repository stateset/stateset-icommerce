"""Capture the retained Cargo units for the pinned Linux storefront preview."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tarfile


def capture(profile, archive, rust_docs):
    native = profile / 'libstateset_embedded_node.so'
    digest = hashlib.sha256(native.read_bytes()).hexdigest()
    with tarfile.open(archive) as tgz:
        body = tgz.extractfile('package/stateset-embedded.linux-x64-gnu.node')
        if hashlib.sha256(body.read()).hexdigest() != digest:
            raise ValueError('The retained build and the supplied native binary differ')

    comment = subprocess.check_output(['readelf', '-p', '.comment', native], text=True)
    compiler = re.search(r'rustc version (\S+) \(([a-f0-9]+) ', comment)
    if not compiler:
        raise ValueError('Missing Rust compiler identification on the native binary')

    fingerprints = profile / '.fingerprint'
    root, = fingerprints.glob('stateset-embedded-node-*/lib-stateset_embedded_node.json')
    # Cargo deps store the dependency fingerprint as a u64. The sidecar stores
    # its little-endian bytes as hexadecimal. Visit only units reachable from the cdylib.
    by_hash = {path.with_suffix('').read_text().strip(): path
               for path in fingerprints.glob('*/*.json') if path.with_suffix('').is_file()}
    pending = [root]
    seen = set()
    registry, workspace = {}, {}
    while pending:
        unit = pending.pop()
        if unit in seen:
            continue
        seen.add(unit)
        data = json.loads(unit.read_text())
        pending.extend(by_hash[struct.pack('<Q', dep[3]).hex()] for dep in data['deps'])
        if not unit.name.startswith('lib-'):
            continue
        name, suffix = unit.parent.name.rsplit('-', 1)
        lib = unit.stem.removeprefix('lib-')
        filename = f'{lib}.d' if unit == root else f'{lib}-{suffix}.d'
        dep_info = (profile / 'deps' / filename).read_text()
        versions = set(re.findall(r'/registry/src/[^/ ]+/' + re.escape(name)
                                  + r'-([0-9][^/ ]*)/', dep_info))
        if versions:
            version, = versions
            bucket = registry.setdefault(f'{name}@{version}', set())
        elif unit == root or (name.startswith('stateset-') and f'/{name}/' in dep_info):
            bucket = workspace.setdefault(name, set())
        else:
            raise ValueError(f'Unknown dependency source for {name}')
        bucket.update(json.loads(data['features']))

    def features(packages):
        return {name: ','.join(sorted(flags)) for name, flags in sorted(packages.items())}

    return {
        'target': 'x86_64-unknown-linux-gnu', 'profile': profile.name,
        'archiveSha256': hashlib.sha256(archive.read_bytes()).hexdigest(),
        'nativeSha256': digest, 'rustVersion': compiler[1], 'rustCommitPrefix': compiler[2],
        'rustLibraryNoticesSha256': hashlib.sha256(rust_docs.read_bytes()).hexdigest(),
        'rootUnit': root.parent.name, 'reachableUnits': len(seen),
        'workspaceFeatures': features(workspace), 'registryFeatures': features(registry),
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('profile', type=Path, help='Original target/release-node directory')
    parser.add_argument('archive', type=Path, help='Packed native npm dependency')
    parser.add_argument('rust_docs', type=Path, help='Matching toolchain COPYRIGHT-library.html')
    args = parser.parse_args()
    print(json.dumps(capture(args.profile, args.archive, args.rust_docs), indent=2))
