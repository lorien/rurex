// UCS to UTF-8 — rurex design note. 16:9, typst 0.15.
#set document(title: "UCS to UTF-8 — why rurex re-encodes Python strings", author: "rurex")
#set page(width: 13.333in, height: 7.5in, margin: 0pt, fill: white)
#set text(font: "Lato", size: 15pt, fill: rgb("#16181A"), lang: "en")
#set par(leading: 0.58em, justify: false)
#show raw: set text(font: "DejaVu Sans Mono", size: 12pt)
#show emph: set text(fill: rgb("#16181A"), weight: "bold")
#show strong: set text(fill: rgb("#16181A"), weight: "bold")

#let ink    = rgb("#16181A")
#let muted  = rgb("#656A70")
#let hair   = rgb("#DCDEE1")
#let accent = rgb("#B2552D")
#let wash   = rgb("#F7F4F1")
#let mono   = "DejaVu Sans Mono"

#let dim(body) = text(size: 13pt, fill: muted, body)
#let chip(body) = box(fill: wash, radius: 3pt, inset: (x: 6pt, y: 2pt), text(size: 12pt, fill: muted, body))
#let val(body) = text(size: 27pt, weight: "bold", tracking: -0.02em, accent, body)
#let kill(body) = text(size: 17pt, body)
#let rule() = { v(11pt); line(length: 100%, stroke: 0.8pt + hair); v(11pt) }

#let bullets(..items) = {
  set list(marker: text(fill: accent, "·"), indent: 0pt, body-indent: 12pt, spacing: 6pt)
  list(..items)
}

#let slide(num, kicker, title, body, center: true) = page(
  margin: (left: 62pt, right: 62pt, top: 40pt, bottom: 36pt),
  footer: grid(columns: (1fr, auto), align: horizon,
    text(size: 10pt, fill: muted, "rurex  ·  UCS to UTF-8"),
    text(size: 10pt, fill: muted, str(num))),
)[
  #text(size: 10pt, fill: accent, weight: "bold", tracking: 0.09em, upper(kicker))
  #v(5pt)
  #text(size: 24pt, weight: "bold", tracking: -0.012em, title)
  #v(8pt)
  #line(length: 100%, stroke: 0.9pt + hair)
  #v(10pt)
  #if center { v(1fr) }
  #body
  #if center { v(1fr) }
]

// ---------- 1. title ----------
#page(margin: (left: 72pt, right: 72pt, top: 60pt, bottom: 44pt), footer: [])[
  #place(top + left, dy: 86pt)[
    #text(size: 10.5pt, fill: accent, weight: "bold", tracking: 0.1em, "RUREX  ·  DESIGN NOTE")
    #v(14pt)
    #text(size: 54pt, weight: "bold", tracking: -0.03em, "UCS to UTF-8")
    #v(10pt)
    #text(size: 20pt, fill: rgb("#3C4147"), "Why every Python string is re-encoded before the regex engine can see it")
    #v(22pt)
    #line(length: 40%, stroke: 2.4pt + accent)
    #v(20pt)
    #dim[The engine matches UTF-8. CPython stores `str` as UCS-1/2/4. The conversion in between is the whole problem.]
  ]
  #place(bottom + left, text(size: 10pt, fill: muted,
    [#link("https://github.com/lorien/rurex")[github.com/lorien/rurex]  ·  ADR-0012  ·  ADR-0013]))
  #place(bottom + right, text(size: 10pt, fill: muted, "1"))
]

// ---------- 2. mismatch ----------
#slide(2, "The mismatch", "One string, two representations")[
  #grid(columns: (1fr, 1fr), column-gutter: 48pt, [
    #text(size: 12.5pt, weight: "bold", "Python `str` — PEP 393 UCS")
    #v(14pt)
    #grid(columns: (auto, 1fr), column-gutter: 14pt, row-gutter: 11pt, align: (left + top, left + top),
      [#chip("UCS-1")], [1 byte per unit. ASCII and Latin-1, and *the bytes are valid UTF-8 already*.],
      [#chip("UCS-2")], [2 bytes per unit. Up to U+FFFF, and never a valid surrogate pair.],
      [#chip("UCS-4")], [4 bytes per unit. Above U+FFFF; lone surrogates and out-of-range values possible.],
    )
    #v(12pt)
    #dim[The kind follows the string's maximum code point, not its content.]
  ], [
    #text(size: 12.5pt, weight: "bold", "Rust `regex` — UTF-8 bytes")
    #v(14pt)
    #grid(columns: 8, column-gutter: 3pt,
      ..("61", "C3", "A9", "CE", "B2", "F0", "9F", "99").map(v => box(
        stroke: 0.8pt + hair, radius: 3pt, inset: (x: 5.5pt, y: 4pt),
        text(font: mono, size: 11.5pt, v))))
    #v(8pt)
    #dim[`a` `é` `β` `🙂` — 1, 2, 2 and 4 bytes]
    #v(16pt)
    #dim[The engine accepts one input format, and cannot be taught another without forking it.]
  ])
  #v(14pt)
  #line(length: 100%, stroke: 0.8pt + hair)
  #v(10pt)
  #kill[`re` matches code points directly, so it never converts. rurex must produce UTF-8 first, inside the call, with the GIL held.]
]

