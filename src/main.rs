//! nvdash — A lightweight, native NVIDIA GPU monitor for ML workloads.
//!
//! Built with egui + nvml-wrapper. No web views, no Electron.
//! Polls NVML at a fixed 500ms interval and renders a compact
//! GPU peek widget with metric bars, sparklines, and process summary.
//! Lives in the system tray; click to toggle, right-click to quit.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod gpu;
mod ui;

use eframe::egui;
use gpu::{GpuHistory, GpuMonitor, GpuSnapshot};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
use winapi::shared::windef::HWND;

/// Shared state between the tray click handler and the egui app.
#[cfg(windows)]
struct Shared {
    hwnd: HWND,
    visible: bool,
}

#[cfg(windows)]
unsafe impl Send for Shared {}
#[cfg(windows)]
unsafe impl Sync for Shared {}

/// Application state
struct NvDash {
    monitor: GpuMonitor,
    snapshots: Vec<GpuSnapshot>,
    histories: Vec<GpuHistory>,
    last_poll: Instant,
    poll_interval: Duration,
    poll_ms: u64,
    always_on_top: bool,
    decorations: bool,
    opacity_pct: u8,
    error_msg: Option<String>,
    #[cfg(windows)]
    shared: Arc<Mutex<Shared>>,
    #[cfg(windows)]
    hwnd_captured: bool,
}

