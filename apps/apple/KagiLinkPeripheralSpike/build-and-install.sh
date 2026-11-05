#!/bin/sh
# Builds the KL-V1 peripheral spike, signs it with the EXISTING AreaTM development certificate and
# an EXISTING wildcard development profile that already contains the device, then installs it.
# No certificate issuance, device registration, profile creation or upload. Synthetic data only.
# Usage: DEVICE_UDID=<udid> TEAM_ID=<team id> IDENTITY="<codesign identity>" sh build-and-install.sh [--launch]
# All three variables are required; there are deliberately no defaults (no personal identifiers in the repo).
set -eu
usage() {
    echo "Usage: DEVICE_UDID=<udid> TEAM_ID=<team id> IDENTITY=\"<codesign identity>\" sh $0 [--launch]" >&2
    exit 2
}
[ -n "${DEVICE_UDID-}" ] && [ -n "${TEAM_ID-}" ] && [ -n "${IDENTITY-}" ] || usage
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
core="$here/../KagiLinkCore"
device=$DEVICE_UDID
team=$TEAM_ID
identity=$IDENTITY
bundle_id=in.minori.key.spike.klv1
out=${OUT_DIR:-"$here/../../../target/apple-spike"}
[ -d "$core/Binary/KagiLinkC.xcframework" ] || { echo "Run KagiLinkCore/build-xcframework.sh first." >&2; exit 2; }
rm -rf "$out"; mkdir -p "$out/KLV1Spike.app"
hdr="$core/Binary/KagiLinkC.xcframework/ios-arm64/Headers"
xcrun --sdk iphoneos swiftc -swift-version 5 -parse-as-library -O -target arm64-apple-ios18.0 \
    -module-name KLV1Spike -I "$hdr" -Xcc "-fmodule-map-file=$hdr/module.modulemap" \
    "$here"/Sources/*.swift "$core/Sources/KagiLinkCore/Native.swift" "$core/Sources/KagiLinkCore/KagiLink.swift" \
    "$core/Binary/KagiLinkC.xcframework/ios-arm64/libareatm_kagilink_ffi.a" \
    -o "$out/KLV1Spike.app/KLV1Spike"
cp "$here/Info.plist" "$out/KLV1Spike.app/Info.plist"
# Pick an existing wildcard profile for the team that lists the device.
profile=$(python3 - "$team" "$device" <<'P'
import glob, os, plistlib, subprocess, sys
team, udid = sys.argv[1], sys.argv[2]
for path in sorted(glob.glob(os.path.expanduser("~/Library/Developer/Xcode/UserData/Provisioning Profiles/*.mobileprovision"))):
    try:
        raw = subprocess.run(["security", "cms", "-D", "-i", path], capture_output=True, check=True).stdout
        p = plistlib.loads(raw)
    except Exception:
        continue
    ent = p.get("Entitlements", {})
    if (team in p.get("TeamIdentifier", []) and ent.get("application-identifier", "").endswith(".*")
            and ent.get("get-task-allow") and udid in p.get("ProvisionedDevices", [])):
        print(path); break
P
)
[ -n "$profile" ] || { echo "No existing wildcard profile for $team containing the device. Nothing was created." >&2; exit 2; }
cp "$profile" "$out/KLV1Spike.app/embedded.mobileprovision"
cat > "$out/entitlements.plist" <<X
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>application-identifier</key><string>$team.$bundle_id</string>
    <key>com.apple.developer.team-identifier</key><string>$team</string>
    <key>get-task-allow</key><true/>
</dict></plist>
X
codesign --force --sign "$identity" --entitlements "$out/entitlements.plist" "$out/KLV1Spike.app"
echo "Profile: $(basename "$profile") (existing)"
xcrun devicectl device install app --device "$device" "$out/KLV1Spike.app"
[ "${1-}" = "--launch" ] && xcrun devicectl device process launch --device "$device" --terminate-existing --console "$bundle_id"
exit 0
