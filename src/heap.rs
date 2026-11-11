//! Values, heap objects, the garbage collector and the insertion-ordered map.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};

pub const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Obj(u32),
    Fn(u32),
    /// An enum variant without a payload.
    Tag(u32),
    Uninit,
}

pub enum Trap {
    /// The memory limit was hit: collect garbage and retry the instruction.
    NeedGc,
    Error(String),
}

pub fn err<T>(msg: impl Into<String>) -> Result<T, Trap> {
    Err(Trap::Error(msg.into()))
}

#[cold]
pub fn bad() -> Trap {
    Trap::Error("internal: bad operand".to_string())
}

pub enum Obj {
    Str(Box<str>),
    List(Vec<Value>),
    Map(Box<KMap>),
    /// Type id (`NONE` for a tuple) and fields.
    Struct(u32, Box<[Value]>),
    Enum(u32, Box<[Value]>),
    Closure(u32, Box<[Value]>),
    Free(u32),
}

impl Obj {
    /// Estimated size in bytes, used for the memory limit.
    pub fn size(&self) -> usize {
        match self {
            Obj::Str(s) => 24 + s.len(),
            Obj::List(v) => 40 + 16 * v.len(),
            Obj::Map(m) => m.size(),
            Obj::Struct(_, f) | Obj::Enum(_, f) | Obj::Closure(_, f) => 32 + 16 * f.len(),
            Obj::Free(_) => 0,
        }
    }
}

// ---------------------------------------------------------------- hashing

/// Keys of a per-VM SipHash-1-3, so that map behaviour cannot be attacked with chosen keys.
#[derive(Clone, Copy)]
pub struct Seed(pub u64, pub u64);

pub struct Sip {
    v0: u64,
    v1: u64,
    v2: u64,
    v3: u64,
}

impl BuildHasher for Seed {
    type Hasher = Sip;
    fn build_hasher(&self) -> Sip {
        Sip {
            v0: self.0 ^ 0x736f6d6570736575,
            v1: self.1 ^ 0x646f72616e646f6d,
            v2: self.0 ^ 0x6c7967656e657261,
            v3: self.1 ^ 0x7465646279746573,
        }
    }
}

impl Sip {
    #[inline]
    fn round(&mut self) {
        self.v0 = self.v0.wrapping_add(self.v1);
        self.v1 = self.v1.rotate_left(13) ^ self.v0;
        self.v0 = self.v0.rotate_left(32);
        self.v2 = self.v2.wrapping_add(self.v3);
        self.v3 = self.v3.rotate_left(16) ^ self.v2;
        self.v0 = self.v0.wrapping_add(self.v3);
        self.v3 = self.v3.rotate_left(21) ^ self.v0;
        self.v2 = self.v2.wrapping_add(self.v1);
        self.v1 = self.v1.rotate_left(17) ^ self.v2;
        self.v2 = self.v2.rotate_left(32);
    }

    #[inline]
    fn word(&mut self, m: u64) {
        self.v3 ^= m;
        self.round();
        self.v0 ^= m;
    }
}

impl Hasher for Sip {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            self.word(u64::from_le_bytes(w));
        }
        let mut last = (bytes.len() as u64) << 56;
        for (i, b) in chunks.remainder().iter().enumerate() {
            last |= (*b as u64) << (8 * i);
        }
        self.word(last);
    }

    fn write_u8(&mut self, x: u8) {
        self.v1 ^= (x as u64) << 8;
    }

    fn write_u64(&mut self, x: u64) {
        self.word(x);
    }

    fn write_i64(&mut self, x: i64) {
        self.word(x as u64);
    }

    fn write_usize(&mut self, x: usize) {
        self.word(x as u64);
    }

    fn finish(&self) -> u64 {
        let mut s = Sip { v0: self.v0, v1: self.v1, v2: self.v2 ^ 0xff, v3: self.v3 };
        s.round();
        s.round();
        s.round();
        s.v0 ^ s.v1 ^ s.v2 ^ s.v3
    }
}

