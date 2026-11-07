#!/bin/sh
# Real-device UI test of the risk acknowledgement screen (D023) through adb + uiautomator dumps.
# Requires: ADB (path to adb), a connected device with the debug build installed, python3.
# It clears the app's data and grants the Nearby-devices permissions to the test app. Synthetic data only;
# no session is started. Usage: ADB=<adb> sh ui-test-risk-ack.sh
set -eu
: "${ADB:?set ADB}"
PKG=com.areatm.kagilink.spike.peripheral
ID=$PKG:id
fail=0; n=0
ok() { n=$((n+1)); if [ "$1" = 0 ]; then echo "PASS: $2"; else echo "FAIL: $2"; fail=$((fail+1)); fi; }
dump() { "$ADB" shell uiautomator dump /sdcard/kl_ui.xml >/dev/null 2>&1; "$ADB" shell cat /sdcard/kl_ui.xml; }
# attr <xml-file> <resource-id suffix> <attribute> -> value, or MISSING when the node is not on screen
attr() { python3 - "$1" "$ID/$2" "$3" <<'PY'
import sys, xml.etree.ElementTree as ET
t = ET.parse(sys.argv[1])
for n in t.iter("node"):
    if n.get("resource-id") == sys.argv[2]:
        print(n.get(sys.argv[3])); break
else:
    print("MISSING")
PY
}
center() { python3 - "$1" "$ID/$2" <<'PY'
import re, sys, xml.etree.ElementTree as ET
for n in ET.parse(sys.argv[1]).iter("node"):
    if n.get("resource-id") == sys.argv[2]:
        a, b, c, d = map(int, re.findall(r"\d+", n.get("bounds")))
        print((a + c) // 2, (b + d) // 2); break
PY
}
tap() { dump > "$T/ui.xml"; set -- $(center "$T/ui.xml" "$1"); "$ADB" shell input tap "$1" "$2"; sleep 1; }
T=$(mktemp -d); trap 'rm -rf "$T"; "$ADB" shell rm -f /sdcard/kl_ui.xml' EXIT
snap() { dump > "$T/ui.xml"; }

"$ADB" shell am force-stop $PKG; "$ADB" shell pm clear $PKG >/dev/null
for p in BLUETOOTH_ADVERTISE BLUETOOTH_CONNECT; do "$ADB" shell pm grant $PKG android.permission.$p; done
"$ADB" shell input keyevent KEYCODE_WAKEUP; "$ADB" shell am start -n $PKG/.PeripheralActivity >/dev/null; sleep 3

snap
[ "$(attr "$T/ui.xml" risk_ack_box class)" != MISSING ]; ok $? "notice is shown first (before code entry / Start)"
[ "$(attr "$T/ui.xml" start_form class)" = MISSING ]; ok $? "start form (code entry, Start) is hidden before acknowledgement"
[ "$(attr "$T/ui.xml" risk_ack_checkbox checked)" = false ]; ok $? "checkbox starts unchecked"
[ "$(attr "$T/ui.xml" risk_ack_continue enabled)" = false ]; ok $? "continue is disabled while unchecked"

tap risk_ack_checkbox; snap
[ "$(attr "$T/ui.xml" risk_ack_checkbox checked)" = true ]; ok $? "checkbox can be checked"
[ "$(attr "$T/ui.xml" risk_ack_continue enabled)" = true ]; ok $? "continue is enabled after checking"
tap risk_ack_checkbox; snap
[ "$(attr "$T/ui.xml" risk_ack_continue enabled)" = false ]; ok $? "unchecking disables continue again"

tap risk_ack_full_button; snap
grep -q "risk notice and disclaimer\|위험 고지 및 면책" "$T/ui.xml"; ok $? "full notice is one tap away (title visible)"
tap risk_ack_full_button

tap risk_ack_checkbox; tap risk_ack_continue; snap
[ "$(attr "$T/ui.xml" start_form class)" != MISSING ]; ok $? "after acknowledgement the start form appears"
[ "$(attr "$T/ui.xml" risk_ack_box class)" = MISSING ]; ok $? "the notice is hidden after acknowledgement"

"$ADB" shell input keyevent KEYCODE_BACK; sleep 1; snap
[ "$(attr "$T/ui.xml" risk_ack_box class)" != MISSING ]; ok $? "back returns to the notice"
[ "$(attr "$T/ui.xml" risk_ack_checkbox checked)" = false ]; ok $? "back leaves no checked state"
[ "$(attr "$T/ui.xml" risk_ack_continue enabled)" = false ]; ok $? "back leaves continue disabled"
[ "$(attr "$T/ui.xml" start_form class)" = MISSING ]; ok $? "back hides the start form again"

tap risk_ack_checkbox; tap risk_ack_continue
"$ADB" shell input keyevent KEYCODE_HOME; sleep 1
"$ADB" shell am start -n $PKG/.PeripheralActivity >/dev/null; sleep 2; snap
[ "$(attr "$T/ui.xml" risk_ack_box class)" != MISSING ] && [ "$(attr "$T/ui.xml" start_form class)" = MISSING ]; ok $? "backgrounding clears the acknowledgement"
[ "$(attr "$T/ui.xml" risk_ack_checkbox checked)" = false ]; ok $? "after backgrounding the checkbox is unchecked"

"$ADB" shell am force-stop $PKG
echo "ui-test-risk-ack: $n checks, $fail failures"
[ "$fail" = 0 ]
