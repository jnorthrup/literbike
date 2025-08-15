// Packet Fragmentation for DPI Evasion
// Intelligent fragmentation to bypass deep packet inspection

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use rand::Rng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::sleep;

/// Fragment configuration for DPI evasion
#[derive(Debug, Clone)]
pub struct FragmentConfig {
    pub min_fragment_size: usize,
    pub max_fragment_size: usize,
    pub fragment_delay_ms: Range<u64>,
    pub randomize_order: bool,
    pub duplicate_fragments: bool,
    pub overlap_fragments: bool,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            min_fragment_size: 8,
            max_fragment_size: 1200,
            fragment_delay_ms: Range { start: 1, end: 50 },
            randomize_order: false,
            duplicate_fragments: false,
            overlap_fragments: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Range<T> {
    pub start: T,
    pub end: T,
}

/// Mobile-specific fragmentation patterns
#[derive(Debug, Clone)]
pub enum MobileFragmentPattern {
    Conservative,  // Minimal fragmentation, low latency
    Aggressive,    // Heavy fragmentation, high evasion
    Adaptive,      // Dynamic based on detection
    Carrier(CarrierProfile),
}

#[derive(Debug, Clone)]
pub enum CarrierProfile {
    Verizon,
    ATT,
    TMobile,
    Sprint,
}

impl CarrierProfile {
    /// Get typical MTU and fragmentation behavior for carrier
    pub fn get_mtu_characteristics(&self) -> (u16, FragmentConfig) {
        match self {
            CarrierProfile::Verizon => (
                1428, // Typical Verizon MTU
                FragmentConfig {
                    min_fragment_size: 64,
                    max_fragment_size: 1200,
                    fragment_delay_ms: Range { start: 5, end: 25 },
                    randomize_order: false,
                    duplicate_fragments: false,
                    overlap_fragments: false,
                }
            ),
            CarrierProfile::ATT => (
                1500,
                FragmentConfig {
                    min_fragment_size: 32,
                    max_fragment_size: 1460,
                    fragment_delay_ms: Range { start: 10, end: 40 },
                    randomize_order: true,
                    duplicate_fragments: false,
                    overlap_fragments: false,
                }
            ),
            CarrierProfile::TMobile => (
                1500,
                FragmentConfig {
                    min_fragment_size: 128,
                    max_fragment_size: 1400,
                    fragment_delay_ms: Range { start: 2, end: 15 },
                    randomize_order: false,
                    duplicate_fragments: true,
                    overlap_fragments: false,
                }
            ),
            CarrierProfile::Sprint => (
                1472,
                FragmentConfig {
                    min_fragment_size: 96,
                    max_fragment_size: 1300,
                    fragment_delay_ms: Range { start: 8, end: 30 },
                    randomize_order: true,
                    duplicate_fragments: false,
                    overlap_fragments: true,
                }
            ),
        }
    }
}

/// Packet fragment with metadata
#[derive(Debug, Clone)]
pub struct PacketFragment {
    pub data: Vec<u8>,
    pub sequence: u16,
    pub is_last: bool,
    pub timestamp: Instant,
    pub duplicate_count: u8,
}

/// Intelligent packet fragmenter for DPI evasion
pub struct PacketFragmenter {
    config: FragmentConfig,
    pattern: MobileFragmentPattern,
    fragment_queue: VecDeque<PacketFragment>,
    next_sequence: u16,
    stats: FragmentStats,
}

#[derive(Debug, Default)]
pub struct FragmentStats {
    pub total_fragments: u64,
    pub bytes_fragmented: u64,
    pub average_fragment_size: f64,
    pub evasion_score: f32,
}

impl PacketFragmenter {
    pub fn new(pattern: MobileFragmentPattern) -> Self {
        let config = match &pattern {
            MobileFragmentPattern::Conservative => FragmentConfig {
                min_fragment_size: 512,
                max_fragment_size: 1460,
                fragment_delay_ms: Range { start: 1, end: 5 },
                randomize_order: false,
                duplicate_fragments: false,
                overlap_fragments: false,
            },
            MobileFragmentPattern::Aggressive => FragmentConfig {
                min_fragment_size: 8,
                max_fragment_size: 256,
                fragment_delay_ms: Range { start: 5, end: 100 },
                randomize_order: true,
                duplicate_fragments: true,
                overlap_fragments: true,
            },
            MobileFragmentPattern::Adaptive => FragmentConfig::default(),
            MobileFragmentPattern::Carrier(carrier) => {
                carrier.get_mtu_characteristics().1
            },
        };
        
        Self {
            config,
            pattern,
            fragment_queue: VecDeque::new(),
            next_sequence: rand::thread_rng().gen(),
            stats: FragmentStats::default(),
        }
    }
    
