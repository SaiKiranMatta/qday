use qday_ui::canvas::{Canvas, Color, HEIGHT, Rect, TextStyle, WIDTH};

const BLACK: Color = Color::hex(0x000000);
const WHITE: Color = Color::hex(0xFFFFFF);
const VIOLET: Color = Color::hex(0x8B7CF6);

fn buffer() -> Vec<u16> {
    vec![0; (WIDTH * HEIGHT) as usize]
}

/// Allow for rounding in blending.
fn close(a: Color, b: Color) -> bool {
    a.r.abs_diff(b.r) <= 8 && a.g.abs_diff(b.g) <= 4 && a.b.abs_diff(b.b) <= 8
}

#[test]
fn colors_survive_rgb565_round_trip() {
    for c in [BLACK, WHITE, VIOLET, Color::hex(0x123456)] {
        assert!(close(Color::from_565(c.to_565()), c), "{c:?}");
    }
    assert_eq!(
        Color::from_565(WHITE.to_565()),
        WHITE,
        "white stays exactly white"
    );
}

#[test]
fn blending_mixes_by_coverage() {
    assert_eq!(WHITE.over(BLACK, 255), WHITE);
    assert_eq!(WHITE.over(BLACK, 0), BLACK);
    let half = WHITE.over(BLACK, 128);
    assert!((126..=130).contains(&half.r), "{half:?}");
}

#[test]
fn clear_fills_every_pixel() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.clear(VIOLET);
    assert!(close(c.get(0, 0), VIOLET));
    assert!(close(c.get(WIDTH - 1, HEIGHT - 1), VIOLET));
}

#[test]
fn drawing_off_screen_is_ignored() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.blend(-1, 5, WHITE, 255);
    c.blend(WIDTH, 5, WHITE, 255);
    c.round_rect(Rect::new(-50.0, -50.0, 500.0, 400.0), 10.0, WHITE, 255);
    assert!(close(c.get(0, 0), WHITE));
}

#[test]
fn rounded_rect_fills_the_inside_and_softens_the_corners() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.round_rect(Rect::new(10.0, 10.0, 100.0, 60.0), 16.0, WHITE, 255);
    assert_eq!(c.get(60, 40), WHITE, "center");
    assert_eq!(c.get(5, 40), BLACK, "outside");
    assert_eq!(c.get(10, 10), BLACK, "the very corner is cut off");
    // A pixel on the curved edge is partly covered: anti-aliased, not stair-stepped.
    let edge = (0..20)
        .map(|i| c.get(10 + i, 10 + i))
        .find(|p| p.r > 20 && p.r < 235);
    assert!(
        edge.is_some(),
        "no partially covered pixel along the corner"
    );
}

#[test]
fn circles_are_round_and_anti_aliased() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.circle(160.0, 120.0, 30.0, WHITE, 255);
    assert_eq!(c.get(160, 120), WHITE);
    assert_eq!(c.get(160 + 31, 120), BLACK);
    assert_eq!(c.get(160 + 25, 120), WHITE);
    let rim = c.get(160 + 21, 120 + 21); // distance ~29.7: near the edge
    assert!(rim.r > 0 && rim.r < 255, "{rim:?}");
}

#[test]
fn rings_leave_the_middle_empty() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.ring(100.0, 100.0, 20.0, 3.0, WHITE, 255);
    assert_eq!(c.get(100, 100), BLACK);
    assert!(close(c.get(120, 100), WHITE) || c.get(120, 100).r > 100);
}

#[test]
fn gradients_go_from_top_to_bottom() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.gradient(Rect::new(0.0, 0.0, 320.0, 240.0), BLACK, WHITE);
    assert!(c.get(5, 0).r < 10);
    assert!(c.get(5, 239).r > 245);
    assert!(c.get(5, 60).r < c.get(5, 180).r);
}

#[test]
fn glows_fade_out_with_distance() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.glow(160.0, 120.0, 50.0, WHITE, 200);
    let (center, mid, edge) = (c.get(160, 120).r, c.get(185, 120).r, c.get(212, 120).r);
    assert!(center > mid && mid > edge, "{center} {mid} {edge}");
    assert_eq!(edge, 0);
}

