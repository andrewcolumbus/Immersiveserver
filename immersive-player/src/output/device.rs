//! Output device abstraction for different output types
//!
//! Supports Virtual (windowed), Fullscreen, Aqueduct streaming, and NDI outputs.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Output device type for a screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputDevice {
    /// Windowed output - draggable window on any display
    Virtual {
        width: u32,
        height: u32,
        #[serde(skip)]
        window_id: Option<u64>,
    },
    /// Fullscreen on a connected display
    Fullscreen {
        display_id: u32,
        display_name: String,
    },
    /// Stream via Aqueduct protocol
    Aqueduct {
        port: u16,
        name: String,
        #[serde(skip)]
        connected: bool,
    },
    /// NDI output (coming soon)
    Ndi {
        name: String,
        #[serde(skip)]
        enabled: bool,
    },
}

impl Default for OutputDevice {
    fn default() -> Self {
        Self::Virtual {
            width: 800,
            height: 600,
            window_id: None,
        }
    }
}

impl fmt::Display for OutputDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputDevice::Virtual { width, height, .. } => {
                write!(f, "Virtual Output ({}×{})", width, height)
            }
            OutputDevice::Fullscreen { display_name, .. } => {
                write!(f, "Fullscreen ({})", display_name)
            }
            OutputDevice::Aqueduct { name, port, .. } => {
                write!(f, "Aqueduct: {} (port {})", name, port)
            }
            OutputDevice::Ndi { name, .. } => {
                write!(f, "NDI: {}", name)
            }
        }
    }
}

impl OutputDevice {
    /// Get a short type name for the device
    pub fn type_name(&self) -> &'static str {
        match self {
            OutputDevice::Virtual { .. } => "Virtual Output",
            OutputDevice::Fullscreen { .. } => "Fullscreen",
            OutputDevice::Aqueduct { .. } => "Aqueduct",
            OutputDevice::Ndi { .. } => "NDI",
        }
    }

    /// Check if this device type is available (NDI is coming soon)
    pub fn is_available(&self) -> bool {
        !matches!(self, OutputDevice::Ndi { .. })
    }

    /// Create a new virtual (windowed) output
    pub fn new_virtual(width: u32, height: u32) -> Self {
        Self::Virtual {
            width,
            height,
            window_id: None,
        }
    }

    /// Create a new fullscreen output
    pub fn new_fullscreen(display_id: u32, display_name: String) -> Self {
        Self::Fullscreen {
            display_id,
            display_name,
        }
    }

    /// Create a new Aqueduct streaming output
    pub fn new_aqueduct(name: String, port: u16) -> Self {
        Self::Aqueduct {
            port,
            name,
            connected: false,
        }
    }

    /// Create a new NDI output (coming soon)
    pub fn new_ndi(name: String) -> Self {
        Self::Ndi {
            name,
            enabled: false,
        }
    }

    /// Get the resolution for this output device
    pub fn resolution(&self) -> Option<(u32, u32)> {
        match self {
            OutputDevice::Virtual { width, height, .. } => Some((*width, *height)),
            OutputDevice::Fullscreen { .. } => None, // Resolution comes from display
            OutputDevice::Aqueduct { .. } => None,   // Uses screen resolution
            OutputDevice::Ndi { .. } => None,        // Uses screen resolution
        }
    }

    /// Set the resolution for virtual outputs
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        if let OutputDevice::Virtual { width: w, height: h, .. } = self {
            *w = width;
            *h = height;
        }
    }
}

/// Device type selector for UI dropdowns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Virtual,
    Fullscreen,
    Aqueduct,
    Ndi,
}

impl DeviceType {
    /// All available device types for iteration
    pub const ALL: [DeviceType; 4] = [
        DeviceType::Virtual,
        DeviceType::Fullscreen,
        DeviceType::Aqueduct,
        DeviceType::Ndi,
    ];

    /// Get the display name for this device type
    pub fn display_name(&self) -> &'static str {
        match self {
            DeviceType::Virtual => "Virtual Output",
            DeviceType::Fullscreen => "Fullscreen",
            DeviceType::Aqueduct => "Aqueduct",
            DeviceType::Ndi => "NDI (Coming Soon)",
        }
    }

    /// Check if this device type is currently available
    pub fn is_available(&self) -> bool {
        !matches!(self, DeviceType::Ndi)
    }
}

