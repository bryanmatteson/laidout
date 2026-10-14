#!/usr/bin/env python3
"""Build and record the real Laidout formatter example with VHS."""
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
output = Path(os.environ.get("README_RECORD_OUTPUT", Path.home() / "Documents/Developer/laidout/readme")).expanduser().resolve()
output.mkdir(parents=True, exist_ok=True)
subprocess.run(["cargo", "build", "--example", "readme"], cwd=root, check=True)
binary = root / "target/debug/examples/readme"
transcript = []
results = []
for kind, width in [("json", "80"), ("json", "38"), ("table", "60"), ("table", "14")]:
    text = subprocess.check_output([str(binary), kind, width], text=True)
    results.append(text)
    transcript.append(f"$ laidout {kind} {width}\n{text}")
(output / "transcript.txt").write_text("\n".join(transcript))
if results[0] == results[1] or results[2] == results[3] or any("東京" not in text for text in results[2:]):
    raise SystemExit("The formatter demo did not preserve its content.")
limiter = Path.home() / ".local/bin/agent-build"
prefix = [str(limiter), "--"] if limiter.exists() else []
tape = output / "demo.tape"
tape.write_text((root / "docs/assets/demo.tape").read_text().replace("Output docs/assets/demo.gif", "Output demo.gif"))
subprocess.run(prefix + ["vhs", str(tape)], cwd=output, env=dict(os.environ, README_DEMO_BIN=str(binary)), check=True, timeout=180)
shutil.copyfile(output / "demo.gif", root / "docs/assets/demo.gif")
print("Recorded docs/assets/demo.gif")
