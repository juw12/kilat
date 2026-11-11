//! Values, heap objects, the garbage collector and the insertion-ordered map.

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

struct Sip {
    v0: u64,
    v1: u64,
    v2: u64,
    v3: u64,
}

impl Seed {
    fn sip(self) -> Sip {
        Sip {
            v0: self.0 ^ 0x736f6d6570736575,
            v1: self.1 ^ 0x646f72616e646f6d,
            v2: self.0 ^ 0x6c7967656e657261,
            v3: self.1 ^ 0x7465646279746573,
        }
    }

    #[inline]
    pub fn hash_int(self, n: i64) -> u64 {
        let mut h = self.sip();
        h.word(n as u64);
        h.finish()
    }

    /// Never 0, so that 0 can mean "not computed yet" in the per-string cache.
    pub fn hash_str(self, s: &str) -> u64 {
        let mut h = self.sip();
        h.write(s.as_bytes());
        h.finish().max(1)
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

    #[inline]
    fn finish(&self) -> u64 {
        let mut s = Sip { v0: self.v0, v1: self.v1, v2: self.v2 ^ 0xff, v3: self.v3 };
        s.round();
        s.round();
        s.round();
        s.v0 ^ s.v1 ^ s.v2 ^ s.v3
    }
}

// ---------------------------------------------------------------- map

#[derive(Clone, Copy)]
struct Entry {
    hash: u64,
    /// `Uninit` marks a removed entry.
    key: Value,
    value: Value,
}

/// A hash map that remembers insertion order. Keys are Int or Str.
///
/// `entries` holds the pairs in insertion order. `slots` is an open-addressing table over them:
/// 0 means empty, anything else is an entry number plus one. The map keeps no copy of the text
/// of a string key: the key value is the heap string itself, and strings never change, so a
/// lookup compares against the heap. Callers pass the hash, which the heap remembers per string.
#[derive(Clone)]
pub struct KMap {
    entries: Vec<Entry>,
    slots: Vec<u32>,
    live: usize,
    key_bytes: usize,
}

#[inline]
fn same_key(objs: &[Obj], a: Value, b: Value) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Value::Obj(i), Value::Obj(j)) => match (objs.get(i as usize), objs.get(j as usize)) {
            (Some(Obj::Str(p)), Some(Obj::Str(q))) => p == q,
            _ => false,
        },
        _ => false,
    }
}

impl KMap {
    pub fn new() -> KMap {
        KMap { entries: Vec::new(), slots: Vec::new(), live: 0, key_bytes: 0 }
    }

    pub fn len(&self) -> usize {
        self.live
    }

    pub fn size(&self) -> usize {
        64 + 80 * self.live + self.key_bytes
    }

