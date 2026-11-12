//! Screen positions and sizes. The values are the same as in `scripts/ui-lib.sh` and the other
//! scripts that make the images (REQUIREMENTS.md §3.1, §9, §10, §11, §15).
//!
//! The coordinates are in the units of the scripts (ImageMagick MVG). The renderer adds
//! `RasterMetrics::pixel_center_offset` to each coordinate (refer to `render.rs`).
//!
//! The values depend on the orientation of the tablet. In portrait (1620 x 2160) they are the values
//! of the images. In landscape (2160 x 1620) the board is at the center, the toolbar is at the right
//! edge and the panels touch the toolbar, as in portrait. The analytics pages have the same scale as in
//! portrait, across the full width; the manual has its own landscape images (REQUIREMENTS.md §3.3).

use std::cell::Cell as StdCell;
use std::sync::OnceLock;

use crate::game::Cell;
use crate::settings::{Color, LineGray, Section, Switch};

/// The tablet models that the app supports (REQUIREMENTS.md §1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    PaperPro,
    Move,
}

impl Model {
    pub const ALL: [Model; 2] = [Model::PaperPro, Model::Move];

    /// The model from the code name of the device tree (`/sys/devices/soc0/machine`): Chiappa is the
    /// Paper Pro Move. All other names (Ferrari is the Paper Pro) give the Paper Pro.
    pub fn detect(machine: &str) -> Model {
        if machine.to_ascii_lowercase().contains("chiappa") { Model::Move } else { Model::PaperPro }
    }

    /// The values of this model.
    pub const fn device(self) -> Device {
        match self {
            Model::PaperPro => Device::PAPER_PRO,
            Model::Move => Device::MOVE,
        }
    }

    /// The name of the model in the log of the app.
    pub const fn name(self) -> &'static str {
        match self {
            Model::PaperPro => "reMarkable Paper Pro",
            Model::Move => "reMarkable Paper Pro Move",
        }
    }

    /// The name of the model in the option `--device` of the tools and in `DEVICE` of
    /// `scripts/build.sh`.
    pub const fn option_name(self) -> &'static str {
        match self {
            Model::PaperPro => "pro",
            Model::Move => "move",
        }
    }

    /// The folder of the design images of this model, from the root of the repository: `images/`
    /// and the option name (`scripts/build.sh` with `DEVICE`, refer to `images/INDEX.md`).
    pub fn images(self) -> String {
        format!("images/{}", self.option_name())
    }

    /// The model with the name `name` of the option `--device`.
    pub fn from_option_name(name: &str) -> Option<Model> {
        Model::ALL.into_iter().find(|m| m.option_name() == name)
    }
}

/// The values of one tablet model: the physical screen, the board and the contents of its cells,
/// the toolbar slot and the icons, the result card, the meter and the slider, the panels of the
/// menu and the pages. Each value is written in full for each model, also when the two models have
/// the same value. All other sizes come from these values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    pub model: Model,
    /// The physical framebuffer, always portrait.
    pub physical_w: u32,
    pub physical_h: u32,
    /// The side of a board cell (px).
    pub cell: i32,
    /// The widths of the board lines, of the micro-cell lines and of the x of a refused tap
    /// (`LineWidths`).
    pub lines: LineWidths,
    /// The numbers, the pencil marks and the x of a refused tap in a cell, in pixels of the
    /// reference cell (`CellContents`).
    pub cell_contents: CellContents,
    /// The qtfb framebuffer type that the app asks for: RGB888, 3 bytes for each pixel and
    /// `3 * physical_w` bytes for each line. AppLoad makes the buffer from this type, not from the
    /// tablet (1 is FBFMT_RMPP_RGB888, 1620 x 2160).
    pub fb_type: u8,
    /// The screen pixels of one layout unit of the manual and the analytics pages (`page_scale`):
    /// about `physical_w / 1404`. The images of the Paper Pro use the rounded value 1.153846.
    pub page_scale: f32,
    /// The side of a toolbar slot (px). The icons have the same size on each model, at the center
    /// of the slot.
    pub tb_slot: i32,
    /// The stroke widths and the outer sizes of the icons (`IconMetrics`).
    pub icons: IconMetrics,
    /// The result card of a completed game (`ResultCard`).
    pub card: ResultCard,
    /// The difficulty slider with its labels (`SliderMetrics`).
    pub slider: SliderMetrics,
    /// The small difficulty meter with no labels, of the result card and the statistics
    /// (`MeterMetrics`).
    pub meter: MeterMetrics,
    /// The frame and the rows of the pop-up panels of the menu (`PanelMetrics`).
    pub panel: PanelMetrics,
    /// The Settings panel (`SettingsMetrics`).
    pub settings: SettingsMetrics,
    /// The on/off switch of the Settings panel and of the New game panel (`SwitchMetrics`).
    pub switch: SwitchMetrics,
    /// The Checkpoints panel (`CheckpointMetrics`).
    pub checkpoints: CheckpointMetrics,
    /// The Cell colors panel (`ColorsPanelMetrics`).
    pub colors_panel: ColorsPanelMetrics,
    /// The frame of the manual, the statistics and the Licenses pages (`PageMetrics`).
    pub page: PageMetrics,
    /// The dialog at the center of a page (`DialogMetrics`).
    pub dialog: DialogMetrics,
    /// The statistics page (`SummaryPageMetrics`).
    pub summary: SummaryPageMetrics,
    /// The list of the completed games (`GamesListMetrics`).
    pub games_list: GamesListMetrics,
    /// The Licenses list and the license text (`LicensesPageMetrics`).
    pub licenses: LicensesPageMetrics,
    /// The ink of the pen (`InkMetrics`).
    pub ink: InkMetrics,
    /// The calibrations of the renderer against the design images, and the margins of its drawings
    /// (`RasterMetrics`).
    pub raster: RasterMetrics,
}

/// The line widths of the board and of the drawings in a cell (REQUIREMENTS.md §4.1, §15), in
/// pixels of the device. They do not change with the cell size (owner request 2026-11-11, BRD-11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineWidths {
    /// The width of the border of the board and of the lines between the 3x3 boxes (BRD-11).
    pub box_line: i32,
    /// The width of the lines between the cells of a box (BRD-11).
    pub cell_line: i32,
    /// The width of a micro-cell line of an empty cell (BRD-6): the line starts at the pixel
    /// `micro_offset(k)` from the cell origin.
    pub micro_line: i32,
    /// The line width of the two lines of the x of a refused tap (MRK-13).
    pub refused_x_line: f32,
}

/// The contents of a board cell (REQUIREMENTS.md §4.1, §15): the numbers, the "?", the pencil marks
/// and the x of a refused tap. The values are in pixels of the reference cell of 147 px (`REF_CELL`),
/// from the top left of the cell. `Device::scale` changes each value to the cell of the device:
/// the methods of `Device` with the same names (`Device::digit_size` and the others) give the
/// pixels of the device.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellContents {
    /// The text size of the numbers and of the "?" (BRD-8).
    pub digit_size: i32,
    /// The baseline of the numbers and of the "?", from the top of the cell.
    pub digit_baseline: i32,
    /// The text size of the pencil marks (MRK-19).
    pub mark_size: i32,
    /// The center of the micro-cell of a pencil mark, on each axis from the top left of the cell,
    /// for the column (or the row) 0, 1 and 2 of the keypad (MRK-1, MRK-13).
    pub mark_pos: [i32; 3],
    /// The baseline of a pencil mark, below the center of its micro-cell.
    pub mark_baseline: i32,
    /// The x of a refused tap (MRK-13): each of its two lines goes this distance on the two axes
    /// from the center of the micro-cell.
    pub refused_x_half: i32,
}

/// The ink of the pen (REQUIREMENTS.md §5.1, §15, INP-25), in screen pixels. The ink is a crisp
/// drawing: each pixel is black or the pixel that was under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkMetrics {
    /// The width of the line of a stroke, with round ends and joins.
    pub width: f32,
    /// The radius of the dot of a tap (a stroke with one point).
    pub dot_radius: f32,
    /// While the pen moves, the screen cells near the pen tip wait one tick (`present.rs`): the
    /// cells nearer to the tip (rounded to full pixels) than `width` rounded up plus this margin.
    pub tip_margin: i32,
}

/// The calibrations and the margins of the renderer (`render.rs`, tiny-skia), in screen pixels.
/// They are not sizes of the screen design.
///
/// The calibrations exist because ImageMagick makes the design images (REQUIREMENTS.md §15,
/// `scripts/build.sh`), and it puts pixels, texts and strokes a little differently from tiny-skia.
/// The values give the smallest difference to the design images (the snapshot tool measures it).
/// The margins are the areas around a drawing where the renderer can change pixels: the frame
/// record (`render::FrameRecord`), the tile cache of the board and the drawing threads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterMetrics {
    /// The distance from the edge of a pixel to its center. ImageMagick puts a pixel center at an
    /// integer coordinate, and tiny-skia at this distance after it: each drawing gets this
    /// translation. A rectangle of ImageMagick covers this distance outside the centers of its
    /// edge pixels (`Canvas::rect`).
    pub pixel_center_offset: f32,
    /// The offset (x, y) of a text in screen units: ImageMagick puts text a little to the left of
    /// and higher than tiny-skia.
    pub text_offset: (f32, f32),
    /// The offset (x, y) of a text in page units (the manual and the analytics pages), in screen
    /// pixels.
    pub page_text_offset: (f32, f32),
    /// The extra width of each stroke: ImageMagick draws strokes a little wider than tiny-skia.
    pub stroke_extra: f32,
    /// The margin of the area that a drawing of a path records (`Canvas::touched_path`), out from
    /// the miter of its stroke.
    pub path_record_margin: f32,
    /// The margin of the one area that the Checkpoints panel records for all of its drawings
    /// (`draw_checkpoints_panel`): it contains the margin of a drawing at the edge of the panel.
    pub checkpoints_record_margin: f32,
    /// The width of the white band around the board cells that the board area of the tile cache
    /// contains (the band tiles of `board_slots`).
    pub board_margin: i32,
    /// The rows that a stripe of the board draws above and below its rows
    /// (`draw_board_stripes`): the clip at the edge of a pixmap changes the pixels near it.
    pub stripe_margin: i32,
    /// The tile of a cell draws only the part of each board line near the tile (`board_lines`
    /// with `near`): the part goes this distance past the tile on each side, thus the ends of the
    /// part do not change the pixels of the tile.
    pub line_near_margin: i32,
}

/// The stroke widths and the outer sizes of the icons (`render::Icon`, REQUIREMENTS.md §3.1, §9, §15,
/// scripts/toolbar.sh and `menu_icon` in scripts/ui-lib.sh). The values are in the units of the
/// canvas: screen pixels, or page units for the icons of the pages (`Icon::on_page`).
///
/// An outer size sets the size of a full icon or the width of its strokes: a stroke width, the
/// radius of the circle of a round icon (the circle that contains the other parts of the icon), the
/// text size of a glyph icon and the position of a badge icon from the icon that it marks. A path
/// point is a coordinate inside the shape of an icon: the ends of its lines, the points of its
/// curves, the radius of an arc that gives points, and the position and the size of a part in the
/// circle of a round icon (the dot of the menu icon, the inner ring of the gear, the mark of the
/// caution icon, the baseline of the "?"). The gear is round with teeth outside its ring: its outer
/// sizes are the ring and the tips of the teeth. The path points stay in
/// `Icon::draw` as the design of each icon (owner decision 2026-11-11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconMetrics {
    /// The line width of the strokes of each icon, other than the bin and the teeth of the gear. In
    /// screen pixels for the toolbar icons, and in page units for the icons of the pages.
    pub stroke: f32,
    /// The line width of the strokes of the bin of a delete button.
    pub bin_stroke: f32,
    /// The radius of the ring of the menu button icon (NAV-7), at the center of the line.
    pub menu_radius: f32,
    /// The radius of the ring of the close button icon, at the center of the line.
    pub close_radius: f32,
    /// The radius of the ring of the gear (the Settings button), at the center of the line. The
    /// teeth start on this ring.
    pub gear_radius: f32,
    /// The distance from the center of the gear to the tips of its teeth. The teeth go from the
    /// ring (`gear_radius`) to this distance.
    pub gear_tooth_tip: f32,
    /// The width of each of the eight teeth of the gear (a line with butt ends).
    pub gear_tooth_width: f32,
    /// The radius of the red circle of the caution icon (§9.2), at the center of its border.
    pub caution_radius: f32,
    /// The center of the caution icon on the gear of the menu (§9.2): this distance to the right of
    /// and above the center of the gear.
    pub caution_offset: f32,
    /// The radius of the ring of the help icon, at the center of the line.
    pub help_radius: f32,
    /// The text size of the "?" in the ring of the help icon.
    pub help_text_size: f32,
}

/// The frame and the rows of the pop-up panels of the menu (REQUIREMENTS.md §9, toolbar.sh `panel`):
/// a row has a name at the left and a switch, a value or an icon at the right. All values are in
/// pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelMetrics {
    /// The height of a row of the Settings panel and of the row "Save checkpoint" (owner decision
    /// 2026-11-10: the rows do not follow the toolbar slot of the Move).
    pub row: i32,
    /// The width of the border of a panel, of the divider between two rows and of the border of a
    /// button in a row.
    pub line: i32,
    /// The distance from each side of the panel to the start of the name of a row and to the end
    /// of its value or of its switch. The slider of the New game panel also has this distance from
    /// each side, and the section titles of the Cell colors panel start at it.
    pub inset: i32,
    /// The text size of the names and of the values of the rows, of the section titles and of the
    /// item names of the Cell colors panel.
    pub text_size: f32,
    /// The center line of a row (the switch and the icons), from the top of the row.
    pub center: i32,
    /// The baseline of a text of one line, below the center line of the row.
    pub baseline: i32,
    /// The baseline of the first line of a name of two lines, above the center line of the row.
    pub first_of_two: i32,
    /// The distance from one baseline of a name to the baseline of the next line.
    pub line_pitch: i32,
    /// The center x of the icon of a row, from the right of the panel.
    pub icon_inset: i32,
}

/// The Settings panel (REQUIREMENTS.md §9.1) and its update row (§9.2), in pixels. The rows use
/// `PanelMetrics`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsMetrics {
    /// The width of the panel. The width of the Cell colors panel does not apply to it (NAV-14).
    pub width: i32,
    /// The button "Update to v<version>" in the update row.
    pub update_button: UpdateButtonMetrics,
}

/// The update button of the Settings panel (§9.2, UPD-1), in pixels: a box with the border of
/// `PanelMetrics::line`, an icon and a text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpdateButtonMetrics {
    /// The distance from each side of the panel to the outer edge of the border.
    pub inset: i32,
    /// The first line of the border, below the top of the row.
    pub top: i32,
    /// The last line of the border, below the top of the row.
    pub bottom: i32,
    /// The center x of the icon, from the left of the button. The center y is the center line of
    /// the row.
    pub icon_x: i32,
    /// The start of the text, from the left of the button. The baseline is the baseline of a text
    /// of one line of the row.
    pub text_x: i32,
}

