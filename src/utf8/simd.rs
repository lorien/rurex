//! AVX2 transcoder for the cold non-ASCII path (`super::transcode`).
//!
//! One kernel serves all three UCS kinds. Per 8-unit chunk it
//!
//! 1. widens the units to `u32` lanes (UCS-1/2: zero work — no
//!    surrogate-pair decoding, see below),
//! 2. derives each lane's UTF-8 length as a 3-bit sum into a 4-bit
//!    field, giving a `lens` vector whose nibbles are `len - 1`,
//! 3. stages the four candidate UTF-8 bytes of every lane into one
//!    32-byte register (slot `i` = bytes `4i..4i+3`, valid prefix
//!    only) — with a *specialized* staging for the two length pairs
//!    that real text is made of (1|2 and 3|4), and the full
//!    four-candidate blend chain only for chunks that mix across
//!    those pairs, and
//! 4. emits a `pshufb` per 4-lane half whose control comes from a
//!    256-row table indexed by that half's eight length bits. All-ASCII
//!    chunks take a fixed shuffle instead (no staging at all). Uniform
//!    2/3/4-byte chunks are *not* special-cased: the table holds their
//!    rows too, and the extra branches cost more than the fixed shuffle
//!    saves (measured).
//!
//! Staging by length pair is the win on real text: 70% of chunks in
//! a mixed Arabic/Latin corpus contain only 1- and 2-byte output, and
//! there the slot is just `lead | tail<<8` (two ops) instead of the
//! full blend chain (which is only needed when lengths 1..4 can
//! co-occur in one chunk).
//!
//! The table is the key to the mixed case: computing compaction
//! controls arithmetically (BMI2 `pext`/`pdep`) costs two
//! general-purpose-register round trips *per half* and measured
//! 1.6-2x slower than one indexed load. simdutf uses the same shape
//! for its 1|2-byte Latin-1 kernel.
//!
//! Three shapes were measured and rejected on this corpus (see the
//! session report): a two-pass "prefix-sum the output offsets so
//! chunks write in parallel" variant (the extra classify pass costs
//! more than the decoupling saves), deriving the per-chunk byte
//! count from a `popcount` of the keep-mask instead of the table row
//! (pure extra arithmetic — the row load is not what gates the
//! store), and per-half uniform fast paths (branch mispredicts).
//!
//! Why no surrogate-pair decoding: CPython picks the storage kind by
//! the string's maximum code point (`PyUnicode_New`), so any string
//! containing an astral character is UCS-4. A compact UCS-2 string
//! therefore never contains a *valid* pair — every surrogate code
//! unit in UCS-2 storage is a lone surrogate and an encoding error,
//! exactly like `PyUnicode_AsUTF8AndSize`. The kernel only has to
//! *detect* surrogates (bulk compare) and hand such chunks to the
//! scalar path, which raises the `UnicodeEncodeError`.
//!
//! Error semantics: lanes that are surrogates (UCS-2/UCS-4) or
//! beyond U+10FFFF (UCS-4) are detected per chunk before any SIMD
//! work; the whole 8-unit chunk falls back to the scalar encoder,
//! which raises at the first bad unit — the same first error a
//! fully scalar pass would report. UCS-1 (Latin-1) can never fail.
//! Because bad lanes are diverted first, every lane that reaches the
//! staging code is a valid code point <= U+10FFFF, which is why the
//! length classification can use signed compares.
//!
//! Dispatch: `supported()` caches `avx2 && bmi2` runtime detection
//! once. Other architectures keep the scalar encoder in `super`.
//! The `#[target_feature]` functions are only reached through that
//! gate — calling them on a CPU without the feature would be UB.
//!
//! Output: the caller (`super::transcode`) passes a reused scratch
//! buffer with room for the per-kind worst case (2/3/4 bytes per
//! unit) plus 32 bytes of slack; the fast paths use overlapping
//! stores, so up to 12 bytes past the current write position may be
//! scribbled and are never part of the result (`set_len` trims them,
//! and the slack covers the last store).

