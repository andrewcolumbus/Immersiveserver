//! GPU profiling with wgpu timestamp queries
//!
//! Provides GPU timing using timestamp queries when supported by the device.

use std::collections::HashMap;
use wgpu::{Buffer, CommandEncoder, Device, QuerySet, QueryType, Queue};

/// GPU profiler using wgpu timestamp queries
///
/// This profiler measures GPU execution time for render passes.
/// It requires the `TIMESTAMP_QUERY` feature to be supported by the device.
pub struct GpuProfiler {
    /// Query set for timestamp queries
    query_set: Option<QuerySet>,
    /// Buffer to resolve query results
    resolve_buffer: Option<Buffer>,
    /// Buffer for CPU readback
    readback_buffer: Option<Buffer>,
    /// Maximum number of query pairs (start + end)
    max_queries: u32,
    /// Current query index
    current_query: u32,
    /// Map of query index to region name
    query_names: HashMap<u32, String>,
    /// Last computed timings (region name -> milliseconds)
    last_timings: HashMap<String, f64>,
    /// Timestamp period in nanoseconds
    timestamp_period: f32,
    /// Whether GPU profiling is enabled
    enabled: bool,
    /// Pending regions that have been started but not ended
    pending_regions: HashMap<String, u32>,
}

impl GpuProfiler {
    /// Create a new GPU profiler
    ///
    /// Returns a disabled profiler if timestamp queries are not supported.
    pub fn new(device: &Device, queue: &Queue, max_queries: u32) -> Self {
        // Check if timestamp queries are supported
        let features = device.features();
        let enabled = features.contains(wgpu::Features::TIMESTAMP_QUERY);

        if !enabled {
            tracing::warn!(
                target: "immersive_server::telemetry",
                "GPU timestamp queries not supported - profiling disabled"
            );
            return Self::disabled();
        }

        // Get timestamp period
        let timestamp_period = queue.get_timestamp_period();

        // Create query set (2 queries per region: start + end)
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("GPU Profiler Query Set"),
            ty: QueryType::Timestamp,
            count: max_queries * 2,
        });

        // Create resolve buffer (8 bytes per query for u64 timestamps)
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Profiler Resolve Buffer"),
            size: (max_queries * 2 * 8) as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create readback buffer
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Profiler Readback Buffer"),
            size: (max_queries * 2 * 8) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        tracing::info!(
            target: "immersive_server::telemetry",
            max_queries = max_queries,
            timestamp_period_ns = timestamp_period,
            "GPU profiler initialized"
        );

        Self {
            query_set: Some(query_set),
            resolve_buffer: Some(resolve_buffer),
            readback_buffer: Some(readback_buffer),
            max_queries,
            current_query: 0,
            query_names: HashMap::new(),
            last_timings: HashMap::new(),
            timestamp_period,
            enabled: true,
            pending_regions: HashMap::new(),
        }
    }

    /// Create a disabled profiler (no GPU support)
    pub fn disabled() -> Self {
        Self {
            query_set: None,
            resolve_buffer: None,
            readback_buffer: None,
            max_queries: 0,
            current_query: 0,
            query_names: HashMap::new(),
            last_timings: HashMap::new(),
            timestamp_period: 1.0,
            enabled: false,
            pending_regions: HashMap::new(),
        }
    }

    /// Check if GPU profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Begin a new frame - resets query counters
    pub fn begin_frame(&mut self) {
        if !self.enabled {
            return;
        }
        self.current_query = 0;
        self.query_names.clear();
        self.pending_regions.clear();
    }

    /// Begin timing a GPU region
    ///
    /// Call this before issuing GPU commands for the region.
    pub fn begin_region(&mut self, encoder: &mut CommandEncoder, name: &str) {
        if !self.enabled {
            return;
        }
        if self.current_query >= self.max_queries * 2 {
            return; // Out of queries
        }

        let query_set = self.query_set.as_ref().unwrap();
        let query_idx = self.current_query;

        encoder.write_timestamp(query_set, query_idx);
        self.pending_regions.insert(name.to_string(), query_idx);
        self.current_query += 1;
    }

    /// End timing a GPU region
    ///
    /// Call this after issuing GPU commands for the region.
    pub fn end_region(&mut self, encoder: &mut CommandEncoder, name: &str) {
        if !self.enabled {
            return;
        }
        if self.current_query >= self.max_queries * 2 {
            return; // Out of queries
        }

        // Get the start query index
        let start_idx = match self.pending_regions.remove(name) {
            Some(idx) => idx,
            None => {
                tracing::warn!(
                    target: "immersive_server::telemetry",
                    region = name,
                    "end_region called without matching begin_region"
                );
                return;
            }
        };

        let query_set = self.query_set.as_ref().unwrap();
        let end_idx = self.current_query;

        encoder.write_timestamp(query_set, end_idx);

        // Store the start index so we can pair them later
        self.query_names.insert(start_idx, name.to_string());
        self.current_query += 1;
    }

    /// Resolve timestamp queries to the resolve buffer
    ///
    /// Call this before submitting the command encoder.
    pub fn resolve(&self, encoder: &mut CommandEncoder) {
        if !self.enabled || self.current_query == 0 {
            return;
        }

        let query_set = self.query_set.as_ref().unwrap();
        let resolve_buffer = self.resolve_buffer.as_ref().unwrap();
        let readback_buffer = self.readback_buffer.as_ref().unwrap();

        // Resolve all written queries
        encoder.resolve_query_set(query_set, 0..self.current_query, resolve_buffer, 0);

        // Copy to readback buffer
        encoder.copy_buffer_to_buffer(
            resolve_buffer,
            0,
            readback_buffer,
            0,
            (self.current_query * 8) as u64,
        );
    }

    /// Process the readback buffer and compute timings
    ///
    /// This function attempts to map the readback buffer and compute timings.
    /// It's non-blocking - if the buffer isn't ready, it returns None.
    pub fn process(&mut self, device: &Device) -> Option<HashMap<String, f64>> {
        if !self.enabled || self.current_query == 0 {
            return None;
        }

        let readback_buffer = self.readback_buffer.as_ref().unwrap();
        let buffer_slice = readback_buffer.slice(..);

        // Try to map the buffer (non-blocking check)
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        // Poll for completion
        device.poll(wgpu::Maintain::Poll);

        // Check if mapping succeeded
        if buffer_slice.get_mapped_range().is_empty() {
            return None;
        }

        // Read timestamps
        let data = buffer_slice.get_mapped_range();
        let timestamps: &[u64] = bytemuck::cast_slice(&data);

        // Compute timings
        let mut timings = HashMap::new();
        let mut total_ms = 0.0;

        for (start_idx, name) in &self.query_names {
            let start_idx = *start_idx as usize;
            let end_idx = start_idx + 1;

            if end_idx < timestamps.len() {
                let start = timestamps[start_idx];
                let end = timestamps[end_idx];

                if end >= start {
                    let delta_ns = (end - start) as f64 * self.timestamp_period as f64;
                    let delta_ms = delta_ns / 1_000_000.0;
                    timings.insert(name.clone(), delta_ms);
                    total_ms += delta_ms;
                }
            }
        }

        timings.insert("total".to_string(), total_ms);

        // Unmap buffer
        drop(data);
        readback_buffer.unmap();

        self.last_timings = timings.clone();
        Some(timings)
    }

    /// Get the last computed timings
    pub fn last_timings(&self) -> &HashMap<String, f64> {
        &self.last_timings
    }

    /// Get a specific timing by region name
    pub fn get_timing(&self, name: &str) -> Option<f64> {
        self.last_timings.get(name).copied()
    }

    /// Get total GPU time from last frame
    pub fn total_ms(&self) -> f64 {
        self.last_timings.get("total").copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_profiler() {
        let profiler = GpuProfiler::disabled();
        assert!(!profiler.is_enabled());
        assert!(profiler.last_timings().is_empty());
    }
}
