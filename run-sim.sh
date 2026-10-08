#!/bin/bash
# Start the simulator: the engine with the agent always on, in one process.
#
#   ./run-sim.sh                                  default scenario
#   ./run-sim.sh scenarios/manhattan-vru.yaml     another scenario
#
# The agent's endpoints and its always-on watcher are part of v2xw-server, so the agent
# panel and the end-of-run analysis work with no second command. Model keys are read from
# .env (V2XW_AGENT_PROVIDER, OPENAI_API_KEY, OPENAI_MODEL, ANTHROPIC_API_KEY) by the
# engine itself; the banner names the provider, never a key.
#
# Then, in a second terminal:  pnpm --dir ui --filter @vwp/studio dev
# and open http://127.0.0.1:5173
cd "$(dirname "$0")" || exit 1
BIN=./target/release/v2xw-server
# Build when missing, or when the binary predates the agent being part of the engine.
if [ ! -x "$BIN" ] || ! "$BIN" --help 2>/dev/null | grep -q -- "--no-agent"; then
  echo "building $BIN (the engine with the agent built in)..." >&2
  cargo build --release -j 1 -p v2xw-cli --bin v2xw-server || exit 1
fi
SCENARIO=${1:-scenarios/manhattan-5min.yaml}
shift 2>/dev/null
PORT=${V2XW_PORT:-8787}
exec "$BIN" --scenario "$SCENARIO" --port "$PORT" --paused --speed 0 --env-file .env "$@"
