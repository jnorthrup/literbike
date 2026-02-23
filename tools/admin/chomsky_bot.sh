#!/usr/bin/env bash
# Lightweight "chomsky"-style message generator (sanitized).
# Generates avatars (name + email md5) and messages in a neutral academic tone.
# Outputs newline-delimited JSON objects: avatars and messages.

set -euo pipefail

USERS=${1:-5}
MESSAGES=${2:-20}
DAYS=${3:-2}

rand_idx() {
  local -n arr=$1
  echo $((RANDOM % ${#arr[@]}))
}

rand_pick() {
  local -n arr=$1
  echo "${arr[$(rand_idx arr)]}"
}

md5hex() {
  # macOS: md5 -q; fallback to openssl if needed
  if command -v md5 >/dev/null 2>&1; then
    printf "%s" "$1" | md5 -q
  else
    printf "%s" "$1" | openssl dgst -md5 -r | awk '{print $1}'
  fi
}

iso8601() {
  # $1 is epoch seconds
  date -r "$1" -u +"%Y-%m-%dT%H:%M:%SZ"
}

first=(Abigail Addison Adrian Adriana Aiden Alanis Alexander Alexis Allison Anna Anthony Ashley Ava Benjamin Caleb Camila Carlos Carter Charlotte Chloe Daniel David Diego Elijah Emma Ethan Evan Gabriel Isabella Jack Jacob James John Joseph Joshua Julia Liam Logan Madison Mason Matthew Mia Michael Natalie Nathan Nicholas Noah Olivia Owen Ryan Samuel Sarah Sophia Taylor Tyler William)

surn=(Smith Johnson Williams Brown Jones Miller Davis Rodriguez Wilson Martinez Anderson Taylor Thomas Hernandez Moore Martin Jackson Thompson White Lopez Lee Gonzalez Harris Clark Lewis Robinson Walker Perez Hall Young Allen Sanchez Wright King Scott Green Baker Adams Nelson Hill Ramirez Campbell Mitchell Roberts Carter Phillips Evans Turner Torres Parker Collins Edwards Stewart Flores Morris Nguyen Murphy Rivera Cook Rogers Morgan Peterson Cooper Reed Bailey Bell Gomez Kelly Howard Ward Cox Diaz Richardson Wood Watson Brooks Bennett Gray James Reyes Cruz Hughes Price Myers Foster Sanders Ross Morales Powell Sullivan Russell Ortiz Jenkins Gutierrez Perry Butler Barnes Fisher)

# sanitized academic fragments (short, non-copyrighted)
intros=(Thus Moreover Consequently Note Interestingly)
clauses=(this suggests that the proposed analysis a descriptive framework the observed distribution an abstract ordering the base component)
verbs=(delimits is not subject to appears to correlate with may remedy is necessary to impose on)
conclusions=(the general pattern. an abstract underlying order. the set of constraints. a descriptive fact. the traditional practice.)

if [ "$USERS" -lt 1 ]; then
  USERS=1
fi
if [ "$MESSAGES" -lt 0 ]; then
  MESSAGES=0
fi

declare -a AVATARS
declare -a TITLES

start_epoch=$(( $(date +%s) - (24 * 3600 * DAYS) ))
span_seconds=$((24 * 3600 * DAYS))

subject_counter=0

jq_null() { printf 'null'; }

echo "["  # begin JSON array

# Emit avatars as objects first
for ((i=0;i<USERS;i++)); do
  fn=${first[$((RANDOM % ${#first[@]}))]}
  sn=${surn[$((RANDOM % ${#surn[@]}))]}
  name="$fn $sn"
  email="$(echo "${fn}.${sn}@example.com" | tr '[:upper:]' '[:lower:]')"
  md5=$(md5hex "$email")
  AVATARS[$i]=$(printf '%s' "$name")
  # Print avatar JSON (escape name safely)
  esc_name=$(printf '%s' "$name" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().rstrip()))')
  printf '  {"type":"avatar","id":%d,"name":%s,"email":"%s","md5":"%s"},\n' "$i" "$esc_name" "$email" "$md5"
done

# Generate messages
for ((i=0;i<MESSAGES;i++)); do
  # Build message body: 1..3 lines
  lines=$((1 + RANDOM % 3))
  body_parts=()
  for ((l=0;l<lines;l++)); do
    intro=${intros[$((RANDOM % ${#intros[@]}))]}
    clause=${clauses[$((RANDOM % ${#clauses[@]}))]}
    verb=${verbs[$((RANDOM % ${#verbs[@]}))]}
    conclusion=${conclusions[$((RANDOM % ${#conclusions[@]}))]}
    # assemble a short academic-flavored sentence
    body_parts+=("$intro $clause $verb $conclusion")
  done
  body=$(IFS=' '; echo "${body_parts[*]}")

  # choose creator
  creator_index=$((RANDOM % USERS))
  creator_name=${AVATARS[$creator_index]}

  # timestamp evenly spaced across span
  if [ "$MESSAGES" -le 1 ]; then
    ts_epoch=$start_epoch
  else
    ts_epoch=$(( start_epoch + (i * span_seconds) / (MESSAGES - 1) ))
  fi
  ts=$(iso8601 $ts_epoch)

  # reply logic: 30% chance to reply to an earlier message
  reply_to=null
  title=""
  if [ $i -gt 0 ] && [ $((RANDOM % 100)) -lt 30 ]; then
    reply_index=$((RANDOM % i))
    reply_to=$reply_index
    title="${TITLES[$reply_index]}"
  else
    title="subject $((subject_counter++))"
  fi

  TITLES[$i]="$title"

  # Print message JSON
  # escape body, title and creator safely via python json
  esc_body=$(printf '%s' "$body" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().rstrip()))')
  esc_title=$(printf '%s' "$title" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().rstrip()))')
  esc_creator=$(printf '%s' "$creator_name" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().rstrip()))')
  if [ "$i" -lt $((MESSAGES-1)) ]; then
    comma="," 
  else
    comma=""
  fi
  printf '  {"type":"message","id":%d,"creator":%s,"title":%s,"reply_to":%s,"ts":"%s","body":%s}%s\n' \
    "$i" "$esc_creator" "$esc_title" "$([ "$reply_to" = null ] && printf 'null' || printf '%d' "$reply_to")" "$ts" "$esc_body" "$comma"
done

echo "]"

exit 0
