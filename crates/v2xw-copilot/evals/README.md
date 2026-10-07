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
