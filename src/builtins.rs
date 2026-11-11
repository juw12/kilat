//! The table of native functions: enum id, Kilat name and signature.
//! Names starting with `$` are internal and can only be used by the prelude.

macro_rules! builtins {
    ($( $variant:ident, $name:expr, $sig:expr; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u16)]
        pub enum B { $( $variant, )* }

        /// (name, signature) in the same order as the enum.
        pub const BUILTINS: &[(&str, &str)] = &[ $( ($name, $sig), )* ];

        pub const ALL: &[B] = &[ $( B::$variant, )* ];
    };
}

builtins! {
    // ---- math
    Abs, "abs", "[T: Num](T) -> T";
    Min2, "min", "[T: Ord](T, T) -> T";
    MinL, "min", "[T: Ord](List[T]) -> T";
    Max2, "max", "[T: Ord](T, T) -> T";
    MaxL, "max", "[T: Ord](List[T]) -> T";
    Clamp, "clamp", "[T: Ord](T, T, T) -> T";
    Floor, "floor", "(Float) -> Int";
    Ceil, "ceil", "(Float) -> Int";
    Round, "round", "(Float) -> Int";
    Trunc, "trunc", "(Float) -> Int";
    RoundTo, "round_to", "(Float, Int) -> Float";
    Sin, "sin", "(Float) -> Float";
    Cos, "cos", "(Float) -> Float";
    Tan, "tan", "(Float) -> Float";
    Asin, "asin", "(Float) -> Float";
    Acos, "acos", "(Float) -> Float";
    Atan, "atan", "(Float) -> Float";
    Exp, "exp", "(Float) -> Float";
    Ln, "ln", "(Float) -> Float";
    Log10, "log10", "(Float) -> Float";
    Log2, "log2", "(Float) -> Float";
    Atan2, "atan2", "[T: Num](T, T) -> Float";
    ToFloat, "float", "(Int) -> Float";
    ToInt, "int", "(Float) -> Int";
    IsNan, "is_nan", "(Float) -> Bool";
    Approx, "approx", "[T: Num](T, T, T) -> Bool";
    Rand, "rand", "() -> Float";
    RandInt, "rand_int", "(Int, Int) -> Int";
    RandSeed, "rand_seed", "(Int)";
    // ---- strings
    LenS, "len", "(Str) -> Int";
    Upper, "upper", "(Str) -> Str";
    Lower, "lower", "(Str) -> Str";
    Trim, "trim", "(Str) -> Str";
    Split, "split", "(Str, Str) -> List[Str]";
    Lines, "lines", "(Str) -> List[Str]";
    Chars, "chars", "(Str) -> List[Str]";
    Join, "join", "(List[Str], Str) -> Str";
    ContainsS, "contains", "(Str, Str) -> Bool";
    StartsWith, "starts_with", "(Str, Str) -> Bool";
    EndsWith, "ends_with", "(Str, Str) -> Bool";
    Replace, "replace", "(Str, Str, Str) -> Str";
    FindS, "find", "(Str, Str) -> Int?";
    SliceS, "slice", "(Str, Int, Int) -> Str";
    Repeat, "repeat", "(Str, Int) -> Str";
    ToIntS, "to_int", "(Str) -> Int?";
    ToFloatS, "to_float", "(Str) -> Float?";
    PadLeft, "pad_left", "(Str, Int) -> Str";
    PadRight, "pad_right", "(Str, Int) -> Str";
    CharCode, "char_code", "(Str) -> Int";
    FromCharCode, "from_char_code", "(Int) -> Str";
    IsEmptyS, "is_empty", "(Str) -> Bool";
    // ---- lists
    LenL, "len", "[T](List[T]) -> Int";
    Push, "push", "[T](List[T], T)";
    Pop, "pop", "[T](List[T]) -> T?";
    Insert, "insert", "[T](List[T], Int, T)";
    RemoveAt, "remove_at", "[T](List[T], Int) -> T";
    First, "first", "[T](List[T]) -> T?";
    Last, "last", "[T](List[T]) -> T?";
    ContainsL, "contains", "[T](List[T], T) -> Bool";
    IndexOf, "index_of", "[T](List[T], T) -> Int?";
    SliceL, "slice", "[T](List[T], Int, Int) -> List[T]";
    Reverse, "reverse", "[T](List[T]) -> List[T]";
    Sort, "sort", "[T: Ord](List[T]) -> List[T]";
    ConcatL, "concat", "[T](List[T], List[T]) -> List[T]";
    Range1, "range", "(Int) -> List[Int]";
    Range2, "range", "(Int, Int) -> List[Int]";
    Range3, "range", "(Int, Int, Int) -> List[Int]";
    Fill, "fill", "[T](Int, T) -> List[T]";
    ClearL, "clear", "[T](List[T])";
    CopyL, "copy", "[T](List[T]) -> List[T]";
    IsEmptyL, "is_empty", "[T](List[T]) -> Bool";
    Extend, "extend", "[T](List[T], List[T])";
    SortKeys, "$sort_keys", "[T, K: Ord](List[T], List[K]) -> List[T]";
    // ---- maps
    LenM, "len", "[K, V](Map[K, V]) -> Int";
    GetM, "get", "[K, V](Map[K, V], K) -> V?";
    GetOr, "get_or", "[K, V](Map[K, V], K, V) -> V";
    Has, "has", "[K, V](Map[K, V], K) -> Bool";
    RemoveM, "remove", "[K, V](Map[K, V], K) -> V?";
    Keys, "keys", "[K, V](Map[K, V]) -> List[K]";
    Values, "values", "[K, V](Map[K, V]) -> List[V]";
    Items, "items", "[K, V](Map[K, V]) -> List[(K, V)]";
    ClearM, "clear", "[K, V](Map[K, V])";
    IsEmptyM, "is_empty", "[K, V](Map[K, V]) -> Bool";
    CopyM, "copy", "[K, V](Map[K, V]) -> Map[K, V]";
    // ---- json
    JsonParse, "json_parse", "(Str) -> Result[Json]";
    JsonStr, "json_str", "(Json) -> Str";
    JsonPretty, "json_pretty", "(Json) -> Str";
    // ---- internal
    Print, "$print", "(Str)";
    Fmt, "$fmt", "[T](T, Int, Int, Int) -> Str";
    Panic, "$panic", "(Str) -> Never";
    Assert, "$assert", "(Bool, Str)";
    AssertEq, "$assert_eq", "[T](T, T)";
    Sqrt, "$sqrt", "(Float) -> Float";
    SumI, "$sum_i", "(List[Int]) -> Int";
    SumF, "$sum_f", "(List[Float]) -> Float";
    Mean, "$mean", "(List[Float]) -> Float";
    Median, "$median", "(List[Float]) -> Float";
    Stddev, "$stddev", "(List[Float]) -> Float";
}
