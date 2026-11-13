//! Shared basics: source positions, diagnostics, dimensions, types and the unit table.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: u32,
    pub col: u16,
    pub file: u16,
}

impl Pos {
    /// Packed form used in the bytecode line table: file in the high 12 bits.
    pub fn pack(self) -> u32 {
        ((self.file as u32 & 0xFFF) << 20) | (self.line & 0xFFFFF)
    }
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub pos: Pos,
    pub msg: String,
    pub hint: Option<String>,
}

impl Diag {
    pub fn new(pos: Pos, msg: impl Into<String>) -> Diag {
        Diag {
            pos,
            msg: msg.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Diag {
        self.hint = Some(hint.into());
        self
    }
}

fn gcd(a: i32, b: i32) -> i32 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

/// A physical dimension: sorted list of (base id, exponent numerator, exponent denominator).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Dim(pub Vec<(u16, i16, i16)>);

impl Dim {
    pub fn none() -> Dim {
        Dim(Vec::new())
    }

    pub fn base(id: u16) -> Dim {
        Dim(vec![(id, 1, 1)])
    }

    pub fn of(parts: &[(u16, i16)]) -> Dim {
        let mut v: Vec<(u16, i16, i16)> = parts.iter().map(|&(b, e)| (b, e, 1)).collect();
        v.sort();
        Dim(v)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }

    fn combine(&self, o: &Dim, sign: i32) -> Option<Dim> {
        let mut out: Vec<(u16, i16, i16)> = Vec::new();
        let (mut i, mut j) = (0usize, 0usize);
        while i < self.0.len() || j < o.0.len() {
            let take_a = j >= o.0.len() || (i < self.0.len() && self.0[i].0 < o.0[j].0);
            let take_b = i >= self.0.len() || (j < o.0.len() && o.0[j].0 < self.0[i].0);
            if take_a {
                out.push(self.0[i]);
                i += 1;
            } else if take_b {
                let (b, n, d) = o.0[j];
                out.push((b, (n as i32 * sign) as i16, d));
                j += 1;
            } else {
                let (b, n1, d1) = self.0[i];
                let (_, n2, d2) = o.0[j];
                let n = n1 as i32 * d2 as i32 + sign * n2 as i32 * d1 as i32;
                let d = d1 as i32 * d2 as i32;
                if n != 0 {
                    let g = gcd(n, d);
                    let (n, d) = (n / g, d / g);
                    if n.abs() > 30000 || d > 30000 {
                        return None;
                    }
                    out.push((b, n as i16, d as i16));
                }
                i += 1;
                j += 1;
            }
        }
        Some(Dim(out))
    }

    pub fn mul(&self, o: &Dim) -> Option<Dim> {
        self.combine(o, 1)
    }

    pub fn div(&self, o: &Dim) -> Option<Dim> {
        self.combine(o, -1)
    }

    /// Raise to the rational power n/d.
    pub fn pow(&self, n: i32, d: i32) -> Option<Dim> {
        if d == 0 {
            return None;
        }
        let mut out = Vec::new();
        for &(b, pn, pd) in &self.0 {
            let mut nn = pn as i32 * n;
            let mut dd = pd as i32 * d;
            if dd < 0 {
                nn = -nn;
                dd = -dd;
            }
            if nn == 0 {
                continue;
            }
            let g = gcd(nn, dd);
            let (nn, dd) = (nn / g, dd / g);
            if nn.abs() > 30000 || dd > 30000 {
                return None;
            }
            out.push((b, nn as i16, dd as i16));
        }
        Some(Dim(out))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Unit,
    Bool,
    Int,
    Num(Dim),
    Str,
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    Struct(u32, Vec<Ty>),
    Enum(u32, Vec<Ty>),
    Param(u32),
    Var(u32),
    Never,
    Error,
}

impl Ty {
    pub fn float() -> Ty {
        Ty::Num(Dim::none())
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Ty::Num(d) if d.is_none())
    }
}

#[derive(Clone, Debug)]
pub struct UnitDef {
    pub dim: Dim,
    pub num: f64,
    pub den: f64,
}

pub const BASE_NAMES: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];

const PI: f64 = std::f64::consts::PI;

/// The built-in unit table as a map. Every unit is `num / den` SI base units.
pub fn builtin_units() -> HashMap<String, UnitDef> {
    builtin_unit_list().into_iter().collect()
}

/// The built-in unit table in its written order (length first, then mass,
/// time, force, ...). The order is what a hint uses to pick which unit to
/// suggest first, so keep the everyday unit of each kind near the top.
pub fn builtin_unit_list() -> Vec<(String, UnitDef)> {
    let none = Dim::none();
    let length = Dim::of(&[(0, 1)]);
    let mass = Dim::of(&[(1, 1)]);
    let time = Dim::of(&[(2, 1)]);
    let current = Dim::of(&[(3, 1)]);
    let force = Dim::of(&[(0, 1), (1, 1), (2, -2)]);
    let pressure = Dim::of(&[(0, -1), (1, 1), (2, -2)]);
    let energy = Dim::of(&[(0, 2), (1, 1), (2, -2)]);
    let power = Dim::of(&[(0, 2), (1, 1), (2, -3)]);
    let charge = Dim::of(&[(2, 1), (3, 1)]);
    let voltage = Dim::of(&[(0, 2), (1, 1), (2, -3), (3, -1)]);
    let resistance = Dim::of(&[(0, 2), (1, 1), (2, -3), (3, -2)]);
    let capacitance = Dim::of(&[(0, -2), (1, -1), (2, 4), (3, 2)]);
    let inductance = Dim::of(&[(0, 2), (1, 1), (2, -2), (3, -2)]);
    let conductance = Dim::of(&[(0, -2), (1, -1), (2, 3), (3, 2)]);
    let frequency = Dim::of(&[(2, -1)]);
    let volume = Dim::of(&[(0, 3)]);
    let speed = Dim::of(&[(0, 1), (2, -1)]);

    let table: Vec<(&str, &Dim, f64, f64)> = vec![
        // length
        ("m", &length, 1.0, 1.0),
        ("km", &length, 1000.0, 1.0),
        ("cm", &length, 1.0, 100.0),
        ("mm", &length, 1.0, 1000.0),
        ("um", &length, 1.0, 1e6),
        ("nm", &length, 1.0, 1e9),
        ("inch", &length, 254.0, 10000.0),
        ("ft", &length, 3048.0, 10000.0),
        ("yd", &length, 9144.0, 10000.0),
        ("mile", &length, 1609344.0, 1000.0),
        // mass
        ("kg", &mass, 1.0, 1.0),
        ("g", &mass, 1.0, 1000.0),
        ("mg", &mass, 1.0, 1e6),
        ("t", &mass, 1000.0, 1.0),
        ("lb", &mass, 45359237.0, 1e8),
        ("oz", &mass, 28349523125.0, 1e12),
        // time
        ("s", &time, 1.0, 1.0),
        ("ms", &time, 1.0, 1000.0),
        ("us", &time, 1.0, 1e6),
        ("ns", &time, 1.0, 1e9),
        ("min", &time, 60.0, 1.0),
        ("h", &time, 3600.0, 1.0),
        ("day", &time, 86400.0, 1.0),
        // force
        ("N", &force, 1.0, 1.0),
        ("kN", &force, 1000.0, 1.0),
        ("MN", &force, 1e6, 1.0),
        ("mN", &force, 1.0, 1000.0),
        ("kgf", &force, 980665.0, 100000.0),
        ("lbf", &force, 44482216152605.0, 1e13),
        // pressure / stress
        ("Pa", &pressure, 1.0, 1.0),
        ("kPa", &pressure, 1000.0, 1.0),
        ("MPa", &pressure, 1e6, 1.0),
        ("GPa", &pressure, 1e9, 1.0),
        ("bar", &pressure, 1e5, 1.0),
        ("mbar", &pressure, 100.0, 1.0),
        ("psi", &pressure, 6894757293168.0, 1e9),
        ("atm", &pressure, 101325.0, 1.0),
        // energy and torque
        ("J", &energy, 1.0, 1.0),
        ("kJ", &energy, 1000.0, 1.0),
        ("MJ", &energy, 1e6, 1.0),
        ("Wh", &energy, 3600.0, 1.0),
        ("kWh", &energy, 3.6e6, 1.0),
        ("cal", &energy, 4184.0, 1000.0),
        ("kcal", &energy, 4184.0, 1.0),
        ("Nm", &energy, 1.0, 1.0),
        // power
        ("W", &power, 1.0, 1.0),
        ("kW", &power, 1000.0, 1.0),
        ("MW", &power, 1e6, 1.0),
        ("mW", &power, 1.0, 1000.0),
        ("hp", &power, 745.69987158227022, 1.0),
        // electrical
        ("A", &current, 1.0, 1.0),
        ("mA", &current, 1.0, 1000.0),
        ("uA", &current, 1.0, 1e6),
        ("C", &charge, 1.0, 1.0),
        ("mAh", &charge, 36.0, 10.0),
        ("Ah", &charge, 3600.0, 1.0),
        ("V", &voltage, 1.0, 1.0),
        ("mV", &voltage, 1.0, 1000.0),
        ("kV", &voltage, 1000.0, 1.0),
        ("ohm", &resistance, 1.0, 1.0),
        ("kohm", &resistance, 1000.0, 1.0),
        ("Mohm", &resistance, 1e6, 1.0),
        ("F", &capacitance, 1.0, 1.0),
        ("uF", &capacitance, 1.0, 1e6),
        ("nF", &capacitance, 1.0, 1e9),
        ("pF", &capacitance, 1.0, 1e12),
        ("H", &inductance, 1.0, 1.0),
        ("mH", &inductance, 1.0, 1000.0),
        ("uH", &inductance, 1.0, 1e6),
        ("S", &conductance, 1.0, 1.0),
        // frequency and angular speed
        ("Hz", &frequency, 1.0, 1.0),
        ("kHz", &frequency, 1000.0, 1.0),
        ("MHz", &frequency, 1e6, 1.0),
        ("GHz", &frequency, 1e9, 1.0),
        ("rpm", &frequency, PI, 30.0),
        // angle (dimensionless)
        ("rad", &none, 1.0, 1.0),
        ("deg", &none, PI, 180.0),
        ("rev", &none, 2.0 * PI, 1.0),
        // volume
        ("L", &volume, 1.0, 1000.0),
        ("mL", &volume, 1.0, 1e6),
        // speed
        ("mph", &speed, 1397.0, 3125.0),
        ("kph", &speed, 5.0, 18.0),
        ("knot", &speed, 1852.0, 3600.0),
        // ratios
        ("pct", &none, 1.0, 100.0),
        ("ppm", &none, 1.0, 1e6),
    ];

    let mut list: Vec<(String, UnitDef)> = Vec::new();
    for (name, dim, num, den) in table {
        list.push((
            name.to_string(),
            UnitDef {
                dim: dim.clone(),
                num,
                den,
            },
        ));
    }
    for (name, base) in [("K", 4u16), ("mol", 5u16), ("cd", 6u16)] {
        list.push((
            name.to_string(),
            UnitDef {
                dim: Dim::base(base),
                num: 1.0,
                den: 1.0,
            },
        ));
    }
    list
}

/// Human-readable SI name of a dimension, e.g. `N`, `m/s^2`, `kg/m^3`.
pub fn dim_name(dim: &Dim, bases: &[String]) -> String {
    if dim.is_none() {
        return "Float".to_string();
    }
    let named: [(&str, Dim); 9] = [
        ("N", Dim::of(&[(0, 1), (1, 1), (2, -2)])),
        ("Pa", Dim::of(&[(0, -1), (1, 1), (2, -2)])),
        ("J", Dim::of(&[(0, 2), (1, 1), (2, -2)])),
        ("W", Dim::of(&[(0, 2), (1, 1), (2, -3)])),
        ("C", Dim::of(&[(2, 1), (3, 1)])),
        ("V", Dim::of(&[(0, 2), (1, 1), (2, -3), (3, -1)])),
        ("ohm", Dim::of(&[(0, 2), (1, 1), (2, -3), (3, -2)])),
        ("F", Dim::of(&[(0, -2), (1, -1), (2, 4), (3, 2)])),
        ("H", Dim::of(&[(0, 2), (1, 1), (2, -2), (3, -2)])),
    ];
    for (name, d) in named.iter() {
        if d == dim {
            return name.to_string();
        }
    }
    // Compose from base units; show kg before m for readability.
    let mut parts: Vec<(u16, i16, i16)> = dim.0.clone();
    parts.sort_by_key(|&(b, _, _)| match b {
        1 => 0u32,
        0 => 1u32,
        other => other as u32 + 1,
    });
    let base_name = |b: u16| -> String {
        bases
            .get(b as usize)
            .cloned()
            .unwrap_or_else(|| format!("unit{}", b))
    };
    let factor = |b: u16, n: i16, d: i16| -> String {
        let name = base_name(b);
        if d == 1 {
            if n == 1 {
                name
            } else {
                format!("{}^{}", name, n)
            }
        } else {
            format!("{}^({}/{})", name, n, d)
        }
    };
    let mut top: Vec<String> = Vec::new();
    let mut bottom: Vec<String> = Vec::new();
    for &(b, n, d) in &parts {
        if n > 0 {
            top.push(factor(b, n, d));
        } else {
            bottom.push(factor(b, -n, d));
        }
    }
    let mut s = if top.is_empty() {
        "1".to_string()
    } else {
        top.join("*")
    };
    for b in bottom {
        s.push('/');
        s.push_str(&b);
    }
    s
}

/// Edit distance (a swap of two neighbouring letters is one edit), used for "did you mean" hints.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev2: Vec<usize> = Vec::new();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            // Two neighbouring letters swapped (`pritn` for `print`) count as one edit.
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cur[j] = cur[j].min(prev2[j - 2] + 1);
            }
        }
        prev2 = prev;
        prev = cur;
    }
    prev[b.len()]
}

/// Pick the closest candidate name, if it is close enough to be a likely typo.
pub fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        if c.starts_with('$') || c == name {
            continue;
        }
        let d = edit_distance(&name.to_lowercase(), &c.to_lowercase());
        let limit = (name.chars().count().max(c.chars().count()) / 3).max(1);
        if d <= limit {
            match best {
                Some((bd, _)) if bd <= d => {}
                _ => best = Some((d, c)),
            }
        }
    }
    best.map(|(_, c)| c.to_string())
}
