
import serial
import serial.tools.list_ports
import time
import csv
import threading
import numpy as np
from collections import defaultdict

def find_esp32_port():
    ports = serial.tools.list_ports.comports()

    for p in ports:
        name = p.device
        desc = p.description.lower()

        # Heuristics that match ESP32-C6 USB UART chips
        if ("usb" in name.lower() or "com" in name.lower()) and (
            "esp" in desc or "silicon labs" in desc or "cp210" in desc or "ch340" in desc
        ):
            return name

    # Fallback: return first available port
    if ports:
        return ports[0].device

    raise RuntimeError("No serial ports found. Is the ESP32-C6 connected?")


PORT = find_esp32_port()
BAUD = 115200
CSV_FILE = "data.csv"

# Global flag for reader thread
reader_running = True

# ---------------------------
# CSV INITIALIZATION
# ---------------------------
with open(CSV_FILE, "w", newline='') as f:
    writer = csv.writer(f, delimiter=';')
    writer.writerow(["Type", "Value", "Time"])  # header


# ---------------------------
# SERIAL READER THREAD
# ---------------------------
def reader(ser):
    """Reader function that uses the shared serial connection."""
    global reader_running
    print("Reader started.")

    with open(CSV_FILE, "a", newline='') as f:
        writer = csv.writer(f, delimiter=';')

        while reader_running:
            line = ser.readline().decode(errors="ignore").strip()

            if not line:
                continue

            parts = line.split(";")
            if len(parts) != 3:
                continue  # bad format, ignore

            data_type, value, timestamp = parts

            if data_type not in ["Temperature", "Humidity", "LED"]:
                continue  # ignore invalid types

            writer.writerow([data_type, value, timestamp])
            f.flush()  # Ensure data is written immediately
            print("Logged:", parts)

    print("Reader stopped.")


# ---------------------------
# SENDER FUNCTION
# ---------------------------
def send(ser, msg):
    ser.write((msg + "\n").encode())
    print("Sent:", msg)
    time.sleep(1)   # 1 second between messages


# ---------------------------
# DATA ANALYSIS FUNCTIONS
# ---------------------------
def load_data(filename):
    numeric_data = defaultdict(lambda: {"Value": [], "Time": []})
    led_data = {"Time": []}

    with open(filename, newline='', encoding='utf-8') as f:
        reader = csv.reader(f, delimiter=';')
        next(reader)  # Skip header

        for row in reader:
            if len(row) != 3:
                continue

            dtype, value, timestamp = row
            try:
                timestamp = float(timestamp)
            except ValueError:
                continue

            if dtype == "LED":
                led_data["Time"].append(timestamp)
            else:
                try:
                    value = float(value)
                except ValueError:
                    continue
                numeric_data[dtype]["Value"].append(value)
                numeric_data[dtype]["Time"].append(timestamp)

    return numeric_data, led_data


def compute_distribution_params(numeric_data, led_data):
    dist = {}

    for dtype, fields in numeric_data.items():
        values = np.array(fields["Value"])
        times = np.array(fields["Time"])
        if len(values) > 1:
            dist[dtype] = {
                "value_mean": float(values.mean()),
                "value_std": float(values.std(ddof=1)),
                "time_mean": float(times.mean()),
                "time_std": float(times.std(ddof=1))
            }
        else:
            dist[dtype] = "Not enough data for distribution"

    led_times = np.array(led_data["Time"])
    if len(led_times) > 1:
        dist["LED"] = {
            "time_mean": float(led_times.mean()),
            "time_std": float(led_times.std(ddof=1)),
        }
    else:
        dist["LED"] = "Not enough LED data"

    return dist


# ---------------------------
# MAIN PROGRAM
# ---------------------------
if __name__ == "__main__":
    ser = serial.Serial(PORT, BAUD, timeout=1)

    reader_running = True
    reader_thread = threading.Thread(target=reader, args=(ser,))
    reader_thread.start()

    time.sleep(0.2)

    send(ser, "TEMP ON")
    send(ser, "HUM ON")

    for _ in range(30):
        send(ser, "LED ON")
        send(ser, "LED OFF")

    send(ser, "TEMP OFF")
    send(ser, "HUM OFF")

    reader_running = False
    reader_thread.join()

    ser.close()
    print("Data collection done.")

    # ---------------------------
    # ANALYSIS AFTER CSV COMPLETE
    # ---------------------------
    numeric_data, led_data = load_data(CSV_FILE)

    print("\nExtracted Data:")
    for k, v in numeric_data.items():
        print(f"{k}: {len(v['Value'])} entries")
    print(f"LED: {len(led_data['Time'])} entries")

    distribution_params = compute_distribution_params(numeric_data, led_data)
    print("\nNormal Distribution Parameters:")
    for dtype, params in distribution_params.items():
        print(f"\n{dtype}:")
        if isinstance(params, dict):
            for key, val in params.items():
                print(f"  {key}: {val}")
        else:
            print(f"  {params}")

