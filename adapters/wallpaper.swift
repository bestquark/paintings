import AppKit
import Foundation

guard CommandLine.arguments.count >= 2 else { exit(2) }
let path = CommandLine.arguments[1]
let url = URL(fileURLWithPath: path)
let fit = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "contain"
let options: [NSWorkspace.DesktopImageOptionKey: Any] = [
    .imageScaling: NSImageScaling.scaleProportionallyUpOrDown.rawValue,
    .allowClipping: fit == "cover",
    .fillColor: NSColor(calibratedWhite: 0.055, alpha: 1)
]
var screens: [[String: String]] = []
for screen in NSScreen.screens {
    do {
        if path != "--inspect" { try NSWorkspace.shared.setDesktopImageURL(url, for: screen, options: options) }
        if path != "--inspect" { Thread.sleep(forTimeInterval: 0.4) }
        screens.append(["screen": screen.localizedName,
                        "url": NSWorkspace.shared.desktopImageURL(for: screen)?.path ?? ""])
    } catch { fputs("\(error)\n", stderr); exit(1) }
}
let data = try! JSONSerialization.data(withJSONObject: screens, options: [.sortedKeys])
print(String(data: data, encoding: .utf8)!)
if screens.isEmpty { exit(3) }