impl From<&OutputDevice> for DeviceType {
    fn from(device: &OutputDevice) -> Self {
        match device {
            OutputDevice::Virtual { .. } => DeviceType::Virtual,
            OutputDevice::Fullscreen { .. } => DeviceType::Fullscreen,
            OutputDevice::Aqueduct { .. } => DeviceType::Aqueduct,
            OutputDevice::Ndi { .. } => DeviceType::Ndi,
        }
    }
}

/// Information about a connected physical display
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Unique display identifier
    pub id: u32,
    /// Human-readable display name
    pub name: String,
    /// Native resolution
    pub resolution: (u32, u32),
    /// Position on virtual desktop (x, y)
    pub position: (i32, i32),
    /// Scale factor (e.g., 2.0 for Retina)
    pub scale_factor: f64,
    /// Whether this is the primary display
    pub is_primary: bool,
}

impl DisplayInfo {
}

/// Output device manager for handling multiple output windows/streams
pub struct DeviceManager {
    /// Available displays on the system
    pub displays: Vec<DisplayInfo>,
    /// Active output windows (indexed by screen ID)
    pub active_outputs: std::collections::HashMap<u32, ActiveOutput>,
}

/// An active output that is currently rendering
pub struct ActiveOutput {
    pub screen_id: u32,
    pub device_type: DeviceType,
    // Window handle or stream sender would go here
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            displays: Vec::new(),
            active_outputs: std::collections::HashMap::new(),
        }
    }

    /// Refresh the list of available displays
    pub fn refresh_displays(&mut self) {
        self.displays = enumerate_displays();
        log::info!("Found {} displays", self.displays.len());
    }

    /// Get a display by ID
    pub fn get_display(&self, id: u32) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.id == id)
    }

    /// Get the primary display
    pub fn primary_display(&self) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.is_primary)
    }
}

/// Enumerate all connected displays on the system
pub fn enumerate_displays() -> Vec<DisplayInfo> {
    #[cfg(target_os = "macos")]
    let displays = enumerate_displays_macos();

    #[cfg(target_os = "windows")]
    let displays = enumerate_displays_windows();

    #[cfg(target_os = "linux")]
    let displays = enumerate_displays_linux();

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let displays = vec![DisplayInfo {
        id: 0,
        name: "Default Display".to_string(),
        resolution: (1920, 1080),
        position: (0, 0),
        scale_factor: 1.0,
        is_primary: true,
    }];

    // Fallback: create a single default display if none found
    if displays.is_empty() {
        vec![DisplayInfo {
            id: 0,
            name: "Primary Display".to_string(),
            resolution: (1920, 1080),
            position: (0, 0),
            scale_factor: 1.0,
            is_primary: true,
        }]
    } else {
        displays
    }
}

#[cfg(target_os = "macos")]
fn enumerate_displays_macos() -> Vec<DisplayInfo> {
    // Use CoreGraphics FFI for reliable display enumeration
    // This links to the CoreGraphics framework which is always available on macOS
    
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGGetActiveDisplayList(max_displays: u32, displays: *mut u32, display_count: *mut u32) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
        fn CGDisplayPixelsWide(display: u32) -> usize;
        fn CGDisplayPixelsHigh(display: u32) -> usize;
        fn CGMainDisplayID() -> u32;
        fn CGDisplayScreenSize(display: u32) -> CGSize;
    }
    
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    
    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    
    let mut displays = Vec::new();
    
    unsafe {
        // Get all active displays (max 16)
        let mut display_ids: [u32; 16] = [0; 16];
        let mut display_count: u32 = 0;
        
        let result = CGGetActiveDisplayList(16, display_ids.as_mut_ptr(), &mut display_count);
        
        if result == 0 && display_count > 0 {
            let main_display = CGMainDisplayID();
            
            for i in 0..display_count as usize {
                let display_id = display_ids[i];
                let bounds = CGDisplayBounds(display_id);
                let width = CGDisplayPixelsWide(display_id) as u32;
                let height = CGDisplayPixelsHigh(display_id) as u32;
                
                // Get physical size to estimate scale factor
                let screen_size = CGDisplayScreenSize(display_id);
                let scale_factor = if screen_size.width > 0.0 {
                    // Estimate based on physical size vs pixel size
                    // Built-in Retina displays are typically 2x
                    if width > 2000 && screen_size.width < 400.0 {
                        2.0
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                
                let is_primary = display_id == main_display;
                let name = if is_primary {
                    "Main Display".to_string()
                } else {
                    format!("Display {}", i + 1)
                };
                
                displays.push(DisplayInfo {
                    id: display_id,
                    name,
                    resolution: (width, height),
                    position: (bounds.origin.x as i32, bounds.origin.y as i32),
                    scale_factor,
                    is_primary,
                });
            }
        }
    }
    
    // Fallback if CoreGraphics didn't return any displays
    if displays.is_empty() {
        log::warn!("CoreGraphics returned no displays, using fallback");
        displays.push(DisplayInfo {
            id: 0,
            name: "Primary Display".to_string(),
            resolution: (1920, 1080),
            position: (0, 0),
            scale_factor: 2.0,
            is_primary: true,
        });
    }
    
    // Sort so primary display is first
    displays.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));
    
    log::info!("Enumerated {} macOS displays via CoreGraphics", displays.len());
    for d in &displays {
        log::info!("  Display {}: {} ({}x{}) at ({}, {})", 
            d.id, d.name, d.resolution.0, d.resolution.1, d.position.0, d.position.1);
    }
    
    displays
}