// ---------------------------------------------------------------- map

pub enum Key<'a> {
    Int(i64),
    Str(&'a str),
}

/// A hash map that remembers insertion order. Keys are Int or Str.
#[derive(Clone)]
pub struct KMap {
    entries: Vec<Option<(Value, Value)>>,
    ints: HashMap<i64, u32, Seed>,
    strs: HashMap<Box<str>, u32, Seed>,
    live: usize,
    key_bytes: usize,
}

impl KMap {
    pub fn new(seed: Seed) -> KMap {
        KMap {
            entries: Vec::new(),
            ints: HashMap::with_hasher(seed),
            strs: HashMap::with_hasher(seed),
            live: 0,
            key_bytes: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.live
    }

    pub fn size(&self) -> usize {
        64 + 80 * self.live + self.key_bytes
    }

    fn index(&self, k: &Key) -> Option<u32> {
        match k {
            Key::Int(i) => self.ints.get(i).copied(),
            Key::Str(s) => self.strs.get(*s).copied(),
        }
    }

    pub fn get(&self, k: &Key) -> Option<Value> {
        let i = self.index(k)?;
        self.entries[i as usize].map(|e| e.1)
    }

    /// Changes the value of a key that is already there; false if the key is absent.
    pub fn replace(&mut self, k: &Key, v: Value) -> bool {
        let Some(i) = self.index(k) else { return false };
        if let Some(e) = &mut self.entries[i as usize] {
            e.1 = v;
        }
        true
    }

    /// `key` is the key as a value (kept so that `keys` does not have to allocate).
    pub fn set(&mut self, k: Key, key: Value, v: Value) {
        if !self.replace(&k, v) {
            self.add(k, key, v);
        }
    }

    /// Adds a key that is known to be absent.
    fn add(&mut self, k: Key, key: Value, v: Value) {
        let i = self.entries.len() as u32;
        self.entries.push(Some((key, v)));
        self.live += 1;
        match k {
            Key::Int(n) => {
                self.ints.insert(n, i);
            }
            Key::Str(s) => {
                self.key_bytes += s.len();
                self.strs.insert(s.into(), i);
            }
        }
    }

    pub fn remove(&mut self, k: &Key) -> Option<Value> {
        let i = match k {
            Key::Int(n) => self.ints.remove(n)?,
            Key::Str(s) => {
                let i = self.strs.remove(*s)?;
                self.key_bytes -= s.len();
                i
            }
        };
        let old = self.entries[i as usize].take();
        self.live -= 1;
        if self.entries.len() > 16 && self.live * 2 < self.entries.len() {
            self.compact();
        }
        old.map(|e| e.1)
    }

    fn compact(&mut self) {
        let mut remap = vec![0u32; self.entries.len()];
        let mut next = 0u32;
        for (i, e) in self.entries.iter().enumerate() {
            if e.is_some() {
                remap[i] = next;
                next += 1;
            }
        }
        self.entries.retain(|e| e.is_some());
        for v in self.ints.values_mut() {
            *v = remap[*v as usize];
        }
        for v in self.strs.values_mut() {
            *v = remap[*v as usize];
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.ints.clear();
        self.strs.clear();
        self.live = 0;
        self.key_bytes = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = (Value, Value)> + '_ {
        self.entries.iter().flatten().copied()
    }
}

// ---------------------------------------------------------------- heap

const MIN_THRESHOLD: usize = 4 << 20;

pub struct Heap {
    pub objs: Vec<Obj>,
    free: u32,
    marks: Vec<bool>,
    /// Estimated bytes in use (recomputed by every collection).
    pub bytes: usize,
    pub limit: usize,
    /// Collect when `bytes` passes this.
    pub threshold: usize,
    /// Objects below this index are never collected (constants).
    pub permanent: u32,
    /// First of the 128 permanent one-character ASCII strings.
    ascii: u32,
    pub seed: Seed,
    pub collections: u32,
}

fn key_in(objs: &[Obj], v: Value) -> Result<Key<'_>, Trap> {
    match v {
        Value::Int(i) => Ok(Key::Int(i)),
        Value::Obj(i) => match objs.get(i as usize) {
            Some(Obj::Str(s)) => Ok(Key::Str(s)),
            _ => err("a map key must be an Int or a Str"),
        },
        _ => err("a map key must be an Int or a Str"),
    }
}

impl Heap {
    pub fn new(limit: usize, seed: Seed) -> Heap {
        Heap {
            objs: Vec::new(),
            free: NONE,
            marks: Vec::new(),
            bytes: 0,
            limit,
            threshold: MIN_THRESHOLD.min(limit),
            permanent: 0,
            ascii: 0,
            seed,
            collections: 0,
        }
    }

