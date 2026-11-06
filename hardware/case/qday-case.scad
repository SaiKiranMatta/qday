// QDay case: tray, bezel, key spacer and fit coupon. All dimensions are in mm.
//
// Origin: bottom-left corner of the breadboard, on the tray floor. X runs right, Y runs up
// toward the USB end, and Z runs up out of the screen.
//
// Export, from this folder:
//   openscad --backend=manifold -D 'part="tray"'   -o tray.stl   qday-case.scad
//   openscad --backend=manifold -D 'part="bezel"'  -o bezel.stl  qday-case.scad
//   openscad --backend=manifold -D 'part="shim"'   -o shim.stl   qday-case.scad
//   openscad --backend=manifold -D 'part="coupon"' -o coupon.stl qday-case.scad
//
// Print in PLA, 0.2 layers, 3 walls, 15% infill, no supports. Tray prints floor down, and bezel
// prints face down.

// One of: tray, bezel, shim, coupon, assembly, closed.
part = "assembly";
// Lift between the layers of the assembly preview. 0 shows it assembled.
explode = 0;

$fn = 48;
far = 1;      // How far a cut runs past the surface it goes through.

// Outer shape
out_x0 = -16.3;
out_x1 = 119.6;
out_y0 = -2.3;
out_y1 = 86.33;
out_z0 = -1.2;
out_z1 = 22.3;
out_radius = 4;

// Inside shape
in_x0 = -8.7;
in_x1 = 117.8;
in_y0 = -0.5;
in_y1 = 84.53;
in_radius = 1.6;

seam_z = 11;
plate_z = 21.1;   // Underside of the top plate

// Lip, step and snaps
lip_grow = 0.8;
lip_top = 12.6;
step_grow = 0.95;
step_top = 12.9;
lip_min_x = -5.7;
snap_spans = [[36, 44], [91, 99]];
snap_y = 85.33;
snap_z0 = 11.1;
snap_z1 = 12.5;
snap_tip_z = 11.8;
ridge_tip_y = 85.63;
groove_tip_y = 85.73;

// Screws
screw1 = [-10.62, 0.7];
screw2 = [114.3, 2.6];
clearance_d = 2.4;
pilot_d = 1.7;
head_d = 4.0;
screw1_bore_top = 8.0;
screw1_cone_top = 8.8;
screw1_pilot_top = 18.5;
screw2_cone_top = -0.4;
boss_d = 6.0;
boss_top = 2;
post_d = 6.0;
post_bottom = 2.1;
post_pilot_top = 9.6;

// Breadboard fence
fence_height = 1.5;
fence_outer = [-1.6, 45.4, -1.6, 84.1];   // x0, x1, y0, y1
fence_inner = [-0.4, 44.2, -0.4, 82.9];
fence_left_gap_y = 20.74;

// Screen support
support = [113.9, 116.3, 35.03, 83.03];

// Stylus holder
bore_d = 5.6;
bore_x = -12.3;
bore_start_y = 5.2;
mouth_d = 7.6;
mouth_start_y = 85.33;
head_slot = [-15.1, -9.5, 5.2, 14.8];   // x0, x1, z0, z1
head_slot_y = 80.4;
grip_y0 = 68.2;
grip_y1 = 80.2;
grip_z = 13.0;

// Screen window
glass = [40.8, 110.8, 34.03, 84.03];
frame_bottom = 19.0;
ribbon = [36.9, 42.8, 68.03, 80.03];
opening = [42.3, 109.3, 35.53, 82.53];
opening_radius = 3;
opening_flare = 1.2;

// Keys
key_xs = [54.44, 69.68, 84.92, 100.16];
key_y = 15.865;
key_block = [47.04, 107.56, 8.465, 23.265];
key_block_bottom = 13.4;
key_bore_d = 7.3;
key_flare_d = 8.1;
key_flare_h = 0.4;
key_socket = 12.4;
key_socket_top = 15.4;

// Grille
grille_d = 3.2;
grille_pitch = 5.2;
grille_columns = 8;
grille_rows = 10;
grille_x = -6.3;
grille_y = 34.23;
grille_skip = [[2, 2], [2, 3]];   // Column and row of the holes beside the RST pinhole

// RST pinhole
rst = [5.4, 47.6];
rst_d = 1.6;
rst_flare_d = 2.6;
rst_flare_h = 0.4;

// Logo
logo_text = "QDay";
logo_size = 9;
logo_depth = 0.6;
logo_font = "Liberation Sans:style=Bold";
logo_centre = [11.9, 15.865];