/// The on/off switch (render.rs `switch`, NAV-15, DIF-8): a track with round ends and a knob at
/// one end. The values are in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchMetrics {
    /// The length of the track, from the left end to the right end.
    pub length: f32,
    /// The radius of the round ends of the track (the half of its height). The center of the knob
    /// is at this distance from the end of the track.
    pub radius: f32,
    /// The distance from the start of a quarter circle of a round end to the control points of its
    /// Bezier curve (about 0.552 of `radius`).
    pub curve_control: f32,
    /// The width of the outline of the track.
    pub outline: f32,
    /// The radius of the knob.
    pub knob_radius: f32,
}

/// The Checkpoints panel (REQUIREMENTS.md §9.3, NAV-18, ui-lib.sh `CKP_*`), in pixels. The row "Save
/// checkpoint" has the height `PanelMetrics::row`, its plus and its text are on the center line and
/// on the baseline of `PanelMetrics`, and the lines are `PanelMetrics::line` wide. Each x is from
/// the left edge of the panel. Each y of the text with no checkpoint is from the top of the panel,
/// and each y of a checkpoint row is from the top of the row: the contents move with the panel in
/// the other layouts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckpointMetrics {
    /// The width of the panel.
    pub width: i32,
    /// The distance from the top of a checkpoint row to the top of the next row (the divider
    /// between the two rows is at the bottom of the first row).
    pub row: i32,
    /// The height of the panel with no checkpoint (it ends at y 864 on the Paper Pro).
    pub empty_height: i32,
    /// The text size of "Save checkpoint" and of "No checkpoints". The other texts of the panel
    /// have the size `PanelMetrics::text_size`.
    pub title_size: f32,
    /// The center x of the plus of the row "Save checkpoint".
    pub save_icon_x: i32,
    /// The start of the text "Save checkpoint".
    pub save_text_x: i32,
    /// The text of the panel with no checkpoint.
    pub empty: EmptyCheckpointText,
    /// The small board at the left of a checkpoint row.
    pub board: SmallBoardMetrics,
    /// The two lines of text of a checkpoint row: the clock and the quantity of filled cells.
    pub text: CheckpointText,
    /// The button "Restore" or "Current" of a checkpoint row (CKP-2).
    pub button: CheckpointButton,
    /// The bin of a checkpoint row.
    pub bin: CheckpointBin,
}

/// The text of the Checkpoints panel with no checkpoint: "No checkpoints" and a hint below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmptyCheckpointText {
    /// The center x of the two lines.
    pub center_x: i32,
    /// The baseline of "No checkpoints", from the top of the panel.
    pub title_baseline: i32,
    /// The baseline of the hint, from the top of the panel.
    pub hint_baseline: i32,
}

/// The small board of a checkpoint row (CKP-7, board-state.py `mini_board`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmallBoardMetrics {
    /// The left x of the first cell, from the left of the panel.
    pub x: i32,
    /// The top y of the first cell, from the top of the row.
    pub y: i32,
    /// The side of a cell, with its line.
    pub cell: i32,
    /// The distance from the top left of a cell to the dot of the mark 1.
    pub mark_inset: i32,
    /// The distance from one mark dot to the next dot in the same line or in the same column.
    pub mark_pitch: i32,
    /// The text size of a number.
    pub digit_size: f32,
    /// The baseline of a number, from the top of its cell.
    pub digit_baseline: i32,
}

/// The two lines of text of a checkpoint row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckpointText {
    /// The start of the two lines.
    pub x: i32,
    /// The baseline of the clock ("<time> played"), from the top of the row.
    pub clock_baseline: i32,
    /// The baseline of "<n> of 81 cells", from the top of the row.
    pub cells_baseline: i32,
}

/// The button "Restore" or "Current" of a checkpoint row: a box with the border of
/// `PanelMetrics::line`. The box is also the tap area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckpointButton {
    /// The first column of the box.
    pub left: i32,
    /// The last column of the box.
    pub right: i32,
    /// The first line of the box, from the top of the row.
    pub top: i32,
    /// The last line of the box, from the top of the row.
    pub bottom: i32,
    /// The center x of the label.
    pub label_x: i32,
    /// The baseline of the label, from the top of the row.
    pub label_baseline: i32,
}

/// The bin of a checkpoint row (§9.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckpointBin {
    /// The center x of the icon.
    pub x: i32,
    /// The center y of the icon, from the top of the row.
    pub y: i32,
    /// The tap area of the bin starts at this x and ends at the right edge of the panel.
    pub area_left: i32,
}

/// The Cell colors panel (REQUIREMENTS.md §10, NAV-17, COL-13, toolbar.sh `color_section`), in
/// pixels: columns of sections, and in each section a title and a grid of 3 x 3 items. The border,
/// the lines between the columns and the dividers between the sections are `PanelMetrics::line`
/// wide. The titles and the names have the size `PanelMetrics::text_size`, and a title starts at
/// `PanelMetrics::inset` from the left of its section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorsPanelMetrics {
    /// The width of a column of sections. Each section has the width of its column.
    pub section_width: i32,
    /// The distance from the top of a section to the top of the next section in the same column.
    pub section_height: i32,
    /// The baseline of the title of a section, from the top of the section.
    pub title_baseline: i32,
    /// The top of the grid of items, from the top of the section.
    pub grid_top: i32,
    /// The left of the grid of items, from the left of the section.
    pub grid_inset: i32,
    /// One item of the grid.
    pub item: ColorItemMetrics,
}

/// One item of the Cell colors panel (render.rs `color_item`): a swatch at the center of the width
/// of the item and its name below it. The values are in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorItemMetrics {
    /// The width of the item: its tap area and the black box of the selected item.
    pub width: i32,
    /// The height of the item, from the top of the item to the top of the item below it.
    pub height: i32,
    /// The center y of the swatch, from the top of the item.
    pub swatch_y: i32,
    /// The radius of the swatch, to the center of its ring.
    pub swatch_radius: f32,
    /// The width of the ring of the swatch.
    pub ring: f32,
    /// The radius of the white disc of a swatch of the Micro-cell lines section, inside its ring
    /// (COL-13): the pattern writes only the pixels inside it.
    pub pattern_radius: f32,
    /// The line through the swatch of a taken color (COL-5) goes from the lower left to the upper
    /// right. Its two ends are at this distance from the center of the swatch, in x and in y.
    pub strike_reach: f32,
    /// The width of the line through the swatch of a taken color.
    pub strike_width: f32,
    /// The baseline of the name, from the top of the item.
    pub name_baseline: i32,
}

/// The difficulty slider with its labels "Easy" and "Hard" (ui-lib.sh `labeled_slider`): the meter
/// below the board (REQUIREMENTS.md §3.1), the slider of the New game panel (§11) and the slider of
/// the result card (§8.3). All values are in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderMetrics {
    /// The width from the start of "Easy" to the end of "Hard".
    pub width: i32,
    /// The distance from the bottom of the board to the center line of the meter below the board.
    pub board_gap: i32,
    /// The text size of the labels "Easy" and "Hard".
    pub label_size: f32,
    /// The baseline of the labels, below the center line of the slider.
    pub label_baseline: i32,
    /// The gap from the end of "Easy" to the first stop, and from the last stop to the start of
    /// "Hard".
    pub label_gap: f32,
    /// The approximate text width of "Easy", for a caller that has no font (most tests).
    pub approx_easy_w: f32,
    /// The approximate text width of "Hard", for a caller that has no font (most tests).
    pub approx_hard_w: f32,
    /// The line, the stops and the knob of the slider.
    pub meter: MeterMetrics,
}

/// The line, the stops and the knob of a difficulty meter (render.rs `slider` and `meter`). The
/// values are in the units of the canvas: pixels, or layout units on the statistics pages (§13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterMetrics {
    /// The line width from the knob to the last stop.
    pub track: f32,
    /// The line width from the first stop to the knob.
    pub active_track: f32,
    /// The radius of a stop.
    pub stop_radius: f32,
    /// The radius of the knob.
    pub knob_radius: f32,
    /// The width of the outline of the stops and of the knob.
    pub outline: f32,
}

/// The result card of a completed game (REQUIREMENTS.md §8.3, CMP-7 to CMP-9): the normal card, or
/// the compact card of a small screen (owner decision 2026-11-10). The orientation selects the
/// position of the card (`card_layout`): below the board in portrait, a column with the height of
/// the board at the side opposite the toolbar in landscape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResultCard {
    /// Portrait: a vertical divider between the result and a column of level buttons. Landscape:
    /// a horizontal divider above a column of level buttons.
    Normal(NormalCard),
    /// The card of a screen that is too small for the normal card. Portrait: centered on the
    /// screen, with a horizontal divider above one row of level buttons. Landscape: a horizontal
    /// divider above a column of level buttons. No "New game" title.
    Compact(CompactCard),
}

/// The normal result card (`ResultCard::Normal`): one layout for each orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalCard {
    /// Portrait (`CardLayout::Below`).
    pub below: BelowCard,
    /// Landscape (`CardLayout::Column`), with the "New game" title.
    pub column: ColumnCard,
}

/// The compact result card (`ResultCard::Compact`): one layout for each orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactCard {
    /// Portrait (`CardLayout::CompactBelow`).
    pub below: CompactBelowCard,
    /// Landscape (`CardLayout::Column`), with no "New game" title.
    pub column: ColumnCard,
}

/// The normal card in portrait (`CardLayout::Below`), in pixels: the result at the left, a vertical
/// divider, then "New game" and a column of level buttons (ui-lib.sh `result_card portrait`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BelowCard {
    /// The distance from the bottom of the board to the top of the card. The card has the width of
    /// the board.
    pub gap: i32,
    /// The height of the card.
    pub height: i32,
    /// The x of the vertical divider, from the left of the card.
    pub divider_dx: i32,
    /// The level buttons at the right of the divider: the top is from the top of the card, and the
    /// inset is from the divider and from the right of the card. "New game" starts at the inset
    /// from the divider.
    pub buttons: ButtonColumn,
    /// The x of the start of the texts and of the slider, from the left of the card.
    pub text_x: i32,
    /// "Completed in": the baseline is from the top of the card.
    pub heading: CardText,
    /// The time: the baseline is from the top of the card.
    pub time: CardText,
    /// The center line of the difficulty slider, from the top of the card.
    pub slider_y: i32,
    /// The comparison in one line: the baseline is from the top of the card.
    pub comparison: CardText,
    /// The start of the divider line, below the top of the card.
    pub divider_top: i32,
    /// The end of the divider line, above the bottom of the card.
    pub divider_bottom: i32,
    /// "New game": the baseline is from the top of the card.
    pub title: CardText,
    /// The contents of a level button.
    pub button: LabelBesideMeter,
}

/// A card in landscape (`CardLayout::Column`), in pixels: a column with the height of the board at
/// the side opposite the toolbar. The result at the top, a horizontal divider, then "New game" (only
/// on the normal card) and a column of level buttons (ui-lib.sh `result_card left`, and
/// `compact_card left` for the compact card).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnCard {
    /// The position of the column at the side opposite the toolbar.
    pub place: ColumnPlace,
    /// The width of the column.
    pub width: i32,
    /// The y of the horizontal divider.
    pub divider: ColumnDivider,
    /// The level buttons below the divider: the top is from the divider, and the inset is from the
    /// two sides of the card.
    pub buttons: ButtonColumn,
    /// The x of the start of the texts and of the divider line, from the left of the card.
    pub text_x: i32,
    /// "Completed in": the baseline is from the top of the card.
    pub heading: CardText,
    /// The time: the baseline is from the top of the card. The size lets a time of one hour or more
    /// fit in the column.
    pub time: CardText,
    /// The meter of the mode, at the center of the column: the y is from the top of the card.
    pub meter: CardMeter,
    /// The comparison with the earlier results.
    pub comparison: ComparisonColumn,
    /// The end of the divider line, from the right of the card.
    pub divider_right: i32,
    /// "New game" at `text_x`: the baseline is from the divider. None for no title (the compact
    /// card).
    pub title: Option<CardText>,
    /// The contents of a level button.
    pub button: LabelAboveMeter,
}

/// The position of the column of `ColumnCard`, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnPlace {
    /// The distance from the screen edge to the column.
    FromScreenEdge(i32),
    /// The distance from the toolbar slot to the column. The column is one toolbar slot and this gap
    /// from the screen edge, thus it does not touch the close button.
    AfterToolbar(i32),
}

impl ColumnPlace {
    /// The distance from the screen edge to the column.
    fn edge(self) -> i32 {
        match self {
            ColumnPlace::FromScreenEdge(edge) => edge,
            ColumnPlace::AfterToolbar(gap) => tb_slot() + gap,
        }
    }
}

/// The y of the horizontal divider of `ColumnCard`, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnDivider {
    /// The distance above the bottom of the card.
    AboveBottom(i32),
    /// The distance below the top of the card.
    BelowTop(i32),
}

impl ColumnDivider {
    /// The y of the divider in the card `r`.
    fn y(self, r: Rect) -> i32 {
        match self {
            ColumnDivider::AboveBottom(dy) => r.y1 - dy,
            ColumnDivider::BelowTop(dy) => r.y0 + dy,
        }
    }
}

/// The compact card in portrait (`CardLayout::CompactBelow`), in pixels: the result at the top, a
/// horizontal divider, then the three level buttons in one row (ui-lib.sh `compact_card portrait`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactBelowCard {
    /// The distance from the toolbar slot to the card. The card is centered on the screen with this
    /// gap from the toolbar, thus the open menu does not cover it.
    pub toolbar_gap: i32,
    /// The distance from the bottom of the board to the top of the card.
    pub gap: i32,
    /// The height of the card.
    pub height: i32,
    /// The y of the horizontal divider, from the top of the card.
    pub divider_dy: i32,
    /// The row of level buttons below the divider.
    pub buttons: ButtonRow,
    /// The x of the start of the texts and of the slider, from the left of the card.
    pub text_x: i32,
    /// "Completed in": the baseline is from the top of the card.
    pub heading: CardText,
    /// The time: the baseline is from the top of the card.
    pub time: CardText,
    /// The center line of the difficulty slider, from the top of the card.
    pub slider_y: i32,
    /// The comparison in one line: the baseline is from the top of the card.
    pub comparison: CardText,
    /// The start of the divider line, from the left of the card.
    pub divider_left: i32,
    /// The end of the divider line, from the right of the card.
    pub divider_right: i32,
    /// The contents of a level button.
    pub button: LabelAboveMeter,
}

