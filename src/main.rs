//! nvdash — A lightweight, native NVIDIA GPU monitor for ML workloads.
//!
//! Built with egui + nvml-wrapper. No web views, no Electron.
//! Polls NVML at a fixed 500ms interval and renders a compact
//! GPU peek widget with metric bars, sparklines, and process summary.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod gpu;
mod ui;

use eframe::egui;
use gpu::{GpuHistory, GpuMonitor, GpuSnapshot};
use std::time::{Duration, Instant};

/// Application state
struct NvDash {
    monitor: GpuMonitor,
    snapshots: Vec<GpuSnapshot>,
    histories: Vec<GpuHistory>,
    last_poll: Instant,
    poll_interval: Duration,
    error_msg: Option<String>,
}

impl NvDash {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let monitor = GpuMonitor::init().expect("Failed to initialize NVML. Is an NVIDIA GPU present?");
        let count = monitor.device_count() as usize;

        let mut snapshots = Vec::with_capacity(count);
        let mut histories = Vec::with_capacity(count);

        for i in 0..count as u32 {
            match monitor.snapshot(i) {
                Ok(snap) => {
                    let mut h = GpuHistory::new();
                    h.push(&snap);
                    histories.push(h);
                    snapshots.push(snap);
                }
                Err(e) => {
                    eprintln!("Warning: failed to read GPU {}: {}", i, e);
                    histories.push(GpuHistory::new());
                    snapshots.push(GpuSnapshot {
                        name: format!("GPU {} (error)", i),
                        index: i,
                        driver_version: String::new(),
                        cuda_version: String::new(),
                        gpu_util: 0,
                        memory_util: 0,
                        vram_used_mb: 0,
                        vram_total_mb: 0,
                        temperature: 0,
                        fan_speed: None,
                        power_draw_w: 0.0,
                        power_limit_w: 0.0,
                        clock_graphics_mhz: 0,
                        clock_memory_mhz: 0,
                        clock_sm_mhz: 0,
                        processes: vec![],
                    });
                }
            }
        }

        Self {
            monitor,
            snapshots,
            histories,
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(500),
            error_msg: None,
        }
    }

    fn poll(&mut self) {
        if self.last_poll.elapsed() < self.poll_interval {
            return;
        }
        self.last_poll = Instant::now();

        for i in 0..self.monitor.device_count() {
            match self.monitor.snapshot(i) {
                Ok(snap) => {
                    let idx = i as usize;
                    if idx < self.histories.len() {
                        self.histories[idx].push(&snap);
                    }
                    if idx < self.snapshots.len() {
                        self.snapshots[idx] = snap;
                    }
                    self.error_msg = None;
                }
                Err(e) => {
                    self.error_msg = Some(format!("GPU {} poll error: {}", i, e));
                }
            }
        }
    }
}

impl eframe::App for NvDash {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::setup_style(ctx);
        self.poll();
        ctx.request_repaint_after(self.poll_interval);

        egui::CentralPanel::default().show(ctx, |main_ui| {
            if let Some(ref err) = self.error_msg {
                main_ui.label(
                    egui::RichText::new(err)
                        .color(ui::ACCENT_RED)
                        .size(10.0),
                );
            }

            for (i, snapshot) in self.snapshots.iter().enumerate() {
                let history = &self.histories[i];

                ui::draw_header(main_ui, snapshot);

                main_ui.separator();
                main_ui.add_space(2.0);

                ui::draw_metric_bars(main_ui, snapshot);

                main_ui.add_space(4.0);
                main_ui.separator();
                main_ui.add_space(2.0);

                ui::draw_mini_sparklines(main_ui, snapshot, history);

                main_ui.add_space(2.0);
                main_ui.separator();
                main_ui.add_space(2.0);

                ui::draw_footer(main_ui, snapshot);

                if i < self.snapshots.len() - 1 {
                    main_ui.add_space(6.0);
                    main_ui.separator();
                    main_ui.add_space(6.0);
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("nvdash")
            .with_inner_size([380.0, 240.0])
            .with_min_inner_size([340.0, 200.0]),
        ..Default::default()
    };

    eframe::run_native(
        "nvdash",
        options,
        Box::new(|cc| Ok(Box::new(NvDash::new(cc)))),
    )
}
