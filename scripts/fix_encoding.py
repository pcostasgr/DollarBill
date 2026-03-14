"""
Recovers a file that was corrupted by PowerShell's Get-Content (CP1252) → Set-Content (UTF-8).

The operation was: UTF-8 bytes → read as CP1252 → written as UTF-8.
To reverse: read as UTF-8 → encode as CP1252 → decode as UTF-8.

CP1252 has 5 undefined byte positions (0x81, 0x8D, 0x8F, 0x90, 0x9D). PowerShell decodes
these as their corresponding C1 control-code Unicode code points. We map them back.
"""

import sys

# CP1252 undefined byte positions: PS decodes them as C1 Unicode control codes
CTRL_TO_BYTE = {
    "\u0081": 0x81,
    "\u008D": 0x8D,
    "\u008F": 0x8F,
    "\u0090": 0x90,
    "\u009D": 0x9D,
}


def fix_file(path: str) -> None:
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()

    result = bytearray()
    for ch in text:
        cp = ord(ch)
        if cp < 128:
            result.append(cp)
        elif ch in CTRL_TO_BYTE:
            result.append(CTRL_TO_BYTE[ch])
        else:
            try:
                result.extend(ch.encode("cp1252"))
            except UnicodeEncodeError:
                # Genuinely unmappable — keep as-is (UTF-8 encoded)
                result.extend(ch.encode("utf-8"))

    recovered = result.decode("utf-8")
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(recovered)
    print(f"Fixed: {path}")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        fix_file(p)
