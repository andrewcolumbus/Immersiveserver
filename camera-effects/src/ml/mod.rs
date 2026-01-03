//! ML inference module
//!
//! Provides person segmentation and hand landmark detection using ONNX Runtime.
//! Uses models from the PINTO Model Zoo compatible with MediaPipe pipelines.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use ndarray::Array4;
use parking_lot::Mutex;

/// Person segmentation result
#[derive(Clone)]
pub struct SegmentationResult {
    /// Segmentation mask (0.0 = background, 1.0 = person)
    pub mask: Vec<f32>,
    /// Mask width
    pub width: u32,
    /// Mask height
    pub height: u32,
}

impl SegmentationResult {
    /// Get mask value at normalized coordinates
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let px = (x * self.width as f32) as u32;
        let py = (y * self.height as f32) as u32;
        let idx = (py.min(self.height - 1) * self.width + px.min(self.width - 1)) as usize;
        self.mask.get(idx).copied().unwrap_or(0.0)
    }
}

/// Hand landmark (21 points per hand, normalized coordinates)
#[derive(Clone, Copy, Debug, Default)]
pub struct HandLandmark {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Detected hand
#[derive(Clone)]
pub struct Hand {
    /// 21 landmarks
    pub landmarks: [HandLandmark; 21],
    /// Confidence score
    pub confidence: f32,
    /// Is right hand
    pub is_right: bool,
}

impl Default for Hand {
    fn default() -> Self {
        Self {
            landmarks: [HandLandmark::default(); 21],
            confidence: 0.0,
            is_right: false,
        }
    }
}

/// ML inference results
#[derive(Clone, Default)]
pub struct MlResult {
    /// Person segmentation mask
    pub segmentation: Option<SegmentationResult>,
    /// Detected hands (up to 2)
    pub hands: Vec<Hand>,
    /// Frame number this result corresponds to
    pub frame_number: u64,
}

/// Frame data to be processed
struct FrameData {
    /// RGBA pixel data
    data: Vec<u8>,
    /// Frame width
    width: u32,
    /// Frame height
    height: u32,
    /// Frame number
    frame_number: u64,
}

/// ML inference engine
pub struct MlInference {
    /// Latest result from inference thread
    latest_result: Arc<Mutex<MlResult>>,
    /// Channel to send frames to inference thread
    frame_sender: Option<Sender<FrameData>>,
    /// Whether inference is running
    running: Arc<AtomicBool>,
    /// Inference thread handle
    thread_handle: Option<std::thread::JoinHandle<()>>,
    /// Whether segmentation is enabled
    pub segmentation_enabled: bool,
    /// Whether hand detection is enabled
    pub hands_enabled: bool,
}

impl MlInference {
    /// Create a new ML inference engine
    pub fn new() -> Result<Self, String> {
        let latest_result = Arc::new(Mutex::new(MlResult::default()));
        let running = Arc::new(AtomicBool::new(false));

        // Create channel for frame communication
        let (frame_sender, frame_receiver) = crossbeam_channel::bounded::<FrameData>(2);

        // Clone for inference thread
        let latest_result_clone = latest_result.clone();
        let running_clone = running.clone();

        // Start inference thread
        let thread_handle = std::thread::Builder::new()
            .name("ml-inference".to_string())
            .spawn(move || {
                Self::inference_thread(frame_receiver, latest_result_clone, running_clone);
            })
            .map_err(|e| format!("Failed to spawn inference thread: {}", e))?;

        Ok(Self {
            latest_result,
            frame_sender: Some(frame_sender),
            running,
            thread_handle: Some(thread_handle),
            segmentation_enabled: true,
            hands_enabled: true,
        })
    }

    /// Inference thread main loop
    fn inference_thread(
        frame_receiver: Receiver<FrameData>,
        latest_result: Arc<Mutex<MlResult>>,
        running: Arc<AtomicBool>,
    ) {
        log::info!("ML inference thread started");

        // Try to initialize ONNX Runtime
        let mut session = match Self::init_ort() {
            Ok(s) => {
                running.store(true, Ordering::Release);
                log::info!("ONNX Runtime initialized successfully");
                Some(s)
            }
            Err(e) => {
                log::warn!("Failed to initialize ONNX Runtime: {}. ML features disabled.", e);
                None
            }
        };

        // Process frames
        while let Ok(frame) = frame_receiver.recv() {
            if let Some(ref mut session) = session {
                match Self::run_inference(session, &frame) {
                    Ok(result) => {
                        *latest_result.lock() = result;
                    }
                    Err(e) => {
                        log::warn!("Inference error: {}", e);
                    }
                }
            }
        }

        running.store(false, Ordering::Release);
        log::info!("ML inference thread stopped");
    }

