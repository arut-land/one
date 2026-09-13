#!/usr/bin/env bash
# Isolated visual review on Hyprland's Lua configuration API (0.55+).
# Never focuses a workspace or injects global keyboard input.
set -euo pipefail
cd "$(dirname "$0")/../.."
for command in hyprctl grim jq gdbus rg; do command -v "$command" >/dev/null; done
: "${WAYLAND_DISPLAY:?A reachable Wayland display is required}"
export CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4
cargo build -q -p arut-linux -p arut-runtime-local
binary="${CARGO_TARGET_DIR:-$PWD/target}/debug/arut-linux"
output=arut-review
app=dev.arut.Review
captures=${ARUT_REVIEW_OUTPUT:-/tmp/claude-1000/-home-raj-Projects-arut/ca942c4c-75e0-42ba-8f7d-899d981bde7f/scratchpad}
step=${ARUT_REVIEW_STEP:-final}
mkdir -p "$captures"
run_dir=$(mktemp -d /tmp/arut-linux-review.XXXXXX)
created=false
close_app() {
  hyprctl eval 'hl.dispatch(hl.dsp.window.close({window="class:dev.arut.Review"}))' >/dev/null
  for _ in {1..100}; do
    if ! hyprctl clients -j | jq -e --arg app "$app" '.[] | select(.class==$app)' >/dev/null; then return; fi
    sleep 0.05
  done
  return 1
}
cleanup() {
  if "$created"; then
    close_app || true
    hyprctl eval 'if arut_review_rule then arut_review_rule:set_enabled(false); arut_review_rule=nil end' >/dev/null || true
    hyprctl output remove "$output" >/dev/null || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM
if hyprctl monitors -j | jq -e --arg output "$output" '.[] | select(.name==$output)' >/dev/null ||
   hyprctl clients -j | jq -e --arg app "$app" '.[] | select(.class==$app)' >/dev/null; then
  echo 'An arut-review output or app already exists; leaving it alone.' >&2
  exit 1
fi
hyprctl output create headless "$output" >/dev/null
created=true
# Rename the new output's already-visible workspace. Switching to a silent
# workspace would leave it hidden; activating it would steal the user's focus.
workspace=$(hyprctl monitors -j | jq -er --arg output "$output" '.[] | select(.name==$output) | .activeWorkspace.id')
hyprctl eval "hl.dispatch(hl.dsp.workspace.rename({workspace=\"$workspace\",name=\"arut-review\"})); arut_review_rule=hl.workspace_rule({workspace=\"name:arut-review\",monitor=\"$output\"})" >/dev/null
capture() {
  # Allow GTK layout and its own reveal duration to settle before a static capture.
  sleep 1
  hyprctl clients -j | jq --arg app "$app" --arg scenario "$1" ' .[] | select(.class==$app) | {scenario:$scenario,size,workspace,monitor} ' >> "$log"
  grim -o "$output" "$captures/linux-review-$step-$1-$size.png"
}
fixture() {
  gdbus call --session --dest "$app" --object-path /dev/arut/Review \
    --method org.gtk.Actions.Activate review "[<'$1'>]" '{}' >/dev/null
  for _ in {1..200}; do
    if rg -q "review $1 ready" "$log"; then return; fi
    sleep 0.05
  done
  cat "$log" >&2
  return 1
}
shortcut() {
  hyprctl eval "hl.dispatch(hl.dsp.send_shortcut({mods=\"CTRL\",key=\"$1\",window=\"class:$app\"}))" >/dev/null
}
for size in 1280x800 1920x1080 800x600; do
  hyprctl eval "hl.monitor({output=\"$output\",mode=\"$size@60\",position=\"auto\",scale=1})" >/dev/null
  data="$run_dir/$size"
  mkdir -p "$data"
  log="$data/app.log"
  printf -v command 'env ARUT_REVIEW=1 ARUT_LINUX_APP_ID=%q XDG_DATA_HOME=%q XDG_STATE_HOME=%q TMPDIR=%q timeout 90s %q > %q 2>&1' \
    "$app" "$data/data" "$data/state" "$data" "$binary" "$log"
  hyprctl eval "hl.exec_cmd([[$command]], {workspace=\"name:arut-review silent\",no_initial_focus=true})" >/dev/null
  for _ in {1..200}; do
    if rg -q 'first frame painted' "$log" 2>/dev/null; then break; fi
    sleep 0.05
  done
  rg -q 'first frame painted' "$log" || { cat "$log" >&2; exit 1; }
  capture cold
  fixture seed
  capture transcript
  fixture one
  capture composer-one
  fixture many
  capture composer-many
  fixture scroll-up
  capture scrolled-up
  shortcut b
  capture collapsed
  shortcut f
  fixture search
  capture search
  fixture clear-search
  fixture error
  capture error
  if [[ "$size" == 800x600 ]]; then
    hyprctl eval "hl.monitor({output=\"$output\",mode=\"640x600@60\",position=\"auto\",scale=1})" >/dev/null
    size=640x600
    capture narrow
    shortcut f
    capture narrow-search
  fi
  close_app
  if rg -n 'CRITICAL|WARNING|panicked' "$log"; then exit 1; fi
  cp "$log" "$captures/linux-review-$step-$size.log"
done
printf 'Captures: %s/linux-review-%s-*.png\nLogs and isolated data: %s\n' "$captures" "$step" "$run_dir"