// USB-C notch
usb_notch = [-3.8, 20.2];
usb_notch_y = 79.98;
usb_notch_z = 10.7;

// Key spacer
shim_size = [63.5, 15.24, 5];

// Fit coupon
coupon_key = [44.44, 64.44, 5.865, 25.865];
coupon_key_z = 12.4;
coupon_edge = [-17.3, -7.7, 66.33, 87.33];
coupon_edge_z = 10;
coupon_gap = 5;

// Electronics, for the previews only
board_z = 11;
board_t = 1.6;

module rrect(x0, x1, y0, y1, r) {
    translate([x0 + r, y0 + r]) offset(r) square([x1 - x0 - 2 * r, y1 - y0 - 2 * r]);
}

module box(x0, x1, y0, y1, z0, z1) {
    translate([x0, y0, z0]) cube([x1 - x0, y1 - y0, z1 - z0]);
}

module outline_i(g) {
    rrect(in_x0 - g, in_x1 + g, in_y0 - g, in_y1 + g, in_radius + g);
}

// One rounded rectangle as a ring of 3D points, for lofting.
function ring(x0, x1, y0, y1, r, z) = [
    for (corner = [[x1 - r, y1 - r, 0], [x0 + r, y1 - r, 90],
                   [x0 + r, y0 + r, 180], [x1 - r, y0 + r, 270]])
        for (i = [0 : $fn / 4])
            let (a = corner[2] + 360 * i / $fn)
                [corner[0] + r * cos(a), corner[1] + r * sin(a), z]
];

function ring_p(l, o, z) =
    ring(out_x0 + l, out_x1 - o, out_y0 + o, out_y1 - o, out_radius - o, z);

// The hull of a stack of equal-length rings.
module loft(rings) {
    count = len(rings[0]);
    last = len(rings) - 1;
    hull() polyhedron(
        points = [for (r = rings) each r],
        faces = [
            [for (i = [count - 1 : -1 : 0]) i],
            [for (i = [0 : count - 1]) last * count + i],
            for (level = [0 : last - 1], i = [0 : count - 1])
                [level * count + i, level * count + (i + 1) % count,
                 (level + 1) * count + (i + 1) % count, (level + 1) * count + i],
        ]
    );
}

module body() {
    loft([
        ring_p(2.2, 0, out_z0),
        ring_p(0, 0, 1.0),
        ring_p(0, 0, 20.1),
        ring_p(1.2, 0, 21.3),
        ring_p(2.2, 1, out_z1),
    ]);
}

module above_x(x) {
    translate([x, out_y0 - far]) square([out_x1 - x + far, out_y1 - out_y0 + 2 * far]);
}

// The inside outline grown by `grow`, on every side but the left.
module wall_block(grow, z0, z1) {
    translate([0, 0, z0]) linear_extrude(z1 - z0) intersection() {
        outline_i(grow);
        above_x(lip_min_x);
    }
}

// The snap triangle, with a sliver running back into the lip so the two parts join.
module snap(span, tip_y) {
    translate([span[0], 0, 0]) rotate([90, 0, 90]) linear_extrude(span[1] - span[0]) polygon([
        [snap_y - 0.03, snap_z0],
        [snap_y, snap_z0],
        [tip_y, snap_tip_z],
        [snap_y, snap_z1],
        [snap_y - 0.03, snap_z1],
    ]);
}

module cylinder_at(xy, d, z0, z1) {
    translate([xy[0], xy[1], z0]) cylinder(d = d, h = z1 - z0);
}

// A round hole cut as one solid, from a list of [radius, z] points.
module revolved(xy, profile) {
    translate([xy[0], xy[1], 0]) rotate_extrude() polygon(profile);
}

module screw1_cut() {
    revolved(screw1, [
        [0, out_z0 - far],
        [head_d / 2, out_z0 - far],
        [head_d / 2, screw1_bore_top],
        [clearance_d / 2, screw1_cone_top],
        [clearance_d / 2, seam_z + far],
        [0, seam_z + far],
    ]);
}

module screw2_cut() {
    slope = (head_d - clearance_d) / 2 / (screw2_cone_top - out_z0);
    revolved(screw2, [
        [0, out_z0 - far],
        [head_d / 2 + far * slope, out_z0 - far],
        [clearance_d / 2, screw2_cone_top],
        [clearance_d / 2, seam_z + far],
        [0, seam_z + far],
    ]);
}

// A straight hole that flares through the top surface.
module flared_hole(xy, d, flare_d, flare_h, bottom) {
    slope = (flare_d - d) / 2 / flare_h;
    revolved(xy, [
        [0, bottom],
        [d / 2, bottom],
        [d / 2, out_z1 - flare_h],
        [flare_d / 2 + far * slope, out_z1 + far],
        [0, out_z1 + far],
    ]);
}