    /// Initialize ONNX Runtime and load models
    fn init_ort() -> Result<InferenceSession, String> {
        // Find model directory
        let model_dir = Self::find_model_dir()?;
        log::info!("Model directory: {:?}", model_dir);

        // Load segmentation model
        let seg_path = model_dir.join("selfie_segmentation.onnx");
        if !seg_path.exists() {
            return Err(format!("Segmentation model not found: {:?}", seg_path));
        }

        // Initialize ONNX Runtime
        ort::init()
            .with_name("CameraEffects")
            .commit()
            .map_err(|e| format!("Failed to initialize ORT: {}", e))?;

        // Create session with appropriate execution provider
        let session_builder = ort::session::Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?;

        // Load segmentation model
        let seg_session = session_builder
            .clone()
            .with_intra_threads(2)
            .map_err(|e| format!("Failed to set threads: {}", e))?
            .commit_from_file(&seg_path)
            .map_err(|e| format!("Failed to load segmentation model: {}", e))?;

        log::info!("Loaded segmentation model from {:?}", seg_path);

        Ok(InferenceSession {
            segmentation: Some(seg_session),
            hand_detection: None,
            hand_landmark: None,
        })
    }

    /// Find the models directory
    fn find_model_dir() -> Result<PathBuf, String> {
        // Try relative to executable first
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let model_dir = parent.join("models");
                if model_dir.exists() {
                    return Ok(model_dir);
                }
                // Try ../../models (for cargo run from target/release or target/debug)
                if let Some(grandparent) = parent.parent() {
                    let model_dir = grandparent.join("models");
                    if model_dir.exists() {
                        return Ok(model_dir);
                    }
                    // Try ../../../models (for cargo run --release from camera-effects/target/release)
                    if let Some(greatgrandparent) = grandparent.parent() {
                        let model_dir = greatgrandparent.join("models");
                        if model_dir.exists() {
                            return Ok(model_dir);
                        }
                    }
                }
            }
        }

        // Try current directory
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let model_dir = cwd.join("models");
        if model_dir.exists() {
            return Ok(model_dir);
        }

        // Also try camera-effects/models from parent cwd
        let model_dir = cwd.join("camera-effects").join("models");
        if model_dir.exists() {
            return Ok(model_dir);
        }

