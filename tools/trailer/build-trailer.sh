#!/bin/bash
# Assemble the Killshot v31 trailer. Runs ON falke64, where the only ffmpeg on
# this network lives (the static build inside the diffusers venv) and where the
# Wan video worker already stores its shots.
#
#   ssh falke64 'bash ~/killshot-trailer/build-trailer.sh'
#
# Inputs, all expected in ~/killshot-trailer:
#   clips/*.mp4     real gameplay, captured by tools/trailer/capture-gameplay.cjs
#   wan/*.mp4       generated cinematic shots, pulled from the local Wan worker
#   ks-[1-6].wav    the voice-over lines (Kokoro on adler40)
#   music-[ab].wav  two 20 s music beds (MMAudio)
#   sfx-*.wav       MMAudio scores for three of the gameplay clips
#   title.png       the title card
#
# Offsets are computed from the real durations of the segments rather than
# written down, so re-cutting a shot cannot silently desynchronise the voice.
set -euo pipefail
# This host is on a comma-decimal locale, where printf "%.3f" rejects "0.6" and
# awk would emit "0,6" into an ffmpeg filter. Every number here is machine-read.
export LC_ALL=C

F=/home/ender/asset-forge/envs/diffusers/lib/python3.12/site-packages/imageio_ffmpeg/binaries/ffmpeg-linux-x86_64-v7.0.2
HERE=/home/ender/killshot-trailer
OUT=$HERE/trailer.mp4
W=1280; H=720; FPS=30
CRF=${CRF:-22}          # 18 looked identical at twice the bytes on this material
TITLE_SECS=4.3

cd "$HERE"
mkdir -p wan seg

# ---------------------------------------------------------------- Wan shots
# The worker runs on this machine but binds its LAN address, not loopback, so
# the shots come from that address. WAN_HOST overrides it.
# The cached copy is keyed by the JOB ID, not just the file name. Caching on the
# name alone silently reuses whatever shot was fetched into that slot last time:
# a run that stood four slots up on one placeholder job left all four slots
# showing that clip, and the film looked deliberate.
fetch() { # id name
  if [ ! -s "wan/$2.mp4" ] || [ "$(cat "wan/$2.id" 2>/dev/null)" != "$1" ]; then
    curl -sf "http://${WAN_HOST:-192.168.178.188}:8189/jobs/$1/video.mp4" -o "wan/$2.mp4"
    echo "$1" > "wan/$2.id"
    echo "fetched wan/$2.mp4 from job $1 ($(stat -c%s "wan/$2.mp4") bytes)"
  else
    echo "wan/$2.mp4 already holds job $1"
  fi
}
fetch "${WAN_OPERATOR:?}"  operator
fetch "${WAN_HARBOR:?}"    harbor
fetch "${WAN_FREIGHT:?}"   freight
fetch "${WAN_SHOTGUN:?}"   shotgun

# Duration is read out of ffmpeg's own banner. `ffmpeg -i` with no output file
# ALWAYS exits 1 ("At least one output file must be specified"), which under
# `set -o pipefail` kills the whole build with no message at all — so the
# failure is swallowed deliberately here.
dur() { { "$F" -i "$1" 2>&1 || true; } | sed -n 's/.*Duration: \([0-9:.]*\),.*/\1/p' \
  | awk -F: '{printf "%.3f", ($1*3600)+($2*60)+$3}'; }

