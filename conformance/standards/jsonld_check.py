#!/usr/bin/env python3
"""Check that a JSON-LD document and a Turtle document hold the same RDF graph,
using two independent JSON-LD 1.1 processors and one Turtle parser.

    jsonld_check.py <name> <file.ttl> <file.jsonld>

* rdflib (Turtle parser; built-in JSON-LD parser) compares the graphs with
  `rdflib.compare.isomorphic`, which is blank-node isomorphism.
* PyLD (the reference JSON-LD 1.1 processor of the JSON-LD CG) expands the
  document to N-Quads (`toRdf`), which rdflib then reads; the result is compared
  to the Turtle the same way.

Prints one line per check and exits non-zero on any difference.
"""
import importlib.metadata
import sys

import pyld
import rdflib
from pyld import jsonld
from rdflib import Graph
from rdflib.compare import isomorphic


def main():
    name, ttl_path, json_path = sys.argv[1:4]
    turtle = Graph().parse(ttl_path, format="turtle")
    via_rdflib = Graph().parse(json_path, format="json-ld")
    import json
    with open(json_path) as handle:
        doc = json.load(handle)
    nquads = jsonld.to_rdf(doc, {"format": "application/n-quads"})
    via_pyld = Graph().parse(data=nquads, format="nquads")
    ok = True
    for label, other in (("rdflib JSON-LD", via_rdflib), ("PyLD toRdf", via_pyld)):
        same = isomorphic(turtle, other)
        ok &= same
        print(f"{name}: Turtle {len(turtle)} triples vs {label} {len(other)} triples: "
              f"isomorphic={same}")
    print(f"{name}: rdflib {rdflib.__version__}, PyLD {importlib.metadata.version('PyLD')}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
