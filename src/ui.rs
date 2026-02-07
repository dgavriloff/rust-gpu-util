use crate::gpu::{GpuHistory, GpuSnapshot};
use egui::{self, Color32, CornerRadius, FontId, Pos2, RichText, Stroke, Vec2};

// ── Color Palette ──────────────────────────────────────────────────────────
pub const BG_DARK: Color32 = Color32::from_rgb(18, 18, 22);
pub const BG_PANEL: Color32 = Color32::from_rgb(26, 26, 32);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(34, 34, 42);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 155);
pub const TEXT_DIM: Color32 = Color32::from_rgb(90, 90, 105);

pub const NVIDIA_GREEN: Color32 = Color32::from_rgb(118, 185, 0);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 200, 215);
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

    style.spacing.item_spacing = Vec2::new(6.0, 3.0);
    style.spacing.window_margin = egui::Margin::same(8);

    ctx.set_style(style);
}

/// Color for a value in a range: green → yellow → red
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

/// Dimmed version of a color for bar track backgrounds
fn dim_color(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * factor) as u8,
        (color.g() as f32 * factor) as u8,
        (color.b() as f32 * factor) as u8,
    )
}

// ── Drawing Functions ──────────────────────────────────────────────────────

/// Header: green hex icon + GPU name (left), temp badge + power badge (right)
pub fn draw_header(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("⬢")
                .color(NVIDIA_GREEN)
                .size(14.0),
        );
        let name_response = ui.label(
            RichText::new(&snapshot.name)
                .color(TEXT_PRIMARY)
                .size(13.0)
                .strong(),
        );
        name_response.on_hover_text(format!(
            "Driver {}  ·  CUDA {}",
            snapshot.driver_version, snapshot.cuda_version
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:.0}W", snapshot.power_draw_w))
                    .color(heat_color(
                        if snapshot.power_limit_w > 0.0 {
                            (snapshot.power_draw_w / snapshot.power_limit_w) * 100.0
                        } else {
                            0.0
                        },
                        0.0,
                        100.0,
                    ))
                    .size(11.0)
                    .font(FontId::monospace(11.0)),
            );
            ui.label(
                RichText::new(format!("{}°C", snapshot.temperature))
                    .color(heat_color(snapshot.temperature as f64, 30.0, 90.0))
                    .size(11.0)
                    .font(FontId::monospace(11.0)),
            );
        });
    });
}

/// 4 inline metric bars: GPU, VRAM, TEMP, PWR
pub fn draw_metric_bars(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
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

    metric_bar_row(
        ui,
        "GPU",
        snapshot.gpu_util as f64,
        &format!("{}%", snapshot.gpu_util),
        heat_color(snapshot.gpu_util as f64, 0.0, 100.0),
    );
    metric_bar_row(
        ui,
        "VRAM",
        vram_pct,
        &format!(
            "{:.1}/{:.0}G",
            snapshot.vram_used_mb as f64 / 1024.0,
            snapshot.vram_total_mb as f64 / 1024.0
        ),
        heat_color(vram_pct, 0.0, 100.0),
    );
    metric_bar_row(
        ui,
        "TEMP",
        snapshot.temperature as f64,
        &format!("{}°C", snapshot.temperature),
        heat_color(snapshot.temperature as f64, 30.0, 90.0),
    );
    metric_bar_row(
        ui,
        "PWR",
        power_pct,
        &format!("{:.0}/{:.0}W", snapshot.power_draw_w, snapshot.power_limit_w),
        heat_color(power_pct, 0.0, 100.0),
    );
}

