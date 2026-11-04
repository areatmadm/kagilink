"""Source/project consistency checks only; NOT native compilation/device evidence."""
import json
import plistlib
import re
import unittest
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APPLE = ROOT / "apps/apple"
ANDROID_NS = "{http://schemas.android.com/apk/res/android}"


def read(relative):
    return (ROOT / relative).read_text(encoding="utf-8")


def combined_source(relative, suffix):
    return "\n".join(path.read_text(encoding="utf-8") for path in
                     sorted((ROOT / relative).rglob("*" + suffix)))


def openstep(text):
    """Parse the checked-in ASCII Xcode project without executing any project code."""
    text = re.sub(r"//[^\n]*|/\*.*?\*/", "", text, flags=re.S)
    tokens = iter(re.findall(r'"(?:\\.|[^"\\])*"|[{}()=;,]|[^\s{}()=;,]+', text))
    current = next(tokens, None)

    def take(expected=None):
        nonlocal current
        value = current
        if value is None or (expected is not None and value != expected):
            raise ValueError(f"Expected {expected}, got {value}")
        current = next(tokens, None)
        return value

    def value():
        if current == "{":
            take("{")
            result = {}
            while current != "}":
                key = scalar()
                take("=")
                if key in result:
                    raise ValueError(f"Duplicate project key: {key}")
                result[key] = value()
                take(";")
            take("}")
            return result
        if current == "(":
            take("(")
            result = []
            while current != ")":
                result.append(value())
                if current != ")":
                    take(",")
            take(")")
            return result
        return scalar()

    def scalar():
        token = take()
        return json.loads(token) if token.startswith('"') else token

    result = value()
    if current is not None:
        raise ValueError("Trailing project tokens")
    return result


