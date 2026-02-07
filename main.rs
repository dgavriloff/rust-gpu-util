//! nvdash — A lightweight, native NVIDIA GPU monitor for ML workloads.
//!
//! Built with egui + nvml-wrapper. No web views, no Electron.
//! Polls NVML at a configurable interval and renders real-time
//! gauges, sparklines, clocks, and a per-process VRAM table.

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
    always_on_top: bool,
    show_clocks: bool,
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
                    // Push a default snapshot
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
            always_on_top: false,
            show_clocks: true,
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
        // Apply theme on every frame (cheap, ensures consistency)
        ui::setup_style(ctx);

        // Poll GPU data
        self.poll();

        // Request repaint at poll interval
        ctx.request_repaint_after(self.poll_interval);

        // Top menu bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |bar_ui| {
            egui::menu::bar(bar_ui, |bar_ui| {
                bar_ui.label(
                    egui::RichText::new("nvdash")
                        .color(ui::NVIDIA_GREEN)
                        .size(13.0)
                        .strong(),
                );

                bar_ui.separator();

                // Poll rate selector
                bar_ui.label(
                    egui::RichText::new("Poll:")
                        .color(ui::TEXT_DIM)
                        .size(10.0),
                );

                let poll_ms = self.poll_interval.as_millis() as u64;
                for &ms in &[250u64, 500, 1000, 2000] {
                    let label = format!("{}ms", ms);
                    let btn = bar_ui.selectable_label(
                        poll_ms == ms,
                        egui::RichText::new(&label).size(10.0),
                    );
                    if btn.clicked() {
                        self.poll_interval = Duration::from_millis(ms);
                    }
                }

                bar_ui.separator();

                if bar_ui
                    .selectable_label(
                        self.show_clocks,
                        egui::RichText::new("Clocks").size(10.0),
                    )
                    .clicked()
                {
                    self.show_clocks = !self.show_clocks;
                }

                // Right-align error message
                bar_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref err) = self.error_msg {
                        ui.label(
                            egui::RichText::new(err)
                                .color(ui::ACCENT_RED)
                                .size(10.0),
                        );
                    }
                });
            });
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |main_ui| {
            egui::ScrollArea::vertical().show(main_ui, |scroll_ui| {
                for (i, snapshot) in self.snapshots.iter().enumerate() {
                    let history = &self.histories[i];

                    // GPU panel frame
                    egui::Frame::new()
                        .fill(ui::BG_PANEL)
                        .stroke(egui::Stroke::new(1.0, ui::BORDER))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(scroll_ui, |panel_ui| {
                            ui::draw_header(panel_ui, snapshot);

                            panel_ui.separator();
                            panel_ui.add_space(4.0);

                            // Gauges row
                            ui::draw_gauges(panel_ui, snapshot);

                            panel_ui.add_space(8.0);

                            // Clocks
                            if self.show_clocks {
                                ui::draw_clocks(panel_ui, snapshot);
                                panel_ui.add_space(8.0);
                            }

                            // Sparklines
                            ui::draw_sparklines(panel_ui, snapshot, history);

                            panel_ui.add_space(8.0);
                            panel_ui.separator();
                            panel_ui.add_space(4.0);

                            // Process table
                            ui::draw_process_table(panel_ui, snapshot);
                        });

                    scroll_ui.add_space(8.0);
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("nvdash")
            .with_inner_size([480.0, 720.0])
            .with_min_inner_size([360.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "nvdash",
        options,
        Box::new(|cc| Ok(Box::new(NvDash::new(cc)))),
    )
}
