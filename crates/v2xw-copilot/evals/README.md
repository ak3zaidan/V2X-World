# The agent's evaluation suite

`prompts.yaml` is what people ask the simulator agent: 42 prompts across normal traffic,
congestion, each attacker family, jamming, revocation, privacy, radio technology, protocol
overhead, emergency priority, pedestrians, weather, timeline events, multi-variant
experiments, follow-ups, vague questions, and impossible or ambiguous requests. Each case
states the scenario changes and run count the prompt implies and what the answer must say;
the file's header explains the check vocabulary.

`../examples/agent_eval.rs` runs it against a live model and a live engine and scores every
case on four checks — configuration, run completed, numbers grounded (strictly: every number
in the answer must be in the runs' data, not just somewhere in the conversation), and
conclusions — plus time to first feedback, time to the model's first output, total time,
model calls, tokens and cost.

## Running it

```sh
set -a; . ./.env; set +a     # OPENAI_API_KEY / ANTHROPIC_API_KEY, V2XW_AGENT_PROVIDER, OPENAI_MODEL
cargo build --release -p v2xw-server
cargo run -p v2xw-copilot --example agent_eval -- \
    --server-bin target/release/v2xw-server --port 8771 \
    --label after --compare docs/validation/agent-eval-before.json \
    --out docs/validation/agent-eval.md
```

The provider is the one the harness picks itself (`V2XW_AGENT_PROVIDER`, `OPENAI_MODEL`,
`OPENAI_REASONING_EFFORT`, the Anthropic fallback). Keys are read from the environment and
never written anywhere; the reports hold prompts, answers, scores and token counts only.

Options: `--only id,id` and `--category a,b` select cases (a follow-up brings the case it
follows); `--append` keeps the cases a previous invocation wrote to the same file, so the
suite can be run in parts; `--transcripts <dir>` writes each case's conversation (prompts,
tool calls, tool results) for debugging; `--engine <url>` uses an engine already running;
with neither `--engine` nor `--server-bin` an engine starts in-process (a debug build,
slow).

A full run of the suite with `gpt-6.1-sol` costs about a dollar and takes about fifteen
minutes; the "before" run of the harness as received cost about four.

A model is not deterministic: a single failing case is a finding to read (the transcript
says why), not a flaky test to re-run until it passes.

## The harness suite (`harness.yaml`)

`harness.yaml` asks for control rather than analysis: 48 cases that set a published
setting by its plain name or a synonym ("make the radio weaker", "use C-V2X mode 4",
"IDM headway 2 s for trucks"), set several at once, pick a city, have a refusal
explained, drive the run (start, pause, speed, restart, duration), compare runs (A vs B,
"compare with the previous run", a sweep) and answer from measured numbers.

Its offline half runs on every `cargo test -p v2xw-copilot --test harness_eval`, with
no model: a settings search with the person's own words finds the key (`resolve`), a
change written as a model writes it ("2 min", "30%", "C-V2X mode 4", a key without its
first segments) lands on the right key and value and the loader accepts it (`repair`),
and a refused value comes back explained (`refuse`). It was 20/60 on the harness as
received and is 60/60 now.

Its live half is the cases marked `live: true`, run by the same example:

```sh
cargo run -p v2xw-copilot --example agent_eval -- \
    --suite crates/v2xw-copilot/evals/harness.yaml --server-bin <v2xw-server> --port 8752
```

`--all` runs every case. On top of `prompts.yaml`'s checks it scores `draft` (the Settings
window's draft after the turn), `calls` (a tool called with arguments containing these)
and `no_calls`. The draft is put back to the engine's starting scenario before every case
that is not a follow-up.