#[test]
fn opacity_scales_shapes() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.round_rect(Rect::new(0.0, 0.0, 50.0, 50.0), 0.0, WHITE, 128);
    let p = c.get(25, 25);
    assert!((120..=136).contains(&p.r), "{p:?}");
}

#[test]
fn text_is_anti_aliased_and_measured_consistently() {
    use qday_ui::fonts::{BODY, HEADLINE};
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    let end = c.text(10, 50, "QDay", &TextStyle::new(&HEADLINE, WHITE));
    assert_eq!(end - 10, HEADLINE.width("QDay", 0));
    let row: Vec<u8> = (10..end)
        .flat_map(|x| (30..55).map(move |y| (x, y)))
        .map(|(x, y)| c.get(x, y).r)
        .collect();
    assert!(row.iter().any(|&r| r > 240), "solid ink");
    assert!(row.iter().any(|&r| r > 20 && r < 230), "soft edges");
    assert!(BODY.width("Send 0.5 ETH", 0) < HEADLINE.width("Send 0.5 ETH", 0));
}

#[test]
fn kerning_tightens_classic_pairs() {
    use qday_ui::fonts::HEADLINE;
    assert!(HEADLINE.kern('A', 'V') < 0, "AV is kerned in Inter");
    assert!(HEADLINE.width("AV", 0) < HEADLINE.width("A", 0) + HEADLINE.width("V", 0));
}

#[test]
fn lines_have_round_caps_and_soft_edges() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    c.line(20.0, 20.0, 120.0, 20.0, 4.0, WHITE, 255);
    assert_eq!(c.get(70, 20), WHITE, "on the line");
    assert_eq!(c.get(70, 30), BLACK, "away from it");
    assert!(c.get(18, 20).r > 0, "round cap past the start point");
    // A diagonal line's edges cross pixels, which must be partly covered.
    c.line(20.0, 60.0, 120.0, 100.0, 3.0, WHITE, 255);
    let soft = (60..100)
        .flat_map(|y| (20..120).map(move |x| (x, y)))
        .any(|(x, y)| {
            let p = c.get(x, y);
            p.r > 10 && p.r < 245
        });
    assert!(soft, "diagonal edges are anti-aliased");
}

#[test]
fn arcs_cover_only_their_sweep() {
    let mut buf = buffer();
    let mut c = Canvas::new(&mut buf);
    // Quarter arc from 12 o'clock clockwise to 3 o'clock.
    c.arc(160.0, 120.0, 40.0, 6.0, 0.0, 0.25, WHITE, 255);
    assert_eq!(c.get(188, 92), WHITE, "1:30 is inside the sweep");
    assert_eq!(c.get(132, 148), BLACK, "7:30 is outside");
    assert_eq!(c.get(160, 120), BLACK, "center stays empty");
}

#[test]
fn dithering_keeps_the_average_color_of_dark_gradients() {
    // A shade between two RGB565 steps comes out as a mix of both.
    let shade = Color::hex(0x0C0C0C);
    let mut sum = 0u32;
    for y in 0..4 {
        for x in 0..4 {
            let v = shade.to_565_dithered(x, y);
            sum += Color::from_565(v).r as u32;
        }
    }
    let average = sum as f32 / 16.0;
    assert!((average - 12.0).abs() < 2.5, "average {average}");
    let levels: std::collections::HashSet<u16> = (0..16)
        .map(|i| shade.to_565_dithered(i % 4, i / 4))
        .collect();
    assert!(levels.len() > 1, "dithering mixes neighbouring levels");
}

#[test]
fn text_gamma_thickens_or_thins_the_soft_edges() {
    use qday_ui::fonts::BODY;
    let ink = |gamma: u8| {
        let mut buf = buffer();
        let mut c = Canvas::new(&mut buf);
        c.text(
            10,
            30,
            "Hamburgefonts",
            &TextStyle::new(&BODY, WHITE).gamma(gamma),
        );
        (0..60)
            .flat_map(|y| (0..200).map(move |x| (x, y)))
            .map(|(x, y)| c.get(x, y).r as u32)
            .sum::<u32>()
    };
    let (thin, normal, heavy) = (ink(14), ink(10), ink(7));
    assert!(thin < normal && normal < heavy, "{thin} {normal} {heavy}");
}