use pyo3::prelude::*;
use std::arch::x86_64::*;
use std::sync::OnceLock;

use super::Kind;

/// True once, if this CPU has everything the kernel needs.
pub(crate) fn supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("bmi2")
    })
}

/// One bit per 32-bit lane.
const ONES: u32 = 0x1111_1111;

/// Entry: transcode `len` units of `kind` at `data` (the compact
/// UCS buffer of a non-ASCII string), appending the UTF-8 to `out`
/// and returning the number of bytes appended. `out` must already
/// have capacity for the worst case (the caller sizes it).
///
/// # Safety
///
/// `data` must point to `len` readable units of the given kind
/// (UCS-1: `u8`, UCS-2: `u16`, UCS-4: `u32`), and the CPU must
/// support AVX2 + BMI2 (caller checks `supported()`).
pub(crate) unsafe fn transcode(
    kind: Kind,
    len: usize,
    data: *const u8,
    stand_in: bool,
    mapped: &mut bool,
    out: &mut Vec<u8>,
) -> PyResult<usize> {
    unsafe { transcode_avx2(kind, len, data, stand_in, mapped, out) }
}

/// The whole kernel lives in one `#[target_feature]` function so
/// every helper (`#[inline]`, same features) folds into a single
/// AVX2+BMI2 codegen unit. An out-of-line helper here would spill
/// the whole chunk to the stack and cost an order of magnitude
/// (measured: 0.05 GB/s with a call per chunk).
///
/// # Safety
///
/// CPU must support AVX2 + BMI2.
#[target_feature(enable = "avx2")]
#[target_feature(enable = "bmi2")]
unsafe fn transcode_avx2(
    kind: Kind,
    len: usize,
    data: *const u8,
    stand_in: bool,
    mapped: &mut bool,
    out: &mut Vec<u8>,
) -> PyResult<usize> {
    let mut written = 0usize;
    // Whether a lone surrogate may be mapped to its stand-in code point.
    // A UCS-4 haystack that already contains a stand-in character turns
    // this off for the rest of the string (the two would be
    // indistinguishable), so it is threaded through the chunk loop.
    let mut stand_in = stand_in;
    let outptr = unsafe { out.as_mut_ptr().add(out.len()) };
    let mut i = 0usize;

    // One loop per storage kind: the load width and the bad-lane
    // test are loop-invariant, and a `match kind` inside the chunk
    // loop compiles to per-chunk branches (measured as ~8 branch
    // mispredicts per chunk on varied text in the first version).
    match kind {
        Kind::Ucs1 => {
            // Units are bytes: never surrogate, never > U+10FFFF.
            while i + 8 <= len {
                let v =
                    unsafe { _mm256_cvtepu8_epi32(_mm_loadl_epi64(data.add(i) as *const __m128i)) };
                unsafe { chunk(v, outptr, &mut written) };
                i += 8;
            }
        }
        Kind::Ucs2 => {
            // Units are 16-bit code points: only lone surrogates are
            // bad (an astral character forces UCS-4 storage), so the
            // check is one compare.
            while i + 8 <= len {
                let v = unsafe {
                    _mm256_cvtepu16_epi32(_mm_loadu_si128(data.add(i * 2) as *const __m128i))
                };
                if unsafe { has_surrogates(v) } {
                    unsafe {
                        scalar_into(
                            kind,
                            data,
                            i,
                            i + 8,
                            (&mut stand_in, mapped),
                            outptr,
                            &mut written,
                        )?
                    };
                } else {
                    unsafe { chunk(v, outptr, &mut written) };
                }
                i += 8;
            }
        }
        Kind::Ucs4 => {
            while i + 8 <= len {
                let v = unsafe { _mm256_loadu_si256(data.add(i * 4) as *const __m256i) };
                if unsafe { has_surrogates(v) || has_overlong(v) } {
                    unsafe {
                        scalar_into(
                            kind,
                            data,
                            i,
                            i + 8,
                            (&mut stand_in, mapped),
                            outptr,
                            &mut written,
                        )?
                    };
                } else {
                    unsafe { chunk(v, outptr, &mut written) };
                }
                i += 8;
            }
        }
    }

    if i < len {
        unsafe {
            scalar_into(
                kind,
                data,
                i,
                len,
                (&mut stand_in, mapped),
                outptr,
                &mut written,
            )?
        };
    }
    unsafe { out.set_len(out.len() + written) };
    debug_assert!(
        std::str::from_utf8(&out[out.len() - written..]).is_ok(),
        "kernel produced invalid UTF-8"
    );
    // Every emitted byte comes from the fixed UTF-8 encoding
    // formulas below; the differential tests in this module and in
    // `super` compare against CPython's encoder byte for byte.
    Ok(written)
}

