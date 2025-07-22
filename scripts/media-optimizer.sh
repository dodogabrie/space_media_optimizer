#!/bin/bash

# Set the quality levels and thresholds
JPEG_QUALITY=80
VIDEO_CRF=26
VIDEO_AUDIO_BITRATE="128k"
SIZE_THRESHOLD=0.9

# Directory to store processing state
STATE_DIR="${HOME}/.media-optimizer"
mkdir -p "$STATE_DIR"

# Function to generate a unique state filename based on the media directory
get_state_file() {
    local media_dir="$1"
    # Use a hash of the full path to create a unique, filename-safe identifier
    local dir_hash=$(echo "$media_dir" | md5 | cut -d' ' -f1)
    echo "${STATE_DIR}/processed_files_${dir_hash}.txt"
}

# Function to get file size in bytes with cross-platform support
get_file_size() {
    stat -f%z "$1" 2>/dev/null || stat -c%s "$1" 2>/dev/null
}

# Function to create a temporary file with cross-platform support
create_temp_file() {
    local suffix="${1:-}"
    mktemp -t media-optimizer${suffix:+-}${suffix} 2>/dev/null
}

# Function to check if a file has been processed before
is_file_processed() {
    local state_file="$1"
    local file_path="$2"
    local file_mtime=$(stat -f%m "$file_path" 2>/dev/null || stat -c%Y "$file_path" 2>/dev/null)
    
    # Check if file is already in the state file
    if grep -q "^${file_path}:${file_mtime}$" "$state_file" 2>/dev/null; then
        return 0  # File has been processed before
    fi
    return 1  # File needs processing
}

# Function to mark a file as processed
mark_file_processed() {
    local state_file="$1"
    local file_path="$2"
    local file_mtime=$(stat -f%m "$file_path" 2>/dev/null || stat -c%Y "$file_path" 2>/dev/null)
    
    echo "${file_path}:${file_mtime}" >> "$state_file"
}

# Function to check and potentially replace file if size reduction is significant
replace_if_reduced() {
    local original_file="$1"
    local temp_file="$2"
    local media_type="$3"
    local state_file="$4"

    # Get file sizes
    local original_size=$(get_file_size "$original_file")
    local compressed_size=$(get_file_size "$temp_file")

    # Calculate size reduction ratio
    if (( $(echo "$compressed_size < $original_size * $SIZE_THRESHOLD" | bc -l) )); then
        # Replace the original file with the compressed version
        mv "$temp_file" "$original_file"
        local reduction_percent=$(printf "%.2f%%" $((100 - compressed_size * 100 / original_size)))
        echo "Optimized $media_type: $original_file ($reduction_percent reduction)"
        
        # Mark the file as processed
        mark_file_processed "$state_file" "$original_file"
        return 0
    else
        # If size reduction is not significant, keep original and remove temp
        rm "$temp_file"
        echo "Skipped $media_type: $original_file (insufficient size reduction)"
        return 1
    fi
}

# Function to optimize images
optimize_images() {
    local media_dir="$1"
    local state_file="$2"

    find "$media_dir" \( -iname "*.jpg" -o -iname "*.jpeg" \) -print0 | while IFS= read -r -d '' original_file; do
        # Skip if file has been processed before
        if is_file_processed "$state_file" "$original_file"; then
            echo "Skipping already processed image: $original_file"
            continue
        fi

        # Create a temporary file for the compressed image
        temp_file=$(create_temp_file)

        # Compress the image using sharp-cli
        if sharp compress \
            --input "$original_file" \
            --output "$temp_file" \
            --quality "$JPEG_QUALITY"; then
            
            # Try to replace if size reduction is significant
            replace_if_reduced "$original_file" "$temp_file" "Image" "$state_file" || true
        else
            # Remove temp file if compression fails
            rm -f "$temp_file"
            echo "Failed to compress image: $original_file"
        fi
    done
}

# Function to optimize videos
optimize_videos() {
    local media_dir="$1"
    local state_file="$2"

    find "$media_dir" -type f -iname "*.mp4" -print0 | while IFS= read -r -d '' original_file; do
        # Skip if file has been processed before
        if is_file_processed "$state_file" "$original_file"; then
            echo "Skipping already processed video: $original_file"
            continue
        fi

        # Create a temporary file for the compressed video
        temp_file=$(create_temp_file .mp4)

        # Compress the video using FFmpeg
        if ffmpeg -i "$original_file" -c:v libx264 -preset veryslow -crf "$VIDEO_CRF" -c:a aac -b:a "$VIDEO_AUDIO_BITRATE" -map_metadata 0 -movflags use_metadata_tags -y "$temp_file"; then
            # Preserve metadata with ExifTool
            if exiftool -tagsFromFile "$original_file" -extractEmbedded -all:all -FileModifyDate -overwrite_original "$temp_file"; then
                # Try to replace if size reduction is significant
                replace_if_reduced "$original_file" "$temp_file" "Video" "$state_file" || true
            else
                # Remove temp file if metadata preservation fails
                rm -f "$temp_file"
                echo "Failed to preserve metadata for video: $original_file"
            fi
        else
            # Remove temp file if compression fails
            rm -f "$temp_file"
            echo "Failed to compress video: $original_file"
        fi
    done
}

# Main script execution
if [ $# -eq 0 ]; then
    echo "Usage: $0 <media_directory>"
    exit 1
fi

media_dir="$1"

# Check if required tools are installed
command -v sharp >/dev/null 2>&1 || { echo >&2 "sharp-cli is required but not installed. Exiting."; exit 1; }
command -v ffmpeg >/dev/null 2>&1 || { echo >&2 "FFmpeg is required but not installed. Exiting."; exit 1; }
command -v exiftool >/dev/null 2>&1 || { echo >&2 "ExifTool is required but not installed. Exiting."; exit 1; }

# Generate a state file specific to this media directory
state_file=$(get_state_file "$media_dir")

# Touch the state file if it doesn't exist
touch "$state_file"

# Run optimizations
echo "Starting media optimization in $media_dir"
optimize_images "$media_dir" "$state_file"
optimize_videos "$media_dir" "$state_file"
echo "Media optimization complete."
