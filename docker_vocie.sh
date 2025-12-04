#!/bin/bash
set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Setting up Dockerized Voice Bridge ===${NC}"

mkdir -p voice_bridge
cd voice_bridge

# 1. Create the Dockerfile
# We use a lightweight Python 3.11 base.
# It handles the git cloning and dependency patching internally.
cat >Dockerfile <<'EOF'
FROM python:3.11-slim

# Install system dependencies required for VibeVoice audio processing
RUN apt-get update && apt-get install -y \
    git \
    libsndfile1 \
    ffmpeg \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Clone the Community Fork (Better compatibility)
RUN git clone https://github.com/JarodMica/VibeVoice.git .

# Patch dependencies to remove flash-attn (prevents build failure on generic hardware)
# and relax python version constraints
RUN sed -i 's/==3.11.12/>=3.10/g' pyproject.toml && \
    sed -i '/flash-attn/d' pyproject.toml

# Install Python Dependencies
RUN pip install --no-cache-dir --upgrade pip setuptools wheel && \
    pip install --no-cache-dir "numpy<2" && \
    pip install --no-cache-dir fastapi uvicorn requests soundfile torch huggingface_hub

# Install the repo itself
RUN pip install -e .

# Copy our server script
COPY server.py .

# Expose the port
EXPOSE 5000

# Run the server
CMD ["python", "server.py"]
EOF

# 2. Create the Server Script (Required for the COPY command above)
cat >server.py <<'PYTHON_EOF'
import os
import io
import uvicorn
import soundfile as sf
import torch
import sys
import numpy as np
from fastapi import FastAPI, Response
from pydantic import BaseModel
from contextlib import asynccontextmanager

# Add VibeVoice to path
sys.path.append(os.getcwd())

# Global Model
model = None

@asynccontextmanager
async def lifespan(app: FastAPI):
    global model
    try:
        print("--- LOADING VIBEVOICE MODEL (DOCKER) ---")
        from vibevoice import VibeVoice
        
        # Determine device
        device = "cuda" if torch.cuda.is_available() else "cpu"
        print(f"Using device: {device}")
        
        # Load the Realtime 0.5B model
        model = VibeVoice.from_pretrained("microsoft/VibeVoice-Realtime-0.5B")
        if device == "cuda":
            model = model.cuda()
            
        print("--- VOICE MODEL LOADED SUCCESSFULLY ---")
    except Exception as e:
        print(f"\nCRITICAL ERROR LOADING MODEL: {e}")
        print("Ensure container has internet access to download weights.\n")
    yield

app = FastAPI(lifespan=lifespan)

class TTSRequest(BaseModel):
    text: str

@app.post("/tts")
async def generate_speech(req: TTSRequest):
    global model
    sr = 24000
    
    if not model:
        print(f"Mocking: {req.text}")
        audio = np.zeros(sr, dtype=np.float32)
    else:
        print(f"Generating: {req.text}")
        try:
            output = model.generate(req.text)
            
            if isinstance(output, tuple):
                audio = output[0]
                sr = output[1]
            else:
                audio = output
                
            if hasattr(audio, 'cpu'):
                audio = audio.cpu().float().numpy()
            
            if len(audio.shape) > 1:
                audio = audio.flatten()
                
        except Exception as e:
            print(f"Generation Error: {e}")
            audio = np.zeros(sr, dtype=np.float32)

    buffer = io.BytesIO()
    sf.write(buffer, audio, sr, format='WAV')
    buffer.seek(0)
    
    return Response(content=buffer.read(), media_type="audio/wav")

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=5000)
PYTHON_EOF

# 3. Create a Runner Script
cat >run_container.sh <<'EOF'
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
EOF
chmod +x run_container.sh

echo -e "${GREEN}Docker setup ready.${NC}"
echo -e "Run this to start the voice server:"
echo -e "  ${BLUE}./voice_bridge/run_container.sh${NC}"
