// Native package smoke on an isolated hosted macOS runner. Window identity is
// scoped to the launched executable; screenshot permission is recorded separately.
import Foundation
import CoreGraphics

let input = CommandLine.arguments
guard input.count == 3 else { fatalError("Usage: swift macos-desktop.swift app-bundle output-directory") }
let files = FileManager.default
let output = URL(fileURLWithPath: input[2], isDirectory: true)
guard !files.fileExists(atPath: output.path) else { fatalError("Output must be absent") }
try files.createDirectory(at: output, withIntermediateDirectories: false)
let bundle = output.appendingPathComponent("Compass.app")
try files.copyItem(at: URL(fileURLWithPath: input[1]), to: bundle)
let binaryDirectory = bundle.appendingPathComponent("Contents/MacOS")
let binaries = try files.contentsOfDirectory(at: binaryDirectory, includingPropertiesForKeys: nil)
guard binaries.count == 1 else { fatalError("Expected one bundle executable") }
try Data().write(to: binaryDirectory.appendingPathComponent("portable.txt"))
let application = Process()
application.executableURL = binaries[0]
application.currentDirectoryURL = binaryDirectory
application.environment = ProcessInfo.processInfo.environment
let log = output.appendingPathComponent("native.log")
files.createFile(atPath: log.path, contents: nil)
let handle = try FileHandle(forWritingTo: log)
application.standardOutput = handle
application.standardError = handle
let started = Date()
try application.run()
defer {
    if application.isRunning { application.terminate(); application.waitUntilExit() }
    try? handle.close()
}
var window: [String: Any]?
while Date().timeIntervalSince(started) < 30 {
    guard application.isRunning else { fatalError("Native app exited during startup") }
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    window = windows.first { row in
        (row[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == application.processIdentifier &&
        (row[kCGWindowLayer as String] as? NSNumber)?.intValue == 0
    }
    if window != nil { break }
    Thread.sleep(forTimeInterval: 0.2)
}
guard let observed = window, let number = observed[kCGWindowNumber as String] as? NSNumber else {
    fatalError("No on-screen window belonging to the launched macOS app")
}
let readyMs = Int(Date().timeIntervalSince(started) * 1000)
Thread.sleep(forTimeInterval: 5)
let capture = Process()
capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
capture.arguments = ["-x", "-l", number.stringValue, output.appendingPathComponent("native-window.png").path]
capture.standardOutput = handle
capture.standardError = handle
try capture.run()
capture.waitUntilExit()
let result: [String: Any] = [
    "platform": "macOS native app bundle",
    "nativeWindow": true,
    "ownerPid": application.processIdentifier,
    "windowTitle": observed[kCGWindowName as String] as? String ?? "unavailable",
    "windowBounds": observed[kCGWindowBounds as String] ?? [:],
    "windowReadyMs": readyMs,
    "screenshot": capture.terminationStatus == 0 ? "captured" : "unavailable",
    "scope": "native package startup and process-owned window; visual coverage depends on screenshot permission"
]
try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent("result.json"))
print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), encoding: .utf8)!)