/// One line of text of the result card, in pixels. Each field that has this type tells the line
/// that the baseline is from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardText {
    /// The text size.
    pub size: f32,
    /// The distance from the line of the field to the baseline.
    pub baseline: i32,
}

/// A small difficulty meter with no labels (render.rs `meter`) at the center of its area, in
/// pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardMeter {
    /// The center line of the meter. Each field that has this type tells the line that it is from.
    pub y: i32,
    /// The distance between two stops.
    pub step: f32,
}

/// The comparison of a landscape card (CMP-8), in pixels: "Average at this level" and "Previous
/// best", each above its time, or the text of a first game in the position of the first label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComparisonColumn {
    /// A label: the baseline of the first label is from the top of the card.
    pub label: CardText,
    /// The time below a label: the baseline is from the baseline of the label.
    pub value: CardText,
    /// The distance from the baseline of the first label to the baseline of the second label.
    pub pitch: i32,
}

/// The contents of a level button of `BelowCard`, in pixels: the label at the left and the meter
/// of the target mode at the right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelBesideMeter {
    /// The x of the start of the label, from the left of the button.
    pub label_x: i32,
    /// The label: the baseline is from the vertical center of the button.
    pub label: CardText,
    /// The distance from the last stop of the meter to the right of the button. The center line of
    /// the meter is at the vertical center of the button.
    pub meter_right: i32,
    /// The distance between two stops of the meter.
    pub meter_step: f32,
}

/// The contents of a level button, in pixels: the label above the meter of the target mode, the
/// two at the center of the button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelAboveMeter {
    /// The label: the baseline is from the top of the button.
    pub label: CardText,
    /// The meter: the y is from the top of the button.
    pub meter: CardMeter,
}

/// The level buttons of the result card one below the other (CMP-9), in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonColumn {
    /// The y of the top of the first button, below the line that the card gives.
    pub top: i32,
    /// The distance from the top of one button to the top of the next button.
    pub pitch: i32,
    /// The height of a button.
    pub height: i32,
    /// The distance from each side of the area to the button.
    pub inset: i32,
}

impl ButtonColumn {
    /// Button `i` (from the top) in the area from `x0` to `x1`, with the first button `top` below `y0`.
    fn button(&self, i: i32, x0: i32, x1: i32, y0: i32) -> Rect {
        let y = y0 + self.top + i * self.pitch;
        Rect::new(x0 + self.inset, y, x1 - self.inset, y + self.height)
    }
}

/// The level buttons of the result card in one row with the same width (CMP-9), in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonRow {
    /// The y of the top of the buttons, below the divider.
    pub top: i32,
    /// The height of a button.
    pub height: i32,
    /// The distance from each side of the card to the nearest button.
    pub inset: i32,
    /// The distance between two buttons.
    pub gap: i32,
}

impl ButtonRow {
    /// Button `i` (from the left) in the area from `x0` to `x1`, `top` below `y0`.
    fn button(&self, i: i32, x0: i32, x1: i32, y0: i32) -> Rect {
        let n = LevelButton::ALL.len() as i32;
        let w = (x1 - x0 - 2 * self.inset - (n - 1) * self.gap) / n;
        let (x, y) = (x0 + self.inset + i * (w + self.gap), y0 + self.top);
        Rect::new(x, y, x + w, y + self.height)
    }
}

/// The frame of the manual, the statistics and the Licenses pages (REQUIREMENTS.md §12, §13,
/// §9.4; manual-lib.sh, analytics.py): the margins, the rows, the lines, "< Back", the title and the
/// page number with its arrows. The values are in layout units of the page (`page_scale`), not in
/// pixels, except `swipe`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMetrics {
    /// The distance from the left edge and from the right edge of the page to the content. The
    /// title starts at it.
    pub margin: f32,
    /// The height of a row of the lists, of a row of the "By difficulty" table in portrait, of a
    /// button of the statistics and of the button row of a dialog.
    pub row: f32,
    /// The text of a row of the tables, of the lists and of the buttons: the baseline is below the
    /// center line of the row.
    pub row_text: PageText,
    /// The width of the lines, the dividers and the frames of the pages and of the dialogs.
    pub line: f32,
    /// "< Back" at the top left of the page.
    pub back: BackButtonMetrics,
    /// The title of the page: the x is `margin`, and the baseline is from the top of the page.
    pub title: PageText,
    /// The page number at the bottom of the page, with the arrows ‹ and ›.
    pub page_number: PageNumberMetrics,
    /// The shortest horizontal finger movement that turns a page (INP-14), in screen pixels (not
    /// layout units).
    pub swipe: f32,
}

/// One line of text of a page, in layout units of the page. Each field that has this type tells
/// the line that the baseline is from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageText {
    /// The text size.
    pub size: f32,
    /// The distance from the line of the field to the baseline.
    pub baseline: f32,
}

/// "< Back" at the top left of a page (`back_button`), in layout units of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackButtonMetrics {
    /// The width of the tap area, from the left edge of the page.
    pub width: f32,
    /// The height of the tap area, from the top of the page.
    pub height: f32,
    /// The center x of the icon "<", from the left edge of the page.
    pub icon_x: f32,
    /// The center y of the icon "<", from the top of the page.
    pub icon_y: f32,
    /// The start of the text "Back", from the left edge of the page.
    pub text_x: f32,
    /// The text "Back": the baseline is from the top of the page.
    pub text: PageText,
}

/// The page number "n / total" at the bottom center of a page and the arrows ‹ and › at its two
/// sides (`page_number_pos`, `prev_arrow`, `next_arrow`), in layout units of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageNumberMetrics {
    /// The text of the page number: the baseline is above the bottom of the page.
    pub text: PageText,
    /// The center x of each arrow icon, from the center of the page number.
    pub arrow_dx: f32,
    /// The center y of the arrow icons, above the baseline of the page number.
    pub arrow_dy: f32,
    /// The side of the tap area of each arrow that is nearest to the page number, from its center.
    pub tap_inner: f32,
    /// The side of the tap area of each arrow that is farthest from the page number, from its
    /// center.
    pub tap_outer: f32,
    /// The height of the tap area of the arrows, above the bottom of the page.
    pub tap_height: f32,
}

/// A dialog at the center of a page (`dialog`): the delete dialogs of the statistics (§13) and the
/// dialog "Restart the interface?" (§9.2). The values are in layout units of the page. The two
/// buttons are in the last row, with the height and the text of `PageMetrics::row`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogMetrics {
    /// The width of the dialog. The dialog is at the horizontal center of the page.
    pub width: f32,
    /// The height of the dialog.
    pub height: f32,
    /// The distance from the vertical center of the page up to the top of the dialog.
    pub above_center: f32,
    /// The start of the title and of the lines, from the left of the dialog.
    pub text_x: f32,
    /// The title: the baseline is from the top of the dialog.
    pub title: PageText,
    /// The text size of the two lines below the title.
    pub body_size: f32,
    /// The baselines of the two lines below the title, from the top of the dialog.
    pub body_baselines: [f32; 2],
}

/// The statistics page (REQUIREMENTS.md §13.1, analytics.py): the two boxes at the top, the heading
/// "By difficulty", its table and the row of the buttons "All completed games" and "Delete all
/// games". The values are in layout units of the page. The content starts at `PageMetrics::margin`
/// and ends at `page_right`, the frames are `PageMetrics::line` wide, and the texts of the table
/// rows and of the buttons have the size and the baseline of `PageMetrics::row_text`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryPageMetrics {
    /// The boxes "Puzzles completed" and "Average time".
    pub boxes: SummaryBoxMetrics,
    /// The heading "By difficulty": the baseline is from the top of the page.
    pub heading: PageText,
    /// The "By difficulty" table.
    pub table: SummaryTableMetrics,
    /// The buttons "All completed games" and "Delete all games", in one row below the table.
    pub buttons: SummaryButtonMetrics,
}

/// The two boxes at the top of the statistics page (ANA-3). The two boxes have the same width
/// (`summary_box_w`) and fill the width of the content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryBoxMetrics {
    /// The top of the boxes, from the top of the page.
    pub top: f32,
    /// The bottom of the boxes, from the top of the page.
    pub bottom: f32,
    /// The distance between the two boxes.
    pub gap: f32,
    /// The start of the label and of the value, from the left of the box.
    pub text_x: f32,
    /// The label ("Puzzles completed", "Average time"): the baseline is from the top of the box.
    pub label: PageText,
    /// The value (the quantity, the average time): the baseline is from the top of the box.
    pub value: PageText,
}

/// The "By difficulty" table of the statistics page (ANA-4): one row for each mode, with the name
/// at the left, the difficulty meter, the column "Completed" and the column "Average time" that
/// ends at `page_right`. A line is above each row and below the last row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryTableMetrics {
    /// The column titles above the first line: the baseline is from the top of the page.
    pub header: PageText,
    /// The top of the first line, from the top of the page.
    pub top: f32,
    /// The height of a row in landscape, so that the rows and the buttons fit on the page. In
    /// portrait, a row has the height `PageMetrics::row`. A taller page adds its extra height to
    /// the rows (`summary_row`).
    pub landscape_row: f32,
    /// The center of the first stop of the meter, from the left edge of the page.
    pub meter_x: f32,
    /// The distance between two stops of the meter.
    pub meter_step: f32,
    /// The end of the texts of the column "Completed", to the left of `page_right`.
    pub completed_end_from_right: f32,
}

/// The row of the buttons "All completed games" and "Delete all games" below the table (§13.1).
/// The buttons have the height `PageMetrics::row`. "All completed games" starts at
/// `PageMetrics::margin`, and "Delete all games" ends at `page_right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryButtonMetrics {
    /// The distance from the top of the line below the last row of the table to the top of the
    /// buttons.
    pub below_table: f32,
    /// The distance between the two buttons.
    pub gap: f32,
    /// The start of the text "All completed games", from the left of its button.
    pub all_games_text_x: f32,
    /// The center of the icon › of "All completed games", to the left of the right of its button.
    pub all_games_icon_dx: f32,
    /// The width of "Delete all games".
    pub delete_all_width: f32,
    /// The center of the bin of "Delete all games", from the left of its button.
    pub delete_all_icon_x: f32,
    /// The start of the text "Delete all games", from the left of its button.
    pub delete_all_text_x: f32,
}

/// The list of the completed games (REQUIREMENTS.md §13.2, analytics.py): the row of the filter
/// segments, the column titles and one table across the full width with a line above each row and
/// below the last row. The values are in layout units of the page, except `row_box_margin`. The
/// segments and the table start at `PageMetrics::margin` and end at `page_right` (`list_column`).
/// A row has the height `PageMetrics::row`, the lines and the frames are `PageMetrics::line` wide,
/// and the texts of a row have the size and the baseline of `PageMetrics::row_text`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamesListMetrics {
    /// The filter segments above the table.
    pub filter: FilterSegmentMetrics,
    /// The column titles "Date", "Difficulty" and "Time" above the first line: the baseline is
    /// from the top of the page.
    pub header: PageText,
    /// The top of the first line, from the top of the page.
    pub top: f32,
    /// The quantity of rows on a page in portrait, on a page with the height of the Paper Pro page.
    /// A taller page adds the rows that fit in its extra height (`list_rows`).
    pub portrait_rows: usize,
    /// The quantity of rows on a page in landscape, on a page with the height of the Paper Pro
    /// page: fewer than in portrait, because the page is not as tall.
    pub landscape_rows: usize,
    /// The center of the first stop of the meter and the start of the column title "Difficulty",
    /// from the left of the list.
    pub meter_dx: f32,
    /// The distance between two stops of the meter of a row.
    pub meter_step: f32,
    /// The end of the time and of the column title "Time", to the left of the right of the list.
    pub time_end_from_right: f32,
    /// The center of the bin, to the left of the right of the list.
    pub bin_from_right: f32,
    /// The half width of the tap area of the bin (`bin_button`), at each side of the center of the
    /// bin. The tap area has the height of the row.
    pub bin_tap_half_width: f32,
    /// The margin of the cached box of a row (render.rs `put_box`), in screen pixels: the box goes
    /// this distance past the left and the right of the list, and starts this distance below the
    /// line above the row.
    pub row_box_margin: i32,
}

/// The row of the filter segments of the games list (§13.2): All, then one segment for each mode.
/// The segments have the same width (`seg_w`) and fill the width of the list. The values are in
/// layout units of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterSegmentMetrics {
    /// The top of the segments, from the top of the page.
    pub top: f32,
    /// The height of the segments. The meter of a mode segment is on the center line of the
    /// segment (render.rs `row_center`).
    pub height: f32,
    /// The text "All", at the horizontal center of its segment: the baseline is from the top of
    /// the segment.
    pub all: PageText,
    /// The distance between two stops of the meter of a mode segment: smaller than in the table,
    /// so that the meter fits in the segment.
    pub meter_step: f32,
}

/// The Licenses screens (REQUIREMENTS.md §9.4): the list of the entries and the license text of
/// one entry. The values are in layout units of the page. The content starts at
/// `PageMetrics::margin` and ends at `page_right` (`list_column`), and the lines are
/// `PageMetrics::line` wide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LicensesPageMetrics {
    /// The Licenses list.
    pub list: LicensesListMetrics,
    /// The license text of one entry.
    pub text: LicenseTextMetrics,
}

/// The Licenses list (§9.4): the text below the title, a line, then one row for each entry with a
/// line below it. A row has the height `PageMetrics::row`, and its tap area is the full width of
/// the page (`licenses_row_rect`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LicensesListMetrics {
    /// The text "The app contains these open source components.": the baseline is from the top of
    /// the page.
    pub intro: PageText,
    /// The top of the line above the first row, and of the first row, from the top of the page.
    pub top: f32,
    /// The quantity of rows on a page in portrait, on a page with the height of the Paper Pro page.
    /// A taller page adds the rows that fit in its extra height (`licenses_rows`).
    pub portrait_rows: usize,
    /// The quantity of rows on a page in landscape, on a page with the height of the Paper Pro
    /// page.
    pub landscape_rows: usize,
    /// The baseline of the name and of the version, below the center line of the row. Here the
    /// center line is the top of the row plus half the row, not rounded down (not `row_center`).
    pub text_below_center: f32,
    /// The text size of the name of the entry.
    pub name_size: f32,
    /// The text size of the version.
    pub version_size: f32,
    /// The end of the version, to the left of the right of the list.
    pub version_end_from_right: f32,
    /// The center of the icon ›, to the left of the right of the list. Its center y is the center
    /// line of the row (not rounded down).
    pub icon_from_right: f32,
}