        Err("Models directory not found. Create a 'models' directory with ONNX models.".to_string())
    }

    /// Run inference on a frame
    fn run_inference(session: &mut InferenceSession, frame: &FrameData) -> Result<MlResult, String> {
        let mut result = MlResult {
            segmentation: None,
            hands: Vec::new(),
            frame_number: frame.frame_number,
        };

        // Run segmentation if available
        if let Some(ref mut seg_session) = session.segmentation {
            result.segmentation = Some(Self::run_segmentation(seg_session, frame)?);
        }

        Ok(result)
    }

    /// Run person segmentation
    fn run_segmentation(
        session: &mut ort::session::Session,
        frame: &FrameData,
    ) -> Result<SegmentationResult, String> {
        const SEG_WIDTH: u32 = 256;
        const SEG_HEIGHT: u32 = 256;

        // Prepare input: resize and convert to RGB float [0, 1]
        let input = Self::preprocess_frame_nhwc(frame, SEG_WIDTH, SEG_HEIGHT);

        // Create input tensor in NHWC format (1, 256, 256, 3) - what this model expects
        let input_array = Array4::from_shape_vec(
            (1, SEG_HEIGHT as usize, SEG_WIDTH as usize, 3),
            input,
        )
        .map_err(|e| format!("Failed to create input array: {}", e))?;

        let input_tensor = ort::value::Tensor::from_array(input_array)
            .map_err(|e| format!("Failed to create tensor: {}", e))?;

        // Run inference
        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Inference failed: {}", e))?;

        // Get first output (segmentation mask)
        let output = outputs
            .iter()
            .next()
            .ok_or("No output from segmentation model")?;

        // Extract tensor data - returns (shape, data slice)
        let (_shape, data) = output
            .1
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output: {}", e))?;

        // Convert to mask
        let mask: Vec<f32> = data
            .iter()
            .map(|&v| v.max(0.0).min(1.0))
            .collect();

        Ok(SegmentationResult {
            mask,
            width: SEG_WIDTH,
            height: SEG_HEIGHT,
        })
    }

    /// Preprocess frame for inference
    fn preprocess_frame(frame: &FrameData, target_width: u32, target_height: u32) -> Vec<f32> {
        let mut output = vec![0.0f32; (target_width * target_height * 3) as usize];

        let x_ratio = frame.width as f32 / target_width as f32;
        let y_ratio = frame.height as f32 / target_height as f32;

        // Resize and convert to CHW format (channels first)
        for y in 0..target_height {
            for x in 0..target_width {
                let src_x = (x as f32 * x_ratio) as u32;
                let src_y = (y as f32 * y_ratio) as u32;
                let src_idx = ((src_y * frame.width + src_x) * 4) as usize;

                if src_idx + 2 < frame.data.len() {
                    let r = frame.data[src_idx] as f32 / 255.0;
                    let g = frame.data[src_idx + 1] as f32 / 255.0;
                    let b = frame.data[src_idx + 2] as f32 / 255.0;

                    let pixel_idx = (y * target_width + x) as usize;
                    let channel_stride = (target_width * target_height) as usize;

                    output[pixel_idx] = r;                        // R channel
                    output[channel_stride + pixel_idx] = g;       // G channel
                    output[2 * channel_stride + pixel_idx] = b;   // B channel
                }
            }
        }

        output
    }

    /// Preprocess frame to NHWC format (height, width, channels) for models that expect it
    fn preprocess_frame_nhwc(frame: &FrameData, target_width: u32, target_height: u32) -> Vec<f32> {
        let mut output = vec![0.0f32; (target_width * target_height * 3) as usize];

        let x_ratio = frame.width as f32 / target_width as f32;
        let y_ratio = frame.height as f32 / target_height as f32;

        // Resize and convert to HWC format (height, width, channels)
        for y in 0..target_height {
            for x in 0..target_width {
                let src_x = (x as f32 * x_ratio) as u32;
                let src_y = (y as f32 * y_ratio) as u32;
                let src_idx = ((src_y * frame.width + src_x) * 4) as usize;

                if src_idx + 2 < frame.data.len() {
                    let r = frame.data[src_idx] as f32 / 255.0;
                    let g = frame.data[src_idx + 1] as f32 / 255.0;
                    let b = frame.data[src_idx + 2] as f32 / 255.0;

                    // HWC format: [y][x][channel]
                    let out_idx = ((y * target_width + x) * 3) as usize;
                    output[out_idx] = r;
                    output[out_idx + 1] = g;
                    output[out_idx + 2] = b;
                }
            }
        }

        output
    }

    /// Send a frame for inference (non-blocking)
    pub fn process_frame(&self, frame: &[u8], width: u32, height: u32, frame_number: u64) {
        if let Some(ref sender) = self.frame_sender {
            // Try to send, but don't block if the channel is full
            let _ = sender.try_send(FrameData {
                data: frame.to_vec(),
                width,
                height,
                frame_number,
            });
        }
    }

    /// Get latest inference result
    pub fn latest_result(&self) -> MlResult {
        self.latest_result.lock().clone()
    }

    /// Check if models are loaded and running
    pub fn is_ready(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Stop the inference thread
    pub fn stop(&mut self) {
        // Drop sender to signal thread to stop
        self.frame_sender = None;

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MlInference {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Default for MlInference {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            log::warn!("Failed to create MlInference: {}", e);
            Self {
                latest_result: Arc::new(Mutex::new(MlResult::default())),
                frame_sender: None,
                running: Arc::new(AtomicBool::new(false)),
                thread_handle: None,
                segmentation_enabled: true,
                hands_enabled: true,
            }
        })
    }
}

/// Holds ONNX Runtime sessions for different models
struct InferenceSession {
    segmentation: Option<ort::session::Session>,
    hand_detection: Option<ort::session::Session>,
    hand_landmark: Option<ort::session::Session>,
}