module stylus_cut() {
    flare = (mouth_d - bore_d) / 2 / (out_y1 - mouth_start_y);
    translate([bore_x, 0, seam_z]) rotate([-90, 0, 0]) rotate_extrude() polygon([
        [0, bore_start_y],
        [bore_d / 2, bore_start_y],
        [bore_d / 2, mouth_start_y],
        [mouth_d / 2 + far * flare, out_y1 + far],
        [0, out_y1 + far],
    ]);
    box(head_slot[0], head_slot[1], head_slot_y, out_y1 + far, head_slot[2], head_slot[3]);
}

module usb_notch_cut() {
    box(usb_notch[0], usb_notch[1], usb_notch_y, out_y1 + far, usb_notch_z, out_z1 + far);
}

module fence() {
    translate([0, 0, -far]) linear_extrude(far + fence_height) difference() {
        translate([fence_outer[0], fence_outer[2]])
            square([fence_outer[1] - fence_outer[0], fence_outer[3] - fence_outer[2]]);
        translate([fence_inner[0], fence_inner[2]])
            square([fence_inner[1] - fence_inner[0], fence_inner[3] - fence_inner[2]]);
        // The ESP32's free header row hangs over the rest of the left bar.
        translate([fence_outer[0] - far, fence_left_gap_y])
            square([fence_inner[0] - fence_outer[0] + far, fence_outer[3]]);
    }
}

module tray() {
    difference() {
        union() {
            difference() {
                union() {
                    intersection() {
                        body();
                        box(out_x0 - far, out_x1 + far, out_y0 - far, out_y1 + far,
                            out_z0 - far, seam_z);
                    }
                    wall_block(lip_grow, seam_z - far, lip_top);
                }
                difference() {
                    linear_extrude(lip_top + far) outline_i(0);
                    fence();
                    box(support[0], support[1], support[2], support[3], -far, seam_z);
                    cylinder_at(screw2, boss_d, -far, boss_top);
                }
            }
            for (span = snap_spans) snap(span, ridge_tip_y);
        }

        screw1_cut();
        screw2_cut();
        stylus_cut();
        usb_notch_cut();
    }
}

// Everything that stays inside the bezel's cavity, apart from the post.
module bezel_keepers() {
    box(key_block[0], key_block[1], key_block[2], key_block[3], key_block_bottom, plate_z + far);
    difference() {
        box(glass[0], glass[1], glass[2], glass[3], frame_bottom, plate_z + far);
        box(ribbon[0], ribbon[1], ribbon[2], ribbon[3], frame_bottom - far, plate_z);
    }
}

module window_cut() {
    translate([0, 0, frame_bottom - far]) linear_extrude(plate_z - frame_bottom + far + 0.1)
        rrect(opening[0], opening[1], opening[2], opening[3], opening_radius);
    grow = opening_flare + far;
    loft([
        ring(opening[0], opening[1], opening[2], opening[3], opening_radius, plate_z),
        ring(opening[0] - grow, opening[1] + grow, opening[2] - grow, opening[3] + grow,
             opening_radius + grow, out_z1 + far),
    ]);
}

module key_cut(x) {
    flared_hole([x, key_y], key_bore_d, key_flare_d, key_flare_h, key_block_bottom - far);
    box(x - key_socket / 2, x + key_socket / 2, key_y - key_socket / 2, key_y + key_socket / 2,
        key_block_bottom - far, key_socket_top);
}

module grille_cut() {
    for (column = [0 : grille_columns - 1], row = [0 : grille_rows - 1]) {
        if (!is_skipped(column, row)) {
            cylinder_at(
                [grille_x + column * grille_pitch, grille_y + row * grille_pitch],
                grille_d, plate_z - far, out_z1 + far
            );
        }
    }
}

function is_skipped(column, row) =
    len([for (hole = grille_skip) if (hole[0] == column && hole[1] == row) 1]) > 0;

