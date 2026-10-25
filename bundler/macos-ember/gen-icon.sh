#!/bin/bash

set -e

HELPER_TEXT="Usage: $0 --base-icon <path_to_base_icon.png> --output-dir <output_directory> [--open]

  --base-icon <path>   Path to the base PNG icon file (required)
  --output-dir <path>  Directory where the generated .icns file will be saved (required)
  --open               Open the generated .icns file after creation
  --help, -h           Show this help message and exit
"

SHOULD_OPEN=false
BASE_ICON_PATH=""
OUTPUT_PATH=""
while [[ $# -gt 0 ]]; do
	case "$1" in
		--open)
			SHOULD_OPEN=true
			shift
			;;
		--base-icon)
			BASE_ICON_PATH="$2"
			if [ ! -f "$BASE_ICON_PATH" ]; then
				echo "Base icon file not found: $BASE_ICON_PATH"
				exit 1
			fi
			shift 2
			;;
		--output-dir)
			OUTPUT_PATH="$2"
			if [ ! -d "$OUTPUT_PATH" ]; then
				echo "Output directory not found: $OUTPUT_PATH"
				exit 1
			fi
			shift 2
			;;
		--help|-h)
			echo "$HELPER_TEXT"
			exit 0
			;;
		*)
			echo "Unknown argument: $1"
			echo "$HELPER_TEXT"
			exit 1
			;;
	esac
done

if [ -z "$BASE_ICON_PATH" ]; then
	echo "Error: Base icon path is required."
	echo "$HELPER_TEXT"
	exit 1
elif [ -z "$OUTPUT_PATH" ]; then
	echo "Error: Output directory is required."
	echo "$HELPER_TEXT"
	exit 1
fi

echo -n "Generating icons..."

TMP_WORK_DIR=$(mktemp -d)
trap "rm -rf $TMP_WORK_DIR" EXIT

TMP_ICONSET_DIR=$TMP_WORK_DIR/app.iconset

mkdir -p "$TMP_ICONSET_DIR"
rm -rf "$TMP_ICONSET_DIR/*"

quiet_sips() {
	output=$(sips "$@" 2>&1)
	exit_code=$?
	if [ $exit_code -ne 0 ]; then
		echo "$output"
		return $exit_code
	fi
}

sizes=(16 32 128 256 512)
for size in "${sizes[@]}"; do
	# Standard resolution
	quiet_sips -z $size $size $BASE_ICON_PATH --out "$TMP_ICONSET_DIR/icon_${size}x${size}.png"

	# Retina resolution (@2x)
	double_size=$((size * 2))
	quiet_sips -z $double_size $double_size $BASE_ICON_PATH --out "$TMP_ICONSET_DIR/icon_${size}x${size}@2x.png"
done

OUTPUT_FILE_PATH="$OUTPUT_PATH/app.icns"

iconutil "$TMP_ICONSET_DIR" --convert icns --output "$OUTPUT_FILE_PATH"

$SHOULD_OPEN && open "$OUTPUT_FILE_PATH"

echo " Done"
