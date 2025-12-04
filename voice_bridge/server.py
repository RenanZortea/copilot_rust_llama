import io
import os
import sys
from contextlib import asynccontextmanager

import numpy as np
import soundfile as sf
import torch
import uvicorn
from fastapi import FastAPI, Response
from pydantic import BaseModel

# Add VibeVoice to python path
sys.path.append(os.getcwd())

# Global Model
model = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    global model
    try:
        print("--- LOADING VIBEVOICE REALTIME 0.5B ---")
        from vibevoice import VibeVoice

        if not torch.cuda.is_available():
            print(
                "WARNING: CUDA not found. VibeVoice requires a GPU to run efficiently."
            )
            device = "cpu"
        else:
            device = "cuda"

        print(f"Using device: {device}")

        # Load the official Realtime 0.5B model
        # This will download about 1GB of weights on first run
        model = VibeVoice.from_pretrained("microsoft/VibeVoice-Realtime-0.5B")
        if device == "cuda":
            model = model.cuda()

        print("--- VOICE MODEL LOADED SUCCESSFULLY ---")
    except Exception as e:
        print(f"\nCRITICAL ERROR LOADING MODEL: {e}")
        print("1. Ensure you have an NVIDIA GPU.")
        print("2. Ensure you have internet access to download weights.")
        print("3. Check if flash-attn installed correctly.\n")
    yield


app = FastAPI(lifespan=lifespan)


class TTSRequest(BaseModel):
    text: str
    speaker: str = "Carter"  # Default speaker for 0.5B model


@app.post("/tts")
async def generate_speech(req: TTSRequest):
    global model
    sr = 24000

    if not model:
        print(f"Mocking (Model not loaded): {req.text}")
        audio = np.zeros(sr, dtype=np.float32)
    else:
        print(f"Generating ({req.speaker}): {req.text}")
        try:
            # The generate function for the 0.5B model
            output = model.generate(req.text, speaker_name=req.speaker)

            # Unpack output (Model usually returns (audio, sr))
            if isinstance(output, tuple):
                audio, sr = output
            else:
                audio = output

            # Move to CPU and numpy
            if hasattr(audio, "cpu"):
                audio = audio.cpu().float().numpy()

            # Flatten to 1D array if needed
            if len(audio.shape) > 1:
                audio = audio.flatten()

        except Exception as e:
            print(f"Generation Error: {e}")
            # Fallback to silent audio on error to prevent client crash
            audio = np.zeros(sr, dtype=np.float32)

    # Convert to WAV in memory
    buffer = io.BytesIO()
    sf.write(buffer, audio, sr, format="WAV")
    buffer.seek(0)

    return Response(content=buffer.read(), media_type="audio/wav")


if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=5000)
