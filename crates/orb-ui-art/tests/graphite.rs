use orb_ui_art::{check_parts, graphite, parts, render};

#[test]
fn default_skin_preserves_all_control_names_sizes_and_layout() {
    let original = parts::all_parts();
    let modern = graphite::all();
    assert_eq!(original.len(), modern.len());
    for (before, after) in original.iter().zip(&modern) {
        assert_eq!(
            (&before.name, before.w, before.h),
            (&after.name, after.w, after.h)
        );
    }
    assert_eq!(
        orb_ui_art::layout::tsv(&orb_ui_art::layout::layout(&original)),
        orb_ui_art::layout::tsv(&orb_ui_art::layout::layout(&modern))
    );
}

#[test]
fn every_default_part_renders_at_all_supported_scales() {
    let problems = check_parts(&graphite::all());
    assert!(problems.is_empty(), "{problems:#?}");
    let panel = render(&graphite::panel(), 2).unwrap();
    assert_eq!((panel.width(), panel.height()), (1248, 1020));
}

#[test]
fn dark_surfaces_remain_dark_while_labels_are_light() {
    let parts = graphite::all();
    for name in ["bg_tb303", "bg_tr808", "bg_tr909", "bg_mixer", "bg_pattern"] {
        let part = parts.iter().find(|p| p.name == name).unwrap();
        let px = render(&part.to_svg(), 1).unwrap();
        // A free pixel of the main face, clear of controls, labels and the seam.
        let p = px.pixel(2, 22).unwrap().demultiply();
        assert!(p.red() < 60 && p.green() < 60 && p.blue() < 60, "{name}");
        assert!(part.body.contains(graphite::INK) || part.body.contains(graphite::MUTED));
    }
}

#[test]
fn velocity_and_power_states_stay_visibly_distinct() {
    let parts = graphite::all();
    let get = |name: &str| parts.iter().find(|p| p.name == name).unwrap();
    for (off, on) in [
        ("power_14x14_off", "power_14x14_on"),
        ("pat_btn_1_off", "pat_btn_1_on"),
        ("step808_red_off", "step808_red_on"),
    ] {
        let a = render(&get(off).to_svg(), 2).unwrap();
        let b = render(&get(on).to_svg(), 2).unwrap();
        assert_ne!(a.data(), b.data(), "{off} / {on}");
    }
    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(
                get(&format!("step909_a_{a}")).body,
                get(&format!("step909_a_{b}")).body
            );
        }
    }
}
