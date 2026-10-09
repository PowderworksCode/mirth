#!/bin/bash
# A sequence of flag-transition walks on fixtures/sink that stops at the first new finding.
#
#     rustc/flag-campaign.sh <dir> [--recheck]
#
# <dir> holds the tables and one walk directory per walk; rerunning resumes where it stopped.
# The compiler is <dir>/rustc (a symlink to a toolchain directory). When a walk pauses on a
# finding (its PAUSED file names the row), the campaign exits 3; patch rustc, point
# <dir>/rustc at the patched toolchain, and rerun with --recheck: the rows with findings run
# again first, then everything not yet walked.
#
# Needs PICT (PICT=..., default ~/mirth-work/tools/pict/pict) and flag-universe.py's results
# (FLAGS=..., default ~/mirth-work/flags).
set -u
D=$(cd "$1" && pwd); shift
here=$(cd "$(dirname "$0")" && pwd)
PICT=${PICT:-$HOME/mirth-work/tools/pict/pict}
FLAGS=${FLAGS:-$HOME/mirth-work/flags}
FIXTURE=${FIXTURE:-$here/../fixtures/sink}
cd "$D"

model() { # name subset
  [ -f "$1.txt" ] || python3 "$here/flag-model.py" "$FLAGS" "$2" "$1.txt" --transitions --cargo --allow-known > /dev/null
}
table() { # model strength seed
  t="$1-t$2-r$3.tsv"
  [ -f "$t" ] || "$PICT" "$1.txt" /o:$2 /r:$3 > "$t" 2> /dev/null
  echo "$t"
}
walk() { # table edits seed [flag-walk options]
  local tab=$1 edits=$2 seed=$3; shift 3
  w="walk-${tab%.tsv}-e$edits"
  python3 "$here/flag-walk.py" --rustc "$D/rustc/bin/rustc" --fixture "$FIXTURE" --flags "$FLAGS" \
    --table "$tab" --work "$w" --workers 10 --edits "$edits" --seed "$seed" --pause-on-finding "$@" \
    > "$w.log" 2>&1
  echo "$(date +%T) $w $(tail -1 "$w.log")" | tee -a campaign.log
  [ ! -f "$w/PAUSED" ] || exit 3
}

model all all
model untracked untracked
model tracked tracked
for s in 1 2 3; do
  walk "$(table all 2 $s)" 0 $s "$@"
  walk "$(table all 2 $s)" 2 $s "$@"
done
for s in 1 2; do
  walk "$(table untracked 3 $s)" 2 $s "$@"
done
for s in 1 2 3 4 5 6; do
  walk "$(table all 2 $((s + 10)))" 3 $s "$@"
done
echo "$(date +%T) DONE" | tee -a campaign.log
