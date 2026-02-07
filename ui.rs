use crate::gpu::{GpuHistory, GpuSnapshot};
use egui::{self, Color32, FontId, RichText, Rounding, Stroke, Vec2};
use egui_plot::{Line, Plot, PlotPoints};

// ── Color Palette ──────────────────────────────────────────────────────────
pub const BG_DARK: Color32 = Color32::from_rgb(18, 18, 22);
pub const BG_PANEL: Color32 = Color32::from_rgb(26, 26, 32);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(34, 34, 42);
pub const BORDER: Color32 = Color32::from_rgb(48, 48, 58);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 155);
pub const TEXT_DIM: Color32 = Color32::from_rgb(90, 90, 105);

pub const NVIDIA_GREEN: Color32 = Color32::from_rgb(118, 185, 0);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 200, 215);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(255, 180, 40);
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 70, 70);

// ── Theming Helpers ────────────────────────────────────────────────────────

pub fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG_DARK;
    style.visuals.window_fill = BG_DARK;
    style.visuals.extreme_bg_color = BG_PANEL;
    style.visuals.faint_bg_color = BG_ELEVATED;
    style.visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    style.visuals.widgets.inactive.bg_fill = BG_ELEVATED;

    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(12);

    ctx.set_style(style);
}

/// Color for a value in 0-100 range: green → yellow → red
pub fn heat_color(value: f64, low: f64, high: f64) -> Color32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    if t < 0.5 {
        let s = (t * 2.0) as f32;
        Color32::from_rgb(
            (118.0 + (255.0 - 118.0) * s) as u8,
            (185.0 + (180.0 - 185.0) * s) as u8,
            (0.0 + (40.0 - 0.0) * s) as u8,
        )
    } else {
        let s = ((t - 0.5) * 2.0) as f32;
        Color32::from_rgb(
            255,
            (180.0 - 110.0 * s) as u8,
            (40.0 - 40.0 * s) as u8,
        )
    }
}

// ── Drawing Functions ──────────────────────────────────────────────────────

pub fn draw_header(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("⬢")
                .color(NVIDIA_GREEN)
                .size(18.0),
        );
        ui.label(
            RichText::new(&snapshot.name)
                .color(TEXT_PRIMARY)
                .size(16.0)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!(
                    "Driver {}  ·  CUDA {}",
                    snapshot.driver_version, snapshot.cuda_version
                ))
                .color(TEXT_DIM)
                .size(11.0),
            );
        });
    });
    ui.add_space(4.0);
}

pub fn draw_gauges(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    let vram_pct = if snapshot.vram_total_mb > 0 {
        (snapshot.vram_used_mb as f64 / snapshot.vram_total_mb as f64) * 100.0
    } else {
        0.0
    };
    let power_pct = if snapshot.power_limit_w > 0.0 {
        (snapshot.power_draw_w / snapshot.power_limit_w) * 100.0
    } else {
        0.0
    };

    ui.columns(4, |cols| {
        draw_gauge_bar(
            &mut cols[0],
            "GPU",
            snapshot.gpu_util as f64,
            &format!("{}%", snapshot.gpu_util),
            heat_color(snapshot.gpu_util as f64, 0.0, 100.0),
        );
        draw_gauge_bar(
            &mut cols[1],
            "VRAM",
            vram_pct,
            &format!("{:.1}/{:.1} GB", snapshot.vram_used_mb as f64 / 1024.0, snapshot.vram_total_mb as f64 / 1024.0),
            heat_color(vram_pct, 0.0, 100.0),
        );
        draw_gauge_bar(
            &mut cols[2],
            "TEMP",
            snapshot.temperature as f64,
            &format!("{}°C", snapshot.temperature),
            heat_color(snapshot.temperature as f64, 30.0, 90.0),
        );
        draw_gauge_bar(
            &mut cols[3],
            "POWER",
            power_pct,
            &format!("{:.0}/{:.0}W", snapshot.power_draw_w, snapshot.power_limit_w),
            heat_color(power_pct, 0.0, 100.0),
        );
    });
}

fn draw_gauge_bar(
    ui: &mut egui::Ui,
    label: &str,
    pct: f64,
    value_text: &str,
    color: Color32,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .color(TEXT_SECONDARY)
                .size(10.0)
                .strong(),
        );

        ui.label(
            RichText::new(value_text)
                .color(color)
                .size(20.0)
                .font(FontId::monospace(20.0)),
        );

        let desired_size = Vec2::new(ui.available_width(), 6.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        let painter = ui.painter();
        // Background
        painter.rect_filled(rect, Rounding::same(3.0), BG_ELEVATED);
        // Fill
        let fill_width = rect.width() * (pct as f32 / 100.0).clamp(0.0, 1.0);
        let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, rect.height()));
        painter.rect_filled(fill_rect, Rounding::same(3.0), color);
    });
}