# ------------------------------------------------------------ video segments
# Every source is conformed to one size, rate and pixel format first. Letting
# concat do that implicitly is how a trailer ends up with a half-second of
# stretched Wan footage in the middle of sharp gameplay.
# Re-encoding every segment on every run costs minutes for nothing while the
# cut is being iterated. FORCE=1 rebuilds regardless.
norm() { # index source
  if [ -z "${FORCE:-}" ] && [ -s "seg/$1.mp4" ] && [ "seg/$1.mp4" -nt "$2" ]; then
    echo "seg/$1.mp4 up to date"; return
  fi
  "$F" -v error -y -i "$2" \
    -vf "scale=$W:$H:force_original_aspect_ratio=increase,crop=$W:$H,fps=$FPS,format=yuv420p" \
    -an -c:v libx264 -preset medium -crf "$CRF" "seg/$1.mp4"
}
norm 01 wan/operator.mp4
norm 02 clips/harbor.mp4
norm 03 wan/harbor.mp4
norm 04 clips/smg-yard.mp4
norm 05 wan/freight.mp4
norm 06 clips/yard-sprint.mp4
norm 07 clips/blade.mp4
norm 08 wan/shotgun.mp4
norm 09 clips/breach-12.mp4

# The title card is a still with a slow push-in, so it moves like the rest of
# the cut instead of freezing the film for four seconds.
TITLE_FRAMES=$(awk "BEGIN{printf \"%d\", $TITLE_SECS * $FPS}")
if [ -n "${FORCE:-}" ] || [ ! -s seg/10.mp4 ] || [ ! seg/10.mp4 -nt title.png ]; then
  # A 2x supersample is enough to keep the wordmark's edges clean through the
  # push-in; 3x quadrupled the zoompan cost for no visible gain.
  # zoompan's `d` is frames emitted PER INPUT FRAME. Feeding it a looped input
  # of TITLE_FRAMES frames therefore produced TITLE_FRAMES squared — a 554
  # second title card that concat happily accepted. Give it the single still and
  # cut the output at the frame count instead.
  "$F" -v error -y -loop 1 -framerate $FPS -i title.png \
    -vf "scale=$((W*2)):$((H*2)),zoompan=z='min(zoom+0.00045,1.06)':d=$TITLE_FRAMES:s=${W}x${H}:fps=$FPS,format=yuv420p" \
    -frames:v "$TITLE_FRAMES" -an -c:v libx264 -preset medium -crf "$CRF" seg/10.mp4
else
  echo "seg/10.mp4 up to date"
fi

# Absolute paths: the concat demuxer resolves a relative entry against the
# LIST FILE's own directory, so "seg/01.mp4" listed inside seg/list.txt is
# looked for at seg/seg/01.mp4 and the build dies with "Impossible to open".
for i in 01 02 03 04 05 06 07 08 09 10; do echo "file '$HERE/seg/$i.mp4'"; done > seg/list.txt
"$F" -v error -y -f concat -safe 0 -i seg/list.txt -c copy video.mp4
TOTAL=$(dur video.mp4)

# ------------------------------------------------------------------- offsets
o=0; declare -A AT
for i in 01 02 03 04 05 06 07 08 09 10; do
  AT[$i]=$o
  o=$(awk "BEGIN{printf \"%.3f\", $o + $(dur seg/$i.mp4)}")
done
ms() { awk "BEGIN{printf \"%d\", ($1) * 1000}"; }

# Sound effects sit exactly on the gameplay segment they were composed for.
SFX_SMG=$(ms "${AT[04]}"); SFX_BLADE=$(ms "${AT[07]}"); SFX_BREACH=$(ms "${AT[09]}")
# Voice cues are expressed against the shot each line is about.
VO1=$(ms "0.6")
VO2=$(ms "${AT[03]} + 0.9")
VO3=$(ms "${AT[04]} + 4.5")
VO4=$(ms "${AT[06]} + 1.9")
VO5=$(ms "${AT[09]} + 0.1")
VO6=$(ms "${AT[10]} + 0.2")
echo "total ${TOTAL}s | sfx $SFX_SMG/$SFX_BLADE/$SFX_BREACH | vo $VO1 $VO2 $VO3 $VO4 $VO5 $VO6"