mod fast_paths {
    use qday_ui::canvas::{Canvas, Color, HEIGHT, Rect, WIDTH};

    fn rng(state: &mut u32) -> f32 {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*state >> 8) as f32 / (1u32 << 24) as f32
    }

    fn pair() -> (Vec<u16>, Vec<u16>) {
        let n = (WIDTH * HEIGHT) as usize;
        (vec![0x1234; n], vec![0x1234; n])
    }

    /// The interior fill skips the per-pixel distance maths only where a
    /// pixel is certainly fully covered, so the result is pixel-identical.
    #[test]
    fn rounded_rects_match_the_exact_path() {
        let mut s = 7u32;
        let (mut a, mut b) = pair();
        for i in 0..3000 {
            let r = Rect::new(
                rng(&mut s) * 340.0 - 10.0,
                rng(&mut s) * 260.0 - 10.0,
                rng(&mut s) * 200.0,
                rng(&mut s) * 120.0,
            );
            let radius = rng(&mut s) * 40.0;
            let color = Color::hex((i as u32).wrapping_mul(2_654_435_761) >> 8);
            let opacity = if i % 3 == 0 {
                255
            } else {
                (rng(&mut s) * 255.0) as u8
            };
            Canvas::new(&mut a).round_rect(r, radius, color, opacity);
            Canvas::new(&mut b).round_rect_exact(r, radius, color, opacity);
            assert!(a == b, "rect {i}: {r:?} radius {radius} opacity {opacity}");
        }
    }

    #[test]
    fn circles_match_the_exact_path() {
        let mut s = 11u32;
        let (mut a, mut b) = pair();
        for i in 0..3000 {
            let (cx, cy, r) = (
                rng(&mut s) * 340.0 - 10.0,
                rng(&mut s) * 260.0 - 10.0,
                rng(&mut s) * 60.0,
            );
            let color = Color::hex((i as u32).wrapping_mul(2_246_822_519) >> 8);
            let opacity = if i % 3 == 0 {
                255
            } else {
                (rng(&mut s) * 255.0) as u8
            };
            Canvas::new(&mut a).circle(cx, cy, r, color, opacity);
            Canvas::new(&mut b).circle_exact(cx, cy, r, color, opacity);
            assert!(a == b, "circle {i}: ({cx}, {cy}) r {r} opacity {opacity}");
        }
    }

    #[test]
    fn darkening_rows_matches_blending_black() {
        let (mut a, mut b) = pair();
        for (i, (x, y)) in a.iter_mut().zip(b.iter_mut()).enumerate() {
            *x = ((i as u32).wrapping_mul(2_654_435_761) >> 16) as u16;
            *y = *x;
        }
        Canvas::new(&mut a).darken_rows(1, 2, 70);
        let mut c = Canvas::new(&mut b);
        for y in (1..HEIGHT).step_by(2) {
            for x in 0..WIDTH {
                c.blend(x, y, Color::hex(0), 70);
            }
        }
        assert!(a == b);
    }
}

#[test]
fn the_frame_bytes_are_what_the_display_takes() {
    use qday_ui::canvas::{Canvas, Color, HEIGHT, Rect, WIDTH};
    let n = (WIDTH * HEIGHT) as usize;
    let mut px = vec![0u16; n];
    let mut c = Canvas::new(&mut px);
    c.clear(Color::hex(0x6B4EE6));
    c.round_rect(
        Rect::new(10.0, 10.0, 200.0, 100.0),
        12.0,
        Color::hex(0xF7F7F7),
        200,
    );
    let mut words = vec![0u16; n];
    c.rows_565(0, HEIGHT, &mut words);
    let bytes = c.as_bytes();
    assert_eq!(bytes.len(), 2 * n);
    for (i, w) in words.iter().enumerate() {
        assert_eq!(
            [bytes[2 * i], bytes[2 * i + 1]],
            w.to_be_bytes(),
            "pixel {i}: RGB565, high byte first"
        );
    }
    assert_eq!(c.get(0, 0), Color::from_565(Color::hex(0x6B4EE6).to_565()));
}