/// The license text of one entry (§9.4): the line "Version <version> · <license>" below the
/// title, a line, then the lines of the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LicenseTextMetrics {
    /// The line "Version <version> · <license>": the baseline is from the top of the page.
    pub subtitle: PageText,
    /// The top of the line below the subtitle, from the top of the page.
    pub divider_top: f32,
    /// The baseline of the first line of text, from the top of the page.
    pub first_baseline: f32,
    /// The distance between the baselines of two lines of text (`license_line`).
    pub line_spacing: f32,
    /// The quantity of lines on a page in portrait, on a page with the height of the Paper Pro
    /// page. A taller page adds the lines that fit in its extra height (`license_lines`).
    pub portrait_lines: usize,
    /// The quantity of lines on a page in landscape, on a page with the height of the Paper Pro
    /// page.
    pub landscape_lines: usize,
    /// The text size of the name of a license file (`Line::Heading`).
    pub heading_size: f32,
    /// The text size of the lines of a license file (`Line::Body`).
    pub body_size: f32,
}

/// The cell of the reference design (the images of REQUIREMENTS.md, the Paper Pro). The contents of
/// a cell are given in pixels of this cell, and `Device::scale` changes them to the cell of a device.
const REF_CELL: i32 = 147;

impl Device {
    /// The reMarkable Paper Pro: 1620 x 2160 at 229 DPI.
    pub const PAPER_PRO: Device = Device {
        model: Model::PaperPro,
        physical_w: 1620,
        physical_h: 2160,
        cell: REF_CELL,
        lines: LineWidths { box_line: 7, cell_line: 3, micro_line: 2, refused_x_line: 3.0 },
        cell_contents: CellContents {
            digit_size: 104,
            digit_baseline: 111,
            mark_size: 31,
            mark_pos: [24, 74, 122],
            mark_baseline: 11,
            refused_x_half: 10,
        },
        fb_type: 1,
        page_scale: 1.153846,
        tb_slot: 112,
        icons: IconMetrics {
            stroke: 3.0,
            bin_stroke: 2.6,
            menu_radius: 18.0,
            close_radius: 17.5,
            gear_radius: 11.0,
            gear_tooth_tip: 17.0,
            gear_tooth_width: 6.0,
            caution_radius: 10.0,
            caution_offset: 16.0,
            help_radius: 19.0,
            help_text_size: 26.0,
        },
        card: ResultCard::Normal(NormalCard {
            below: BelowCard {
                gap: 49,
                height: 320,
                divider_dx: 640,
                buttons: ButtonColumn { top: 75, pitch: 80, height: 70, inset: 32 },
                text_x: 40,
                heading: CardText { size: 30.0, baseline: 66 },
                time: CardText { size: 96.0, baseline: 180 },
                slider_y: 232,
                comparison: CardText { size: 25.0, baseline: 293 },
                divider_top: 24,
                divider_bottom: 25,
                title: CardText { size: 28.0, baseline: 55 },
                button: LabelBesideMeter {
                    label_x: 28,
                    label: CardText { size: 30.0, baseline: 11 },
                    meter_right: 40,
                    meter_step: 30.0,
                },
            },
            column: ColumnCard {
                place: ColumnPlace::FromScreenEdge(48),
                width: 322,
                divider: ColumnDivider::AboveBottom(560),
                buttons: ButtonColumn { top: 90, pitch: 140, height: 120, inset: 20 },
                text_x: 28,
                heading: CardText { size: 30.0, baseline: 70 },
                time: CardText { size: 72.0, baseline: 185 },
                meter: CardMeter { y: 255, step: 40.0 },
                comparison: ComparisonColumn {
                    label: CardText { size: 25.0, baseline: 370 },
                    value: CardText { size: 40.0, baseline: 48 },
                    pitch: 110,
                },
                divider_right: 29,
                title: Some(CardText { size: 28.0, baseline: 60 }),
                button: LabelAboveMeter {
                    label: CardText { size: 32.0, baseline: 50 },
                    meter: CardMeter { y: 88, step: 36.0 },
                },
            },
        }),
        slider: SliderMetrics {
            width: 440,
            board_gap: 50,
            label_size: 25.0,
            label_baseline: 9,
            label_gap: 24.0,
            approx_easy_w: 50.0,
            approx_hard_w: 70.0,
            meter: MeterMetrics { track: 3.0, active_track: 5.0, stop_radius: 6.0, knob_radius: 13.0, outline: 3.0 },
        },
        meter: MeterMetrics { track: 2.5, active_track: 4.5, stop_radius: 5.0, knob_radius: 11.0, outline: 2.5 },
        panel: PanelMetrics {
            row: 112,
            line: 2,
            inset: 33,
            text_size: 25.0,
            center: 56,
            baseline: 9,
            first_of_two: 7,
            line_pitch: 32,
            icon_inset: 45,
        },
        settings: SettingsMetrics {
            width: 400,
            update_button: UpdateButtonMetrics { inset: 33, top: 22, bottom: 90, icon_x: 40, text_x: 72 },
        },
        switch: SwitchMetrics { length: 72.0, radius: 18.0, curve_control: 9.94, outline: 3.0, knob_radius: 14.0 },
        checkpoints: CheckpointMetrics {
            width: 600,
            row: 180,
            empty_height: 305,
            title_size: 28.0,
            save_icon_x: 58,
            save_text_x: 100,
            empty: EmptyCheckpointText { center_x: 299, title_baseline: 200, hint_baseline: 240 },
            board: SmallBoardMetrics { x: 18, y: 14, cell: 17, mark_inset: 3, mark_pitch: 5, digit_size: 14.0, digit_baseline: 14 },
            text: CheckpointText { x: 190, clock_baseline: 81, cells_baseline: 115 },
            button: CheckpointButton { left: 381, right: 521, top: 55, bottom: 125, label_x: 451, label_baseline: 99 },
            bin: CheckpointBin { x: 560, y: 90, area_left: 533 },
        },
        colors_panel: ColorsPanelMetrics {
            section_width: 400,
            section_height: 360,
            title_baseline: 46,
            grid_top: 64,
            grid_inset: 32,
            item: ColorItemMetrics {
                width: 112,
                height: 90,
                swatch_y: 30,
                swatch_radius: 19.0,
                ring: 2.0,
                pattern_radius: 17.5,
                strike_reach: 17.0,
                strike_width: 2.5,
                name_baseline: 76,
            },
        },
        page: PageMetrics {
            margin: 72.0,
            row: 97.0,
            row_text: PageText { size: 34.0, baseline: 12.0 },
            line: 1.7,
            back: BackButtonMetrics {
                width: 180.0,
                height: 100.0,
                icon_x: 43.0,
                icon_y: 48.0,
                text_x: 66.0,
                text: PageText { size: 22.0, baseline: 56.0 },
            },
            title: PageText { size: 84.0, baseline: 170.0 },
            page_number: PageNumberMetrics {
                text: PageText { size: 28.0, baseline: 37.0 },
                arrow_dx: 79.0,
                arrow_dy: 10.0,
                tap_inner: 42.0,
                tap_outer: 142.0,
                tap_height: 102.0,
            },
            swipe: 120.0,
        },
        dialog: DialogMetrics {
            width: 900.0,
            height: 350.0,
            above_center: 176.0,
            text_x: 40.0,
            title: PageText { size: 44.0, baseline: 75.0 },
            body_size: 34.0,
            body_baselines: [140.0, 190.0],
        },
        summary: SummaryPageMetrics {
            boxes: SummaryBoxMetrics {
                top: 250.0,
                bottom: 440.0,
                gap: 28.0,
                text_x: 30.0,
                label: PageText { size: 30.0, baseline: 55.0 },
                value: PageText { size: 84.0, baseline: 155.0 },
            },
            heading: PageText { size: 50.0, baseline: 550.0 },
            table: SummaryTableMetrics {
                header: PageText { size: 30.0, baseline: 615.0 },
                top: 640.0,
                landscape_row: 76.0,
                meter_x: 230.0,
                meter_step: 34.0,
                completed_end_from_right: 432.0,
            },
            buttons: SummaryButtonMetrics {
                below_table: 70.0,
                gap: 28.0,
                all_games_text_x: 30.0,
                all_games_icon_dx: 34.0,
                delete_all_width: 400.0,
                delete_all_icon_x: 50.0,
                delete_all_text_x: 90.0,
            },
        },
        games_list: GamesListMetrics {
            filter: FilterSegmentMetrics {
                top: 240.0,
                height: 80.0,
                all: PageText { size: 32.0, baseline: 52.0 },
                meter_step: 18.0,
            },
            header: PageText { size: 30.0, baseline: 395.0 },
            top: 420.0,
            portrait_rows: 12,
            landscape_rows: 6,
            meter_dx: 488.0,
            meter_step: 34.0,
            time_end_from_right: 222.0,
            bin_from_right: 42.0,
            bin_tap_half_width: 45.0,
            row_box_margin: 2,
        },
        licenses: LicensesPageMetrics {
            list: LicensesListMetrics {
                intro: PageText { size: 30.0, baseline: 270.0 },
                top: 300.0,
                portrait_rows: 14,
                landscape_rows: 9,
                text_below_center: 11.0,
                name_size: 32.0,
                version_size: 30.0,
                version_end_from_right: 60.0,
                icon_from_right: 15.0,
            },
            text: LicenseTextMetrics {
                subtitle: PageText { size: 30.0, baseline: 232.0 },
                divider_top: 260.0,
                first_baseline: 320.0,
                line_spacing: 36.0,
                portrait_lines: 40,
                landscape_lines: 27,
                heading_size: 28.0,
                body_size: 25.0,
            },
        },
        ink: InkMetrics { width: 5.0, dot_radius: 2.5, tip_margin: 2 },
        raster: RasterMetrics {
            pixel_center_offset: 0.5,
            text_offset: (-1.0, -1.0),
            page_text_offset: (-0.5, -0.5),
            stroke_extra: 0.25,
            path_record_margin: 2.0,
            checkpoints_record_margin: 3.0,
            board_margin: 8,
            stripe_margin: 8,
            line_near_margin: 16,
        },
    };

    /// The reMarkable Paper Pro Move: 954 x 1696 at 264 DPI (owner decisions 2026-11-10). The board
    /// has cells of 90 px in the two orientations. The page scale keeps the pages 1404 units wide.
    pub const MOVE: Device = Device {
        model: Model::Move,
        physical_w: 954,
        physical_h: 1696,
        cell: 78,
        lines: LineWidths { box_line: 7, cell_line: 3, micro_line: 2, refused_x_line: 3.0 },
        cell_contents: CellContents {
            digit_size: 104,
            digit_baseline: 111,
            mark_size: 31,
            mark_pos: [24, 74, 122],
            mark_baseline: 11,
            refused_x_half: 10,
        },
        fb_type: 4,
        page_scale: 0.679487,
        tb_slot: 96,
        icons: IconMetrics {
            stroke: 3.0,
            bin_stroke: 2.6,
            menu_radius: 18.0,
            close_radius: 17.5,
            gear_radius: 11.0,
            gear_tooth_tip: 17.0,
            gear_tooth_width: 6.0,
            caution_radius: 10.0,
            caution_offset: 16.0,
            help_radius: 19.0,
            help_text_size: 26.0,
        },
        card: ResultCard::Compact(CompactCard {
            below: CompactBelowCard {
                toolbar_gap: 24,
                gap: 49,
                height: 370,
                divider_dy: 250,
                buttons: ButtonRow { top: 18, height: 80, inset: 32, gap: 16 },
                text_x: 40,
                heading: CardText { size: 30.0, baseline: 50 },
                time: CardText { size: 72.0, baseline: 130 },
                slider_y: 175,
                comparison: CardText { size: 25.0, baseline: 225 },
                divider_left: 24,
                divider_right: 25,
                button: LabelAboveMeter {
                    label: CardText { size: 28.0, baseline: 34 },
                    meter: CardMeter { y: 58, step: 22.0 },
                },
            },
            column: ColumnCard {
                place: ColumnPlace::AfterToolbar(24),
                width: 300,
                divider: ColumnDivider::BelowTop(380),
                buttons: ButtonColumn { top: 20, pitch: 100, height: 88, inset: 20 },
                text_x: 28,
                heading: CardText { size: 30.0, baseline: 50 },
                time: CardText { size: 64.0, baseline: 128 },
                meter: CardMeter { y: 176, step: 36.0 },
                comparison: ComparisonColumn {
                    label: CardText { size: 25.0, baseline: 232 },
                    value: CardText { size: 36.0, baseline: 42 },
                    pitch: 80,
                },
                divider_right: 29,
                title: None,
                button: LabelAboveMeter {
                    label: CardText { size: 30.0, baseline: 38 },
                    meter: CardMeter { y: 66, step: 30.0 },
                },
            },
        }),
        slider: SliderMetrics {
            width: 440,
            board_gap: 50,
            label_size: 25.0,
            label_baseline: 9,
            label_gap: 24.0,
            approx_easy_w: 50.0,
            approx_hard_w: 70.0,
            meter: MeterMetrics { track: 3.0, active_track: 5.0, stop_radius: 6.0, knob_radius: 13.0, outline: 3.0 },
        },
        meter: MeterMetrics { track: 2.5, active_track: 4.5, stop_radius: 5.0, knob_radius: 11.0, outline: 2.5 },
        panel: PanelMetrics {
            row: 112,
            line: 2,
            inset: 33,
            text_size: 25.0,
            center: 56,
            baseline: 9,
            first_of_two: 7,
            line_pitch: 32,
            icon_inset: 45,
        },
        settings: SettingsMetrics {
            width: 400,
            update_button: UpdateButtonMetrics { inset: 33, top: 22, bottom: 90, icon_x: 40, text_x: 72 },
        },
        switch: SwitchMetrics { length: 72.0, radius: 18.0, curve_control: 9.94, outline: 3.0, knob_radius: 14.0 },
        checkpoints: CheckpointMetrics {
            width: 600,
            row: 180,
            empty_height: 305,
            title_size: 28.0,
            save_icon_x: 58,
            save_text_x: 100,
            empty: EmptyCheckpointText { center_x: 299, title_baseline: 200, hint_baseline: 240 },
            board: SmallBoardMetrics { x: 18, y: 14, cell: 17, mark_inset: 3, mark_pitch: 5, digit_size: 14.0, digit_baseline: 14 },
            text: CheckpointText { x: 190, clock_baseline: 81, cells_baseline: 115 },
            button: CheckpointButton { left: 381, right: 521, top: 55, bottom: 125, label_x: 451, label_baseline: 99 },
            bin: CheckpointBin { x: 560, y: 90, area_left: 533 },
        },
        colors_panel: ColorsPanelMetrics {
            section_width: 400,
            section_height: 360,
            title_baseline: 46,
            grid_top: 64,
            grid_inset: 32,
            item: ColorItemMetrics {
                width: 112,
                height: 90,
                swatch_y: 30,
                swatch_radius: 19.0,
                ring: 2.0,
                pattern_radius: 17.5,
                strike_reach: 17.0,
                strike_width: 2.5,
                name_baseline: 76,
            },
        },
        page: PageMetrics {
            margin: 72.0,
            row: 97.0,
            row_text: PageText { size: 34.0, baseline: 12.0 },
            line: 1.7,
            back: BackButtonMetrics {
                width: 180.0,
                height: 100.0,
                icon_x: 43.0,
                icon_y: 48.0,
                text_x: 66.0,
                text: PageText { size: 22.0, baseline: 56.0 },
            },
            title: PageText { size: 84.0, baseline: 170.0 },
            page_number: PageNumberMetrics {
                text: PageText { size: 28.0, baseline: 37.0 },
                arrow_dx: 79.0,
                arrow_dy: 10.0,
                tap_inner: 42.0,
                tap_outer: 142.0,
                tap_height: 102.0,
            },
            swipe: 120.0,
        },
        dialog: DialogMetrics {
            width: 900.0,
            height: 350.0,
            above_center: 176.0,
            text_x: 40.0,
            title: PageText { size: 44.0, baseline: 75.0 },
            body_size: 34.0,
            body_baselines: [140.0, 190.0],
        },
        summary: SummaryPageMetrics {
            boxes: SummaryBoxMetrics {
                top: 250.0,
                bottom: 440.0,
                gap: 28.0,
                text_x: 30.0,
                label: PageText { size: 30.0, baseline: 55.0 },
                value: PageText { size: 84.0, baseline: 155.0 },
            },
            heading: PageText { size: 50.0, baseline: 550.0 },
            table: SummaryTableMetrics {
                header: PageText { size: 30.0, baseline: 615.0 },
                top: 640.0,
                landscape_row: 76.0,
                meter_x: 230.0,
                meter_step: 34.0,
                completed_end_from_right: 432.0,
            },
            buttons: SummaryButtonMetrics {
                below_table: 70.0,
                gap: 28.0,
                all_games_text_x: 30.0,
                all_games_icon_dx: 34.0,
                delete_all_width: 400.0,
                delete_all_icon_x: 50.0,
                delete_all_text_x: 90.0,
            },
        },
        games_list: GamesListMetrics {
            filter: FilterSegmentMetrics {
                top: 240.0,
                height: 80.0,
                all: PageText { size: 32.0, baseline: 52.0 },
                meter_step: 18.0,
            },
            header: PageText { size: 30.0, baseline: 395.0 },
            top: 420.0,
            portrait_rows: 12,
            landscape_rows: 6,
            meter_dx: 488.0,
            meter_step: 34.0,
            time_end_from_right: 222.0,
            bin_from_right: 42.0,
            bin_tap_half_width: 45.0,
            row_box_margin: 2,
        },
        licenses: LicensesPageMetrics {
            list: LicensesListMetrics {
                intro: PageText { size: 30.0, baseline: 270.0 },
                top: 300.0,
                portrait_rows: 14,
                landscape_rows: 9,
                text_below_center: 11.0,
                name_size: 32.0,
                version_size: 30.0,
                version_end_from_right: 60.0,
                icon_from_right: 15.0,
            },
            text: LicenseTextMetrics {
                subtitle: PageText { size: 30.0, baseline: 232.0 },
                divider_top: 260.0,
                first_baseline: 320.0,
                line_spacing: 36.0,
                portrait_lines: 40,
                landscape_lines: 27,
                heading_size: 28.0,
                body_size: 25.0,
            },
        },
        // The ink is not scaled for the 264 DPI of the Move: the owner decides it (an open question).
        ink: InkMetrics { width: 5.0, dot_radius: 2.5, tip_margin: 2 },
        raster: RasterMetrics {
            pixel_center_offset: 0.5,
            text_offset: (-1.0, -1.0),
            page_text_offset: (-0.5, -0.5),
            stroke_extra: 0.25,
            path_record_margin: 2.0,
            checkpoints_record_margin: 3.0,
            board_margin: 8,
            stripe_margin: 8,
            line_near_margin: 16,
        },
    };

