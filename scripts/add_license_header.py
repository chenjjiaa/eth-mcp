import sys
import pathlib

LICENSE_MAP = {
    ".sh": "#",
    ".rs": "//",
    ".py": "#",
    ".go": "//",
    ".js": "//",
    ".jsx": "//",
    ".ts": "//",
    ".tsx": "//"
}

LICENSE_LINES = [
    "Copyright 2025 chenjjiaa",
    "",
    "Licensed under the Apache License, Version 2.0 (the \"License\");",
    "you may not use this file except in compliance with the License.",
    "You may obtain a copy of the License at",
    "",
    "    http://www.apache.org/licenses/LICENSE-2.0",
    "",
    "Unless required by applicable law or agreed to in writing, software",
    "distributed under the License is distributed on an \"AS IS\" BASIS,",
    "WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.",
    "See the License for the specific language governing permissions and",
    "limitations under the License."
]

def add_header(file_path):
    ext = pathlib.Path(file_path).suffix
    comment = LICENSE_MAP.get(ext)
    if not comment:
        return  # unsupported file type
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    # skip if already has license
    if any("Licensed under the Apache License" in line for line in lines[:15]):
        return

    commented_header = [f"{comment} {line}".rstrip() + '\n' if line else f"{comment}\n" for line in LICENSE_LINES]
    with open(file_path, 'w', encoding='utf-8') as f:
        f.writelines(commented_header + ['\n'] + lines)

if __name__ == "__main__":
    for file in sys.argv[1:]:
        add_header(file)
