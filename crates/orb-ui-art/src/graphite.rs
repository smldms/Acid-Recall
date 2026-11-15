//! The default Graphite skin. Geometry and control identifiers belong to the original
//! layout; these parts replace only their visual surfaces. Legacy generated assets remain
//! reproducible for the art tools and reference documentation.

use crate::font::{Anchor, text};
use crate::parts::{KNOB_SWEEP, KNOBS, all_parts};
use crate::svg::{Part, Svg, arc_path, pal};

pub const BACKGROUND: &str = "#0e0e10";
pub const SURFACE: &str = "#202023";
pub const RAISED: &str = "#303034";
pub const OUTLINE: &str = "#49494e";
pub const INK: &str = "#eeeef0";
pub const MUTED: &str = "#aaaab1";
pub const ACCENT: &str = "#d5e786";
pub const ARC: [u8; 3] = [0xd5, 0xe7, 0x86];

fn recolor(body: &str, replacements: &[(&str, &str)]) -> String {
    // Replace tokens once: a new surface color must never be interpreted as an old ink.
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find('#') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = 1 + rest[1..].bytes().take_while(u8::is_ascii_hexdigit).count();
        let token = &rest[..end];
        out.push_str(
            replacements
                .iter()
                .find(|(old, _)| *old == token)
                .map_or(token, |(_, new)| *new),
        );
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn surface_colors() -> [(&'static str, &'static str); 17] {
    [
        (pal::BG, BACKGROUND),
        (pal::SLATE, SURFACE),
        (pal::SLATE_DARK, RAISED),
        (pal::SLATE_LINE, OUTLINE),
        (pal::INK, INK),
        (pal::INK_DIM, MUTED),
        (pal::PAPER, "#242427"),
        (pal::CREAM, "#242427"),
        (pal::CHARCOAL, "#1b1b1e"),
        (pal::LCD_BG, "#101012"),
        ("#0c0707", "#080809"),
        ("#bf4a2b", RAISED),
        ("#3b8a8f", RAISED),
        (pal::BLUE, RAISED),
        (pal::GOLD, RAISED),
        (pal::WHITE, INK),
        ("#7d8087", "#18181b"),
    ]
}

fn rail() -> Part {
    Part::new("wood", 16.0, 510.0, |s| {
        s.rect(0.0, 0.0, 16.0, 510.0, 0.0, BACKGROUND);
        s.rect(3.0, 1.0, 10.0, 508.0, 3.0, "#19191c");
        s.line(3.5, 5.0, 3.5, 505.0, "#ffffff0c", 0.7);
        for y in [12.0, 163.0, 347.0, 498.0] {
            s.circle(8.0, y, 2.0, "#080809");
            s.circle_stroke(8.0, y, 1.5, OUTLINE, 0.5);
            s.line(7.3, y, 8.7, y, MUTED, 0.5);
        }
    })
}

fn flatten_headers(body: String) -> String {
    // The base art names the embossed header overlay hdr. A flat panel has neither
    // the bevel overlay nor the bright top highlight; retain its exact text placement.
    recolor(
        &body,
        &[
            ("#762e1b", RAISED),
            ("#255659", RAISED),
            ("#1d3f6f", RAISED),
            ("#7f702e", RAISED),
        ],
    )
    .replace("fill=\"url(#hdr)\"", "fill=\"none\"")
    .replace("#ffffff2e", "#ffffff00")
}

fn knob(part: &Part) -> Option<Part> {
    let k = KNOBS.iter().find(|k| {
        part.name == format!("{}_body", k.name) || part.name == format!("{}_pointer", k.name)
    })?;
    Some(Part::new(&part.name, part.w, part.h, |s| {
        if part.name.ends_with("_body") {
            s.path_stroke(
                &arc_path(k.cx, k.cy, k.ring_r, -KNOB_SWEEP, KNOB_SWEEP),
                "#55555c",
                k.ring_w,
            );
            s.circle(k.cx, k.cy + 0.5, k.body_r + 0.5, "#080809");
            s.circle(k.cx, k.cy, k.body_r, "#303034");
            s.circle_stroke(k.cx, k.cy, k.body_r - 0.3, "#5a5a61", 0.5);
            s.circle(k.cx, k.cy, k.body_r * 0.68, "#29292d");
        } else {
            s.line(k.cx, k.cy - k.ptr0, k.cx, k.cy - k.ptr1, INK, k.ptr_w);
        }
    }))
}

fn steps(part: &Part) -> Option<Part> {
    if let Some(suffix) = part.name.strip_prefix("step808_") {
        let (group, state) = suffix.rsplit_once('_')?;
        let fill = match group {
            "red" => "#333336",
            "orange" => "#39393d",
            "yellow" => "#414146",
            _ => "#4a4a50",
        };
        return Some(Part::new(&part.name, part.w, part.h, |s| {
            s.rect(0.5, 0.0, 15.0, 17.0, 2.0, fill);
            s.rect_stroke(0.9, 0.4, 14.2, 16.2, 1.6, "#ffffff16", 0.6);
            s.rect(
                2.5,
                1.5,
                11.0,
                4.0,
                1.0,
                if state == "on" { ACCENT } else { "#18181b" },
            );
        }));
    }
    if let Some(suffix) = part.name.strip_prefix("step909_") {
        let (group, value) = suffix.rsplit_once('_')?;
        let color = match value {
            "1" => "#d9a76b",
            "2" => "#f07e77",
            "3" => ACCENT,
            _ => "#18181b",
        };
        return Some(Part::new(&part.name, part.w, part.h, |s| {
            s.rect(
                0.5,
                0.0,
                20.0,
                16.0,
                2.0,
                if group == "a" { "#36363b" } else { "#424247" },
            );
            s.rect_stroke(0.9, 0.4, 19.2, 15.2, 1.6, "#ffffff16", 0.6);
            s.rect(3.5, 1.5, 14.0, 5.0, 1.0, color);
        }));
    }
    None
}

fn power(part: &Part) -> Option<Part> {
    if !part.name.starts_with("power_") {
        return None;
    }
    let on = part.name.ends_with("_on");
    Some(Part::new(&part.name, part.w, part.h, |s| {
        let (cx, cy) = (part.w * 0.5, part.h * 0.5);
        s.circle(cx, cy, 4.3, "#151518");
        s.circle_stroke(cx, cy, 3.7, if on { ACCENT } else { "#68686e" }, 0.7);
        s.circle(cx, cy, 2.2, if on { ACCENT } else { "#3f3f45" });
    }))
}

fn background(part: &Part) -> Part {
    let mut colors = surface_colors().to_vec();
    if matches!(part.name.as_str(), "bg_tb303" | "bg_tr909" | "bg_master") {
        colors.push((pal::BLACK, INK));
    }
    colors.extend([
        ("#e0a06a", RAISED),
        ("#cfcabd", "#303034"),
        ("#c4c0b5", "#49494e"),
        ("#1c1d1f", "#29292d"),
    ]);
    let mut body = flatten_headers(recolor(&part.body, &colors));
    if part.name == "bg_tr909" {
        let mut s = Svg { s: body };
        text(&mut s, 288.5, 25.3, 2.8, Anchor::Middle, INK, "LEVEL");
        body = s.s;
    }
    // The original 303 uses BLACK for both label ink and the two dark key-band fills.
    // Restore those surfaces after brightening the labels.
    if part.name == "bg_tb303" {
        body = body.replace(
            "height=\"66\" rx=\"4\" fill=\"#eeeef0\"",
            "height=\"66\" rx=\"4\" fill=\"#151518\"",
        );
        let mut s = Svg { s: body };
        text(&mut s, 59.0, 37.0, 4.0, Anchor::Start, MUTED, "TB-303");
        body = s.s;
    }
    Part {
        body,
        ..part.clone()
    }
}

fn control(part: &Part) -> Part {
    let colors = [
        (pal::BG, BACKGROUND),
        (pal::SLATE, SURFACE),
        (pal::SLATE_DARK, RAISED),
        (pal::SLATE_LINE, OUTLINE),
        (pal::INK, INK),
        (pal::INK_DIM, MUTED),
        (pal::ORANGE, ACCENT),
        (pal::AMBER, ACCENT),
        (pal::YELLOW, "#b8b8bf"),
        (pal::OLIVE, "#85858d"),
        (pal::RED, "#f07e77"),
        (pal::GREEN, ACCENT),
        (pal::GREEN_DIM, "#30352b"),
        (pal::LCD_ON, INK),
        (pal::LCD_OFF, "#2a2a2f"),
        (pal::LCD_BG, "#101012"),
        (pal::STEEL, "#b9b9c0"),
        (pal::WHITE, INK),
        ("#4e5a6b", "#36363b"),
        ("#2a323d", "#242427"),
        ("#4b5767", "#39393f"),
        ("#dde1e6", "#36363b"),
        ("#a7afba", "#202023"),
        ("#e8503f", "#b2c579"),
        ("#9c2b25", "#5a6541"),
        ("#a85d14", "#5a6541"),
        ("#b9b5aa", "#44444b"),
        ("#f08a2440", "#d5e78620"),
        ("#e2413b40", "#f07e7720"),
        ("#4a2a28", "#39393f"),
    ];
    let mut body = recolor(&part.body, &colors);
    // Unselected pattern/shuffle keys need light legends on their new graphite faces.
    if (part.name.starts_with("pat_btn_") || part.name.starts_with("toggle_shuffle_"))
        && part.name.ends_with("_off")
    {
        body = body.replace(pal::BLACK, INK);
    }
    // Selected pattern keys use dark legends against their pale accent surface.
    if (part.name.starts_with("pat_btn_") || part.name.starts_with("toggle_shuffle_"))
        && part.name.ends_with("_on")
    {
        body = body.replace(pal::WHITE, "#151518").replace(INK, "#151518");
    }
    Part {
        body,
        ..part.clone()
    }
}

/// All default skin parts, keeping the legacy names, dimensions and interaction geometry.
pub fn all() -> Vec<Part> {
    all_parts()
        .iter()
        .map(|p| {
            if p.name == "wood" {
                return rail();
            }
            if let Some(k) = knob(p) {
                return k;
            }
            if let Some(s) = steps(p) {
                return s;
            }
            if let Some(p) = power(p) {
                return p;
            }
            if p.name.starts_with("bg_") {
                return background(p);
            }
            control(p)
        })
        .collect()
}

/// A headless panel preview with the same value arcs and surfaces as the desktop app.
pub fn panel() -> String {
    let parts = all();
    let layout = crate::layout::layout(&parts);
    recolor(
        &crate::layout::compose(&parts, &layout),
        &[
            (pal::BG, BACKGROUND),
            (pal::ORANGE, ACCENT),
            (pal::RED, ACCENT),
            (pal::AMBER, ACCENT),
            (pal::YELLOW, ACCENT),
        ],
    )
}
