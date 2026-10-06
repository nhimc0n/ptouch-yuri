# Fixture notes (captured 2026-10-06, P-touch Editor on Windows, PT-E850TKW over Wi-Fi infrastructure)

Common: cassette TZe-S661 (36 mm), "paper size 1.4 in", half-cut ON, chain/"in chuoi" ON in the UI,
feed 0.04 in (14 dots), quality standard 360x360. Print path: LPR to TCP 515, queue BINARY_P1.
Extracted with the pure-Python LPD reassembler (no tshark needed); data file only.

| File | Capture | LPR job | Lines | Content | Result |
|---|---|---|---|---|---|
| cap2_005I_15863B.bin | 2.pcapng | A005 | 1200 | solid black rectangle, full 36 mm | ok |
| cap2_006I_15863B.bin | 2.pcapng | A006 | 1200 | same as 005 (reprint, identical raster) | ok |
| cap2_007I_14160B.bin | 2.pcapng | A007 | 1069 | shorter solid rectangle | ok |
| cap3_008I_11680B.bin | 3.pcapng | A008 | 622/1069 | letter F, **aborted / truncated stream** | do not use as golden |
| cap3_011I_19892B.bin | 3.pcapng | A011 | 1069 | letter F (upright, correct) | ok, used by Rust golden test |
| cap3_012I_19892B.bin | 3.pcapng | A012 | 1069 | same as 011 | ok |

Capture 1 is idle startup: one SNMP GET (serial) and mDNS only; no job.
Rectangle lengths in the UI were not recorded (1200 lines = 84.7 mm, 1069 = 75.4 mm); the
original 30 mm request does not match, so treat lengths as measured, not intended.
