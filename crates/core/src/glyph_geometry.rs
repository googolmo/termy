//! Renderer-neutral geometry for terminal glyphs that should not be shaped as text.
//!
//! Coordinates are relative to one terminal cell. Rectangle edges use `0.0..=1.0`
//! for the cell bounds (strokes may intentionally overshoot those bounds). Hosts
//! transform the geometry into their backend's coordinate space and perform the
//! final device-pixel snapping described by each rectangle's snap mode.

pub const MAX_TERMINAL_GLYPH_RECTS: usize = 8;
pub const MAX_TERMINAL_GLYPH_STROKES: usize = 2;
pub const MAX_TERMINAL_GLYPH_STROKE_POINTS: usize = 6;

const BOX_DRAWING_START: u32 = 0x2500;
const BOX_DRAWING_END: u32 = 0x257F;
const BLOCK_ELEMENTS_START: u32 = 0x2580;
const BLOCK_ELEMENTS_END: u32 = 0x259F;
const SEXTANT_MOSAIC_START: u32 = 0x1FB00;
const SEXTANT_MOSAIC_END: u32 = 0x1FB3B;
const BRAILLE_PATTERNS_START: u32 = 0x2800;
const BRAILLE_PATTERNS_END: u32 = 0x28FF;
const QUAD_UPPER_LEFT: u8 = 0b0001;
const QUAD_UPPER_RIGHT: u8 = 0b0010;
const QUAD_LOWER_LEFT: u8 = 0b0100;
const QUAD_LOWER_RIGHT: u8 = 0b1000;

/// Layout metrics used to resolve stroke widths and box-drawing joins.
///
/// Values must use the same coordinate space. For exact device-pixel output,
/// pass device-pixel metrics and transform the returned normalized geometry into
/// snapped device-pixel cell bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalGlyphMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    pub font_size: f32,
}

impl TerminalGlyphMetrics {
    fn is_usable(self) -> bool {
        self.cell_width.is_finite()
            && self.cell_width > 0.0
            && self.cell_height.is_finite()
            && self.cell_height > 0.0
            && self.font_size.is_finite()
            && self.font_size > 0.0
    }
}

/// Nearby codepoints used for context-sensitive terminal glyph decisions.
///
/// Termy renders Braille as geometry only in runs of at least three cells so
/// short animated Braille spinners keep their font-designed appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalGlyphNeighbors {
    pub two_before: Option<char>,
    pub before: Option<char>,
    pub after: Option<char>,
    pub two_after: Option<char>,
}

impl TerminalGlyphNeighbors {
    pub fn from_row(row: &[char], index: usize) -> Self {
        Self {
            two_before: index
                .checked_sub(2)
                .and_then(|index| row.get(index))
                .copied(),
            before: index
                .checked_sub(1)
                .and_then(|index| row.get(index))
                .copied(),
            after: row.get(index.saturating_add(1)).copied(),
            two_after: row.get(index.saturating_add(2)).copied(),
        }
    }
}

/// Why a cell bypasses ordinary font shaping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalGlyphRenderKind {
    BlockElement,
    BoxDrawing,
    Sextant,
    Braille,
    RoundedCorner,
    Diagonal,
}

/// How a host snaps a normalized rectangle after transforming it into device space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalGlyphRectSnap {
    /// Round each transformed edge to the nearest device-pixel boundary.
    #[default]
    Nearest,
    /// Floor the left/top edges and ceil the right/bottom edges.
    /// Used for sextants so fractional thirds cannot expose seams.
    Outward,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalGlyphRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub alpha: f32,
    pub snap: TerminalGlyphRectSnap,
}

