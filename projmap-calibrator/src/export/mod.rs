//! Export module for calibration data.

use crate::blending::BlendMask;
use crate::config::ProjectConfig;
use std::path::Path;

/// Export calibration data to various formats.
pub struct CalibrationExporter;

impl CalibrationExporter {
    /// Export project configuration to XML (.projmap file).
    pub fn export_xml(project: &ProjectConfig, path: &Path) -> std::io::Result<()> {
        let xml = quick_xml::se::to_string(project)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(path, xml)
    }

    /// Export project configuration to JSON.
    pub fn export_json(project: &ProjectConfig, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(project)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(path, json)
    }

    /// Export blend mask as 8-bit grayscale PNG image.
    pub fn export_blend_mask(mask: &BlendMask, path: &Path) -> std::io::Result<()> {
        let img = image::GrayImage::from_fn(mask.width, mask.height, |x, y| {
            let idx = (y * mask.width + x) as usize;
            let value = (mask.data[idx] * 255.0).clamp(0.0, 255.0) as u8;
            image::Luma([value])
        });

        img.save(path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    /// Export blend mask as 16-bit grayscale PNG image for higher precision.
    pub fn export_blend_mask_16bit(mask: &BlendMask, path: &Path) -> std::io::Result<()> {
        let img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(
            mask.width,
            mask.height,
            |x, y| {
                let idx = (y * mask.width + x) as usize;
                let value = (mask.data[idx] * 65535.0).clamp(0.0, 65535.0) as u16;
                image::Luma([value])
            },
        );

        img.save(path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    /// Export all blend masks from overlap detection to a directory.
    pub fn export_all_blend_masks(
        masks: &[BlendMask],
        output_dir: &Path,
        use_16bit: bool,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        for (i, mask) in masks.iter().enumerate() {
            let filename = format!("blend_mask_projector_{}.png", i);
            let path = output_dir.join(&filename);

            if use_16bit {
                Self::export_blend_mask_16bit(mask, &path)?;
            } else {
                Self::export_blend_mask(mask, &path)?;
            }

            log::info!("Exported blend mask: {}", filename);
        }

        Ok(())
    }
}

/// Load project configuration.
pub fn load_project(path: &Path) -> std::io::Result<ProjectConfig> {
    let contents = std::fs::read_to_string(path)?;

    // Try JSON first, then XML
    if path.extension().map(|e| e == "json").unwrap_or(false) {
        serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    } else {
        quick_xml::de::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}
