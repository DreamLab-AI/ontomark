#!/usr/bin/env python3
"""Validate an ontomark frontmatter data graph with an independent SHACL engine
(pyshacl) and compare the result with `ontomark validate`.

    shacl_check.py --shapes shapes.ttl --data knowledge.data.ttl \
                   --validate validate.json --vault knowledge [--json out.json]

The shapes graph is checked against SHACL-SHACL (`meta_shacl=True`); no
inference and no SHACL Advanced Features are used. Each validation result is
mapped back to the code `ontomark validate` reports through the shape's
`ruleCode` annotation, and to a page id through the data graph's `meta-id`.
Shapes without a `ruleCode` are profile-only rules (`profileRule true`) and are
reported under `PROFILE:<shape name>`.

Exit status is 0 whatever the comparison finds; the point is the report.
"""
import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from urllib.parse import unquote

import rdflib
from rdflib import Graph, Literal, URIRef
from rdflib.namespace import RDF
import pyshacl

SH = rdflib.Namespace("http://www.w3.org/ns/shacl#")
SEVERITY = {str(SH.Violation): "error", str(SH.Warning): "warning", str(SH.Info): "info"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--shapes", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--validate", required=True, help="`ontomark --json validate --vault all` output")
    ap.add_argument("--vault", required=True, help="knowledge or working")
    ap.add_argument("--json", help="write the comparison as JSON here")
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--per-page", metavar="REGEX", help="also tabulate pages whose id matches REGEX (the mutations)")
    args = ap.parse_args()

    shapes = Graph().parse(args.shapes, format="turtle")
    data = Graph().parse(args.data, format="turtle")
    # the vocabulary namespace is whatever the shapes' ruleCode property lives in
    rule_code = next(p for _, p, _ in shapes if str(p).endswith("ruleCode"))
    ns = str(rule_code)[: -len("ruleCode")]
    meta_id = URIRef(ns + "meta-id")
    meta_identity = URIRef(ns + "meta-identity")

    conforms, report_graph, _ = pyshacl.validate(
        data, shacl_graph=shapes, meta_shacl=True, inference="none",
        advanced=False, js=False, abort_on_first=False,
    )

    # `ruleCode` marks a shape that implements all of a validate rule;
    # `partialRuleCode` one that implements the SHACL-expressible part of a
    # procedural rule (reported under the code, but not counted as covered).
    partial_code = URIRef(ns + "partialRuleCode")
    code_of = {}
    for shape, _, code in shapes.triples((None, rule_code, None)):
        code_of[str(shape)] = str(code)
    for shape, _, code in shapes.triples((None, partial_code, None)):
        code_of[str(shape)] = str(code)
    covered = {str(c) for _, _, c in shapes.triples((None, rule_code, None))}
    def shape_name(shape):
        return str(shape).split("shape-", 1)[-1]

    id_of = {s: str(o) for s, _, o in data.triples((None, meta_id, None))}
    owners = defaultdict(list)
    for s, _, o in data.triples((None, meta_identity, None)):
        owners[o].append(id_of.get(s, str(s)))

    results = []  # (page, code, severity, shape, path, value)
    details = set(report_graph.objects(None, SH.detail))  # nested results of sh:node/sh:or
    for r in report_graph.subjects(RDF.type, SH.ValidationResult):
        if r in details:
            continue
        focus = report_graph.value(r, SH.focusNode)
        shape = report_graph.value(r, SH.sourceShape)
        sev = SEVERITY[str(report_graph.value(r, SH.resultSeverity))]
        code = code_of.get(str(shape), "PROFILE:" + shape_name(shape))
        value = report_graph.value(r, SH.value)
        path = report_graph.value(r, SH.resultPath)
        # DUPLICATE_IRI: the focus is the shared identity; `validate` names the first owner.
        pages = [id_of.get(focus)] if focus in id_of else sorted(owners.get(focus, [str(focus)]))[:1]
        for page in pages:
            results.append((page, code, sev, shape_name(shape), str(path) if path else "", str(value) if value is not None else ""))

    v = json.load(open(args.validate))[args.vault]
    expected = [(i["path"], i["code"], i["severity"]) for i in v["issues"]]

    shacl_pc = Counter((p, c) for p, c, *_ in results if not c.startswith("PROFILE:"))
    val_pc = Counter((p, c) for p, c, _ in expected)
    shacl_codes = Counter(c for _, c, *_ in results)
    val_codes = Counter(c for _, c, _ in expected)
    # DUPLICATE_IRI: validate names owners[0] only; SHACL names every owner.
    # Compare it on the first owner of each shape result.
    all_codes = sorted(set(shacl_codes) | set(val_codes))
    table = []
    for code in all_codes:
        s_set = {pc for pc in shacl_pc if pc[1] == code}
        v_set = {pc for pc in val_pc if pc[1] == code}
        table.append({
            "code": code,
            "validate_issues": val_codes.get(code, 0),
            "shacl_results": shacl_codes.get(code, 0),
            "validate_pages": len(v_set),
            "shacl_pages": len(s_set),
            "only_validate": sorted(p for p, _ in v_set - s_set)[:2000],
            "only_shacl": sorted(p for p, _ in s_set - v_set)[:2000],
            "agree": (s_set == v_set) if not code.startswith("PROFILE:") else None,
        })
    # DANGLING_LINK counts: `validate` reports one issue per (key, link); the
    # SHACL path is an alternative path, so repeats collapse. Compare distinct
    # (page, target) pairs.
    v_dangling = {(i["path"], re.search(r"points at `(.*)`, which", i["message"]).group(1))
                  for i in v["issues"] if i["code"] == "DANGLING_LINK"}
    s_dangling = {(p, unquote(val.rsplit("unresolved/", 1)[-1])) for p, c, _, _, _, val in results if c == "DANGLING_LINK"}
    dangling_agree = v_dangling == s_dangling
    by_sev = Counter(sev for _, _, sev, *_ in results)
    out = {
        "pyshacl": pyshacl.__version__, "rdflib": rdflib.__version__,
        "data_triples": len(data), "shapes_triples": len(shapes),
        "conforms_any_severity": bool(conforms),
        "results_by_severity": dict(by_sev),
        "validate_totals": {k: v[k] for k in ("errors", "warnings", "info")},
        "dangling_distinct": {"validate": len(v_dangling), "shacl": len(s_dangling), "agree": dangling_agree},
        "table": table,
        "results": [list(r) for r in results[:2000]],
    }
    if args.json:
        json.dump(out, open(args.json, "w"), indent=1)
    if not args.quiet:
        print(f"pyshacl {pyshacl.__version__} / rdflib {rdflib.__version__}: {len(data)} data triples, "
              f"{len(shapes)} shape triples; sh:conforms={conforms}; results {dict(by_sev)}; "
              f"validate errors/warnings/info = {v['errors']}/{v['warnings']}/{v['info']}")
        print(f"{'code':32} {'validate':>8} {'shacl':>8}  pages(v/s)   agree")
        for row in table:
            agree = {True: "YES", False: "NO", None: "-"}[row["agree"]]
            print(f"{row['code']:32} {row['validate_issues']:>8} {row['shacl_results']:>8}  "
                  f"{row['validate_pages']:>5}/{row['shacl_pages']:<5}  {agree}")
            if row["agree"] is False:
                print(f"    only validate: {row['only_validate'][:5]}  only shacl: {row['only_shacl'][:5]}")
        print(f"DANGLING_LINK distinct (page, target) pairs: validate {len(v_dangling)}, shacl {len(s_dangling)}, "
              f"identical={dangling_agree}")
        print(f"profile-only shape results (no validate rule): "
              f"{dict(Counter(c for _, c, *_ in results if c.startswith('PROFILE:')))}")
    if args.per_page:
        pat = re.compile(args.per_page)
        pages = sorted({p for p, _ in list(shacl_pc) + list(val_pc) if pat.match(p)}
                       | {p for p, c, *_ in results if pat.match(p)})
        rows = []
        for page in pages:
            v_codes = sorted({c for p, c, _ in expected if p == page})
            s_codes = sorted({c for p, c, *_ in results if p == page and not c.startswith("PROFILE:")})
            prof = sorted({c[8:] for p, c, *_ in results if p == page and c.startswith("PROFILE:")})
            v_cov = sorted(c for c in v_codes if c in covered)
            proc = sorted(c for c in v_codes if c not in covered)
            rows.append({"page": page, "validate": v_codes, "shacl": s_codes, "profile_only": prof,
                         "validate_procedural_only": proc, "agree": v_cov == s_codes})
        out["mutations"] = rows
        if args.json:
            json.dump(out, open(args.json, "w"), indent=1)
        if not args.quiet:
            print("\nper page (validate codes | shacl codes | profile-only shapes | agree on shacl-covered codes):")
            for r in rows:
                print(f"  {r['page'][:42]:42} V={','.join(r['validate']) or '-'} | S={','.join(r['shacl']) or '-'} "
                      f"| P={','.join(r['profile_only']) or '-'} | {'AGREE' if r['agree'] else 'DIFFER'}"
                      + (f" (procedural only in validate: {','.join(r['validate_procedural_only'])})" if r['validate_procedural_only'] else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
