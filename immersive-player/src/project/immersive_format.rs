//! Immersive file format (.immersive)
//!
//! XML-based composition format similar to Resolume's .avc format.

use crate::composition::{
    BlendMode, Clip, ClipSlot, Composition, GeneratorClip, GeneratorType, ImageClip, Layer,
    SolidColorClip, TriggerMode, VideoClip,
};
use std::io::{Read, Write};
use std::path::Path;

/// Version info for the file format
const FORMAT_VERSION_MAJOR: u32 = 1;
const FORMAT_VERSION_MINOR: u32 = 0;
const FORMAT_VERSION_MICRO: u32 = 0;

/// Generate a unique ID based on current time
fn generate_unique_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Save a composition to an .immersive XML file
pub fn save_immersive(composition: &Composition, path: &Path) -> anyhow::Result<()> {
    let xml = composition_to_xml(composition)?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(xml.as_bytes())?;
    log::info!("Saved composition to {:?}", path);
    Ok(())
}

/// Load a composition from an .immersive XML file
pub fn load_immersive(path: &Path) -> anyhow::Result<Composition> {
    let mut file = std::fs::File::open(path)?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;
    let composition = xml_to_composition(&xml)?;
    log::info!("Loaded composition from {:?}", path);
    Ok(composition)
}

/// Convert a composition to XML string
pub fn composition_to_xml(composition: &Composition) -> anyhow::Result<String> {
    let mut xml = String::new();

    // XML declaration
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");

    // Root Composition element
    let comp_id = generate_unique_id();
    xml.push_str(&format!(
        "<Composition name=\"Composition\" uniqueId=\"{}\" numLayers=\"{}\" numColumns=\"{}\">\n",
        comp_id,
        composition.layers.len(),
        composition.columns
    ));

    // Version info
    xml.push_str(&format!(
        "\t<versionInfo name=\"Immersive Player\" majorVersion=\"{}\" minorVersion=\"{}\" microVersion=\"{}\"/>\n",
        FORMAT_VERSION_MAJOR, FORMAT_VERSION_MINOR, FORMAT_VERSION_MICRO
    ));

    // Composition info
    xml.push_str(&format!(
        "\t<CompositionInfo name=\"{}\" description=\"\" width=\"{}\" height=\"{}\" fps=\"{}\">\n",
        "Untitled",
        composition.settings.width,
        composition.settings.height,
        composition.settings.fps
    ));
    xml.push_str(&format!(
        "\t\t<BackgroundColor r=\"{}\" g=\"{}\" b=\"{}\" a=\"{}\"/>\n",
        composition.settings.background_color[0],
        composition.settings.background_color[1],
        composition.settings.background_color[2],
        composition.settings.background_color[3]
    ));
    xml.push_str("\t</CompositionInfo>\n");

    // Master params
    xml.push_str("\t<Params name=\"Master\">\n");
    xml.push_str(&format!(
        "\t\t<Param name=\"Opacity\" type=\"DOUBLE\" value=\"{}\"/>\n",
        composition.master_opacity
    ));
    xml.push_str(&format!(
        "\t\t<Param name=\"Speed\" type=\"DOUBLE\" value=\"{}\"/>\n",
        composition.master_speed
    ));
    xml.push_str("\t</Params>\n");

    // Layers
    for (layer_idx, layer) in composition.layers.iter().enumerate() {
        xml.push_str(&layer_to_xml(layer, layer_idx, 1)?);
    }

    // Clips (organized by layer and column like Resolume)
    xml.push_str("\t<Clips>\n");
    for (layer_idx, layer) in composition.layers.iter().enumerate() {
        for (col_idx, clip_opt) in layer.clips.iter().enumerate() {
            if let Some(clip_slot) = clip_opt {
                xml.push_str(&clip_slot_to_xml(clip_slot, layer_idx, col_idx, 2)?);
            }
        }
    }
    xml.push_str("\t</Clips>\n");

    // Close composition
    xml.push_str("</Composition>\n");

    Ok(xml)
}

