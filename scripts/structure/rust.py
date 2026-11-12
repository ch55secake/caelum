"""Count production and test lines without interpreting braces in Rust literals."""
import re
from dataclasses import dataclass
from pathlib import PurePosixPath

RAW = re.compile(r'(?:br|cr|r)(#*)"')
CHAR = re.compile(r"'(?:\\(?:u\{[\da-fA-F_]+\}|x[\da-fA-F]{2}|.)|[^\\'\n])'")
ATTRIBUTE = re.compile(r"\s*#\s*\[")


@dataclass(frozen=True)
class Size:
    source: int
    tests: int


def code_only(text: str) -> str:
    """Mask comments and literals, retaining offsets and line breaks."""
    result = list(text)
    i = 0
    while i < len(text):
        start = i
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
        elif text.startswith("/*", i):
            i += 2
            depth = 1
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        elif match := RAW.match(text, i):
            closing = '"' + match[1]
            end = text.find(closing, i + len(match[0]))
            i = len(text) if end < 0 else end + len(closing)
        elif text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
        elif match := CHAR.match(text, i):
            i += len(match[0])
        else:
            i += 1
            continue
        for at in range(start, min(i, len(text))):
            if result[at] != "\n":
                result[at] = " "
    return "".join(result)


def is_test(path: str) -> bool:
    path = PurePosixPath(path)
    return "tests" in path.parts or bool(re.search(r"(^|_)tests?$", path.stem))


def test_only(predicate: str) -> bool:
    """Conservatively recognize test-only cfgs, including platform-gated tests."""
    predicate = predicate.strip()
    if predicate == "test":
        return True
    match = re.fullmatch(r"(all|any)\s*\((.*)\)", predicate, re.S)
    if not match:
        return False
    parts = []
    depth = start = 0
    for i, char in enumerate(match[2]):
        depth += (char == "(") - (char == ")")
        if char == "," and depth == 0:
            parts.append(match[2][start:i])
            start = i + 1
    parts.append(match[2][start:])
    parts = [part for part in parts if part.strip()]
    return bool(parts) and (any if match[1] == "all" else all)(test_only(part) for part in parts)


def measure(path: str, text: str) -> Size:
    total = len(text.splitlines())
    if is_test(path):
        return Size(0, total)
    code = code_only(text)
    excluded = set()
    maximum = 0
    consumed = 0
    for attribute in re.finditer(r"#\s*\[\s*cfg\s*\(([^\]]*)\)\s*\]", code):
        if attribute.start() < consumed or not test_only(attribute[1]):
            continue
        i = attribute.end()
        # An item may have more attributes, including a #[path] declaration.
        while match := ATTRIBUTE.match(code, i):
            i += len(match[0])
            depth = 1
            while i < len(code) and depth:
                depth += (code[i] == "[") - (code[i] == "]")
                i += 1
        while i < len(code) and code[i] not in "{;":
            i += 1
        if i < len(code) and code[i] == "{":
            i += 1
            depth = 1
            while i < len(code) and depth:
                depth += (code[i] == "{") - (code[i] == "}")
                i += 1
        else:
            i = min(i + 1, len(code))
        first = code.count("\n", 0, attribute.start())
        last = code.count("\n", 0, max(attribute.start(), i - 1))
        excluded.update(range(first, last + 1))
        maximum = max(maximum, last - first + 1)
        consumed = i
    return Size(total - len(excluded), maximum)
