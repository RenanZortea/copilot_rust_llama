#!/bin/bash
echo "Building Docker Image (agerus-voice-bridge)..."
docker build -t agerus-voice-bridge .

echo "Starting Container..."
# --gpus all: Pass GPU if available (requires nvidia-container-toolkit)
# -p 5000:5000: Map port
# -v hug_cache:/root/.cache/huggingface: Cache weights so we don't redownload every time
if command -v nvidia-smi &> /dev/null; then
    docker run -it --rm --gpus all -p 5000:5000 -v agerus_hf_cache:/root/.cache/huggingface agerus-voice-bridge
else
    echo "No NVIDIA GPU detected (or nvidia-smi missing). Running in CPU mode."
    docker run -it --rm -p 5000:5000 -v agerus_hf_cache:/root/.cache/huggingface agerus-voice-bridge
fi
