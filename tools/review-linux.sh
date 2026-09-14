#!/usr/bin/env bash
# Isolated visual review of the GTK surface at four sizes.
#
# Two backends, chosen by what the machine has. A reviewer already running
# Hyprland 0.55+ gets a headless output on their own session, which is the
# fastest path and never touches their workspaces. Everyone else -- GNOME, KDE,
# a CI runner -- gets a headless Weston of its own. The scenario steps are the
# same either way: fixtures are GTK actions activated over D-Bus, so only
# output creation, capture and key injection differ.
set -euo pipefail
cd "$(dirname "$0")/.."
have() {
  local command
  for command in "$@"; do command -v "$command" >/dev/null || return 1; done
}
have jq gdbus rg || {
  echo 'jq, gdbus and rg are required.' >&2
  exit 1
}
if [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} && -n ${WAYLAND_DISPLAY:-} ]] && have hyprctl grim; then
  backend=hyprland
elif have weston weston-screenshooter; then
  backend=weston
else
  cat >&2 <<'MISSING'
No usable compositor. Install one of:
  weston (with weston-screenshooter) -- the portable path this script implements
  hyprland 0.55+ and grim           -- used automatically inside a Hyprland session
`wlheadless-run` (xwayland-run) and `mutter --headless --virtual-monitor` are the
other two portable compositors; neither is wired up here, because wlheadless-run
ends its compositor with its single client and mutter exposes capture only over
the ScreenCast D-Bus API.
MISSING
  exit 1
fi
export CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 G_DEBUG=fatal-criticals
debugger=
if [[ ${ARUT_REVIEW_GDB:-0} == 1 ]]; then
  command -v gdb >/dev/null
  debugger="gdb -batch -ex run -ex bt --args"
fi
# ECOSYSTEM note 1 forbids setting GTK_THEME: a themed dev machine would capture
# what no default desktop shows. Opt in explicitly to review one deliberately.
theme=${ARUT_REVIEW_GTK_THEME:-}
cargo build -q -p arut-runtime-local
binary="${CARGO_TARGET_DIR:-$PWD/target}/debug/arut-linux"
output=arut-review
app=dev.arut.Review
captures=${ARUT_REVIEW_OUTPUT:-/tmp/arut-linux-review}
step=${ARUT_REVIEW_STEP:-final}
mkdir -p "$captures"
run_dir=$(mktemp -d /tmp/arut-linux-review.XXXXXX)
created=false
compositor=
app_pid=

hyprland_close_app() {
  hyprctl eval 'hl.dispatch(hl.dsp.window.close({window="class:dev.arut.Review"}))' >/dev/null
  for _ in {1..100}; do
    if ! hyprctl clients -j | jq -e --arg app "$app" '.[] | select(.class==$app)' >/dev/null; then return; fi
    sleep 0.05
  done
  return 1
}
weston_close_app() {
  [[ -n $app_pid ]] || return 0
  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
  app_pid=
}
close_app() { "${backend}_close_app"; }

cleanup() {
  if "$created"; then
    close_app || true
    if [[ $backend == hyprland ]]; then
      hyprctl eval 'if arut_review_rule then arut_review_rule:set_enabled(false); arut_review_rule=nil end' >/dev/null || true
      hyprctl output remove "$output" >/dev/null || true
    elif [[ -n $compositor ]]; then
      kill "$compositor" 2>/dev/null || true
      wait "$compositor" 2>/dev/null || true
      compositor=
    fi
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

hyprland_open() {
  if hyprctl monitors -j | jq -e --arg output "$output" '.[] | select(.name==$output)' >/dev/null ||
     hyprctl clients -j | jq -e --arg app "$app" '.[] | select(.class==$app)' >/dev/null; then
    echo 'An arut-review output or app already exists; leaving it alone.' >&2
    exit 1
  fi
  hyprctl output create headless "$output" >/dev/null
  created=true
  # Rename the new output's already-visible workspace. Switching to a silent
  # workspace would leave it hidden; activating it would steal the user's focus.
  local workspace
  workspace=$(hyprctl monitors -j | jq -er --arg output "$output" '.[] | select(.name==$output) | .activeWorkspace.id')
  hyprctl eval "hl.dispatch(hl.dsp.workspace.rename({workspace=\"$workspace\",name=\"arut-review\"})); arut_review_rule=hl.workspace_rule({workspace=\"name:arut-review\",monitor=\"$output\"})" >/dev/null
}
weston_open() { created=true; }
open_backend() { "${backend}_open"; }

# Bring the output to $1. Hyprland retunes the headless output in place; Weston
# has no runtime mode change, so its compositor is restarted per size.
hyprland_size() {
  hyprctl eval "hl.monitor({output=\"$output\",mode=\"$1@60\",position=\"auto\",scale=1})" >/dev/null
}
weston_size() {
  if [[ -n $compositor ]]; then
    kill "$compositor" 2>/dev/null || true
    wait "$compositor" 2>/dev/null || true
  fi
  export WAYLAND_DISPLAY="arut-review-$$"
  local backend_module=headless-backend.so
  weston --help 2>&1 | rg -q 'headless-backend\.so' || backend_module=headless
  weston --backend="$backend_module" --shell=kiosk-shell.so \
    --width="${1%x*}" --height="${1#*x}" --socket="$WAYLAND_DISPLAY" \
    >"$run_dir/weston-$1.log" 2>&1 &
  compositor=$!
  for _ in {1..200}; do
    [[ -S ${XDG_RUNTIME_DIR:?a runtime directory is required}/$WAYLAND_DISPLAY ]] && return
    sleep 0.05
  done
  cat "$run_dir/weston-$1.log" >&2
  return 1
}
resize() { "${backend}_size" "$1"; }

hyprland_launch() {
  hyprctl eval "hl.exec_cmd([[$1]], {workspace=\"name:arut-review silent\",no_initial_focus=true})" >/dev/null
}
weston_launch() {
  setsid bash -c "$1" &
  app_pid=$!
}
launch() { "${backend}_launch" "$1"; }

hyprland_capture() { grim -o "$output" "$1"; }
weston_capture() {
  # weston-screenshooter names its own file in the working directory.
  (cd "$run_dir" && rm -f wayland-screenshot-*.png && weston-screenshooter)
  mv "$(ls -t "$run_dir"/wayland-screenshot-*.png | head -1)" "$1"
}

hyprland_shortcut() {
  hyprctl eval "hl.dispatch(hl.dsp.send_shortcut({mods=\"CTRL\",key=\"$1\",window=\"class:$app\"}))" >/dev/null
}
# Without a compositor that injects keys, the same window action is activated
# over D-Bus. It is the action the accelerator reaches, so the step is the same.
weston_shortcut() {
  case "$1" in
    l | k) fixture focus-composer ;;
    b) fixture sidebar-toggle ;;
    f) fixture search ;;
    *) return 1 ;;
  esac
}
shortcut() { "${backend}_shortcut" "$1"; }