    /// The entry number of a key. Every probe ends: at most 3/4 of the slots are ever in use.
    #[inline]
    fn find(&self, objs: &[Obj], k: Value, hash: u64) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut s = hash as usize & mask;
        loop {
            let slot = self.slots[s];
            if slot == 0 {
                return None;
            }
            let i = slot as usize - 1;
            let e = &self.entries[i];
            if e.hash == hash && same_key(objs, e.key, k) {
                return Some(i);
            }
            s = (s + 1) & mask;
        }
    }

    #[inline]
    pub fn get(&self, objs: &[Obj], k: Value, hash: u64) -> Option<Value> {
        self.find(objs, k, hash).map(|i| self.entries[i].value)
    }

    /// Changes the value of a key that is already there; false if the key is absent.
    #[inline]
    pub fn replace(&mut self, objs: &[Obj], k: Value, hash: u64, v: Value) -> bool {
        let Some(i) = self.find(objs, k, hash) else { return false };
        self.entries[i].value = v;
        true
    }

    /// `key_len` is the length of the text of a string key (0 for an Int key).
    pub fn set(&mut self, objs: &[Obj], k: Value, hash: u64, key_len: usize, v: Value) {
        if !self.replace(objs, k, hash, v) {
            self.add(k, hash, key_len, v);
        }
    }

    /// Adds a key that is known to be absent.
    pub fn add(&mut self, k: Value, hash: u64, key_len: usize, v: Value) {
        if (self.entries.len() + 1) * 4 > self.slots.len() * 3 {
            self.rebuild();
        }
        let mask = self.slots.len() - 1;
        let mut s = hash as usize & mask;
        while self.slots[s] != 0 {
            s = (s + 1) & mask;
        }
        self.entries.push(Entry { hash, key: k, value: v });
        self.slots[s] = self.entries.len() as u32;
        self.live += 1;
        self.key_bytes += key_len;
    }

    pub fn remove(&mut self, objs: &[Obj], k: Value, hash: u64, key_len: usize) -> Option<Value> {
        let i = self.find(objs, k, hash)?;
        let old = self.entries[i].value;
        self.entries[i].key = Value::Uninit;
        self.entries[i].value = Value::Unit;
        self.live -= 1;
        self.key_bytes = self.key_bytes.saturating_sub(key_len);
        if self.entries.len() > 16 && self.live * 2 < self.entries.len() {
            self.rebuild();
        }
        Some(old)
    }

    /// Drops removed entries and lays the slot table out again, at most half full, so that the
    /// next rebuild is a number of insertions away that grows with the map.
    fn rebuild(&mut self) {
        self.entries.retain(|e| !matches!(e.key, Value::Uninit));
        let need = self.entries.len() + 1;
        let mut n = 8;
        while need * 2 > n {
            n *= 2;
        }
        self.slots.clear();
        self.slots.resize(n, 0);
        let mask = n - 1;
        for (i, e) in self.entries.iter().enumerate() {
            let mut s = e.hash as usize & mask;
            while self.slots[s] != 0 {
                s = (s + 1) & mask;
            }
            self.slots[s] = i as u32 + 1;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.slots.clear();
        self.live = 0;
        self.key_bytes = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = (Value, Value)> + '_ {
        self.entries.iter().filter(|e| !matches!(e.key, Value::Uninit)).map(|e| (e.key, e.value))
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
    /// Remembered hash of each string that was used as a map key (0 = not computed).
    /// Parallel to `objs`, grown on demand; strings never change, so an entry stays valid
    /// until the collector frees its object.
    str_hash: Vec<u64>,
    pub collections: u32,
}

const KEY_KIND: &str = "a map key must be an Int or a Str";

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
            str_hash: Vec::new(),
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

    /// A hint that `n` more objects are about to be made; it never fails.
    pub fn reserve(&mut self, n: usize) {
        if self.free == NONE {
            self.objs.reserve(n);
        }
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

    /// The hash of a map key. A string's hash is computed once and remembered.
    #[inline]
    pub fn hash_of(&mut self, k: Value) -> Result<u64, Trap> {
        match k {
            Value::Int(n) => Ok(self.seed.hash_int(n)),
            Value::Obj(i) => {
                let i = i as usize;
                if let Some(&h) = self.str_hash.get(i) {
                    if h != 0 {
                        return Ok(h);
                    }
                }
                let Some(Obj::Str(s)) = self.objs.get(i) else { return err(KEY_KIND) };
                let h = self.seed.hash_str(s);
                if self.str_hash.len() <= i {
                    self.str_hash.resize(self.objs.len(), 0);
                }
                self.str_hash[i] = h;
                Ok(h)
            }
            _ => err(KEY_KIND),
        }
    }

    /// The same hash without remembering it, for code that only has a shared reference.
    pub fn hash_key(&self, k: Value) -> Result<u64, Trap> {
        match k {
            Value::Int(n) => Ok(self.seed.hash_int(n)),
            Value::Obj(i) => match self.objs.get(i as usize) {
                Some(Obj::Str(s)) => Ok(self.seed.hash_str(s)),
                _ => err(KEY_KIND),
            },
            _ => err(KEY_KIND),
        }
    }

    /// Bytes of text in a string key (0 for an Int key), for the memory accounting.
    #[inline]
    pub fn key_len(&self, k: Value) -> usize {
        match k {
            Value::Obj(i) => match self.objs.get(i as usize) {
                Some(Obj::Str(s)) => s.len(),
                _ => 0,
            },
            _ => 0,
        }
    }

    #[inline]
    pub fn map_get(&mut self, m: Value, k: Value) -> Result<Option<Value>, Trap> {
        let hash = self.hash_of(k)?;
        Ok(self.map_of(m)?.get(&self.objs, k, hash))
    }

    #[inline]
    pub fn map_set(&mut self, m: Value, k: Value, v: Value) -> Result<(), Trap> {
        let hash = self.hash_of(k)?;
        if let Some(i) = self.map_of(m)?.find(&self.objs, k, hash) {
            self.map_mut(m)?.entries[i].value = v;
            return Ok(());
        }
        let key_len = self.key_len(k);
        self.charge(80 + key_len)?;
        self.map_mut(m)?.add(k, hash, key_len, v);
        Ok(())
    }

    pub fn map_remove(&mut self, m: Value, k: Value) -> Result<Option<Value>, Trap> {
        let hash = self.hash_of(k)?;
        let key_len = self.key_len(k);
        let Value::Obj(i) = m else { return Err(bad()) };
        let i = i as usize;
        // The map is taken out of the heap for a moment so that it can be changed while the
        // heap's strings are read.
        let Some(Obj::Map(_)) = self.objs.get(i) else { return Err(bad()) };
        let Obj::Map(mut map) = std::mem::replace(&mut self.objs[i], Obj::Free(NONE)) else { return Err(bad()) };
        let old = map.remove(&self.objs, k, hash, key_len);
        self.objs[i] = Obj::Map(map);
        Ok(old)
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
                if let Some(h) = self.str_hash.get_mut(i) {
                    *h = 0;
                }
            }
        }
        self.bytes = bytes;
        self.threshold = bytes.saturating_mul(2).max(MIN_THRESHOLD);
        self.collections += 1;
    }
}