/// Convert a layer to XML
fn layer_to_xml(layer: &Layer, layer_idx: usize, indent: usize) -> anyhow::Result<String> {
    let tabs = "\t".repeat(indent);
    let mut xml = String::new();

    xml.push_str(&format!(
        "{}<Layer name=\"{}\" uniqueId=\"{}\" layerIndex=\"{}\">\n",
        tabs,
        escape_xml(&layer.name),
        layer.id,
        layer_idx
    ));

    // Layer params
    xml.push_str(&format!("{}\t<Params name=\"LayerParams\">\n", tabs));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Opacity\" type=\"DOUBLE\" value=\"{}\"/>\n",
        tabs, layer.opacity
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"BlendMode\" type=\"STRING\" value=\"{}\"/>\n",
        tabs,
        layer.blend_mode.name()
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Bypass\" type=\"BOOL\" value=\"{}\"/>\n",
        tabs,
        if layer.bypass { "1" } else { "0" }
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Solo\" type=\"BOOL\" value=\"{}\"/>\n",
        tabs,
        if layer.solo { "1" } else { "0" }
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Volume\" type=\"DOUBLE\" value=\"{}\"/>\n",
        tabs, layer.volume
    ));
    xml.push_str(&format!("{}\t</Params>\n", tabs));

    // Transform
    xml.push_str(&format!("{}\t<Transform>\n", tabs));
    xml.push_str(&format!(
        "{}\t\t<Position x=\"{}\" y=\"{}\"/>\n",
        tabs, layer.transform.position.0, layer.transform.position.1
    ));
    xml.push_str(&format!(
        "{}\t\t<Scale x=\"{}\" y=\"{}\"/>\n",
        tabs, layer.transform.scale.0, layer.transform.scale.1
    ));
    xml.push_str(&format!(
        "{}\t\t<Rotation value=\"{}\"/>\n",
        tabs, layer.transform.rotation
    ));
    xml.push_str(&format!(
        "{}\t\t<Anchor x=\"{}\" y=\"{}\"/>\n",
        tabs, layer.transform.anchor.0, layer.transform.anchor.1
    ));
    xml.push_str(&format!("{}\t</Transform>\n", tabs));

    // Active column
    if let Some(active) = layer.active_column {
        xml.push_str(&format!(
            "{}\t<ActiveColumn value=\"{}\"/>\n",
            tabs, active
        ));
    }

    xml.push_str(&format!("{}</Layer>\n", tabs));

    Ok(xml)
}