hyprland_type() {
  local key
  for key in h e l l o Return; do
    hyprctl eval "hl.dispatch(hl.dsp.send_shortcut({mods=\"\",key=\"$key\",window=\"class:$app\"}))" >/dev/null
    sleep 0.1
  done
}
weston_type() { fixture typed; }
type_hello() { "${backend}_type"; }

capture() {
  # Allow GTK layout and its own reveal duration to settle before a static capture.
  sleep 1
  if [[ $backend == hyprland ]]; then
    hyprctl clients -j | jq --arg app "$app" --arg scenario "$1" ' .[] | select(.class==$app) | {scenario:$scenario,size,workspace,monitor} ' >> "$log"
  fi
  "${backend}_capture" "$captures/linux-review-$step-$1-$size.png"
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

# Component windows are realized but never mapped onto a desktop workspace.
# The tests run without the review feature, which rewrites the binary, so build
# the fixture binary after them and not before.
GSK_RENDERER=cairo cargo test -q -p arut-linux -- --ignored --test-threads=1
cargo build -q -p arut-linux --features review
open_backend
# Hyprland retunes its headless output in place, so the narrow pass rides along
# inside the 800x600 iteration. Weston restarts per mode, so it gets its own.
sizes="1280x800 1920x1080 800x600"
[[ $backend == hyprland ]] || sizes="$sizes 640x600"
for size in $sizes; do
  resize "$size"
  data="$run_dir/$size"
  mkdir -p "$data"
  log="$data/app.log"
  printf -v command 'env G_DEBUG=fatal-criticals GTK_THEME=%q ARUT_LINUX_APP_ID=%q XDG_DATA_HOME=%q XDG_STATE_HOME=%q TMPDIR=%q timeout 90s %s %q > %q 2>&1' \
    "$theme" "$app" "$data/data" "$data/state" "$data" "$debugger" "$binary" "$log"
  launch "$command"
  for _ in {1..200}; do
    if rg -q 'first frame painted' "$log" 2>/dev/null; then break; fi
    sleep 0.05
  done
  rg -q 'first frame painted' "$log" || { cat "$log" >&2; exit 1; }
  capture cold
  fixture seed
  capture transcript
  shortcut l
  type_hello
  fixture one
  capture composer-one
  fixture send
  fixture switch-away
  sleep 0.2
  fixture switch-back
  sleep 0.2
  fixture groups
  sleep 0.2
  fixture latest
  capture groups
  fixture many
  capture composer-many
  fixture scroll-up
  capture scrolled-up
  shortcut b
  capture collapsed
  fixture sidebar-toggle
  capture expanded-sidebar
  shortcut f
  fixture search
  capture search
  fixture menu
  capture menu
  fixture menu-close
  fixture clear-search
  fixture error
  capture error
  if [[ "$size" == 800x600 && $backend == hyprland ]]; then
    resize 640x600
    size=640x600
    capture narrow
    shortcut f
    capture narrow-search
  fi
  close_app
  if rg -n 'CRITICAL|WARNING|panicked' "$log"; then exit 1; fi
  cp "$log" "$captures/linux-review-$step-$size.log"
done
printf 'Backend: %s\nCaptures: %s/linux-review-%s-*.png\nLogs and isolated data: %s\n' \
  "$backend" "$captures" "$step" "$run_dir"