mod outline_fast_paths {
    use qday_ui::canvas::{Canvas, Color, HEIGHT, Rect, WIDTH};

    fn rng(state: &mut u32) -> f32 {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*state >> 8) as f32 / (1u32 << 24) as f32
    }

    #[test]
    fn rounded_outlines_match_the_exact_path() {
        let n = (WIDTH * HEIGHT) as usize;
        let (mut a, mut b) = (vec![0x1234u16; n], vec![0x1234u16; n]);
        let mut s = 5u32;
        for i in 0..3000 {
            let r = Rect::new(
                rng(&mut s) * 340.0 - 10.0,
                rng(&mut s) * 260.0 - 10.0,
                rng(&mut s) * 200.0,
                rng(&mut s) * 120.0,
            );
            let (radius, width) = (rng(&mut s) * 40.0, 0.5 + rng(&mut s) * 6.0);
            let color = Color::hex((i as u32).wrapping_mul(2_654_435_761) >> 8);
            let opacity = if i % 2 == 0 {
                255
            } else {
                (rng(&mut s) * 255.0) as u8
            };
            Canvas::new(&mut a).round_rect_stroke(r, radius, width, color, opacity);
            Canvas::new(&mut b).round_rect_stroke_exact(r, radius, width, color, opacity);
            assert!(a == b, "outline {i}: {r:?} radius {radius} width {width}");
        }
    }

    #[test]
    fn rings_match_the_exact_path() {
        let n = (WIDTH * HEIGHT) as usize;
        let (mut a, mut b) = (vec![0x1234u16; n], vec![0x1234u16; n]);
        let mut s = 9u32;
        for i in 0..3000 {
            let (cx, cy, r, w) = (
                rng(&mut s) * 340.0 - 10.0,
                rng(&mut s) * 260.0 - 10.0,
                rng(&mut s) * 60.0,
                0.5 + rng(&mut s) * 8.0,
            );
            let color = Color::hex((i as u32).wrapping_mul(2_246_822_519) >> 8);
            Canvas::new(&mut a).ring(cx, cy, r, w, color, 255);
            Canvas::new(&mut b).ring_exact(cx, cy, r, w, color, 255);
            assert!(a == b, "ring {i}: ({cx}, {cy}) r {r} width {w}");
        }
    }
}

#[test]
fn bitmap_text_matches_embedded_graphics() {
    use embedded_graphics::mono_font::ascii::{FONT_8X13, FONT_10X20};
    use qday_ui::canvas::{Canvas, Color, HEIGHT, WIDTH};
    use qday_ui::widgets::{mono, mono_exact};
    let n = (WIDTH * HEIGHT) as usize;
    let (mut a, mut b) = (vec![0x0841u16; n], vec![0x0841u16; n]);
    let lines = [
        "STATUS ......... READY",
        "0X5AAE B605 3F3E 94C9",
        "approve? [y/n] ~!@#$%^&*()_+{}|",
        "\u{e9}t\u{e9} ok",
    ];
    for (i, text) in lines.iter().enumerate() {
        for (font, bloom) in [
            (&FONT_8X13, true),
            (&FONT_10X20, false),
            (&FONT_8X13, false),
        ] {
            let (x, y) = (-3 + 40 * i as i32, -5 + 50 * i as i32);
            let ea = mono(
                &mut Canvas::new(&mut a),
                x,
                y,
                text,
                font,
                Color::hex(0x33FF66),
                bloom,
            );
            let eb = mono_exact(
                &mut Canvas::new(&mut b),
                x,
                y,
                text,
                font,
                Color::hex(0x33FF66),
                bloom,
            );
            assert_eq!(ea, eb);
        }
    }
    assert!(a == b);
}
