use std::collections::BTreeMap;

use common::world::Material;
use serde::{Deserialize, Serialize};

pub const HOTBAR_SLOTS: usize = 9;
pub const STORAGE_SLOTS: usize = 27;
pub const INVENTORY_SLOTS: usize = HOTBAR_SLOTS + STORAGE_SLOTS;
pub const STACK_LIMIT: u16 = 64;

const DEFAULT_HOTBAR: [Material; HOTBAR_SLOTS] = [
    Material::WoodPlanks,
    Material::Grass,
    Material::Dirt,
    Material::Sand,
    Material::Snow,
    Material::WhiteBrick,
    Material::GreyBrick,
    Material::Basalt,
    Material::Water,
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemStack {
    pub material: Option<Material>,
    pub count: u16,
}

impl ItemStack {
    pub const EMPTY: Self = Self {
        material: None,
        count: 0,
    };

    pub const fn new(material: Material, count: u16) -> Self {
        Self {
            material: Some(material),
            count,
        }
    }

    pub fn is_empty(self) -> bool {
        self.material.is_none() || self.count == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InventoryLayout {
    pub slots: Vec<ItemStack>,
    pub selected_hotbar: usize,
    pub creative_tab: bool,
}

impl Default for InventoryLayout {
    fn default() -> Self {
        let mut slots = vec![ItemStack::EMPTY; INVENTORY_SLOTS];
        for (slot, material) in slots.iter_mut().zip(DEFAULT_HOTBAR) {
            *slot = ItemStack::new(material, STACK_LIMIT);
        }
        Self {
            slots,
            selected_hotbar: 0,
            creative_tab: true,
        }
    }
}

impl InventoryLayout {
    pub fn sanitize(&mut self) {
        self.slots.resize(INVENTORY_SLOTS, ItemStack::EMPTY);
        self.slots.truncate(INVENTORY_SLOTS);
        self.selected_hotbar = self.selected_hotbar.min(HOTBAR_SLOTS - 1);
        for slot in &mut self.slots {
            if slot.material == Some(Material::Void) || slot.count == 0 {
                *slot = ItemStack::EMPTY;
            } else {
                slot.count = slot.count.min(STACK_LIMIT);
            }
        }
    }

    pub fn selected_stack(&self) -> ItemStack {
        self.slots
            .get(self.selected_hotbar)
            .copied()
            .unwrap_or(ItemStack::EMPTY)
    }

    pub fn select_hotbar(&mut self, index: usize) {
        self.selected_hotbar = index.min(HOTBAR_SLOTS - 1);
    }

    pub fn cycle_hotbar(&mut self, delta: i32) {
        self.selected_hotbar =
            (self.selected_hotbar as i32 + delta).rem_euclid(HOTBAR_SLOTS as i32) as usize;
    }

    pub fn select_existing_or_replace(&mut self, material: Material, unlimited: bool) {
        if let Some(index) = self.slots[..HOTBAR_SLOTS]
            .iter()
            .position(|stack| stack.material == Some(material))
        {
            self.selected_hotbar = index;
        } else if unlimited {
            self.slots[self.selected_hotbar] = ItemStack::new(material, STACK_LIMIT);
        }
    }

    pub fn reconcile(
        &mut self,
        counts: &[usize; Material::COUNT],
        unlimited: bool,
        held: Option<ItemStack>,
    ) {
        self.sanitize();
        if unlimited {
            for slot in &mut self.slots {
                if slot.material.is_some() {
                    slot.count = STACK_LIMIT;
                }
            }
            return;
        }

        let mut remaining = *counts;
        if let Some(held) = held.filter(|stack| !stack.is_empty())
            && let Some(material) = held.material
        {
            remaining[material as usize] =
                remaining[material as usize].saturating_sub(held.count as usize);
        }

        for slot in &mut self.slots {
            let Some(material) = slot.material else {
                continue;
            };
            let available = &mut remaining[material as usize];
            let assigned = (*available)
                .min(slot.count as usize)
                .min(STACK_LIMIT as usize);
            if assigned == 0 {
                *slot = ItemStack::EMPTY;
            } else {
                slot.count = assigned as u16;
                *available -= assigned;
            }
        }

        for material in Material::VALUES.into_iter().skip(1) {
            while remaining[material as usize] > 0 {
                let index = self
                    .slots
                    .iter()
                    .position(|stack| stack.material == Some(material) && stack.count < STACK_LIMIT)
                    .or_else(|| self.slots.iter().position(|stack| stack.is_empty()));
                let Some(index) = index else {
                    break;
                };
                let free = if self.slots[index].material == Some(material) {
                    usize::from(STACK_LIMIT - self.slots[index].count)
                } else {
                    self.slots[index] = ItemStack::new(material, 0);
                    STACK_LIMIT as usize
                };
                let moved = remaining[material as usize].min(free);
                self.slots[index].count += moved as u16;
                remaining[material as usize] -= moved;
            }
        }
    }

    pub fn left_click_slot(&mut self, index: usize, held: &mut Option<ItemStack>, unlimited: bool) {
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        if unlimited {
            if let Some(stack) = held.filter(|stack| !stack.is_empty()) {
                *slot = ItemStack::new(stack.material.unwrap(), STACK_LIMIT);
            } else if !slot.is_empty() {
                *held = Some(ItemStack::new(slot.material.unwrap(), STACK_LIMIT));
            }
            return;
        }

        match (*held, *slot) {
            (Some(mut cursor), target)
                if !cursor.is_empty()
                    && cursor.material == target.material
                    && target.count < STACK_LIMIT =>
            {
                let moved = cursor.count.min(STACK_LIMIT - target.count);
                slot.count += moved;
                cursor.count -= moved;
                *held = (cursor.count > 0).then_some(cursor);
            }
            (cursor, target) => {
                *slot = cursor.unwrap_or(ItemStack::EMPTY);
                *held = (!target.is_empty()).then_some(target);
            }
        }
    }

    pub fn return_held(&mut self, held: &mut Option<ItemStack>, unlimited: bool) {
        let Some(mut cursor) = held.take().filter(|stack| !stack.is_empty()) else {
            return;
        };
        if unlimited {
            return;
        }
        for slot in &mut self.slots {
            if slot.material == cursor.material && slot.count < STACK_LIMIT {
                let moved = cursor.count.min(STACK_LIMIT - slot.count);
                slot.count += moved;
                cursor.count -= moved;
                if cursor.count == 0 {
                    return;
                }
            }
        }
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_empty()) {
            *slot = cursor;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InventorySettings {
    pub worlds: BTreeMap<String, InventoryLayout>,
}

impl InventorySettings {
    pub fn layout_mut(&mut self, world_id: &str) -> &mut InventoryLayout {
        let layout = self.worlds.entry(world_id.to_owned()).or_default();
        layout.sanitize();
        layout
    }

    pub fn layout(&self, world_id: &str) -> InventoryLayout {
        let mut layout = self.worlds.get(world_id).cloned().unwrap_or_default();
        layout.sanitize();
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survival_reconciliation_preserves_order_and_matches_counts() {
        let mut layout = InventoryLayout::default();
        layout.slots.fill(ItemStack::EMPTY);
        layout.slots[4] = ItemStack::new(Material::Dirt, 10);
        let mut counts = [0; Material::COUNT];
        counts[Material::Dirt as usize] = 70;
        layout.reconcile(&counts, false, None);
        assert_eq!(layout.slots[4], ItemStack::new(Material::Dirt, STACK_LIMIT));
        assert_eq!(
            layout
                .slots
                .iter()
                .filter(|stack| stack.material == Some(Material::Dirt))
                .map(|stack| usize::from(stack.count))
                .sum::<usize>(),
            70
        );
    }

    #[test]
    fn hotbar_cycles_in_both_directions() {
        let mut layout = InventoryLayout::default();
        layout.select_hotbar(0);
        layout.cycle_hotbar(-1);
        assert_eq!(layout.selected_hotbar, 8);
        layout.cycle_hotbar(1);
        assert_eq!(layout.selected_hotbar, 0);
    }
}