fn metric_bar_row(
    ui: &mut egui::Ui,
    label: &str,
    pct: f64,
    value_text: &str,
    color: Color32,
) {
    let bar_height = 14.0;
    let label_width = 36.0;
    let value_width = 80.0;

    ui.horizontal(|ui| {
        // Dim label
        ui.allocate_ui(Vec2::new(label_width, bar_height), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(label)
                        .color(TEXT_DIM)
                        .size(10.0)
                        .font(FontId::monospace(10.0)),
                );
            });
        });

        // Bar (fills remaining width minus value column)
        let bar_width = (ui.available_width() - value_width - 8.0).max(40.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, bar_height), egui::Sense::hover());
        let painter = ui.painter();

        // Track (dimmed color)
        painter.rect_filled(rect, CornerRadius::same(3), dim_color(color, 0.15));

        // Fill
        let fill_frac = (pct as f32 / 100.0).clamp(0.0, 1.0);
        let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * fill_frac, rect.height()));
        painter.rect_filled(fill_rect, CornerRadius::same(3), color);

        // Value text (right-aligned, monospace)
        ui.allocate_ui(Vec2::new(value_width, bar_height), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(value_text)
                        .color(color)
                        .size(11.0)
                        .font(FontId::monospace(11.0)),
                );
            });
        });
    });
}

/// Two custom-painted mini sparklines side by side (GPU util + VRAM)
pub fn draw_mini_sparklines(ui: &mut egui::Ui, snapshot: &GpuSnapshot, history: &GpuHistory) {
    let sparkline_height = 20.0;

    ui.columns(2, |cols| {
        paint_sparkline(
            &mut cols[0],
            "GPU %",
            &history.gpu_util,
            0.0,
            100.0,
            NVIDIA_GREEN,
            sparkline_height,
        );
        paint_sparkline(
            &mut cols[1],
            "VRAM",
            &history.vram_used,
            0.0,
            snapshot.vram_total_mb as f64,
            ACCENT_CYAN,
            sparkline_height,
        );
    });
}

fn paint_sparkline(
    ui: &mut egui::Ui,
    label: &str,
    data: &std::collections::VecDeque<f64>,
    y_min: f64,
    y_max: f64,
    color: Color32,
    height: f32,
) {
    // Tiny label
    ui.label(
        RichText::new(label)
            .color(TEXT_DIM)
            .size(9.0),
    );

    // Allocate rect for sparkline
    let desired_size = Vec2::new(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, CornerRadius::same(2), BG_ELEVATED);

    if data.len() < 2 {
        return;
    }

    let range = (y_max - y_min).max(1.0);
    let n = data.len();
    let max_points = 120;
    let x_step = rect.width() / (max_points as f32 - 1.0);

    // Build points, right-aligned (most recent data at right edge)
    let points: Vec<Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x_offset = (max_points as i32 - n as i32 + i as i32) as f32;
            let x = rect.left() + x_offset * x_step;
            let t = ((v - y_min) / range).clamp(0.0, 1.0) as f32;
            let y = rect.bottom() - t * rect.height();
            Pos2::new(x, y)
        })
        .collect();

    // Draw line segments
    for window in points.windows(2) {
        painter.line_segment(
            [window[0], window[1]],
            Stroke::new(1.5, color),
        );
    }
}

/// Footer: clocks + fan on line 1, process summary on line 2
pub fn draw_footer(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    // Line 1: clocks + fan
    let mut clock_parts = vec![
        format!("GFX {}", snapshot.clock_graphics_mhz),
        format!("MEM {}", snapshot.clock_memory_mhz),
        format!("SM {}", snapshot.clock_sm_mhz),
    ];
    if let Some(fan) = snapshot.fan_speed {
        clock_parts.push(format!("FAN {}%", fan));
    }
    ui.label(
        RichText::new(clock_parts.join(" · "))
            .color(TEXT_DIM)
            .size(10.0)
            .font(FontId::monospace(10.0)),
    );

    // Line 2: process summary
    let proc_count = snapshot.processes.len();
    if proc_count == 0 {
        ui.label(
            RichText::new("No GPU processes")
                .color(TEXT_DIM)
                .size(10.0),
        );
    } else {
        let mut parts = vec![format!("{} procs", proc_count)];
        for proc in snapshot.processes.iter().take(2) {
            let vram_text = if proc.vram_mb >= 1024 {
                format!("{:.1}G", proc.vram_mb as f64 / 1024.0)
            } else {
                format!("{}M", proc.vram_mb)
            };
            parts.push(format!("{} {}", proc.name, vram_text));
        }
        ui.label(
            RichText::new(parts.join(" · "))
                .color(TEXT_SECONDARY)
                .size(10.0)
                .font(FontId::monospace(10.0)),
        );
    }
}
