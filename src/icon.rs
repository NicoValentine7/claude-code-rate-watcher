use tray_icon::Icon;

/// Generate a colored circle icon that reflects usage status.
///
/// Colors follow Apple HIG status conventions:
///   - Green  (#34C759): 0–69% — safe
///   - Orange (#FF9500): 70–89% — warning
///   - Red    (#FF3B30): 90–100% — critical
///
/// Size: 32x32 (@2x for Retina). macOS will downscale for @1x displays.
pub fn generate_status_icon(percent: u32) -> Icon {
    let size: u32 = 32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    let (r, g, b) = status_color(percent);

    let center = size as f32 / 2.0;
    let radius = center - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center + 0.5;
            let dy = y as f32 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius + 0.5 {
                // Anti-alias the edge
                let alpha = if dist > radius - 0.5 {
                    ((radius + 0.5 - dist) * 255.0).clamp(0.0, 255.0) as u8
                } else {
                    255
                };

                let idx = ((y * size + x) * 4) as usize;
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = alpha;
            }
        }
    }

    Icon::from_rgba(pixels, size, size).expect("Failed to create icon")
}

fn status_color(percent: u32) -> (u8, u8, u8) {
    if percent >= 90 {
        (255, 59, 48) // Apple Red
    } else if percent >= 70 {
        (255, 149, 0) // Apple Orange
    } else {
        (52, 199, 89) // Apple Green
    }
}
