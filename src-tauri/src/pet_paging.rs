//! 桌宠分页的纯计算与前端专用视图快照。

use crate::model::{
    LanguagePreference, MonitorState, MonitorTile, PetLayout, PetWindowPreferences,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PageDirection {
    Previous,
    Next,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetViewSlot {
    pub(crate) slot_index: usize,
    pub(crate) tile: Option<MonitorTile>,
}

/// 桌宠渲染一次所需的完整快照，不参与偏好持久化。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetViewState {
    pub(crate) language: LanguagePreference,
    pub(crate) port: u16,
    pub(crate) layout: PetLayout,
    pub(crate) locked: bool,
    pub(crate) page_index: usize,
    pub(crate) page_count: usize,
    pub(crate) page_has_image: bool,
    pub(crate) has_any_image: bool,
    pub(crate) slots: Vec<PetViewSlot>,
}

pub(crate) const fn layout_capacity(layout: PetLayout) -> usize {
    layout.capacity()
}

pub(crate) fn visible_slot_count(rows: u8, columns: u8) -> usize {
    usize::from(rows)
        .saturating_mul(usize::from(columns))
        .clamp(1, 25)
}

pub(crate) fn page_count(visible_slots: usize, capacity: usize) -> usize {
    visible_slots.max(1).div_ceil(capacity.max(1))
}

pub(crate) fn page_index(focused_slot: u8, visible_slots: usize, capacity: usize) -> usize {
    let pages = page_count(visible_slots, capacity);
    (usize::from(focused_slot) / capacity.max(1)).min(pages - 1)
}

pub(crate) fn page_start(page_index: usize, capacity: usize) -> usize {
    page_index.saturating_mul(capacity.max(1))
}

pub(crate) fn turn_page_index(
    current_page: usize,
    pages: usize,
    direction: PageDirection,
) -> usize {
    let pages = pages.max(1);
    let current_page = current_page.min(pages - 1);
    match direction {
        PageDirection::Previous => (current_page + pages - 1) % pages,
        PageDirection::Next => (current_page + 1) % pages,
    }
}

pub(crate) fn focused_slot_after_turn(
    layout: PetLayout,
    rows: u8,
    columns: u8,
    focused_slot: u8,
    direction: PageDirection,
) -> u8 {
    let visible_slots = visible_slot_count(rows, columns);
    let capacity = layout_capacity(layout);
    let pages = page_count(visible_slots, capacity);
    if pages == 1 {
        return clamp_focused_slot(focused_slot, rows, columns);
    }
    let current = page_index(focused_slot, visible_slots, capacity);
    page_start(turn_page_index(current, pages, direction), capacity) as u8
}

pub(crate) fn clamp_focused_slot(slot: u8, rows: u8, columns: u8) -> u8 {
    slot.min(visible_slot_count(rows, columns) as u8 - 1)
}

fn tile_has_image(tile: &MonitorTile) -> bool {
    tile.image_filename
        .as_deref()
        .is_some_and(|filename| !filename.is_empty())
}

pub(crate) fn page_has_image(
    tiles: &[MonitorTile],
    visible_slots: usize,
    start: usize,
    capacity: usize,
) -> bool {
    let end = start.saturating_add(capacity).min(visible_slots);
    tiles
        .get(start..end)
        .is_some_and(|page| page.iter().any(tile_has_image))
}

pub(crate) fn first_populated_slot(tiles: &[MonitorTile], visible_slots: usize) -> Option<usize> {
    tiles.iter().take(visible_slots).position(tile_has_image)
}

/// 仅在当前页无图时选中第一个有图的槽位。
pub(crate) fn first_populated_focus(
    tiles: &[MonitorTile],
    layout: PetLayout,
    rows: u8,
    columns: u8,
    focused_slot: u8,
) -> Option<u8> {
    let visible_slots = visible_slot_count(rows, columns);
    let capacity = layout_capacity(layout);
    let current_page = page_index(focused_slot, visible_slots, capacity);
    let start = page_start(current_page, capacity);
    if page_has_image(tiles, visible_slots, start, capacity) {
        return None;
    }
    first_populated_slot(tiles, visible_slots).map(|slot| slot as u8)
}

pub(crate) fn build_pet_view_state(
    state: &MonitorState,
    preferences: &PetWindowPreferences,
) -> PetViewState {
    let visible_slots = visible_slot_count(state.rows, state.columns);
    let capacity = layout_capacity(preferences.layout);
    let pages = page_count(visible_slots, capacity);
    let current_page = page_index(preferences.focused_slot, visible_slots, capacity);
    let start = page_start(current_page, capacity);
    let slots = (start..start + capacity)
        .map(|slot_index| PetViewSlot {
            slot_index,
            tile: (slot_index < visible_slots)
                .then(|| state.tiles.get(slot_index).cloned())
                .flatten(),
        })
        .collect();
    PetViewState {
        language: state.language,
        port: state.port,
        layout: preferences.layout,
        locked: preferences.locked,
        page_index: current_page,
        page_count: pages,
        page_has_image: page_has_image(&state.tiles, visible_slots, start, capacity),
        has_any_image: first_populated_slot(&state.tiles, visible_slots).is_some(),
        slots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles_with_images(indices: &[usize]) -> Vec<MonitorTile> {
        (0..25)
            .map(|index| MonitorTile {
                image_filename: indices.contains(&index).then(|| format!("{index}.png")),
                ..MonitorTile::default()
            })
            .collect()
    }

    #[test]
    fn all_six_layout_capacities_are_defined() {
        assert_eq!(layout_capacity(PetLayout::Single), 1);
        assert_eq!(layout_capacity(PetLayout::Row), 2);
        assert_eq!(layout_capacity(PetLayout::Column), 2);
        assert_eq!(layout_capacity(PetLayout::Row3), 3);
        assert_eq!(layout_capacity(PetLayout::Column3), 3);
        assert_eq!(layout_capacity(PetLayout::Grid), 4);
    }

    #[test]
    fn single_page_stays_on_the_same_page() {
        assert_eq!(page_count(4, 4), 1);
        assert_eq!(turn_page_index(0, 1, PageDirection::Previous), 0);
        assert_eq!(turn_page_index(0, 1, PageDirection::Next), 0);
        assert_eq!(
            focused_slot_after_turn(PetLayout::Grid, 2, 2, 3, PageDirection::Next),
            3
        );
    }

    #[test]
    fn page_turn_wraps_at_both_ends() {
        assert_eq!(turn_page_index(0, 3, PageDirection::Previous), 2);
        assert_eq!(turn_page_index(2, 3, PageDirection::Next), 0);
    }

    #[test]
    fn partial_last_page_and_out_of_range_focus_are_clamped() {
        assert_eq!(page_count(5, 4), 2);
        assert_eq!(page_index(24, 5, 4), 1);
        assert_eq!(page_start(1, 4), 4);
        assert_eq!(clamp_focused_slot(24, 1, 5), 4);
    }

    #[test]
    fn first_image_is_selected_only_when_current_page_has_none() {
        let tiles = tiles_with_images(&[1]);
        assert_eq!(
            first_populated_focus(&tiles, PetLayout::Grid, 2, 3, 5),
            Some(1)
        );
        assert_eq!(
            first_populated_focus(&tiles, PetLayout::Grid, 2, 3, 0),
            None
        );
    }

    #[test]
    fn view_state_pads_partial_page_with_null_tiles() {
        let mut state = test_state();
        state.rows = 1;
        state.columns = 5;
        state.tiles = tiles_with_images(&[4]);
        let preferences = PetWindowPreferences {
            focused_slot: 24,
            ..PetWindowPreferences::default()
        };
        let view = build_pet_view_state(&state, &preferences);
        let value = serde_json::to_value(view).unwrap();
        assert_eq!(value["pageIndex"], 1);
        assert_eq!(value["pageCount"], 2);
        assert_eq!(value["pageHasImage"], true);
        assert_eq!(value["hasAnyImage"], true);
        assert_eq!(value["slots"].as_array().unwrap().len(), 4);
        assert_eq!(value["slots"][0]["slotIndex"], 4);
        assert!(value["slots"][0]["tile"].is_object());
        assert!(value["slots"][1]["tile"].is_null());
        assert!(value["slots"][3]["tile"].is_null());
    }

    fn test_state() -> MonitorState {
        MonitorState {
            rows: 2,
            columns: 2,
            image_display_mode: Default::default(),
            auto_start: false,
            language: LanguagePreference::System,
            port: 10_241,
            app_version: String::new(),
            device_id: String::new(),
            device_name: String::new(),
            is_server_running: true,
            local_ip: String::new(),
            tiles: tiles_with_images(&[]),
        }
    }
}
