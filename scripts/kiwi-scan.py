#!/usr/bin/env python3
"""Look inside a .fig's Kiwi data without going through figlens.

Format research only: find which stored fields a feature uses before decoding it in
src/. figlens itself never runs this; it trusts its input and has no parsing budgets,
so point it only at files you own.

  kiwi-scan.py defs <file.fig> [Type...]           schema definitions (all, or the named types)
  kiwi-scan.py defs <file.fig> --field <name>...   which types declare a field with that name
  kiwi-scan.py scan <file.fig>... --field <name>...
                                                   count NodeChanges carrying each field, with one sample

Zstandard-compressed files need Python 3.14 (compression.zstd); deflate files run anywhere.
"""
import argparse
import io
import json
import struct
import sys
import zipfile
import zlib

KINDS = ['ENUM', 'STRUCT', 'MESSAGE']
BASIC = {-1: 'bool', -2: 'byte', -3: 'int', -4: 'uint', -5: 'float', -6: 'string', -7: 'int64', -8: 'uint64'}


def chunks(path):
    data = open(path, 'rb').read()
    if data[:2] == b'PK':
        data = zipfile.ZipFile(io.BytesIO(data)).read('canvas.fig')
    if data[:8] != b'fig-kiwi':
        sys.exit(f'{path}: not a fig-kiwi container (starts with {data[:8]!r})')
    off, out = 12, []
    while off + 4 <= len(data):
        n = struct.unpack_from('<I', data, off)[0]
        off += 4
        out.append(data[off:off + n])
        off += n
    return out


def inflate(chunk):
    if chunk[:4] == b'\x28\xb5\x2f\xfd':
        try:
            from compression import zstd
        except ImportError:
            sys.exit('this file uses Zstandard; run with Python 3.14 or later')
        return zstd.decompress(chunk)
    return zlib.decompress(chunk, -15)


class Reader:
    def __init__(self, data):
        self.data, self.i = data, 0

    def byte(self):
        v = self.data[self.i]
        self.i += 1
        return v

    def uint(self):
        result = shift = 0
        while True:
            c = self.byte()
            result |= (c & 127) << shift
            shift += 7
            if c < 128 or shift >= 35:
                return result

    def uint64(self):
        # As vendor/kiwi-schema's read_var_uint64: the ninth byte keeps all eight bits.
        result = shift = 0
        while True:
            c = self.byte()
            if c < 128 or shift >= 56:
                return result | c << shift
            result |= (c & 127) << shift
            shift += 7

    def int64(self):
        v = self.uint64()
        return ~(v >> 1) if v & 1 else v >> 1

    def int(self):
        v = self.uint()
        return ~(v >> 1) if v & 1 else v >> 1

    def float(self):
        first = self.byte()
        if first == 0:
            return 0.0
        bits = first | self.byte() << 8 | self.byte() << 16 | self.byte() << 24
        bits = ((bits << 23) | (bits >> 9)) & 0xffffffff
        return struct.unpack('<f', struct.pack('<I', bits))[0]

    def string(self):
        end = self.data.index(0, self.i)
        v = self.data[self.i:end].decode('utf-8', 'replace')
        self.i = end + 1
        return v


def schema(data):
    r, defs = Reader(data), []
    for _ in range(r.uint()):
        name, kind, fields = r.string(), KINDS[r.byte()], []
        for _ in range(r.uint()):
            fields.append((r.string(), r.int(), r.byte(), r.uint()))
        defs.append((name, kind, fields))
    return defs


def type_name(defs, t, array):
    return (BASIC[t] if t < 0 else defs[t][0]) + ('[]' if array else '')


def value(defs, t, r):
    if t < 0:
        return {-1: lambda: bool(r.byte()), -2: r.byte, -3: r.int, -4: r.uint, -5: r.float,
                -6: r.string, -7: r.int64, -8: r.uint64}[t]()
    name, kind, fields = defs[t]
    if kind == 'ENUM':
        v = r.uint()
        return next((f[0] for f in fields if f[3] == v), v)
    if kind == 'STRUCT':
        return {f[0]: field(defs, f[1], f[2], r) for f in fields}
    out, by_key = {}, {f[3]: f for f in fields}
    while (key := r.uint()) != 0:
        if key not in by_key:
            # A newer field this schema lacks: keep what was read.
            out['_unknownField'] = key
            return out
        fname, ft, array, _ = by_key[key]
        out[fname] = field(defs, ft, array, r)
    return out


def field(defs, t, array, r):
    if not array:
        return value(defs, t, r)
    n = r.uint()
    if t == -2:
        r.i += n
        return f'<{n} bytes>'
    return [value(defs, t, r) for _ in range(n)]


def show_defs(path, names, fields):
    defs = schema(inflate(chunks(path)[0]))
    if fields:
        for name, kind, fs in defs:
            for f in fs:
                if kind != 'ENUM' and f[0] in fields:
                    print(f'{name}.{f[0]}: {type_name(defs, f[1], f[2])}')
        return
    wanted = set(names)
    for name, kind, fs in defs:
        if wanted and name not in wanted:
            continue
        body = ', '.join(f[0] if kind == 'ENUM' else f'{f[0]}: {type_name(defs, f[1], f[2])}' for f in fs)
        print(f'{kind} {name} {{ {body} }}')
    missing = wanted - {d[0] for d in defs}
    if missing:
        print('not in schema:', ', '.join(sorted(missing)), file=sys.stderr)


def scan(path, fields, sample_chars):
    parts = chunks(path)
    defs = schema(inflate(parts[0]))
    message = next(i for i, d in enumerate(defs) if d[0] == 'Message')
    nodes = value(defs, message, Reader(inflate(parts[1]))).get('nodeChanges', [])
    counts = {f: 0 for f in fields}
    samples = {}
    for node in nodes:
        for f in fields:
            if node.get(f) not in (None, [], {}, ''):
                counts[f] += 1
                samples.setdefault(f, {'guid': node.get('guid'), 'type': node.get('type'), f: node[f]})
    print(f'{path}: {len(nodes)} nodes')
    for f in fields:
        print(f'  {f}: {counts[f]}')
        if f in samples:
            print('    sample', json.dumps(samples[f], ensure_ascii=False, default=str)[:sample_chars])


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('mode', choices=['defs', 'scan'])
    p.add_argument('files', nargs='+')
    p.add_argument('--field', action='append', default=[], help='field name; repeatable')
    p.add_argument('--sample-chars', type=int, default=600)
    a = p.parse_args(argv)
    if a.mode == 'defs':
        show_defs(a.files[0], a.files[1:], a.field)
        return 0
    if not a.field:
        p.error('scan needs at least one --field')
    for path in a.files:
        scan(path, a.field, a.sample_chars)
    return 0


if __name__ == '__main__':
    sys.exit(main())