    /// A length `v` of the reference cell, in pixels of the cell of this device (rounded to full
    /// pixels, thus the contents of a cell stay crisp). On the Paper Pro, it is `v`.
    pub const fn scale(&self, v: i32) -> i32 {
        (2 * v * self.cell + REF_CELL) / (2 * REF_CELL)
    }

    /// The text size of the numbers and of the "?" (px).
    pub const fn digit_size(&self) -> f32 {
        self.scale(self.cell_contents.digit_size) as f32
    }

    /// The baseline of the numbers and of the "?", from the top of the cell.
    pub const fn digit_baseline(&self) -> f32 {
        self.scale(self.cell_contents.digit_baseline) as f32
    }

    /// The text size of the pencil marks (px, §15).
    pub const fn mark_size(&self) -> f32 {
        self.scale(self.cell_contents.mark_size) as f32
    }

    /// The center of the micro-cell of a pencil mark, on each axis from the top left of the cell, for
    /// the column (or the row) 0, 1 and 2 of the keypad (MRK-1).
    pub const fn mark_pos(&self, i: usize) -> f32 {
        self.scale(self.cell_contents.mark_pos[i]) as f32
    }

    /// The baseline of a pencil mark below the center of its micro-cell.
    pub const fn mark_baseline(&self) -> f32 {
        self.scale(self.cell_contents.mark_baseline) as f32
    }

    /// The x of a refused tap (MRK-13): each line goes this distance on the two axes from the center.
    pub const fn refused_x_half(&self) -> f32 {
        self.scale(self.cell_contents.refused_x_half) as f32
    }

    /// The width of board line k (0..=9, from the left or the top): the border (0, 9) and the box
    /// lines (3, 6) are `LineWidths::box_line`, the other lines `LineWidths::cell_line`.
    pub fn line_width(&self, k: i32) -> i32 {
        if k % 3 == 0 { self.lines.box_line } else { self.lines.cell_line }
    }

    /// The first pixel of board line k (0..=9) from the edge of the board. Each line has its own
    /// pixels between the cells: no line covers a pixel of a cell (owner request 2026-11-11).
    pub fn line_offset(&self, k: i32) -> i32 {
        let box_lines = (k + 2) / 3; // the lines 0, 3 and 6 before line k
        k * self.cell + box_lines * self.lines.box_line + (k - box_lines) * self.lines.cell_line
    }

    /// The first pixel of the cells of column (or row) i from the edge of the board: after line i.
    pub fn cell_offset(&self, i: i32) -> i32 {
        self.line_offset(i) + self.line_width(i)
    }

    /// The side of the board (px): 9 cells, 4 box lines and 6 cell lines (1369 px on the Paper Pro).
    pub fn board_size(&self) -> i32 {
        self.line_offset(9) + self.lines.box_line
    }
}

static DEVICE: OnceLock<Device> = OnceLock::new();

/// The tablet that the app runs on. Until the platform sets it (the tests, the tools), it is the
/// Paper Pro.
pub fn device() -> &'static Device {
    DEVICE.get_or_init(|| Device::PAPER_PRO)
}

/// Sets the tablet one time, before the first drawing. It is a value of the process, not of a
/// thread: a thread that draws gets the same device. Returns false if the device was already set to
/// a different value.
pub fn set_device(d: Device) -> bool {
    *DEVICE.get_or_init(|| d) == d
}

/// The side of a board cell (px).
pub fn cell_size() -> i32 {
    device().cell
}

/// The side of the board (px) on the device (`Device::board_size`).
pub fn board_size() -> i32 {
    device().board_size()
}

/// The column (or row) of the cell at the distance p (px) from the edge of the board, 0 <= p <
/// board_size. A pixel of a line goes to the cell nearest to it: the first half of the line to the
/// cell before it, the other half to the cell after it. The border goes to the cell next to it.
fn cell_index(p: f32) -> i32 {
    let d = device();
    (1..9).rev().find(|&i| p >= (d.line_offset(i) + d.line_width(i) / 2) as f32).unwrap_or(0)
}

/// The physical framebuffer (portrait).
pub fn physical_w() -> u32 {
    device().physical_w
}

pub fn physical_h() -> u32 {
    device().physical_h
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// The rotation of the tablet, as qtfb gives it: 0, 1 (90° left), 2 (90° right) or 3 (180°).
pub type Rotation = i32;

/// The landscape rotation in which the top of the screen is at the physical right edge (the bottom
/// of the app is at x = 1619 of the framebuffer). The other landscape rotation is the opposite. On the
/// Paper Pro, AppLoad sends 2 for the orientation "InvertedLandscape" of xochitl, and this mapping is
/// correct for it (examined on the device).
const ROTATION_TOP_RIGHT: Rotation = 2;

thread_local! {
    static ROTATION: StdCell<Rotation> = const { StdCell::new(0) };
    static RIGHT_HANDED: StdCell<bool> = const { StdCell::new(false) };
}

/// The handedness of the player (REQUIREMENTS.md §3.1). For a left-handed player (the images), the
/// toolbar is at the right edge and the close button is at the top left. For a right-handed player,
/// the game screen shows them as a mirror image: the toolbar is at the left edge and the close button
/// is at the top right. The platform reads the handedness of the device.
pub fn right_handed() -> bool {
    RIGHT_HANDED.with(|r| r.get())
}

pub fn set_right_handed(on: bool) {
    RIGHT_HANDED.with(|r| r.set(on));
}

/// The x of the mirror image of x (a pixel center), if the player is right-handed.
fn mirror_x(x: f32) -> f32 {
    if right_handed() { screen_w() as f32 - 1.0 - x } else { x }
}

/// The orientation of the layout. The platform sets the rotation when the tablet turns.
pub fn orientation() -> Orientation {
    match rotation() {
        1 | 2 => Orientation::Landscape,
        _ => Orientation::Portrait,
    }
}

pub fn rotation() -> Rotation {
    ROTATION.with(|r| r.get())
}

pub fn set_rotation(r: Rotation) {
    ROTATION.with(|c| c.set(r.clamp(0, 3)));
}

pub fn set_orientation(o: Orientation) {
    set_rotation(if o == Orientation::Landscape { ROTATION_TOP_RIGHT } else { 0 });
}

/// The pixel index in the physical framebuffer (portrait) of the screen point (x, y):
/// `base + x * sx + y * sy`.
pub fn physical_steps() -> (i64, i64, i64) {
    let (w, h) = (physical_w() as i64, physical_h() as i64);
    match rotation() {
        3 => ((h - 1) * w + (w - 1), -1, -w),
        r if r == ROTATION_TOP_RIGHT => ((h - 1) * w, -w, 1),
        1 | 2 => (w - 1, w, -1),
        _ => (0, 1, w),
    }
}

/// The physical point of a screen point.
fn to_physical(x: i32, y: i32) -> (i32, i32) {
    let (base, sx, sy) = physical_steps();
    let i = base + x as i64 * sx + y as i64 * sy;
    ((i % physical_w() as i64) as i32, (i / physical_w() as i64) as i32)
}

/// The screen point of a physical point (the input of qtfb is in physical pixels).
pub fn to_screen(px: f32, py: f32) -> (f32, f32) {
    let (w, h) = (physical_w() as f32, physical_h() as f32);
    match rotation() {
        3 => (w - 1.0 - px, h - 1.0 - py),
        r if r == ROTATION_TOP_RIGHT => (h - 1.0 - py, px),
        1 | 2 => (py, w - 1.0 - px),
        _ => (px, py),
    }
}

/// The physical rectangle (x, y, width, height) of a screen rectangle (x0, y0, x1, y1; all included).
pub fn physical_rect((x0, y0, x1, y1): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (a, b) = (to_physical(x0, y0), to_physical(x1, y1));
    let (px0, px1) = (a.0.min(b.0), a.0.max(b.0));
    let (py0, py1) = (a.1.min(b.1), a.1.max(b.1));
    (px0, py0, px1 - px0 + 1, py1 - py0 + 1)
}

/// The screen rectangle (x, y, width, height) of a physical rectangle (x, y, width, height). It is
/// the inverse of `physical_rect`.
pub fn screen_rect((x, y, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (ax, ay) = to_screen(x as f32, y as f32);
    let (bx, by) = to_screen((x + w - 1) as f32, (y + h - 1) as f32);
    let (x0, x1) = (ax.min(bx) as i32, ax.max(bx) as i32);
    let (y0, y1) = (ay.min(by) as i32, ay.max(by) as i32);
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

fn landscape() -> bool {
    orientation() == Orientation::Landscape
}

/// The width and the height of the screen as the player sees it.
pub fn screen_w() -> u32 {
    if landscape() { physical_h() } else { physical_w() }
}

pub fn screen_h() -> u32 {
    if landscape() { physical_w() } else { physical_h() }
}

/// The top left of the board (at the center of the screen): (125, 395) in portrait on the Paper Pro,
/// (395, 125) in landscape.
pub fn board_x() -> i32 {
    (screen_w() as i32 - board_size()) / 2
}

pub fn board_y() -> i32 {
    (screen_h() as i32 - board_size()) / 2
}

/// The offset of micro-cell line k (0..=3) in a cell: round(k * cell / 3) (cell 147 px on the Paper
/// Pro).
pub fn micro_offset(k: i32) -> i32 {
    (k * cell_size() + 1) / 3
}

/// The board cell at a screen point, if the point is on the board.
pub fn board_cell(x: f32, y: f32) -> Option<Cell> {
    let (bx, by) = (x - board_x() as f32, y - board_y() as f32);
    if bx < 0.0 || by < 0.0 || bx >= board_size() as f32 || by >= board_size() as f32 {
        return None;
    }
    Some((cell_index(by) as u8, cell_index(bx) as u8))
}

/// The cell and the micro-cell digit (1..=9, keypad positions) at a screen point.
pub fn board_micro(x: f32, y: f32) -> Option<(Cell, u8)> {
    let cell = board_cell(x, y)?;
    let (ox, oy) = cell_origin(cell);
    let ix = (x as i32 - ox) * 3 / cell_size();
    let iy = (y as i32 - oy) * 3 / cell_size();
    Some((cell, (iy.clamp(0, 2) * 3 + ix.clamp(0, 2) + 1) as u8))
}

pub fn cell_origin((r, c): Cell) -> (i32, i32) {
    let d = device();
    (board_x() + d.cell_offset(c as i32), board_y() + d.cell_offset(r as i32))
}

/// The difficulty meter below the board (render.sh): "Easy" at `meter_xl`, "Hard" ends at `meter_xr`.
pub const STOPS: i32 = crate::generator::MODES as i32;

pub fn meter_y() -> i32 {
    board_y() + board_size() + device().slider.board_gap // 1814 in portrait
}

pub fn meter_xl() -> i32 {
    board_x() + board_size() / 2 - device().slider.width / 2 // 589 in portrait
}

pub fn meter_xr() -> i32 {
    meter_xl() + device().slider.width
}

/// The three layouts of the result card: the device selects the normal or the compact card
/// (`Device::card`), and the orientation selects the position of the card. In landscape the two
/// cards are the same column (`Column`, with the `ColumnCard` of the device). Each layout has only
/// the sizes of its own card and orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CardLayout {
    /// Portrait: below the board, with a vertical divider between the result and a column of
    /// level buttons.
    Below(BelowCard),
    /// Landscape (the normal card and the compact card): a column at the side opposite the toolbar,
    /// with a horizontal divider above the level buttons.
    Column(ColumnCard),
    /// Compact portrait: below the board and centered on the screen, with a horizontal divider
    /// above one row of level buttons.
    CompactBelow(CompactBelowCard),
}

/// The layout of the result card for the device and the orientation.
pub fn card_layout() -> CardLayout {
    match (device().card, landscape()) {
        (ResultCard::Normal(n), false) => CardLayout::Below(n.below),
        (ResultCard::Normal(n), true) => CardLayout::Column(n.column),
        (ResultCard::Compact(k), false) => CardLayout::CompactBelow(k.below),
        (ResultCard::Compact(k), true) => CardLayout::Column(k.column),
    }
}

/// The frame of the result card of a complete puzzle (REQUIREMENTS.md §8.3, CMP-7), the same for
/// the two hands in portrait.
pub fn result_card() -> Rect {
    let (w, board_bottom) = (screen_w() as i32, board_y() + board_size());
    match card_layout() {
        CardLayout::Below(card) => {
            let y0 = board_bottom + card.gap;
            Rect::new(board_x(), y0, board_x() + board_size(), y0 + card.height)
        }
        CardLayout::Column(card) => {
            let edge = card.place.edge();
            let x0 = if right_handed() { w - edge - card.width } else { edge };
            Rect::new(x0, board_y(), x0 + card.width, board_bottom)
        }
        CardLayout::CompactBelow(card) => {
            let half = w / 2 - tb_slot() - card.toolbar_gap;
            let (cx, y0) = (w / 2, board_bottom + card.gap);
            Rect::new(cx - half, y0, cx + half, y0 + card.height)
        }
    }
}

/// The center line of the 2 px divider of the card: the x for `Below` (between the result and the
/// level buttons), else the y (the divider is horizontal, above the level buttons).
pub fn card_divider() -> i32 {
    let r = result_card();
    match card_layout() {
        CardLayout::Below(card) => r.x0 + card.divider_dx,
        CardLayout::Column(card) => card.divider.y(r),
        CardLayout::CompactBelow(card) => r.y0 + card.divider_dy,
    }
}

/// A level button of the result card (§8.3, CMP-9), in the order from the top. The discriminant is
/// the position of the button in the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LevelButton {
    Easier = 0,
    Same = 1,
    Harder = 2,
}

impl LevelButton {
    pub const ALL: [LevelButton; 3] = [LevelButton::Easier, LevelButton::Same, LevelButton::Harder];

    pub fn label(self) -> &'static str {
        match self {
            LevelButton::Easier => "Easier",
            LevelButton::Same => "Same",
            LevelButton::Harder => "Harder",
        }
    }

    /// target(b): the mode of the new game after a game of `mode`. None if the button is not
    /// available (Easier after Easy, Harder after Hard). Thus a level button never gives a mode
    /// outside 0..MODES (CMP-10).
    pub fn target(self, mode: u8) -> Option<u8> {
        let m = match self {
            LevelButton::Easier => mode.checked_sub(1)?,
            LevelButton::Same => mode,
            LevelButton::Harder => mode + 1,
        };
        (m < crate::generator::MODES).then_some(m)
    }

    /// The position of the button in the card: 0 at the top (or at the left).
    fn index(self) -> i32 {
        self as i32
    }
}