    /// Call once, after the constants are allocated.
    pub fn freeze(&mut self) {
        self.ascii = self.objs.len() as u32;
        for c in 0u8..128 {
            self.objs.push(Obj::Str((c as char).to_string().into()));
        }
        self.permanent = self.objs.len() as u32;
        self.bytes = self.objs.iter().map(|o| o.size()).sum();
    }

    /// Fails if `n` more bytes would not fit.
    #[inline]
    pub fn check(&self, n: usize) -> Result<(), Trap> {
        if self.bytes.saturating_add(n) > self.limit {
            return Err(Trap::NeedGc);
        }
        Ok(())
    }

    /// Account for `n` bytes added to an existing object.
    #[inline]
    pub fn charge(&mut self, n: usize) -> Result<(), Trap> {
        self.check(n)?;
        self.bytes += n;
        Ok(())
    }

    #[inline]
    pub fn alloc(&mut self, obj: Obj) -> Result<Value, Trap> {
        self.charge(obj.size())?;
        let i = self.free;
        if i != NONE {
            if let Obj::Free(next) = self.objs[i as usize] {
                self.free = next;
            }
            self.objs[i as usize] = obj;
            return Ok(Value::Obj(i));
        }
        if self.objs.len() >= (NONE - 1) as usize {
            return Err(Trap::NeedGc);
        }
        self.objs.push(obj);
        Ok(Value::Obj((self.objs.len() - 1) as u32))
    }

    pub fn str(&mut self, s: impl Into<Box<str>>) -> Result<Value, Trap> {
        self.alloc(Obj::Str(s.into()))
    }

    /// A one-character string (no allocation for ASCII).
    pub fn char_str(&mut self, c: char) -> Result<Value, Trap> {
        if c.is_ascii() {
            return Ok(Value::Obj(self.ascii + c as u32));
        }
        self.str(c.to_string())
    }

    pub fn list(&mut self, items: Vec<Value>) -> Result<Value, Trap> {
        self.alloc(Obj::List(items))
    }