#[cfg(target_os = "windows")]
fn enumerate_displays_windows() -> Vec<DisplayInfo> {
    use std::process::Command;
    
    let mut displays = Vec::new();
    
    // Use PowerShell to get monitor information with positions
    // This script gets actual monitor geometry from Win32_VideoController and display settings
    let ps_script = r#"
        Add-Type -TypeDefinition @'
        using System;
        using System.Runtime.InteropServices;
        public class MonitorInfo {
            [DllImport("user32.dll")]
            public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip, MonitorEnumDelegate lpfnEnum, IntPtr dwData);
            
            [DllImport("user32.dll")]
            public static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFOEX lpmi);
            
            public delegate bool MonitorEnumDelegate(IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData);
            
            [StructLayout(LayoutKind.Sequential)]
            public struct RECT { public int Left, Top, Right, Bottom; }
            
            [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
            public struct MONITORINFOEX {
                public int cbSize;
                public RECT rcMonitor;
                public RECT rcWork;
                public uint dwFlags;
                [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
                public string szDevice;
            }
        }
'@
        $monitors = @()
        $callback = {
            param($hMonitor, $hdcMonitor, [ref]$lprcMonitor, $dwData)
            $mi = New-Object MonitorInfo+MONITORINFOEX
            $mi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($mi)
            if ([MonitorInfo]::GetMonitorInfo($hMonitor, [ref]$mi)) {
                $monitors += [PSCustomObject]@{
                    Left = $mi.rcMonitor.Left
                    Top = $mi.rcMonitor.Top
                    Width = $mi.rcMonitor.Right - $mi.rcMonitor.Left
                    Height = $mi.rcMonitor.Bottom - $mi.rcMonitor.Top
                    Primary = ($mi.dwFlags -band 1) -eq 1
                    Device = $mi.szDevice
                }
            }
            return $true
        }
        [MonitorInfo]::EnumDisplayMonitors([IntPtr]::Zero, [IntPtr]::Zero, $callback, [IntPtr]::Zero)
        $monitors | ConvertTo-Json
    "#;
    
    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output()
    {
        if output.status.success() {
            if let Ok(json_str) = String::from_utf8(output.stdout) {
                // Parse JSON output - handle both single object and array
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let monitors = if json.is_array() {
                        json.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![json]
                    };
                    
                    for (i, mon) in monitors.iter().enumerate() {
                        let left = mon.get("Left").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let top = mon.get("Top").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let width = mon.get("Width").and_then(|v| v.as_i64()).unwrap_or(1920) as u32;
                        let height = mon.get("Height").and_then(|v| v.as_i64()).unwrap_or(1080) as u32;
                        let is_primary = mon.get("Primary").and_then(|v| v.as_bool()).unwrap_or(i == 0);
                        let device = mon.get("Device").and_then(|v| v.as_str()).unwrap_or("");
                        
                        let name = if is_primary {
                            "Primary Display".to_string()
                        } else {
                            format!("Display {} ({})", i + 1, device)
                        };
                        
                        displays.push(DisplayInfo {
                            id: i as u32,
                            name,
                            resolution: (width, height),
                            position: (left, top),
                            scale_factor: 1.0, // Would need DPI info for accurate scale
                            is_primary,
                        });
                    }
                }
            }
        }
    }
    
    // Fallback
    if displays.is_empty() {
        log::warn!("Windows display enumeration failed, using fallback");
        displays.push(DisplayInfo {
            id: 0,
            name: "Primary Display".to_string(),
            resolution: (1920, 1080),
            position: (0, 0),
            scale_factor: 1.0,
            is_primary: true,
        });
    }
    
    // Sort so primary is first
    displays.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));
    
    log::info!("Enumerated {} Windows displays", displays.len());
    for d in &displays {
        log::info!("  Display {}: {} ({}x{}) at ({}, {})", 
            d.id, d.name, d.resolution.0, d.resolution.1, d.position.0, d.position.1);
    }
    
    displays
}