impl TerminalGlyphRect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32, alpha: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
            alpha,
            snap: TerminalGlyphRectSnap::Nearest,
        }
    }

    const fn outward(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
            alpha: 1.0,
            snap: TerminalGlyphRectSnap::Outward,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalGlyphPoint {
    pub x: f32,
    pub y: f32,
}

impl TerminalGlyphPoint {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalGlyphStrokeKind {
    Line,
    RoundedCorner,
}

/// One normalized stroked path.
///
/// A line contains two points. A rounded corner contains six points in this
/// order: start, curve start, control A, control B, curve end, end. Its path is
/// `move(start) -> line(curve start) -> cubic(control A, control B, curve end)
/// -> line(end)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalGlyphStroke {
    pub kind: TerminalGlyphStrokeKind,
    pub points: [TerminalGlyphPoint; MAX_TERMINAL_GLYPH_STROKE_POINTS],
    pub point_count: u8,
    /// Stroke width as a fraction of the cell width.
    pub width: f32,
}

impl TerminalGlyphStroke {
    fn line(start: TerminalGlyphPoint, end: TerminalGlyphPoint, width: f32) -> Self {
        let mut points = [TerminalGlyphPoint::default(); MAX_TERMINAL_GLYPH_STROKE_POINTS];
        points[0] = start;
        points[1] = end;
        Self {
            kind: TerminalGlyphStrokeKind::Line,
            points,
            point_count: 2,
            width,
        }
    }

    fn rounded(points: [TerminalGlyphPoint; MAX_TERMINAL_GLYPH_STROKE_POINTS], width: f32) -> Self {
        Self {
            kind: TerminalGlyphStrokeKind::RoundedCorner,
            points,
            point_count: MAX_TERMINAL_GLYPH_STROKE_POINTS as u8,
            width,
        }
    }

    pub fn points(&self) -> &[TerminalGlyphPoint] {
        &self.points[..usize::from(self.point_count)]
    }
}

const EMPTY_RECT: TerminalGlyphRect = TerminalGlyphRect::new(0.0, 0.0, 0.0, 0.0, 0.0);
const EMPTY_POINT: TerminalGlyphPoint = TerminalGlyphPoint::new(0.0, 0.0);
const EMPTY_STROKE: TerminalGlyphStroke = TerminalGlyphStroke {
    kind: TerminalGlyphStrokeKind::Line,
    points: [EMPTY_POINT; MAX_TERMINAL_GLYPH_STROKE_POINTS],
    point_count: 0,
    width: 0.0,
};

/// Fixed-capacity, allocation-free plan for one special terminal glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalGlyphPlan {
    kind: TerminalGlyphRenderKind,
    geometry: TerminalGlyphGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TerminalGlyphGeometry {
    Rects {
        rects: [TerminalGlyphRect; MAX_TERMINAL_GLYPH_RECTS],
        count: u8,
    },
    Strokes {
        strokes: [TerminalGlyphStroke; MAX_TERMINAL_GLYPH_STROKES],
        count: u8,
    },
}

impl TerminalGlyphPlan {
    fn empty_rects(kind: TerminalGlyphRenderKind) -> Self {
        Self {
            kind,
            geometry: TerminalGlyphGeometry::Rects {
                rects: [EMPTY_RECT; MAX_TERMINAL_GLYPH_RECTS],
                count: 0,
            },
        }
    }

    fn empty_strokes(kind: TerminalGlyphRenderKind) -> Self {
        Self {
            kind,
            geometry: TerminalGlyphGeometry::Strokes {
                strokes: [EMPTY_STROKE; MAX_TERMINAL_GLYPH_STROKES],
                count: 0,
            },
        }
    }

    fn one_rect(kind: TerminalGlyphRenderKind, rect: TerminalGlyphRect) -> Self {
        let mut result = Self::empty_rects(kind);
        result.push_rect(rect);
        result
    }

    fn one_stroke(kind: TerminalGlyphRenderKind, stroke: TerminalGlyphStroke) -> Self {
        let mut result = Self::empty_strokes(kind);
        result.push_stroke(stroke);
        result
    }

    pub const fn kind(&self) -> TerminalGlyphRenderKind {
        self.kind
    }

    pub fn rects(&self) -> &[TerminalGlyphRect] {
        match &self.geometry {
            TerminalGlyphGeometry::Rects { rects, count } => &rects[..usize::from(*count)],
            TerminalGlyphGeometry::Strokes { .. } => &[],
        }
    }

    pub fn strokes(&self) -> &[TerminalGlyphStroke] {
        match &self.geometry {
            TerminalGlyphGeometry::Rects { .. } => &[],
            TerminalGlyphGeometry::Strokes { strokes, count } => &strokes[..usize::from(*count)],
        }
    }

    fn push_rect(&mut self, rect: TerminalGlyphRect) {
        let TerminalGlyphGeometry::Rects { rects, count } = &mut self.geometry else {
            debug_assert!(false, "cannot add a rectangle to a stroke plan");
            return;
        };
        let index = usize::from(*count);
        debug_assert!(index < rects.len(), "glyph rectangle capacity exceeded");
        if index < rects.len() {
            rects[index] = rect;
            *count += 1;
        }
    }

    fn push_stroke(&mut self, stroke: TerminalGlyphStroke) {
        let TerminalGlyphGeometry::Strokes { strokes, count } = &mut self.geometry else {
            debug_assert!(false, "cannot add a stroke to a rectangle plan");
            return;
        };
        let index = usize::from(*count);
        debug_assert!(index < strokes.len(), "glyph stroke capacity exceeded");
        if index < strokes.len() {
            strokes[index] = stroke;
            *count += 1;
        }
    }

    fn merge_collinear_rects(&mut self) {
        const EPSILON: f32 = 1e-6;
        let TerminalGlyphGeometry::Rects { rects, count } = &mut self.geometry else {
            return;
        };

        let mut i = 0;
        while i < usize::from(*count) {
            let mut j = i + 1;
            while j < usize::from(*count) {
                let a = rects[i];
                let b = rects[j];
                let same_vertical_track = (a.left - b.left).abs() <= EPSILON
                    && (a.right - b.right).abs() <= EPSILON
                    && a.top <= b.bottom + EPSILON
                    && b.top <= a.bottom + EPSILON;
                let same_horizontal_track = (a.top - b.top).abs() <= EPSILON
                    && (a.bottom - b.bottom).abs() <= EPSILON
                    && a.left <= b.right + EPSILON
                    && b.left <= a.right + EPSILON;

                if same_vertical_track || same_horizontal_track {
                    rects[i] = TerminalGlyphRect::new(
                        a.left.min(b.left),
                        a.top.min(b.top),
                        a.right.max(b.right),
                        a.bottom.max(b.bottom),
                        a.alpha.max(b.alpha),
                    );
                    let current_count = usize::from(*count);
                    for index in j..(current_count - 1) {
                        rects[index] = rects[index + 1];
                    }
                    rects[current_count - 1] = EMPTY_RECT;
                    *count -= 1;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
}

/// Returns the canonical special-glyph plan for one cell, or `None` when the
/// character should use ordinary font shaping.
pub fn terminal_glyph_plan(
    character: char,
    metrics: TerminalGlyphMetrics,
    neighbors: TerminalGlyphNeighbors,
) -> Option<TerminalGlyphPlan> {
    if let Some(plan) = block_element_plan(character) {
        return Some(plan);
    }
    if let Some(plan) = sextant_plan(character) {
        return Some(plan);
    }
    if should_render_braille_as_geometry(character, neighbors) {
        return braille_plan(character);
    }
    if !metrics.is_usable() {
        return None;
    }
    rounded_corner_plan(character, metrics)
        .or_else(|| diagonal_plan(character, metrics))
        .or_else(|| box_drawing_plan(character, metrics))
}

fn full_cell_rect(alpha: f32) -> TerminalGlyphRect {
    TerminalGlyphRect::new(0.0, 0.0, 1.0, 1.0, alpha)
}

fn vertical_fill_from_bottom(fraction: f32) -> TerminalGlyphPlan {
    TerminalGlyphPlan::one_rect(
        TerminalGlyphRenderKind::BlockElement,
        TerminalGlyphRect::new(0.0, 1.0 - fraction, 1.0, 1.0, 1.0),
    )
}

fn horizontal_fill_from_left(fraction: f32) -> TerminalGlyphPlan {
    TerminalGlyphPlan::one_rect(
        TerminalGlyphRenderKind::BlockElement,
        TerminalGlyphRect::new(0.0, 0.0, fraction, 1.0, 1.0),
    )
}

fn quadrants(mask: u8) -> TerminalGlyphPlan {
    let mut plan = TerminalGlyphPlan::empty_rects(TerminalGlyphRenderKind::BlockElement);
    if mask & QUAD_UPPER_LEFT != 0 {
        plan.push_rect(TerminalGlyphRect::new(0.0, 0.0, 0.5, 0.5, 1.0));
    }
    if mask & QUAD_UPPER_RIGHT != 0 {
        plan.push_rect(TerminalGlyphRect::new(0.5, 0.0, 1.0, 0.5, 1.0));
    }
    if mask & QUAD_LOWER_LEFT != 0 {
        plan.push_rect(TerminalGlyphRect::new(0.0, 0.5, 0.5, 1.0, 1.0));
    }
    if mask & QUAD_LOWER_RIGHT != 0 {
        plan.push_rect(TerminalGlyphRect::new(0.5, 0.5, 1.0, 1.0, 1.0));
    }
    plan
}

fn block_element_plan(character: char) -> Option<TerminalGlyphPlan> {
    if !(BLOCK_ELEMENTS_START..=BLOCK_ELEMENTS_END).contains(&(character as u32)) {
        return None;
    }

    Some(match character {
        '\u{2580}' => TerminalGlyphPlan::one_rect(
            TerminalGlyphRenderKind::BlockElement,
            TerminalGlyphRect::new(0.0, 0.0, 1.0, 0.5, 1.0),
        ),
        '\u{2581}' => vertical_fill_from_bottom(1.0 / 8.0),
        '\u{2582}' => vertical_fill_from_bottom(2.0 / 8.0),
        '\u{2583}' => vertical_fill_from_bottom(3.0 / 8.0),
        '\u{2584}' => vertical_fill_from_bottom(4.0 / 8.0),
        '\u{2585}' => vertical_fill_from_bottom(5.0 / 8.0),
        '\u{2586}' => vertical_fill_from_bottom(6.0 / 8.0),
        '\u{2587}' => vertical_fill_from_bottom(7.0 / 8.0),
        '\u{2588}' => {
            TerminalGlyphPlan::one_rect(TerminalGlyphRenderKind::BlockElement, full_cell_rect(1.0))
        }
        '\u{2589}' => horizontal_fill_from_left(7.0 / 8.0),
        '\u{258A}' => horizontal_fill_from_left(6.0 / 8.0),
        '\u{258B}' => horizontal_fill_from_left(5.0 / 8.0),
        '\u{258C}' => horizontal_fill_from_left(4.0 / 8.0),
        '\u{258D}' => horizontal_fill_from_left(3.0 / 8.0),
        '\u{258E}' => horizontal_fill_from_left(2.0 / 8.0),
        '\u{258F}' => horizontal_fill_from_left(1.0 / 8.0),
        '\u{2590}' => TerminalGlyphPlan::one_rect(
            TerminalGlyphRenderKind::BlockElement,
            TerminalGlyphRect::new(0.5, 0.0, 1.0, 1.0, 1.0),
        ),
        '\u{2591}' => {
            TerminalGlyphPlan::one_rect(TerminalGlyphRenderKind::BlockElement, full_cell_rect(0.25))
        }
        '\u{2592}' => {
            TerminalGlyphPlan::one_rect(TerminalGlyphRenderKind::BlockElement, full_cell_rect(0.50))
        }
        '\u{2593}' => {
            TerminalGlyphPlan::one_rect(TerminalGlyphRenderKind::BlockElement, full_cell_rect(0.75))
        }
        '\u{2594}' => TerminalGlyphPlan::one_rect(
            TerminalGlyphRenderKind::BlockElement,
            TerminalGlyphRect::new(0.0, 0.0, 1.0, 1.0 / 8.0, 1.0),
        ),
        '\u{2595}' => TerminalGlyphPlan::one_rect(
            TerminalGlyphRenderKind::BlockElement,
            TerminalGlyphRect::new(7.0 / 8.0, 0.0, 1.0, 1.0, 1.0),
        ),
        '\u{2596}' => quadrants(QUAD_LOWER_LEFT),
        '\u{2597}' => quadrants(QUAD_LOWER_RIGHT),
        '\u{2598}' => quadrants(QUAD_UPPER_LEFT),
        '\u{2599}' => quadrants(QUAD_UPPER_LEFT | QUAD_LOWER_LEFT | QUAD_LOWER_RIGHT),
        '\u{259A}' => quadrants(QUAD_UPPER_LEFT | QUAD_LOWER_RIGHT),
        '\u{259B}' => quadrants(QUAD_UPPER_LEFT | QUAD_UPPER_RIGHT | QUAD_LOWER_LEFT),
        '\u{259C}' => quadrants(QUAD_UPPER_LEFT | QUAD_UPPER_RIGHT | QUAD_LOWER_RIGHT),
        '\u{259D}' => quadrants(QUAD_UPPER_RIGHT),
        '\u{259E}' => quadrants(QUAD_UPPER_RIGHT | QUAD_LOWER_LEFT),
        '\u{259F}' => quadrants(QUAD_UPPER_RIGHT | QUAD_LOWER_LEFT | QUAD_LOWER_RIGHT),
        _ => return None,
    })
}

fn reverse_lower_six_bits(value: u8) -> u8 {
    ((value & 0b00_0001) << 5)
        | ((value & 0b00_0010) << 3)
        | ((value & 0b00_0100) << 1)
        | ((value & 0b00_1000) >> 1)
        | ((value & 0b01_0000) >> 3)
        | ((value & 0b10_0000) >> 5)
}

fn sextant_char_to_packed(character: char) -> Option<u8> {
    let codepoint = character as u32;
    if !(SEXTANT_MOSAIC_START..=SEXTANT_MOSAIC_END).contains(&codepoint) {
        return None;
    }
    let offset = codepoint - SEXTANT_MOSAIC_START;
    let sextant = (offset + 1 + u32::from(offset >= 20) + u32::from(offset >= 40)) as u8;
    Some(reverse_lower_six_bits(sextant) ^ 0b11_1111)
}

fn sextant_plan(character: char) -> Option<TerminalGlyphPlan> {
    let packed = sextant_char_to_packed(character)?;
    let mut plan = TerminalGlyphPlan::empty_rects(TerminalGlyphRenderKind::Sextant);
    for row in 0..3usize {
        for col in 0..2usize {
            let bit = 5usize - (row * 2 + col);
            if packed & (1 << bit) == 0 {
                plan.push_rect(TerminalGlyphRect::outward(
                    col as f32 / 2.0,
                    row as f32 / 3.0,
                    (col + 1) as f32 / 2.0,
                    (row + 1) as f32 / 3.0,
                ));
            }
        }
    }
    Some(plan)
}

fn is_braille_pattern_char(character: char) -> bool {
    (BRAILLE_PATTERNS_START..=BRAILLE_PATTERNS_END).contains(&(character as u32))
}

fn should_render_braille_as_geometry(character: char, neighbors: TerminalGlyphNeighbors) -> bool {
    if !is_braille_pattern_char(character) {
        return false;
    }
    let is_braille = |candidate: Option<char>| candidate.is_some_and(is_braille_pattern_char);
    (is_braille(neighbors.two_before) && is_braille(neighbors.before))
        || (is_braille(neighbors.before) && is_braille(neighbors.after))
        || (is_braille(neighbors.after) && is_braille(neighbors.two_after))
}

fn braille_plan(character: char) -> Option<TerminalGlyphPlan> {
    if !is_braille_pattern_char(character) {
        return None;
    }
    let pattern = (character as u32 - BRAILLE_PATTERNS_START) as u8;
    if pattern == 0 {
        return None;
    }

    const DOT_WIDTH: f32 = 0.24;
    const DOT_HEIGHT: f32 = 0.16;
    const LEFT_X: f32 = 0.22;
    const RIGHT_X: f32 = 0.64;
    const ROW_Y: [f32; 4] = [0.08, 0.31, 0.54, 0.77];
    const DOT_MASKS: [(u8, f32, f32); 8] = [
        (0b0000_0001, LEFT_X, ROW_Y[0]),
        (0b0000_0010, LEFT_X, ROW_Y[1]),
        (0b0000_0100, LEFT_X, ROW_Y[2]),
        (0b0100_0000, LEFT_X, ROW_Y[3]),
        (0b0000_1000, RIGHT_X, ROW_Y[0]),
        (0b0001_0000, RIGHT_X, ROW_Y[1]),
        (0b0010_0000, RIGHT_X, ROW_Y[2]),
        (0b1000_0000, RIGHT_X, ROW_Y[3]),
    ];

    let mut plan = TerminalGlyphPlan::empty_rects(TerminalGlyphRenderKind::Braille);
    for (mask, left, top) in DOT_MASKS {
        if pattern & mask != 0 {
            plan.push_rect(TerminalGlyphRect::new(
                left,
                top,
                (left + DOT_WIDTH).min(1.0),
                (top + DOT_HEIGHT).min(1.0),
                1.0,
            ));
        }
    }
    Some(plan)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoxLineStyle {
    None,
    Light,
    Heavy,
    Double,
}

impl BoxLineStyle {
    fn is_double(self) -> bool {
        self == Self::Double
    }

    fn is_heavy(self) -> bool {
        self == Self::Heavy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoxDrawSegments {
    up: BoxLineStyle,
    down: BoxLineStyle,
    left: BoxLineStyle,
    right: BoxLineStyle,
}

const fn box_segments(
    up: BoxLineStyle,
    down: BoxLineStyle,
    left: BoxLineStyle,
    right: BoxLineStyle,
) -> BoxDrawSegments {
    BoxDrawSegments {
        up,
        down,
        left,
        right,
    }
}

#[allow(clippy::too_many_lines)]
fn box_draw_segments(character: char) -> Option<BoxDrawSegments> {
    use BoxLineStyle::{Double, Heavy, Light, None as Empty};

    if !(BOX_DRAWING_START..=BOX_DRAWING_END).contains(&(character as u32)) {
        return None;
    }

    Some(match character {
        '\u{2500}' | '\u{2504}' | '\u{2508}' | '\u{254C}' => {
            box_segments(Empty, Empty, Light, Light)
        }
        '\u{2501}' | '\u{2505}' | '\u{2509}' | '\u{254D}' => {
            box_segments(Empty, Empty, Heavy, Heavy)
        }
        '\u{2502}' | '\u{2506}' | '\u{250A}' | '\u{254E}' => {
            box_segments(Light, Light, Empty, Empty)
        }
        '\u{2503}' | '\u{2507}' | '\u{250B}' | '\u{254F}' => {
            box_segments(Heavy, Heavy, Empty, Empty)
        }
        '\u{250C}' => box_segments(Empty, Light, Empty, Light),
        '\u{250D}' => box_segments(Empty, Light, Empty, Heavy),
        '\u{250E}' => box_segments(Empty, Heavy, Empty, Light),
        '\u{250F}' => box_segments(Empty, Heavy, Empty, Heavy),
        '\u{2510}' => box_segments(Empty, Light, Light, Empty),
        '\u{2511}' => box_segments(Empty, Light, Heavy, Empty),
        '\u{2512}' => box_segments(Empty, Heavy, Light, Empty),
        '\u{2513}' => box_segments(Empty, Heavy, Heavy, Empty),
        '\u{2514}' => box_segments(Light, Empty, Empty, Light),
        '\u{2515}' => box_segments(Light, Empty, Empty, Heavy),
        '\u{2516}' => box_segments(Heavy, Empty, Empty, Light),
        '\u{2517}' => box_segments(Heavy, Empty, Empty, Heavy),
        '\u{2518}' => box_segments(Light, Empty, Light, Empty),
        '\u{2519}' => box_segments(Light, Empty, Heavy, Empty),
        '\u{251A}' => box_segments(Heavy, Empty, Light, Empty),
        '\u{251B}' => box_segments(Heavy, Empty, Heavy, Empty),
        '\u{251C}' => box_segments(Light, Light, Empty, Light),
        '\u{251D}' => box_segments(Light, Light, Empty, Heavy),
        '\u{251E}' => box_segments(Heavy, Light, Empty, Light),
        '\u{251F}' => box_segments(Light, Heavy, Empty, Light),
        '\u{2520}' => box_segments(Heavy, Heavy, Empty, Light),
        '\u{2521}' => box_segments(Light, Heavy, Empty, Heavy),
        '\u{2522}' => box_segments(Heavy, Light, Empty, Heavy),
        '\u{2523}' => box_segments(Heavy, Heavy, Empty, Heavy),
        '\u{2524}' => box_segments(Light, Light, Light, Empty),
        '\u{2525}' => box_segments(Light, Light, Heavy, Empty),
        '\u{2526}' => box_segments(Heavy, Light, Light, Empty),
        '\u{2527}' => box_segments(Light, Heavy, Light, Empty),
        '\u{2528}' => box_segments(Heavy, Heavy, Light, Empty),
        '\u{2529}' => box_segments(Light, Heavy, Heavy, Empty),
        '\u{252A}' => box_segments(Heavy, Light, Heavy, Empty),
        '\u{252B}' => box_segments(Heavy, Heavy, Heavy, Empty),
        '\u{252C}' => box_segments(Empty, Light, Light, Light),
        '\u{252D}' => box_segments(Empty, Light, Heavy, Light),
        '\u{252E}' => box_segments(Empty, Light, Light, Heavy),
        '\u{252F}' => box_segments(Empty, Light, Heavy, Heavy),
        '\u{2530}' => box_segments(Empty, Heavy, Light, Light),
        '\u{2531}' => box_segments(Empty, Heavy, Heavy, Light),
        '\u{2532}' => box_segments(Empty, Heavy, Light, Heavy),
        '\u{2533}' => box_segments(Empty, Heavy, Heavy, Heavy),
        '\u{2534}' => box_segments(Light, Empty, Light, Light),
        '\u{2535}' => box_segments(Light, Empty, Heavy, Light),
        '\u{2536}' => box_segments(Light, Empty, Light, Heavy),
        '\u{2537}' => box_segments(Light, Empty, Heavy, Heavy),
        '\u{2538}' => box_segments(Heavy, Empty, Light, Light),
        '\u{2539}' => box_segments(Heavy, Empty, Heavy, Light),
        '\u{253A}' => box_segments(Heavy, Empty, Light, Heavy),
        '\u{253B}' => box_segments(Heavy, Empty, Heavy, Heavy),
        '\u{253C}' => box_segments(Light, Light, Light, Light),
        '\u{253D}' => box_segments(Light, Light, Heavy, Light),
        '\u{253E}' => box_segments(Light, Light, Light, Heavy),
        '\u{253F}' => box_segments(Light, Light, Heavy, Heavy),
        '\u{2540}' => box_segments(Heavy, Light, Light, Light),
        '\u{2541}' => box_segments(Light, Heavy, Light, Light),
        '\u{2542}' => box_segments(Heavy, Heavy, Light, Light),
        '\u{2543}' => box_segments(Heavy, Light, Heavy, Light),
        '\u{2544}' => box_segments(Heavy, Light, Light, Heavy),
        '\u{2545}' => box_segments(Light, Heavy, Heavy, Light),
        '\u{2546}' => box_segments(Light, Heavy, Light, Heavy),
        '\u{2547}' => box_segments(Light, Heavy, Heavy, Heavy),
        '\u{2548}' => box_segments(Heavy, Light, Heavy, Heavy),
        '\u{2549}' => box_segments(Heavy, Heavy, Heavy, Light),
        '\u{254A}' => box_segments(Heavy, Heavy, Light, Heavy),
        '\u{254B}' => box_segments(Heavy, Heavy, Heavy, Heavy),
        '\u{2550}' => box_segments(Empty, Empty, Double, Double),
        '\u{2551}' => box_segments(Double, Double, Empty, Empty),
        '\u{2552}' => box_segments(Empty, Light, Empty, Double),
        '\u{2553}' => box_segments(Empty, Double, Empty, Light),
        '\u{2554}' => box_segments(Empty, Double, Empty, Double),
        '\u{2555}' => box_segments(Empty, Light, Double, Empty),
        '\u{2556}' => box_segments(Empty, Double, Light, Empty),
        '\u{2557}' => box_segments(Empty, Double, Double, Empty),
        '\u{2558}' => box_segments(Light, Empty, Empty, Double),
        '\u{2559}' => box_segments(Double, Empty, Empty, Light),
        '\u{255A}' => box_segments(Double, Empty, Empty, Double),
        '\u{255B}' => box_segments(Light, Empty, Double, Empty),
        '\u{255C}' => box_segments(Double, Empty, Light, Empty),
        '\u{255D}' => box_segments(Double, Empty, Double, Empty),
        '\u{255E}' => box_segments(Light, Light, Empty, Double),
        '\u{255F}' => box_segments(Double, Double, Empty, Light),
        '\u{2560}' => box_segments(Double, Double, Empty, Double),
        '\u{2561}' => box_segments(Light, Light, Double, Empty),
        '\u{2562}' => box_segments(Double, Double, Light, Empty),
        '\u{2563}' => box_segments(Double, Double, Double, Empty),
        '\u{2564}' => box_segments(Empty, Light, Double, Double),
        '\u{2565}' => box_segments(Empty, Double, Light, Light),
        '\u{2566}' => box_segments(Empty, Double, Double, Double),
        '\u{2567}' => box_segments(Light, Empty, Double, Double),
        '\u{2568}' => box_segments(Double, Empty, Light, Light),
        '\u{2569}' => box_segments(Double, Empty, Double, Double),
        '\u{256A}' => box_segments(Light, Light, Double, Double),
        '\u{256B}' => box_segments(Double, Double, Light, Light),
        '\u{256C}' => box_segments(Double, Double, Double, Double),
        '\u{256D}'..='\u{2573}' => return None,
        '\u{2574}' => box_segments(Empty, Empty, Light, Empty),
        '\u{2575}' => box_segments(Light, Empty, Empty, Empty),
        '\u{2576}' => box_segments(Empty, Empty, Empty, Light),
        '\u{2577}' => box_segments(Empty, Light, Empty, Empty),
        '\u{2578}' => box_segments(Empty, Empty, Heavy, Empty),
        '\u{2579}' => box_segments(Heavy, Empty, Empty, Empty),
        '\u{257A}' => box_segments(Empty, Empty, Empty, Heavy),
        '\u{257B}' => box_segments(Empty, Heavy, Empty, Empty),
        '\u{257C}' => box_segments(Empty, Empty, Light, Heavy),
        '\u{257D}' => box_segments(Light, Heavy, Empty, Empty),
        '\u{257E}' => box_segments(Empty, Empty, Heavy, Light),
        '\u{257F}' => box_segments(Heavy, Light, Empty, Empty),
        _ => return None,
    })
}

fn push_box_rect_px(
    plan: &mut TerminalGlyphPlan,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    metrics: TerminalGlyphMetrics,
) {
    let left = left.clamp(0.0, metrics.cell_width);
    let right = right.clamp(0.0, metrics.cell_width);
    let top = top.clamp(0.0, metrics.cell_height);
    let bottom = bottom.clamp(0.0, metrics.cell_height);
    if right <= left || bottom <= top {
        return;
    }
    plan.push_rect(TerminalGlyphRect::new(
        left / metrics.cell_width,
        top / metrics.cell_height,
        right / metrics.cell_width,
        bottom / metrics.cell_height,
        1.0,
    ));
}

fn box_drawing_plan(character: char, metrics: TerminalGlyphMetrics) -> Option<TerminalGlyphPlan> {
    use BoxLineStyle::{Double, Heavy, Light, None as Empty};

    let segments = box_draw_segments(character)?;
    let light = (metrics.font_size * 0.0675).ceil().max(1.0);
    let heavy = light * 2.0;
    let h_light_top = snapped_line_start(metrics.cell_height, light);
    let h_light_bottom = (h_light_top + light).min(metrics.cell_height);
    let h_heavy_top = snapped_line_start(metrics.cell_height, heavy);
    let h_heavy_bottom = (h_heavy_top + heavy).min(metrics.cell_height);
    let h_double_top = (h_light_top - light).max(0.0);
    let h_double_bottom = (h_light_bottom + light).min(metrics.cell_height);
    let v_light_left = snapped_line_start(metrics.cell_width, light);
    let v_light_right = (v_light_left + light).min(metrics.cell_width);
    let v_heavy_left = snapped_line_start(metrics.cell_width, heavy);
    let v_heavy_right = (v_heavy_left + heavy).min(metrics.cell_width);
    let v_double_left = (v_light_left - light).max(0.0);
    let v_double_right = (v_light_right + light).min(metrics.cell_width);

    let up_bottom = if segments.left.is_heavy() || segments.right.is_heavy() {
        h_heavy_bottom
    } else if segments.left != segments.right || segments.down == segments.up {
        if segments.left.is_double() || segments.right.is_double() {
            h_double_bottom
        } else {
            h_light_bottom
        }
    } else if segments.left == Empty && segments.right == Empty {
        h_light_bottom
    } else {
        h_light_top
    };
    let down_top = if segments.left.is_heavy() || segments.right.is_heavy() {
        h_heavy_top
    } else if segments.left != segments.right || segments.up == segments.down {
        if segments.left.is_double() || segments.right.is_double() {
            h_double_top
        } else {
            h_light_top
        }
    } else if segments.left == Empty && segments.right == Empty {
        h_light_top
    } else {
        h_light_bottom
    };
    let left_right = if segments.up.is_heavy() || segments.down.is_heavy() {
        v_heavy_right
    } else if segments.up != segments.down || segments.left == segments.right {
        if segments.up.is_double() || segments.down.is_double() {
            v_double_right
        } else {
            v_light_right
        }
    } else if segments.up == Empty && segments.down == Empty {
        v_light_right
    } else {
        v_light_left
    };
    let right_left = if segments.up.is_heavy() || segments.down.is_heavy() {
        v_heavy_left
    } else if segments.up != segments.down || segments.right == segments.left {
        if segments.up.is_double() || segments.down.is_double() {
            v_double_left
        } else {
            v_light_left
        }
    } else if segments.up == Empty && segments.down == Empty {
        v_light_left
    } else {
        v_light_right
    };

    let mut plan = TerminalGlyphPlan::empty_rects(TerminalGlyphRenderKind::BoxDrawing);
    match segments.up {
        Empty => {}
        Light => push_box_rect_px(
            &mut plan,
            v_light_left,
            0.0,
            v_light_right,
            up_bottom,
            metrics,
        ),
        Heavy => push_box_rect_px(
            &mut plan,
            v_heavy_left,
            0.0,
            v_heavy_right,
            up_bottom,
            metrics,
        ),
        Double => {
            let left_bottom = if segments.left == Double {
                h_light_top
            } else {
                up_bottom
            };
            let right_bottom = if segments.right == Double {
                h_light_top
            } else {
                up_bottom
            };
            push_box_rect_px(
                &mut plan,
                v_double_left,
                0.0,
                v_light_left,
                left_bottom,
                metrics,
            );
            push_box_rect_px(
                &mut plan,
                v_light_right,
                0.0,
                v_double_right,
                right_bottom,
                metrics,
            );
        }
    }
    match segments.right {
        Empty => {}
        Light => push_box_rect_px(
            &mut plan,
            right_left,
            h_light_top,
            metrics.cell_width,
            h_light_bottom,
            metrics,
        ),
        Heavy => push_box_rect_px(
            &mut plan,
            right_left,
            h_heavy_top,
            metrics.cell_width,
            h_heavy_bottom,
            metrics,
        ),
        Double => {
            let top_left = if segments.up == Double {
                v_light_right
            } else {
                right_left
            };
            let bottom_left = if segments.down == Double {
                v_light_right
            } else {
                right_left
            };
            push_box_rect_px(
                &mut plan,
                top_left,
                h_double_top,
                metrics.cell_width,
                h_light_top,
                metrics,
            );
            push_box_rect_px(
                &mut plan,
                bottom_left,
                h_light_bottom,
                metrics.cell_width,
                h_double_bottom,
                metrics,
            );
        }
    }
    match segments.down {
        Empty => {}
        Light => push_box_rect_px(
            &mut plan,
            v_light_left,
            down_top,
            v_light_right,
            metrics.cell_height,
            metrics,
        ),
        Heavy => push_box_rect_px(
            &mut plan,
            v_heavy_left,
            down_top,
            v_heavy_right,
            metrics.cell_height,
            metrics,
        ),
        Double => {
            let left_top = if segments.left == Double {
                h_light_bottom
            } else {
                down_top
            };
            let right_top = if segments.right == Double {
                h_light_bottom
            } else {
                down_top
            };
            push_box_rect_px(
                &mut plan,
                v_double_left,
                left_top,
                v_light_left,
                metrics.cell_height,
                metrics,
            );
            push_box_rect_px(
                &mut plan,
                v_light_right,
                right_top,
                v_double_right,
                metrics.cell_height,
                metrics,
            );
        }
    }
    match segments.left {
        Empty => {}
        Light => push_box_rect_px(
            &mut plan,
            0.0,
            h_light_top,
            left_right,
            h_light_bottom,
            metrics,
        ),
        Heavy => push_box_rect_px(
            &mut plan,
            0.0,
            h_heavy_top,
            left_right,
            h_heavy_bottom,
            metrics,
        ),
        Double => {
            let top_right = if segments.up == Double {
                v_light_left
            } else {
                left_right
            };
            let bottom_right = if segments.down == Double {
                v_light_left
            } else {
                left_right
            };
            push_box_rect_px(
                &mut plan,
                0.0,
                h_double_top,
                top_right,
                h_light_top,
                metrics,
            );
            push_box_rect_px(
                &mut plan,
                0.0,
                h_light_bottom,
                bottom_right,
                h_double_bottom,
                metrics,
            );
        }
    }
    plan.merge_collinear_rects();
    Some(plan)
}

/// Leading edge of a centered line, on the same whole pixel that
/// `snapped_stroke_center` uses for rounded corners. Centering on a half pixel
/// (odd line in an even cell) makes host edge rounding collapse thin lines,
/// which dropped one stroke of double-line glyphs.
fn snapped_line_start(size: f32, width: f32) -> f32 {
    (size / 2.0 - width / 2.0)
        .round()
        .clamp(0.0, (size - width).max(0.0))
}

fn snapped_stroke_center(size: f32, stroke_width: f32) -> f32 {
    let center = size / 2.0;
    let min = (center - stroke_width / 2.0).round();
    let max = (center + stroke_width / 2.0).round();
    (min + max) / 2.0
}

fn normalized_point(x: f32, y: f32, metrics: TerminalGlyphMetrics) -> TerminalGlyphPoint {
    TerminalGlyphPoint::new(x / metrics.cell_width, y / metrics.cell_height)
}

fn rounded_corner_plan(
    character: char,
    metrics: TerminalGlyphMetrics,
) -> Option<TerminalGlyphPlan> {
    if !matches!(character, '\u{256D}' | '\u{256E}' | '\u{256F}' | '\u{2570}') {
        return None;
    }
    let stroke = (metrics.font_size * 0.0675).ceil().max(1.0);
    let radius = ((metrics.cell_width.min(metrics.cell_height) - stroke).max(0.0)) / 2.0;
    let control = radius / 4.0;
    let overlap = stroke / 2.0;
    let center_x = snapped_stroke_center(metrics.cell_width, stroke);
    let center_y = snapped_stroke_center(metrics.cell_height, stroke);

    let points = match character {
        '\u{256D}' => [
            normalized_point(center_x, metrics.cell_height + overlap, metrics),
            normalized_point(center_x, center_y + radius, metrics),
            normalized_point(center_x, center_y + control, metrics),
            normalized_point(center_x + control, center_y, metrics),
            normalized_point(center_x + radius, center_y, metrics),
            normalized_point(metrics.cell_width + overlap, center_y, metrics),
        ],
        '\u{256E}' => [
            normalized_point(center_x, metrics.cell_height + overlap, metrics),
            normalized_point(center_x, center_y + radius, metrics),
            normalized_point(center_x, center_y + control, metrics),
            normalized_point(center_x - control, center_y, metrics),
            normalized_point(center_x - radius, center_y, metrics),
            normalized_point(-overlap, center_y, metrics),
        ],
        '\u{256F}' => [
            normalized_point(center_x, -overlap, metrics),
            normalized_point(center_x, center_y - radius, metrics),
            normalized_point(center_x, center_y - control, metrics),
            normalized_point(center_x - control, center_y, metrics),
            normalized_point(center_x - radius, center_y, metrics),
            normalized_point(-overlap, center_y, metrics),
        ],
        '\u{2570}' => [
            normalized_point(center_x, -overlap, metrics),
            normalized_point(center_x, center_y - radius, metrics),
            normalized_point(center_x, center_y - control, metrics),
            normalized_point(center_x + control, center_y, metrics),
            normalized_point(center_x + radius, center_y, metrics),
            normalized_point(metrics.cell_width + overlap, center_y, metrics),
        ],
        _ => return None,
    };
    Some(TerminalGlyphPlan::one_stroke(
        TerminalGlyphRenderKind::RoundedCorner,
        TerminalGlyphStroke::rounded(points, stroke / metrics.cell_width),
    ))
}

fn diagonal_plan(character: char, metrics: TerminalGlyphMetrics) -> Option<TerminalGlyphPlan> {
    if !matches!(character, '\u{2571}' | '\u{2572}' | '\u{2573}') {
        return None;
    }
    let stroke = (metrics.font_size * 0.0675).ceil().max(1.0) / metrics.cell_width;
    let slope_x = 0.5 * (metrics.cell_width / metrics.cell_height).min(1.0);
    let slope_y = 0.5 * (metrics.cell_height / metrics.cell_width).min(1.0);
    let upper_right_to_lower_left = TerminalGlyphStroke::line(
        normalized_point(metrics.cell_width + slope_x, -slope_y, metrics),
        normalized_point(-slope_x, metrics.cell_height + slope_y, metrics),
        stroke,
    );
    let upper_left_to_lower_right = TerminalGlyphStroke::line(
        normalized_point(-slope_x, -slope_y, metrics),
        normalized_point(
            metrics.cell_width + slope_x,
            metrics.cell_height + slope_y,
            metrics,
        ),
        stroke,
    );
    let mut plan = TerminalGlyphPlan::empty_strokes(TerminalGlyphRenderKind::Diagonal);
    match character {
        '\u{2571}' => plan.push_stroke(upper_right_to_lower_left),
        '\u{2572}' => plan.push_stroke(upper_left_to_lower_right),
        '\u{2573}' => {
            plan.push_stroke(upper_right_to_lower_left);
            plan.push_stroke(upper_left_to_lower_right);
        }
        _ => return None,
    }
    Some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: TerminalGlyphMetrics = TerminalGlyphMetrics {
        cell_width: 10.0,
        cell_height: 20.0,
        font_size: 14.0,
    };

    fn plan(character: char) -> Option<TerminalGlyphPlan> {
        terminal_glyph_plan(character, METRICS, TerminalGlyphNeighbors::default())
    }

    #[test]
    fn block_element_range_is_complete() {
        for codepoint in BLOCK_ELEMENTS_START..=BLOCK_ELEMENTS_END {
            let character = char::from_u32(codepoint).expect("block codepoint");
            assert_eq!(
                plan(character).map(|plan| plan.kind()),
                Some(TerminalGlyphRenderKind::BlockElement)
            );
        }
    }

    #[test]
    fn box_drawing_range_is_complete() {
        for codepoint in BOX_DRAWING_START..=BOX_DRAWING_END {
            let character = char::from_u32(codepoint).expect("box codepoint");
            assert!(plan(character).is_some(), "missing U+{codepoint:04X}");
        }
    }

    #[test]
    fn upper_half_block_is_normalized() {
        let geometry = plan('\u{2580}').expect("upper half block");
        assert_eq!(
            geometry.rects(),
            &[TerminalGlyphRect::new(0.0, 0.0, 1.0, 0.5, 1.0)]
        );
    }

    #[test]
    fn braille_requires_a_three_cell_run() {
        let row = ['\u{28FF}', '\u{28FF}', '\u{28FF}'];
        for index in 0..row.len() {
            let geometry = terminal_glyph_plan(
                row[index],
                METRICS,
                TerminalGlyphNeighbors::from_row(&row, index),
            )
            .expect("braille geometry");
            assert_eq!(geometry.kind(), TerminalGlyphRenderKind::Braille);
            assert_eq!(geometry.rects().len(), 8);
        }
        let short = ['\u{28FF}', '\u{28FF}'];
        assert!(
            terminal_glyph_plan(
                short[0],
                METRICS,
                TerminalGlyphNeighbors::from_row(&short, 0)
            )
            .is_none()
        );
    }

    #[test]
    fn sextants_use_outward_snapping() {
        let geometry = plan('\u{1FB00}').expect("sextant geometry");
        assert_eq!(geometry.kind(), TerminalGlyphRenderKind::Sextant);
        assert!(
            geometry
                .rects()
                .iter()
                .all(|rect| rect.snap == TerminalGlyphRectSnap::Outward)
        );
    }

    #[test]
    fn rounded_corners_and_diagonals_emit_strokes() {
        let rounded = plan('\u{256D}').expect("rounded corner");
        assert_eq!(rounded.kind(), TerminalGlyphRenderKind::RoundedCorner);
        assert_eq!(rounded.strokes()[0].points().len(), 6);
        let diagonal = plan('\u{2573}').expect("crossed diagonal");
        assert_eq!(diagonal.kind(), TerminalGlyphRenderKind::Diagonal);
        assert_eq!(diagonal.strokes().len(), 2);
    }

    #[test]
    fn straight_box_lines_reach_cell_edges() {
        let vertical = plan('\u{2551}').expect("double vertical");
        assert!(
            vertical
                .rects()
                .iter()
                .all(|rect| rect.top == 0.0 && rect.bottom == 1.0)
        );
        let horizontal = plan('\u{2550}').expect("double horizontal");
        assert!(
            horizontal
                .rects()
                .iter()
                .all(|rect| rect.left == 0.0 && rect.right == 1.0)
        );
    }

    /// Hosts round each rect edge to device pixels. Every edge must already sit
    /// on a whole pixel, or a 1px line centered on a half pixel can round to
    /// zero height and drop one stroke of a double line.
    #[test]
    fn box_lines_land_on_whole_pixels_and_survive_host_rounding() {
        let sizes = [
            (8.0, 18.0, 14.0),
            (9.0, 19.0, 14.0),
            (17.0, 37.0, 28.0),
            (16.0, 36.0, 28.0),
        ];
        for (cell_width, cell_height, font_size) in sizes {
            let metrics = TerminalGlyphMetrics {
                cell_width,
                cell_height,
                font_size,
            };
            for code in 0x2500..=0x257F_u32 {
                let character = char::from_u32(code).unwrap();
                let Some(plan) =
                    terminal_glyph_plan(character, metrics, TerminalGlyphNeighbors::default())
                else {
                    continue;
                };
                if plan.kind() != TerminalGlyphRenderKind::BoxDrawing {
                    continue;
                }
                for rect in plan.rects() {
                    let edges = [
                        rect.left * cell_width,
                        rect.right * cell_width,
                        rect.top * cell_height,
                        rect.bottom * cell_height,
                    ];
                    for edge in edges {
                        assert!(
                            (edge - edge.round()).abs() < 1e-3,
                            "{character:?} edge {edge} at {cell_width}x{cell_height}"
                        );
                    }
                    assert!(
                        edges[1].round() > edges[0].round() && edges[3].round() > edges[2].round(),
                        "{character:?} collapses at {cell_width}x{cell_height}"
                    );
                }
            }
        }
    }

    #[test]
    fn double_lines_keep_two_equal_strokes_with_a_gap() {
        let metrics = TerminalGlyphMetrics {
            cell_width: 8.0,
            cell_height: 18.0,
            font_size: 14.0,
        };
        let plan = terminal_glyph_plan('\u{2550}', metrics, TerminalGlyphNeighbors::default())
            .expect("double horizontal");
        let rows: Vec<_> = plan
            .rects()
            .iter()
            .map(|rect| ((rect.top * 18.0).round(), (rect.bottom * 18.0).round()))
            .collect();
        assert_eq!(rows, vec![(8.0, 9.0), (10.0, 11.0)]);
    }
}