impl NvDash {
    #[cfg(windows)]
    fn new(_cc: &eframe::CreationContext<'_>, shared: Arc<Mutex<Shared>>) -> Self {
        let monitor =
            GpuMonitor::init().expect("Failed to initialize NVML. Is an NVIDIA GPU present?");
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
            poll_ms: 500,
            always_on_top: false,
            decorations: true,
            opacity_pct: 100,
            error_msg: None,
            shared,
            hwnd_captured: false,
        }
    }

    #[cfg(not(windows))]
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let monitor =
            GpuMonitor::init().expect("Failed to initialize NVML. Is an NVIDIA GPU present?");
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
            poll_ms: 500,
            always_on_top: false,
            decorations: true,
            opacity_pct: 100,
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
        // Capture HWND on the first frame and apply WS_EX_TOOLWINDOW
        #[cfg(windows)]
        if !self.hwnd_captured {
            use winapi::um::winuser::{
                GetForegroundWindow, GetWindowLongW, SetWindowLongW, GWL_EXSTYLE,
                WS_EX_TOOLWINDOW,
            };
            unsafe {
                let hwnd = GetForegroundWindow();
                if !hwnd.is_null() {
                    // Store HWND in shared state for the tray click handler
                    if let Ok(mut s) = self.shared.lock() {
                        s.hwnd = hwnd;
                        s.visible = true;
                    }
                    // Remove from taskbar by adding WS_EX_TOOLWINDOW
                    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW as i32);
                    self.hwnd_captured = true;
                }
            }
        }

        // Handle close request: hide to tray instead of quitting
        #[cfg(windows)]
        {
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if let Ok(s) = self.shared.lock() {
                    if !s.hwnd.is_null() {
                        unsafe {
                            winapi::um::winuser::ShowWindow(
                                s.hwnd,
                                winapi::um::winuser::SW_HIDE,
                            );
                        }
                    }
                }
                if let Ok(mut s) = self.shared.lock() {
                    s.visible = false;
                }
            }
        }

        // Handle tray icon events
        #[cfg(windows)]
        {
            use tray_icon::TrayIconEvent;

            if let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    rect,
                    ..
                } = event
                {
                    if let Ok(mut s) = self.shared.lock() {
                        if !s.hwnd.is_null() {
                            unsafe {
                                use winapi::um::winuser::*;
                                if s.visible {
                                    ShowWindow(s.hwnd, SW_HIDE);
                                    s.visible = false;
                                } else {
                                    let x = rect.position.x as i32
                                        + (rect.size.width as i32 / 2)
                                        - (380 / 2);
                                    let y = rect.position.y as i32 - 260;
                                    SetWindowPos(
                                        s.hwnd,
                                        HWND_TOPMOST,
                                        x,
                                        y,
                                        0,
                                        0,
                                        SWP_NOSIZE | SWP_SHOWWINDOW,
                                    );
                                    SetForegroundWindow(s.hwnd);
                                    s.visible = true;
                                }
                            }
                        }
                    }
                }
            }

            // Handle menu events (Quit)
            use tray_icon::menu::MenuEvent;
            if let Ok(_event) = MenuEvent::receiver().try_recv() {
                std::process::exit(0);
            }
        }

        ui::setup_style(ctx);
        self.poll();
        ctx.request_repaint_after(self.poll_interval);

        egui::TopBottomPanel::bottom("poll_bar")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(ui::BG_DARK)
                    .inner_margin(egui::Margin::symmetric(4, 0)),
            )
            .show(ctx, |bar_ui| {
                bar_ui.horizontal(|ui| {
                    // Left side: toggles
                    let pin_label = if self.always_on_top { "pinned" } else { "pin" };
                    if ui
                        .selectable_label(
                            self.always_on_top,
                            egui::RichText::new(pin_label)
                                .size(10.0)
                                .color(ui::TEXT_SECONDARY),
                        )
                        .clicked()
                    {
                        self.always_on_top = !self.always_on_top;
                        let level = if self.always_on_top {
                            egui::viewport::WindowLevel::AlwaysOnTop
                        } else {
                            egui::viewport::WindowLevel::Normal
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                    }

                    let frame_label = if self.decorations {
                        "frame"
                    } else {
                        "frameless"
                    };
                    if ui
                        .selectable_label(
                            !self.decorations,
                            egui::RichText::new(frame_label)
                                .size(10.0)
                                .color(ui::TEXT_SECONDARY),
                        )
                        .clicked()
                    {
                        self.decorations = !self.decorations;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(
                            self.decorations,
                        ));
                    }

                    // Right side: poll rate + opacity
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let poll_label = format!("{}ms", self.poll_ms);
                            egui::ComboBox::from_id_salt("poll_rate")
                                .selected_text(
                                    egui::RichText::new(&poll_label)
                                        .size(10.0)
                                        .font(egui::FontId::monospace(10.0))
                                        .color(ui::TEXT_SECONDARY),
                                )
                                .width(50.0)
                                .show_ui(ui, |ui| {
                                    for &ms in &[250u64, 500, 1000, 2000] {
                                        let text = format!("{}ms", ms);
                                        if ui
                                            .selectable_value(&mut self.poll_ms, ms, &text)
                                            .changed()
                                        {
                                            self.poll_interval = Duration::from_millis(ms);
                                        }
                                    }
                                });
                            ui.label(
                                egui::RichText::new("Poll:")
                                    .size(10.0)
                                    .color(ui::TEXT_SECONDARY),
                            );

                            ui.add_space(4.0);

                            let opacity_label = format!("{}%", self.opacity_pct);
                            egui::ComboBox::from_id_salt("opacity")
                                .selected_text(
                                    egui::RichText::new(&opacity_label)
                                        .size(10.0)
                                        .font(egui::FontId::monospace(10.0))
                                        .color(ui::TEXT_SECONDARY),
                                )
                                .width(42.0)
                                .show_ui(ui, |ui| {
                                    for pct in (10..=100).step_by(10) {
                                        let text = format!("{}%", pct);
                                        if ui
                                            .selectable_value(&mut self.opacity_pct, pct, &text)
                                            .changed()
                                        {
                                            set_window_opacity(pct);
                                        }
                                    }
                                });
                            ui.label(
                                egui::RichText::new("Opacity:")
                                    .size(10.0)
                                    .color(ui::TEXT_SECONDARY),
                            );
                        },
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ui::BG_DARK)
                    .inner_margin(egui::Margin::same(4))
                    .outer_margin(egui::Margin::ZERO),
            )
            .show(ctx, |main_ui| {
                main_ui.style_mut().spacing.item_spacing.y = 2.0;
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
                    ui::draw_text_sparklines(main_ui, snapshot, history);
                    ui::draw_temp_bar(main_ui, snapshot);
                    main_ui.separator();
                    ui::draw_process_list(main_ui, snapshot);
                    main_ui.separator();
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

#[cfg(windows)]
fn set_window_opacity(pct: u8) {
    use winapi::um::winuser::{
        GetForegroundWindow, GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW,
        GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as i32);
            let alpha = (pct as f32 / 100.0 * 255.0) as u8;
            SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
        }
    }
}

#[cfg(not(windows))]
fn set_window_opacity(_pct: u8) {
    // Not supported on this platform
}

fn main() -> eframe::Result<()> {
    // Create the tray icon (Windows only)
    #[cfg(windows)]
    let shared = {
        use tray_icon::menu::{Menu, MenuItem};
        use tray_icon::TrayIconBuilder;

        // 16x16 NVIDIA green RGBA icon
        let mut icon_rgba = Vec::with_capacity(16 * 16 * 4);
        for _ in 0..(16 * 16) {
            icon_rgba.push(118); // R
            icon_rgba.push(185); // G
            icon_rgba.push(0); // B
            icon_rgba.push(255); // A
        }
        let icon = tray_icon::Icon::from_rgba(icon_rgba, 16, 16).expect("Failed to create icon");

        // Right-click menu with "Quit"
        let menu = Menu::new();
        let quit_item = MenuItem::new("Quit", true, None);
        menu.append(&quit_item).expect("Failed to add menu item");

        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("nvdash")
            .with_icon(icon)
            .build()
            .expect("Failed to create tray icon");

        // Keep tray icon alive by leaking it (it needs to live for the entire app lifetime)
        // Box::leak keeps it alive without needing a global variable
        Box::leak(Box::new(_tray_icon));

        let shared = Arc::new(Mutex::new(Shared {
            hwnd: std::ptr::null_mut(),
            visible: true,
        }));

        shared
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("nvdash")
            .with_inner_size([380.0, 240.0])
            .with_min_inner_size([340.0, 140.0]),
        ..Default::default()
    };

    #[cfg(windows)]
    {
        let shared_clone = shared.clone();
        eframe::run_native(
            "nvdash",
            options,
            Box::new(move |cc| Ok(Box::new(NvDash::new(cc, shared_clone)))),
        )
    }

    #[cfg(not(windows))]
    {
        eframe::run_native(
            "nvdash",
            options,
            Box::new(|cc| Ok(Box::new(NvDash::new(cc)))),
        )
    }
}