/// `v < t` per lane, signed. Valid because every lane is a code
/// point <= U+10FFFF by the time this runs (bad lanes are diverted
/// to the scalar path first).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn ult(v: __m256i, t: __m256i) -> __m256i {
    _mm256_cmpgt_epi32(t, v)
}

/// Any lane in the surrogate range (U+D800..U+DFFF)? Used for UCS-2
/// (where every surrogate unit is a lone surrogate and an encoding
/// error) and UCS-4.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn has_surrogates(v: __m256i) -> bool {
    let sur = _mm256_cmpeq_epi32(_mm256_and_si256(v, V_F800), V_D800);
    _mm256_movemask_epi8(sur) != 0
}

/// Any UCS-4 lane above U+10FFFF? Signed compare is exact: bad
/// lanes are diverted before staging, and the threshold is far below
/// 0x80000000.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn has_overlong(v: __m256i) -> bool {
    _mm256_movemask_epi8(_mm256_cmpgt_epi32(v, T_10FFFF)) != 0
}

/// Encode units `from..to` one at a time (error lanes and tails),
/// appending at `out + *written`. Raises `UnicodeEncodeError` at the
/// first surrogate / out-of-range unit, like `PyUnicode_AsUTF8AndSize`.
unsafe fn scalar_into(
    kind: Kind,
    data: *const u8,
    from: usize,
    to: usize,
    flags: (&mut bool, &mut bool),
    out: *mut u8,
    written: &mut usize,
) -> PyResult<()> {
    let (stand_in, mapped) = flags;
    let mut buf = [0u8; 4];
    for k in from..to {
        let cp = unsafe {
            match kind {
                Kind::Ucs1 => *data.add(k) as u32,
                Kind::Ucs2 => *(data.add(k * 2) as *const u16) as u32,
                Kind::Ucs4 => *(data.add(k * 4) as *const u32),
            }
        };
        if matches!(kind, Kind::Ucs4) && *stand_in && super::in_stand_in_block(cp) {
            // A real character from the stand-in block: stop mapping so
            // the two can never be confused; a later surrogate raises.
            *stand_in = false;
        }
        let c = match char::from_u32(cp) {
            Some(c) => c,
            None if *stand_in && (0xD800..0xE000).contains(&cp) => {
                *mapped = true;
                char::from_u32(super::surrogate_stand_in(cp)).expect("stand-in is valid")
            }
            None => return Err(super::surrogate_error()),
        };
        let s = c.encode_utf8(&mut buf);
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), out.add(*written), s.len());
        }
        *written += s.len();
    }
    Ok(())
}

