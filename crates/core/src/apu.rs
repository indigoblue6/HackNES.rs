//! # APU (Audio Processing Unit)
//! NES APU implementation with 5 channels:
//! - 2 Pulse channels
//! - 1 Triangle channel
//! - 1 Noise channel
//! - 1 DMC channel

const PULSE_DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 75% (inverted 25%)
];

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

const DMC_RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

pub struct Apu {
    // Pulse channels
    pulse1: PulseChannel,
    pulse2: PulseChannel,
    // Triangle channel
    triangle: TriangleChannel,
    // Noise channel
    noise: NoiseChannel,
    // DMC channel
    dmc: DmcChannel,
    // Frame counter
    frame_counter: FrameCounter,
    // Status
    status: u8,
    // Audio output
    pub sample_buffer: Vec<f32>,
    cycles: u64,
    samples_per_frame: usize,
    cycle_rate: f32,
}

#[derive(Default)]
struct PulseChannel {
    enabled: bool,
    duty: u8,
    duty_position: u8,
    length_counter: u8,
    length_halt: bool,
    constant_volume: bool,
    volume: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,
    timer: u16,
    timer_period: u16,
    is_pulse2: bool,
}

#[derive(Default)]
struct TriangleChannel {
    enabled: bool,
    length_counter: u8,
    length_halt: bool,
    linear_counter: u8,
    linear_counter_reload: u8,
    linear_counter_reload_flag: bool,
    timer: u16,
    timer_period: u16,
    sequence_position: u8,
}

#[derive(Default)]
struct NoiseChannel {
    enabled: bool,
    length_counter: u8,
    length_halt: bool,
    constant_volume: bool,
    volume: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
    mode: bool,
    timer: u16,
    timer_period: u16,
    shift_register: u16,
}

#[derive(Default)]
struct DmcChannel {
    enabled: bool,
    irq_enabled: bool,
    irq_pending: bool,
    loop_flag: bool,
    rate: u16,
    timer: u16,
    output_level: u8,
    sample_address: u16,
    sample_length: u16,
    current_address: u16,
    bytes_remaining: u16,
    sample_buffer: Option<u8>,
    shift_register: u8,
    bits_remaining: u8,
    silence: bool,
}

#[derive(Default)]
struct FrameCounter {
    mode: bool, // false = 4-step, true = 5-step
    irq_inhibit: bool,
    irq_pending: bool,
    step: u8,
    divider: u16,
}

impl Apu {
    pub fn new() -> Self {
        const SAMPLE_RATE: f32 = 44100.0;
        const CPU_FREQUENCY: f32 = 1789773.0;
        let samples_per_frame = (SAMPLE_RATE / 60.0) as usize;

        Self {
            pulse1: PulseChannel::new(false),
            pulse2: PulseChannel::new(true),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            dmc: DmcChannel::new(),
            frame_counter: FrameCounter::new(),
            status: 0,
            sample_buffer: Vec::with_capacity(samples_per_frame),
            cycles: 0,
            samples_per_frame,
            cycle_rate: CPU_FREQUENCY / SAMPLE_RATE,
        }
    }

    pub fn tick(&mut self) {
        self.cycles += 1;

        // Clock triangle timer every CPU cycle
        self.triangle.clock_timer();

        // Clock other timers every other CPU cycle
        if self.cycles % 2 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
            self.noise.clock_timer();
            self.dmc.clock_timer();
        }

        // Frame counter (approximately 240Hz)
        self.frame_counter.divider += 1;
        if self.frame_counter.divider >= 7457 {
            self.frame_counter.divider = 0;
            self.clock_frame_counter();
        }

