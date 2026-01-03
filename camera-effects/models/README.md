# ML Models for Camera Effects

This directory should contain ONNX models for ML-powered effects.

**Note:** The app works without ML models - press `T` to spawn test particles, or the effects will use mouse/keyboard input instead of body tracking.

## Required Models

### Person Segmentation (Optional)
- **File:** `selfie_segmentation.onnx`
- **Input:** RGB image (1, 3, 256, 256) float32
- **Output:** Segmentation mask (1, 1, 256, 256) float32

### Getting Models

Due to Git LFS limitations, models must be downloaded manually.

**Option 1: Convert from TFLite**
```bash
pip install tf2onnx
python -m tf2onnx.convert --tflite selfie_segmenter.tflite --output selfie_segmentation.onnx
```

**Option 2: Use PINTO Model Zoo**
Clone the repo and copy the model:
```bash
git clone https://github.com/PINTO0309/PINTO_model_zoo.git
cp PINTO_model_zoo/109_Selfie_Segmentation/saved_model_256x256/model_float32.onnx models/selfie_segmentation.onnx
```

**Option 3: Train your own**
Use any person segmentation model that outputs a mask, convert to ONNX format.

## ONNX Runtime

Install ONNX Runtime on macOS:
```bash
brew install onnxruntime
```

Run the app with the library path:
```bash
DYLD_LIBRARY_PATH=/opt/homebrew/lib cargo run --release
```

## Model Compatibility

The app expects:
- Input: NCHW format (batch, channels, height, width)
- Input range: 0-1 (normalized RGB)
- Output: Single channel mask, 0-1 range