/// Convert a clip slot to XML
fn clip_slot_to_xml(
    slot: &ClipSlot,
    layer_idx: usize,
    col_idx: usize,
    indent: usize,
) -> anyhow::Result<String> {
    let tabs = "\t".repeat(indent);
    let mut xml = String::new();

    let clip_type = match &slot.clip {
        Clip::Video(_) => "Video",
        Clip::Image(_) => "Image",
        Clip::SolidColor(_) => "SolidColor",
        Clip::Generator(_) => "Generator",
    };

    xml.push_str(&format!(
        "{}<Clip name=\"{}\" uniqueId=\"{}\" layerIndex=\"{}\" columnIndex=\"{}\" type=\"{}\">\n",
        tabs,
        escape_xml(&slot.name()),
        slot.id,
        layer_idx,
        col_idx,
        clip_type
    ));

    // Clip params
    xml.push_str(&format!("{}\t<Params>\n", tabs));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"TriggerMode\" type=\"STRING\" value=\"{}\"/>\n",
        tabs,
        match slot.trigger_mode {
            TriggerMode::Toggle => "Toggle",
            TriggerMode::Flash => "Flash",
            TriggerMode::OneShot => "OneShot",
        }
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Speed\" type=\"DOUBLE\" value=\"{}\"/>\n",
        tabs, slot.speed
    ));
    xml.push_str(&format!(
        "{}\t\t<Param name=\"Opacity\" type=\"DOUBLE\" value=\"{}\"/>\n",
        tabs, slot.opacity
    ));
    xml.push_str(&format!("{}\t</Params>\n", tabs));

    // Clip content based on type
    match &slot.clip {
        Clip::Video(video) => {
            xml.push_str(&format!("{}\t<VideoClip>\n", tabs));
            xml.push_str(&format!(
                "{}\t\t<Path value=\"{}\"/>\n",
                tabs,
                escape_xml(&video.path.to_string_lossy())
            ));
            xml.push_str(&format!(
                "{}\t\t<Duration value=\"{}\"/>\n",
                tabs, video.duration
            ));
            xml.push_str(&format!(
                "{}\t\t<Dimensions width=\"{}\" height=\"{}\"/>\n",
                tabs, video.dimensions.0, video.dimensions.1
            ));
            xml.push_str(&format!(
                "{}\t\t<FrameRate value=\"{}\"/>\n",
                tabs, video.frame_rate
            ));
            xml.push_str(&format!(
                "{}\t\t<LoopMode value=\"{:?}\"/>\n",
                tabs, video.loop_mode
            ));
            xml.push_str(&format!("{}\t</VideoClip>\n", tabs));
        }
        Clip::Image(image) => {
            xml.push_str(&format!("{}\t<ImageClip>\n", tabs));
            xml.push_str(&format!(
                "{}\t\t<Path value=\"{}\"/>\n",
                tabs,
                escape_xml(&image.path.to_string_lossy())
            ));
            xml.push_str(&format!(
                "{}\t\t<Dimensions width=\"{}\" height=\"{}\"/>\n",
                tabs, image.dimensions.0, image.dimensions.1
            ));
            xml.push_str(&format!("{}\t</ImageClip>\n", tabs));
        }
        Clip::SolidColor(solid) => {
            xml.push_str(&format!("{}\t<SolidColorClip>\n", tabs));
            xml.push_str(&format!(
                "{}\t\t<Color r=\"{}\" g=\"{}\" b=\"{}\" a=\"{}\"/>\n",
                tabs, solid.color[0], solid.color[1], solid.color[2], solid.color[3]
            ));
            xml.push_str(&format!("{}\t</SolidColorClip>\n", tabs));
        }
        Clip::Generator(gen) => {
            xml.push_str(&format!("{}\t<GeneratorClip>\n", tabs));
            xml.push_str(&format!(
                "{}\t\t<Type value=\"{}\"/>\n",
                tabs,
                gen.generator_type.name()
            ));
            xml.push_str(&format!(
                "{}\t\t<Speed value=\"{}\"/>\n",
                tabs, gen.speed
            ));
            // Add generator-specific params
            match &gen.generator_type {
                GeneratorType::Noise { seed, scale, octaves } => {
                    xml.push_str(&format!(
                        "{}\t\t<NoiseParams seed=\"{}\" scale=\"{}\" octaves=\"{}\"/>\n",
                        tabs, seed, scale, octaves
                    ));
                }
                GeneratorType::Gradient { colors, angle, gradient_type } => {
                    xml.push_str(&format!(
                        "{}\t\t<GradientParams angle=\"{}\" type=\"{:?}\">\n",
                        tabs, angle, gradient_type
                    ));
                    for (i, color) in colors.iter().enumerate() {
                        xml.push_str(&format!(
                            "{}\t\t\t<Color index=\"{}\" r=\"{}\" g=\"{}\" b=\"{}\" a=\"{}\"/>\n",
                            tabs, i, color[0], color[1], color[2], color[3]
                        ));
                    }
                    xml.push_str(&format!("{}\t\t</GradientParams>\n", tabs));
                }
                GeneratorType::TestPattern(pattern_type) => {
                    xml.push_str(&format!(
                        "{}\t\t<TestPatternParams type=\"{:?}\"/>\n",
                        tabs, pattern_type
                    ));
                }
                GeneratorType::Plasma { speed, scale } => {
                    xml.push_str(&format!(
                        "{}\t\t<PlasmaParams speed=\"{}\" scale=\"{}\"/>\n",
                        tabs, speed, scale
                    ));
                }
                GeneratorType::ColorBars => {
                    xml.push_str(&format!("{}\t\t<ColorBarsParams/>\n", tabs));
                }
            }
            xml.push_str(&format!("{}\t</GeneratorClip>\n", tabs));
        }
    }

    xml.push_str(&format!("{}</Clip>\n", tabs));

    Ok(xml)
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Unescape XML characters
fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Parse XML to composition (basic parser)
pub fn xml_to_composition(xml: &str) -> anyhow::Result<Composition> {
    let mut composition = Composition::default();

    // Parse composition attributes
    if let Some(caps) = extract_attr(xml, "Composition", "numLayers") {
        let num_layers: usize = caps.parse().unwrap_or(4);
        // Adjust layers
        while composition.layers.len() > num_layers {
            composition.layers.pop();
        }
        while composition.layers.len() < num_layers {
            composition.add_layer();
        }
    }

    if let Some(caps) = extract_attr(xml, "Composition", "numColumns") {
        let num_cols: usize = caps.parse().unwrap_or(6);
        while composition.columns > num_cols {
            composition.remove_column();
        }
        while composition.columns < num_cols {
            composition.add_column();
        }
    }

    // Parse CompositionInfo
    if let Some(width) = extract_attr(xml, "CompositionInfo", "width") {
        composition.settings.width = width.parse().unwrap_or(1920);
    }
    if let Some(height) = extract_attr(xml, "CompositionInfo", "height") {
        composition.settings.height = height.parse().unwrap_or(1080);
    }
    if let Some(fps) = extract_attr(xml, "CompositionInfo", "fps") {
        composition.settings.fps = fps.parse().unwrap_or(60.0);
    }

    // Parse master opacity
    if let Some(opacity) = extract_param_value(xml, "Master", "Opacity") {
        composition.master_opacity = opacity.parse().unwrap_or(1.0);
    }
    if let Some(speed) = extract_param_value(xml, "Master", "Speed") {
        composition.master_speed = speed.parse().unwrap_or(1.0);
    }

    // Parse layers
    let layer_sections = extract_sections(xml, "Layer");
    for layer_xml in layer_sections {
        if let Some(layer_idx_str) = extract_attr(&layer_xml, "Layer", "layerIndex") {
            let layer_idx: usize = layer_idx_str.parse().unwrap_or(0);
            if layer_idx < composition.layers.len() {
                parse_layer_xml(&layer_xml, &mut composition.layers[layer_idx]);
            }
        }
    }

    // Parse clips
    let clip_sections = extract_sections(xml, "Clip");
    for clip_xml in clip_sections {
        if let (Some(layer_idx_str), Some(col_idx_str)) = (
            extract_attr(&clip_xml, "Clip", "layerIndex"),
            extract_attr(&clip_xml, "Clip", "columnIndex"),
        ) {
            let layer_idx: usize = layer_idx_str.parse().unwrap_or(0);
            let col_idx: usize = col_idx_str.parse().unwrap_or(0);

            if layer_idx < composition.layers.len() {
                if let Some(clip_slot) = parse_clip_xml(&clip_xml) {
                    if col_idx < composition.layers[layer_idx].clips.len() {
                        composition.layers[layer_idx].clips[col_idx] = Some(clip_slot);
                    }
                }
            }
        }
    }

    Ok(composition)
}

