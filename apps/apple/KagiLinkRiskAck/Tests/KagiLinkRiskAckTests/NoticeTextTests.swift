import Foundation
import XCTest
@testable import KagiLinkRiskAck

/// Reads docs/RISK_NOTICE.md from the repository and compares it with the resource strings.
final class NoticeTextTests: XCTestCase {
    private func loadDocument() throws -> [String] {
        var dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = dir.appendingPathComponent("docs/RISK_NOTICE.md")
            if let text = try? String(contentsOf: candidate, encoding: .utf8) { return text.components(separatedBy: "\n") }
            dir.deleteLastPathComponent()
        }
        throw XCTSkip("docs/RISK_NOTICE.md not found above the test file")
    }

    private struct Parsed {
        var title = ""
        var items: [String] = []
        var short: [String] = []
        var checkbox = ""
    }

    private func parse(_ lines: [String], header: String, nextHeader: String?, shortMarker: String) -> Parsed {
        var parsed = Parsed()
        var active = false
        var inShort = false
        for line in lines {
            if line == header { active = true; continue }
            if active, let nextHeader, line == nextHeader { break }
            guard active else { continue }
            if line.hasPrefix("**") && line.hasSuffix("**") && line.contains("KagiLink") && !line.contains(shortMarker) {
                parsed.title = line
            } else if line.contains(shortMarker) {
                inShort = true
            } else if inShort, line.hasPrefix("> ") {
                let body = String(line.dropFirst(2))
                if body.hasPrefix("☐ ") { parsed.checkbox = String(body.dropFirst(2)) } else { parsed.short.append(body) }
            } else if !inShort, let first = line.first, first.isNumber, line.dropFirst().hasPrefix(". ") {
                parsed.items.append(line)
            }
        }
        return parsed
    }

    func testKoreanMatchesDocument() throws {
        let doc = try parse(loadDocument(), header: "## 한국어", nextHeader: "## English", shortMarker: "화면 문구(짧은 버전)")
        let ko = NoticeResource.ko
        XCTAssertEqual(ko.title, doc.title)
        XCTAssertEqual(ko.fullItems, doc.items)
        XCTAssertEqual(ko.shortLines, doc.short)
        XCTAssertEqual(ko.checkboxLabel, doc.checkbox)
        XCTAssertEqual(doc.items.count, 6)
    }

    func testEnglishMatchesDocument() throws {
        let doc = try parse(loadDocument(), header: "## English", nextHeader: nil, shortMarker: "On-screen text (short)")
        let en = NoticeResource.en
        XCTAssertEqual(en.title, doc.title)
        XCTAssertEqual(en.fullItems, doc.items)
        XCTAssertEqual(en.shortLines, doc.short)
        XCTAssertEqual(en.checkboxLabel, doc.checkbox)
        XCTAssertEqual(doc.items.count, 6)
    }

    private func allStrings(_ t: NoticeText, includeFull: Bool) -> [String] {
        var out = [t.title] + t.shortLines + [t.checkboxLabel, t.draftBanner,
            t.ui.viewFull, t.ui.fullTitle, t.ui.close, t.ui.continueButton, t.ui.cancelButton,
            t.ui.unavailable, t.ui.storageFailed, t.ui.checkboxHint]
        if includeFull { out += t.fullItems }
        return out
    }

    private let forbidden = ["verified safe", "secure pc", "safe pc", "안전함", "검증됨", "안전한 pc", "안전한 컴퓨터"]

    func testScreenAndUiStringsHaveNoForbiddenWords() {
        for t in [NoticeResource.ko, NoticeResource.en] {
            for s in allStrings(t, includeFull: false) {
                let lower = s.lowercased()
                for word in forbidden { XCTAssertFalse(lower.contains(word), "'\(word)' in: \(s)") }
                XCTAssertFalse(lower.contains("safe"), "bare 'safe' on screen text: \(s)")
            }
        }
    }

    /// The full notice may mention the words only to say a device result is NOT a guarantee (item 4, quoted).
    func testFullNoticeUsesForbiddenWordsOnlyInTheNegatedItem() {
        for t in [NoticeResource.ko, NoticeResource.en] {
            for (index, item) in t.fullItems.enumerated() where index != 3 {
                let lower = item.lowercased()
                for word in forbidden { XCTAssertFalse(lower.contains(word), "'\(word)' in item \(index + 1)") }
            }
        }
        XCTAssertTrue(NoticeResource.ko.fullItems[3].contains("\"안전함\", \"검증됨\"으로 해석하지 마세요"))
        XCTAssertTrue(NoticeResource.en.fullItems[3].contains("do not read it as \"safe\" or \"verified\""))
    }

    func testDraftBannerIsPresentInBothLanguages() {
        XCTAssertTrue(NoticeResource.ko.draftBanner.contains("초안") && NoticeResource.ko.draftBanner.contains("법무"))
        XCTAssertTrue(NoticeResource.en.draftBanner.lowercased().contains("draft")
                      && NoticeResource.en.draftBanner.lowercased().contains("legally reviewed"))
    }

    func testLanguageSelectionFallsBackToEnglish() {
        XCTAssertEqual(NoticeLanguage.current(preferred: ["ko-KR"]), .ko)
        XCTAssertEqual(NoticeLanguage.current(preferred: ["ko"]), .ko)
        XCTAssertEqual(NoticeLanguage.current(preferred: ["ja-JP"]), .en)
        XCTAssertEqual(NoticeLanguage.current(preferred: ["en-US"]), .en)
        XCTAssertEqual(NoticeLanguage.current(preferred: []), .en)
    }

    func testBuiltInSourceReturnsBothLanguages() {
        let source = BuiltInNoticeTextSource()
        XCTAssertNotNil(source.text(for: .ko))
        XCTAssertNotNil(source.text(for: .en))
        XCTAssertEqual(RiskNotice.version, 1)
    }
}