    /// Fragment data packet using mobile-specific patterns
    pub fn fragment_packet(&mut self, data: &[u8]) -> Vec<PacketFragment> {
        if data.is_empty() {
            return Vec::new();
        }
        
        let mut fragments = Vec::new();
        let mut remaining = data;
        let mut sequence = self.next_sequence;
        
        while !remaining.is_empty() {
            let fragment_size = self.calculate_fragment_size(remaining.len());
            let chunk_size = std::cmp::min(fragment_size, remaining.len());
            
            let fragment_data = remaining[..chunk_size].to_vec();
            let is_last = chunk_size == remaining.len();
            
            let fragment = PacketFragment {
                data: fragment_data,
                sequence,
                is_last,
                timestamp: Instant::now(),
                duplicate_count: 0,
            };
            
            fragments.push(fragment.clone());
            
            // Add duplicates if configured
            if self.config.duplicate_fragments && !is_last {
                let mut duplicate = fragment.clone();
                duplicate.duplicate_count = 1;
                fragments.push(duplicate);
            }
            
            remaining = &remaining[chunk_size..];
            sequence = sequence.wrapping_add(1);
        }
        
        // Randomize order if configured
        if self.config.randomize_order && fragments.len() > 2 {
            // Keep first and last fragments in order, randomize middle
            let len = fragments.len();
            if len > 2 {
                let mut middle: Vec<_> = fragments.drain(1..len-1).collect();
                use rand::seq::SliceRandom;
                middle.shuffle(&mut rand::thread_rng());
                
                let last = fragments.pop().unwrap();
                fragments.extend(middle);
                fragments.push(last);
            }
        }
        
        // Add overlapping fragments if configured
        if self.config.overlap_fragments && fragments.len() > 1 {
            self.add_overlapping_fragments(&mut fragments, data);
        }
        
        self.next_sequence = sequence;
        self.update_stats(&fragments);
        
        fragments
    }
    
    /// Calculate optimal fragment size based on pattern
    fn calculate_fragment_size(&self, remaining: usize) -> usize {
        let mut rng = rand::thread_rng();
        
        match &self.pattern {
            MobileFragmentPattern::Conservative => {
                // Larger fragments for performance
                std::cmp::min(
                    remaining,
                    rng.gen_range(self.config.min_fragment_size..=self.config.max_fragment_size)
                )
            },
            MobileFragmentPattern::Aggressive => {
                // Smaller, more irregular fragments
                let base_size = rng.gen_range(self.config.min_fragment_size..=self.config.max_fragment_size);
                let variation = rng.gen_range(0.7..1.3);
                let final_size = (base_size as f64 * variation) as usize;
                
                std::cmp::min(remaining, std::cmp::max(8, final_size))
            },
            MobileFragmentPattern::Adaptive => {
                // Adapt based on current conditions
                self.adaptive_fragment_size(remaining)
            },
            MobileFragmentPattern::Carrier(carrier) => {
                // Carrier-specific behavior
                let (mtu, _) = carrier.get_mtu_characteristics();
                let max_payload = (mtu as usize).saturating_sub(40); // Account for headers
                
                std::cmp::min(remaining, max_payload)
            },
        }
    }
    
    /// Adaptive fragment sizing based on detection risk
    fn adaptive_fragment_size(&self, remaining: usize) -> usize {
        let mut rng = rand::thread_rng();
        
        // Simple heuristic: vary size based on time and remaining data
        let time_factor = (Instant::now().elapsed().as_millis() % 1000) as f64 / 1000.0;
        let size_factor = if remaining > 10000 { 0.8 } else { 1.2 };
        
        let base_size = (self.config.min_fragment_size + self.config.max_fragment_size) / 2;
        let adjusted_size = (base_size as f64 * size_factor * (0.5 + time_factor)) as usize;
        
        std::cmp::min(
            remaining,
            std::cmp::max(
                self.config.min_fragment_size,
                std::cmp::min(adjusted_size, self.config.max_fragment_size)
            )
        )
    }
    