// ---------- 3. cost ----------
#slide(3, "The cost", "Encoding is neither free nor small")[
  #grid(columns: (1fr, 1fr, 1fr), column-gutter: 34pt,
    [ #val("0.45 GB/s") #v(6pt) #dim[`PyUnicode_AsUTF8AndSize`, CPython's scalar encoder, cached in the object] ],
    [ #val("0.48 GB/s") #v(6pt) #dim[Rust's scalar encoder; `String::from_utf16` lands in the same class] ],
    [ #val("x3.0") #v(6pt) #dim[rurex against `re` on cold non-ASCII strings, before this work] ],
  )
  #rule()
  #dim[Telegram corpus, 1M short non-ASCII posts, cold strings, GIL, one thread:]
  #v(6pt)
  #kill[1.58 s of work, about 1.0 s of it encoding. The conversion dominated the call, not the matching.]
]

// ---------- 4. three paths ----------
#slide(4, "The fix", "Three ways to view a str as UTF-8")[
  #let p(n, head, cond, cost, hot: false) = grid(
    columns: (22pt, 1fr, 138pt), column-gutter: 14pt, align: (left, left + top, right + top),
    text(size: 11.5pt, fill: accent, weight: "bold", n),
    [ #text(weight: "bold", head) #v(3pt) #par(dim(cond)) ],
    text(size: 12.5pt, fill: if hot { accent } else { muted }, weight: if hot { "bold" } else { "regular" }, cost),
  )
  #p("1", "Compact ASCII", "The UCS-1 data is a UTF-8 string already. Borrow the object's own buffer.", "zero copies")
  #rule()
  #p("2", "CPython's cache is warm", "An earlier C-API consumer encoded it. Borrow the cached buffer.", "zero copies")
  #rule()
  #p("3", "Cold non-ASCII", "Prefilter the raw UCS storage, then transcode into a reused scratch buffer.", "1.4 – 3.8 GB/s", hot: true)
  #v(18pt)
  #line(length: 100%, stroke: 0.8pt + hair)
  #v(11pt)
  #dim[rurex never writes CPython's cache: the buffer is `PyMem_Malloc`-owned, and only `PyUnicode_AsUTF8AndSize` may fill it. Strings warmed solely by rurex re-transcode next scan, traded deliberately against a side cache that would have to survive object identity and lifetime.]
]

// ---------- 5. prefilter ----------
#slide(5, "Path 3a", "Cheapest answer first: do not transcode at all")[
  #grid(columns: (1fr, 1fr), column-gutter: 44pt, [
    #kill[When every possible match must begin with one of at most three distinct ASCII bytes, that byte is searched in the raw UCS storage instead.]
    #v(14pt)
    #dim[The literal set is extracted once, at compile time, from `regex-syntax`'s prefix-literal analysis. The haystack is not touched until the pattern exists.]
  ], [
    #box(fill: wash, radius: 3pt, inset: 16pt, width: 100%)[
      #text(size: 11pt, fill: muted, weight: "bold", "THE CHECK, IN ORDER") #v(9pt)
      #text(size: 13.5pt)[
        + UCS-1: a plain `memchr` over the units.
        + UCS-2 / UCS-4: a unit-aligned scan, so a hit cannot be a fragment of a neighbouring unit.
        + Byte absent: no match can exist, return the no-match answer.
      ]
    ]
  ])
  #v(18pt)
  #line(length: 100%, stroke: 0.8pt + hair)
  #v(11pt)
  #dim[Cold path only — never for ASCII, a warm cache, or an unvalidated layout. A false positive costs one wasted transcode; a false negative would be a wrong answer, so the byte set must cover every match.]
]

// ---------- 6. transcoder ----------
#slide(6, "Path 3b", "The transcoder")[
  #grid(columns: (1fr, 1fr), column-gutter: 44pt, [
    #bullets(
      [Hand-written AVX2 + BMI2 kernel, runtime detection, one `target_feature` function.],
      [One kernel for all three kinds: units widen to eight `u32` lanes, and no surrogate-pair decoding is ever needed.],
      [Uniform chunk: a fixed shuffle emits the bytes. Mixed chunk: four candidate bytes staged per lane, compacted by a `pshufb` from a 256-row table.],
      [The table beats `pext`/`pdep` control synthesis by 1.6 – 2x.],
    )
  ], [
    #box(fill: wash, radius: 3pt, inset: 14pt, width: 100%)[
      #text(size: 11pt, fill: muted, weight: "bold", "GB/s OF UTF-8 OUT") #v(9pt)
      #grid(columns: (1fr, auto), column-gutter: 10pt, row-gutter: 5pt, align: (left, right),
        text(size: 13.5pt)[mixed Arabic], text(size: 13.5pt, weight: "bold")[1.9],
        text(size: 13.5pt)[pure CJK],     text(size: 13.5pt, weight: "bold")[3.8],
        text(size: 13.5pt)[emoji],        text(size: 13.5pt, weight: "bold")[1.5],
        text(size: 13.5pt, fill: muted)[scalar fallback], text(size: 13.5pt, fill: muted)[0.48],
      )
    ]
    #v(12pt)
    #dim[No new dependency: `encoding_rs` stays scalar on stable Rust, `xutf` is nightly and permissive, `simdutf` would drag C++ into every wheel.]
  ])
]

