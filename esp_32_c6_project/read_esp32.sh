#!/bin/bash

# Configuration
DEVICE="/dev/ttyUSB0"  # Common ESP32 USB-UART device, adjust if needed
BAUD_RATE=115200
OUTPUT_FILE="data.csv"

# Check if a different device was provided as argument
if [ -n "$1" ]; then
    DEVICE="$1"
fi

# Check if device exists
if [ ! -e "$DEVICE" ]; then
    echo "Error: Device $DEVICE not found."
    echo "Available serial devices:"
    ls /dev/ttyUSB* /dev/ttyACM* 2>/dev/null || echo "No USB serial devices found"
    exit 1
fi

# Create CSV file with header
echo "Type;Value;Time" > "$OUTPUT_FILE"
echo "Created $OUTPUT_FILE with header"

# Configure serial port
stty -F "$DEVICE" "$BAUD_RATE" cs8 -cstopb -parenb raw -echo

echo "Listening on $DEVICE at $BAUD_RATE baud..."
echo "Press Ctrl+C to stop"

# Read from serial and append to CSV
cat "$DEVICE" | while IFS= read -r line; do
    # Remove carriage return if present and trim whitespace
    clean_line=$(echo "$line" | tr -d '\r' | xargs)
    
    # Only write non-empty lines
    if [ -n "$clean_line" ]; then
        echo "$clean_line" >> "$OUTPUT_FILE"
        echo "Received: $clean_line"
    fi
done
