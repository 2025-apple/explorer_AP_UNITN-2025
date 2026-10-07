//! Eco's resource inventory. All access goes through these methods —
//! nothing outside this module reaches into the raw `Vec` directly.

use std::collections::{HashMap, HashSet};

use common_game::components::resource::{BasicResourceType, ComplexResourceType, GenericResource};

#[derive(Debug, Default)]
pub struct Bag {
    resources: Vec<GenericResource>,
}

impl Bag {
    /// Removes and returns the first basic resource of the given type, if any.
    pub fn take_basic(&mut self, want: BasicResourceType) -> Option<GenericResource> {
        let idx = self.resources.iter().position(|r| match r {
            GenericResource::BasicResources(br) => br.get_type() == want,
            GenericResource::ComplexResources(_) => false,
        })?;
        Some(self.resources.remove(idx))
    }

    /// Removes and returns the first complex resource of the given type, if any.
    /// VERIFY: assumes `ComplexResource::get_type() -> ComplexResourceType`
    /// exists, symmetric to the confirmed `BasicResource::get_type()`.
    pub fn take_complex(&mut self, want: ComplexResourceType) -> Option<GenericResource> {
        let idx = self.resources.iter().position(|r| match r {
            GenericResource::ComplexResources(cr) => cr.get_type() == want,
            GenericResource::BasicResources(_) => false,
        })?;
        Some(self.resources.remove(idx))
    }

    /// Adds a resource to the bag — used both for "put this back, a
    /// combine attempt failed" and for "here's what I just mined/built".
    pub fn put_back(&mut self, r: GenericResource) {
        self.resources.push(r);
    }

    pub fn basic_counts(&self) -> HashMap<BasicResourceType, u32> {
        let mut m = HashMap::new();
        for r in &self.resources {
            if let GenericResource::BasicResources(b) = r {
                *m.entry(b.get_type()).or_insert(0) += 1;
            }
        }
        m
    }

    pub fn complex_set(&self) -> HashSet<ComplexResourceType> {
        self.resources
            .iter()
            .filter_map(|r| match r {
                GenericResource::ComplexResources(c) => Some(c.get_type()),
                GenericResource::BasicResources(_) => None,
            })
            .collect()
    }
}