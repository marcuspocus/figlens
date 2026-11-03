import contextlib
import io
from pathlib import Path
import runpy
import unittest

HERE = Path(__file__).resolve().parent
SCAN = runpy.run_path(str(HERE / 'kiwi-scan.py'))
FIXTURE = str(HERE.parent / 'examples' / 'first-success' / 'design.fig')


def run(*args):
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        SCAN['main'](list(args))
    return out.getvalue()


class KiwiScanTest(unittest.TestCase):
    def test_defs_lists_a_named_type_with_its_fields(self):
        self.assertRegex(run('defs', FIXTURE, 'NodeChange'), r'MESSAGE NodeChange \{ .*name: string')

    def test_defs_field_names_every_type_declaring_it(self):
        self.assertIn('NodeChange.guid: GUID', run('defs', FIXTURE, '--field', 'guid').splitlines())

    def test_scan_counts_nodes_carrying_a_field(self):
        out = run('scan', FIXTURE, '--field', 'name')
        nodes = int(out.split(': ')[1].split()[0])
        self.assertIn(f'  name: {nodes}', out.splitlines())

    def test_64_bit_integers_match_the_kiwi_wire_format(self):
        read = lambda b: SCAN['Reader'](bytes(b))
        self.assertEqual(read([1]).int64(), -1)
        self.assertEqual(read([0x81] + [0x80] * 7 + [0x20]).int64(), -0x1000_0000_0000_0001)
        self.assertEqual(read([0xFF] * 9 + [0]).uint64(), 0xFFFF_FFFF_FFFF_FFFF)

    def test_a_ninth_byte_ends_a_64_bit_integer(self):
        r = SCAN['Reader'](bytes([0xFF] * 9 + [7]))
        r.uint64()
        self.assertEqual(r.byte(), 7)


if __name__ == '__main__':
    unittest.main()
