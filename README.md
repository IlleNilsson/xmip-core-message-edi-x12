# xmip-core-message-edi-x12

ASC X12 shape: an ISA interchange as segments named by their tag, the first ST01 as the announced type; the one reader of the ISA, which the X12 contract reads an interchange's head by. A technology of [xmip-core-message](https://github.com/IlleNilsson/xmip-core-message).

The `ISA` is read once, here (`delimiters`): the element separator, ISA16
and the terminator at their fixed positions, each held to ASCII because the
walk reads bytes, the element separator and the terminator neither one
another nor a letter or digit. The X12 contract
([xmip-core-contract-edi-x12](https://github.com/IlleNilsson/xmip-core-contract-edi-x12))
reads an interchange's head by it, the one technology it depends on
(ADR-0044, amendment 2026-11-15), so the two read and refuse an `ISA` in
the same words.

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
