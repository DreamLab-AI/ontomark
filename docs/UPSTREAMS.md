# Upstream projects and release attribution

Ontomark builds on the work of these upstream developers. Their licences remain in force; the project licence and DreamLab relicensing grant do not replace third-party terms. [Full licence and copyright notices](../THIRD-PARTY-NOTICES.md) are collected for inclusion with portable engine and plugin distributions.

## Whelk lineage

- [Whelk](https://github.com/INCATools/whelk), the Scala OWL EL reasoner created by Jim Balhoff and contributors, is BSD-3-Clause. The former `balhoff/whelk` URL redirects to this repository. The preserved notice is from [`LICENSE.txt`](https://raw.githubusercontent.com/INCATools/whelk/7db20a8da3c8cdf3790e6f08b41bf25f5974d514/LICENSE.txt).
- [whelk-rs](https://github.com/b-gehrke/whelk-rs), the Rust implementation by Björn (`b-gehrke`) and contributors, is MIT. Ontomark derives from the [jjohare fork at `79a1ee21789add5a66faa0ec1a830c32dba9b539`](https://github.com/jjohare/whelk-rs/tree/79a1ee21789add5a66faa0ec1a830c32dba9b539).
- `whelk-el` preserves the Rust MIT notice in `LICENSE-WHELK` and the Scala BSD notice in `LICENSE-WHELK-BSD`. DreamLab changes use MIT OR Apache-2.0. The older published `whelk-el` 0.1.0 package contains the previous incomplete attribution; these corrections do not rewrite that immutable package.

## Portable engine audit scope

This inventory covers the engine archive `ontomark-engine-0.1.0-rc.1.tar.gz`, source `21cd9ffd7b08317248afb4e06b74b650290de5b4`, SHA-256 `f810962b1b71e28653737f2cbbe3a06a5e1a8e6c059e452a2292b6af05b2023a`. It was produced from the locked `ontomark-wasm` dependency graph for `wasm32-unknown-unknown`, including build and procedural-macro dependencies and excluding dev dependencies. The graph has 80 package/version pairs: four Ontomark/Whelk packages and 76 registry packages. It is not an inventory of every native CLI or compiler feature.

The most visible upstream components are:

- [Oxigraph](https://github.com/oxigraph/oxigraph), by Thomas Tanon and contributors: RDF terms, SPARQL parsing, evaluation and datatype support.
- [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) and contributors: the browser/JavaScript boundary and generated glue.
- [Serde](https://github.com/serde-rs/serde), by David Tolnay, Erick Tryzelaar and contributors: serialisation.
- [yaml-rust2](https://github.com/Ethiraric/yaml-rust2), by Chen Yuheng, Ethiraric, David Aguilar and contributors, continuing [yaml-rust](https://github.com/chyh1990/yaml-rust): YAML parsing.
- [im-rs](https://github.com/bodil/im-rs), [bitmaps](https://github.com/bodil/bitmaps) and [sized-chunks](https://github.com/bodil/sized-chunks), by Bodil Stokke and contributors: persistent collections and their backing structures.
- [RustCrypto](https://github.com/RustCrypto), contributors to the SHA/digest and supporting crates; [rand](https://github.com/rust-random/rand) and Rust Random contributors: hashes and browser entropy.
- [similar](https://github.com/mitsuhiko/similar), by Armin Ronacher and contributors: patch diffs.

The three MPL dependencies are used unchanged. Their exact source packages, including licence text, are available from crates.io:

- [bitmaps 2.1.0 source](https://crates.io/api/v1/crates/bitmaps/2.1.0/download), under MPL-2.0 (metadata declares `MPL-2.0+`).
- [im 15.1.0 source](https://crates.io/api/v1/crates/im/15.1.0/download), under MPL-2.0 (metadata declares `MPL-2.0+`).
- [sized-chunks 0.6.5 source](https://crates.io/api/v1/crates/sized-chunks/0.6.5/download), under MPL-2.0 (metadata declares `MPL-2.0+`).

Recipients may obtain and use those source files under the MPL terms reproduced in the notices. No project-specific restriction replaces their rights. Mozilla describes this source-notice requirement in its [MPL distribution FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/#q8).

## Rust dependency inventory

Versions and licence expressions below come from the locked engine metadata. Dual alternatives are retained here; the notices select MIT where available, Apache-2.0 for `similar`, and retain the required MPL, Unicode and Zlib terms. Build/procedural dependencies are conservatively included.

| Package | Version | Declared licence | Repository |
| --- | --- | --- | --- |
| aho-corasick | 1.1.5 | `Unlicense OR MIT` | [Upstream](https://github.com/BurntSushi/aho-corasick) |
| arraydeque | 0.5.1 | `MIT OR Apache-2.0` | [Upstream](https://github.com/andylokandy/arraydeque) |
| bitmaps | 2.1.0 | `MPL-2.0+` | [Upstream](https://github.com/bodil/bitmaps) |
| block-buffer | 0.12.1 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/utils) |
| bumpalo | 3.20.3 | `MIT OR Apache-2.0` | [Upstream](https://github.com/fitzgen/bumpalo) |
| cfg-if | 1.0.5 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/cfg-if) |
| const-oid | 0.10.2 | `Apache-2.0 OR MIT` | [Upstream](https://github.com/RustCrypto/formats) |
| crypto-common | 0.2.2 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/traits) |
| digest | 0.11.3 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/traits) |
| equivalent | 1.0.2 | `Apache-2.0 OR MIT` | [Upstream](https://github.com/indexmap-rs/equivalent) |
| foldhash | 0.2.0 | `Zlib` | [Upstream](https://github.com/orlp/foldhash) |
| futures-core | 0.3.34 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/futures-rs) |
| futures-task | 0.3.34 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/futures-rs) |
| futures-util | 0.3.34 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/futures-rs) |
| getrandom | 0.3.4 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/getrandom) |
| hashbrown | 0.17.1 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/hashbrown) |
| hashlink | 0.12.2 | `MIT OR Apache-2.0` | [Upstream](https://github.com/djc/hashlink) |
| hex | 0.4.3 | `MIT OR Apache-2.0` | [Upstream](https://github.com/KokaKiwi/rust-hex) |
| hybrid-array | 0.4.15 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/hybrid-array) |
| im | 15.1.0 | `MPL-2.0+` | [Upstream](https://github.com/bodil/im-rs) |
| indexmap | 2.14.2 | `Apache-2.0 OR MIT` | [Upstream](https://github.com/indexmap-rs/indexmap) |
| itoa | 1.0.18 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/itoa) |
| js-sys | 0.3.104 | `MIT OR Apache-2.0` | [Upstream](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys) |
| json-event-parser | 0.2.3 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/json-event-parser) |
| md-5 | 0.11.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/hashes) |
| memchr | 2.8.3 | `Unlicense OR MIT` | [Upstream](https://github.com/BurntSushi/memchr) |
| once_cell | 1.21.4 | `MIT OR Apache-2.0` | [Upstream](https://github.com/matklad/once_cell) |
| ontomark-parse | 0.1.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/DreamLab-AI/ontomark) |
| ontomark-reason | 0.1.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/DreamLab-AI/ontomark) |
| ontomark-wasm | 0.1.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/DreamLab-AI/ontomark) |
| oxilangtag | 0.1.6 | `MIT` | [Upstream](https://github.com/oxigraph/oxilangtag) |
| oxiri | 0.2.11 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxiri) |
| oxrdf | 0.3.4 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/oxrdf) |
| oxsdatatypes | 0.2.3 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/oxsdatatypes) |
| peg | 0.8.6 | `MIT` | [Upstream](https://github.com/kevinmehall/rust-peg) |
| peg-macros | 0.8.6 | `MIT` | [Upstream](https://github.com/kevinmehall/rust-peg) |
| peg-runtime | 0.8.6 | `MIT` | [Upstream](https://github.com/kevinmehall/rust-peg) |
| pin-project-lite | 0.2.17 | `Apache-2.0 OR MIT` | [Upstream](https://github.com/taiki-e/pin-project-lite) |
| ppv-lite86 | 0.2.21 | `MIT OR Apache-2.0` | [Upstream](https://github.com/cryptocorrosion/cryptocorrosion) |
| proc-macro2 | 1.0.107 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/proc-macro2) |
| quick-xml | 0.37.5 | `MIT` | [Upstream](https://github.com/tafia/quick-xml) |
| quote | 1.0.47 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/quote) |
| rand | 0.9.5 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/rand) |
| rand_chacha | 0.9.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/rand) |
| rand_core | 0.6.4 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/rand) |
| rand_core | 0.9.5 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/rand) |
| rand_xoshiro | 0.6.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-random/rngs) |
| regex | 1.13.1 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/regex) |
| regex-automata | 0.4.18 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/regex) |
| regex-syntax | 0.8.11 | `MIT OR Apache-2.0` | [Upstream](https://github.com/rust-lang/regex) |
| rustc-hash | 2.1.3 | `Apache-2.0 OR MIT` | [Upstream](https://github.com/rust-lang/rustc-hash) |
| rustversion | 1.0.23 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/rustversion) |
| serde | 1.0.229 | `MIT OR Apache-2.0` | [Upstream](https://github.com/serde-rs/serde) |
| serde_core | 1.0.229 | `MIT OR Apache-2.0` | [Upstream](https://github.com/serde-rs/serde) |
| serde_derive | 1.0.229 | `MIT OR Apache-2.0` | [Upstream](https://github.com/serde-rs/serde) |
| serde_json | 1.0.154 | `MIT OR Apache-2.0` | [Upstream](https://github.com/serde-rs/json) |
| sha1 | 0.11.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/hashes) |
| sha2 | 0.11.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/RustCrypto/hashes) |
| similar | 2.7.0 | `Apache-2.0` | [Upstream](https://github.com/mitsuhiko/similar) |
| sized-chunks | 0.6.5 | `MPL-2.0+` | [Upstream](https://github.com/bodil/sized-chunks) |
| slab | 0.4.12 | `MIT` | [Upstream](https://github.com/tokio-rs/slab) |
| sparesults | 0.3.4 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/sparesults) |
| spareval | 0.2.7 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/spareval) |
| spargebra | 0.4.7 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/spargebra) |
| sparopt | 0.3.7 | `MIT OR Apache-2.0` | [Upstream](https://github.com/oxigraph/oxigraph/tree/main/lib/sparopt) |
| syn | 2.0.119 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/syn) |
| syn | 3.0.8 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/syn) |
| thiserror | 2.0.21 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/thiserror) |
| thiserror-impl | 2.0.21 | `MIT OR Apache-2.0` | [Upstream](https://github.com/dtolnay/thiserror) |
| typenum | 1.20.1 | `MIT OR Apache-2.0` | [Upstream](https://github.com/paholg/typenum) |
| unicode-ident | 1.0.26 | `(MIT OR Apache-2.0) AND Unicode-3.0` | [Upstream](https://github.com/dtolnay/unicode-ident) |
| version_check | 0.9.5 | `MIT OR Apache-2.0` | [Upstream](https://github.com/SergioBenitez/version_check) |
| wasm-bindgen | 0.2.127 | `MIT OR Apache-2.0` | [Upstream](https://github.com/wasm-bindgen/wasm-bindgen) |
| wasm-bindgen-macro | 0.2.127 | `MIT OR Apache-2.0` | [Upstream](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro) |
| wasm-bindgen-macro-support | 0.2.127 | `MIT OR Apache-2.0` | [Upstream](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro-support) |
| wasm-bindgen-shared | 0.2.127 | `MIT OR Apache-2.0` | [Upstream](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared) |
| whelk-el | 0.1.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/DreamLab-AI/ontomark) |
| yaml-rust2 | 0.13.0 | `MIT OR Apache-2.0` | [Upstream](https://github.com/Ethiraric/yaml-rust2) |
| zerocopy | 0.8.62 | `BSD-2-Clause OR Apache-2.0 OR MIT` | [Upstream](https://github.com/google/zerocopy) |
| zmij | 1.0.23 | `MIT` | [Upstream](https://github.com/dtolnay/zmij) |

## JavaScript build and test tools

The standalone plugin at `93b6a05` locks the following direct build/test tools. They are not an installed-user dependency: the release embeds the engine and worker in `main.js`, and leaves Obsidian as a host external. All 100 locked npm package records, including optional binaries for other platforms, are listed below for reproducibility. Full notices for the five direct tools, including TypeScript’s own third-party notices, are preserved in the release notice file.

| Tool | Version | Licence | Repository |
| --- | --- | --- | --- |
| @types/node | 22.20.5 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| esbuild | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| obsidian | 1.13.1 | `MIT` | [Upstream](https://github.com/obsidianmd/obsidian-api) |
| typescript | 6.0.3 | `Apache-2.0` | [Upstream](https://github.com/microsoft/TypeScript) |
| vitest | 5.0.3 | `MIT` | [Upstream](https://github.com/vitest-dev/vitest) |

| Locked npm package | Version | Licence | Repository |
| --- | --- | --- | --- |
| @codemirror/state | 6.5.0 | `MIT` | [Upstream](https://github.com/codemirror/state) |
| @codemirror/view | 6.38.6 | `MIT` | [Upstream](https://github.com/codemirror/view) |
| @esbuild/aix-ppc64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/android-arm | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/android-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/android-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/darwin-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/darwin-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/freebsd-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/freebsd-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-arm | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-ia32 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-loong64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-mips64el | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-ppc64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-riscv64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-s390x | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/linux-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/netbsd-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/netbsd-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/openbsd-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/openbsd-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/openharmony-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/sunos-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/win32-arm64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/win32-ia32 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @esbuild/win32-x64 | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| @jridgewell/resolve-uri | 3.1.2 | `MIT` | [Upstream](https://github.com/jridgewell/resolve-uri) |
| @jridgewell/sourcemap-codec | 1.6.0 | `MIT` | [Upstream](https://github.com/jridgewell/sourcemaps) |
| @jridgewell/trace-mapping | 0.3.31 | `MIT` | [Upstream](https://github.com/jridgewell/sourcemaps) |
| @marijn/find-cluster-break | 1.0.4 | `MIT` | [Upstream](https://code.haverbeke.berlin/marijn/find-cluster-break) |
| @oxc-project/types | 0.153.0 | `MIT` | [Upstream](https://github.com/oxc-project/oxc) |
| @rolldown/binding-android-arm-eabi | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-android-arm64 | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-darwin-arm64 | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-darwin-x64 | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-freebsd-x64 | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-arm-gnueabihf | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-arm64-gnu | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-arm64-musl | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-ppc64-gnu | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-s390x-gnu | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-x64-gnu | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-linux-x64-musl | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-openharmony-arm64 | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-win32-arm64-msvc | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/binding-win32-x64-msvc | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| @rolldown/pluginutils | 1.0.1 | `MIT` | [Upstream](https://github.com/rolldown/plugins) |
| @types/chai | 5.2.3 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @types/codemirror | 5.60.8 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @types/deep-eql | 4.0.2 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @types/estree | 1.0.9 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @types/node | 22.20.5 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @types/tern | 0.23.9 | `MIT` | [Upstream](https://github.com/DefinitelyTyped/DefinitelyTyped) |
| @vitest/mocker | 5.0.3 | `MIT` | [Upstream](https://github.com/vitest-dev/vitest) |
| @vitest/spy | 5.0.3 | `MIT` | [Upstream](https://github.com/vitest-dev/vitest) |
| assertion-error | 2.0.1 | `MIT` | [Upstream](git@github.com:chaijs/assertion-error) |
| chai | 6.3.0 | `MIT` | [Upstream](https://github.com/chaijs/chai) |
| crelt | 1.0.7 | `MIT` | [Upstream](https://code.haverbeke.berlin/marijn/crelt) |
| detect-libc | 2.1.2 | `Apache-2.0` | [Upstream](git://github.com/lovell/detect-libc) |
| es-module-lexer | 2.3.2 | `MIT` | [Upstream](https://github.com/guybedford/es-module-lexer) |
| esbuild | 0.28.2 | `MIT` | [Upstream](https://github.com/evanw/esbuild) |
| estree-walker | 3.0.3 | `MIT` | [Upstream](https://github.com/Rich-Harris/estree-walker) |
| expect-type | 1.4.0 | `Apache-2.0` | [Upstream](https://github.com/mmkal/expect-type) |
| fdir | 6.5.0 | `MIT` | [Upstream](https://github.com/thecodrr/fdir) |
| fsevents | 2.3.3 | `MIT` | [Upstream](https://github.com/fsevents/fsevents) |
| lightningcss | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-android-arm64 | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-darwin-arm64 | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-darwin-x64 | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-freebsd-x64 | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-linux-arm-gnueabihf | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-linux-arm64-gnu | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-linux-arm64-musl | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-linux-x64-gnu | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-linux-x64-musl | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-win32-arm64-msvc | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| lightningcss-win32-x64-msvc | 1.33.0 | `MPL-2.0` | [Upstream](https://github.com/parcel-bundler/lightningcss) |
| magic-string | 1.4.3 | `MIT` | [Upstream](https://github.com/Rich-Harris/magic-string) |
| moment | 2.29.4 | `MIT` | [Upstream](https://github.com/moment/moment) |
| nanoid | 3.3.20 | `MIT` | [Upstream](ai/nanoid) |
| obsidian | 1.13.1 | `MIT` | [Upstream](https://github.com/obsidianmd/obsidian-api) |
| obug | 2.2.1 | `MIT` | [Upstream](https://github.com/sxzz/obug) |
| picocolors | 1.1.1 | `ISC` | [Upstream](alexeyraspopov/picocolors) |
| picomatch | 4.0.7 | `MIT` | [Upstream](micromatch/picomatch) |
| postcss | 8.5.29 | `MIT` | [Upstream](postcss/postcss) |
| rolldown | 1.2.13 | `MIT` | [Upstream](https://github.com/rolldown/rolldown) |
| source-map-js | 1.2.2 | `BSD-3-Clause` | [Upstream](7rulnik/source-map-js) |
| std-env | 4.3.0 | `MIT` | [Upstream](unjs/std-env) |
| style-mod | 4.1.4 | `MIT` | [Upstream](https://code.haverbeke.berlin/marijn/style-mod) |
| tinybench | 6.2.2 | `MIT` | [Upstream](tinylibs/tinybench) |
| tinyexec | 1.3.1 | `MIT` | [Upstream](https://github.com/tinylibs/tinyexec) |
| tinyglobby | 0.2.17 | `MIT` | [Upstream](https://github.com/SuperchupuDev/tinyglobby) |
| typescript | 6.0.3 | `Apache-2.0` | [Upstream](https://github.com/microsoft/TypeScript) |
| undici-types | 6.21.0 | `MIT` | [Upstream](https://github.com/nodejs/undici) |
| vite | 8.3.4 | `MIT` | [Upstream](https://github.com/vitejs/vite) |
| vitest | 5.0.3 | `MIT` | [Upstream](https://github.com/vitest-dev/vitest) |
| w3c-keyname | 2.2.8 | `MIT` | [Upstream](https://github.com/marijnh/w3c-keyname) |
| why-is-node-running | 3.2.1 | `MIT` | [Upstream](https://github.com/mafintosh/why-is-node-running) |

## Toolchain and engine build credits

The bundled engine was built with Rust 1.99.0 (`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`), [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) 0.2.127 and [Binaryen/wasm-opt](https://github.com/WebAssembly/binaryen) 132. Plugin builds use Node 22 (local verification: 22.23.3), esbuild 0.28.2 and the TypeScript/test tools listed above. These engineering tools are acknowledged independently of the embedded runtime.

The engine links the Rust standard library and compiler-builtins runtime; they are absent from the application Cargo graph. Rust source is generally MIT OR Apache-2.0, with component-specific exceptions. `compiler_builtins` 0.1.160 in this toolchain declares `MIT AND Apache-2.0 WITH LLVM-exception AND (MIT OR Apache-2.0)`. The full inherited compiler-runtime notice and the official standard-library copyright collection, conservatively including other targets/build dependencies, are preserved in THIRD-PARTY-NOTICES.md. See [Rust’s release copyright explanation](https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/COPYRIGHT) and [source](https://github.com/rust-lang/rust/tree/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4).

The separately released Linux CLI was built with Rust 1.96.0. That toolchain credit does not expand this engine audit to the native CLI dependency closure.

## Verification and maintenance

Audit date: 2026-10-11. `cargo-about` 0.9.2 ran against the clean engine snapshot with `--locked --target wasm32-unknown-unknown --fail`, with dev dependencies ignored. Its 80 package/version pairs were checked against an independent traversal of Cargo’s normal/build graph. Six Oxigraph crate archives omit licence files: their actual MIT notice was retrieved from the source commit recorded in `.cargo_vcs_info.json`, `0e81a29d27575ab69e7f85ea9c7f1058b277fcd0`, rather than using a generic MIT placeholder. Whelk lineage was checked against the original repository notices; it is not represented as an ordinary registry dependency.

Engine Cargo metadata SHA-256: `61bb1ad9c44b3e78e3bce6999759a563b507bf1a1bad794da490b5b79a21a82b`. Plugin package-lock SHA-256: `ea378440c867628b2f875bb655c8b1807d94b1853013b6061b342a35af1a1ea7`.

When the engine source, target, features or npm lockfile changes, regenerate and review this inventory and the full notices, then include the notice file in the plugin ZIP and release assets. New shared graph packages or native feature closures need their own dependency review; this inventory does not silently cover them.
