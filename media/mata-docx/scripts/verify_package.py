#!/usr/bin/env python3
"""Validate package structure with Python stdlib (test helper; not Rust dependency)."""
import sys
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

for item in sys.argv[1:]:
    with zipfile.ZipFile(item) as z:
        assert z.testzip() is None, f"Bad member CRC: {item}"
        names = set(z.namelist())
        for required in ('[Content_Types].xml', '_rels/.rels', 'word/document.xml'):
            assert required in names, f'Missing {required}'
        for name in names:
            if name.endswith(('.xml', '.rels')) or name == '[Content_Types].xml':
                ET.fromstring(z.read(name))
        print(f'{Path(item).name}: {len(names)} entries, ZIP CRC and XML valid')