    pub fn str_of(&self, v: Value) -> Result<&str, Trap> {
        match v {
            Value::Obj(i) => match self.objs.get(i as usize) {
                Some(Obj::Str(s)) => Ok(s),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        }
    }

    pub fn list_of(&self, v: Value) -> Result<&Vec<Value>, Trap> {
        match v {
            Value::Obj(i) => match self.objs.get(i as usize) {
                Some(Obj::List(items)) => Ok(items),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        }
    }

    pub fn list_mut(&mut self, v: Value) -> Result<&mut Vec<Value>, Trap> {
        match v {
            Value::Obj(i) => match self.objs.get_mut(i as usize) {
                Some(Obj::List(items)) => Ok(items),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        }
    }

    pub fn map_of(&self, v: Value) -> Result<&KMap, Trap> {
        match v {
            Value::Obj(i) => match self.objs.get(i as usize) {
                Some(Obj::Map(m)) => Ok(m),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        }
    }

    pub fn map_mut(&mut self, v: Value) -> Result<&mut KMap, Trap> {
        match v {
            Value::Obj(i) => match self.objs.get_mut(i as usize) {
                Some(Obj::Map(m)) => Ok(m),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        }
    }

    pub fn key(&self, v: Value) -> Result<Key<'_>, Trap> {
        key_in(&self.objs, v)
    }

    pub fn map_get(&self, m: Value, k: Value) -> Result<Option<Value>, Trap> {
        Ok(self.map_of(m)?.get(&self.key(k)?))
    }

    /// Run `f` on a map that is temporarily taken out of the heap, so that `f` can also read keys.
    fn with_map<T>(&mut self, m: Value, f: impl FnOnce(&mut KMap, &mut Heap) -> Result<T, Trap>) -> Result<T, Trap> {
        let Value::Obj(i) = m else { return Err(bad()) };
        let i = i as usize;
        if !matches!(self.objs.get(i), Some(Obj::Map(_))) {
            return Err(bad());
        }
        let Obj::Map(mut map) = std::mem::replace(&mut self.objs[i], Obj::Free(NONE)) else { return Err(bad()) };
        let result = f(&mut map, self);
        self.objs[i] = Obj::Map(map);
        result
    }

    pub fn map_set(&mut self, m: Value, k: Value, v: Value) -> Result<(), Trap> {
        self.with_map(m, |map, heap| {
            let key = key_in(&heap.objs, k)?;
            if map.replace(&key, v) {
                return Ok(());
            }
            let cost = match &key {
                Key::Str(s) => 80 + s.len(),
                Key::Int(_) => 80,
            };
            if heap.bytes.saturating_add(cost) > heap.limit {
                return Err(Trap::NeedGc);
            }
            heap.bytes += cost;
            map.add(key, k, v);
            Ok(())
        })
    }

    pub fn map_remove(&mut self, m: Value, k: Value) -> Result<Option<Value>, Trap> {
        self.with_map(m, |map, heap| Ok(map.remove(&key_in(&heap.objs, k)?)))
    }

    /// Mark from the roots and free everything else.
    pub fn collect(&mut self, stack: &[Value], globals: &[Value]) {
        let n = self.objs.len();
        let permanent = self.permanent as usize;
        self.marks.clear();
        self.marks.resize(n, false);
        let marks = &mut self.marks;
        let mut work: Vec<u32> = Vec::new();
        let visit = |v: Value, marks: &mut Vec<bool>, work: &mut Vec<u32>| {
            if let Value::Obj(i) = v {
                let i = i as usize;
                if i >= permanent && i < n && !marks[i] {
                    marks[i] = true;
                    work.push(i as u32);
                }
            }
        };
        for v in stack.iter().chain(globals) {
            visit(*v, marks, &mut work);
        }
        while let Some(i) = work.pop() {
            match &self.objs[i as usize] {
                Obj::List(items) => {
                    for v in items {
                        visit(*v, marks, &mut work);
                    }
                }
                Obj::Map(m) => {
                    for (k, v) in m.iter() {
                        visit(k, marks, &mut work);
                        visit(v, marks, &mut work);
                    }
                }
                Obj::Struct(_, f) | Obj::Enum(_, f) | Obj::Closure(_, f) => {
                    for v in f.iter() {
                        visit(*v, marks, &mut work);
                    }
                }
                Obj::Str(_) | Obj::Free(_) => {}
            }
        }
        let mut bytes = 0;
        for i in 0..n {
            if i < permanent || marks[i] {
                bytes += self.objs[i].size();
            } else if !matches!(self.objs[i], Obj::Free(_)) {
                self.objs[i] = Obj::Free(self.free);
                self.free = i as u32;
            }
        }
        self.bytes = bytes;
        self.threshold = bytes.saturating_mul(2).max(MIN_THRESHOLD);
        self.collections += 1;
    }
}