module bezel() {
    difference() {
        union() {
            difference() {
                intersection() {
                    body();
                    box(out_x0 - far, out_x1 + far, out_y0 - far, out_y1 + far,
                        seam_z, out_z1 + far);
                }
                difference() {
                    translate([0, 0, seam_z - far]) linear_extrude(plate_z - seam_z + far)
                        outline_i(0);
                    bezel_keepers();
                }
                wall_block(step_grow, seam_z - far, step_top);
                for (span = snap_spans) snap(span, groove_tip_y);
            }
            cylinder_at(screw2, post_d, post_bottom, plate_z + 0.6);
        }

        cylinder_at(screw1, pilot_d, seam_z - far, screw1_pilot_top);
        cylinder_at(screw2, pilot_d, post_bottom - far, post_pilot_top);

        difference() {
            stylus_cut();
            box(out_x0, in_x0, grip_y0, grip_y1, grip_z, out_z1);
        }
        usb_notch_cut();

        window_cut();
        for (x = key_xs) key_cut(x);
        grille_cut();
        flared_hole(rst, rst_d, rst_flare_d, rst_flare_h, plate_z - far);

        translate([logo_centre[0], logo_centre[1], out_z1 - logo_depth])
            linear_extrude(logo_depth + far)
                text(logo_text, size = logo_size, font = logo_font,
                     halign = "center", valign = "center");
    }
}

module shim() {
    cube(shim_size);
}

// Face down on the bed. rotate(), never mirror(), so the text stays the right way round.
module face_down(x0, y1, z1) {
    translate([-x0, y1, z1]) rotate([180, 0, 0]) children();
}

module coupon() {
    key_width = coupon_key[1] - coupon_key[0];
    edge_width = coupon_edge[1] - coupon_edge[0];

    face_down(coupon_key[0], coupon_key[3], out_z1) intersection() {
        bezel();
        box(coupon_key[0], coupon_key[1], coupon_key[2], coupon_key[3],
            coupon_key_z, out_z1 + far);
    }

    translate([key_width + coupon_gap - coupon_edge[0], -coupon_edge[2], -out_z0])
        intersection() {
            tray();
            box(coupon_edge[0], coupon_edge[1], coupon_edge[2], coupon_edge[3],
                out_z0 - far, lip_top + far);
        }

    translate([key_width + edge_width + 2 * coupon_gap, 0, 0])
        face_down(coupon_edge[0], coupon_edge[3], out_z1) intersection() {
            bezel();
            box(coupon_edge[0], coupon_edge[1], coupon_edge[2], coupon_edge[3],
                coupon_edge_z, out_z1 + far);
        }
}

// Stand-ins for the previews

module breadboard() {
    color("#f2f0e8") box(0, 43.8, 0, 82.5, 0, 8.5);
}

module esp32() {
    color("#1b6b61") box(-6.1, 22.5, 16.08, 80.48, board_z, board_z + board_t);
    color("#a8a8a8") box(-0.6, 16.0, 72, 81.5, board_z + board_t, board_z + board_t + 3.2);
}

module screen() {
    color("#3a3f8f") box(31.3, 117.3, 34.03, 84.03, board_z, board_z + board_t);
    color("#101820") box(glass[0], glass[1], glass[2], glass[3], board_z + board_t, 18.6);
}

module keys() {
    color("#b9855a") box(44.28, 107.78, 9.515, 24.755, 10.2, 11.8);
    for (x = key_xs) {
        color("#333333") box(x - 6, x + 6, key_y - 6, key_y + 6, 11.8, 15.4);
        color("#d8d8d8") cylinder_at([x, key_y], 6.8, 15.4, 23.8);
    }
}

module spacer_in_place() {
    // Just clear of the fence's right bar, so it sits 1.12 right of the stripboard.
    color("#c8a2e8") translate([fence_outer[1], 9.515, 0]) shim();
}

module stylus() {
    tip_y = 93.73 - 88;
    color("#e8e8e8") translate([bore_x, tip_y, seam_z]) rotate([-90, 0, 0])
        cylinder(d = 5, h = 88 - 13);
    color("#e8e8e8") box(bore_x - 2.5, bore_x + 2.5, 93.73 - 13, 93.73, seam_z - 5.5, seam_z + 3.5);
}

module assembly() {
    color("#5a4bb0") tray();
    translate([0, 0, explode]) {
        breadboard();
        spacer_in_place();
    }
    translate([0, 0, 2 * explode]) {
        keys();
        esp32();
        screen();
    }
    translate([-explode, 0, 0]) stylus();
    translate([0, 0, 3.4 * explode]) color("#7a68d8") bezel();
}

module closed() {
    color("#5a4bb0") tray();
    color("#7a68d8") bezel();
}

if (part == "tray") {
    translate([-out_x0, -out_y0, -out_z0]) tray();
} else if (part == "bezel") {
    translate([0, -out_y0, 0]) face_down(out_x0, out_y1 + out_y0, out_z1) bezel();
} else if (part == "shim") {
    shim();
} else if (part == "coupon") {
    coupon();
} else if (part == "assembly") {
    assembly();
} else if (part == "closed") {
    closed();
}
