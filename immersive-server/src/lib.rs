//! Immersive Server Library
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

pub mod api;
pub mod app;
pub mod audio;
pub mod compositor;
pub mod converter;
pub mod effects;
pub mod gpu_context;
pub mod layer_runtime;
pub mod network;
pub mod previs;
pub mod preview_player;
pub mod settings;
pub mod shaders;
pub mod telemetry;
pub mod ui;
pub mod video;

pub use app::App;
pub use compositor::{BlendMode, ClipCell, ClipSource, ClipTransition, DEFAULT_CLIP_SLOTS, Environment, Layer, LayerSource, Transform2D, Viewport};
pub use effects::{
    BpmClock, EffectDefinition, EffectInstance, EffectManager, EffectParams, EffectProcessor,
    EffectRegistry, EffectStack, EffectStackRuntime, EffectTarget, GpuEffectRuntime, Parameter,
    ParameterMeta, ParameterValue, ParamBuilder,
};
pub use layer_runtime::LayerRuntime;
pub use preview_player::PreviewPlayer;
pub use settings::{AppPreferences, EnvironmentSettings};
pub use video::{DecodedFrame, LayerParams, VideoDecoder, VideoDecoderError, VideoInfo, VideoParams, VideoPlayer, VideoRenderer, VideoTexture};
pub use network::{DiscoveredSource, OmtFrame, OmtReceiver, OmtSender, SourceDiscovery, SourceType};
pub use audio::{AudioBand, AudioManager, FftData};
pub use previs::{OrbitCamera, PrevisMesh, PrevisRenderer, PrevisSettings, PrevisVertex, SurfaceType};
pub use gpu_context::{GpuContext, WindowGpuContext};

