"""Collect upstream notices from the checksum-pinned native pilot build (Python 3.11+)."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib


HERE = Path(__file__).resolve().parent
NAMED_NOTICE = re.compile(
    r'^(licen[sc]e|copying|copyright|notices?|patents?|unlicense|third.party|authors|credits)(?:$|[. _-])',
    re.IGNORECASE,
)
# Preserve upstream source headers alongside the stand-alone license files. This
# also captures embedded C, assembly generators, SQLite's dedication, and ring's
# file-specific BoringSSL notices.
SOURCE = {'.rs', '.c', '.h', '.cc', '.cpp', '.hpp', '.inc', '.s', '.asm', '.pl'}
COMMENT = re.compile(
    rb'[ \t\r\n]*(/\*.*?\*/|(?:[ \t]*(?://|#(?=[ \t\r\n!])|[;@](?=[ \t\r\n]))[^\n]*(?:\n|$))+)',
    re.DOTALL,
)
LEGAL = re.compile(rb'copyright|public domain|permission is hereby|spdx-license|licensed under|license, version', re.I)
NAPI_REVISIONS = {
    'f2178312d0e3e07beecc19836b91716a229107d3', '67e711d69b7e681807d157ea85cf8173fd923537',
    'a312b7eb4e12d3e0a3e9770429ee05c333947a39', 'f1b8ab5e645e674df33c796ef75aa278cd1b4a31',
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked(path, expected):
    if digest(path.read_bytes()) != expected:
        raise ValueError(f'SHA-256 mismatch for {path.name}')


def source_headers(data):
    offset = 0
    while match := COMMENT.match(data, offset):
        if LEGAL.search(match[1]):
            yield match[1]
        offset = match.end()


def collect_crate(archive, target, name, version):
    prefix = f'{name}-{version}'
    destination = target / prefix
    destination.mkdir()
    headers, notices = {}, []
    with tarfile.open(archive) as source:
        manifest = tomllib.loads(source.extractfile(f'{prefix}/Cargo.toml').read().decode())['package']
        if (manifest['name'], manifest['version']) != (name, version):
            raise ValueError(f'Manifest mismatch for {prefix}')
        for member in source:
            path = PurePosixPath(member.name)
            if not member.isfile():
                continue
            if path.parts[0] != prefix or '..' in path.parts:
                raise ValueError(f'Unexpected member in {prefix}: {path}')
            relative = path.relative_to(prefix)
            if (NAMED_NOTICE.search(path.name) or relative.as_posix() == manifest.get('license-file')
                    or any(p.lower() in {'license', 'licenses', 'notices'} for p in relative.parts[:-1])):
                out = destination / relative
                out.parent.mkdir(parents=True, exist_ok=True)
                out.write_bytes(source.extractfile(member).read())
                notices.append(str(relative))
            elif path.suffix.lower() in SOURCE:
                data = source.extractfile(member).read()
                for header in source_headers(data):
                    headers.setdefault(header, []).append(str(relative))
                if name == 'libsqlite3-sys' and str(relative) == 'sqlite3/sqlite3.c':
                    # The amalgamation contains preprocessor directives between
                    # its source identification and the original dedication.
                    comments = re.finditer(rb'/\*.*?\*/', data, re.DOTALL)
                    first = next(comments)[0]
                    dedication = next((c[0] for c in comments if b'disclaims copyright' in c[0]), None)
                    if dedication is None:
                        raise ValueError('Missing SQLite amalgamation dedication')
                    for header in (first, dedication):
                        headers.setdefault(header, []).append(str(relative))

        origin = None
        if not notices and name in {'napi', 'napi-build', 'napi-derive', 'napi-derive-backend', 'napi-sys'}:
            vcs = json.load(source.extractfile(f'{prefix}/.cargo_vcs_info.json'))['git']['sha1']
            if vcs not in NAPI_REVISIONS:
                raise ValueError(f'Review the upstream napi-rs license at {vcs}')
            origin = f'https://raw.githubusercontent.com/napi-rs/napi-rs/{vcs}/LICENSE'
            notice = HERE / 'napi-rs.LICENSE'
            checked(notice, '3f1ce66533302df3a32edbfdfc0b78f0dd34659e4c1f5817162e5ea3c2297215')
            shutil.copyfile(notice, destination / 'LICENSE')
            notices.append('LICENSE')
    if not notices:
        raise ValueError(f'{prefix} has no packaged license. Review its exact upstream revision.')
    if headers:
        with (destination / 'SOURCE-HEADERS.txt').open('wb') as stream:
            for header, paths in sorted(headers.items(), key=lambda item: min(item[1])):
                for path in sorted(set(paths)):
                    stream.write(f'{path}\n'.encode())
                stream.write(b'\n' + header + b'\n\n')
        notices.append('SOURCE-HEADERS.txt')
    result = {'license': manifest.get('license'), 'source': f'https://static.crates.io/crates/{name}/{prefix}.crate',
              'repository': manifest.get('repository'), 'notices': sorted(notices)}
    if origin:
        result['licenseSource'] = origin
    return result


def collect(args, out):
    build = json.loads((HERE / 'pilot-build.json').read_text())
    checked(args.native_archive, build['archiveSha256'])
    with tarfile.open(args.native_archive) as source:
        binary = source.extractfile('package/stateset-embedded.linux-x64-gnu.node')
        if digest(binary.read()) != build['nativeSha256']:
            raise ValueError('Native member differs from the retained build')

    rust_doc = args.rust_doc
    if rust_doc is None:
        sysroot = subprocess.check_output(['rustc', '+' + build['rustVersion'], '--print', 'sysroot'], text=True)
        rust_doc = Path(sysroot.strip()) / 'share/doc/rust'
    checked(rust_doc / 'COPYRIGHT-library.html', build['rustLibraryNoticesSha256'])
    rust_out = out / 'rust-standard-library'
    rust_out.mkdir()
    shutil.copyfile(rust_doc / 'COPYRIGHT-library.html', rust_out / 'COPYRIGHT-library.html')
    for filename in ['Apache-2.0.txt', 'MIT.txt', 'LLVM-exception.txt']:
        shutil.copyfile(rust_doc / 'licenses' / filename, rust_out / filename)

    lock = tomllib.loads((HERE.parent.parent / 'Cargo.lock').read_text())['package']
    locked = {(p['name'], p['version']): p['checksum'] for p in lock
              if p.get('source') == 'registry+https://github.com/rust-lang/crates.io-index'}
    crates = out / 'crates'
    crates.mkdir()
    entries = []
    for key, flags in build['registryFeatures'].items():
        name, version = key.split('@')
        checksum = locked[name, version]
        filename = f'{name}-{version}.crate'
        archive = next(args.cache.glob(f'*/{filename}'), None)
        if archive is None:
            raise ValueError(f'Missing {filename} from {args.cache}. Run cargo fetch --locked first.')
        checked(archive, checksum)
        record = collect_crate(archive, crates, name, version)
        entries.append({'name': name, 'version': version, 'features': flags.split(',') if flags else [],
                        'sourceSha256': checksum, **record})
    index = {'build': {k: v for k, v in build.items() if k != 'registryFeatures'},
             'rustStandardLibrary': 'rust-standard-library/COPYRIGHT-library.html', 'crates': entries}
    (out / 'inventory.json').write_text(json.dumps(index, indent=2) + '\n')
    shutil.copyfile(HERE / 'NOTICES.md', out / 'README.md')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native_archive', type=Path, help='The pinned @stateset/embedded-linux-x64-gnu npm tarball')
    parser.add_argument('output', type=Path, help='A new directory for the verified notice bundle')
    cargo = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    parser.add_argument('--cache', type=Path, default=cargo / 'registry/cache', help='Cargo registry cache root')
    parser.add_argument('--rust-doc', type=Path, help='Rust toolchain share/doc/rust directory')
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Select a new output directory')
    with tempfile.TemporaryDirectory(dir=args.output.parent) as staging:
        out = Path(staging) / 'notices'
        out.mkdir()
        collect(args, out)
        out.rename(args.output)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError, tarfile.TarError) as error:
        sys.exit(str(error))
