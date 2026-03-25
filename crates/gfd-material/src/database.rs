//! Built-in material property database.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A single material entry in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialEntry {
    /// Material name.
    pub name: String,
    /// Material category ("fluid" or "solid").
    pub category: String,
    /// Density [kg/m^3].
    pub density: f64,
    /// Dynamic viscosity [Pa*s] (fluids only, 0 for solids).
    pub viscosity: f64,
    /// Specific heat capacity [J/(kg*K)].
    pub specific_heat: f64,
    /// Thermal conductivity [W/(m*K)].
    pub conductivity: f64,
    /// Young's modulus [Pa] (solids only, 0 for fluids).
    pub youngs_modulus: f64,
    /// Poisson's ratio (solids only, 0 for fluids).
    pub poissons_ratio: f64,
}

/// Material database holding named material entries.
#[derive(Debug, Clone)]
pub struct MaterialDatabase {
    /// Map from material name to entry.
    pub entries: HashMap<String, MaterialEntry>,
}

impl MaterialDatabase {
    /// Creates an empty material database.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Inserts a material entry into the database.
    pub fn insert(&mut self, entry: MaterialEntry) {
        self.entries.insert(entry.name.clone(), entry);
    }

    /// Looks up a material by name.
    pub fn get(&self, name: &str) -> Option<&MaterialEntry> {
        self.entries.get(name)
    }

    /// Returns all material names in the database.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for MaterialDatabase {
    fn default() -> Self {
        load_default_database()
    }
}

/// Loads the default material database with common engineering materials.
pub fn load_default_database() -> MaterialDatabase {
    let mut db = MaterialDatabase::new();

    // Air at 20 C, 1 atm
    db.insert(MaterialEntry {
        name: "air".to_string(),
        category: "fluid".to_string(),
        density: 1.225,
        viscosity: 1.789e-5,
        specific_heat: 1006.0,
        conductivity: 0.0257,
        youngs_modulus: 0.0,
        poissons_ratio: 0.0,
    });

    // Water at 20 C, 1 atm
    db.insert(MaterialEntry {
        name: "water".to_string(),
        category: "fluid".to_string(),
        density: 998.2,
        viscosity: 1.002e-3,
        specific_heat: 4182.0,
        conductivity: 0.598,
        youngs_modulus: 0.0,
        poissons_ratio: 0.0,
    });

    // Aluminum 6061-T6
    db.insert(MaterialEntry {
        name: "aluminum".to_string(),
        category: "solid".to_string(),
        density: 2700.0,
        viscosity: 0.0,
        specific_heat: 896.0,
        conductivity: 167.0,
        youngs_modulus: 69.0e9,
        poissons_ratio: 0.33,
    });

    // Structural steel (AISI 1020)
    db.insert(MaterialEntry {
        name: "steel".to_string(),
        category: "solid".to_string(),
        density: 7850.0,
        viscosity: 0.0,
        specific_heat: 486.0,
        conductivity: 51.9,
        youngs_modulus: 200.0e9,
        poissons_ratio: 0.3,
    });

    // Copper (pure)
    db.insert(MaterialEntry {
        name: "copper".to_string(),
        category: "solid".to_string(),
        density: 8960.0,
        viscosity: 0.0,
        specific_heat: 385.0,
        conductivity: 401.0,
        youngs_modulus: 117.0e9,
        poissons_ratio: 0.34,
    });

    db
}
