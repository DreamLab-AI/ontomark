# Grounding evidence: the figures the READMEs cite

Verified 12 October 2026 against the primary sources listed. **V** = figure read in the source (abstract or full text); **R** = reported by a secondary source only. Estate-internal figures (corpus counts, the Loom benchmark) are in the [README](../README.md) and come from generation `visionGraph@ae913f93`.

| # | Claim | Figure | Source | Status |
| --- | --- | --- | --- | --- |
| 1 | Knowledge-graph grounding beats schema-only for enterprise question answering | GPT-4 zero-shot over the SQL schema: 16% accuracy; over a KG of the same database: 54% | Sequeda, Allemang & Jacob (2023), *A Benchmark to Understand the Role of Knowledge Graphs on LLM's Accuracy for Question Answering on Enterprise SQL Databases*, [arXiv:2311.07509](https://arxiv.org/abs/2311.07509) | V (abstract) |
| 2 | Graph-structured retrieval helps global sensemaking | GraphRAG vs vector RAG, LLM-judged win rates: comprehensiveness 72–83% (Podcast), 72–80% (News); diversity 75–82% and 62–71% | Edge et al. (2024, v2 2025), *From Local to Global: A Graph RAG Approach to Query-Focused Summarization*, [arXiv:2404.16130](https://arxiv.org/abs/2404.16130) | V (full text; the 72–83 reading should be checked against Table 6 before quoting further) |
| 3a | Grounding names against an ontology beats asking a model for identifiers | Correct GO identifier for 98/100 terms (GPT-3.5-turbo) and 97/100 (GPT-4-turbo) with SPIRES; 3/100 with plain prompting. MONDO: 97/100 with SPIRES+GPT-3.5, 18 with GPT-4-turbo, at most 1 without | Caufield et al. (2023/2024), *SPIRES: Structured prompt interrogation and recursive extraction of semantics*, [arXiv:2304.02711](https://arxiv.org/abs/2304.02711); *Bioinformatics* btad757 | V (arXiv v2; journal text not checked) |
| 3b | Zero-shot relation extraction, same paper | BC5CDR F 43.80 (P 0.69, R 0.32), "just below the average of all 18 teams" | Same | V |
| 3c | Ontology-driven KG generation benchmark | 13,474 sentences / 10 ontologies (Wikidata-TekGen); 4,860 / 19 (DBpedia-WebNLG); 7 metrics incl. conformance and hallucination | Mihindukulasooriya et al. (2023), *Text2KGBench*, ISWC 2023, [arXiv:2308.02357](https://arxiv.org/abs/2308.02357) | V (abstract; per-model scores not captured) |
| 4a | Models fabricate references | 55% of GPT-3.5 and 18% of GPT-4 citations fabricated | Walters & Wilder (2023), *Scientific Reports* 13:12856, [doi:10.1038/s41598-023-41032-5](https://doi.org/10.1038/s41598-023-41032-5) | V for the rates; R for the 636/84/42 breakdown |
| 4b | Citation-grounded generation is still unreliable | On ELI5 the best models lack complete citation support 50% of the time | Gao, Yen, Yu & Chen (2023), *Enabling Large Language Models to Generate Text with Citations*, EMNLP 2023, [arXiv:2305.14627](https://arxiv.org/abs/2305.14627) | V (abstract) |
| 5a | Graph-structured memory for retrieval | Up to 20% over SOTA on multi-hop QA; single-step retrieval matches iterative IRCoT at 10–30× lower cost and 6–13× lower latency | Jimenez Gutierrez et al. (2024), *HippoRAG*, NeurIPS 2024, [arXiv:2405.14831](https://arxiv.org/abs/2405.14831) | V (abstract) |
| 5b | Temporal KG as agent memory | DMR 94.8% vs MemGPT 93.4%; LongMemEval up to 18.5% accuracy gain, 90% lower latency (vendor benchmarks) | Rasmussen et al. (2025), *Zep: A Temporal Knowledge Graph Architecture for Agent Memory*, [arXiv:2501.13956](https://arxiv.org/abs/2501.13956) | V (abstract) |
| 6a | EL tractability | Subsumption stays tractable in EL with general concept inclusions; most added constructors make it EXPTIME-complete | Baader, Brandt & Lutz (2005), *Pushing the EL Envelope*, IJCAI 2005, [PDF](https://lat.inf.tu-dresden.de/research/papers/2005/BaaderBrandtLutz-IJCAI-05.pdf) | V (abstract) |
| 6b | EL at scale | SNOMED CT (~300,000 concepts): ELK classifies in as little as 5 s, load and classify under 10 s; CEL and jcel ~10 min, Snorocket ~25 s | Kazakov, Krötzsch & Simančík (2014), *The Incredible ELK*, *J. Automated Reasoning* 53(1), [author PDF](https://www.uni-ulm.de/fileadmin/website_uni_ulm/iui.inst.090/Publikationen/2014/KazKroSim13ELK_JAR.pdf) | V (full text) |

## Not substantiated

- No single well-cited paper was found showing a before/after hallucination reduction from grounding on entity or relation extraction. The nearest evidence is 3a and 4a.
- A circulating "295,428 concepts in 10 seconds" quote is not in the ELK manuscript; use "~300,000 concepts" and "under 10 seconds". Do not write "350k".
- The Agentic RAG survey (2025) was not checked.
- The venues for 6a and 6b are confirmed from knowledge of the papers, not from a venue line in the PDFs.
