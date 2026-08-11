use std::collections::HashSet;

use pci::{PciAddress, PciDevice, PciSnapshot};

use crate::tree::BridgeWindow;

pub struct Row {
    pub depth: usize,
    pub parent: Option<usize>,
    pub label: String,
    pub address: Option<PciAddress>,
    pub expandable: bool,
}

pub struct TreeModel {
    pub rows: Vec<Row>,
    pub filter: String,
    expanded: HashSet<usize>,
}

impl TreeModel {
    pub fn build(snapshot: &PciSnapshot, windows: &[(PciAddress, BridgeWindow)]) -> TreeModel {
        let mut rows: Vec<Row> = Vec::new();

        let mut buses: Vec<(u16, u8)> = snapshot
            .devices()
            .iter()
            .map(|device| (device.address.domain, device.address.bus))
            .collect();
        buses.sort_unstable();
        buses.dedup();

        for (domain, bus) in buses {
            let covered = windows
                .iter()
                .any(|(_, window)| bus >= window.secondary && bus <= window.subordinate);
            if covered {
                continue;
            }
            let parent = rows.len();
            rows.push(Row {
                depth: 0,
                parent: None,
                label: format!("{domain:04x}:{bus:02x}"),
                address: None,
                expandable: true,
            });
            push_bus_devices(&mut rows, snapshot, windows, parent, bus, 1);
        }

        let expanded = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.depth == 0)
            .map(|(index, _)| index)
            .collect();

        TreeModel {
            rows,
            filter: String::new(),
            expanded,
        }
    }

    pub fn visible_rows(&self) -> Vec<usize> {
        let filtering = !self.filter.is_empty();
        let lower = self.filter.to_lowercase();
        let matched: Vec<bool> = self
            .rows
            .iter()
            .map(|row| filtering && row.label.to_lowercase().contains(&lower))
            .collect();

        let mut out = Vec::new();
        let mut i = 0;
        while i < self.rows.len() {
            let has_match = subtree_has_match(&self.rows, &matched, i);
            let keep = if filtering {
                matched[i] || has_match
            } else {
                true
            };
            let descend = if filtering {
                has_match
            } else {
                self.rows[i].expandable && self.expanded.contains(&i)
            };
            if keep {
                out.push(i);
            }
            if keep && descend {
                i += 1;
            } else {
                i = next_sibling_index(&self.rows, i);
            }
        }
        out
    }

    pub fn expand(&mut self, row: usize) {
        if self.rows[row].expandable {
            self.expanded.insert(row);
        }
    }

    pub fn collapse(&mut self, row: usize) {
        self.expanded.remove(&row);
    }

    pub fn is_expanded(&self, row: usize) -> bool {
        self.expanded.contains(&row)
    }

    pub fn parent(&self, row: usize) -> Option<usize> {
        self.rows[row].parent
    }
}

fn push_bus_devices(
    rows: &mut Vec<Row>,
    snapshot: &PciSnapshot,
    windows: &[(PciAddress, BridgeWindow)],
    parent: usize,
    bus: u8,
    depth: usize,
) {
    let mut devices: Vec<&PciDevice> = snapshot
        .devices()
        .iter()
        .filter(|device| device.address.bus == bus)
        .collect();
    devices.sort_by_key(|device| (device.address.slot, device.address.function));

    for device in devices {
        let window = windows
            .iter()
            .find(|(address, _)| *address == device.address)
            .map(|(_, window)| window);
        let mut label = format!(
            "{:02x}:{:02x}.{} {} {}",
            device.address.bus,
            device.address.slot,
            device.address.function,
            device.vendor_name,
            device.device_name
        );
        if let Some(window) = window {
            label.push_str(&format!(
                " -[{:02x}-{:02x}]",
                window.secondary, window.subordinate
            ));
        }
        let index = rows.len();
        rows.push(Row {
            depth,
            parent: Some(parent),
            label,
            address: Some(device.address),
            expandable: window.is_some(),
        });
        if let Some(window) = window {
            push_bus_devices(rows, snapshot, windows, index, window.secondary, depth + 1);
        }
    }
}

fn next_sibling_index(rows: &[Row], index: usize) -> usize {
    let depth = rows[index].depth;
    let mut j = index + 1;
    while j < rows.len() && rows[j].depth > depth {
        j += 1;
    }
    j
}

fn subtree_has_match(rows: &[Row], matched: &[bool], index: usize) -> bool {
    if matched[index] {
        return true;
    }
    let depth = rows[index].depth;
    let mut j = index + 1;
    while j < rows.len() && rows[j].depth > depth {
        if matched[j] {
            return true;
        }
        j += 1;
    }
    false
}
