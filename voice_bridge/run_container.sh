#!/bin/bash

# Stop any running instance
docker rm -f agerus-voice-bridge 2>/dev/null

echo "Building Docker Image (this will take a while the first time)..."
docker build -t agerus-voice-bridge .

echo "Starting Container..."
# IMPORTANT: --gpus all is required for the model to work
# We mount a volume for huggingface cache so you don't redownload the model every time
docker run -it --rm \
    --gpus all \
    -p 5000:5000 \
    -v agerus_hf_cache:/root/.cache/huggingface \
    --name agerus-voice-bridge \
    agerus-voice-bridge