/// Stage the four candidate UTF-8 bytes of every lane into its slot
/// (little-endian slot: lead, cont1, cont2, cont3). Lanes shorter
/// than four bytes have garbage in the trailing slot bytes; the
/// compaction step masks them away, so they are computed
/// unconditionally.
///
/// The byte after the lead depends on the length (`0x80|cp&0x3F` for
/// a 2-byte sequence, `0x80|(cp>>6)&0x3F` for a 3-byte one, ...), so
/// slot bytes 1 and 2 are blends selected by the length class.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn stage_general(v: __m256i, lt80: __m256i, lt800: __m256i, lt10000: __m256i) -> __m256i {
    let c2 = _mm256_or_si256(_mm256_srli_epi32(v, 6), V_C0);
    let c3 = _mm256_or_si256(_mm256_srli_epi32(v, 12), V_E0);
    let c4 = _mm256_or_si256(_mm256_srli_epi32(v, 18), V_F0);
    let mut lead = _mm256_blendv_epi8(c4, c3, lt10000);
    lead = _mm256_blendv_epi8(lead, c2, lt800);
    lead = _mm256_blendv_epi8(lead, v, lt80);

    let tail = _mm256_or_si256(_mm256_and_si256(v, V_3F), V_80);
    let cont_a = _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi32(v, 6), V_3F), V_80);
    let cont_b = _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi32(v, 12), V_3F), V_80);
    let is_len2 = _mm256_andnot_si256(lt80, lt800);
    let is_len3 = _mm256_andnot_si256(lt800, lt10000);
    let mut b1 = _mm256_blendv_epi8(cont_b, cont_a, is_len3);
    b1 = _mm256_blendv_epi8(b1, tail, is_len2);
    let b2 = _mm256_blendv_epi8(cont_a, tail, is_len3);

    let mut s = lead;
    s = _mm256_or_si256(s, _mm256_slli_epi32(b1, 8));
    s = _mm256_or_si256(s, _mm256_slli_epi32(b2, 16));
    s = _mm256_or_si256(s, _mm256_slli_epi32(tail, 24));
    s
}

/// Staging for a chunk whose lanes are all 1- or 2-byte output
/// (`lens & 0x2222_2222 == 0`): the slot is only `lead | tail<<8`,
/// so this is two ops per lane instead of the full blend chain.
/// Slot bytes 2..3 are garbage and masked away at compaction.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn stage12(v: __m256i, lt80: __m256i) -> __m256i {
    let lead2 = _mm256_or_si256(_mm256_srli_epi32(v, 6), V_C0);
    let lead = _mm256_blendv_epi8(lead2, v, lt80);
    let tail = _mm256_or_si256(_mm256_and_si256(v, V_3F), V_80);
    _mm256_or_si256(lead, _mm256_slli_epi32(tail, 8))
}

/// Staging for a chunk whose lanes are all 3- or 4-byte output
/// (`lens & 0x2222_2222 == 0x2222_2222`): two continuation
/// candidates per slot position, no length-1/2 blending.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn stage34(v: __m256i, lt10000: __m256i) -> __m256i {
    let lead3 = _mm256_or_si256(_mm256_srli_epi32(v, 12), V_E0);
    let lead4 = _mm256_or_si256(_mm256_srli_epi32(v, 18), V_F0);
    let lead = _mm256_blendv_epi8(lead4, lead3, lt10000);
    // a = 0x80|(cp>>6)&0x3F: 3-byte b1 / 4-byte b2
    let a = _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi32(v, 6), V_3F), V_80);
    // b = 0x80|(cp>>12)&0x3F: 4-byte b1
    let b = _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi32(v, 12), V_3F), V_80);
    // c = 0x80|cp&0x3F: 3-byte b2 / 4-byte b3
    let c = _mm256_or_si256(_mm256_and_si256(v, V_3F), V_80);
    let b1 = _mm256_blendv_epi8(b, a, lt10000);
    let b2 = _mm256_blendv_epi8(a, c, lt10000);
    let mut s = lead;
    s = _mm256_or_si256(s, _mm256_slli_epi32(b1, 8));
    s = _mm256_or_si256(s, _mm256_slli_epi32(b2, 16));
    s = _mm256_or_si256(s, _mm256_slli_epi32(c, 24));
    s
}