class NativeProbeContract(unittest.TestCase):
    def test_shared_diagnostic_identifiers_and_default_mtu_budget(self):
        fixture = json.loads(read("tests/vectors/phase0-ble.json"))
        for name in ("service", "characteristic"):
            self.assertEqual(str(uuid.UUID(fixture[name])), fixture[name])
        self.assertNotEqual(fixture["service"], fixture["characteristic"])
        for name in ("request", "response"):
            self.assertEqual(len(fixture[name].encode("ascii")), 16)
        for relative, source in (
            ("Windows", read("apps/windows/ble-probe.h")),
            ("Android", combined_source("apps/android/app/src/main/java", ".kt")),
            ("Apple", combined_source("apps/apple/Shared", ".swift")),
        ):
            for name in ("service", "characteristic", "request", "response"):
                self.assertIn('"' + fixture[name] + '"', source, (relative, name))

    def test_android_has_only_advertising_connect_permissions_and_no_backup(self):
        manifest = ET.fromstring(read("apps/android/app/src/main/AndroidManifest.xml"))
        permissions = {p.attrib[ANDROID_NS + "name"] for p in manifest.findall("uses-permission")}
        self.assertEqual(permissions, {
            "android.permission.BLUETOOTH_ADVERTISE", "android.permission.BLUETOOTH_CONNECT"
        })
        application = manifest.find("application")
        self.assertEqual(application.attrib[ANDROID_NS + "allowBackup"], "false")
        self.assertEqual(application.find("activity").attrib[ANDROID_NS + "name"], ".MainActivity")
        self.assertFalse(application.findall("service"))

    def test_android_support_floor_stable_sdk_and_pinned_build_tools(self):
        app = read("apps/android/app/build.gradle.kts")
        self.assertRegex(app, r"minSdk\s*=\s*34\b")
        self.assertRegex(app, r"targetSdk\s*=\s*37\b")
        self.assertRegex(app, r"release\(37\)")
        self.assertRegex(app, r"minorApiLevel\s*=\s*2\b")
        self.assertRegex(app, r'buildToolsVersion\s*=\s*"37\.0\.0"')
        self.assertRegex(read("apps/android/build.gradle.kts"),
                         r'id\("com.android.application"\)\s+version\s+"9\.5\.0"')
        self.assertIn("9.8.2", read("apps/android/build-probe.ps1"))

    def test_android_locked_app_runtime_has_no_extra_sdks(self):
        lines = read("apps/android/app/gradle.lockfile").splitlines()
        for configuration in ("debugRuntimeClasspath", "releaseRuntimeClasspath"):
            packages = set()
            for line in lines:
                if not line or line.startswith("#") or "=" not in line:
                    continue
                coordinate, configurations = line.split("=", 1)
                if configuration in configurations.split(","):
                    group, artifact, version = coordinate.split(":")
                    self.assertRegex(version, r"^\d+(\.\d+)+$")
                    packages.add((group, artifact))
            self.assertEqual(packages, {
                ("org.jetbrains.kotlin", "kotlin-stdlib"),
                ("org.jetbrains", "annotations"),
            }, configuration)

    def test_apple_privacy_plists_and_bluetooth_only_sandbox(self):
        for filename in ("iOS-Info.plist", "macOS-Info.plist"):
            info = plistlib.loads((APPLE / "Config" / filename).read_bytes())
            self.assertTrue(info["NSBluetoothAlwaysUsageDescription"])
            self.assertNotIn("UIBackgroundModes", info)
        entitlements = plistlib.loads((APPLE / "Config/macOS.entitlements").read_bytes())
        self.assertEqual(entitlements, {
            "com.apple.security.app-sandbox": True,
            "com.apple.security.device.bluetooth": True,
        })

    def test_xcode_targets_references_sources_and_unsigned_build_settings(self):
        project = openstep((APPLE / "KagiLinkPhase0.xcodeproj/project.pbxproj").read_text(encoding="utf-8"))
        objects = project["objects"]
        root = objects[project["rootObject"]]
        targets = [objects[item] for item in root["targets"]]
        self.assertEqual({t["name"] for t in targets}, {"KagiLink-iOS-Phase0", "KagiLink-macOS-Phase0"})
        for target in targets:
            self.assertIn(target["productReference"], objects)
            phases = [objects[item] for item in target["buildPhases"]]
            source_phase = next(p for p in phases if p["isa"] == "PBXSourcesBuildPhase")
            self.assertTrue(source_phase["files"])
            source_names = {objects[objects[item]["fileRef"]]["path"] for item in source_phase["files"]}
            self.assertIn("KagiLinkPhase0App.swift", source_names)
            self.assertTrue(all(name.endswith(".swift") for name in source_names))
            configs = objects[target["buildConfigurationList"]]["buildConfigurations"]
            for config_id in configs:
                settings = objects[config_id]["buildSettings"]
                self.assertEqual(settings["CODE_SIGNING_ALLOWED"], "NO")
                self.assertTrue((APPLE / settings["INFOPLIST_FILE"]).is_file())
                if target["name"] == "KagiLink-iOS-Phase0":
                    self.assertEqual(settings["IPHONEOS_DEPLOYMENT_TARGET"], "18.0")
                    self.assertEqual(settings["SDKROOT"], "iphoneos27.1")
        def check_group(group_id, directory):
            group = objects[group_id]
            directory = directory / group.get("path", "")
            for child_id in group["children"]:
                child = objects[child_id]
                if child["isa"] == "PBXGroup":
                    check_group(child_id, directory)
                elif child.get("sourceTree") != "BUILT_PRODUCTS_DIR":
                    self.assertTrue((directory / child["path"]).is_file(), child["path"])
        check_group(root["mainGroup"], APPLE)

    def test_apple_glass_availability_and_sdk_requirement_are_explicit(self):
        source = combined_source("apps/apple/Shared", ".swift")
        self.assertIn("glassEffect", source)
        self.assertRegex(source, r"#available\(iOS 26(?:\.0)?[,)]")
        self.assertRegex(source, r"\.(?:ultraThinMaterial|thinMaterial|regularMaterial)")
        self.assertIn("accessibilityReduceTransparency", source)
        self.assertIn("iphoneos27.1", read("apps/apple/build-probes.sh"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
