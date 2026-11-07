#!/usr/bin/env python3
"""Risk notice string checks for the Android sender spike (docs/RISK_ACK_SCREEN_SPEC.md, D023).

  python3 tools/check-risk-notice-strings.py           check (exit 1 on any problem)
  python3 tools/check-risk-notice-strings.py --write   regenerate the Android string resources from docs/RISK_NOTICE.md

Checks: (1) the ko/en short texts, checkbox label, title and the six numbered items in the Android
resources equal docs/RISK_NOTICE.md (markdown emphasis removed); (2) no forbidden wording for a PC in the
UI strings or Kotlin sources outside the verbatim notice items; (3) the draft marker exists; (4) the text hash
recorded for risk_notice_version matches (a text change must bump the version and the hash).
Synthetic/no secrets; stdlib only.
"""
import hashlib, pathlib, re, sys
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC = ROOT / "docs" / "RISK_NOTICE.md"
RES = ROOT / "apps" / "android" / "peripheral" / "src" / "main" / "res"
EN_XML, KO_XML = RES / "values" / "risk_notice.xml", RES / "values-ko" / "risk_notice.xml"
VER_XML = RES / "values" / "risk_notice_version.xml"
KEYS = ["risk_short_1", "risk_short_2", "risk_checkbox", "risk_draft_marker", "risk_full_title"] + [f"risk_full_{i}" for i in range(1, 7)]
DRAFT = {"en": "Draft text, not reviewed by a lawyer.", "ko": "초안이며 법무 검토를 받지 않았습니다."}
FORBIDDEN = [r"verified safe", r"secure pc", r"안전함", r"검증됨", r"\bsafe pc\b", r"\bsafe computer\b"]
VERBATIM_PREFIX = "risk_full_"  # the verbatim notice items quote the forbidden words on purpose (negated)


def strip_md(s):
    return s.replace("**", "").replace("*", "").strip()


def doc_texts():
    text = DOC.read_text(encoding="utf-8")
    ko_i, en_i = text.index("## 한국어"), text.index("## English")
    out = {}
    for lang, sec in (("ko", text[ko_i:en_i]), ("en", text[en_i:])):
        title = re.search(r"^\*\*(.+)\*\*$", sec, re.M).group(1)
        items = {int(m.group(1)): strip_md(m.group(2)) for m in re.finditer(r"^(\d)\. (.+)$", sec, re.M)}
        quote = [l[1:].strip() for l in sec.splitlines() if l.startswith(">")]
        quote = [l for l in quote if l]
        d = {"risk_full_title": strip_md(title), "risk_short_1": quote[0], "risk_short_2": quote[1],
             "risk_checkbox": quote[2].lstrip("☐").strip(), "risk_draft_marker": DRAFT[lang]}
        assert sorted(items) == [1, 2, 3, 4, 5, 6], f"{lang}: numbered items {sorted(items)}"
        for i in range(1, 7):
            d[f"risk_full_{i}"] = items[i]
        assert len(quote) == 3, f"{lang}: short text lines {quote}"
        out[lang] = d
    return out


def esc(v):
    v = v.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', '\\"').replace("'", "\\'")
    return ("\\" + v) if v[:1] in "@?" else v


def unesc(v):
    return v.replace('\\"', '"').replace("\\'", "'").replace("\\n", "\n").replace("\\@", "@").replace("\\?", "?")


def read_xml(p):
    return {e.get("name"): unesc(e.text or "") for e in ET.parse(p).getroot().iter("string")}


def text_hash(d):
    blob = "\n".join(f"{lang}:{k}={d[lang][k]}" for lang in ("en", "ko") for k in sorted(KEYS))
    return hashlib.sha256(blob.encode("utf-8")).hexdigest()


def write(d):
    for lang, p in (("en", EN_XML), ("ko", KO_XML)):
        p.parent.mkdir(parents=True, exist_ok=True)
        lines = ['<?xml version="1.0" encoding="utf-8"?>',
                 "<!-- GENERATED from docs/RISK_NOTICE.md by tools/check-risk-notice-strings.py (write mode). DRAFT text, not legally reviewed. Do not edit by hand. -->",
                 "<resources>"]
        lines += [f'    <string name="{k}">{esc(d[lang][k])}</string>' for k in KEYS]
        lines.append("</resources>")
        p.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("wrote", EN_XML.relative_to(ROOT), KO_XML.relative_to(ROOT))
    print("text sha256:", text_hash(d))


def main():
    d = doc_texts()
    if "--write" in sys.argv:
        write(d); return 0
    problems = []
    res = {"en": read_xml(EN_XML), "ko": read_xml(KO_XML)}
    for lang in ("en", "ko"):
        for k in KEYS:
            if k not in res[lang]:
                problems.append(f"{lang}: resource {k} missing")
            elif res[lang][k] != d[lang][k]:
                problems.append(f"{lang}: {k} differs from docs/RISK_NOTICE.md\n   res: {res[lang][k]!r}\n   doc: {d[lang][k]!r}")
        extra = set(res[lang]) - set(KEYS)
        if extra:
            problems.append(f"{lang}: unexpected strings {sorted(extra)}")
        low = (res[lang].get("risk_draft_marker", "")).lower()
        if not ("draft" in low and "lawyer" in low) and not ("초안" in low and "법무" in low):
            problems.append(f"{lang}: draft marker must say draft and not legally reviewed")
        for k, v in res[lang].items():
            if k.startswith(VERBATIM_PREFIX):
                continue
            for pat in FORBIDDEN:
                if re.search(pat, v, re.I):
                    problems.append(f"{lang}: {k} uses forbidden wording /{pat}/")
    for p in list((ROOT / "apps" / "android").rglob("*.kt")) + list(RES.rglob("*.xml")):
        if "/build/" in str(p) or p in (EN_XML, KO_XML):
            continue
        body = p.read_text(encoding="utf-8", errors="ignore")
        for pat in FORBIDDEN:
            if re.search(pat, body, re.I):
                problems.append(f"{p.relative_to(ROOT)}: forbidden wording /{pat}/")
    try:
        ver = ET.parse(VER_XML).getroot()
        sha = [e.text for e in ver.iter("string") if e.get("name") == "risk_notice_text_sha256"][0]
        version = int([e.text for e in ver.iter("integer") if e.get("name") == "risk_notice_version"][0])
        if version < 1:
            problems.append("risk_notice_version must be >= 1")
        want = text_hash(res)
        if sha != want:
            problems.append(f"text hash differs from the recorded one: the text changed. Bump risk_notice_version "
                            f"and set risk_notice_text_sha256 to {want}")
    except Exception as e:  # missing or malformed version resource
        problems.append(f"version resource unreadable: {e!r}")
    if problems:
        print("\n".join("FAIL: " + p for p in problems)); return 1
    print(f"OK: ko/en notice resources equal docs/RISK_NOTICE.md; no forbidden wording; version lock holds (version {version})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