        // Generate sample
        let sample_cycle = (self.cycles as f32 / self.cycle_rate) as usize;
        if sample_cycle > self.sample_buffer.len() && self.sample_buffer.len() < self.samples_per_frame {
            let sample = self.mix_output();
            self.sample_buffer.push(sample);
        }
    }

    fn clock_frame_counter(&mut self) {
        let step = self.frame_counter.step;
        self.frame_counter.step = (step + 1) % if self.frame_counter.mode { 5 } else { 4 };

        // Quarter frame (envelope and linear counter)
        self.pulse1.clock_envelope();
        self.pulse2.clock_envelope();
        self.triangle.clock_linear_counter();
        self.noise.clock_envelope();

        // Half frame (length counter and sweep)
        if (!self.frame_counter.mode && (step == 1 || step == 3))
            || (self.frame_counter.mode && (step == 1 || step == 4))
        {
            self.pulse1.clock_length();
            self.pulse2.clock_length();
            self.triangle.clock_length();
            self.noise.clock_length();

            self.pulse1.clock_sweep();
            self.pulse2.clock_sweep();
        }

        // IRQ
        if !self.frame_counter.mode && step == 3 && !self.frame_counter.irq_inhibit {
            self.frame_counter.irq_pending = true;
        }
    }

    fn mix_output(&self) -> f32 {
        let pulse1 = if self.pulse1.enabled && self.pulse1.length_counter > 0 {
            self.pulse1.output() as f32
        } else {
            0.0
        };
        let pulse2 = if self.pulse2.enabled && self.pulse2.length_counter > 0 {
            self.pulse2.output() as f32
        } else {
            0.0
        };
        let triangle = if self.triangle.enabled && self.triangle.length_counter > 0 {
            self.triangle.output() as f32
        } else {
            0.0
        };
        let noise = if self.noise.enabled && self.noise.length_counter > 0 {
            self.noise.output() as f32
        } else {
            0.0
        };
        let dmc = self.dmc.output_level as f32;

        // Non-linear mixing approximation
        let pulse_out = 0.00752 * (pulse1 + pulse2);
        let tnd_out = 0.00851 * triangle + 0.00494 * noise + 0.00335 * dmc;

        pulse_out + tnd_out
    }

    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut status = 0u8;
                if self.pulse1.length_counter > 0 {
                    status |= 0x01;
                }
                if self.pulse2.length_counter > 0 {
                    status |= 0x02;
                }
                if self.triangle.length_counter > 0 {
                    status |= 0x04;
                }
                if self.noise.length_counter > 0 {
                    status |= 0x08;
                }
                if self.dmc.bytes_remaining > 0 {
                    status |= 0x10;
                }
                if self.frame_counter.irq_pending {
                    status |= 0x40;
                }
                if self.dmc.irq_pending {
                    status |= 0x80;
                }
                // Reading $4015 clears frame interrupt flag
                self.frame_counter.irq_pending = false;
                status
            }
            _ => 0,
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr {
            // Pulse 1
            0x4000 => {
                self.pulse1.duty = (value >> 6) & 0x03;
                self.pulse1.length_halt = (value & 0x20) != 0;
                self.pulse1.constant_volume = (value & 0x10) != 0;
                self.pulse1.volume = value & 0x0F;
            }
            0x4001 => {
                self.pulse1.sweep_enabled = (value & 0x80) != 0;
                self.pulse1.sweep_period = (value >> 4) & 0x07;
                self.pulse1.sweep_negate = (value & 0x08) != 0;
                self.pulse1.sweep_shift = value & 0x07;
                self.pulse1.sweep_reload = true;
            }
            0x4002 => {
                self.pulse1.timer_period =
                    (self.pulse1.timer_period & 0x0700) | (value as u16);
            }
            0x4003 => {
                self.pulse1.timer_period =
                    (self.pulse1.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.pulse1.envelope_start = true;
                self.pulse1.duty_position = 0;
            }
            // Pulse 2
            0x4004 => {
                self.pulse2.duty = (value >> 6) & 0x03;
                self.pulse2.length_halt = (value & 0x20) != 0;
                self.pulse2.constant_volume = (value & 0x10) != 0;
                self.pulse2.volume = value & 0x0F;
            }
            0x4005 => {
                self.pulse2.sweep_enabled = (value & 0x80) != 0;
                self.pulse2.sweep_period = (value >> 4) & 0x07;
                self.pulse2.sweep_negate = (value & 0x08) != 0;
                self.pulse2.sweep_shift = value & 0x07;
                self.pulse2.sweep_reload = true;
            }
            0x4006 => {
                self.pulse2.timer_period =
                    (self.pulse2.timer_period & 0x0700) | (value as u16);
            }
            0x4007 => {
                self.pulse2.timer_period =
                    (self.pulse2.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.pulse2.envelope_start = true;
                self.pulse2.duty_position = 0;
            }
            // Triangle
            0x4008 => {
                self.triangle.length_halt = (value & 0x80) != 0;
                self.triangle.linear_counter_reload = value & 0x7F;
            }
            0x400A => {
                self.triangle.timer_period =
                    (self.triangle.timer_period & 0x0700) | (value as u16);
            }
            0x400B => {
                self.triangle.timer_period =
                    (self.triangle.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.triangle.linear_counter_reload_flag = true;
            }
            // Noise
            0x400C => {
                self.noise.length_halt = (value & 0x20) != 0;
                self.noise.constant_volume = (value & 0x10) != 0;
                self.noise.volume = value & 0x0F;
            }
            0x400E => {
                self.noise.mode = (value & 0x80) != 0;
                self.noise.timer_period = NOISE_PERIOD_TABLE[(value & 0x0F) as usize];
            }
            0x400F => {
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.noise.envelope_start = true;
            }
            // DMC
            0x4010 => {
                self.dmc.irq_enabled = (value & 0x80) != 0;
                self.dmc.loop_flag = (value & 0x40) != 0;
                self.dmc.rate = DMC_RATE_TABLE[(value & 0x0F) as usize];
                if !self.dmc.irq_enabled {
                    self.dmc.irq_pending = false;
                }
            }
            0x4011 => {
                self.dmc.output_level = value & 0x7F;
            }
            0x4012 => {
                self.dmc.sample_address = 0xC000 + (value as u16 * 64);
            }
            0x4013 => {
                self.dmc.sample_length = (value as u16 * 16) + 1;
            }
            // Status
            0x4015 => {
                self.pulse1.enabled = (value & 0x01) != 0;
                self.pulse2.enabled = (value & 0x02) != 0;
                self.triangle.enabled = (value & 0x04) != 0;
                self.noise.enabled = (value & 0x08) != 0;
                self.dmc.enabled = (value & 0x10) != 0;

                if !self.pulse1.enabled {
                    self.pulse1.length_counter = 0;
                }
                if !self.pulse2.enabled {
                    self.pulse2.length_counter = 0;
                }
                if !self.triangle.enabled {
                    self.triangle.length_counter = 0;
                }
                if !self.noise.enabled {
                    self.noise.length_counter = 0;
                }
                if !self.dmc.enabled {
                    self.dmc.bytes_remaining = 0;
                } else if self.dmc.bytes_remaining == 0 {
                    self.dmc.current_address = self.dmc.sample_address;
                    self.dmc.bytes_remaining = self.dmc.sample_length;
                }
                self.dmc.irq_pending = false;
                self.status = value;
            }
            // Frame counter
            0x4017 => {
                self.frame_counter.mode = (value & 0x80) != 0;
                self.frame_counter.irq_inhibit = (value & 0x40) != 0;
                if self.frame_counter.irq_inhibit {
                    self.frame_counter.irq_pending = false;
                }
                self.frame_counter.step = 0;
                self.frame_counter.divider = 0;
                // Immediately clock if 5-step mode
                if self.frame_counter.mode {
                    self.clock_frame_counter();
                }
            }
            _ => {}
        }
    }

    pub fn get_samples(&mut self) -> Vec<f32> {
        let samples = std::mem::take(&mut self.sample_buffer);
        self.sample_buffer = Vec::with_capacity(self.samples_per_frame);
        samples
    }

    pub fn reset(&mut self) {
        self.sample_buffer.clear();
        self.cycles = 0;
    }

    pub fn irq_pending(&self) -> bool {
        self.frame_counter.irq_pending || self.dmc.irq_pending
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl PulseChannel {
    fn new(is_pulse2: bool) -> Self {
        Self {
            is_pulse2,
            ..Default::default()
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.duty_position = (self.duty_position + 1) % 8;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if self.length_halt {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn clock_sweep(&mut self) {
        if self.sweep_reload {
            if self.sweep_enabled && self.sweep_divider == 0 {
                self.update_sweep();
            }
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else if self.sweep_divider > 0 {
            self.sweep_divider -= 1;
        } else {
            if self.sweep_enabled {
                self.update_sweep();
            }
            self.sweep_divider = self.sweep_period;
        }
    }

    fn update_sweep(&mut self) {
        let delta = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            self.timer_period = self.timer_period.saturating_sub(delta);
            if self.is_pulse2 {
                // Pulse 2 doesn't subtract the additional 1
            } else {
                self.timer_period = self.timer_period.saturating_sub(1);
            }
        } else {
            self.timer_period = self.timer_period.saturating_add(delta);
        }
    }

    fn output(&self) -> u8 {
        if self.timer_period < 8 || self.timer_period > 0x7FF {
            return 0;
        }
        if PULSE_DUTY_TABLE[self.duty as usize][self.duty_position as usize] == 0 {
            return 0;
        }
        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

impl TriangleChannel {
    fn new() -> Self {
        Self::default()
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.sequence_position = (self.sequence_position + 1) % 32;
            }
        } else {
            self.timer -= 1;
        }
    }

    fn clock_linear_counter(&mut self) {
        if self.linear_counter_reload_flag {
            self.linear_counter = self.linear_counter_reload;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.length_halt {
            self.linear_counter_reload_flag = false;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.timer_period < 2 {
            return 7; // Prevent ultrasonic frequencies
        }
        TRIANGLE_SEQUENCE[self.sequence_position as usize]
    }
}

impl NoiseChannel {
    fn new() -> Self {
        Self {
            shift_register: 1,
            ..Default::default()
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            let feedback = if self.mode {
                (self.shift_register & 1) ^ ((self.shift_register >> 6) & 1)
            } else {
                (self.shift_register & 1) ^ ((self.shift_register >> 1) & 1)
            };
            self.shift_register = (self.shift_register >> 1) | (feedback << 14);
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if self.length_halt {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if (self.shift_register & 1) != 0 {
            return 0;
        }
        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

impl DmcChannel {
    fn new() -> Self {
        Self {
            rate: DMC_RATE_TABLE[0],
            ..Default::default()
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.rate;
            if !self.silence {
                if (self.shift_register & 1) != 0 {
                    if self.output_level <= 125 {
                        self.output_level += 2;
                    }
                } else if self.output_level >= 2 {
                    self.output_level -= 2;
                }
                self.shift_register >>= 1;
            }
            self.bits_remaining = self.bits_remaining.saturating_sub(1);
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                if let Some(sample) = self.sample_buffer.take() {
                    self.silence = false;
                    self.shift_register = sample;
                } else {
                    self.silence = true;
                }
            }
        } else {
            self.timer -= 1;
        }
    }
}

impl FrameCounter {
    fn new() -> Self {
        Self::default()
    }
}