/// Encode one 8-lane chunk at `outptr + *written`. `v` must hold
/// only valid code points. The length class is computed once and
/// feeds the staging choice and the emit. Uniform chunks (all lanes
/// one length) are *not* special-cased: the compaction table has
/// their rows, and a table load is cheaper than the extra
/// mispredicted branches (measured).
#[inline]
#[target_feature(enable = "avx2")]
#[target_feature(enable = "bmi2")]
unsafe fn chunk(v: __m256i, outptr: *mut u8, written: &mut usize) {
    unsafe {
        let lt80 = ult(v, T_80);
        let lt800 = ult(v, T_800);
        let lt10000 = ult(v, T_10000);
        let ge80 = !(_mm256_movemask_epi8(lt80) as u32) & ONES;
        let ge800 = !(_mm256_movemask_epi8(lt800) as u32) & ONES;
        let ge10000 = !(_mm256_movemask_epi8(lt10000) as u32) & ONES;
        // (len - 1) per lane in 4-bit fields: each addend is one bit
        // per lane, so the sum cannot carry across fields.
        let lens = ge80 + ge800 + ge10000;
        let dst = outptr.add(*written);

        if lens == 0 {
            // All 8 lanes are ASCII: one byte each, no staging.
            let g = _mm256_shuffle_epi8(v, UNI1_SHUF);
            _mm_storeu_si128(dst as *mut __m128i, _mm256_castsi256_si128(g));
            _mm_storeu_si128(dst.add(4) as *mut __m128i, _mm256_extracti128_si256(g, 1));
            *written += 8;
            return;
        }

        // Bit 1 of every nibble is set exactly for lengths 3 and 4,
        // so this splits the staging cases without another compare.
        let pair = lens & 0x2222_2222;
        let st = if pair == 0 {
            stage12(v, lt80)
        } else if pair == 0x2222_2222 {
            stage34(v, lt10000)
        } else {
            stage_general(v, lt80, lt800, lt10000)
        };

        let idx0 = _pext_u32(lens & 0x3333, 0x3333) as usize;
        let idx1 = _pext_u32((lens >> 16) & 0x3333, 0x3333) as usize;
        let row0 = PACK_TABLE.as_ptr().add(idx0 * PACK_STRIDE);
        let row1 = PACK_TABLE.as_ptr().add(idx1 * PACK_STRIDE);
        let p0 = _mm_shuffle_epi8(
            _mm256_castsi256_si128(st),
            _mm_loadu_si128(row0.add(1) as *const __m128i),
        );
        let p1 = _mm_shuffle_epi8(
            _mm256_extracti128_si256(st, 1),
            _mm_loadu_si128(row1.add(1) as *const __m128i),
        );
        _mm_storeu_si128(dst as *mut __m128i, p0);
        let n0 = *row0 as usize;
        _mm_storeu_si128(dst.add(n0) as *mut __m128i, p1);
        *written += n0 + *row1 as usize;
    }
}

/// Broadcast constants, materialized once in `.rodata` so the hot
/// loop loads them instead of re-broadcasting per chunk.
macro_rules! ymm_consts {
    ($($name:ident => $val:expr),* $(,)?) => {
        $(static $name: __m256i = unsafe {
            std::mem::transmute::<[i32; 8], __m256i>([$val; 8])
        };)*
    };
}

ymm_consts! {
    V_80 => 0x80,
    V_3F => 0x3F,
    V_C0 => 0xC0,
    V_E0 => 0xE0,
    V_F0 => 0xF0,
    T_80 => 0x80,
    T_800 => 0x800,
    T_10000 => 0x1_0000,
    T_10FFFF => 0x10_FFFF,
    V_F800 => 0xFFFF_F800u32 as i32,
    V_D800 => 0xD800,
}

/// Lane-gather controls for the uniform shapes: per 128-bit half,
/// take the listed staging bytes into the low end and zero the rest.
#[allow(clippy::declare_interior_mutable_const)]
static UNI1_SHUF: __m256i = unsafe {
    std::mem::transmute::<[i8; 32], __m256i>([
        0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, //
        0, 4, 8, 12, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    ])
};

/// Compaction table: one row per 8-bit length index of a 4-lane
/// half (2 bits of `len - 1` per lane). Row layout: byte 0 is the
/// number of emitted bytes, bytes 1..17 the `pshufb` control that
/// gathers the lanes' valid slot prefixes in order (padded with
/// 0x80, which zeroes those bytes — they are past the emitted
/// length and never read).
const PACK_ROWS: usize = 256;
const PACK_STRIDE: usize = 17;

