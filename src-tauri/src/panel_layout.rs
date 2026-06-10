/// Right-edge X for a panel of `outer_width` inside a monitor work area.
pub fn panel_anchor_x(work_area_x: i32, work_area_width: u32, outer_width: u32) -> i32 {
    work_area_x + work_area_width as i32 - outer_width as i32
}

/// Initial place X when opening the panel with a given width.
pub fn panel_place_x(work_area_x: i32, work_area_width: u32, panel_width: u32) -> i32 {
    panel_anchor_x(work_area_x, work_area_width, panel_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_anchor_x_at_right_edge() {
        assert_eq!(panel_anchor_x(100, 1920, 360), 100 + 1920 - 360);
    }

    #[test]
    fn panel_place_x_matches_anchor_for_same_width() {
        assert_eq!(panel_place_x(0, 800, 400), panel_anchor_x(0, 800, 400));
    }

    #[test]
    fn panel_anchor_x_updates_when_panel_widens() {
        let work_x = 0;
        let work_w = 1920;
        let narrow = panel_anchor_x(work_x, work_w, 360);
        let wide = panel_anchor_x(work_x, work_w, 520);
        assert_eq!(narrow - wide, 520 - 360);
        assert_eq!(wide + 520, work_x + work_w as i32);
    }
}