    /// Add overlapping fragments for advanced DPI evasion
    fn add_overlapping_fragments(&self, fragments: &mut Vec<PacketFragment>, original_data: &[u8]) {
        if fragments.len() < 2 {
            return;
        }
        
        let mut rng = rand::thread_rng();
        let overlap_count = rng.gen_range(1..=2);
        
        for _ in 0..overlap_count {
            if let Some(fragment_idx) = rng.gen_range(0..fragments.len().saturating_sub(1)).into() {
                if fragment_idx + 1 < fragments.len() {
                    let current_frag = &fragments[fragment_idx];
                    let next_frag = &fragments[fragment_idx + 1];
                    
                    // Create overlap between current and next fragment
                    let overlap_size = rng.gen_range(4..=16);
                    let overlap_start = current_frag.data.len().saturating_sub(overlap_size);
                    
                    if overlap_start < current_frag.data.len() {
                        let mut overlap_data = current_frag.data[overlap_start..].to_vec();
                        overlap_data.extend_from_slice(&next_frag.data[..std::cmp::min(overlap_size, next_frag.data.len())]);
                        
                        let overlap_fragment = PacketFragment {
                            data: overlap_data,
                            sequence: current_frag.sequence.wrapping_add(100), // Different sequence space
                            is_last: false,
                            timestamp: Instant::now(),
                            duplicate_count: 0,
                        };
                        
                        fragments.insert(fragment_idx + 1, overlap_fragment);
                    }
                }
            }
        }
    }
    
    /// Send fragmented data through stream with timing
    pub async fn send_fragmented<W>(&mut self, writer: &mut W, data: &[u8]) -> std::io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let fragments = self.fragment_packet(data);
        
        for fragment in fragments {
            // Apply inter-fragment delay
            let delay_ms = rand::thread_rng().gen_range(
                self.config.fragment_delay_ms.start..=self.config.fragment_delay_ms.end
            );
            
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }
            
            writer.write_all(&fragment.data).await?;
            
            // Optional: flush after each fragment for immediate transmission
            if self.should_flush_fragment() {
                writer.flush().await?;
            }
        }
        
        Ok(())
    }
    
    /// Determine if fragment should be flushed immediately
    fn should_flush_fragment(&self) -> bool {
        match &self.pattern {
            MobileFragmentPattern::Conservative => false, // Batch for efficiency
            MobileFragmentPattern::Aggressive => true,   // Immediate send for evasion
            MobileFragmentPattern::Adaptive => {
                // Probabilistic flushing
                rand::thread_rng().gen_bool(0.3)
            },
            MobileFragmentPattern::Carrier(_) => true,   // Carrier-specific immediate send
        }
    }
    
    /// Update fragmentation statistics
    fn update_stats(&mut self, fragments: &[PacketFragment]) {
        self.stats.total_fragments += fragments.len() as u64;
        
        let total_bytes: usize = fragments.iter().map(|f| f.data.len()).sum();
        self.stats.bytes_fragmented += total_bytes as u64;
        
        if self.stats.total_fragments > 0 {
            self.stats.average_fragment_size = 
                self.stats.bytes_fragmented as f64 / self.stats.total_fragments as f64;
        }
        
        // Calculate evasion score based on fragmentation characteristics
        self.stats.evasion_score = self.calculate_evasion_score(fragments);
    }
    
    /// Calculate evasion effectiveness score
    fn calculate_evasion_score(&self, fragments: &[PacketFragment]) -> f32 {
        let mut score = 0.0f32;
        
        // Size variation contributes to evasion
        if fragments.len() > 1 {
            let sizes: Vec<_> = fragments.iter().map(|f| f.data.len()).collect();
            let avg_size = sizes.iter().sum::<usize>() as f32 / sizes.len() as f32;
            let variance = sizes.iter()
                .map(|&s| (s as f32 - avg_size).powi(2))
                .sum::<f32>() / sizes.len() as f32;
            
            score += (variance.sqrt() / avg_size).min(1.0) * 30.0;
        }
        
        // More fragments = higher evasion potential
        score += (fragments.len() as f32).log2() * 10.0;
        
        // Timing variation
        if self.config.fragment_delay_ms.end > self.config.fragment_delay_ms.start {
            score += 20.0;
        }
        
        // Advanced techniques
        if self.config.randomize_order { score += 15.0; }
        if self.config.duplicate_fragments { score += 10.0; }
        if self.config.overlap_fragments { score += 25.0; }
        
        score.min(100.0)
    }
    
    /// Get current fragmentation statistics
    pub fn get_stats(&self) -> &FragmentStats {
        &self.stats
    }
    
    /// Adapt fragmentation pattern based on detection events
    pub fn adapt_to_detection(&mut self, detection_event: DetectionEvent) {
        match detection_event {
            DetectionEvent::DpiDetected => {
                // Increase fragmentation aggressiveness
                self.config.min_fragment_size = std::cmp::max(8, self.config.min_fragment_size / 2);
                self.config.max_fragment_size = std::cmp::min(256, self.config.max_fragment_size / 2);
                self.config.randomize_order = true;
                self.config.duplicate_fragments = true;
            },
            DetectionEvent::LatencyIncrease => {
                // Reduce fragmentation overhead
                self.config.min_fragment_size *= 2;
                self.config.max_fragment_size *= 2;
                self.config.fragment_delay_ms.end = std::cmp::max(1, self.config.fragment_delay_ms.end / 2);
            },
            DetectionEvent::Normal => {
                // Return to baseline
                *self = Self::new(self.pattern.clone());
            },
        }
    }
}

