//! Gray code pattern generation for structured light calibration.

/// Direction of pattern stripes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternDirection {
    /// Stripes run horizontally, decode Y coordinate.
    Horizontal,
    /// Stripes run vertically, decode X coordinate.
    Vertical,
}

/// A single Gray code pattern specification.
#[derive(Debug, Clone)]
pub struct PatternSpec {
    /// Which bit of the Gray code this pattern encodes.
    pub bit_index: u32,
    /// Pattern direction.
    pub direction: PatternDirection,
    /// Whether this is the inverted version.
    pub inverted: bool,
}

/// Configuration for pattern generation.
#[derive(Debug, Clone)]
pub struct PatternConfig {
    /// Projector resolution width.
    pub projector_width: u32,
    /// Projector resolution height.
    pub projector_height: u32,
    /// Number of bits for horizontal patterns.
    pub horizontal_bits: u32,
    /// Number of bits for vertical patterns.
    pub vertical_bits: u32,
}

impl PatternConfig {
    pub fn new(width: u32, height: u32) -> Self {
        let horizontal_bits = (width as f32).log2().ceil() as u32;
        let vertical_bits = (height as f32).log2().ceil() as u32;

        Self {
            projector_width: width,
            projector_height: height,
            horizontal_bits,
            vertical_bits,
        }
    }

    /// Total number of patterns needed (including inverted pairs).
    pub fn total_patterns(&self) -> usize {
        // Each direction: bits * 2 (positive + inverted) + 2 (white/black reference)
        ((self.horizontal_bits + self.vertical_bits) * 2 + 2) as usize
    }

    /// Generate the sequence of all patterns to project.
    pub fn pattern_sequence(&self) -> Vec<PatternSpec> {
        let mut patterns = Vec::with_capacity(self.total_patterns());

        // Horizontal patterns (decode Y coordinate)
        for bit in 0..self.vertical_bits {
            patterns.push(PatternSpec {
                bit_index: bit,
                direction: PatternDirection::Horizontal,
                inverted: false,
            });
            patterns.push(PatternSpec {
                bit_index: bit,
                direction: PatternDirection::Horizontal,
                inverted: true,
            });
        }

        // Vertical patterns (decode X coordinate)
        for bit in 0..self.horizontal_bits {
            patterns.push(PatternSpec {
                bit_index: bit,
                direction: PatternDirection::Vertical,
                inverted: false,
            });
            patterns.push(PatternSpec {
                bit_index: bit,
                direction: PatternDirection::Vertical,
                inverted: true,
            });
        }

        patterns
    }
}

/// Gray code pattern generator.
pub struct GrayCodeGenerator {
    config: PatternConfig,
}

impl GrayCodeGenerator {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            config: PatternConfig::new(width, height),
        }
    }

    pub fn config(&self) -> &PatternConfig {
        &self.config
    }

    /// Convert binary value to Gray code.
    pub fn binary_to_gray(binary: u32) -> u32 {
        binary ^ (binary >> 1)
    }

    /// Convert Gray code back to binary.
    pub fn gray_to_binary(gray: u32) -> u32 {
        let mut binary = gray;
        let mut shift = 1;
        while shift < 32 {
            binary ^= binary >> shift;
            shift *= 2;
        }
        binary
    }

    /// Generate pixel data for a pattern (returns grayscale bytes).
    pub fn generate_pattern(&self, spec: &PatternSpec) -> Vec<u8> {
        let width = self.config.projector_width;
        let height = self.config.projector_height;
        let mut data = vec![0u8; (width * height) as usize];

        let total_bits = match spec.direction {
            PatternDirection::Horizontal => self.config.vertical_bits,
            PatternDirection::Vertical => self.config.horizontal_bits,
        };

        for y in 0..height {
            for x in 0..width {
                let coord = match spec.direction {
                    PatternDirection::Horizontal => y,
                    PatternDirection::Vertical => x,
                };

                let gray = Self::binary_to_gray(coord);
                let bit_position = total_bits - 1 - spec.bit_index;
                let bit_value = (gray >> bit_position) & 1;

                let value = if spec.inverted { 1 - bit_value } else { bit_value };
                let pixel = if value == 1 { 255u8 } else { 0u8 };

                data[(y * width + x) as usize] = pixel;
            }
        }

        data
    }

    /// Generate all-white reference pattern.
    pub fn generate_white(&self) -> Vec<u8> {
        let size = (self.config.projector_width * self.config.projector_height) as usize;
        vec![255u8; size]
    }

    /// Generate all-black reference pattern.
    pub fn generate_black(&self) -> Vec<u8> {
        let size = (self.config.projector_width * self.config.projector_height) as usize;
        vec![0u8; size]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gray_code_conversion() {
        // Test binary to gray and back
        for i in 0..256 {
            let gray = GrayCodeGenerator::binary_to_gray(i);
            let back = GrayCodeGenerator::gray_to_binary(gray);
            assert_eq!(i, back, "Failed for {}", i);
        }
    }

    #[test]
    fn test_pattern_count() {
        let config = PatternConfig::new(1920, 1080);
        // 1920 needs 11 bits (2048 > 1920)
        // 1080 needs 11 bits (2048 > 1080)
        // Total: 11 * 2 + 11 * 2 + 2 = 46
        assert_eq!(config.horizontal_bits, 11);
        assert_eq!(config.vertical_bits, 11);
        assert_eq!(config.total_patterns(), 46);
    }
}
