//! Delay with BPM sync and feedback

/// Simple ping-pong delay with feedback
pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    max_delay_samples: usize,
}

impl Delay {
    pub fn new(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_samples = (sample_rate * max_delay_ms * 0.001) as usize;
        // Power of 2 for efficient modulo
        let size = max_samples.next_power_of_two();
        Self {
            buffer: vec![0.0; size],
            write_pos: 0,
            sample_rate,
            max_delay_samples: max_samples,
        }
    }

    /// Process one sample with delay time in ms and feedback
    #[inline]
    pub fn process_sample(&mut self, input: f32, delay_ms: f32, feedback: f32, mix: f32) -> f32 {
        let delay_samples = ((delay_ms * 0.001 * self.sample_rate) as usize).min(self.max_delay_samples);
        let read_pos = (self.write_pos + self.buffer.len() - delay_samples) % self.buffer.len();
        
        let delayed = self.buffer[read_pos];
        
        // Write input + feedback * delayed
        self.buffer[self.write_pos] = input + delayed * feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        
        // Dry/wet mix
        input * (1.0 - mix) + delayed * mix
    }

    /// Process block (mono)
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32], delay_ms: f32, feedback: f32, mix: f32) {
        for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
            *out_sample = self.process_sample(*in_sample, delay_ms, feedback, mix);
        }
    }

    /// Clear delay buffer
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}