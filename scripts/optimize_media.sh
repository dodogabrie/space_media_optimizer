#!/bin/bash

QUALITY=80
THRESHOLD=15      # Minimum percentage saved to replace original
LOG_FILE="processed_log.csv"

# Usage check
if [[ -z "$1" ]]; then
  echo "Usage: $0 /path/to/media-directory"
  exit 1
fi

MEDIA_DIR="$1"

# Ensure log exists
touch "$LOG_FILE"

is_already_processed() {
  local file="$1"
  local mod_time="$2"
  grep -Fq "$file,$mod_time" "$LOG_FILE"
}

update_log() {
  local file="$1"
  local mod_time="$2"
  # Remove old entry if exists
  grep -Fv "$file" "$LOG_FILE" > "${LOG_FILE}.tmp"
  echo "$file,$mod_time" >> "${LOG_FILE}.tmp"
  mv "${LOG_FILE}.tmp" "$LOG_FILE"
}

process_media() {
  local media="$1"
  local ext_lower="${media##*.}"
  ext_lower=$(echo "$ext_lower" | tr '[:upper:]' '[:lower:]')

  local mod_time
  mod_time=$(stat -c %Y "$media" 2>/dev/null || stat -f %m "$media")

  if is_already_processed "$media" "$mod_time"; then
    echo "⏩ Skipping (already optimized & unchanged): $media"
    return
  fi

  local tmp_file
  tmp_file=$(mktemp)
  local tmp_out="${tmp_file}.${ext_lower}"

  if [[ "$ext_lower" == "mp4" ]]; then
    echo "🎬 Optimizing video: $media"
    ffmpeg -i "$media" -c:v libx264 -preset veryslow -crf 26 -c:a aac -b:a 128k \
      -map_metadata 0 -movflags use_metadata_tags -y "$tmp_out"

    exiftool -tagsFromFile "$media" -extractEmbedded -all:all -FileModifyDate -overwrite_original "$tmp_out"

  else
    echo "🖼 Processing image: $media"
    sharp -i "$media" quality=$QUALITY withMetadata -o "$tmp_out"
    exiftool -TagsFromFile "$media" -overwrite_original "$tmp_out"
  fi

  # Shared size evaluation
  local original_size new_size size_diff percent_saved
  original_size=$(stat -c %s "$media" 2>/dev/null || stat -f %z "$media")
  new_size=$(stat -c %s "$tmp_out" 2>/dev/null || stat -f %z "$tmp_out")
  size_diff=$((original_size - new_size))
  percent_saved=$((size_diff * 100 / original_size))

  if (( percent_saved >= THRESHOLD )); then
    echo "✅ Saved $percent_saved%, replacing original."
    mv "$tmp_out" "$media"
  else
    echo "❌ Saved only $percent_saved%, skipping replacement."
    rm -f "$tmp_out"
  fi

  # Update log regardless of whether replaced or skipped
  update_log "$media" "$mod_time"

  rm -f "$tmp_file"
}

export -f process_media is_already_processed update_log

# Main loop with dynamic directory
find "$MEDIA_DIR" -type f \( -iname "*.jpg" -o -iname "*.jpeg" -o -iname "*.mp4" \) -print0 | \
while IFS= read -r -d '' media; do
  process_media "$media"
done