// The order of `LevelButton::ALL` is the order of the discriminants, thus `index` and the loops
// over `ALL` agree (checked at compile time).
const _: () = {
    let mut i = 0;
    while i < LevelButton::ALL.len() {
        assert!(LevelButton::ALL[i] as usize == i);
        i += 1;
    }
};

/// The rectangle of a level button.
pub fn level_button(b: LevelButton) -> Rect {
    let (r, i) = (result_card(), b.index());
    match card_layout() {
        CardLayout::Below(card) => card.buttons.button(i, card_divider(), r.x1, r.y0),
        CardLayout::Column(card) => card.buttons.button(i, r.x0, r.x1, card_divider()),
        CardLayout::CompactBelow(card) => card.buttons.button(i, r.x0, r.x1, card_divider()),
    }
}

/// The level button at a point (available or not).
pub fn level_button_at(x: f32, y: f32) -> Option<LevelButton> {
    LevelButton::ALL.into_iter().find(|&b| level_button(b).contains(x, y))
}

/// The toolbar (the open menu) at the right edge: x 1508 and the bottom slot at y 2048 in portrait.
/// For a right-handed player it is at the left edge (x 0).
pub fn tb_slot() -> i32 {
    device().tb_slot
}

/// The center of an icon in its slot, from the top left of the slot: the middle of the pixel
/// centers of the slot (ImageMagick puts the center of the first pixel at 0). For example 55.5 px
/// in a slot of 112 px (the Paper Pro). ui-lib.sh `TB_C` gives the same value for the even slots
/// of all models.
fn slot_center() -> f32 {
    (tb_slot() - 1) as f32 / 2.0
}

pub fn tb_x() -> i32 {
    if right_handed() { 0 } else { screen_w() as i32 - tb_slot() }
}

/// The x of the toolbar edge that the panels touch.
fn tb_inner_x() -> i32 {
    if right_handed() { tb_slot() } else { tb_x() }
}

pub fn tb_cx() -> f32 {
    tb_x() as f32 + slot_center()
}

pub fn tb_bottom_y() -> i32 {
    screen_h() as i32 - tb_slot()
}

/// The center y of top slot k.
pub fn tb_cy(k: i32) -> f32 {
    slot_center() + (k * tb_slot()) as f32
}

/// The close button (top left, or top right for a right-handed player). Hit area: one toolbar slot.
pub fn close_cx() -> f32 {
    mirror_x(slot_center())
}

pub fn close_cy() -> f32 {
    slot_center()
}

/// The buttons of the game screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Menu,
    Colors,
    Settings,
    Undo,
    Redo,
    /// The bookmark: it opens the Checkpoints panel (§9.3).
    Checkpoints,
    Help,
    Statistics,
    NewGame,
    Close,
}

impl Button {
    /// The buttons of the top group of the toolbar, in the order of their slots from the top (0 is
    /// the menu button).
    pub const TOP: [Button; 6] = [Button::Menu, Button::Colors, Button::Settings, Button::Undo, Button::Redo, Button::Checkpoints];

    /// The buttons of the bottom group of the toolbar, in the order of their slots from the bottom
    /// (0 is the + of the new game).
    pub const BOTTOM: [Button; 3] = [Button::NewGame, Button::Statistics, Button::Help];
}

/// The position of a button in a group of the toolbar.
fn index_in(group: &[Button], b: Button) -> Option<i32> {
    group.iter().position(|&g| g == b).map(|k| k as i32)
}

/// The top y of the toolbar slot of a button. A panel starts at the top of the slot of the button
/// that opens it. The close button is not in the toolbar.
pub fn slot_top(b: Button) -> i32 {
    if let Some(k) = index_in(&Button::TOP, b) {
        k * tb_slot()
    } else if let Some(j) = index_in(&Button::BOTTOM, b) {
        tb_bottom_y() - j * tb_slot()
    } else {
        panic!("the button {b:?} is not in the toolbar")
    }
}

/// The center of the icon of a button. In the top group, the icon is 55.5 px below the top of its
/// slot (the same as the close button); in the bottom group, 56 px (on the Paper Pro).
pub fn icon_center(b: Button) -> (f32, f32) {
    if b == Button::Close {
        (close_cx(), close_cy())
    } else if let Some(k) = index_in(&Button::TOP, b) {
        (tb_cx(), tb_cy(k))
    } else {
        (tb_cx(), (slot_top(b) + tb_slot() / 2) as f32)
    }
}

/// The game-screen button at a point. Only the menu button and the close button show when the menu
/// is closed. The close button shows only when the menu is open.
pub fn game_button(x: f32, y: f32, menu_open: bool) -> Option<Button> {
    let (x, y) = (mirror_x(x) as i32, y as i32);
    let slot = tb_slot();
    if x < slot && y < slot {
        return menu_open.then_some(Button::Close);
    }
    if x < screen_w() as i32 - slot {
        return None;
    }
    let top = y / slot;
    if top == 0 {
        return Some(Button::Menu);
    }
    if !menu_open {
        return None;
    }
    if let Some(&b) = usize::try_from(top).ok().and_then(|k| Button::TOP.get(k)) {
        return Some(b);
    }
    // The bottom group: the first slot from the bottom that starts at or above y.
    Button::BOTTOM.into_iter().find(|&b| y >= slot_top(b))
}

/// Rectangle: x0, y0, x1, y1 (x1 and y1 not included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Rect {
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 as f32 && x < self.x1 as f32 && y >= self.y0 as f32 && y < self.y1 as f32
    }

    /// True if the two rectangles have a pixel in common.
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
}

/// The Cell colors panel (toolbar.sh, NAV-17): sections of `ColorsPanelMetrics` from the top of the
/// slot of the paint bucket (y 112 on the Paper Pro). Two columns of three rows, or, when three rows
/// do not fit on the screen (the Move in landscape), three columns of two rows. The sections go from
/// the top of the left column to the bottom of that column, then from the top of the next column:
/// the four color sections (`Section`), then the Micro-cell lines section (`LineGray`). The space
/// after that section is empty. On the Paper Pro the panel ends at y 1192 in the two orientations.
pub fn colors_top() -> i32 {
    slot_top(Button::Colors)
}

/// The quantity of section places: the five sections and one empty place.
const COLOR_PLACES: i32 = 6;

/// The quantity of section rows in each column: 3, or 2 when 3 rows do not fit on the screen.
fn color_rows() -> i32 {
    if colors_top() + 3 * device().colors_panel.section_height < screen_h() as i32 { 3 } else { 2 }
}

/// The quantity of columns of the panel: 2 or 3.
pub fn color_columns() -> i32 {
    COLOR_PLACES / color_rows()
}

/// The last row of the panel (its bottom line).
pub fn colors_bottom() -> i32 {
    colors_top() + color_rows() * device().colors_panel.section_height
}
/// The index of the Micro-cell lines section, after the color sections.
pub const LINES_SECTION: usize = Section::ALL.len();

/// The left and the right x of a panel of width w (pixels pl..=pr) that touches the toolbar.
fn panel_x(w: i32) -> (i32, i32) {
    if right_handed() { (tb_inner_x(), tb_inner_x() + w - 1) } else { (tb_inner_x() - w, tb_inner_x() - 1) }
}

/// The width of the Cell colors panel: all its columns.
fn colors_width() -> i32 {
    color_columns() * device().colors_panel.section_width
}

pub fn colors_pl() -> i32 {
    panel_x(colors_width()).0 // Paper Pro: 708 in portrait, 1248 in landscape, 112 for a right-handed player
}

pub fn colors_pr() -> i32 {
    panel_x(colors_width()).1 // Paper Pro: 1507 in portrait, 2047 in landscape, 911 for a right-handed player
}

pub fn colors_panel() -> Rect {
    Rect::new(colors_pl(), colors_top(), colors_pr() + 1, colors_bottom() + 1)
}

/// The left x of the column of the section `index` of the Cell colors panel (`Section::panel_index`
/// or `LINES_SECTION`). The order of the columns is the same for the two hands.
pub fn color_section_left(index: usize) -> i32 {
    colors_pl() + (index as i32 / color_rows()) * device().colors_panel.section_width
}

/// The top of the section `index` of the Cell colors panel (`Section::panel_index` or `LINES_SECTION`).
pub fn color_section_top(index: usize) -> i32 {
    colors_top() + (index as i32 % color_rows()) * device().colors_panel.section_height
}

/// The grid cell `i` (0..9, from the top left) of the section `index` of the Cell colors panel: its
/// tap area, and the black area when its item is selected.
pub fn color_grid_cell(index: usize, i: usize) -> Rect {
    let (m, i) = (device().colors_panel, i as i32);
    let x = color_section_left(index) + m.grid_inset + (i % 3) * m.item.width;
    let y = color_section_top(index) + m.grid_top + (i / 3) * m.item.height;
    Rect::new(x, y, x + m.item.width, y + m.item.height)
}

/// The grid cell of a color in a color section.
pub fn color_cell(section: Section, color: Color) -> Rect {
    color_grid_cell(section.panel_index(), color.grid_index())
}

/// The grid cell of a gray in the Micro-cell lines section.
pub fn line_gray_cell(g: LineGray) -> Rect {
    color_grid_cell(LINES_SECTION, g.grid_index())
}

/// The gray at a point in the Micro-cell lines section.
pub fn line_gray_at(x: f32, y: f32) -> Option<LineGray> {
    LineGray::ALL.into_iter().find(|&g| line_gray_cell(g).contains(x, y))
}

/// The Settings panel (REQUIREMENTS.md §9.1): one row of `PanelMetrics::row` for each switch
/// (`Switch::row`), one row with the version and, in a stable or a preview build, one row for updates
/// (§9.2), from the top of the gear (y 224). It has its own width (`SettingsMetrics::width`): the
/// width of the Cell colors panel does not apply to it (NAV-14).
fn settings_top() -> i32 {
    slot_top(Button::Settings)
}

/// The row of the version, below the switches.
pub const VERSION_ROW: usize = Switch::ALL.len();
/// The row "Third-party licenses", below the version. A tap opens the Licenses screen.
pub const LICENSES_ROW: usize = VERSION_ROW + 1;
/// The row for updates (only when `Update::has_row`).
pub const UPDATE_ROW: usize = LICENSES_ROW + 1;

pub fn settings_pl() -> i32 {
    panel_x(device().settings.width).0 // 1108 in portrait
}

pub fn settings_pr() -> i32 {
    panel_x(device().settings.width).1 // 1507 in portrait
}

pub fn settings_panel(update_row: bool) -> Rect {
    let p = device().panel;
    let rows = UPDATE_ROW as i32 + i32::from(update_row);
    // A border at the top and at the bottom, and a line between the rows.
    Rect::new(settings_pl(), settings_top(), settings_pr() + 1, settings_top() + rows * (p.row + p.line) + p.line)
}

/// A row of the Settings panel: a switch (`Switch::row`), `VERSION_ROW` or `UPDATE_ROW`.
pub fn settings_row(i: usize) -> Rect {
    let p = device().panel;
    let y = settings_top() + p.line + i as i32 * (p.row + p.line);
    Rect::new(settings_pl(), y, settings_pr() + 1, y + p.row)
}