pub fn draw_sparklines(ui: &mut egui::Ui, snapshot: &GpuSnapshot, history: &GpuHistory) {
    let chart_height = 60.0;

    ui.columns(2, |cols| {
        // GPU Utilization sparkline
        draw_sparkline(
            &mut cols[0],
            "GPU Utilization",
            &history.gpu_util,
            0.0,
            100.0,
            "%",
            NVIDIA_GREEN,
            chart_height,
        );
        // VRAM sparkline
        draw_sparkline(
            &mut cols[1],
            "VRAM Usage",
            &history.vram_used,
            0.0,
            snapshot.vram_total_mb as f64,
            "MB",
            ACCENT_CYAN,
            chart_height,
        );
    });

    ui.add_space(4.0);

    ui.columns(2, |cols| {
        // Temperature sparkline
        draw_sparkline(
            &mut cols[0],
            "Temperature",
            &history.temperature,
            20.0,
            100.0,
            "°C",
            ACCENT_AMBER,
            chart_height,
        );
        // Power sparkline
        draw_sparkline(
            &mut cols[1],
            "Power Draw",
            &history.power_draw,
            0.0,
            snapshot.power_limit_w.max(1.0),
            "W",
            ACCENT_RED,
            chart_height,
        );
    });
}

fn draw_sparkline(
    ui: &mut egui::Ui,
    label: &str,
    data: &std::collections::VecDeque<f64>,
    y_min: f64,
    y_max: f64,
    unit: &str,
    color: Color32,
    height: f32,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(TEXT_SECONDARY).size(10.0));
            if let Some(&last) = data.back() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}{}", last, unit))
                            .color(color)
                            .size(10.0)
                            .font(FontId::monospace(10.0)),
                    );
                });
            }
        });

        let points: PlotPoints = data
            .iter()
            .enumerate()
            .map(|(i, &v)| [i as f64, v])
            .collect();

        let line = Line::new(points)
            .color(color)
            .width(1.5)
            .fill(y_min as f32);

        Plot::new(format!("spark_{}", label))
            .height(height)
            .show_axes(false)
            .show_grid(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .include_y(y_min)
            .include_y(y_max)
            .include_x(0.0)
            .include_x(120.0)
            .show_x(false)
            .show_y(false)
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });
    });
}

pub fn draw_clocks(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    ui.horizontal(|ui| {
        clock_chip(ui, "GFX", snapshot.clock_graphics_mhz);
        clock_chip(ui, "MEM", snapshot.clock_memory_mhz);
        clock_chip(ui, "SM", snapshot.clock_sm_mhz);
        if let Some(fan) = snapshot.fan_speed {
            clock_chip(ui, "FAN", fan);
        }
    });
}

fn clock_chip(ui: &mut egui::Ui, label: &str, value: u32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(90.0, 28.0), egui::Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect, Rounding::same(4.0), BG_ELEVATED);
    painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0, BORDER));

    painter.text(
        rect.left_center() + Vec2::new(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(9.0),
        TEXT_DIM,
    );

    painter.text(
        rect.right_center() + Vec2::new(-8.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        &format!("{}", value),
        FontId::monospace(12.0),
        TEXT_PRIMARY,
    );
}

pub fn draw_process_table(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("PROCESSES")
                .color(TEXT_SECONDARY)
                .size(10.0)
                .strong(),
        );
        ui.label(
            RichText::new(format!("({})", snapshot.processes.len()))
                .color(TEXT_DIM)
                .size(10.0),
        );
    });
    ui.add_space(2.0);

    if snapshot.processes.is_empty() {
        ui.label(RichText::new("  No GPU processes").color(TEXT_DIM).size(11.0));
        return;
    }

    // Header
    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(60.0, 16.0), |ui| {
            ui.label(RichText::new("PID").color(TEXT_DIM).size(9.0));
        });
        ui.allocate_ui(Vec2::new(200.0, 16.0), |ui| {
            ui.label(RichText::new("PROCESS").color(TEXT_DIM).size(9.0));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("VRAM").color(TEXT_DIM).size(9.0));
        });
    });

    let sep_rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            egui::pos2(sep_rect.left(), sep_rect.top()),
            egui::pos2(sep_rect.right(), sep_rect.top()),
        ],
        Stroke::new(1.0, BORDER),
    );
    ui.add_space(2.0);

    // Rows (show at most 10)
    for proc in snapshot.processes.iter().take(10) {
        ui.horizontal(|ui| {
            ui.allocate_ui(Vec2::new(60.0, 18.0), |ui| {
                ui.label(
                    RichText::new(format!("{}", proc.pid))
                        .color(TEXT_DIM)
                        .size(11.0)
                        .font(FontId::monospace(11.0)),
                );
            });
            ui.allocate_ui(Vec2::new(200.0, 18.0), |ui| {
                ui.label(
                    RichText::new(&proc.name)
                        .color(TEXT_PRIMARY)
                        .size(11.0),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let vram_text = if proc.vram_mb >= 1024 {
                    format!("{:.1} GB", proc.vram_mb as f64 / 1024.0)
                } else {
                    format!("{} MB", proc.vram_mb)
                };
                ui.label(
                    RichText::new(vram_text)
                        .color(ACCENT_CYAN)
                        .size(11.0)
                        .font(FontId::monospace(11.0)),
                );
            });
        });
    }

    if snapshot.processes.len() > 10 {
        ui.label(
            RichText::new(format!("  +{} more...", snapshot.processes.len() - 10))
                .color(TEXT_DIM)
                .size(10.0),
        );
    }
}