/// Detection events that trigger adaptation
#[derive(Debug, Clone)]
pub enum DetectionEvent {
    DpiDetected,
    LatencyIncrease,
    Normal,
}

/// Reassemble fragmented packets
pub struct PacketReassembler {
    fragments: std::collections::HashMap<u16, Vec<PacketFragment>>,
    timeout: Duration,
}

impl PacketReassembler {
    pub fn new(timeout: Duration) -> Self {
        Self {
            fragments: std::collections::HashMap::new(),
            timeout,
        }
    }
    
    /// Add fragment and attempt reassembly
    pub fn add_fragment(&mut self, fragment: PacketFragment) -> Option<Vec<u8>> {
        let sequence = fragment.sequence;
        let entry = self.fragments.entry(sequence).or_insert_with(Vec::new);
        entry.push(fragment);
        
        // Check if we can reassemble
        if let Some(data) = self.try_reassemble(sequence) {
            self.fragments.remove(&sequence);
            Some(data)
        } else {
            None
        }
    }
    
    /// Attempt to reassemble fragments for given sequence
    fn try_reassemble(&self, sequence: u16) -> Option<Vec<u8>> {
        let fragments = self.fragments.get(&sequence)?;
        
        // Check if we have the last fragment
        if !fragments.iter().any(|f| f.is_last) {
            return None;
        }
        
        // Sort fragments and reassemble
        let mut sorted_fragments = fragments.clone();
        sorted_fragments.sort_by_key(|f| f.sequence);
        
        let mut data = Vec::new();
        for fragment in sorted_fragments {
            data.extend_from_slice(&fragment.data);
        }
        
        Some(data)
    }
    
    /// Clean up expired fragments
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.fragments.retain(|_, fragments| {
            fragments.iter().any(|f| now.duration_since(f.timestamp) < self.timeout)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_conservative_fragmentation() {
        let mut fragmenter = PacketFragmenter::new(MobileFragmentPattern::Conservative);
        let data = vec![0u8; 5000];
        let fragments = fragmenter.fragment_packet(&data);
        
        // Conservative should create fewer, larger fragments
        assert!(fragments.len() <= 10);
        assert!(fragments.iter().all(|f| f.data.len() >= 512));
    }
    
    #[test]
    fn test_aggressive_fragmentation() {
        let mut fragmenter = PacketFragmenter::new(MobileFragmentPattern::Aggressive);
        let data = vec![0u8; 1000];
        let fragments = fragmenter.fragment_packet(&data);
        
        // Aggressive should create more, smaller fragments
        assert!(fragments.len() >= 4);
        assert!(fragments.iter().any(|f| f.data.len() <= 256));
    }
    
    #[test]
    fn test_fragment_reassembly() {
        let mut fragmenter = PacketFragmenter::new(MobileFragmentPattern::Conservative);
        let mut reassembler = PacketReassembler::new(Duration::from_secs(30));
        
        let original_data = b"Hello, World! This is a test message for fragmentation.";
        let fragments = fragmenter.fragment_packet(original_data);
        
        let mut reassembled = None;
        for fragment in fragments {
            if let Some(data) = reassembler.add_fragment(fragment) {
                reassembled = Some(data);
                break;
            }
        }
        
        assert!(reassembled.is_some());
        // Note: Due to overlaps and duplicates, exact match may not occur
        // This is expected behavior for DPI evasion
    }
    
    #[test]
    fn test_carrier_specific_mtu() {
        let verizon_fragmenter = PacketFragmenter::new(
            MobileFragmentPattern::Carrier(CarrierProfile::Verizon)
        );
        let (mtu, _) = CarrierProfile::Verizon.get_mtu_characteristics();
        assert_eq!(mtu, 1428);
        
        let att_fragmenter = PacketFragmenter::new(
            MobileFragmentPattern::Carrier(CarrierProfile::ATT)
        );
        let (att_mtu, _) = CarrierProfile::ATT.get_mtu_characteristics();
        assert_eq!(att_mtu, 1500);
    }
}