/// The button "Update to v<version>" (§9.2, `UpdateButtonMetrics`). x1 and y1 are the last pixels
/// of the border.
pub fn update_button() -> Rect {
    let b = device().settings.update_button;
    let r = settings_row(UPDATE_ROW);
    Rect::new(settings_pl() + b.inset, r.y0 + b.top, settings_pr() - b.inset, r.y0 + b.bottom)
}

/// The tap area of the update button.
pub fn update_button_area() -> Rect {
    let b = update_button();
    Rect::new(b.x0, b.y0, b.x1 + 1, b.y1 + 1)
}

/// The switch at a point in the Settings panel.
pub fn settings_at(x: f32, y: f32) -> Option<Switch> {
    Switch::ALL.into_iter().find(|s| settings_row(s.row()).contains(x, y))
}

/// The section and the color at a point in the Cell colors panel.
pub fn color_at(x: f32, y: f32) -> Option<(Section, Color)> {
    Section::ALL
        .into_iter()
        .flat_map(|s| Color::ALL.map(|c| (s, c)))
        .find(|&(s, c)| color_cell(s, c).contains(x, y))
}

/// The Checkpoints panel (§9.3, NAV-18, `CheckpointMetrics`). It touches the toolbar and starts at
/// the top of the slot of the bookmark (y 560 on the Paper Pro). If the panel at its maximum size
/// does not fit below that slot (the Move in landscape), it starts at the top of slot 1.
pub fn ckp_top() -> i32 {
    let d = device();
    let at_icon = slot_top(Button::Checkpoints);
    let height = d.panel.row + crate::game::MAX_CHECKPOINTS as i32 * d.checkpoints.row;
    if at_icon + height < screen_h() as i32 { at_icon } else { slot_top(Button::Colors) }
}

/// The top of the first checkpoint row, below the row "Save checkpoint".
fn ckp_row0() -> i32 {
    ckp_top() + device().panel.row
}

/// A control of the Checkpoints panel. The index is the row: the index in `Game::checkpoints`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointControl {
    Save,
    Restore(usize),
    Delete(usize),
}

/// The left x of the Checkpoints panel: 908 in portrait, 1448 in landscape, 112 for a right-handed player.
pub fn ckp_pl() -> i32 {
    ckp_x().0
}

/// The left x and the right x (the last pixel) of the Checkpoints panel.
fn ckp_x() -> (i32, i32) {
    panel_x(device().checkpoints.width)
}

/// The Checkpoints panel with `n` checkpoints. It ends at y 864 with no checkpoint (NAV-18).
pub fn ckp_panel(n: usize) -> Rect {
    let (pl, pr) = ckp_x();
    let bottom = if n == 0 { ckp_top() + device().checkpoints.empty_height - 1 } else { ckp_row_top(n) };
    Rect::new(pl, ckp_top(), pr + 1, bottom + 1)
}

/// The area of the Checkpoints panel at its maximum size: the guard after a delete uses it (CKP-8).
pub fn ckp_guard_area() -> Rect {
    ckp_panel(crate::game::MAX_CHECKPOINTS)
}

/// The top of row `i` of the Checkpoints panel.
pub fn ckp_row_top(i: usize) -> i32 {
    ckp_row0() + i as i32 * device().checkpoints.row
}

/// The row "Save checkpoint": the full row is the tap area.
pub fn ckp_save_row() -> Rect {
    let (pl, pr) = ckp_x();
    Rect::new(pl, ckp_top(), pr + 1, ckp_row0())
}

/// The tap area of the button of row `i` (`CheckpointButton`): the box of the button.
pub fn ckp_button(i: usize) -> Rect {
    let b = device().checkpoints.button;
    let (x, y) = (ckp_pl(), ckp_row_top(i));
    Rect::new(x + b.left, y + b.top, x + b.right + 1, y + b.bottom + 1)
}

/// The tap area of the bin of row `i`: to the right edge of the panel, the full height of the row.
pub fn ckp_bin_area(i: usize) -> Rect {
    let m = device().checkpoints;
    let (pl, pr) = ckp_x();
    let y = ckp_row_top(i);
    Rect::new(pl + m.bin.area_left, y, pr + 1, y + m.row)
}

/// The control at a point in the Checkpoints panel with `n` checkpoints.
pub fn checkpoint_control_at(x: f32, y: f32, n: usize) -> Option<CheckpointControl> {
    if ckp_save_row().contains(x, y) {
        return Some(CheckpointControl::Save);
    }
    (0..n).find_map(|i| {
        if ckp_button(i).contains(x, y) {
            Some(CheckpointControl::Restore(i))
        } else if ckp_bin_area(i).contains(x, y) {
            Some(CheckpointControl::Delete(i))
        } else {
            None
        }
    })
}

/// The New game panel (toolbar.sh, §11): three rows of one toolbar slot with a divider of
/// `PanelMetrics::line` between each two rows. From the top: the slider, Symmetrical, and Start new
/// game (in the slot of the +). The slider fills the width between the insets of the rows
/// (`PanelMetrics::inset`). Returns the first and the last pixel column of the panel.
fn ng_panel_columns() -> (i32, i32) {
    let d = device();
    panel_x(d.slider.width + 2 * d.panel.inset)
}

pub fn ng_pl() -> i32 {
    ng_panel_columns().0 // 1002 in portrait on the Paper Pro
}

pub fn ng_pr() -> i32 {
    ng_panel_columns().1
}

/// The top of the panel: two rows and one divider above the bottom slot (on the Paper Pro in
/// portrait: 2048 - 226 = 1822).
pub fn ng_top() -> i32 {
    tb_bottom_y() - 2 * tb_slot() - device().panel.line
}

pub fn ng_panel() -> Rect {
    Rect::new(ng_pl(), ng_top(), ng_pr() + 1, screen_h() as i32)
}

/// The start of the slider and of the names of the rows.
pub fn ng_tx() -> i32 {
    ng_pl() + device().panel.inset
}

/// The end of the slider.
pub fn ng_tr() -> i32 {
    ng_tx() + device().slider.width
}

/// The center of the slider row.
pub fn ng_slider_y() -> i32 {
    ng_top() + tb_slot() / 2
}

/// The center of the Symmetrical row: half a toolbar slot above the bottom slot.
pub fn ng_symmetric_y() -> i32 {
    tb_bottom_y() - tb_slot() / 2
}

pub fn ng_start_y() -> i32 {
    icon_center(Button::NewGame).1 as i32
}

pub fn ng_slider_row() -> Rect {
    Rect::new(ng_pl(), ng_top(), ng_pr() + 1, ng_top() + tb_slot())
}

/// The Symmetrical row (DIF-8): the slot directly above the bottom slot, below the divider of the
/// slider row. A tap anywhere in it changes the switch.
pub fn ng_symmetric_row() -> Rect {
    Rect::new(ng_pl(), tb_bottom_y() - tb_slot(), ng_pr() + 1, tb_bottom_y())
}

pub fn ng_start_row() -> Rect {
    Rect::new(ng_pl(), tb_bottom_y() + device().panel.line, ng_pr() + 1, screen_h() as i32)
}

/// The x positions of the stops of a labeled slider from `xl` to `xr` (ui-lib.sh `labeled_slider`).
/// `easy_w` and `hard_w` are the text widths of the two labels ("Easy" and "Hard").
pub fn slider_stops(xl: i32, xr: i32, easy_w: f32, hard_w: f32) -> [f32; STOPS as usize] {
    let gap = device().slider.label_gap;
    let xa = xl as f32 + easy_w.round() + gap;
    let xb = xr as f32 - hard_w.round() - gap;
    let step = (xb - xa) / (STOPS - 1) as f32;
    std::array::from_fn(|i| xa + i as f32 * step)
}

/// The nearest stop to x.
pub fn nearest_stop(stops: &[f32; STOPS as usize], x: f32) -> u8 {
    let mut best = 0;
    for (i, s) in stops.iter().enumerate() {
        if (s - x).abs() < (stops[best] - x).abs() {
            best = i;
        }
    }
    best as u8
}

// ---- Manual and analytics: layout units of 1404x1872, with a scale to the screen (manual-lib.sh).

/// The scale of the layout units of the manual and the analytics pages (manual-lib.sh,
/// `Device::page_scale`). On the Paper Pro, the pages are 1404 x 1872 units in portrait and
/// 1872 x 1404 units in landscape.
pub fn page_scale() -> f32 {
    device().page_scale
}

/// The width and the height of a page, in layout units.
fn page_w() -> f32 {
    screen_w() as f32 / page_scale()
}

fn page_h() -> f32 {
    screen_h() as f32 / page_scale()
}

/// The height of a page of the Paper Pro in the current orientation, in layout units: its screen
/// over its page scale, rounded (1872 in portrait, 1404 in landscape). The fixed row counts of the
/// lists and of the "By difficulty" table fill this page.
fn paper_pro_page_h() -> f32 {
    let pro = Device::PAPER_PRO;
    let screen_h = if landscape() { pro.physical_w } else { pro.physical_h };
    (screen_h as f32 / pro.page_scale).round()
}

/// The height of a page more than the page of the Paper Pro in the same orientation: 624 units in
/// portrait on the Paper Pro Move, else 0. The lists and the rows of the "By difficulty" table use it
/// (REQUIREMENTS.md §1.1).
fn extra_page_h() -> f32 {
    (page_h() - paper_pro_page_h()).round().max(0.0)
}

/// The quantity of rows of height `row` that fit in the extra height of the page.
fn extra_rows(row: f32) -> usize {
    (extra_page_h() / row) as usize
}

/// The right edge of the content of a page (1332 in portrait).
pub fn page_right() -> f32 {
    page_w() - device().page.margin
}

/// Converts a rectangle in the layout units of the manual to screen pixels.
pub fn page_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::new(
        (x0 * page_scale()) as i32,
        (y0 * page_scale()) as i32,
        (x1 * page_scale()).ceil() as i32,
        (y1 * page_scale()).ceil() as i32,
    )
}

/// "< Back" at the top left of the manual and the analytics screens.
pub fn back_button() -> Rect {
    let back = device().page.back;
    page_rect(0.0, 0.0, back.width, back.height)
}

/// The center x and the baseline of the page number. The arrows are at 74 to 84 units from the
/// center (on the Paper Pro in portrait: ‹ at x 618..628, › at x 776..786, y 1815..1835).
pub fn page_number_pos() -> (f32, f32) {
    (page_w() / 2.0, page_h() - device().page.page_number.text.baseline)
}

/// The tap area of ‹, at the left of the page number.
pub fn prev_arrow() -> Rect {
    let (cx, _) = page_number_pos();
    let n = device().page.page_number;
    page_rect(cx - n.tap_outer, page_h() - n.tap_height, cx - n.tap_inner, page_h())
}

/// The tap area of ›, at the right of the page number.
pub fn next_arrow() -> Rect {
    let (cx, _) = page_number_pos();
    let n = device().page.page_number;
    page_rect(cx + n.tap_inner, page_h() - n.tap_height, cx + n.tap_outer, page_h())
}

/// The top of the row of "All completed games" and "Delete all games".
pub fn summary_buttons_y() -> f32 {
    let s = device().summary;
    s.table.top + STOPS as f32 * summary_row() + s.buttons.below_table
}

/// The height of a row of the "By difficulty" table: 97 units in portrait. In landscape the seven rows
/// must fit on a page of 1404 units, thus the rows are 76 units (REQUIREMENTS.md §13.1). A taller page
/// (the Move in portrait: 186 units) divides its extra height between the seven rows.
pub fn summary_row() -> f32 {
    let d = device();
    (if landscape() { d.summary.table.landscape_row } else { d.page.row }) + (extra_page_h() / STOPS as f32).floor()
}

/// The left edge of "Delete all games".
pub fn delete_all_x() -> f32 {
    page_right() - device().summary.buttons.delete_all_width
}

/// The right edge of "All completed games", at the left of "Delete all games".
pub fn all_games_right() -> f32 {
    delete_all_x() - device().summary.buttons.gap
}

/// "All completed games", at the left of the row below the table (REQUIREMENTS.md §13.1).
pub fn all_games_button() -> Rect {
    let (p, y) = (device().page, summary_buttons_y());
    page_rect(p.margin, y, all_games_right(), y + p.row)
}

/// "Delete all games", at the right of "All completed games" (REQUIREMENTS.md §13.1).
pub fn delete_all_button() -> Rect {
    let y = summary_buttons_y();
    page_rect(delete_all_x(), y, page_right(), y + device().page.row)
}

/// The width of each of the two boxes at the top of the summary (616 in portrait on the Paper Pro).
pub fn summary_box_w() -> f32 {
    let d = device();
    (page_w() - (2.0 * d.page.margin + d.summary.boxes.gap)) / 2.0
}

/// The right edge of the "Completed" column of the summary (900 in portrait on the Paper Pro).
pub fn summary_count_x() -> f32 {
    page_right() - device().summary.table.completed_end_from_right
}

/// The width of each filter segment of the games list (All, then modes 0..6): the segments fill the
/// width of the list (157.5 units in portrait on the Paper Pro).
pub fn seg_w() -> f32 {
    list_column().1 / (STOPS + 1) as f32
}

/// The filters of the games list, in the order of their segments from the left: All (`None`), then
/// each mode (`Some(mode)`). The value is the filter that `ListUi::filter` keeps.
pub fn list_filters() -> impl Iterator<Item = Option<u8>> {
    std::iter::once(None).chain((0..crate::generator::MODES).map(Some))
}

/// The left x of the segment of a filter, in page units.
pub fn filter_segment_x(filter: Option<u8>) -> f32 {
    let position = filter.map_or(0, |mode| mode as usize + 1);
    list_column().0 + position as f32 * seg_w()
}

/// The tap area of the segment of a filter: the full segment.
pub fn filter_segment(filter: Option<u8>) -> Rect {
    let x = filter_segment_x(filter);
    let f = device().games_list.filter;
    page_rect(x, f.top, x + seg_w(), f.top + f.height)
}

/// The quantity of games on a page of the list: 12 in portrait, 6 in landscape (the height of the
/// screen is too small for 12 rows) on the Paper Pro, plus the rows that fit on a taller page (18
/// on the Move in portrait). The table is across the full width (REQUIREMENTS.md §3.3).
pub fn list_rows() -> usize {
    let d = device();
    let g = d.games_list;
    (if landscape() { g.landscape_rows } else { g.portrait_rows }) + extra_rows(d.page.row)
}

/// The left x and the width of the games list and of the Licenses list: from the margin at the left
/// of the page to the margin at the right.
pub fn list_column() -> (f32, f32) {
    let margin = device().page.margin;
    (margin, page_w() - 2.0 * margin)
}

/// The x offsets in a row of the games list, from the left of the list: the meter, the end of the
/// time, and the center of the bin.
pub fn list_offsets() -> (f32, f32, f32) {
    let (g, w) = (device().games_list, list_column().1);
    (g.meter_dx, w - g.time_end_from_right, w - g.bin_from_right)
}

