use crate::abstractions::ProtocolDetector;

pub struct DetectionOrchestrator {
    detectors: Vec<Box<dyn ProtocolDetector>>
}

impl DetectionOrchestrator {
    pub fn new() -> Self {
        DetectionOrchestrator { detectors: Vec::new() }
    }

    pub fn add_detector(&mut self, detector: Box<dyn ProtocolDetector>) {
        self.detectors.push(detector);
    }
}