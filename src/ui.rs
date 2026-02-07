use crate::gpu::{GpuHistory, GpuSnapshot};
use egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use std::collections::VecDeque;

// ── Color Palette ──────────────────────────────────────────────────────────
pub const BG_DARK: Color32 = Color32::from_rgb(18, 18, 22);
pub const BG_PANEL: Color32 = Color32::from_rgb(26, 26, 32);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(34, 34, 42);
const BAR_TRACK: Color32 = Color32::from_rgb(40, 40, 48);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 155);
pub const TEXT_DIM: Color32 = Color32::from_rgb(90, 90, 105);

pub const NVIDIA_GREEN: Color32 = Color32::from_rgb(118, 185, 0);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(80, 180, 190);
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 70, 70);

const SPARK_BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const SPARK_WIDTH: usize = 36;
const BAR_WIDTH: usize = 28;
const FONT_SIZE: f32 = 11.0;

// ── Theming ──────────────────────────────────────────────────────────────

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

    style.spacing.item_spacing = Vec2::new(6.0, 1.0);
    style.spacing.window_margin = egui::Margin::ZERO;

    ctx.set_style(style);
}

/// Soft heat color: sage green → muted gold → dusty rose
pub fn heat_color(value: f64, low: f64, high: f64) -> Color32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    if t < 0.5 {
        let s = (t * 2.0) as f32;
        Color32::from_rgb(
            (105.0 + (195.0 - 105.0) * s) as u8,
            (160.0 + (165.0 - 160.0) * s) as u8,
            (95.0 + (90.0 - 95.0) * s) as u8,
        )
    } else {
        let s = ((t - 0.5) * 2.0) as f32;
        Color32::from_rgb(
            (195.0 + (190.0 - 195.0) * s) as u8,
            (165.0 - 65.0 * s) as u8,
            (90.0 + (10.0 - 90.0) * s) as u8,
        )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn tf(color: Color32) -> egui::TextFormat {
    egui::TextFormat {
        font_id: FontId::monospace(FONT_SIZE),
        color,
        ..Default::default()
    }
}

fn build_sparkline_str(data: &VecDeque<f64>, y_min: f64, y_max: f64) -> String {
    let range = (y_max - y_min).max(1.0);
    let n = data.len();
    let mut s = String::with_capacity(SPARK_WIDTH);
    for i in 0..SPARK_WIDTH {
        let di = i as i32 - (SPARK_WIDTH as i32 - n as i32);
        if di >= 0 && (di as usize) < n {
            let v = data[di as usize];
            let t = ((v - y_min) / range).clamp(0.0, 1.0);
            let idx = (t * 7.0).round() as usize;
            s.push(SPARK_BLOCKS[idx.min(7)]);
        } else {
            s.push(' ');
        }
    }
    s
}

// ── Drawing Functions ────────────────────────────────────────────────────

/// Header: ⬢ GPU name (left), temp + power badges (right)
pub fn draw_header(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    let temp_color = heat_color(snapshot.temperature as f64, 30.0, 90.0);
    let pwr_pct = if snapshot.power_limit_w > 0.0 {
        (snapshot.power_draw_w / snapshot.power_limit_w) * 100.0
    } else {
        0.0
    };
    let pwr_color = heat_color(pwr_pct, 0.0, 100.0);

    ui.horizontal(|ui| {
        let mut name_job = egui::text::LayoutJob::default();
        name_job.append("⬢ ", 0.0, tf(NVIDIA_GREEN));
        name_job.append(&snapshot.name, 0.0, tf(TEXT_PRIMARY));
        let resp = ui.label(name_job);
        resp.on_hover_text(format!(
            "Driver {} · CUDA {}",
            snapshot.driver_version, snapshot.cuda_version
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut badge_job = egui::text::LayoutJob::default();
            badge_job.append(
                &format!("{}°C", snapshot.temperature),
                0.0,
                tf(temp_color),
            );
            badge_job.append("  ", 0.0, tf(TEXT_DIM));
            badge_job.append(
                &format!("{:.0}W", snapshot.power_draw_w),
                0.0,
                tf(pwr_color),
            );
            ui.label(badge_job);
        });
    });
}

/// GPU and VRAM text sparklines using block characters
pub fn draw_text_sparklines(
    ui: &mut egui::Ui,
    snapshot: &GpuSnapshot,
    history: &GpuHistory,
) {
    // GPU sparkline
    {
        let spark = build_sparkline_str(&history.gpu_util, 0.0, 100.0);
        let mut job = egui::text::LayoutJob::default();
        job.append(" GPU  ", 0.0, tf(TEXT_DIM));
        job.append(&spark, 0.0, tf(NVIDIA_GREEN));
        job.append(&format!("  {}%", snapshot.gpu_util), 0.0, tf(TEXT_SECONDARY));
        ui.label(job);
    }

    // VRAM sparkline
    {
        let spark =
            build_sparkline_str(&history.vram_used, 0.0, snapshot.vram_total_mb as f64);
        let mut job = egui::text::LayoutJob::default();
        job.append(" VRAM ", 0.0, tf(TEXT_DIM));
        job.append(&spark, 0.0, tf(ACCENT_CYAN));
        job.append(
            &format!(
                "  {:.1}/{:.0}G",
                snapshot.vram_used_mb as f64 / 1024.0,
                snapshot.vram_total_mb as f64 / 1024.0
            ),
            0.0,
            tf(TEXT_SECONDARY),
        );
        ui.label(job);
    }
}

/// Single temp bar using block characters
pub fn draw_temp_bar(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    let pct = ((snapshot.temperature as f64 - 30.0) / 60.0).clamp(0.0, 1.0);
    let filled = (pct * BAR_WIDTH as f64).round() as usize;
    let empty = BAR_WIDTH - filled;
    let color = heat_color(snapshot.temperature as f64, 30.0, 90.0);

    let mut job = egui::text::LayoutJob::default();
    job.append(" TEMP ", 0.0, tf(TEXT_DIM));
    if filled > 0 {
        job.append(&"█".repeat(filled), 0.0, tf(color));
    }
    if empty > 0 {
        job.append(&"░".repeat(empty), 0.0, tf(BAR_TRACK));
    }
    job.append(
        &format!("  {}°C", snapshot.temperature),
        0.0,
        tf(TEXT_SECONDARY),
    );
    ui.label(job);
}

/// Top 3 GPU processes by VRAM
pub fn draw_process_list(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    if snapshot.processes.is_empty() {
        ui.label(
            RichText::new(" No GPU processes")
                .color(TEXT_DIM)
                .font(FontId::monospace(FONT_SIZE)),
        );
        return;
    }

    for proc in snapshot.processes.iter().take(3) {
        let vram_text = if proc.vram_mb >= 1024 {
            format!("{:.1}G", proc.vram_mb as f64 / 1024.0)
        } else {
            format!("{}M", proc.vram_mb)
        };

        let name: &str = if proc.name.len() > 28 {
            &proc.name[..28]
        } else {
            &proc.name
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(" {}", name))
                    .color(TEXT_PRIMARY)
                    .font(FontId::monospace(FONT_SIZE)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(&vram_text)
                        .color(ACCENT_CYAN)
                        .font(FontId::monospace(FONT_SIZE)),
                );
            });
        });
    }

    let remaining = snapshot.processes.len().saturating_sub(3);
    if remaining > 0 {
        ui.label(
            RichText::new(format!(" +{} more", remaining))
                .color(TEXT_DIM)
                .font(FontId::monospace(9.0)),
        );
    }
}

/// Footer: clocks + fan in a single line
pub fn draw_footer(ui: &mut egui::Ui, snapshot: &GpuSnapshot) {
    let mut parts = vec![
        format!("GFX {}", snapshot.clock_graphics_mhz),
        format!("MEM {}", snapshot.clock_memory_mhz),
        format!("SM {}", snapshot.clock_sm_mhz),
    ];
    if let Some(fan) = snapshot.fan_speed {
        parts.push(format!("FAN {}%", fan));
    }
    ui.label(
        RichText::new(format!(" {}", parts.join(" · ")))
            .color(TEXT_DIM)
            .font(FontId::monospace(FONT_SIZE)),
    );
}
