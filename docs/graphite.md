# Graphite: the default interface

The built-in interface uses graphite and neutral gray surfaces, dark concentric knobs,
white pointers, pale lime value arcs and indicators, flat section headers, and black rack
rails. Gray step keys retain the four-step groupings of the drum machines. The 909 keeps
distinct colors for its three note velocities; clipping and recording retain a red warning.
TB white and black keys remain distinct. The MIDI, Random, LINK and ARP controls share the
new surfaces.

This is the default appearance, embedded in the executable. No external mod or setting is
needed. Drum-sample mods and their artwork in the mod display continue to work normally.
Control positions, gesture behavior, learned MIDI assignments and song files stay compatible.

The implementation lives in `crates/orb-ui-art/src/graphite.rs`. The UI build script generates
and embeds these parts into its build output, keeping the legacy art set in `assets/ui`
reproducible. Use `cargo run -p orb-ui-art -- graphite OUTPUT_DIRECTORY` for a headless panel
preview and render checks at 1x, 2x and 3x. The running app adds the interactive Random/ARP
buttons and mod display over this composed art preview.