# Emit the voice cues so the caption file is generated from the same numbers the
# mix used. A .vtt written by hand drifts the moment a shot is recut.
{
  echo "{"
  echo "  \"totalSeconds\": $TOTAL,"
  echo "  \"cues\": ["
  first=1
  for i in 1 2 3 4 5 6; do
    eval "start=\$VO$i"
    len=$(dur "ks-$i.wav")
    [ $first -eq 1 ] || echo ","
    first=0
    printf '    {"line": %d, "startSeconds": %.3f, "durationSeconds": %s}' \
      "$i" "$(awk "BEGIN{print $start/1000}")" "$len"
  done
  echo ""
  echo "  ]"
  echo "}"
} > cues.json
echo "wrote cues.json"

# --------------------------------------------------------------------- audio
# The narrator chain: down a little in pitch with the word timing preserved,
# chest from the bass shelf, a short hall tail, then compression.
VOX="asetrate=24000*0.90,aresample=48000,atempo=1.1111,bass=g=5:f=100,\
acompressor=threshold=0.05:ratio=4:attack=5:release=200,\
aecho=0.85:0.9:80|200:0.3|0.18,volume=6.0"
# The voice bus is padded to the full length before it keys the ducking:
# sidechaincompress stops when its key input stops, which would otherwise cut
# the music dead at the last spoken word.
PAD=$(awk "BEGIN{printf \"%.2f\", $TOTAL + 0.3}")

"$F" -v error -y -i video.mp4 \
  -i music-a.wav -i music-b.wav \
  -i sfx-smg.wav -i sfx-blade.wav -i sfx-breach.wav \
  -i ks-1.wav -i ks-2.wav -i ks-3.wav -i ks-4.wav -i ks-5.wav -i ks-6.wav \
  -filter_complex "
[1:a]aformat=sample_rates=48000:channel_layouts=stereo,afade=t=in:st=0:d=1.5[m1];
[2:a]aformat=sample_rates=48000:channel_layouts=stereo[m2];
[m1][m2]acrossfade=d=3:c1=tri:c2=tri,apad,atrim=0:$TOTAL,
  afade=t=out:st=$(awk "BEGIN{printf \"%.2f\", $TOTAL-2.5}"):d=2.5,volume=0.40[music];

[3:a]aformat=sample_rates=48000:channel_layouts=stereo,volume=1.15,adelay=$SFX_SMG|$SFX_SMG[s1];
[4:a]aformat=sample_rates=48000:channel_layouts=stereo,volume=1.15,adelay=$SFX_BLADE|$SFX_BLADE[s2];
[5:a]aformat=sample_rates=48000:channel_layouts=stereo,volume=1.25,adelay=$SFX_BREACH|$SFX_BREACH[s3];
[s1][s2][s3]amix=inputs=3:normalize=0,apad,atrim=0:$TOTAL[sfx];

[music][sfx]amix=inputs=2:normalize=0[bed];

[6:a]$VOX,adelay=$VO1|$VO1[v1];
[7:a]$VOX,adelay=$VO2|$VO2[v2];
[8:a]$VOX,adelay=$VO3|$VO3[v3];
[9:a]$VOX,adelay=$VO4|$VO4[v4];
[10:a]$VOX,adelay=$VO5|$VO5[v5];
[11:a]$VOX,adelay=$VO6|$VO6[v6];
[v1][v2][v3][v4][v5][v6]amix=inputs=6:normalize=0,apad=whole_dur=$PAD[vo];
[vo]asplit=2[vo1][vokey];

[bed][vokey]sidechaincompress=threshold=0.02:ratio=9:attack=20:release=350[ducked];
[ducked][vo1]amix=inputs=2:normalize=0,alimiter=limit=0.95,
  loudnorm=I=-16:TP=-1.5:LRA=11,atrim=0:$TOTAL[aout]" \
  -map 0:v -map "[aout]" -c:v copy -c:a aac -b:a 192k -ar 48000 -movflags +faststart "$OUT"

echo "wrote $OUT ($(stat -c%s "$OUT") bytes, ${TOTAL}s)"
