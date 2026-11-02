#!/usr/bin/env python3
"""Measure nodes sharing and assert its documented lossless reference contract.

Run in Docker with --binary target/debug/figlens. --scopes is a JSON array of
{file, ids: [node_id, ...]}; keep private inputs and reports under samples/.
"""

import argparse
import copy
import json
import statistics
import subprocess

STYLE_FIELDS = ("fills", "strokes", "effects", "style", "boundVariables", "layoutGrids")


def resolve(compact):
    """Decode only documented reference slots; shared style values stay literal."""
    assert compact["format"] == "figlens.nodes.compact.v1"
    data = copy.deepcopy(compact["data"])

    def place(node, placements):
        node.update(next(placements))
        for child in node.get("children", []):
            place(child, placements)

    def node(value):
        if "$subtree" in value:
            template = copy.deepcopy(compact["subtrees"][value["$subtree"]])
            placements = iter(value["placements"])
            place(template, placements)
            assert next(placements, None) is None
            value = template
        for key in STYLE_FIELDS:
            ref = value.get(key)
            if isinstance(ref, dict) and set(ref) == {"$style"}:
                value[key] = copy.deepcopy(compact["sharedStyles"][ref["$style"]])
        if "children" in value:
            value["children"] = [node(child) for child in value["children"]]
        return value

    data["document"] = node(data["document"])
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/figlens")
    parser.add_argument("--scopes", required=True)
    args = parser.parse_args()
    with open(args.scopes, encoding="utf-8") as source:
        scopes = json.load(source)
    for scope in scopes:
        modes = []
        for flags in ([], ["--compact"], ["--compact", "--subtrees"]):
            run = subprocess.run(
                [args.binary, "nodes", scope["file"], "--ids", *scope["ids"], *flags],
                check=True, capture_output=True, text=True,
            )
            rows = [(json.loads(line), len(line.encode("utf-8"))) for line in run.stdout.splitlines()]
            assert len(rows) == len(scope["ids"])
            modes.append(rows)
        savings = [[], []]
        for original, styles, subtrees in zip(*modes):
            default, size = original
            assert "error" not in default, default
            node_id = default.pop("nodeId")
            for i, (compact, length) in enumerate((styles, subtrees)):
                assert "error" not in compact, compact
                assert compact.pop("nodeId") == node_id
                assert resolve(compact) == default, (scope["file"], node_id, i)
                savings[i].append(1 - length / size)
        print(json.dumps({
            "file": scope["file"], "screens": len(modes[0]),
            "medianDefaultBytes": statistics.median(size for _, size in modes[0]),
            "medianStyleSavingsPercent": 100 * statistics.median(savings[0]),
            "medianSubtreeSavingsPercent": 100 * statistics.median(savings[1]),
            "roundtrip": "exact",
        }))


if __name__ == "__main__":
    main()
