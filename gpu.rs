use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;
use std::collections::VecDeque;

/// Maximum number of history samples to keep (at 500ms poll = ~60s of history)
const MAX_HISTORY: usize = 120;

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub vram_mb: u64,
}

#[derive(Clone, Debug)]
pub struct GpuSnapshot {
    pub name: String,
    pub index: u32,
    pub driver_version: String,
    pub cuda_version: String,

    // Utilization
    pub gpu_util: u32,       // 0-100%
    pub memory_util: u32,    // 0-100%

    // Memory
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,

    // Thermals & Power
    pub temperature: u32,    // Celsius
    pub fan_speed: Option<u32>, // 0-100%, None if not available
    pub power_draw_w: f64,
    pub power_limit_w: f64,

    // Clocks
    pub clock_graphics_mhz: u32,
    pub clock_memory_mhz: u32,
    pub clock_sm_mhz: u32,

    // Processes
    pub processes: Vec<ProcessInfo>,
}

#[derive(Clone, Debug)]
pub struct GpuHistory {
    pub gpu_util: VecDeque<f64>,
    pub vram_used: VecDeque<f64>,
    pub temperature: VecDeque<f64>,
    pub power_draw: VecDeque<f64>,
}

impl GpuHistory {
    pub fn new() -> Self {
        Self {
            gpu_util: VecDeque::with_capacity(MAX_HISTORY),
            vram_used: VecDeque::with_capacity(MAX_HISTORY),
            temperature: VecDeque::with_capacity(MAX_HISTORY),
            power_draw: VecDeque::with_capacity(MAX_HISTORY),
        }
    }

    pub fn push(&mut self, snapshot: &GpuSnapshot) {
        Self::push_val(&mut self.gpu_util, snapshot.gpu_util as f64);
        Self::push_val(&mut self.vram_used, snapshot.vram_used_mb as f64);
        Self::push_val(&mut self.temperature, snapshot.temperature as f64);
        Self::push_val(&mut self.power_draw, snapshot.power_draw_w);
    }

    fn push_val(buf: &mut VecDeque<f64>, val: f64) {
        if buf.len() >= MAX_HISTORY {
            buf.pop_front();
        }
        buf.push_back(val);
    }
}

pub struct GpuMonitor {
    nvml: Nvml,
    device_count: u32,
}

impl GpuMonitor {
    pub fn init() -> Result<Self, NvmlError> {
        let nvml = Nvml::init()?;
        let device_count = nvml.device_count()?;
        Ok(Self { nvml, device_count })
    }

    pub fn device_count(&self) -> u32 {
        self.device_count
    }

    pub fn driver_version(&self) -> String {
        self.nvml.sys_driver_version().unwrap_or_else(|_| "N/A".into())
    }

    pub fn cuda_version(&self) -> String {
        match self.nvml.sys_cuda_driver_version() {
            Ok(v) => {
                let major = v / 1000;
                let minor = (v % 1000) / 10;
                format!("{}.{}", major, minor)
            }
            Err(_) => "N/A".into(),
        }
    }

    pub fn snapshot(&self, index: u32) -> Result<GpuSnapshot, NvmlError> {
        let device = self.nvml.device_by_index(index)?;

        let name = device.name().unwrap_or_else(|_| "Unknown GPU".into());

        let utilization = device.utilization_rates().unwrap_or(
            nvml_wrapper::struct_wrappers::device::Utilization { gpu: 0, memory: 0 },
        );

        let mem_info = device.memory_info()?;

        let temperature = device
            .temperature(TemperatureSensor::Gpu)
            .unwrap_or(0);

        let fan_speed = device.fan_speed(0).ok();

        let power_draw_mw = device.power_usage().unwrap_or(0) as f64;
        let power_limit_mw = device.enforced_power_limit().unwrap_or(0) as f64;

        let clock_graphics = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .unwrap_or(0);
        let clock_memory = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory)
            .unwrap_or(0);
        let clock_sm = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::SM)
            .unwrap_or(0);

        // Collect all PIDs first, then resolve names in one batch
        let mut processes = Vec::new();
        let mut all_pids = Vec::new();

        if let Ok(compute_procs) = device.running_compute_processes() {
            for proc in compute_procs {
                let vram_bytes = match proc.used_gpu_memory {
                    Some(bytes) => bytes,
                    None => 0,
                };
                all_pids.push(proc.pid);
                processes.push(ProcessInfo {
                    pid: proc.pid,
                    name: String::new(), // resolved below
                    vram_mb: vram_bytes / (1024 * 1024),
                });
            }
        }
        if let Ok(gfx_procs) = device.running_graphics_processes() {
            for proc in gfx_procs {
                if processes.iter().any(|p| p.pid == proc.pid) {
                    continue;
                }
                let vram_bytes = match proc.used_gpu_memory {
                    Some(bytes) => bytes,
                    None => 0,
                };
                all_pids.push(proc.pid);
                processes.push(ProcessInfo {
                    pid: proc.pid,
                    name: String::new(),
                    vram_mb: vram_bytes / (1024 * 1024),
                });
            }
        }

        // Batch resolve process names
        resolve_process_names(&mut processes);

        // Sort by VRAM usage descending
        processes.sort_by(|a, b| b.vram_mb.cmp(&a.vram_mb));

        Ok(GpuSnapshot {
            name,
            index,
            driver_version: self.driver_version(),
            cuda_version: self.cuda_version(),
            gpu_util: utilization.gpu,
            memory_util: utilization.memory,
            vram_used_mb: mem_info.used / (1024 * 1024),
            vram_total_mb: mem_info.total / (1024 * 1024),
            temperature,
            fan_speed,
            power_draw_w: power_draw_mw / 1000.0,
            power_limit_w: power_limit_mw / 1000.0,
            clock_graphics_mhz: clock_graphics,
            clock_memory_mhz: clock_memory,
            clock_sm_mhz: clock_sm,
            processes,
        })
    }
}

fn resolve_process_names(processes: &mut [ProcessInfo]) {
    use sysinfo::{Pid, System};
    let pids: Vec<Pid> = processes.iter().map(|p| Pid::from_u32(p.pid)).collect();
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&pids), true);
    for proc in processes.iter_mut() {
        let pid = Pid::from_u32(proc.pid);
        proc.name = sys
            .process(pid)
            .map(|p| p.name().to_string_lossy().to_string())
            .unwrap_or_else(|| format!("PID {}", proc.pid));
    }
}