// ---------- 7. errors ----------
#slide(7, "The contract", "Errors must match CPython exactly")[
  #grid(columns: (1fr, 1fr), column-gutter: 44pt, [
    #kill[A Python `str` may legally hold a lone surrogate, and a UCS-4 string may hold a value above U+10FFFF. Neither survives an encode.]
    #v(12pt)
    #dim[Both raise `UnicodeEncodeError` at the *first* offending unit: the same error, at the same index, that `PyUnicode_AsUTF8AndSize` reports.]
  ], [
    #box(fill: wash, radius: 3pt, inset: 16pt, width: 100%)[
      #text(size: 11pt, fill: muted, weight: "bold", "HOW THE KERNEL KEEPS THAT") #v(9pt)
      #text(size: 13.5pt)[
        + Surrogate and out-of-range lanes detected in bulk, before any SIMD work.
        + One bad lane sends the whole 8-unit chunk to the scalar encoder.
        + Scalar raises at the first bad unit, so the reported index is identical.
      ]
    ]
  ])
  #v(18pt)
  #line(length: 100%, stroke: 0.8pt + hair)
  #v(11pt)
  #dim[Guarded by differential tests against CPython's own encoder: a torture corpus, seeded fuzzing over raw code points including lone surrogates, and every chunk boundary and uniform shape.]
]

// ---------- 8. layout ----------
#slide(8, "The trap", "PEP 393 field offsets are not portable")[
  #grid(columns: (1fr, 1fr), column-gutter: 44pt, [
    #bullets(
      [Reading `kind`, `compact`, `ascii`, the length and the data pointer means reading CPython's fields through raw pointers.],
      [`PyObject` is 16 bytes on a GIL build, 32 on a free-threaded one.],
      [`PyCompactUnicodeObject` carried an extra `wstr_length` before 3.12.],
      [Free-threaded builds widen `state.interned`, shifting the `kind`/`compact`/`ascii` bits upward.],
    )
  ], [
    #box(fill: wash, radius: 3pt, inset: 16pt, width: 100%)[
      #text(size: 11pt, fill: muted, weight: "bold", "WHAT THE OLD CONSTANTS DID") #v(9pt)
      #text(size: 13.5pt)[On a free-threaded build they pointed at the refcount: the ASCII bit was read from a refcount word, and on a differently aligned build the peek could have returned a bogus slice.]
      #v(10pt)
      #line(length: 100%, stroke: 0.8pt + hair)
      #v(9pt)
      #text(size: 13.5pt, weight: "bold", fill: accent)[Detect once per process, validate against CPython's own behaviour. If it fails, every `str` takes `to_cow`.]
    ]
  ])
  #v(16pt)
  #kill[Slow on an unknown layout. Never wrong on one.]
]

// ---------- 9. summary ----------
#slide(9, "Summary", "One table, in the order a haystack meets them", center: false)[
  #v(14pt)
  #table(
    columns: (1.9fr, 3.2fr, 1.5fr), stroke: none, inset: (x: 0pt, y: 9pt), align: (left, left, right),
    table.hline(stroke: 0.9pt + hair),
    text(size: 10.5pt, fill: muted, weight: "bold", "PATH"),
    text(size: 10.5pt, fill: muted, weight: "bold", "CONDITION"),
    text(size: 10.5pt, fill: muted, weight: "bold", "COST"),
    table.hline(stroke: 0.8pt + hair),
    text(weight: "bold", "Borrow UCS-1"), text(fill: muted)[compact ASCII], text[zero],
    table.hline(stroke: 0.8pt + hair),
    text(weight: "bold", "Borrow cache"), text(fill: muted)[non-ASCII, CPython cache warm], text[zero],
    table.hline(stroke: 0.8pt + hair),
    text(weight: "bold", "Prefilter miss"), text(fill: muted)[required ASCII byte absent from UCS], text[a scan only],
    table.hline(stroke: 0.8pt + hair),
    text(weight: "bold", "Transcode"), text(fill: muted)[cold, required byte present], text[1.4 – 3.8 GB/s],
    table.hline(stroke: 0.8pt + hair),
    text(weight: "bold", raw("to_cow")), text(fill: muted)[layout unvalidated], text[0.45 GB/s],
    table.hline(stroke: 0.9pt + hair),
  )
  #v(24pt)
  #kill[The optimisation is not faster encoding. It is arranging, four times over, that no encoding happens.]
]
