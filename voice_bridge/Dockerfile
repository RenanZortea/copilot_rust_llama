# Use a valid PyTorch image with CUDA 12.4 and CuDNN 9 (Devel version for compilation)
FROM pytorch/pytorch:2.4.0-cuda12.4-cudnn9-devel

# Install system audio/video libraries
RUN apt-get update && apt-get install -y \
    git \
    libsndfile1 \
    ffmpeg \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 1. Install Flash Attention (Critical for VibeVoice)
# We do this first because it takes time to compile
RUN pip install --no-cache-dir flash-attn --no-build-isolation

# 2. Clone the OFFICIAL Microsoft VibeVoice repository
RUN git clone https://github.com/microsoft/VibeVoice.git .

# 3. Install the repo in editable mode
RUN pip install --no-cache-dir -e .

# 4. Install additional server requirements
RUN pip install --no-cache-dir fastapi uvicorn requests soundfile

# Copy our server script
COPY server.py .

# Expose the port
EXPOSE 5000

# Run the server
CMD ["python", "server.py"]