/// Extract attribute value from XML element
fn extract_attr(xml: &str, element: &str, attr: &str) -> Option<String> {
    let pattern = format!("<{}", element);
    if let Some(start) = xml.find(&pattern) {
        let rest = &xml[start..];
        if let Some(end) = rest.find('>') {
            let tag = &rest[..end];
            let attr_pattern = format!("{}=\"", attr);
            if let Some(attr_start) = tag.find(&attr_pattern) {
                let value_start = attr_start + attr_pattern.len();
                let value_rest = &tag[value_start..];
                if let Some(value_end) = value_rest.find('"') {
                    return Some(unescape_xml(&value_rest[..value_end]));
                }
            }
        }
    }
    None
}

/// Extract param value from XML
fn extract_param_value(xml: &str, params_name: &str, param_name: &str) -> Option<String> {
    let params_pattern = format!("<Params name=\"{}\">", params_name);
    if let Some(params_start) = xml.find(&params_pattern) {
        let rest = &xml[params_start..];
        if let Some(params_end) = rest.find("</Params>") {
            let params_section = &rest[..params_end];
            let param_pattern = format!("<Param name=\"{}\"", param_name);
            if let Some(param_start) = params_section.find(&param_pattern) {
                let param_rest = &params_section[param_start..];
                if let Some(end) = param_rest.find("/>") {
                    let param_tag = &param_rest[..end];
                    if let Some(val_start) = param_tag.find("value=\"") {
                        let val_rest = &param_tag[val_start + 7..];
                        if let Some(val_end) = val_rest.find('"') {
                            return Some(unescape_xml(&val_rest[..val_end]));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract all sections of a given element type
fn extract_sections(xml: &str, element: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let open_tag = format!("<{} ", element);
    let close_tag = format!("</{}>", element);

    let mut search_start = 0;
    while let Some(start) = xml[search_start..].find(&open_tag) {
        let abs_start = search_start + start;
        if let Some(end) = xml[abs_start..].find(&close_tag) {
            let abs_end = abs_start + end + close_tag.len();
            sections.push(xml[abs_start..abs_end].to_string());
            search_start = abs_end;
        } else {
            break;
        }
    }

    sections
}

/// Parse layer XML into a Layer
fn parse_layer_xml(xml: &str, layer: &mut Layer) {
    if let Some(name) = extract_attr(xml, "Layer", "name") {
        layer.name = name;
    }

    if let Some(opacity) = extract_param_value(xml, "LayerParams", "Opacity") {
        layer.opacity = opacity.parse().unwrap_or(1.0);
    }

    if let Some(blend_mode) = extract_param_value(xml, "LayerParams", "BlendMode") {
        layer.blend_mode = match blend_mode.as_str() {
            "Add" => BlendMode::Add,
            "Multiply" => BlendMode::Multiply,
            "Screen" => BlendMode::Screen,
            "Overlay" => BlendMode::Overlay,
            _ => BlendMode::Normal,
        };
    }

    if let Some(bypass) = extract_param_value(xml, "LayerParams", "Bypass") {
        layer.bypass = bypass == "1";
    }

    if let Some(solo) = extract_param_value(xml, "LayerParams", "Solo") {
        layer.solo = solo == "1";
    }

    // Parse transform
    if let Some(pos_x) = extract_attr(xml, "Position", "x") {
        layer.transform.position.0 = pos_x.parse().unwrap_or(0.0);
    }
    if let Some(pos_y) = extract_attr(xml, "Position", "y") {
        layer.transform.position.1 = pos_y.parse().unwrap_or(0.0);
    }
    if let Some(scale_x) = extract_attr(xml, "Scale", "x") {
        layer.transform.scale.0 = scale_x.parse().unwrap_or(1.0);
    }
    if let Some(scale_y) = extract_attr(xml, "Scale", "y") {
        layer.transform.scale.1 = scale_y.parse().unwrap_or(1.0);
    }
    if let Some(rotation) = extract_attr(xml, "Rotation", "value") {
        layer.transform.rotation = rotation.parse().unwrap_or(0.0);
    }
}

/// Parse clip XML into a ClipSlot
fn parse_clip_xml(xml: &str) -> Option<ClipSlot> {
    let clip_type = extract_attr(xml, "Clip", "type")?;

    let clip = match clip_type.as_str() {
        "Video" => {
            let path = extract_attr(xml, "Path", "value").unwrap_or_default();
            let mut video = VideoClip::new(std::path::PathBuf::from(path));
            if let Some(duration) = extract_attr(xml, "Duration", "value") {
                video.duration = duration.parse().unwrap_or(10.0);
            }
            if let Some(width) = extract_attr(xml, "Dimensions", "width") {
                video.dimensions.0 = width.parse().unwrap_or(1920);
            }
            if let Some(height) = extract_attr(xml, "Dimensions", "height") {
                video.dimensions.1 = height.parse().unwrap_or(1080);
            }
            Clip::Video(video)
        }
        "Image" => {
            let path = extract_attr(xml, "Path", "value").unwrap_or_default();
            let mut image = ImageClip::new(std::path::PathBuf::from(path));
            if let Some(width) = extract_attr(xml, "Dimensions", "width") {
                image.dimensions.0 = width.parse().unwrap_or(1920);
            }
            if let Some(height) = extract_attr(xml, "Dimensions", "height") {
                image.dimensions.1 = height.parse().unwrap_or(1080);
            }
            Clip::Image(image)
        }
        "SolidColor" => {
            let r = extract_attr(xml, "Color", "r")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let g = extract_attr(xml, "Color", "g")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let b = extract_attr(xml, "Color", "b")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let a = extract_attr(xml, "Color", "a")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            Clip::SolidColor(SolidColorClip::new([r, g, b, a]))
        }
        "Generator" => {
            let gen_type = extract_attr(xml, "Type", "value").unwrap_or_default();
            let generator_type = match gen_type.as_str() {
                "Noise" => GeneratorType::simple_noise(),
                "Gradient" => GeneratorType::horizontal_gradient(
                    [0.0, 0.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0, 1.0],
                ),
                "Plasma" => GeneratorType::Plasma {
                    speed: 1.0,
                    scale: 1.0,
                },
                "Color Bars" => GeneratorType::ColorBars,
                _ => GeneratorType::simple_noise(),
            };
            let speed = extract_attr(xml, "Speed", "value")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let mut gen = GeneratorClip::new(generator_type);
            gen.speed = speed;
            Clip::Generator(gen)
        }
        _ => return None,
    };

    let mut slot = ClipSlot::new(clip);

    // Parse clip params
    if let Some(trigger_mode) = extract_param_value(xml, "Params", "TriggerMode") {
        slot.trigger_mode = match trigger_mode.as_str() {
            "Flash" => TriggerMode::Flash,
            "OneShot" => TriggerMode::OneShot,
            _ => TriggerMode::Toggle,
        };
    }
    if let Some(speed) = extract_param_value(xml, "Params", "Speed") {
        slot.speed = speed.parse().unwrap_or(1.0);
    }
    if let Some(opacity) = extract_param_value(xml, "Params", "Opacity") {
        slot.opacity = opacity.parse().unwrap_or(1.0);
    }

    Some(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a < b"), "a &lt; b");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }

    #[test]
    fn test_roundtrip() {
        let composition = Composition::default();
        let xml = composition_to_xml(&composition).unwrap();
        let loaded = xml_to_composition(&xml).unwrap();

        assert_eq!(loaded.layers.len(), composition.layers.len());
        assert_eq!(loaded.columns, composition.columns);
        assert_eq!(loaded.settings.width, composition.settings.width);
    }
}

