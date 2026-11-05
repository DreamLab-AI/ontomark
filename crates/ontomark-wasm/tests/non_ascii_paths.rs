//! A path that does not end in ASCII `.md` can end mid-character at byte
//! `len - 3`; the engine must ignore it, not panic (`handle_json` contract).

use ontomark_wasm::engine::Engine;
use serde_json::{json, Value};

const PATHS: [&str; 5] = ["éé", "😊", "notes/Mood 😊", "ßé", "日本"];

fn ok(engine: &mut Engine, request: &Value) -> Value {
    let reply: Value = serde_json::from_str(&engine.handle_json(&request.to_string())).unwrap();
    assert_eq!(reply["ok"], true, "{request} -> {reply}");
    reply
}

#[test]
fn load_ignores_non_markdown_paths_ending_in_multibyte_characters() {
    let mut engine = Engine::new();
    let mut files = vec![json!({"path": "Animal.md", "text": "---\ntype: class\n---\n"})];
    files.extend(
        PATHS
            .iter()
            .map(|p| json!({"path": p, "text": "---\ntype: class\n---\n"})),
    );
    ok(
        &mut engine,
        &json!({"kind": "load", "generation": 1, "files": files}),
    );
    let index = ok(&mut engine, &json!({"kind": "index", "generation": 1}));
    assert_eq!(
        index["result"]["entities"].as_array().map(Vec::len),
        Some(1),
        "{index}"
    );
}

#[test]
fn rename_to_a_non_markdown_path_drops_the_note_without_panicking() {
    for (generation, to) in PATHS.iter().enumerate() {
        let generation = generation as u64 * 2 + 1;
        let mut engine = Engine::new();
        let files = [json!({"path": "Mood.md", "text": "---\ntype: class\n---\n"})];
        ok(
            &mut engine,
            &json!({"kind": "load", "generation": generation, "files": files}),
        );
        ok(
            &mut engine,
            &json!({"kind": "rename", "generation": generation + 1, "from": "Mood.md", "to": to}),
        );
        let index = ok(
            &mut engine,
            &json!({"kind": "index", "generation": generation + 1}),
        );
        assert_eq!(
            index["result"]["entities"].as_array().map(Vec::len),
            Some(0),
            "{to}: {index}"
        );
    }
}

/// Seeded xorshift: deterministic, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Characters of every UTF-8 width, plus path and Markdown punctuation.
const ALPHABET: [char; 16] = [
    'a', 'Z', '.', '/', 'm', 'd', 'M', 'D', ' ', 'é', 'ß', '€', '日', '😊', '\u{0301}', '\u{FEFF}',
];

fn random_path(rng: &mut Rng) -> String {
    let mut s: String = (0..rng.below(8))
        .map(|_| ALPHABET[rng.below(ALPHABET.len())])
        .collect();
    if rng.below(3) == 0 {
        s.push_str(".md");
    }
    s
}

#[test]
fn random_paths_and_mangled_requests_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for round in 0..2_000u64 {
        let mut engine = Engine::new();
        let generation = round + 1;
        let files: Vec<Value> = (0..1 + rng.below(4))
            .map(|_| json!({"path": random_path(&mut rng), "text": "---\ntype: class\n---\n"}))
            .collect();
        let requests = [
            json!({"kind": "load", "generation": generation, "files": files}).to_string(),
            json!({"kind": "rename", "generation": generation + 1, "from": random_path(&mut rng), "to": random_path(&mut rng)}).to_string(),
            json!({"kind": "remove", "generation": generation + 2, "path": random_path(&mut rng)}).to_string(),
            json!({"kind": "index", "generation": generation + 2}).to_string(),
        ];
        for raw in requests {
            // Also send each request cut at a random character boundary.
            let cut: String = raw
                .chars()
                .take(rng.below(raw.chars().count() + 1))
                .collect();
            for input in [raw.as_str(), cut.as_str()] {
                let reply = engine.handle_json(input);
                let parsed: Result<Value, _> = serde_json::from_str(&reply);
                assert!(parsed.is_ok(), "round {round}: {input} -> {reply}");
            }
        }
    }
}