/// The top y of the row of game i of a page.
pub fn list_row(i: usize) -> f32 {
    let d = device();
    d.games_list.top + i as f32 * d.page.row
}

/// The bin of list row i.
pub fn bin_button(i: usize) -> Rect {
    let d = device();
    let (y, half) = (list_row(i), d.games_list.bin_tap_half_width);
    let bx = list_column().0 + list_offsets().2;
    page_rect(bx - half, y, bx + half, y + d.page.row)
}

/// The confirm dialogs (the delete dialogs of analytics.py and the restart dialog) at the center
/// of the page: x 252..1152, y 760..1110 on the Paper Pro in portrait.
/// The buttons are in the last row.
pub fn dialog() -> (f32, f32, f32, f32) {
    let d = device().dialog;
    let (x0, y0) = (page_w() / 2.0 - d.width / 2.0, page_h() / 2.0 - d.above_center);
    (x0, y0, x0 + d.width, y0 + d.height)
}

/// The left button of the dialog (Cancel or Later).
pub fn dialog_cancel() -> Rect {
    let (x0, _, x1, y1) = dialog();
    page_rect(x0, y1 - device().page.row, (x0 + x1) / 2.0, y1)
}

/// The right button of the dialog (Delete or Restart).
pub fn dialog_delete() -> Rect {
    let (x0, _, x1, y1) = dialog();
    page_rect((x0 + x1) / 2.0, y1 - device().page.row, x1, y1)
}

// ---- Licenses screens (page units)

/// The quantity of entries on a page of the Licenses list: 14 in portrait, 9 in landscape on the
/// Paper Pro, plus the rows that fit on a taller page (20 on the Move in portrait).
pub fn licenses_rows() -> usize {
    let d = device();
    let l = d.licenses.list;
    (if landscape() { l.landscape_rows } else { l.portrait_rows }) + extra_rows(d.page.row)
}

/// The top y of the row of entry i of a page of the Licenses list.
pub fn licenses_row(i: usize) -> f32 {
    let d = device();
    d.licenses.list.top + i as f32 * d.page.row
}

/// The tap area of row i of the Licenses list: the full width of the page.
pub fn licenses_row_rect(i: usize) -> Rect {
    let y = licenses_row(i);
    page_rect(0.0, y, page_w(), y + device().page.row)
}

/// The quantity of lines of license text on a page: 40 in portrait, 27 in landscape on the Paper
/// Pro, plus the lines that fit on a taller page (57 on the Move in portrait).
pub fn license_lines() -> usize {
    let t = device().licenses.text;
    (if landscape() { t.landscape_lines } else { t.portrait_lines }) + extra_rows(t.line_spacing)
}

/// The baseline of line i of a page of license text.
pub fn license_line(i: usize) -> f32 {
    let t = device().licenses.text;
    t.first_baseline + i as f32 * t.line_spacing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_rect_is_the_inverse_of_physical_rect_in_each_rotation() {
        for rotation in 0..4 {
            set_rotation(rotation);
            for r in [(0, 0, 0, 0), (148, 418, 294, 564), (3, 7, 100, 40), (screen_w() as i32 - 5, screen_h() as i32 - 9, screen_w() as i32 - 1, screen_h() as i32 - 1)] {
                let (x, y, w, h) = screen_rect(physical_rect(r));
                assert_eq!((x, y, x + w - 1, y + h - 1), r, "rotation {rotation}");
            }
        }
        set_rotation(0);
    }

    #[test]
    fn board_geometry_agrees_with_requirements() {
        assert_eq!((board_x(), board_y()), (125, 395));
        assert_eq!(board_x() + board_size(), 1494);
        assert_eq!(board_y() + board_size(), 1764);
        // BRD-11: the lines have their own pixels: cell (0, 0) starts after the border, and the
        // cells of a box are 3 px apart, the boxes 7 px.
        assert_eq!(cell_origin((0, 0)), (132, 402));
        assert_eq!(cell_origin((0, 1)).0 - cell_origin((0, 0)).0, 147 + 3);
        assert_eq!(cell_origin((0, 3)).0 - cell_origin((0, 2)).0, 147 + 7);
        assert_eq!(cell_origin((8, 8)).0 + 147 + 7, board_x() + board_size());
        assert_eq!([micro_offset(1), micro_offset(2), micro_offset(3)], [49, 98, 147]);
        // NAV-10: the open toolbar does not cover the board.
        for o in [Orientation::Portrait, Orientation::Landscape] {
            set_orientation(o);
            assert!(tb_x() > board_x() + board_size(), "{o:?}");
        }
        set_orientation(Orientation::Portrait);
    }

    #[test]
    fn rotations_map_the_screen_onto_the_framebuffer() {
        for r in 0..4 {
            set_rotation(r);
            let (w, h) = (screen_w() as i32, screen_h() as i32);
            let mut seen = std::collections::HashSet::new();
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1), (w / 3, h / 5)] {
                let (px, py) = to_physical(x, y);
                assert!((0..physical_w() as i32).contains(&px) && (0..physical_h() as i32).contains(&py), "r {r}");
                assert!(seen.insert((px, py)));
                // The input of qtfb comes back to the same screen point.
                assert_eq!(to_screen(px as f32, py as f32), (x as f32, y as f32), "r {r}");
            }
        }
        set_rotation(0);
    }

    #[test]
    fn portrait_pages_keep_the_values_of_the_images() {
        set_rotation(0);
        assert_eq!((page_w().round(), page_h().round()), (1404.0, 1872.0));
        assert_eq!((page_right().round(), summary_box_w().round(), summary_count_x().round(), (seg_w() * 2.0).round()), (1332.0, 616.0, 900.0, 315.0));
        let (x0, y0, x1, y1) = dialog();
        assert_eq!((x0.round(), y0.round(), x1.round(), y1.round()), (252.0, 760.0, 1152.0, 1110.0));
        assert_eq!(((list_column().0 + list_offsets().1).round(), (list_column().0 + list_offsets().2).round()), (1110.0, 1290.0));
        assert_eq!(prev_arrow(), page_rect(560.0, 1770.0, 660.0, 1872.0));
        set_orientation(Orientation::Landscape);
        assert_eq!((list_column().0 + list_offsets().2).round(), (page_right() - 42.0).round(), "the table is across the full width");
        assert!(list_row(list_rows()) < page_h() - 102.0, "the rows fit above the page arrows");
        set_rotation(0);
    }

    #[test]
    fn micro_cells_use_keypad_positions() {
        let (x, y) = cell_origin((4, 4));
        assert_eq!(board_micro(x as f32 + 5.0, y as f32 + 5.0), Some(((4, 4), 1)));
        assert_eq!(board_micro(x as f32 + 140.0, y as f32 + 5.0), Some(((4, 4), 3)));
        assert_eq!(board_micro(x as f32 + 70.0, y as f32 + 70.0), Some(((4, 4), 5)));
        assert_eq!(board_micro(x as f32 + 140.0, y as f32 + 140.0), Some(((4, 4), 9)));
        assert_eq!(board_micro(100.0, 100.0), None);
    }

    #[test]
    fn toolbar_buttons() {
        assert_eq!(game_button(1560.0, 50.0, false), Some(Button::Menu));
        assert_eq!(game_button(1560.0, 160.0, false), None);
        assert_eq!(game_button(50.0, 50.0, false), None);
        assert_eq!(game_button(50.0, 50.0, true), Some(Button::Close));
        assert_eq!(game_button(1560.0, 160.0, true), Some(Button::Colors));
        assert_eq!(game_button(1560.0, 280.0, true), Some(Button::Settings));
        assert_eq!(game_button(1560.0, 390.0, true), Some(Button::Undo));
        assert_eq!(game_button(1560.0, 500.0, true), Some(Button::Redo));
        assert_eq!(game_button(1560.0, 1880.0, true), Some(Button::Help));
        assert_eq!(game_button(1560.0, 1992.0, true), Some(Button::Statistics));
        assert_eq!(game_button(1560.0, 2104.0, true), Some(Button::NewGame));
        assert_eq!(game_button(1560.0, 1000.0, true), None);
    }

    #[test]
    fn right_handed_toolbar_buttons() {
        // NAV-15: the toolbar is at the left edge and the close button at the top right.
        set_right_handed(true);
        assert_eq!(game_button(60.0, 50.0, false), Some(Button::Menu));
        assert_eq!(game_button(1560.0, 50.0, false), None);
        assert_eq!(game_button(1560.0, 50.0, true), Some(Button::Close));
        assert_eq!(close_cx(), 1563.5);
        assert_eq!(game_button(60.0, 160.0, true), Some(Button::Colors));
        assert_eq!(game_button(60.0, 2104.0, true), Some(Button::NewGame));
        assert_eq!(game_button(200.0, 160.0, true), None);
        assert_eq!((colors_pl(), colors_pr(), ng_pl(), ng_pr()), (112, 911, 112, 617));
        assert_eq!((settings_pl(), settings_pr()), (112, 511));
        set_right_handed(false);
        assert_eq!((colors_pl(), colors_pr(), ng_pl(), ng_pr()), (708, 1507, 1002, 1507));
        assert_eq!((settings_pl(), settings_pr()), (1108, 1507));
    }

    #[test]
    fn the_colors_panel_agrees_with_nav_17_in_all_layouts() {
        for (rotation, right, x0, x1) in [(0, false, 708, 1507), (2, false, 1248, 2047), (0, true, 112, 911), (2, true, 112, 911)] {
            set_rotation(rotation);
            set_right_handed(right);
            let p = colors_panel();
            assert_eq!((p.x0, p.x1 - 1, p.y0, p.y1 - 1), (x0, x1, 112, 1192), "{rotation} {right}");
            // The order of the columns does not change with the hand.
            assert_eq!(color_section_left(Section::Starting.panel_index()), x0);
            assert_eq!(color_section_left(Section::Completed.panel_index()), x0 + 400);
            assert_eq!(color_section_left(LINES_SECTION), x0 + 400);
            assert_eq!(color_section_top(Section::Highlight.panel_index()), 832);
            assert_eq!(color_section_top(Section::Completed.panel_index()), 112);
            assert_eq!(color_section_top(LINES_SECTION), 472);
            // The corners of the panel are on no button, and the meter is below the panel.
            for (x, y) in [(p.x0, p.y0), (p.x1 - 1, p.y0), (p.x0, p.y1 - 1), (p.x1 - 1, p.y1 - 1)] {
                assert_eq!(game_button(x as f32, y as f32, true), None, "{rotation} {right} {x} {y}");
            }
            assert!(p.y1 + 13 < meter_y(), "{rotation} {right}");
            // The grids of the two columns stay in their columns.
            for s in Section::ALL {
                for c in Color::ALL {
                    let r = color_cell(s, c);
                    let left = color_section_left(s.panel_index());
                    assert!(r.x0 >= left && r.x1 <= left + device().colors_panel.section_width, "{s:?} {c:?}");
                    assert_eq!(color_at(r.x0 as f32, r.y0 as f32), Some((s, c)));
                }
            }
            // The empty space below the Micro-cell lines section is not an item.
            let empty = (p.x1 - 200, colors_top() + 2 * device().colors_panel.section_height + 100);
            assert_eq!(color_at(empty.0 as f32, empty.1 as f32), None);
            assert_eq!(line_gray_at(empty.0 as f32, empty.1 as f32), None);
        }
        set_right_handed(false);
        set_rotation(0);
    }

    #[test]
    fn the_checkpoints_panel_agrees_with_nav_18_in_all_layouts() {
        for (rotation, right, x0, x1) in [(0, false, 908, 1507), (2, false, 1448, 2047), (0, true, 112, 711), (2, true, 112, 711)] {
            set_rotation(rotation);
            set_right_handed(right);
            for (n, bottom) in [(0, 864), (1, 852), (3, 1212), (5, 1572)] {
                let p = ckp_panel(n);
                assert_eq!((p.x0, p.x1 - 1, p.y0, p.y1 - 1), (x0, x1, 560, bottom), "{rotation} {right} {n}");
                // The panel covers no button and not the meter (CKP-9: it never shows with the result card).
                for (x, y) in [(p.x0, p.y0), (p.x1 - 1, p.y0), (p.x0, p.y1 - 1), (p.x1 - 1, p.y1 - 1)] {
                    assert_eq!(game_button(x as f32, y as f32, true), None, "{rotation} {right} {x} {y}");
                }
                assert!(p.y1 + 13 < meter_y() || p.x1 <= meter_xl() || p.x0 >= meter_xr(), "{rotation} {right}");
            }
            assert_eq!(game_button(tb_cx(), 560.0, true), Some(Button::Checkpoints));
            assert_eq!(game_button(tb_cx(), 671.0, true), Some(Button::Checkpoints));
            let (row, button) = (ckp_row_top(1), ckp_button(1));
            assert_eq!((button.x0 - x0, button.x1 - 1 - x0, button.y0 - row, button.y1 - 1 - row), (381, 521, 55, 125));
            // CKP-7: the 11 px between the button and the bin operate nothing. x 908 + 533 = 1441.
            let on = |x: i32, y: i32| checkpoint_control_at(x as f32, y as f32, 5);
            assert_eq!(on(x0 + 521, row + 90), Some(CheckpointControl::Restore(1)));
            assert_eq!(on(x0 + 522, row + 90), None);
            assert_eq!(on(x0 + 532, row + 90), None);
            assert_eq!(on(x0 + 533, row + 90), Some(CheckpointControl::Delete(1)));
            assert_eq!(on(x1, row), Some(CheckpointControl::Delete(1)));
            assert_eq!(on(x0, 560), Some(CheckpointControl::Save));
            assert_eq!(on(x0, 672), None);
            assert_eq!(checkpoint_control_at((x0 + 533) as f32, row as f32, 1), None, "there is no row 1");
        }
        set_right_handed(false);
        set_rotation(0);
    }

    #[test]
    fn the_settings_panel_stays_400_px_wide_with_its_rows() {
        // NAV-14, UPD-1, LIC-1
        assert_eq!(VERSION_ROW, 3);
        assert_eq!(LICENSES_ROW, 4);
        assert_eq!(UPDATE_ROW, 5);
        let (dev, stable) = (settings_panel(false), settings_panel(true));
        assert_eq!((dev.x0, dev.x1 - 1, dev.y0, dev.y1 - 1), (1108, 1507, 224, 795));
        assert_eq!(stable.y1 - 1, 909);
        assert_eq!(settings_row(LICENSES_ROW).y0, 682);
        assert_eq!(settings_row(2).y0, 454);
        assert_eq!(settings_at(1200.0, 500.0), Some(Switch::ColorCompletedGroups));
        assert_eq!(settings_at(1200.0, 600.0), None, "the version row is not a switch");
        let b = update_button();
        assert_eq!((b.x0, b.x1, b.y0, b.y1), (1141, 1474, 818, 886));
    }
}