#[cfg(target_os = "linux")]
fn enumerate_displays_linux() -> Vec<DisplayInfo> {
    use std::process::Command;
    
    let mut displays = Vec::new();
    
    // Use xrandr to enumerate displays
    // Example output:
    // DP-1 connected primary 2560x1440+0+0 (normal left inverted right x axis y axis) 597mm x 336mm
    // HDMI-1 connected 1920x1080+2560+0 (normal left inverted right x axis y axis) 527mm x 296mm
    if let Ok(output) = Command::new("xrandr")
        .args(["--query"])
        .output()
    {
        if output.status.success() {
            if let Ok(xrandr_output) = String::from_utf8(output.stdout) {
                let mut id = 0u32;
                for line in xrandr_output.lines() {
                    if line.contains(" connected") {
                        // Parse display info from xrandr output
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        let name = parts.first().unwrap_or(&"Display").to_string();
                        let is_primary = line.contains("primary");
                        
                        // Find the geometry string (e.g., "2560x1440+0+0" or "1920x1080+2560+0")
                        // It's the part that matches the pattern WxH+X+Y
                        let mut resolution = (1920, 1080);
                        let mut position = (0, 0);
                        
                        for part in &parts {
                            // Look for geometry in format WIDTHxHEIGHT+X+Y
                            if part.contains('x') && part.contains('+') {
                                // Split by 'x' first, then by '+'
                                let geo_parts: Vec<&str> = part.split(|c| c == 'x' || c == '+').collect();
                                if geo_parts.len() >= 4 {
                                    if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                                        geo_parts[0].parse::<u32>(),
                                        geo_parts[1].parse::<u32>(),
                                        geo_parts[2].parse::<i32>(),
                                        geo_parts[3].parse::<i32>(),
                                    ) {
                                        resolution = (w, h);
                                        position = (x, y);
                                        break;
                                    }
                                }
                            }
                        }
                        
                        // Try to get scale factor from xrandr --verbose or assume 1.0
                        let scale_factor = 1.0;
                        
                        displays.push(DisplayInfo {
                            id,
                            name,
                            resolution,
                            position,
                            scale_factor,
                            is_primary,
                        });
                        id += 1;
                    }
                }
            }
        }
    }
    
    // Fallback
    if displays.is_empty() {
        log::warn!("xrandr enumeration failed, using fallback");
        displays.push(DisplayInfo {
            id: 0,
            name: "Primary Display".to_string(),
            resolution: (1920, 1080),
            position: (0, 0),
            scale_factor: 1.0,
            is_primary: true,
        });
    }
    
    // Sort so primary is first
    displays.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));
    
    log::info!("Enumerated {} Linux displays via xrandr", displays.len());
    for d in &displays {
        log::info!("  Display {}: {} ({}x{}) at ({}, {})", 
            d.id, d.name, d.resolution.0, d.resolution.1, d.position.0, d.position.1);
    }
    
    displays
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn enumerate_displays_fallback() -> Vec<DisplayInfo> {
    vec![DisplayInfo {
        id: 0,
        name: "Default Display".to_string(),
        resolution: (1920, 1080),
        position: (0, 0),
        scale_factor: 1.0,
        is_primary: true,
    }]
}