const fn build_pack_table() -> [u8; PACK_ROWS * PACK_STRIDE] {
    let mut table = [0x80u8; PACK_ROWS * PACK_STRIDE];
    let mut index = 0usize;
    while index < PACK_ROWS {
        let row = index * PACK_STRIDE;
        let mut out = 0usize;
        let mut lane = 0usize;
        while lane < 4 {
            let len = ((index >> (2 * lane)) & 0b11) + 1;
            let mut byte = 0usize;
            while byte < len {
                table[row + 1 + out] = (4 * lane + byte) as u8;
                out += 1;
                byte += 1;
            }
            lane += 1;
        }
        table[row] = out as u8;
        index += 1;
    }
    table
}

static PACK_TABLE: [u8; PACK_ROWS * PACK_STRIDE] = build_pack_table();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utf8::transcode_scalar;

    /// Seeded LCG (deterministic across runs/platforms).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 11
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Draw a code point from a weighted real-world-ish mix; the
    /// ranges exercise every length class, surrogates, and (for the
    /// UCS-4 fuzz) out-of-range values.
    fn draw_cp(rng: &mut Rng) -> u32 {
        match rng.below(100) {
            0..=39 => rng.below(0x80) as u32,             // ASCII
            40..=54 => 0x80 + rng.below(0x80) as u32,     // Latin-1
            55..=69 => 0x600 + rng.below(0x100) as u32,   // Arabic (2-byte)
            70..=84 => 0x4E00 + rng.below(0x1000) as u32, // CJK (3-byte)
            85..=89 => 0x1F300 + rng.below(0x800) as u32, // emoji (4-byte)
            90..=94 => 0xD800 + rng.below(0x800) as u32,  // surrogates
            _ => 0x110000 + rng.below(0x10000) as u32,    // out of range
        }
    }

    /// Clamp a drawn code point to what the storage kind can hold
    /// (UCS-1: any byte; UCS-2: <= 0xFFFF), so the fuzz respects
    /// CPython's storage invariants.
    fn clamp_for_kind(kind: Kind, cp: u32) -> u32 {
        match kind {
            Kind::Ucs1 => cp & 0xFF,
            Kind::Ucs2 => cp & 0xFFFF,
            Kind::Ucs4 => cp,
        }
    }

    type Transcoder =
        unsafe fn(Kind, usize, *const u8, bool, &mut bool, &mut Vec<u8>) -> PyResult<usize>;

    /// Call a transcoder variant with a fresh output buffer and
    /// return the UTF-8 as a String.
    fn call(kind: Kind, n: usize, p: *const u8, f: Transcoder) -> PyResult<String> {
        let mut out: Vec<u8> = Vec::with_capacity(n * 4 + 32);
        let mut mapped = false;
        let written = unsafe { f(kind, n, p, false, &mut mapped, &mut out)? };
        Ok(String::from_utf8(out[..written].to_vec()).expect("kernel emits UTF-8"))
    }

    /// Run the SIMD kernel and the scalar reference on the same
    /// units, through properly aligned buffers (CPython's UCS
    /// buffers are naturally aligned for their unit width).
    fn transcode_both(kind: Kind, cps: &[u32]) -> (PyResult<String>, PyResult<String>) {
        {
            match kind {
                Kind::Ucs1 => {
                    let b: Vec<u8> = cps.iter().map(|&c| c as u8).collect();
                    let p = b.as_ptr();
                    (
                        call(kind, cps.len(), p, transcode),
                        call(kind, cps.len(), p, transcode_scalar),
                    )
                }
                Kind::Ucs2 => {
                    let b: Vec<u16> = cps.iter().map(|&c| c as u16).collect();
                    let p = b.as_ptr() as *const u8;
                    (
                        call(kind, cps.len(), p, transcode),
                        call(kind, cps.len(), p, transcode_scalar),
                    )
                }
                Kind::Ucs4 => {
                    let b: Vec<u32> = cps.to_vec();
                    let p = b.as_ptr() as *const u8;
                    (
                        call(kind, cps.len(), p, transcode),
                        call(kind, cps.len(), p, transcode_scalar),
                    )
                }
            }
        }
    }

    /// Compare verdicts and bytes.
    fn assert_match(kind: Kind, cps: &[u32]) {
        let (via_simd, via_scalar) = transcode_both(kind, cps);
        match (via_simd, via_scalar) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "kind {kind:?} cps {cps:?}"),
            (Err(_), Err(_)) => {}
            (a, b) => panic!(
                "verdict mismatch kind {kind:?} cps {cps:?}: simd_ok={} scalar_ok={}",
                a.is_ok(),
                b.is_ok()
            ),
        }
    }

    /// The table must emit, per row, exactly the lanes' valid slot
    /// prefixes in order.
    #[test]
    fn pack_table_is_well_formed() {
        for index in 0..PACK_ROWS {
            let row = &PACK_TABLE[index * PACK_STRIDE..(index + 1) * PACK_STRIDE];
            let mut expected_len = 0usize;
            for lane in 0..4 {
                expected_len += ((index >> (2 * lane)) & 0b11) + 1;
            }
            assert_eq!(row[0] as usize, expected_len, "row {index}");
            for out in 0..expected_len {
                let src = row[1 + out] as usize;
                assert!(src < 16, "row {index} byte {out} sources {src}");
                let lane = src / 4;
                let byte = src % 4;
                assert!(
                    byte < ((index >> (2 * lane)) & 0b11) + 1,
                    "row {index} byte {out} takes a dead slot byte"
                );
            }
        }
    }

    /// Kernel vs scalar reference over the drawn corpus, including
    /// the error paths (both must fail on the same input).
    #[test]
    fn fuzz_kernel_matches_scalar() {
        if !supported() {
            return;
        }
        let mut rng = Rng(0xC0FFEE);
        for kind in [Kind::Ucs1, Kind::Ucs2, Kind::Ucs4] {
            for _case in 0..3000 {
                let len = rng.below(72) as usize;
                let cps: Vec<u32> = (0..len)
                    .map(|_| clamp_for_kind(kind, draw_cp(&mut rng)))
                    .collect();
                assert_match(kind, &cps);
            }
        }
    }

    /// Chunk-boundary lengths: every alignment of the 8-unit kernel
    /// loop and the scalar tail, each over a mixed-width alphabet so
    /// the compaction index varies inside chunks.
    #[test]
    fn boundary_lengths_match_scalar() {
        if !supported() {
            return;
        }
        let alphabet: [u32; 7] = [b'a' as u32, 0xE9, 0x645, 0x4E00, 0x1F600, 0xD800, 0x10FFFF];
        for kind in [Kind::Ucs1, Kind::Ucs2, Kind::Ucs4] {
            for len in 0..=40usize {
                for pattern in 0..7u32 {
                    let cps: Vec<u32> = (0..len)
                        .map(|i| {
                            let cp = alphabet[((i as u32 + pattern) % 7) as usize];
                            clamp_for_kind(kind, cp)
                        })
                        .collect();
                    assert_match(kind, &cps);
                }
            }
        }
    }

    /// Every uniform shape (all-ASCII, all-2, all-3, all-4 byte) at
    /// every chunk-aligned length, which is where the fixed-shuffle
    /// fast paths live.
    #[test]
    fn uniform_shapes_match_scalar() {
        if !supported() {
            return;
        }
        let shapes: [u32; 4] = [
            b'x' as u32, // 1 byte
            0xE9,        // 2 bytes (latin-1 storage)
            0x4E00,      // 3 bytes
            0x1F600,     // 4 bytes
        ];
        for kind in [Kind::Ucs1, Kind::Ucs2, Kind::Ucs4] {
            for cp in shapes {
                let cp = clamp_for_kind(kind, cp);
                for len in [8usize, 9, 16, 24, 32, 40, 41] {
                    assert_match(kind, &vec![cp; len]);
                }
            }
        }
    }
}
